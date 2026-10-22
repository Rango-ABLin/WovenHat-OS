"""Boot the full WovenHat OS (desktop, sign-in, apps) in an interactive QEMU
window, for hands-on exploration. Unlike the Stage acceptance harnesses, this
is not a headless QMP-driven test: QEMU opens a normal display window with
keyboard/mouse, and the serial console is interleaved with the QEMU monitor
on this terminal's stdio. Close the QEMU window (or quit the monitor) to end
the session. This script produces no acceptance evidence.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def build_woven_shell(root):
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
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=2)
    parser.add_argument('--mem', default='512M')
    parser.add_argument('--mode', choices=('hardware-demo', 'system-apps', 'plain'), default='hardware-demo',
                         help='hardware-demo (default): full desktop + demo sign-in (ada/1234, lin/9876) + 10 apps; '
                              'system-apps: production app catalog, no demo accounts; plain: base kernel only')
    parser.add_argument('--disk', type=Path, help='persistent FAT32 data disk (default: wovenhat-disk.img at repo root)')
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing files.')

    build_env = os.environ.copy()
    feature_arguments = []
    if args.mode in ('hardware-demo', 'system-apps'):
        test_public_key = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'
        package_env = os.environ.copy()
        package_env.setdefault('WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX', test_public_key)
        subprocess.run(
            [sys.executable, str(root / 'scripts' / 'package-system-apps.py'), '--version', '0.8.0'],
            cwd=root, env=package_env, check=True,
        )
        output = root / 'target' / 'system-apps' / '0.8.0'
        manifest = json.loads((output / 'manifest.json').read_text(encoding='utf-8'))
        packages = {item['id']: output / item['package'] for item in manifest['apps']}
        # The production kernel embeds the signed WovenPackage service image
        # separately from the ten catalog applications. Build it here so the
        # interactive launcher is self-contained and does not depend on a
        # caller-prepared WOVENHAT_PACKAGE_IMAGE environment variable.
        package_service = subprocess.check_output(
            [sys.executable, str(root / 'scripts' / 'build-package-service.py')],
            cwd=root,
            env=package_env,
            text=True,
        ).strip()
        build_env['WOVENHAT_PACKAGE_IMAGE'] = package_service
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
        feature_arguments = ['--features', args.mode]

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

    disk = args.disk or (root / 'wovenhat-disk.img')
    subprocess.run([sys.executable, str(root / 'scripts' / 'create-fat32.py'), str(disk)], cwd=root, check=True)

    command = [
        str(qemu), '-machine', 'pc', '-m', args.mem, '-smp', str(args.cpus),
        '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-drive', f'if=ide,index=0,format=raw,file={disk}',
        '-netdev', 'user,id=net0',
        '-device', 'virtio-net-pci,netdev=net0,disable-modern=on',
        '-serial', 'mon:stdio', '-no-reboot',
    ]
    print('Image: ' + image)
    print('Data disk: ' + str(disk))
    print('Launching interactive QEMU (close the window, or "quit" at the monitor, to end)...')
    print(' '.join(command))
    subprocess.run(command, cwd=root)


if __name__ == '__main__':
    main()
