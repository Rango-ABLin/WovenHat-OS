"""Stage 25.7: stage and tear down the low-memory S3 wake trampoline.

The guest never writes the FACS waking vector or enters S3. It installs the
existing AP real-mode-to-long-mode image on the reserved wake page, validates
the private page-table/stack patching path, releases the allocated tables, and
then exits through the QEMU test device.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=240)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')
    out = root / 'audit-artifacts' / f'stage25.7-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    env = os.environ.copy()
    env.setdefault('CARGO_BUILD_BUILD_DIR', str(root / '.cargo-build'))
    env.setdefault('CARGO_TARGET_DIR', str(root / 'target'))
    build = subprocess.run(
        ['cargo', 'run', '--quiet', '--features', 'stage25-7-test', '--', '--print-image'],
        cwd=root, env=env, text=True, capture_output=True,
    )
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M',
        '-smp', str(args.cpus), '-display', 'none', '-serial', f'file:{serial}',
        '-no-reboot', '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    with (out / 'qemu.log').open('w') as log:
        try:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log,
                                  timeout=args.timeout, creationflags=flags).returncode
        except subprocess.TimeoutExpired:
            print(f'QEMU timeout; evidence: {out}', file=sys.stderr)
            return 1
    text = serial.read_text(errors='replace') if serial.exists() else ''
    if code != 33 or '[S25.7] wake trampoline staging: PASSED' not in text:
        print(f'Stage 25.7 failed (exit {code}); evidence: {out}', file=sys.stderr)
        return 1
    print(f'Stage 25.7 PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
