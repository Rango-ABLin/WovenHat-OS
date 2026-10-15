"""Stage 13.18: USB mass storage.

Boots the stage13-18-test kernel on q35 with QEMU's xHCI controller, a USB
keyboard (kbd0) and a 32 MiB raw image attached as usb-storage. Every
sector of the image starts with WOVEN-LBA- and its LBA in eight decimal
digits. The kernel identifies the disk, reads the pattern, writes 64
sectors at LBA 4096 and reads them back, refuses a block past the end, and
then keeps reading while this harness types u, s, b, Enter on the USB
keyboard (after the kernel's ready line). The keys must arrive in order
while storage transfers share the controller's event ring.

Afterwards exactly the 64 written sectors must differ from the original
image, each holding the kernel's S13.18-WRITE- record for its LBA.
"""
import argparse
import importlib.util
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time

READY = '[S13.18] ready for input'
PASS_MARKER = '[S13.18] USB mass storage: PASSED'
SECTOR = 512
BLOCKS = 32 * 1024 * 1024 // SECTOR
WRITE_BASE = 4096
WRITTEN = 64

# The QMP client and key helper are the Stage 13.13 harness's.
_spec = importlib.util.spec_from_file_location('usb_input', Path(__file__).with_name('test-stage13-13-usb-input.py'))
usb_input = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(usb_input)


def patterned_image(path):
    with path.open('xb') as image:
        for lba in range(BLOCKS):
            block = bytearray(SECTOR)
            block[:18] = b'WOVEN-LBA-%08d' % lba
            image.write(block)


def expected_write(lba):
    block = bytearray(SECTOR)
    block[:13] = b'S13.18-WRITE-'
    block[13:21] = struct.pack('<Q', lba)
    block[SECTOR - 1] = lba & 0xff
    return bytes(block)


def inject(qmp):
    for code in ('u', 's', 'b', 'ret'):
        qmp.events('kbd0', [usb_input.key(code, True)])
        time.sleep(0.15)
        qmp.events('kbd0', [usb_input.key(code, False)])
        time.sleep(0.15)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--no-input', action='store_true',
                        help='negative control: type nothing (the gate must fail)')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')
    out = root / 'audit-artifacts' / f'stage13.18-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage13-18-test', '--', '--print-image'],
                           cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    stick = out / 'stick.img'
    patterned_image(stick)
    serial = out / 'serial.log'
    port = usb_input.free_port()
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-qmp', f'tcp:127.0.0.1:{port},server=on,wait=off',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-device', 'qemu-xhci,id=xhci',
               '-device', 'usb-kbd,bus=xhci.0,id=kbd0',
               '-drive', f'if=none,id=stick,format=raw,file={stick}',
               '-device', 'usb-storage,bus=xhci.0,drive=stick,id=stick0']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            qmp = usb_input.Qmp(port, started + 30)
            injected = False
            while process.poll() is None:
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if not injected and READY in log:
                    injected = True
                    if not args.no_input:
                        try:
                            inject(qmp)
                        except OSError:
                            time.sleep(1)
                            if process.poll() is None:
                                raise
                time.sleep(0.2)
        except Exception as error:  # noqa: BLE001 - recorded as a failure
            errors.append(f'QMP: {error}')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    if process.returncode != 33:
        errors.append(f'exit code {process.returncode}, expected 33')
    if PASS_MARKER not in log:
        errors.append(f'missing {PASS_MARKER!r}')
    data = stick.read_bytes()
    changed = [lba for lba in range(BLOCKS)
               if data[lba * SECTOR:lba * SECTOR + 18] != b'WOVEN-LBA-%08d' % lba
               or any(data[lba * SECTOR + 18:(lba + 1) * SECTOR])]
    if changed != list(range(WRITE_BASE, WRITE_BASE + WRITTEN)):
        errors.append(f'changed sectors {changed[:8]}... ({len(changed)}), expected {WRITE_BASE}..{WRITE_BASE + WRITTEN - 1}')
    wrong = [lba for lba in range(WRITE_BASE, WRITE_BASE + WRITTEN)
             if data[lba * SECTOR:(lba + 1) * SECTOR] != expected_write(lba)]
    if wrong:
        errors.append(f'written sectors with the wrong contents: {wrong[:8]}')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.18' in line or 'DRIVERS' in line or '[USB]' in line),
              file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.18: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
