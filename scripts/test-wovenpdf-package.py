"""Build the experimental WovenPDF Ring-3 ELF and validate its WAPP envelope.

This does NOT boot QEMU or establish graphical runtime correctness.
Do not add WovenPDF to the production system-app catalog before its runtime gates.
"""

import argparse
import os
from pathlib import Path
import subprocess
import sys


def run(args: list[str], cwd: Path, env: dict[str, str]) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run(args, cwd=cwd, env=env, check=True)


def verify_wapp(package: Path, elf: Path) -> None:
    data = package.read_bytes()
    header_size = 4 + 2 + 2 + 8 + 64
    if len(data) < header_size or data[:4] != b"WAPP":
        raise RuntimeError("missing WAPP envelope")
    if int.from_bytes(data[4:6], "little") != 1:
        raise RuntimeError("unexpected WAPP version")
    if int.from_bytes(data[6:8], "little") != header_size:
        raise RuntimeError("incorrect WAPP header length")
    payload_size = int.from_bytes(data[8:16], "little")
    if payload_size != len(data) - header_size:
        raise RuntimeError("inconsistent WAPP payload length")
    if data[header_size:header_size + 4] != b"\x7fELF":
        raise RuntimeError("WAPP payload does not begin with ELF")
    if data[header_size:] != elf.read_bytes():
        raise RuntimeError("WAPP payload differs from built ELF")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo", default="cargo")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / "apps" / "woven-pdf"
    env = dict(os.environ)
    env.pop("CARGO_BUILD_BUILD_DIR", None)
    env["CARGO_TARGET_DIR"] = str(app / "target")
    run([args.cargo, "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app, env=env)
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-pdf"
    if not elf.is_file():
        raise RuntimeError(f"missing native ELF: {elf}")
    package = elf.with_suffix(".wapp")
    pack_env = dict(os.environ)
    pack_env.pop("CARGO_BUILD_BUILD_DIR", None)
    pack_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    run([args.cargo, "run", "--quiet", "--manifest-path",
         str(root / "tools" / "wapp-packager" / "Cargo.toml"),
         "--", str(elf), str(package)], cwd=root, env=pack_env)
    verify_wapp(package, elf)
    print(f"WovenPDF WAPP packaging: PASS ({package})")
    print("This is a package check only; WovenShell and QEMU runtime remain unverified.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"WovenPDF packaging: FAIL ({error})", file=sys.stderr)
        sys.exit(1)
