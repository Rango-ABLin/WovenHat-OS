"""Build the deliberately small genuine-musl static bootstrap subset.

This is not a full musl build and is not a POSIX-conformance claim.  It
generates musl's required type header, compiles the bounded upstream
process/file/I-O sources against the WovenHat syscall adapter, and archives
their objects with the cancellation-point bridge.  Startup, errno/TLS,
allocator, threads and the rest of musl are later, independently accepted
increments.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


MUSL_VERSION = "1.2.6"
SOURCES = (
    "ports/musl/src/crt_start.c",
    "ports/musl/src/syscall_cp.c",
    "ports/musl/src/errno_location.c",
    "ports/musl/src/string.c",
    "ports/musl/src/malloc.c",
    "ports/musl/src/memory.c",
    "ports/musl/src/pthread.c",
    "ports/musl/src/open.c",
    "vendor/musl/src/exit/_Exit.c",
    "vendor/musl/src/internal/syscall_ret.c",
    "vendor/musl/src/unistd/getpid.c",
    "vendor/musl/src/unistd/getppid.c",
    "vendor/musl/src/unistd/getuid.c",
    "vendor/musl/src/unistd/getgid.c",
    "ports/musl/src/chdir.c",
    "ports/musl/src/getcwd.c",
    "vendor/musl/src/unistd/read.c",
    "vendor/musl/src/unistd/write.c",
    "vendor/musl/src/unistd/close.c",
    "vendor/musl/src/unistd/lseek.c",
    "vendor/musl/src/unistd/dup.c",
    "vendor/musl/src/unistd/dup2.c",
    "ports/musl/src/pipe.c",
    "ports/musl/src/fork.c",
    "ports/musl/src/sched_yield.c",
    "ports/musl/src/waitpid.c",
    "ports/musl/src/signal.c",
    "ports/musl/src/kill.c",
)
ASSEMBLY_SOURCES = ("ports/musl/src/crt_start.S",)
FIXTURE = "tools/runtime-fixtures/musl-bootstrap.c"


def find_tool(explicit: str | None, names: tuple[str, ...]) -> str | None:
    if explicit:
        return explicit if Path(explicit).is_file() else None
    for name in names:
        found = shutil.which(name)
        if found:
            return found
    return None


def git_sed() -> str | None:
    found = shutil.which("sed")
    if found:
        return found
    if os.name == "nt":
        candidate = Path(os.environ.get("ProgramFiles", r"C:\\Program Files")) / "Git" / "usr" / "bin" / "sed.exe"
        if candidate.is_file():
            return str(candidate)
    return None


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def find_lld(clang: str) -> str | None:
    candidate = Path(clang).resolve().parent / ("ld.lld.exe" if os.name == "nt" else "ld.lld")
    return str(candidate) if candidate.is_file() else shutil.which("ld.lld")


def run(command: list[str], log: Path, root: Path) -> None:
    result = subprocess.run(command, cwd=root, capture_output=True, text=True, check=False)
    log.write_text("$ " + " ".join(command) + "\n" + result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}): {command[0]}")


def object_stem(source: str) -> str:
    """Avoid collisions such as the paired crt_start.c/crt_start.S files."""
    return source.replace("/", "-").replace(".", "-")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clang")
    parser.add_argument("--ar")
    parser.add_argument("--output", type=Path, default=Path("target") / "musl-wovenhat-bootstrap")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = (root / args.output).resolve()
    if output.exists() and any(output.iterdir()):
        raise SystemExit(f"refusing to overwrite nonempty output directory: {output}")
    output.mkdir(parents=True, exist_ok=True)

    clang = find_tool(args.clang, ("clang", "clang.exe"))
    ar = find_tool(args.ar, ("llvm-ar", "llvm-ar.exe"))
    sed = git_sed()
    report: dict[str, object] = {
        "contract": "WOVENHAT-MUSL-BOOTSTRAP-V1",
        "musl_version": MUSL_VERSION,
        "sources": {source: sha256(root / source) for source in SOURCES + ASSEMBLY_SOURCES},
        "fixture": {"path": FIXTURE, "sha256": sha256(root / FIXTURE)},
        "qualification": "NOT_QUALIFIED",
        "scope": "bounded upstream musl process/file/I-O objects plus WovenHat cancellation bridge",
    }
    if (root / "vendor/musl/VERSION").read_text(encoding="utf-8").strip() != MUSL_VERSION:
        raise SystemExit("pinned musl version is not the expected bootstrap source")
    lld = find_lld(clang) if clang else None
    if not clang or not ar or not sed or not lld:
        report["status"] = "NOT_AVAILABLE"
        report["missing_tools"] = [name for name, value in (("clang", clang), ("ld.lld", lld), ("llvm-ar", ar), ("sed", sed)) if not value]
        (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report, indent=2))
        return 2

    generated = output / "include" / "bits"
    generated.mkdir(parents=True)
    alltypes = generated / "alltypes.h"
    with alltypes.open("wb") as destination:
        result = subprocess.run(
            [sed, "-f", str(root / "vendor/musl/tools/mkalltypes.sed"),
             str(root / "vendor/musl/arch/x86_64/bits/alltypes.h.in"),
             str(root / "vendor/musl/include/alltypes.h.in")],
            cwd=root, stdout=destination, stderr=subprocess.PIPE, check=False)
    (output / "generate-alltypes.log").write_bytes(result.stderr)
    if result.returncode:
        raise RuntimeError("musl alltypes header generation failed")
    shutil.copy2(root / "ports/musl/include/bits/syscall.h", generated / "syscall.h")

    common = [clang, "--target=x86_64-unknown-none", "-std=c99", "-ffreestanding",
              "-fno-stack-protector", "-fno-builtin", "-mcmodel=large", "-nostdinc", "-D_XOPEN_SOURCE=700",
              # musl's own upstream build does not enable -Wextra.  In
              # particular, its generic socket fallback intentionally has an
              # unused argument on x86_64, where SYS_socketcall is absent.
              "-Wall", "-Werror", "-I", str(root / "ports/musl/include"),
              "-I", str(output / "include"), "-I", str(root / "vendor/musl/arch/x86_64"),
              "-I", str(root / "vendor/musl/arch/generic"), "-I", str(root / "vendor/musl/src/include"),
              "-I", str(root / "vendor/musl/src/internal"), "-I", str(root / "vendor/musl/include")]
    objects: list[Path] = []
    for source in SOURCES:
        stem = object_stem(source)
        object_file = output / (stem + ".o")
        run(common + ["-c", str(root / source), "-o", str(object_file)],
            output / (stem + ".compile.log"), root)
        objects.append(object_file)
    for source in ASSEMBLY_SOURCES:
        stem = object_stem(source)
        object_file = output / (stem + ".o")
        run(common + ["-c", str(root / source), "-o", str(object_file)],
            output / (stem + ".compile.log"), root)
        objects.append(object_file)
    archive = output / "libmusl-wovenhat-bootstrap.a"
    run([ar, "rcs", str(archive), *map(str, objects)], output / "archive.log", root)
    fixture_object = output / "musl-bootstrap-fixture.o"
    run(common + ["-c", str(root / FIXTURE), "-o", str(fixture_object)],
        output / "fixture.compile.log", root)
    elf = output / "musl-bootstrap.elf"
    run([lld, "--entry=_start", f"--script={root / 'tools/runtime-fixtures/link.x'}",
         "-z", "norelro", "--oformat=elf64-x86-64", "-o", str(elf),
         str(fixture_object), str(archive)], output / "fixture.link.log", root)
    report.update({
        "status": "BUILT_NOT_EXECUTED",
        "archive": str(archive.relative_to(root)),
        "archive_sha256": sha256(archive),
        "elf": str(elf.relative_to(root)),
        "elf_sha256": sha256(elf),
        "objects": {str(path.relative_to(root)): sha256(path) for path in objects},
        "reason": "Static errno/TLS/write link passed; allocator, threads and broader loader/QEMU coverage are separate gates",
    })
    (output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
