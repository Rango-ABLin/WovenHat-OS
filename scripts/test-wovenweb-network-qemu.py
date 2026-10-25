"""Exercise the native SDK through a real Ring-3 SOCKS/HTTP probe and QEMU.

Uses a local protocol fixture, NOT Tor or a VPN. Evidence includes serial logs,
peer transcripts and a packet capture checked for unintended IPv4 DNS traffic.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import threading
import time

BODY = b"<html><body><h1>WovenWeb native transport</h1></body></html>"
PORT = 18085
MARKER = "[WOVENWEB] native network platform: PASSED"


def exact(connection, count):
    result = b""
    while len(result) < count:
        block = connection.recv(count - len(result))
        if not block:
            raise RuntimeError("premature guest EOF")
        result += block
    return result


def serve(listener, errors, events, finished):
    try:
        listener.settimeout(30)
        with listener:
            for index in range(3):
                connection, _ = listener.accept()
                with connection:
                    connection.settimeout(15)
                    if exact(connection, 3) != bytes([5, 1, 0]):
                        raise RuntimeError("invalid native SOCKS greeting")
                    if index == 1:
                        connection.sendall(bytes([5, 2]))
                        if connection.recv(1) != b"":
                            raise RuntimeError("guest sent data after authentication refusal")
                        events.append("refused authentication: guest closed without CONNECT")
                        continue
                    if index == 2:
                        # Deliberately stall method selection. The guest deadline
                        # must cancel and close without any target CONNECT.
                        if connection.recv(1) != b"":
                            raise RuntimeError("guest sent data after a stalled method reply")
                        events.append("stalled SOCKS method: guest timed out and closed")
                        continue
                    for part in [bytes([5]), bytes([0])]:
                        connection.sendall(part)
                        time.sleep(0.005)
                    header = exact(connection, 5)
                    if header[:4] != bytes([5, 1, 0, 3]):
                        raise RuntimeError("guest did not request remote-name resolution")
                    host = exact(connection, header[4])
                    port = int.from_bytes(exact(connection, 2), "big")
                    if host != b"fixture.invalid" or port != 80:
                        raise RuntimeError("unexpected proxy destination")
                    # Domain-address reply, fragmented across header/length/address.
                    for part in [bytes([5, 0]), bytes([0, 3]), bytes([1, 120]), bytes([0, 80])]:
                        connection.sendall(part)
                        time.sleep(0.005)
                    request = b""
                    while not request.endswith(b"\r\n\r\n") and len(request) < 512:
                        request += exact(connection, 1)
                    if request != b"GET / HTTP/1.1\r\nHost: fixture.invalid\r\nConnection: close\r\n\r\n":
                        raise RuntimeError("unexpected HTTP request")
                    response = b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(BODY)).encode() + b"\r\n\r\n" + BODY
                    for start in range(0, len(response), 7):
                        connection.sendall(response[start:start + 7])
                        time.sleep(0.002)
                    connection.shutdown(socket.SHUT_WR)
                    if connection.recv(1) != b"":
                        raise RuntimeError("unexpected bytes after HTTP exchange")
                    events.append("remote-name CONNECT + fragmented HTTP response completed")
        finished.set()
    except Exception as error:
        errors.append(str(error))


def dns_packets(path):
    data = path.read_bytes()
    if len(data) < 24 or data[:4] != bytes.fromhex("d4c3b2a1"):
        raise RuntimeError("missing or unsupported packet capture")
    if struct.unpack_from("<I", data, 20)[0] != 1:
        raise RuntimeError("packet capture is not Ethernet")
    offset = 24
    count = 0
    tcp = 0
    while offset < len(data):
        if offset + 16 > len(data):
            raise RuntimeError("truncated capture record")
        size = struct.unpack_from("<I", data, offset + 8)[0]
        offset += 16
        frame = data[offset:offset + size]
        if len(frame) != size:
            raise RuntimeError("truncated captured packet")
        offset += size
        if len(frame) < 34 or frame[12:14] != bytes.fromhex("0800"):
            continue
        if frame[23] == 6:
            tcp += 1
        if frame[23] != 17:
            continue
        header_size = (frame[14] & 15) * 4
        udp = 14 + header_size
        if header_size >= 20 and len(frame) >= udp + 4:
            source, destination = struct.unpack_from("!HH", frame, udp)
            if source == 53 or destination == 53:
                count += 1
    return {"ipv4_dns_packets": count, "ipv4_tcp_packets": tcp}


def run_build(command, cwd, env, log):
    with log.open("w", encoding="utf-8") as errors:
        result = subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=errors, text=True)
    if result.returncode:
        raise RuntimeError(f"build failed: {log}\n{log.read_text(encoding='utf-8')[-4000:]}")
    return result.stdout.strip()



def image_from_cargo(output):
    paths = set()
    for line in output.splitlines():
        message = json.loads(line)
        if message.get("reason") == "build-script-executed":
            paths.update(value for key, value in message.get("env", []) if key == "WOVENHAT_UEFI_IMAGE")
    if len(paths) != 1:
        raise RuntimeError("Cargo did not emit exactly one WovenHat boot-image path")
    return Path(paths.pop())


def build_image(root, env, feature, log):
    # The host executable only prints this build-time value. Reading Cargo's
    # metadata avoids executing a generated Windows launcher unnecessarily.
    output = run_build(["cargo", "build", "--features", feature, "--message-format=json"], root, env, log)
    log.with_suffix(".cargo.jsonl").write_text(output, encoding="utf-8")
    return image_from_cargo(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--image", type=Path, help="reuse an already built probe boot image")
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    out = root / "audit-artifacts" / f"wovenweb-network-{time.time_ns()}-{args.cpus}cpu"
    out.mkdir(parents=True)
    env = dict(os.environ)
    env["CARGO_BUILD_BUILD_DIR"] = str(root / ".cargo-build")
    env["CARGO_TARGET_DIR"] = str(root / "target")
    image = args.image
    if image is None:
        probe = root / "tools" / "wovenweb-net-probe"
        app_env = dict(env)
        app_env.pop("CARGO_BUILD_BUILD_DIR", None)
        app_env["CARGO_TARGET_DIR"] = str(probe / "target")
        print("Building native probe and kernel; logs:", out, flush=True)
        run_build(["cargo", "build", "--release", "--locked"], probe, app_env, out / "probe-build.log")
        env["WOVENHAT_WEB_NET_PROBE_IMAGE"] = str(probe / "target/x86_64-unknown-none/release/wovenweb-net-probe")
        image = build_image(root, env, "wovenweb-net-test", out / "kernel-build.log")
    if not image.is_file():
        raise RuntimeError("boot image does not exist")
    (out / "image.txt").write_text(str(image.resolve()), encoding="utf-8")
    def digest(path):
        value = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                value.update(block)
        return value.hexdigest()
    sources = ["libwoven/src/net.rs", "libwoven/src/socks5.rs", "kernel/src/wovenweb_net_gate.rs", "tools/wovenweb-net-probe/src/main.rs"]
    provenance = {"image_sha256": digest(image), "source_sha256": {name: digest(root / name) for name in sources}}
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2))
    qemu = Path(args.qemu)
    firmware = qemu.parent / "share/edk2-x86_64-code.fd"
    serial = out / "serial.log"
    capture = out / "network.pcap"
    errors, events = [], []
    finished = threading.Event()
    listener = socket.socket()
    listener.bind(("127.0.0.1", PORT))
    listener.listen(2)
    server = threading.Thread(target=serve, args=(listener, errors, events, finished), daemon=True)
    server.start()
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M", "-smp", str(args.cpus),
               "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
               "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}", "-device", "virtio-blk-pci,drive=boot,bootindex=1",
               "-device", "virtio-net-pci,netdev=net0,disable-modern=on", "-netdev", "user,id=net0",
               "-object", f"filter-dump,id=webcapture,netdev=net0,file={capture}"]
    code = None
    try:
        with (out / "qemu.log").open("w") as log:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log,
                timeout=args.timeout, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0).returncode
    finally:
        server.join(timeout=2)
        if server.is_alive():
            listener.close()
            errors.append("fixture server did not finish")
        (out / "host-events.json").write_text(json.dumps({"events": events, "errors": errors}, indent=2))
    text = serial.read_text(errors="replace") if serial.exists() else ""
    required = [MARKER, "[WOVENWEB] remote-name SOCKS + HTTP + native DOM: PASSED",
                "[WOVENWEB] deadline + cancellation + proxy refusal: PASSED",
                f"[SMP] online={args.cpus} expected={args.cpus}"]
    packets = dns_packets(capture)
    passed = code == 33 and not errors and finished.is_set() and all(marker in text for marker in required) and packets["ipv4_dns_packets"] == 0 and packets["ipv4_tcp_packets"] > 0
    (out / "result.json").write_text(json.dumps({"passed": passed, "cpus": args.cpus, "exit": code, **packets}, indent=2))
    print(text[-5000:] if not passed else "\n".join(line for line in text.splitlines() if "[WOVENWEB]" in line))
    print("Evidence:", out)
    print("Boot image:", image)
    if not passed:
        print("Host errors:", errors)
        print((out / "qemu.log").read_text(errors="replace")[-2000:])
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
