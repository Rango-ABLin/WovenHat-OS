"""Stage 13.13: USB keyboard and mouse into WovenInput.

Boots the stage13-13-test kernel on q35 with QEMU's xHCI controller, a USB
keyboard (kbd0) and a USB mouse (mouse0). When the kernel prints its ready
line, input is injected over QMP. QEMU delivers input to the most recently
added handler, which is the USB keyboard and mouse (added after the chipset's
PS/2 devices); the kernel also requires USB reports and device interrupts,
so input that took the PS/2 path would fail the gate:

  keys   w, h, 1, shift+a, Enter        -> 'w' 'h' '1' 'A' Enter
  mouse  right 40, down 25, left click  -> motion (40, 25), press, release

The kernel checks the events arrived exactly and through device interrupts.
QMP listens on a free local port chosen here, so parallel QEMUs never share
one.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

READY = '[S13.13] ready for input'
PASS_MARKER = '[S13.13] USB input: PASSED'


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


class Qmp:
    def __init__(self, port, deadline):
        while True:
            try:
                self.sock = socket.create_connection(('127.0.0.1', port), timeout=5)
                break
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.2)
        self.file = self.sock.makefile('rw', encoding='utf-8', newline='\n')
        self.file.readline()  # greeting
        self.command('qmp_capabilities')

    def command(self, name, **arguments):
        message = {'execute': name}
        if arguments:
            message['arguments'] = arguments
        self.file.write(json.dumps(message) + '\n')
        self.file.flush()
        while True:
            reply = json.loads(self.file.readline())
            if 'return' in reply:
                return reply['return']
            if 'error' in reply:
                raise RuntimeError(f'{name}: {reply["error"]}')

    def events(self, _device, events):
        # input-send-event's `device` names a display console, not an input
        # device; with -display none QEMU aborts on it, so it is not passed.
        self.command('input-send-event', events=events)


def key(code, down):
    return {'type': 'key', 'data': {'down': down, 'key': {'type': 'qcode', 'data': code}}}


def inject(qmp):
    for code in ('w', 'h', '1'):
        qmp.events('kbd0', [key(code, True)])
        time.sleep(0.15)
        qmp.events('kbd0', [key(code, False)])
        time.sleep(0.15)
    qmp.events('kbd0', [key('shift', True)])
    time.sleep(0.15)
    qmp.events('kbd0', [key('a', True)])
    time.sleep(0.15)
    qmp.events('kbd0', [key('a', False)])
    time.sleep(0.15)
    qmp.events('kbd0', [key('shift', False)])
    time.sleep(0.15)
    qmp.events('kbd0', [key('ret', True)])
    time.sleep(0.15)
    qmp.events('kbd0', [key('ret', False)])
    time.sleep(0.15)
    qmp.events('mouse0', [{'type': 'rel', 'data': {'axis': 'x', 'value': 40}},
                          {'type': 'rel', 'data': {'axis': 'y', 'value': 25}}])
    time.sleep(0.2)
    qmp.events('mouse0', [{'type': 'btn', 'data': {'down': True, 'button': 'left'}}])
    time.sleep(0.2)
    qmp.events('mouse0', [{'type': 'btn', 'data': {'down': False, 'button': 'left'}}])


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=240)
    parser.add_argument('--no-input', action='store_true',
                        help='negative control: inject nothing (the gate must fail)')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')
    out = root / 'audit-artifacts' / f'stage13.13-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage13-13-test', '--', '--print-image'],
                           cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    port = free_port()
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-qmp', f'tcp:127.0.0.1:{port},server=on,wait=off',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-device', 'qemu-xhci,id=xhci',
               '-device', 'usb-kbd,bus=xhci.0,id=kbd0',
               '-device', 'usb-mouse,bus=xhci.0,id=mouse0']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            qmp = Qmp(port, started + 30)
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
                            # The guest may finish, and QEMU exit, before the
                            # last event is sent; anything else is an error.
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
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.13' in line or 'DRIVERS' in line), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.13: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
