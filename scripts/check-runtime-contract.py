"""Inspect the bounded S02 native runtime contract and C fixture toolchain.

This checker is intentionally conservative. It validates the fixture source
shape and records compiler availability, but it does not claim ELF execution
unless a target-qualified C compiler is found and an explicit build/loader
gate is added.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path


def run_version(command):
    try:
        result = subprocess.run(command, capture_output=True, text=True,
                                check=False, timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode:
        return None
    return (result.stdout + result.stderr).strip()


def check(output):
    root = Path(__file__).resolve().parents[1]
    source = root / "tools" / "runtime-fixtures" / "c-abi.c"
    text = source.read_text(encoding="utf-8")
    required = ("void _start", "int $0x80", "mov $3, %%eax", "42")
    missing = [token for token in required if token not in text]
    if missing:
        raise RuntimeError(f"C fixture contract missing tokens: {missing}")

    compiler = None
    version = None
    for candidate in ("clang", "clang.exe"):
        path = shutil.which(candidate)
        if path:
            compiler = path
            version = run_version([path, "--version"])
            break

    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {
        "contract": "WOVENHAT-S02-FREESTANDING-C-ABI-V1",
        "source": str(source.relative_to(root)),
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "required_source_tokens": required,
        "source_shape": "PASS" if not missing else "FAIL",
        "elf_c_compiler": "AVAILABLE" if compiler and version else "NOT_AVAILABLE",
        "compiler": compiler,
        "compiler_version": version,
        "execution_gate": "NOT_RUN",
        "qualification": "NOT_QUALIFIED",
        "reason": "No target-qualified ELF C compiler was available" if not compiler else
                  "Compilation and loader/QEMU acceptance are separate gates",
    }
    (output / "runtime-contract.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path,
                        default=root / "audit-artifacts" /
                        f"runtime-contract-{time.time_ns()}")
    args = parser.parse_args()
    print(json.dumps(check(args.output), indent=2))


if __name__ == "__main__":
    main()
