"""Stage 13.14: storage mounted from NVMe or AHCI.

The machine is q35, which has no legacy ATA ports, so the only data disk is a
blank FAT32 image (scripts/create-fat32.py) attached as:

  nvme        an NVMe namespace
  ahci        a disk on the chipset's own AHCI controller (port ide.0)
  production  an NVMe namespace, booting a normal (no test feature) kernel

In the nvme and ahci modes the stage13-14-test kernel mounts the volume,
persists /mnt/WOVEN.TXT, remounts and reads it back from the disk; this
harness then requires the file's bytes and its FAT directory entry to be in
the image on the host. In production mode the boot must report the mount and
reach the shell.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

PASS_MARKER = '[S13.14] NVMe/AHCI storage: PASSED'
MARKER = b'WovenHat 13.14: persisted through the bound storage driver\n'
SHELL_MARKER = '[BOOT] shell-first runtime ready'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mode', choices=('nvme', 'ahci', 'production'), default='nvme')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')
    out = root / 'audit-artifacts' / f'stage13.14-{args.mode}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    production = args.mode == 'production'
    build_command = ['cargo', 'run', '--quiet'] + ([] if production else ['--features', 'stage13-14-test'])
    build = subprocess.run(build_command + ['--', '--print-image'], cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    disk = out / 'fat32.img'
    subprocess.check_call([sys.executable, str(root / 'scripts' / 'create-fat32.py'), str(disk)], cwd=root,
                          stdout=subprocess.DEVNULL)
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-drive', f'if=none,id=data,format=raw,file={disk}']
    if args.mode == 'ahci':
        command += ['-device', 'ide-hd,drive=data,bus=ide.0']
    else:
        command += ['-device', 'nvme,drive=data,serial=WOVENHAT1314']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            while process.poll() is None:
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if production and SHELL_MARKER in log:
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
            errors.append('production boot did not mount the NVMe volume')
        if SHELL_MARKER not in log:
            errors.append('production boot did not reach the shell')
    else:
        if process.returncode != 33:
            errors.append(f'exit code {process.returncode}, expected 33')
        if PASS_MARKER not in log:
            errors.append(f'missing {PASS_MARKER!r}')
        image = disk.read_bytes()
        if MARKER not in image:
            errors.append('the persisted bytes are not in the disk image')
        if b'WOVEN   TXT' not in image:
            errors.append('no WOVEN.TXT directory entry in the disk image')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.14' in line or 'DRIVERS' in line or '[FS]' in line),
              file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.14 {args.mode}: PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
