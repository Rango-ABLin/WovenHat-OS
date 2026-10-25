"""Experimental Tor-only host launcher; requires patched Ladybird and local Tor."""
import argparse
import os
from pathlib import Path
import socket
import subprocess
import sys
from urllib.parse import urlsplit

HERE = Path(__file__).resolve().parent
PROXY = "socks5h://127.0.0.1:9050"


def receive_exact(stream, count):
    data = b""
    while len(data) < count:
        part = stream.recv(count - len(data))
        if not part:
            raise ConnectionError("SOCKS server closed the handshake")
        data += part
    return data


def check_proxy():
    # This checks SOCKS availability, not Tor identity or bootstrap status.
    with socket.create_connection(("127.0.0.1", 9050), timeout=3) as stream:
        stream.settimeout(3)
        stream.sendall(b"\x05\x01\x00")
        if receive_exact(stream, 2) != b"\x05\x00":
            raise ConnectionError("local endpoint did not accept SOCKS5")


def validate_url(value):
    if any(c.isspace() or ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError("URLs cannot contain whitespace or control characters")
    parsed = urlsplit(value)
    if parsed.scheme not in ("http", "https") or not parsed.hostname:
        raise ValueError("only explicit http:// and https:// URLs are accepted")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError("URL credentials are not accepted")
    if parsed.port == 0:
        raise ValueError("invalid port")
    return value


def launch_environment(parent):
    environment = {key: value for key, value in parent.items()
                   if not key.lower().endswith("_proxy")}
    environment.update({"WOVENWEB_TOR_ONLY": "1", "all_proxy": PROXY,
                        "http_proxy": PROXY, "https_proxy": PROXY,
                        "no_proxy": "", "NO_PROXY": ""})
    return environment


def command(binary, urls):
    return [str(binary), "--force-new-process", "--temporary-profile",
            "--disable-http-disk-cache", "--disable-scripting",
            "--default-time-zone=UTC",
            f"--content-blocker-list={HERE / 'starter-filters.txt'}", *urls]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path,
                        help="path to the WovenWeb-patched ladybird executable")
    parser.add_argument("urls", nargs="*")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not binary.is_file():
        parser.error("binary must be a file")
    urls = [validate_url(value) for value in args.urls]
    check_proxy()
    print("WovenWeb: SOCKS5 available; Tor identity, VPN and leak isolation unverified.", file=sys.stderr)
    return subprocess.run(command(binary, urls), env=launch_environment(os.environ), check=False).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, ConnectionError) as error:
        print(f"WovenWeb refused to start: {error}", file=sys.stderr)
        sys.exit(1)
