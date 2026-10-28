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
    transport = root / "ports" / "musl" / "include" / "wovenhat" / "syscall.h"
    musl_transport = root / "ports" / "musl" / "include" / "syscall_arch.h"
    musl_bootstrap = root / "scripts" / "build-musl-wovenhat-bootstrap.py"
    rust_sources = {
        "shared-main": root / "tools" / "loader-fixtures" / "shared-main.rs",
        "library": root / "tools" / "loader-fixtures" / "library.rs",
        "static-tls": root / "tools" / "loader-fixtures" / "static-tls.rs",
        "tls-threads": root / "tools" / "loader-fixtures" / "tls-threads.rs",
    }
    text = source.read_text(encoding="utf-8")
    transport_text = transport.read_text(encoding="utf-8")
    musl_transport_text = musl_transport.read_text(encoding="utf-8")
    musl_bootstrap_text = musl_bootstrap.read_text(encoding="utf-8")
    required = ("void _start", "__syscall1", "WH_LINUX_EXIT", "42")
    required_transport = ("int $0x80", "WH_SYS_CLOSE", "wh_syscall3", "wh_syscall4_unmapped")
    missing = [token for token in required if token not in text]
    missing += [f"transport:{token}" for token in required_transport if token not in transport_text]
    required_musl_transport = ("WH_LINUX_EXIT", "WH_LINUX_CLOSE", "__syscall3", "-WH_ENOSYS")
    missing += [f"musl_transport:{token}" for token in required_musl_transport if token not in musl_transport_text]
    required_musl_bootstrap = ("crt_start.S", "vendor/musl/src/unistd/getpid.c", "vendor/musl/src/unistd/getppid.c", "ports/musl/src/open.c", "vendor/musl/src/unistd/read.c", "vendor/musl/src/unistd/write.c", "vendor/musl/src/unistd/close.c", "syscall_ret.c", "BUILT_NOT_EXECUTED")
    missing += [f"musl_bootstrap:{token}" for token in required_musl_bootstrap if token not in musl_bootstrap_text]
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
        "transport": str(transport.relative_to(root)),
        "transport_sha256": hashlib.sha256(transport.read_bytes()).hexdigest(),
        "musl_transport": str(musl_transport.relative_to(root)),
        "musl_transport_sha256": hashlib.sha256(musl_transport.read_bytes()).hexdigest(),
        "musl_bootstrap": str(musl_bootstrap.relative_to(root)),
        "musl_bootstrap_sha256": hashlib.sha256(musl_bootstrap.read_bytes()).hexdigest(),
        "required_source_tokens": required,
        "required_transport_tokens": required_transport,
        "required_musl_transport_tokens": required_musl_transport,
        "required_musl_bootstrap_tokens": required_musl_bootstrap,
        "source_shape": "PASS" if not missing else "FAIL",
        "rust_fixtures": {
            name: {
                "path": str(path.relative_to(root)),
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "present": path.is_file(),
            }
            for name, path in rust_sources.items()
        },
        "rust_fixture_inventory": "PASS" if all(path.is_file() for path in rust_sources.values()) else "FAIL",
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
