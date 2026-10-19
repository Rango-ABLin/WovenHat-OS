"""Launch a packaged WovenShell GUI app and capture its desktop window.

This is the live visual acceptance for the production desktop clients. It
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
    parser.add_argument('--app', choices=('calculator', 'files', 'settings', 'monitor', 'security', 'editor', 'images', 'update', 'store', 'terminal'), default='files')
    parser.add_argument('--exercise-resize', action='store_true', help='exercise the WovenShell WSIZ/WRSZ path (Calculator, Terminal, Settings, Monitor, Security, Editor, Images, Update, Store, or Files)')
    parser.add_argument('--exercise-pointer', action='store_true', help='exercise the desktop PS/2 pointer cursor path with relative motion and a click')
    parser.add_argument('--exercise-client-pointer', action='store_true', help='exercise client pointer forwarding through the desktop PS/2 path')
    parser.add_argument('--exercise-client-click', action='store_true', help='exercise a bounded client control through forwarded PS/2 pointer input')
    args = parser.parse_args()
    if args.exercise_resize and args.app not in ('calculator', 'terminal', 'settings', 'monitor', 'security', 'editor', 'images', 'update', 'store', 'files'):
        parser.error('--exercise-resize currently supports calculator, terminal, settings, monitor, security, editor, images, update, store, and files only')
    if args.exercise_client_click and args.app not in ('calculator', 'settings', 'files'):
        parser.error('--exercise-client-click currently supports calculator, settings, and files only')
    app = {
        'calculator': ('calculator', 'WovenCalculator', 'woven-calculator', 'calculator'),
        'files': ('files', 'WovenFiles', 'woven-files', 'files'),
        'settings': ('settings', 'WovenSettings', 'woven-settings', 'settings'),
        'monitor': ('monitor', 'WovenMonitor', 'woven-monitor', 'monitor'),
        'security': ('security', 'WovenSecurity', 'woven-security', 'security'),
        'editor': ('editor', 'WovenEditor', 'woven-editor', 'editor'),
        'images': ('images', 'WovenImages', 'woven-images', 'images'),
        # Avoid the qcode edge case for the `u` key on this QEMU build; the
        # substring still uniquely selects WovenUpdate.
        'update': ('update', 'WovenUpdate', 'woven-update', 'date'),
        'store': ('store', 'WovenStore', 'woven-store', 'store'),
        'terminal': ('terminal', 'WovenTerminal', 'woven-terminal', 'terminal'),
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
        time.sleep(4)
        for attempt in range(2):
            for key in ['1', '2', '3', '4', 'ret']:
                command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 150})
                time.sleep(0.4)
            if 'woven-shell: signed in: ada' in serial.read_text(errors='replace'):
                break
            time.sleep(0.5)
        while 'woven-shell: signed in: ada' not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError('demo sign-in failed')
            time.sleep(0.1)
        baseline = out / 'desktop-before-files.ppm'
        command_qmp('screendump', {'filename': str(baseline)})
        for key in ['f1', *app[3], 'ret']:
            command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 80})
            time.sleep(0.12)
        while f'{app[1]}: window opened' not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError(f'{app[1]} did not open a desktop window')
            time.sleep(0.1)
        # The client logs immediately after WOPN; allow the compositor's
        # event-driven repaint to reach the framebuffer before capture.
        time.sleep(2)
        input_baseline = None
        input_sequence = {
            'terminal': ['h', 'e', 'l', 'l', 'o', 'backspace', 'p', 'ret'],
            'calculator': ['4', '2'] if args.exercise_client_click else ['4', '2', 'backspace', '2', 'ret'],
        }.get(args.app)
        if input_sequence is not None:
            input_baseline = out / f'woven-{app[0]}-before-input.ppm'
            command_qmp('screendump', {'filename': str(input_baseline)})
            for key in input_sequence:
                command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 160})
                time.sleep(0.3)
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
        if input_baseline is not None:
            input_width, input_height, input_pixels = ppm_pixels(input_baseline)
            if (input_width, input_height) != (width, height):
                raise RuntimeError('terminal input capture dimensions changed')
            input_changed = sum(left != right for left, right in zip(input_pixels, pixels)) // 3
            if input_changed < 100:
                raise RuntimeError(f'{app[1]} input did not change its presented surface: changed_pixels={input_changed}')
        pointer_baseline = None
        if args.exercise_pointer:
            pointer_baseline = out / f'woven-{app[0]}-before-pointer.ppm'
            command_qmp('screendump', {'filename': str(pointer_baseline)})
            command_qmp('input-send-event', {'events': [
                {'type': 'rel', 'data': {'axis': 'x', 'value': 40}},
                {'type': 'rel', 'data': {'axis': 'y', 'value': 25}},
            ]})
            time.sleep(0.5)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': True, 'button': 'left'}},
            ]})
            time.sleep(0.2)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': False, 'button': 'left'}},
            ]})
            time.sleep(1)
            pointer_capture = out / f'woven-{app[0]}-after-pointer.ppm'
            command_qmp('screendump', {'filename': str(pointer_capture)})
            pointer_width, pointer_height, pointer_pixels = ppm_pixels(pointer_capture)
            before_pointer_width, before_pointer_height, before_pointer = ppm_pixels(pointer_baseline)
            if (pointer_width, pointer_height) != (before_pointer_width, before_pointer_height):
                raise RuntimeError('pointer capture dimensions changed')
            pointer_changed = sum(left != right for left, right in zip(before_pointer, pointer_pixels)) // 3
            if pointer_changed < 100:
                raise RuntimeError(f'{app[1]} pointer motion did not change its presented cursor/frame: changed_pixels={pointer_changed}')
        if args.exercise_client_pointer and args.app not in ('calculator', 'terminal', 'files', 'settings', 'monitor', 'security', 'editor', 'images', 'update', 'store'):
            parser.error('--exercise-client-pointer currently supports the packaged desktop clients only')
        if args.exercise_client_pointer:
            client_pointer_baseline = out / f'woven-{app[0]}-before-client-pointer.ppm'
            command_qmp('screendump', {'filename': str(client_pointer_baseline)})
            # The first window opens at (40, 40) in the 1280x800 QEMU
            # desktop. Move from the established centre to a safe point in
            # Calculator's client surface, then click it.
            command_qmp('input-send-event', {'events': [
                {'type': 'rel', 'data': {'axis': 'x', 'value': -540}},
                {'type': 'rel', 'data': {'axis': 'y', 'value': -320}},
            ]})
            time.sleep(0.5)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': True, 'button': 'left'}},
            ]})
            time.sleep(0.2)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': False, 'button': 'left'}},
            ]})
            while f'{app[1]}: pointer updated' not in serial.read_text(errors='replace'):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('WovenCalculator did not receive a forwarded pointer event')
                time.sleep(0.1)
            time.sleep(1)
            client_pointer_capture = out / f'woven-{app[0]}-after-client-pointer.ppm'
            command_qmp('screendump', {'filename': str(client_pointer_capture)})
            client_width, client_height, client_pixels = ppm_pixels(client_pointer_capture)
            before_client_width, before_client_height, before_client = ppm_pixels(client_pointer_baseline)
            if (client_width, client_height) != (before_client_width, before_client_height):
                raise RuntimeError('client pointer capture dimensions changed')
            client_changed = sum(left != right for left, right in zip(before_client, client_pixels)) // 3
            if client_changed < 20:
                raise RuntimeError(f'{app[1]} client pointer did not change its presented surface: changed_pixels={client_changed}')
        if args.exercise_client_click:
            control = {
                'calculator': {'move': (-100, -41), 'marker': 'WovenCalculator: pointer evaluated', 'name': 'calculator'},
                'settings': {'move': (0, 40), 'marker': 'WovenSettings: pointer refreshed', 'name': 'settings'},
                'files': {'move': (-40, 56), 'marker': 'WovenFiles: pointer refreshed', 'name': 'files'},
            }[args.app]
            click_baseline = out / f"woven-{control['name']}-before-client-click.ppm"
            command_qmp('screendump', {'filename': str(click_baseline)})
            # The first window opens at (40, 40) in the logical 640x400
            # desktop. Each client control uses a fixed bounded local target.
            command_qmp('input-send-event', {'events': [
                {'type': 'rel', 'data': {'axis': 'x', 'value': control['move'][0]}},
                {'type': 'rel', 'data': {'axis': 'y', 'value': control['move'][1]}},
            ]})
            time.sleep(0.5)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': True, 'button': 'left'}},
            ]})
            time.sleep(0.2)
            command_qmp('input-send-event', {'events': [
                {'type': 'btn', 'data': {'down': False, 'button': 'left'}},
            ]})
            command_qmp('screendump', {'filename': str(out / f"woven-{control['name']}-after-client-click.ppm")})
            while control['marker'] not in serial.read_text(errors='replace'):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f'{app[1]} did not activate its bounded control through forwarded pointer input')
                time.sleep(0.1)
            time.sleep(1)
            click_capture = out / f"woven-{control['name']}-after-client-click.ppm"
            command_qmp('screendump', {'filename': str(click_capture)})
            click_width, click_height, click_pixels = ppm_pixels(click_capture)
            before_click_width, before_click_height, before_click = ppm_pixels(click_baseline)
            if (click_width, click_height) != (before_click_width, before_click_height):
                raise RuntimeError('client click capture dimensions changed')
            click_changed = sum(left != right for left, right in zip(before_click, click_pixels)) // 3
            if click_changed < 100:
                raise RuntimeError(f'{app[1]} Evaluate click did not change its presented result: changed_pixels={click_changed}')
        resize_baseline = None
        if args.exercise_resize:
            resize_baseline = out / f'woven-{app[0]}-before-resize.ppm'
            command_qmp('screendump', {'filename': str(resize_baseline)})
            command_qmp('send-key', {'keys': [{'type': 'qcode', 'data': 'f2'}], 'hold-time': 160})
            while f'woven-shell: window resized: {app[1].replace("Woven", "")}' not in serial.read_text(errors='replace'):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f'{app[1]} did not complete the WSIZ/WRSZ resize handshake')
                time.sleep(0.1)
            time.sleep(1)
            resized_capture = out / f'woven-{app[0]}-after-resize.ppm'
            command_qmp('screendump', {'filename': str(resized_capture)})
            resize_width, resize_height, resize_pixels = ppm_pixels(resized_capture)
            before_resize_width, before_resize_height, before_resize = ppm_pixels(resize_baseline)
            if (resize_width, resize_height) != (before_resize_width, before_resize_height):
                raise RuntimeError('resize capture dimensions changed')
            resize_changed = sum(left != right for left, right in zip(before_resize, resize_pixels)) // 3
            if resize_changed < 1000:
                raise RuntimeError(f'{app[1]} resize did not change the presented desktop: changed_pixels={resize_changed}')
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
