"""Stage 20.14a: two monitors through virtio-gpu.

Boots the stage20-14-test kernel on q35 with the standard VGA as the boot
console and a virtio-gpu with two outputs (id gpu0). The kernel takes the GPU
over, gives each head a 640x400 resource and draws:

  head 0  an exact gradient: R = x*255/639, G = y*255/399, B = 0x40
  head 1  a 16-pixel checkerboard of (0x20,0x40,0xc0) and (0xf0,0xd0,0x20)

When it prints its ready line, this harness captures each head from the host
with QMP screendump and requires every pixel to match.
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

READY = '[S20.14] ready for capture'
PASS_MARKER = '[S20.14] two monitors: PASSED'
WIDTH, HEIGHT = 640, 400


def expected(head, x, y):
    if head == 0:
        return (x * 255 // (WIDTH - 1), y * 255 // (HEIGHT - 1), 0x40)
    return (0x20, 0x40, 0xc0) if (x // 16 + y // 16) % 2 == 0 else (0xf0, 0xd0, 0x20)


def read_ppm(path):
    data = path.read_bytes()
    parts = []
    index = 0
    while len(parts) < 4:
        while data[index:index + 1].isspace():
            index += 1
        if data[index:index + 1] == b'#':
            index = data.index(b'\n', index) + 1
            continue
        start = index
        while not data[index:index + 1].isspace():
            index += 1
        parts.append(data[start:index])
    index += 1
    if parts[0] != b'P6' or parts[3] != b'255':
        raise ValueError(f'unexpected PPM header {parts}')
    return int(parts[1]), int(parts[2]), data[index:]


def head_errors(head, path):
    width, height, pixels = read_ppm(path)
    if (width, height) != (WIDTH, HEIGHT):
        return [f'head {head} capture is {width}x{height}, expected {WIDTH}x{HEIGHT}']
    wrong = 0
    first = None
    for y in range(HEIGHT):
        for x in range(WIDTH):
            offset = (y * WIDTH + x) * 3
            got = tuple(pixels[offset:offset + 3])
            if got != expected(head, x, y):
                wrong += 1
                if first is None:
                    first = (x, y, got, expected(head, x, y))
    return [f'head {head}: {wrong} pixels differ, first {first}'] if wrong else []


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def qmp_session(port, deadline):
    while True:
        try:
            sock = socket.create_connection(('127.0.0.1', port), timeout=10)
            break
        except OSError:
            if time.time() > deadline:
                raise
            time.sleep(0.2)
    stream = sock.makefile('rw', encoding='utf-8', newline='\n')
    stream.readline()

    def command(name, **arguments):
        message = {'execute': name}
        if arguments:
            message['arguments'] = arguments
        stream.write(json.dumps(message) + '\n')
        stream.flush()
        while True:
            reply = json.loads(stream.readline())
            if 'return' in reply:
                return reply['return']
            if 'error' in reply:
                raise RuntimeError(f'{name}: {reply["error"]}')

    command('qmp_capabilities')
    return sock, command


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
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
    out = root / 'audit-artifacts' / f'stage20.14-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage20-14-test', '--', '--print-image'],
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
               '-vga', 'std',
               '-device', 'virtio-gpu-pci,id=gpu0,max_outputs=2']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    captured = False
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            sock, qmp = qmp_session(port, started + 30)
            while process.poll() is None:
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if not captured and READY in log:
                    captured = True
                    for head in (0, 1):
                        qmp('screendump', filename=str(out / f'head{head}.ppm'), device='gpu0', head=head)
                time.sleep(0.2)
            sock.close()
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
    if captured:
        for head in (0, 1):
            path = out / f'head{head}.ppm'
            if path.exists():
                errors += head_errors(head, path)
            else:
                errors.append(f'no capture of head {head}')
    else:
        errors.append('the kernel never reported both heads ready')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S20.14' in line), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 20.14: PASS ({args.cpus} CPU, both heads verified pixel for pixel). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
