#!/usr/bin/env python3
"""S17 virtual mTLS/WADM service-boundary contract.

Generates a disposable test CA, server and client certificates, starts the
real ``remote-admin-tls-service`` in its local journal mode, and drives Query,
Apply and Rollback WADM frames over mutually authenticated TLS. This closes
the service-side policy-authority and client-identity binding virtually.

A Ring-3 guest listener is NOT claimed: the kernel has no listen/accept
syscall, so guest-hosted TLS termination needs a new ABI and is a separate
increment. No production key, host service or physical device is used.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import os
import socket
import ssl
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
HEADER = 26
MAGIC = b"WADM"
VERSION = 1
COMMANDS = {"apply": 1, "rollback": 2, "query": 3}


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(command, capture_output=True, text=True, check=False, **kwargs)


def certs(out: Path, openssl: Path) -> dict[str, Path]:
    ext = out / "san.ext"
    ext.write_text("subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\n", encoding="utf-8")
    client_ext = out / "client.ext"
    client_ext.write_text(
        "extendedKeyUsage=clientAuth\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature\n",
        encoding="utf-8",
    )
    steps = [
        ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", str(out / "ca.key"),
         "-out", str(out / "ca.pem"), "-days", "1", "-subj", "/CN=WovenHat Virtual Admin CA",
         "-addext", "keyUsage=critical,keyCertSign,cRLSign", "-sha256"],
        ["req", "-newkey", "rsa:2048", "-nodes", "-keyout", str(out / "server.key"),
         "-out", str(out / "server.csr"), "-subj", "/CN=localhost", "-sha256"],
        ["x509", "-req", "-in", str(out / "server.csr"), "-CA", str(out / "ca.pem"),
         "-CAkey", str(out / "ca.key"), "-CAcreateserial", "-out", str(out / "server.pem"),
         "-days", "1", "-extfile", str(ext), "-sha256"],
        ["x509", "-req", "-in", str(out / "client.csr"), "-CA", str(out / "ca.pem"),
         "-CAkey", str(out / "ca.key"), "-CAcreateserial", "-out", str(out / "client.pem"),
         "-days", "1", "-extfile", str(client_ext), "-sha256"],
        ["x509", "-in", str(out / "server.pem"), "-outform", "DER", "-out", str(out / "server.der")],
        ["pkcs8", "-topk8", "-nocrypt", "-in", str(out / "server.key"),
         "-outform", "DER", "-out", str(out / "server-key.der")],
        ["x509", "-in", str(out / "ca.pem"), "-outform", "DER", "-out", str(out / "ca.der")],
        ["x509", "-in", str(out / "client.pem"), "-outform", "DER", "-out", str(out / "client.der")],
    ]
    # The client CSR is generated first so x509 -req can sign it.
    steps.insert(3, ["req", "-newkey", "rsa:2048", "-nodes", "-keyout", str(out / "client.key"),
                     "-out", str(out / "client.csr"), "-subj", "/CN=wovenhat-admin", "-sha256"])
    for step in steps:
        result = run([str(openssl), *step])
        if result.returncode != 0:
            raise RuntimeError(f"openssl {' '.join(step[:2])} failed: {result.stderr.strip()}")
    return {name: out / name for name in
            ("server.der", "server-key.der", "ca.der", "ca.pem", "client.der", "client.pem", "client.key")}


def frame(command: str, nonce: int, generation: int, payload: bytes) -> bytes:
    head = (MAGIC + bytes([VERSION, COMMANDS[command], 0, 0])
            + nonce.to_bytes(8, "big") + generation.to_bytes(8, "big")
            + len(payload).to_bytes(2, "big"))
    return head + payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openssl", type=Path, default=Path("C:/Program Files/Git/usr/bin/openssl.exe"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    out = args.output or ROOT / "audit-artifacts" / f"remote-admin-virtual-{time.time_ns()}"
    out.mkdir(parents=True)
    errors: list[str] = []
    service = None
    try:
        material = certs(out, args.openssl)
        fingerprint = hashlib.sha256(material["client.der"].read_bytes()).hexdigest()
        service_dir = ROOT / "tools" / "remote-admin-tls-service"
        build = run(["cargo", "build", "--quiet", "--manifest-path", str(service_dir / "Cargo.toml")], cwd=ROOT)
        (out / "build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
        if build.returncode:
            raise RuntimeError("service build failed")
        target = Path(os.environ.get("CARGO_TARGET_DIR") or (service_dir / "target"))
        executable = target / "debug" / "wovenhat-remote-admin-tls-service"
        if not executable.is_file():
            executable = executable.with_suffix(".exe")
        if not executable.is_file():
            raise RuntimeError(f"service executable not found: {executable}")
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        service = subprocess.Popen(
            [str(executable), f"127.0.0.1:{port}", str(material["server.der"]),
             str(material["server-key.der"]), str(material["ca.der"]), fingerprint],
            cwd=ROOT, stdout=(out / "service.log").open("w"), stderr=subprocess.STDOUT,
        )
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline and "listener ready" not in (out / "service.log").read_text(encoding="utf-8", errors="replace"):
            time.sleep(0.1)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(str(material["ca.pem"]))
        context.load_cert_chain(str(material["client.pem"]), str(material["client.key"]))
        context.check_hostname = True
        context.maximum_version = ssl.TLSVersion.TLSv1_2
        with socket.create_connection(("127.0.0.1", port), timeout=10) as raw:
            with context.wrap_socket(raw, server_hostname="localhost") as tls:
                exchanges = []
                for command, nonce, generation, payload in (
                    ("query", 1, 0, b""),
                    ("apply", 1, 1, b"virtual-policy-v1"),
                    ("query", 2, 0, b""),
                    ("rollback", 2, 2, b""),
                    ("query", 3, 0, b""),
                ):
                    tls.sendall(frame(command, nonce, generation, payload))
                    response = tls.recv(16)
                    if len(response) != 16 or response[:4] != b"WADR":
                        raise RuntimeError(f"bad WADR for {command}: {response.hex()}")
                    status = response[5]
                    response_generation = int.from_bytes(response[8:16], "big")
                    if status != 0:
                        raise RuntimeError(f"{command} returned status {status}")
                    exchanges.append({"command": command, "generation": response_generation})
    except (OSError, RuntimeError) as error:
        errors.append(str(error))
    finally:
        if service is not None:
            service.terminate()
            try:
                service.wait(timeout=5)
            except subprocess.TimeoutExpired:
                service.kill()
    generations = [item["generation"] for item in locals().get("exchanges", [])]
    if len(generations) >= 4 and not (generations[2] > generations[0] and generations[3] > generations[2]):
        errors.append("journal generations did not advance on apply and rollback")
    result = {
        "schema": "wovenhat.remote-admin-virtual/v1",
        "pass": not errors,
        "mtls": True,
        "expected_client_fingerprint": locals().get("fingerprint", ""),
        "guest_listener": False,
        "guest_listener_blocker": "kernel has no listen/accept syscall",
        "exchanges": locals().get("exchanges", []),
        "errors": errors,
    }
    (out / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(f"Virtual admin gate FAILED; evidence: {out}", file=sys.stderr)
        return 1
    print(f"Virtual admin gate PASS: mTLS + WADM journal. Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
