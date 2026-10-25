"""Build and boot the native pager-owner lifetime fixture on 1/2/4 CPUs."""
import argparse
import hashlib
import importlib.util
import json
import os
import re
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
    parser.add_argument("--firmware", type=Path, help="OVMF CODE image (required on Linux)")
    parser.add_argument("--firmware-vars", type=Path, help="optional OVMF VARS template; copied per run")
    args = parser.parse_args()
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / "share/edk2-x86_64-code.fd"
    if not qemu.is_file() or not firmware.is_file():
        parser.error("QEMU and OVMF CODE must exist; use --qemu and --firmware")
    if args.firmware_vars is not None and not args.firmware_vars.is_file():
        parser.error("OVMF VARS template does not exist")
    root = Path(__file__).resolve().parents[1]
    out = root / "audit-artifacts" / f"wovenweb-vm-{time.time_ns()}-{args.cpus}cpu"
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
        print("Building pager ownership probe; evidence:", out, flush=True)
        NETWORK.run_build(["cargo", "build", "--release", "--locked"], probe, app_env, out / "probe-build.log")
        env["WOVENHAT_WEB_NET_PROBE_IMAGE"] = str(probe / "target/x86_64-unknown-none/release/wovenweb-net-probe")
        NETWORK.run_build(["cargo", "clippy", "-p", "wovenhat-kernel", "--target", "x86_64-unknown-none", "--features", "wovenweb-vm-test", "--locked", "--", "-D", "warnings"], root, env, out / "kernel-clippy.log")
        image = NETWORK.build_image(root, env, "wovenweb-vm-test", out / "kernel-build.log")
    if not image.is_file():
        raise RuntimeError("boot image does not exist")
    sources = ["kernel/src/task.rs", "kernel/src/userspace.rs", "kernel/src/pager_queue.rs", "kernel/src/vm_handoff.rs", "kernel/src/main.rs", "kernel/Cargo.toml", "tools/wovenweb-net-probe/src/main.rs"]
    provenance = {"image_sha256": sha(image), "source_sha256": {file: sha(root / file) for file in sources}}
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2))
    (out / "image.txt").write_text(str(image.resolve()))
    vars_copy = None
    if args.firmware_vars is not None:
        vars_copy = out / "OVMF_VARS.fd"
        shutil.copyfile(args.firmware_vars, vars_copy)
    serial = out / "serial.log"
    command = [str(qemu), "-accel", "tcg,tb-size=128", "-machine", "q35", "-m", "256M", "-smp", str(args.cpus),
               "-display", "none", "-serial", f"file:{serial}", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
               *(["-drive", f"if=pflash,unit=1,format=raw,file={vars_copy}"] if vars_copy else []),
               "-drive", f"if=none,id=boot,format=raw,readonly=on,file={image}", "-device", "virtio-blk-pci,drive=boot,bootindex=1"]
    with (out / "qemu.log").open("w") as log:
        result = subprocess.run(command, cwd=root, stdout=log, stderr=log, timeout=args.timeout,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    text = serial.read_text(errors="replace") if serial.exists() else ""
    markers = [f"[SMP] online={args.cpus} expected={args.cpus}",
               "[PAGER] Ring-3 faults queued=",
               "[WOVENWEB-VM] physical handoff reaping guard: PASSED",
               "[WOVENWEB-VM] queued + active pager work retains zombie address space: PASSED",
               "[WOVENWEB-VM] completion owner cleanup + exact reaping baseline: PASSED",
               "[WOVENWEB-VM] sibling-thread lazy faults + canonical publication + exact cleanup: PASSED",
               "[WOVENWEB-VM] queued sibling page deltas + duplicate coalescing + exact cleanup: PASSED",
               "[WOVENWEB-VM] pager owner lifetime: PASSED"]
    faults = re.search(r"\[PAGER\] Ring-3 faults queued=(\d+) completed=(\d+): PASSED", text)
    counts = tuple(map(int, faults.groups())) if faults else (0, 0)
    passed = result.returncode == 33 and all(marker in text for marker in markers) and counts[0] > 0 and counts[1] >= counts[0]
    (out / "result.json").write_text(json.dumps({"passed": passed, "cpus": args.cpus, "exit": result.returncode, "real_pager_faults": counts}, indent=2))
    print("\n".join(line for line in text.splitlines() if "[WOVENWEB-VM]" in line) if passed else text[-6000:])
    print("Evidence:", out)
    print("Boot image:", image)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
