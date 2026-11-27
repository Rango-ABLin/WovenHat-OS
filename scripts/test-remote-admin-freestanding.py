"""Exercise the freestanding WADM service through a host mTLS proxy."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import struct
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
TEST_PUBLIC_KEY = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"


def reserve_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def run(command, *, cwd=ROOT, env=None, capture=False):
    return subprocess.run(
        command, cwd=cwd, env=env, check=True, capture_output=capture, text=capture
    )


def request(command, nonce, generation, payload=b""):
    return (
        b"WADM"
        + bytes((1, command, 0, 0))
        + struct.pack(">QQH", nonce, generation, len(payload))
        + payload
    )


def exchange(stream, frame):
    stream.sendall(frame)
    response = bytearray()
    while len(response) < 16:
        chunk = stream.recv(16 - len(response))
        if not chunk:
            raise RuntimeError("remote-admin response truncated")
        response.extend(chunk)
    if response[:4] != b"WADR" or response[4] != 1:
        raise RuntimeError("invalid remote-admin response")
    return (
        response[5],
        int.from_bytes(response[6:8], "big"),
        int.from_bytes(response[8:16], "big"),
    )


def generate_certificates(directory: Path, openssl: Path):
    def openssl_run(*args):
        run([str(openssl), *args], cwd=directory)

    openssl_run("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=WovenHat Test CA",
                "-addext", "basicConstraints=critical,CA:TRUE",
                "-addext", "keyUsage=critical,keyCertSign,cRLSign",
                "-keyout", "ca-key.pem", "-out", "ca.pem", "-days", "1")
    for name, cn in (("server", "localhost"), ("client", "release-operator")):
        extensions = "serverAuth" if name == "server" else "clientAuth"
        extfile = directory / f"{name}.ext"
        extfile.write_text(
            f"basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\n"
            f"extendedKeyUsage={extensions}\nsubjectAltName=DNS:localhost\n",
            encoding="utf-8",
        )
        openssl_run("req", "-newkey", "rsa:2048", "-nodes", "-subj", f"/CN={cn}",
                    "-keyout", f"{name}-key.pem", "-out", f"{name}.csr")
        openssl_run("x509", "-req", "-in", f"{name}.csr", "-CA", "ca.pem",
                    "-CAkey", "ca-key.pem", "-CAcreateserial", "-out", f"{name}.pem",
                    "-days", "1", "-extfile", str(extfile.name))
        openssl_run("x509", "-in", f"{name}.pem", "-outform", "DER",
                    "-out", f"{name}.der")
    openssl_run("pkcs8", "-topk8", "-nocrypt", "-in", "server-key.pem",
                "-outform", "DER", "-out", "server-key.der")
    openssl_run("x509", "-in", "ca.pem", "-outform", "DER", "-out", "ca.der")
    return {
        "server_der": directory / "server.der",
        "server_key_der": directory / "server-key.der",
        "ca_der": directory / "ca.der",
        "client_der": directory / "client.der",
        "client_pem": directory / "client.pem",
        "client_key": directory / "client-key.pem",
        "ca_pem": directory / "ca.pem",
    }


def wait_for(path: Path, marker: str, process: subprocess.Popen, timeout: float):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = path.read_text(errors="replace") if path.exists() else ""
        if marker in text:
            return
        if process.poll() is not None:
            raise RuntimeError(
                f"process exited {process.returncode} before {marker!r}"
            )
        time.sleep(0.05)
    raise TimeoutError(f"timed out waiting for {marker!r}")


def wait_for_count(path: Path, marker: str, count: int, process: subprocess.Popen, timeout: float):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = path.read_text(errors="replace") if path.exists() else ""
        if text.count(marker) >= count:
            return
        if process.poll() is not None:
            raise RuntimeError(
                f"process exited {process.returncode} before {count}x {marker!r}"
            )
        time.sleep(0.05)
    raise TimeoutError(f"timed out waiting for {count}x {marker!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--qemu", type=Path, default=Path(shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe"))
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--openssl", type=Path, default=Path(shutil.which("openssl") or r"C:\Program Files\Git\usr\bin\openssl.exe"))
    args = parser.parse_args()
    firmware = args.firmware or args.qemu.parent / "share" / "edk2-x86_64-code.fd"
    if not args.qemu.is_file() or not firmware.is_file() or not args.openssl.is_file():
        parser.error("qemu, OVMF CODE and OpenSSL are required")

    with tempfile.TemporaryDirectory(prefix="wovenhat-remote-admin-") as temporary:
        temp = Path(temporary)
        certs = generate_certificates(temp, args.openssl)
        app = ROOT / "apps" / "woven-remote-admin"
        app_env = dict(os.environ)
        app_env.setdefault("CARGO_BUILD_BUILD_DIR", str(ROOT / ".cargo-build"))
        app_env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
        app_env.pop("CARGO_BUILD_BUILD_DIR", None)
        run(["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
            cwd=app, env=app_env)
        app_elf = Path(app_env["CARGO_TARGET_DIR"]) / "x86_64-unknown-none" / "release" / "wovenhat-remote-admin"
        package = temp / "woven-remote-admin.wapp"
        package_env = dict(os.environ)
        package_env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target" / "wapp-packager"))
        package_env.pop("CARGO_BUILD_BUILD_DIR", None)
        run(["cargo", "run", "--quiet", "--manifest-path",
             str(ROOT / "tools" / "wapp-packager" / "Cargo.toml"), "--",
             str(app_elf), str(package)], env=package_env)
        build_env = dict(
            os.environ,
            WOVENHAT_REMOTE_ADMIN_IMAGE=str(package),
        )
        build_env.setdefault("CARGO_BUILD_BUILD_DIR", str(ROOT / ".cargo-build"))
        build_env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
        build = run(["cargo", "run", "--quiet", "--features", "remote-admin-test",
                     "--", "--print-image"], env=build_env, capture=True)
        image = Path([line for line in build.stdout.splitlines() if line.endswith(".img")][-1])
        guest_forward = reserve_port()
        tls_port = reserve_port()
        out = ROOT / "audit-artifacts" / (
            f"remote-admin-freestanding-{args.cpus}cpu-{time.time_ns()}"
        )
        out.mkdir(parents=True, exist_ok=True)
        serial = out / "serial.log"
        qemu_log = out / "qemu.log"
        serial.write_text("")
        qemu_command = [
            str(args.qemu), "-accel", "tcg,tb-size=128", "-machine", "q35",
            "-m", "2G", "-smp", str(args.cpus), "-display", "none",
            "-serial", f"file:{serial}", "-no-reboot",
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={firmware}",
            "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}",
            "-device", "virtio-blk-pci,drive=boot,bootindex=1",
            "-netdev", f"user,id=net0,hostfwd=tcp:127.0.0.1:{guest_forward}-10.0.2.15:7001",
            "-device", "e1000e,netdev=net0",
        ]
        with qemu_log.open("w") as log:
            qemu = subprocess.Popen(qemu_command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        proxy_log = out / "proxy.log"
        try:
            wait_for(serial, "[REMOTE ADMIN] deployed freestanding service", qemu, args.timeout)
            wait_for(serial, "[LISTEN] accept submitted", qemu, args.timeout)
            client_fingerprint = hashlib.sha256(certs["client_der"].read_bytes()).hexdigest()
            proxy_args = [
                "cargo", "run", "--quiet", "--manifest-path",
                str(ROOT / "tools" / "remote-admin-tls-service" / "Cargo.toml"), "--",
                f"127.0.0.1:{tls_port}", str(certs["server_der"]),
                str(certs["server_key_der"]), str(certs["ca_der"]),
                client_fingerprint, f"127.0.0.1:{guest_forward}",
            ]
            with proxy_log.open("w") as log:
                proxy = subprocess.Popen(proxy_args, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            wait_for(proxy_log, "remote-admin TLS listener ready", proxy, 60)
            context = ssl.create_default_context(cafile=str(certs["ca_pem"]))
            context.load_cert_chain(certs["client_pem"], certs["client_key"])
            # TCG emulation cost grows with the guest CPU count; keep the
            # accepted small-profile budget and scale only wider profiles.
            socket_timeout = 15 * max(1, args.cpus // 4)
            with socket.create_connection(("127.0.0.1", tls_port), timeout=socket_timeout) as raw:
                with context.wrap_socket(raw, server_hostname="localhost") as stream:
                    assert exchange(stream, request(3, 1, 1)) == (0, 4, 1)
                    assert exchange(stream, request(1, 2, 1, b"locked")) == (0, 6, 2)
                    assert exchange(stream, request(3, 3, 2)) == (0, 6, 2)
                    assert exchange(stream, request(2, 4, 2)) == (0, 4, 3)
                    assert exchange(stream, request(1, 4, 3, b"replay"))[0] == 1
            wait_for(serial, "[LISTEN] accept complete", qemu, 30)
            # The first client is gone. The kernel re-arms the same listening
            # descriptor after its FIN, and the guest service waits on accept
            # again; a fresh query must observe the journal the first client
            # left behind. This is a sequential multi-client lifecycle, not a
            # fabricated simultaneous backlog.
            # Wait for the kernel to re-arm the listener, not for the guest's
            # accept to complete: accept only completes once a client arrives.
            wait_for(serial, "[LISTEN] listener re-armed on port 7001", qemu, 30)
            with socket.create_connection(("127.0.0.1", tls_port), timeout=socket_timeout) as raw2:
                with context.wrap_socket(raw2, server_hostname="localhost") as stream2:
                    assert exchange(stream2, request(3, 6, 3)) == (0, 4, 3)
            wait_for_count(serial, "[LISTEN] accept complete", 2, qemu, 30)
            (out / "result.json").write_text(json.dumps({
                "valid": True,
                "cpus": args.cpus,
                "transport": "mTLS host boundary -> QEMU TCP -> Ring-3 listen/accept ABI service",
                "listener_abi": True,
                "multi_client": True,
                "clients": 2,
                "operations": ["query", "apply", "query", "rollback", "replay-rejection",
                               "second-client-query"],
                "physical_access": "none",
            }, indent=2) + "\n", encoding="utf-8")
        finally:
            if "proxy" in locals():
                proxy.terminate()
                proxy.wait(timeout=10)
            qemu.terminate()
            qemu.wait(timeout=30)
    print(f"Remote admin freestanding deployment: PASS ({args.cpus} CPU). Evidence: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
