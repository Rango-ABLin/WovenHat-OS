"""Build and boot the native reclaiming-heap fixture on 1/2/4 CPUs."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

SPEC = importlib.util.spec_from_file_location("wovenweb_network_harness", Path(__file__).with_name("test-wovenweb-network-qemu.py"))
NETWORK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NETWORK)


def sha(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpus", type=int, choices=(1, 2, 4), default=1)
    parser.add_argument("--image", type=Path)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--qemu", default=shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe")
    NETWORK.add_firmware_arguments(parser)
    args = parser.parse_args()
    qemu, firmware = NETWORK.resolve_firmware(args)
    root = Path(__file__).resolve().parents[1]
    out = root / "audit-artifacts" / f"wovenweb-memory-{time.time_ns()}-{args.cpus}cpu"
    out.mkdir(parents=True)
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(root / "target")
    env["CARGO_BUILD_BUILD_DIR"] = str(root / ".cargo-build")
    image = args.image
    if image is None:
        probe = root / "tools/wovenweb-net-probe"
        app_env = dict(env)
        app_env.pop("CARGO_BUILD_BUILD_DIR", None)
        app_env["CARGO_TARGET_DIR"] = str(probe / "target")
        print("Building memory runtime probe; evidence:", out, flush=True)
        NETWORK.run_build(["cargo", "build", "--release", "--locked"], probe, app_env, out / "probe-build.log")
        env["WOVENHAT_WEB_NET_PROBE_IMAGE"] = str(probe / "target/x86_64-unknown-none/release/wovenweb-net-probe")
        image = NETWORK.build_image(root, env, "wovenweb-memory-test", out / "kernel-build.log")
    if not image.is_file():
        raise RuntimeError("boot image does not exist")
    sources = ["kernel/src/task.rs", "kernel/src/userspace.rs", "kernel/src/vm_generation.rs", "kernel/src/vm_handoff.rs"] + ["libwoven/src/lib.rs", "libwoven/src/heap.rs", "libwoven/src/heap_allocator.rs", "kernel/src/wovenweb_memory_gate.rs", "tools/wovenweb-net-probe/src/heap.rs", "tools/wovenweb-net-probe/src/main.rs", "libwoven/Cargo.toml", "tools/wovenweb-net-probe/Cargo.toml"]
    provenance = {"image_sha256": sha(image), "source_sha256": {file: sha(root / file) for file in sources}}
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2))
    (out / "image.txt").write_text(str(image.resolve()))
    pflash = NETWORK.firmware_drives(firmware, args.firmware_vars, out)
    serial = out / "serial.log"
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M", "-smp", str(args.cpus),
               "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               *pflash,
               "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}", "-device", "virtio-blk-pci,drive=boot,bootindex=1"]
    with (out / "qemu.log").open("w") as log:
        result = subprocess.run(command, cwd=root, stdout=log, stderr=log, timeout=args.timeout,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    text = serial.read_text(errors="replace") if serial.exists() else ""
    markers = [f"[SMP] online={args.cpus} expected={args.cpus}",
               "[WOVENWEB-HEAP] concurrent initialization + 800 thread allocations: PASSED",
               "[WOVENWEB-HEAP] reuse + alignment + cross-region realloc + zeroing: PASSED",
               "[WOVENWEB-HEAP] kernel slot exhaustion + recovery + idle-region release: PASSED",
               "[WOVENWEB-HEAP] standard Rust Vec integration: PASSED",
               "[WOVENWEB-HEAP] native memory platform: PASSED"]
    passed = result.returncode == 33 and all(marker in text for marker in markers)
    (out / "result.json").write_text(json.dumps({"passed": passed, "cpus": args.cpus, "exit": result.returncode}, indent=2))
    print("\n".join(line for line in text.splitlines() if "[WOVENWEB-HEAP]" in line) if passed else text[-6000:])
    print("Evidence:", out)
    print("Boot image:", image)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
