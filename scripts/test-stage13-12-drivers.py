"""Stage 13.12: boot-time driver bring-up.

Boots the stage13-12-test kernel, whose gate runs the same bring-up a normal
boot runs and checks it against the driver manager and the device registry,
on one of four machines:

  devices     q35 with NVMe, a disk on the chipset's AHCI controller, xHCI
              with a USB keyboard, and HDA: every driver must be Ready.
  nodisk      q35 with no disk on its AHCI controller: AHCI's initialization
              fails, and its PCI binding must be rolled back; the rest absent.
  absent      the i440FX `pc` machine, which has none of these controllers:
              every driver must be absent.
  production  a normal (no test feature) kernel on the `devices` machine: the
              boot must bind every driver and still reach the shell.
  restart     a production kernel with the controller-owned restart gate; the
              NVMe, AHCI, and HDA lifecycles must quiesce, detach, and rebind.

Expectations are written here, not derived from the kernel under test.
"""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

DRIVERS = ('nvme0', 'ahci0', 'xhci0', 'hda0')
EXPECTED = {
    'devices': {'nvme0': 'Ready', 'ahci0': 'Ready', 'xhci0': 'Ready', 'hda0': 'Ready'},
    'nodisk': {'nvme0': 'absent', 'ahci0': 'InitFailed', 'xhci0': 'absent', 'hda0': 'absent'},
    'absent': {'nvme0': 'absent', 'ahci0': 'absent', 'xhci0': 'absent', 'hda0': 'absent'},
    'production': {'nvme0': 'Ready', 'ahci0': 'Ready', 'xhci0': 'Ready', 'hda0': 'Ready'},
    'restart': {'nvme0': 'Ready', 'ahci0': 'Ready', 'xhci0': 'Ready', 'hda0': 'Ready'},
}
PASS_MARKER = '[S13.12] driver bring-up: PASSED'
RESTART_MARKER = '[S31 DRIVER] controller-owned restart: PASSED'
SHELL_MARKER = '[BOOT] shell-first runtime ready'
LINE = re.compile(r'^\[DRIVERS\] (\w+)(?: at ([0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.\d))?: (\w+)', re.M)


def firmware_drive_args(firmware, firmware_vars, out):
    """Use a private writable VARS image; never modify the supplied template."""
    drives = ['-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}']
    if firmware_vars is not None:
        if not firmware_vars.is_file():
            raise FileNotFoundError(f'OVMF VARS template does not exist: {firmware_vars}')
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(firmware_vars, vars_copy)
        drives.extend(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'])
    return drives


def outcomes(log):
    """The first bring-up's outcome per driver (the gate runs it twice)."""
    seen = {}
    for name, _, outcome in LINE.findall(log):
        seen.setdefault(name, outcome)
    return seen


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mode', choices=tuple(EXPECTED), default='devices')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path,
                        help='matching writable OVMF VARS template, copied per QEMU run')
    parser.add_argument('--timeout', type=float, default=240)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars
    if not qemu.is_file() or not firmware.is_file() or (firmware_vars is not None and not firmware_vars.is_file()):
        parser.error('QEMU and firmware files must exist; --firmware-vars must name an existing OVMF VARS template when supplied')
    out = root / 'audit-artifacts' / f'stage13.12-{args.mode}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    production = args.mode == 'production'
    build_command = ['cargo', 'run', '--quiet']
    if args.mode == 'restart':
        build_command += ['--features', 'stage31-driver-restart-test']
    elif not production:
        build_command += ['--features', 'stage13-12-test']
    build = subprocess.run(build_command + ['--', '--print-image'], cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(firmware_vars, vars_copy)
    serial = out / 'serial.log'
    machine = 'pc' if args.mode == 'absent' else 'q35'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', machine, '-m', '4G', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'] if vars_copy else []),
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1']
    if args.mode in ('devices', 'production', 'restart'):
        for name in ('nvme', 'sata'):
            with (out / f'{name}.img').open('wb') as image:
                image.truncate(16 * 1024 * 1024)
        command += ['-drive', f'if=none,id=nvme0,format=raw,file={out / "nvme.img"}',
                    '-device', 'nvme,drive=nvme0,serial=WOVENHAT1312',
                    # q35's own AHCI controller (00:1f.2); its first port is ide.0.
                    '-drive', f'if=none,id=sata0,format=raw,file={out / "sata.img"}',
                    '-device', 'ide-hd,drive=sata0,bus=ide.0',
                    '-device', 'qemu-xhci,id=xhci', '-device', 'usb-kbd,bus=xhci.0',
                    '-audiodev', 'none,id=wh_audio', '-device', 'intel-hda,id=hda',
                    '-device', 'hda-duplex,bus=hda.0,audiodev=wh_audio']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        log = ''
        try:
            while True:
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if production and SHELL_MARKER in log:
                    break
                if process.poll() is not None:
                    break
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                time.sleep(0.5)
        finally:
            # A production boot never exits by itself; this QEMU is ours.
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    if production:
        if SHELL_MARKER not in log:
            errors.append('production boot did not reach the shell')
    else:
        if process.returncode != 33:
            errors.append(f'exit code {process.returncode}, expected 33')
        if args.mode == 'restart' and RESTART_MARKER not in log:
            errors.append(f'missing {RESTART_MARKER!r}')
        elif args.mode != 'restart' and PASS_MARKER not in log:
            errors.append(f'missing {PASS_MARKER!r}')
    got = outcomes(log)
    for name, expected in EXPECTED[args.mode].items():
        if got.get(name) != expected:
            errors.append(f'{name}: {got.get(name)!r}, expected {expected!r}')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print(log[-8000:], file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.12 {args.mode}: PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
