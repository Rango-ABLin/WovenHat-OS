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

def normal_boot_serial_errors(log: str, system_apps: bool) -> list[str]:
    """Reject fatal evidence even when all positive boot markers appeared."""
    errors = []
    normalized = log.lower()
    for marker in ('kernel panic', 'panicked at', 'security violation', 'security: failed'):
        if marker in normalized:
            errors.append(f'Guest serial contains fatal evidence: {marker}')
    expected = ('[BOOT] desktop-first runtime ready' if system_apps
                else '[BOOT] shell-first runtime ready')
    if expected not in log:
        errors.append(f'Missing normal boot marker: {expected}')
    if system_apps:
        for marker in ('[APPS] 10 signed applications installed',
                       '[SESSION] WovenShell desktop started',
                       '[SESSION] diagnostic console'):
            if marker not in log:
                errors.append(f'Missing system-apps marker: {marker}')
    if '[SMP] acknowledged TLB shootdowns: PASSED' not in log:
        errors.append('Missing keyboard-driven SMP shell acceptance marker')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path, help='matching writable OVMF VARS template')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=4)
    parser.add_argument('--system-apps', action='store_true',
                        help='build the production system-app image and verify normal-boot installation')
    args = parser.parse_args()
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
        build_env.setdefault('WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX', test_public_key)
        build_env['WOVENHAT_SHELL_IMAGE'] = str(build_woven_shell(root))
        feature_arguments = ['--features', 'system-apps']
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
            if args.system_apps and '[APPS] 10 signed applications installed' not in serial.read_text(errors='replace'):
                raise RuntimeError('Production system-app catalog was not installed; inspect shell-regression logs')
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
                    time.sleep(2)
                    execute('send-key', {'keys': [{'type': 'qcode', 'data': 'f12'}], 'hold-time': 40})
                    deadline = time.monotonic() + 30
                    while '[SESSION] diagnostic console' not in serial.read_text(errors='replace'):
                        if process.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError('F12 did not open the diagnostic console; inspect shell-regression logs')
                        time.sleep(0.1)
                for key in list('smptest') + ['ret']:
                    execute('send-key', {'keys': [{'type': 'qcode', 'data': key}], 'hold-time': 40})
                    time.sleep(0.1)
                deadline = time.monotonic() + 60
                while '[SMP] acknowledged TLB shootdowns: PASSED' not in serial.read_text(errors='replace'):
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError('Keyboard/SMP shell command failed; inspect shell-regression logs')
                    time.sleep(0.1)
                # Check the complete serial trace, not just the positive marker
                # that ended the polling loop. A panic after boot is a failure.
                evidence = serial.read_text(errors='replace')
                failures = normal_boot_serial_errors(evidence, args.system_apps)
                if failures:
                    raise RuntimeError('; '.join(failures))
                execute('quit')
            process.wait(timeout=5)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
    print('Normal release shell + PS/2 IOAPIC + smptest: PASS')
    print('Serial log:', serial)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
