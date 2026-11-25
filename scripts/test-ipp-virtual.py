#!/usr/bin/env python3
"""Close the S13 IPP software slice with a local virtual TLS printer.

Generates a disposable test CA and a server certificate whose SAN matches the
probe URI hostname, serves one bounded IPP Get-Printer-Attributes response over
Rustls-compatible TLS, and requires the real ``ipp-tls-probe`` to accept it with
an explicit CA DER and ``--connect-address``. No printer, DNS change, hosts-file
edit or external certificate is involved.

This validates the transport, certificate and IPP wire contract only. It does
not claim a certified printer, an issuing CA, or native network qualification.
"""

from __future__ import annotations

import argparse
import json
import os
import socketserver
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
HOSTNAME = "localhost"
STATUS_OK = 0x0000
REQUEST_ID = 1
EXPECTED_REQUEST_PREFIX = bytes([1, 1, 0, 0x0B])


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(command, capture_output=True, text=True, check=False, **kwargs)


def generate_certificates(out: Path, openssl: Path) -> dict[str, Path]:
    ext = out / "server.ext"
    ext.write_text("subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\n", encoding="utf-8")
    steps = [
        ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", str(out / "ca.key"),
         "-out", str(out / "ca.pem"), "-days", "1", "-subj", "/CN=WovenHat Virtual IPP CA",
         "-sha256"],
        ["req", "-newkey", "rsa:2048", "-nodes", "-keyout", str(out / "server.key"),
         "-out", str(out / "server.csr"), "-subj", f"/CN={HOSTNAME}", "-sha256"],
        ["x509", "-req", "-in", str(out / "server.csr"), "-CA", str(out / "ca.pem"),
         "-CAkey", str(out / "ca.key"), "-CAcreateserial", "-out", str(out / "server.pem"),
         "-days", "1", "-extfile", str(ext), "-sha256"],
        ["x509", "-in", str(out / "ca.pem"), "-outform", "DER", "-out", str(out / "ca.der")],
    ]
    for step in steps:
        result = run([str(openssl), *step])
        if result.returncode != 0:
            raise RuntimeError(f"openssl {' '.join(step[:2])} failed: {result.stderr.strip()}")
    return {name: out / name for name in ("ca.der", "server.pem", "server.key")}


class IppHandler(socketserver.BaseRequestHandler):
    def handle(self):
        stream = self.request
        head = bytearray()
        while b"\r\n\r\n" not in head and len(head) < 4096:
            chunk = stream.recv(4096)
            if not chunk:
                return
            head.extend(chunk)
        end = head.find(b"\r\n\r\n")
        if end < 0:
            return
        headers = bytes(head[:end]).decode("ascii", errors="replace")
        length = 0
        for line in headers.split("\r\n"):
            if line.lower().startswith("content-length:"):
                length = int(line.split(":", 1)[1].strip())
        if length < 0 or length > 4096:
            return
        body = bytearray(head[end + 4:])
        while len(body) < length:
            chunk = stream.recv(4096)
            if not chunk:
                break
            body.extend(chunk)
        self.server.requests.append(bytes(body[:length]))  # type: ignore[attr-defined]
        payload = bytes([1, 1, STATUS_OK >> 8, STATUS_OK & 0xFF,
                         0, 0, 0, REQUEST_ID])
        response = (
            b"HTTP/1.1 200 OK\r\n"
            b"Content-Type: application/ipp\r\n"
            + f"Content-Length: {len(payload)}\r\n".encode("ascii")
            + b"Connection: close\r\n\r\n" + payload
        )
        stream.sendall(response)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openssl", type=Path, default=Path("C:/Program Files/Git/usr/bin/openssl.exe"))
    parser.add_argument("--output", type=Path, help="evidence directory (default timestamped)")
    args = parser.parse_args()
    out = args.output or ROOT / "audit-artifacts" / f"ipp-virtual-{time.time_ns()}"
    out.mkdir(parents=True)
    errors: list[str] = []
    try:
        certs = generate_certificates(out, args.openssl)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(str(certs["server.pem"]), str(certs["server.key"]))
        server = socketserver.TCPServer(("127.0.0.1", 0), IppHandler)
        server.requests = []  # type: ignore[attr-defined]
        server.socket = context.wrap_socket(server.socket, server_side=True)
        port = server.server_address[1]
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        probe_dir = ROOT / "tools" / "ipp-tls-probe"
        build = run(["cargo", "build", "--quiet", "--manifest-path", str(probe_dir / "Cargo.toml")], cwd=ROOT)
        (out / "build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
        if build.returncode != 0:
            errors.append("probe build failed")
        else:
            target_dir = Path(os.environ.get("CARGO_TARGET_DIR") or (probe_dir / "target"))
            probe = target_dir / "debug" / "wovenhat-ipp-tls-probe"
            if not probe.is_file():
                probe = probe.with_suffix(".exe")
            if not probe.is_file():
                errors.append(f"probe executable not found at {probe}")
            else:
                result = run([
                    str(probe), f"https://{HOSTNAME}:{port}/ipp", "Virtual Printer",
                    str(certs["ca.der"]), "--connect-address", "127.0.0.1",
                ])
                (out / "probe.log").write_text(result.stdout + result.stderr, encoding="utf-8")
                if result.returncode != 0:
                    errors.append(f"probe exit {result.returncode}: {result.stderr.strip()}")
                expected = f"TLS IPP probe passed: status=0x{STATUS_OK:04x} request_id={REQUEST_ID}"
                if expected not in result.stdout:
                    errors.append(f"missing {expected!r}")
        server.shutdown()
        server.server_close()
        requests = server.requests
        if not requests:
            errors.append("virtual printer received no IPP request")
        elif not requests[0].startswith(EXPECTED_REQUEST_PREFIX):
            errors.append(f"unexpected IPP request prefix: {requests[0][:8].hex()}")
        (out / "request.bin").write_bytes(requests[0] if requests else b"")
    except (OSError, RuntimeError) as error:
        errors.append(str(error))
    result = {
        "schema": "wovenhat.ipp-virtual/v1",
        "pass": not errors,
        "hostname": HOSTNAME,
        "connect_address": "127.0.0.1",
        "ipp_status": STATUS_OK,
        "request_id": REQUEST_ID,
        "physical_device": False,
        "external_certificate": False,
        "errors": errors,
    }
    (out / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(f"Virtual IPP gate FAILED; evidence: {out}", file=sys.stderr)
        return 1
    print(f"Virtual IPP gate PASS: probe accepted local TLS printer. Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
