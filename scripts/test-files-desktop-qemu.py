"""Launch a packaged WovenShell GUI app and capture its desktop window.

This is the live visual acceptance for the WovenFiles and WovenSettings
graphics clients. It
boots the disposable hardware-demo session, signs in through QMP keyboard
events, launches the catalog entry by its launcher query, waits for the
client's WOPN/open marker, and captures the compositor output. The post-launch
frame must materially differ from the signed-in desktop baseline; serial
output alone is not sufficient evidence that the window was presented.
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


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def ppm_pixels(path):
    data = path.read_bytes()
    tokens = []
    index = 0
    while len(tokens) < 4:
        while data[index:index + 1].isspace():
            index += 1
        start = index
        while not data[index:index + 1].isspace():
            index += 1
        tokens.append(data[start:index])
    while data[index:index + 1].isspace():
        index += 1
    if tokens[0] != b'P6' or tokens[3] != b'255':
        raise ValueError(f'unexpected PPM header: {tokens}')
    return int(tokens[1]), int(tokens[2]), data[index:]


def build_shell(root):
    app_dir = root / 'apps' / 'woven-shell'
    env = os.environ.copy()
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=env, check=True)
    return app_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-woven-shell'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--app', choices=('files', 'settings', 'monitor'), default='files')
    args = parser.parse_args()
    app = {
        'files': ('files', 'WovenFiles', 'woven-files'),
        'settings': ('settings', 'WovenSettings', 'woven-settings'),
        'monitor': ('monitor', 'WovenMonitor', 'woven-monitor'),
    }[args.app]
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing files.')

    test_key = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'
    package_env = os.environ.copy()
    package_env.setdefault('WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX', test_key)
    subprocess.run(
        [sys.executable, str(root / 'scripts' / 'package-system-apps.py'), '--version', '0.8.0'],
        cwd=root, env=package_env, check=True,
    )
    output = root / 'target' / 'system-apps' / '0.8.0'
    manifest = json.loads((output / 'manifest.json').read_text(encoding='utf-8'))
    packages = {item['id']: output / item['package'] for item in manifest['apps']}
    env = os.environ.copy()
    env.update({
        'WOVENHAT_CALCULATOR_IMAGE': str(packages['woven-calculator']),
        'WOVENHAT_TERMINAL_IMAGE': str(packages['woven-terminal']),
        'WOVENHAT_FILES_IMAGE': str(packages['woven-files']),
        'WOVENHAT_MONITOR_IMAGE': str(packages['woven-monitor']),
        'WOVENHAT_SETTINGS_IMAGE': str(packages['woven-settings']),
        'WOVENHAT_SECURITY_IMAGE': str(packages['woven-security']),
        'WOVENHAT_EDITOR_IMAGE': str(packages['woven-editor']),
        'WOVENHAT_IMAGES_IMAGE': str(packages['woven-images']),
        'WOVENHAT_UPDATE_IMAGE': str(packages['woven-update']),
        'WOVENHAT_STORE_IMAGE': str(packages['woven-store']),
        'WOVENHAT_SHELL_IMAGE': str(build_shell(root)),
        'WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX': test_key,
    })
    env['CARGO_TARGET_DIR'] = str(root / 'target')
    env['CARGO_BUILD_BUILD_DIR'] = str(root / '.cargo-build')
    image = subprocess.check_output(
        ['cargo', 'run', '--quiet', '--release', '--features', 'hardware-demo',
         '--target-dir', str(root / 'target'), '--', '--print-image'],
        cwd=root, env=env, text=True,
    ).strip()

    out = root / 'audit-artifacts' / f'{app[0]}-desktop-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    serial = out / 'serial.log'
    qmp_port = free_port()
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'pc', '-m', '256M',
               '-smp', str(args.cpus), '-display', 'none', '-serial', f'file:{serial}',
               '-no-reboot', '-qmp', f'tcp:127.0.0.1:{qmp_port},server=on,wait=off',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1', '-vga', 'std']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    process = subprocess.Popen(command, cwd=root, stdout=(out / 'qemu.log').open('w'),
                               stderr=subprocess.STDOUT, creationflags=flags)
    try:
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            try:
                sock = socket.create_connection(('127.0.0.1', qmp_port), timeout=2)
                break
            except OSError:
                time.sleep(0.2)
        else:
            raise RuntimeError('QMP did not open')
        stream = sock.makefile('rwb')
        json.loads(stream.readline())

        def command_qmp(name, arguments=None):
            request = {'execute': name}
            if arguments is not None:
                request['arguments'] = arguments
            stream.write((json.dumps(request) + '\n').encode())
            stream.flush()
            while True:
                response = json.loads(stream.readline())
                if 'error' in response:
                    raise RuntimeError(response['error'])
                if 'return' in response:
                    return response['return']

        command_qmp('qmp_capabilities')
        deadline = time.monotonic() + 180
        while '[SESSION] WovenShell desktop started' not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError('desktop session did not start')
            time.sleep(0.1)
        # Match the established shell acceptance: the session has started,
        # but the lock-screen compositor/input handoff needs one frame before
        # the first PS/2 sequence is meaningful.
        time.sleep(2)
        for attempt in range(2):
            for key in ['1', '2', '3', '4', 'ret']:
                command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 100})
                time.sleep(0.25)
            if 'woven-shell: signed in: ada' in serial.read_text(errors='replace'):
                break
            time.sleep(0.5)
        while 'woven-shell: signed in: ada' not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError('demo sign-in failed')
            time.sleep(0.1)
        baseline = out / 'desktop-before-files.ppm'
        command_qmp('screendump', {'filename': str(baseline)})
        for key in ['f1', *app[0], 'ret']:
            command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 80})
            time.sleep(0.12)
        while f'{app[1]}: window opened' not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError(f'{app[1]} did not open a desktop window')
            time.sleep(0.1)
        # The client logs immediately after WOPN; allow the compositor's
        # event-driven repaint to reach the framebuffer before capture.
        time.sleep(2)
        capture = out / f'woven-{app[0]}.ppm'
        command_qmp('screendump', {'filename': str(capture)})
        time.sleep(0.5)
        width, height, pixels = ppm_pixels(capture)
        before_width, before_height, before = ppm_pixels(baseline)
        if (width, height) != (before_width, before_height):
            raise RuntimeError(f'capture dimensions changed: before={before_width}x{before_height}, after={width}x{height}')
        changed = sum(left != right for left, right in zip(before, pixels)) // 3
        if changed < 1000:
            raise RuntimeError(f'{app[1]} capture did not change the presented desktop: {width}x{height}, changed_pixels={changed}')
        command_qmp('quit')
        sock.close()
    except Exception as error:  # noqa: BLE001 - preserve acceptance evidence
        errors.append(str(error))
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=10)
    log = serial.read_text(errors='replace') if serial.exists() else ''
    (out / 'result.txt').write_text(
        ('PASS\n' if not errors else 'FAILED\n') + '\n'.join(errors) + '\n', encoding='utf-8')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('Evidence:', out, file=sys.stderr)
        return 1
    print(f'{app[1]} desktop visual acceptance: PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
