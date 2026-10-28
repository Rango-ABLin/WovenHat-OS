"""Stage 13.17: disks WovenHat does not own are never written.

The machine is q35 with no ATA disk. Its only data disk is a FAT32 image
formatted as another operating system would format its EFI system
partition: OEM name MSDOS5.0, volume label NO NAME, and one file,
HELLO.TXT. It is attached as:

  nvme        an NVMe namespace; the stage13-17-test kernel mounts the
              volume, reads the file, then attempts to change the file,
              create a file, write a raw sector and flush, and requires
              every write to be refused
  ahci        the same, on the chipset's AHCI controller
  production  an NVMe namespace, booting a normal (no test feature)
              kernel, which must mount the volume and reach the shell

In every mode the disk image must be byte-for-byte unchanged afterwards.
The firmware gets its own writable variable store: without one, OVMF saves
its variables as NvVars on the first FAT volume it finds, which would be
this disk.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time

PASS_MARKER = '[S13.17] foreign disks are read-only: PASSED'
SHELL_MARKER = '[BOOT] shell-first runtime ready'
CONTENT = b"Another system's file\n"


def foreign_fat32(path):
    """A FAT32 volume as another system formats it, holding HELLO.TXT."""
    sector = 512
    total = 70000
    reserved = 32
    fat_sectors = 600
    first_data = reserved + 2 * fat_sectors
    clusters = total - first_data
    boot = bytearray(sector)
    boot[:3] = b'\xeb\x58\x90'
    boot[3:11] = b'MSDOS5.0'
    struct.pack_into('<H', boot, 11, sector)
    boot[13] = 1
    struct.pack_into('<H', boot, 14, reserved)
    boot[16] = 2
    boot[21] = 0xF8
    struct.pack_into('<I', boot, 32, total)
    struct.pack_into('<I', boot, 36, fat_sectors)
    struct.pack_into('<I', boot, 44, 2)
    struct.pack_into('<H', boot, 48, 1)
    struct.pack_into('<H', boot, 50, 6)
    boot[64] = 0x80
    boot[66] = 0x29
    struct.pack_into('<I', boot, 67, 0x1234ABCD)
    boot[71:82] = b'NO NAME    '
    boot[82:90] = b'FAT32   '
    boot[510:512] = b'\x55\xaa'
    info = bytearray(sector)
    struct.pack_into('<I', info, 0, 0x41615252)
    struct.pack_into('<I', info, 484, 0x61417272)
    struct.pack_into('<I', info, 488, clusters - 2)
    struct.pack_into('<I', info, 492, 4)
    struct.pack_into('<I', info, 508, 0xAA550000)
    fat = bytearray(sector)
    # Media, reserved, root directory (cluster 2) and HELLO.TXT (cluster 3).
    struct.pack_into('<IIII', fat, 0, 0x0FFFFFF8, 0xFFFFFFFF, 0x0FFFFFFF, 0x0FFFFFFF)
    root = bytearray(sector)
    root[0:11] = b'HELLO   TXT'
    root[11] = 0x20
    struct.pack_into('<H', root, 20, 0)
    struct.pack_into('<H', root, 26, 3)
    struct.pack_into('<I', root, 28, len(CONTENT))
    data = bytearray(sector)
    data[:len(CONTENT)] = CONTENT
    with path.open('xb') as disk:
        disk.truncate(total * sector)
        for lba, block in ((0, boot), (1, info), (6, boot), (7, info), (reserved, fat),
                           (reserved + fat_sectors, fat), (first_data, root), (first_data + 1, data)):
            disk.seek(lba * sector)
            disk.write(block)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mode', choices=('nvme', 'ahci', 'production'), default='nvme')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars or qemu.parent / 'share' / 'edk2-i386-vars.fd'
    if not qemu.is_file() or not firmware.is_file() or not firmware_vars.is_file():
        parser.error('QEMU, firmware and firmware variable files must exist')
    out = root / 'audit-artifacts' / f'stage13.17-{args.mode}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    production = args.mode == 'production'
    build_command = ['cargo', 'run', '--quiet'] + ([] if production else ['--features', 'stage13-17-test'])
    build = subprocess.run(build_command + ['--', '--print-image'], cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    disk = out / 'foreign.img'
    foreign_fat32(disk)
    before = hashlib.sha256(disk.read_bytes()).hexdigest()
    vars_copy = out / 'OVMF_VARS.fd'
    shutil.copyfile(firmware_vars, vars_copy)
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-drive', f'if=none,id=data,format=raw,file={disk}']
    if args.mode == 'ahci':
        command += ['-device', 'ide-hd,drive=data,bus=ide.0']
    else:
        command += ['-device', 'nvme,drive=data,serial=WOVENHAT1317']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            while process.poll() is None:
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if production and SHELL_MARKER in log:
                    # Give the running system a moment with the disk mounted.
                    time.sleep(5)
                    break
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                time.sleep(0.5)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    if production:
        if '[FS] FAT32 mounted at /mnt from nvme0' not in log:
            errors.append('production boot did not mount the foreign volume')
        if SHELL_MARKER not in log:
            errors.append('production boot did not reach the shell')
    else:
        if process.returncode != 33:
            errors.append(f'exit code {process.returncode}, expected 33')
        if PASS_MARKER not in log:
            errors.append(f'missing {PASS_MARKER!r}')
    after = hashlib.sha256(disk.read_bytes()).hexdigest()
    if after != before:
        errors.append(f'the foreign disk image changed: {before} -> {after}')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.17' in line or 'DRIVERS' in line or '[FS]' in line),
              file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.17 {args.mode}: PASS ({args.cpus} CPU); disk unchanged. Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
