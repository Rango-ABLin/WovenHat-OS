"""M5 shared DMA primitive: QEMU gate.

Builds the kernel with the `m5-dma-test` feature and boots it under QEMU/q35.
The gate runs `dma::self_test()`, which exercises the shared DMA page's
CPU/device ownership transitions, bounds checks, device-address range check
and write/read ordering on a real allocated frame.

Pass requires all of:
  * the serial log contains `[M5 DMA] shared DMA ownership: PASSED`;
  * QEMU exits with the success code 33 written by qemu_test_exit_success();
  * no `[DMA] quarantined` line appears (the self-test must not drop a page
    while the device owns it).

No physical disk is selected or written. The only disk is the boot image
attached as a virtio-blk device, and it is opened read-only.

This is a software gate for the primitive. It does not qualify any device;
device DMA is verified by the storage and network gates that adopt the API.
"""
import argparse
from pathlib import Path
import os
import re
import shutil
import subprocess
import sys
import time

PASS_MARKER = '[M5 DMA] shared DMA ownership: PASSED'
QUARANTINE = re.compile(r'\[DMA\] quarantined')
SUCCESS_EXIT = 33


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--memory', default='4G', help='guest RAM; the stage harnesses use 4G')
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=240)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')

    out = root / 'audit-artifacts' / f'm5-dma-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)

    build = subprocess.run(
        ['cargo', 'run', '--quiet', '--features', 'm5-dma-test', '--', '--print-image'],
        cwd=root, text=True, capture_output=True,
    )
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1

    serial = out / 'serial.log'
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', args.memory,
        '-smp', str(args.cpus), '-display', 'none', '-serial', f'file:{serial}',
        '-no-reboot', '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            while process.poll() is None:
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                time.sleep(0.5)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    if process.returncode != SUCCESS_EXIT:
        errors.append(f'exit code {process.returncode}, expected {SUCCESS_EXIT}')
    if PASS_MARKER not in log:
        errors.append(f'missing {PASS_MARKER!r}')
    if QUARANTINE.search(log):
        errors.append('unexpected DMA quarantine during self-test')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print(log[-8000:], file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'M5 DMA ({args.cpus} CPU): PASS. Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
