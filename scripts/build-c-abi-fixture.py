"""Build and validate the S02 freestanding C ABI ELF fixture."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


USER_BASE = 0x400000000000


def find_clang(explicit: str | None) -> str | None:
    if explicit:
        return explicit if Path(explicit).is_file() else None
    return shutil.which("clang") or shutil.which("clang.exe")


def elf_header(path: Path) -> dict[str, int]:
    data = path.read_bytes()
    if len(data) < 64 or data[:4] != b"\x7fELF":
        raise RuntimeError("output is not ELF")
    if data[4] != 2 or data[5] != 1:
        raise RuntimeError("output is not little-endian ELF64")
    machine = int.from_bytes(data[18:20], "little")
    elf_type = int.from_bytes(data[16:18], "little")
    entry = int.from_bytes(data[24:32], "little")
    if machine != 62 or elf_type != 2 or entry < USER_BASE:
        raise RuntimeError("output is not a static x86-64 user ELF")
    return {"machine": machine, "type": elf_type, "entry": entry}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clang")
    parser.add_argument("--output", type=Path,
                        default=Path("target") / "s02-c-abi")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = (root / args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    source = root / "tools" / "runtime-fixtures" / "c-abi.c"
    linker = root / "tools" / "runtime-fixtures" / "link.x"
    clang = find_clang(args.clang)
    report: dict[str, object] = {
        "contract": "WOVENHAT-S02-FREESTANDING-C-ABI-V1",
        "source": str(source.relative_to(root)),
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "linker": str(linker.relative_to(root)),
        "compiler": clang,
        "execution_gate": "NOT_RUN",
        "qualification": "NOT_QUALIFIED",
    }
    if clang is None:
        report["status"] = "NOT_AVAILABLE"
        report["reason"] = "No target-qualified clang was found"
        (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report, indent=2))
        return 2
    version = subprocess.run([clang, "--version"], cwd=root, capture_output=True,
                             text=True, check=False)
    (output / "compiler.txt").write_text(version.stdout + version.stderr, encoding="utf-8")
    object_file = output / "c-abi.o"
    elf_file = output / "c-abi.elf"
    compile_command = [clang, "--target=x86_64-unknown-none", "-ffreestanding",
                       "-fno-stack-protector", "-fno-builtin", "-nostdlib",
                       "-nostartfiles", "-nodefaultlibs", "-c", str(source),
                       "-o", str(object_file)]
    compiled = subprocess.run(compile_command, cwd=root, capture_output=True,
                              text=True, check=False)
    (output / "compile.log").write_text(compiled.stdout + compiled.stderr, encoding="utf-8")
    if compiled.returncode != 0:
        report["status"] = "COMPILE_FAILED"
        report["reason"] = "clang rejected the freestanding fixture"
        (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        return 1
    link_command = [clang, "--target=x86_64-unknown-none", "-fuse-ld=lld",
                    "-nostdlib", "-nostartfiles", "-nodefaultlibs", str(object_file),
                    "-Wl,-e,_start", f"-Wl,-T,{linker}", "-Wl,-z,norelro",
                    "-o", str(elf_file)]
    linked = subprocess.run(link_command, cwd=root, capture_output=True,
                            text=True, check=False)
    (output / "link.log").write_text(linked.stdout + linked.stderr, encoding="utf-8")
    if linked.returncode != 0:
        report["status"] = "LINK_FAILED"
        report["reason"] = "clang/lld could not produce the static ELF"
        (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        return 1
    report["elf"] = str(elf_file.relative_to(root))
    report["elf_sha256"] = hashlib.sha256(elf_file.read_bytes()).hexdigest()
    report["elf_header"] = elf_header(elf_file)
    report["status"] = "BUILT_NOT_EXECUTED"
    report["reason"] = "ELF build passed; loader/QEMU execution remains a separate gate"
    (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
