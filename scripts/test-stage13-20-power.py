"""Stage 13.20: ACPI power-off and reset.

  poweroff  the stage13-20-test kernel reads the PM1 control blocks and
            \\_S5 from ACPI and enters S5. QEMU (without -no-shutdown) exits
            when the machine turns off.
  reboot    the stage13-20b-test kernel resets the machine. With -no-reboot
            QEMU exits instead of restarting.

Either way QEMU must exit by itself with status 0 after the kernel's request
line, and no "still running" or FAILED line may follow. A kernel that
fails to turn the machine off reports it and exits through the test device
with a failure status instead.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mode', choices=('poweroff', 'reboot'), default='poweroff')
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
    out = root / 'audit-artifacts' / f'stage13.20-{args.mode}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    feature = 'stage13-20-test' if args.mode == 'poweroff' else 'stage13-20b-test'
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', feature, '--', '--print-image'],
                           cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1']
    if args.mode == 'reboot':
        command.append('-no-reboot')
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        try:
            process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            errors.append('timeout: the machine neither turned off nor reset')
            process.kill()
            process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    request = '[S13.20] requesting power-off' if args.mode == 'poweroff' else '[S13.20] requesting reset'
    if process.returncode != 0:
        errors.append(f'QEMU exit status {process.returncode}, expected 0 (stopped by the guest)')
    if request not in log:
        errors.append(f'missing {request!r}')
    tail = log.split(request, 1)[-1]
    for bad in ('still running', 'FAILED', 'FAIL'):
        if bad in tail:
            errors.append(f'{bad!r} after the request')
    relevant = [line for line in log.splitlines() if 'S13.20' in line or '[POWER]' in line]
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(relevant), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print('\n'.join(relevant[-3:]))
    print(f'Stage 13.20 {args.mode}: PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
