"""Stage 20.14b: the desktop across two monitors.

Builds WovenShell and hello-gui, boots the stage20-14b-test kernel on q35 with
the standard VGA as boot console and a two-output virtio-gpu (gpu0). The
kernel makes the GPU the display, starts the shell, checks the taskbar is on
the primary monitor only and drags window A onto the second monitor, then
prints sample pixels as `head=H x=X y=Y rgb=RRGGBB`. This harness captures
both heads from the host and requires every sample to match.
"""
import argparse
import json
import re
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

READY = '[S20.14B] ready for capture'
PASS_MARKER = '[S20.14B] desktop across monitors: PASSED'
SAMPLE = re.compile(r'\[S20\.14B\] sample head=(\d) x=(\d+) y=(\d+) rgb=([0-9a-f]{6})')


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


def sample_errors(log, out):
    samples = SAMPLE.findall(log)
    if len(samples) < 6:
        return [f'only {len(samples)} samples reported']
    captures = {head: read_ppm(out / f'head{head}.ppm') for head in (0, 1)}
    errors = []
    for head, x, y, rgb in samples:
        width, height, pixels = captures[int(head)]
        x, y = int(x), int(y)
        if x >= width or y >= height:
            errors.append(f'sample ({head}, {x}, {y}) outside a {width}x{height} capture')
            continue
        offset = (y * width + x) * 3
        got = pixels[offset:offset + 3].hex()
        if got != rgb:
            errors.append(f'head {head} ({x}, {y}): captured {got}, kernel shows {rgb}')
    return errors


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
    out = root / 'audit-artifacts' / f'stage20.14b-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    env = dict(os.environ)
    for app, variable, binary in (('woven-shell', 'WOVENHAT_SHELL_IMAGE', 'wovenhat-woven-shell'),
                                  ('hello-gui', 'WOVENHAT_HELLO_GUI_IMAGE', 'wovenhat-hello-gui')):
        app_dir = root / 'apps' / app
        app_env = dict(os.environ)
        app_env.pop('CARGO_BUILD_BUILD_DIR', None)
        app_env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
        if subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=app_env).returncode:
            return 1
        env[variable] = str(app_dir / 'target' / 'x86_64-unknown-none' / 'release' / binary)
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage20-14b-test', '--', '--print-image'],
                           cwd=root, env=env, text=True, capture_output=True)
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
        if all((out / f'head{head}.ppm').exists() for head in (0, 1)):
            errors += sample_errors(log, out)
        else:
            errors.append('a head was not captured')
    else:
        errors.append('the kernel never reported both heads ready')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S20.14B' in line), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 20.14b: PASS ({args.cpus} CPU, samples verified on both heads). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
