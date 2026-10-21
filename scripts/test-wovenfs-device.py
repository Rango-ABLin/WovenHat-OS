"""Validate WovenFS against a real block device through the ATA driver.

The disk is attached with `-machine pc` and `if=ide,index=0`, which is the
configuration in which the kernel's ATA PIO driver finds a primary master. The
`q35` machine used by the general runtime harness routes `if=ide` through the
ICH9 AHCI controller instead, where the legacy ports this driver probes are not
present, so a scratch disk attached there is invisible to the filesystem.

By default the disk is a disposable blank image under `target/`. Pass
`--physical-drive N` to drive a real device instead; that requires an elevated
shell and erases the device.
"""
import argparse
import shutil
import subprocess
import sys
import time
from pathlib import Path

MARKER = '[S15.2] WovenFS physical device: PASSED'


def wovenfs_device_errors(log: str, exit_code: int) -> list[str]:
    """Fail closed on contradictory guest evidence, not just the PASS marker."""
    errors = []
    if exit_code != 33:
        errors.append(f'Unexpected QEMU exit code: {exit_code} (expected 33)')
    if not any(MARKER in line for line in log.splitlines()):
        errors.append(f'Missing WovenFS acceptance marker: {MARKER}')
    normalized = log.lower()
    for marker in ('kernel panic', 'panicked at', 'security violation', 'security: failed'):
        if marker in normalized:
            errors.append(f'Guest serial contains fatal evidence: {marker}')
    for line in log.splitlines():
        if line.strip().startswith('[S15.2]') and ('FAILED' in line or ' FAIL' in line):
            errors.append(f'Contradictory WovenFS failure marker: {line.strip()}')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64')
                        or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path,
                        help='matching writable OVMF VARS template')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--timeout', type=float, default=240)
    parser.add_argument('--size-mib', type=int, default=16,
                        help='scratch image size when not using a physical drive')
    parser.add_argument('--physical-drive', type=int,
                        help='raw physical drive number; ERASES IT, needs elevation')
    args = parser.parse_args()
    if args.size_mib <= 0:
        parser.error('--size-mib must be positive')

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing QEMU and OVMF files.')

    image = subprocess.check_output(
        ['cargo', 'run', '--quiet', '--features', 'stage15-2-test', '--', '--print-image'],
        cwd=root, text=True).strip()

    label = 'physical' if args.physical_drive is not None else 'scratch'
    # Retain each attempt independently, including failed runs and disk images.
    out = root / 'target' / f'wovenfs-device-{label}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True, exist_ok=False)
    serial = out / 'serial.log'
    serial.write_text('')

    vars_copy = None
    if args.firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(args.firmware_vars, vars_copy)

    if args.physical_drive is not None:
        target = f'\\\\.\\PhysicalDrive{args.physical_drive}'
        disk_argument = f'if=ide,index=0,format=raw,cache=none,file={target}'
        print(f'Using RAW PHYSICAL DEVICE {target} — its contents will be destroyed.')
    else:
        disk = out / 'wovenfs.img'
        if disk.exists():
            disk.unlink()
        with disk.open('wb') as handle:
            handle.truncate(args.size_mib * 1024 * 1024)
        disk_argument = f'if=ide,index=0,format=raw,file={disk}'
        print(f'Using disposable scratch image {disk} ({args.size_mib} MiB)')

    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'pc',
        '-m', '256M', '-smp', str(args.cpus), '-display', 'none',
        '-serial', f'file:{serial}', '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
        *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}']
          if vars_copy is not None else []),
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-drive', disk_argument,
    ]

    with (out / 'qemu.log').open('w') as output:
        try:
            result = subprocess.run(command, cwd=root, stdout=output, stderr=output,
                                    timeout=args.timeout).returncode
        except subprocess.TimeoutExpired:
            print(f'QEMU timed out. Evidence: {out}', file=sys.stderr)
            return 1

    log = serial.read_text(errors='replace') if serial.exists() else ''
    for line in log.splitlines():
        if '[S15.' in line:
            print('  ' + line.strip())

    errors = wovenfs_device_errors(log, result)
    if errors:
        for error in errors:
            print(f'FAILED: {error}', file=sys.stderr)
        print(f'QEMU exit {result}. Evidence: {out}', file=sys.stderr)
        print(log[-4000:], file=sys.stderr)
        return 1
    print(f'WovenFS {label} device: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
