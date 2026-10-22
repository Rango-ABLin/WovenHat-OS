"""Exercise normal-boot PS/2 input through IOAPIC using QMP key events."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time


def build_woven_shell(root):
    """Build WovenShell, the desktop session a system-apps image embeds
    (Stage 20.15), in its own target directory; returns the image path."""
    app_dir = root / 'apps' / 'woven-shell'
    env = os.environ.copy()
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=env, check=True)
    return app_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-woven-shell'

def apps_installed_marker(app_count: int) -> str:
    """Kernel marker for a fully installed signed system-app catalog."""
    return f'[APPS] {app_count} signed applications installed'


def normal_boot_serial_errors(log: str, system_apps: bool, app_count: int | None = None) -> list[str]:
    """Reject fatal evidence even when all positive boot markers appeared.

    `app_count` is the number of applications in the packaged catalog
    manifest; it is required for a system-apps boot, so the expected install
    marker follows the catalog instead of a hard-coded count.
    """
    errors = []
    normalized = log.lower()
    for marker in ('kernel panic', 'panicked at', 'security violation', 'security: failed'):
        if marker in normalized:
            errors.append(f'Guest serial contains fatal evidence: {marker}')
    for line in log.splitlines():
        if line.startswith(('[BOOT]', '[SESSION]', '[APPS]', '[SMP]')) and ('FAILED' in line or ' FAIL' in line):
            errors.append(f'Contradictory normal boot failure marker: {line}')
    expected = ('[BOOT] desktop-first runtime ready' if system_apps
                else '[BOOT] shell-first runtime ready')
    if expected not in log:
        errors.append(f'Missing normal boot marker: {expected}')
    if system_apps:
        if app_count is None:
            raise ValueError('a system-apps boot needs the packaged app count')
        for marker in (apps_installed_marker(app_count),
                       '[SESSION] WovenShell desktop started',
                       '[SESSION] diagnostic console',
                       '[SESSION] back to the desktop'):
            if marker not in log:
                errors.append(f'Missing system-apps marker: {marker}')
    if '[SMP] acknowledged TLB shootdowns: PASSED' not in log:
        errors.append('Missing keyboard-driven SMP shell acceptance marker')
    return errors



def validate_desktop_frame(path: Path) -> tuple[int, int]:
    """Require a complete, non-uniform QEMU P6 framebuffer capture.

    A successful screendump command can still capture a blank screen. Keep
    the PPM as evidence and fail the normal desktop boot acceptance if it
    contains no visible content.
    """
    data = path.read_bytes()
    # QEMU's screendump uses binary P6 with an optional comment in its header.
    offset = 0
    tokens = []
    while len(tokens) < 4:
        while offset < len(data) and data[offset] in b' \t\r\n':
            offset += 1
        if offset < len(data) and data[offset] == ord('#'):
            end = data.find(b'\n', offset)
            if end < 0:
                raise ValueError('Unterminated PPM header comment')
            offset = end + 1
            continue
        start = offset
        while offset < len(data) and data[offset] not in b' \t\r\n#':
            offset += 1
        if start == offset:
            raise ValueError('Incomplete PPM header')
        tokens.append(data[start:offset])
    try:
        magic, width_raw, height_raw, maximum_raw = tokens
        width, height, maximum = int(width_raw), int(height_raw), int(maximum_raw)
    except ValueError as error:
        raise ValueError('Invalid PPM dimensions or color depth') from error
    if magic != b'P6' or width < 64 or height < 64 or maximum != 255:
        raise ValueError('Invalid QEMU desktop framebuffer format or dimensions')
    if offset >= len(data) or data[offset] not in b' \t\r\n':
        raise ValueError('Missing PPM pixel separator')
    # Exactly one delimiter follows maxval; further bytes are pixel content
    # and may legitimately start with whitespace.
    if data[offset:offset + 2] == b'\r\n':
        offset += 2
    else:
        offset += 1
    pixels = data[offset:]
    if len(pixels) != width * height * 3:
        raise ValueError('Truncated or oversized desktop framebuffer')
    first = pixels[:3]
    if all(pixels[index:index + 3] == first for index in range(0, len(pixels), 3)):
        raise ValueError('Desktop framebuffer is a uniform blank image')
    return width, height


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path, help='matching writable OVMF VARS template')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4, 8, 16), default=4)
    parser.add_argument('--system-apps', action='store_true',
                        help='build the production system-app image and verify normal-boot installation')
    parser.add_argument('--hardware-demo', action='store_true',
                        help='build the disposable demo image and verify demo sign-in')
    args = parser.parse_args()
    if args.hardware_demo:
        args.system_apps = True
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars
    if not qemu.is_file() or not firmware.is_file() or (firmware_vars is not None and not firmware_vars.is_file()):
        parser.error('Set --qemu and --firmware to existing files.')
    # bootloader runs a nested `cargo install` in release mode. Sharing the
    # parent's release artifact/build directories deadlocks on Cargo's locks.
    # Keep the main build in the launcher-selected directories; only the nested
    # installer inherits dedicated project-local subdirectories.
    build_env = os.environ.copy()
    feature_arguments = []
    app_count = None
    if args.system_apps:
        # Acceptance only: production images must replace this deterministic
        # root with a provisioned release public key.
        test_public_key = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'
        package_env = os.environ.copy()
        package_env.setdefault('WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX', test_public_key)
        subprocess.run(
            [sys.executable, str(root / 'scripts' / 'package-system-apps.py'), '--version', '0.8.0'],
            cwd=root,
            env=package_env,
            check=True,
        )
        output = root / 'target' / 'system-apps' / '0.8.0'
        manifest = json.loads((output / 'manifest.json').read_text(encoding='utf-8'))
        packages = {item['id']: output / item['package'] for item in manifest['apps']}
        app_count = len(manifest['apps'])
        build_env['WOVENHAT_CALCULATOR_IMAGE'] = str(packages['woven-calculator'])
        build_env['WOVENHAT_TERMINAL_IMAGE'] = str(packages['woven-terminal'])
        build_env['WOVENHAT_FILES_IMAGE'] = str(packages['woven-files'])
        build_env['WOVENHAT_MONITOR_IMAGE'] = str(packages['woven-monitor'])
        build_env['WOVENHAT_SETTINGS_IMAGE'] = str(packages['woven-settings'])
        build_env['WOVENHAT_SECURITY_IMAGE'] = str(packages['woven-security'])
        build_env['WOVENHAT_EDITOR_IMAGE'] = str(packages['woven-editor'])
        build_env['WOVENHAT_IMAGES_IMAGE'] = str(packages['woven-images'])
        build_env['WOVENHAT_UPDATE_IMAGE'] = str(packages['woven-update'])
        build_env['WOVENHAT_STORE_IMAGE'] = str(packages['woven-store'])
        build_env['WOVENHAT_INSTALLER_IMAGE'] = str(packages['woven-installer'])
        package_service = subprocess.check_output(
            [sys.executable, str(root / 'scripts' / 'build-package-service.py')],
            cwd=root,
            env=package_env,
            text=True,
        ).strip()
        build_env['WOVENHAT_PACKAGE_IMAGE'] = package_service
        build_env.setdefault('WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX', test_public_key)
        build_env['WOVENHAT_SHELL_IMAGE'] = str(build_woven_shell(root))
        feature_arguments = ['--features', 'hardware-demo' if args.hardware_demo else 'system-apps']
    cargo_command = ['cargo']
    target_arguments = []
    for variable, setting, default in (
            ('CARGO_TARGET_DIR', 'build.target-dir', 'target'),
            ('CARGO_BUILD_BUILD_DIR', 'build.build-dir', '.cargo-build')):
        directory = Path(build_env.get(variable, str(root / default)))
        if not directory.is_absolute():
            directory = root / directory
        if variable == 'CARGO_TARGET_DIR':
            target_arguments = ['--target-dir', str(directory)]
        else:
            cargo_command.extend(['--config', setting + '=' + json.dumps(str(directory))])
        build_env[variable] = str(directory / 'bootloader')
    image = subprocess.check_output(
        cargo_command + ['run', '--quiet', '--release'] + feature_arguments + target_arguments + ['--', '--print-image'],
        cwd=root, env=build_env, text=True).strip()
    # Keep evidence for each CPU/boot mode distinct; a later run must not
    # silently overwrite the previous run's serial or QEMU logs.
    out = root / 'target' / 'shell-regression' / (
        f'{"system-apps" if args.system_apps else "diagnostic"}-{args.cpus}cpu-{time.time_ns()}'
    )
    out.mkdir(parents=True, exist_ok=False)
    serial = out / 'serial.log'
    serial.write_text('')
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(firmware_vars, vars_copy)
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'pc', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-qmp', f'tcp:127.0.0.1:{port},server=on,wait=off',
               '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
               *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'] if vars_copy is not None else []),
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    with (out / 'qemu.log').open('w') as errors:
        process = subprocess.Popen(command, cwd=root, stdout=errors, stderr=errors, creationflags=flags)
        try:
            deadline = time.monotonic() + 180
            expected_boot = ('[BOOT] desktop-first runtime ready' if args.system_apps
                             else '[BOOT] shell-first runtime ready')
            while expected_boot not in serial.read_text(errors='replace'):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Normal boot failed; inspect shell-regression logs')
                time.sleep(0.1)
            if args.system_apps and apps_installed_marker(app_count) not in serial.read_text(errors='replace'):
                raise RuntimeError('Production system-app catalog was not installed; inspect shell-regression logs')
            if args.system_apps and '[SERVICES] woven-package published as pid ' not in serial.read_text(errors='replace'):
                raise RuntimeError('WovenPackage production service was not published; inspect shell-regression logs')
            if args.system_apps and '[SERVICES] woven-update published as pid ' not in serial.read_text(errors='replace'):
                raise RuntimeError('WovenUpdate production service was not published; inspect shell-regression logs')
            if args.hardware_demo and '[DEMO] sample accounts installed in volatile VFS (ada, lin)' not in serial.read_text(errors='replace'):
                raise RuntimeError('Hardware-demo accounts were not provisioned; inspect shell-regression logs')
            time.sleep(1)
            with socket.create_connection(('127.0.0.1', port), timeout=5) as connection:
                stream = connection.makefile('rwb')
                json.loads(stream.readline())

                def execute(command, arguments=None):
                    request = {'execute': command}
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

                execute('qmp_capabilities')
                if args.system_apps:
                    # The desktop session holds the keyboard (Stage 20.15):
                    # F12 switches to the diagnostic console first.
                    deadline = time.monotonic() + 60
                    while '[SESSION] WovenShell desktop started' not in serial.read_text(errors='replace'):
                        if process.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError('The desktop session did not start; inspect shell-regression logs')
                        time.sleep(0.1)
                    if args.hardware_demo:
                        deadline = time.monotonic() + 60
                        while 'woven-shell: desktop ready' not in serial.read_text(errors='replace'):
                            if process.poll() is not None or time.monotonic() > deadline:
                                raise RuntimeError('The desktop did not become input-ready; inspect shell-regression logs')
                            time.sleep(0.1)
                        # Let the compositor publish the shell's input focus
                        # after its first frame before injecting the PIN.
                        time.sleep(1)
                        for key in ['1', '2', '3', '4', 'ret']:
                            execute('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 80})
                            time.sleep(0.12)
                        deadline = time.monotonic() + 15
                        while 'woven-shell: signed in: ada' not in serial.read_text(errors='replace'):
                            if process.poll() is not None or time.monotonic() > deadline:
                                raise RuntimeError('The demo account did not sign in; inspect shell-regression logs')
                            time.sleep(0.1)
                    time.sleep(2)
                    # Retain the framebuffer *before* switching to diagnostics.
                    # A serial startup marker alone cannot prove that a desktop
                    # frame is visible; reviewers must inspect this image.
                    screenshot = out / 'desktop-before-f12.ppm'
                    execute('screendump', {'filename': str(screenshot)})
                    if not screenshot.is_file() or screenshot.stat().st_size == 0:
                        raise RuntimeError('No pre-diagnostic display screenshot was captured')
                    try:
                        frame_width, frame_height = validate_desktop_frame(screenshot)
                    except ValueError as error:
                        raise RuntimeError(f'Invalid pre-F12 desktop framebuffer: {error}') from error
                    print(f'Normal desktop first-frame evidence: {frame_width}x{frame_height}', flush=True)
                    execute('send-key', {'keys': [{'type': 'qcode', 'data': 'f12'}], 'hold-time': 40})
                    deadline = time.monotonic() + 30
                    while '[SESSION] diagnostic console' not in serial.read_text(errors='replace'):
                        if process.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError('F12 did not open the diagnostic console; inspect shell-regression logs')
                        time.sleep(0.1)
                for key in list('smptest') + ['ret']:
                    execute('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 40})
                    time.sleep(0.1)
                # The SMP shell command runs the full migration/TLB suite on
                # every requested CPU.  Under host TCG, the 16-CPU profile
                # can legitimately take several minutes even though the
                # guest is making progress; keep the normal 60-second gate
                # for the supported 1/2/4/8 profiles and give the largest
                # bounded profile a proportional diagnostic window.
                smp_deadline = 300 if args.cpus == 16 else 60
                deadline = time.monotonic() + smp_deadline
                while '[SMP] acknowledged TLB shootdowns: PASSED' not in serial.read_text(errors='replace'):
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError('Keyboard/SMP shell command failed; inspect shell-regression logs')
                    time.sleep(0.1)
                if args.system_apps:
                    # Return to the graphical desktop through real QMP keyboard
                    # input after verifying the diagnostic console's SMP path.
                    execute('send-key', {'keys': [{'type': 'qcode', 'data': 'f12'}], 'hold-time': 40})
                    deadline = time.monotonic() + 30
                    while '[SESSION] back to the desktop' not in serial.read_text(errors='replace'):
                        if process.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError('F12 did not return focus to WovenShell')
                        time.sleep(0.1)
                    time.sleep(1)
                    restored = out / 'desktop-after-f12-return.ppm'
                    execute('screendump', {'filename': str(restored)})
                    if not restored.is_file() or restored.stat().st_size == 0:
                        raise RuntimeError('No restored-desktop framebuffer was captured')
                    try:
                        validate_desktop_frame(restored)
                    except ValueError as error:
                        raise RuntimeError(f'Invalid restored desktop framebuffer: {error}') from error
                # Check the complete serial trace, not just the positive marker
                # that ended the polling loop. A panic after boot is a failure.
                evidence = serial.read_text(errors='replace')
                failures = normal_boot_serial_errors(evidence, args.system_apps, app_count)
                if failures:
                    raise RuntimeError('; '.join(failures))
                # With -no-reboot a guest crash also exits QEMU with status 0,
                # so the guest must still be running when the harness quits.
                if process.poll() is not None:
                    raise RuntimeError('QEMU exited before the harness requested quit')
                try:
                    execute('quit')
                except (ConnectionResetError, BrokenPipeError, json.JSONDecodeError):
                    # QEMU can tear down the QMP socket before its 'quit'
                    # reply is read (observed: reset with QEMU still alive,
                    # then a clean exit). Completion is proven below by the
                    # exit status and a final serial check, not by the reply.
                    pass
            exit_status = process.wait(timeout=5)
            if exit_status != 0:
                raise RuntimeError(f'QEMU exited with status {exit_status} after quit')
            # The guest ran until QEMU stopped; re-check everything it logged.
            failures = normal_boot_serial_errors(serial.read_text(errors='replace'), args.system_apps, app_count)
            if failures:
                raise RuntimeError('; '.join(failures))
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
    if args.hardware_demo:
        print(f'Hardware demo: sign-in + {app_count} applications + desktop + PS/2 IOAPIC + smptest: PASS')
    else:
        print('Normal release shell + PS/2 IOAPIC + smptest: PASS')
    print('Serial log:', serial)
    print('UEFI image:', image)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
