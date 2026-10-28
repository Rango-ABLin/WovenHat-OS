"""Build and execute WovenEditor through the signed Ring-3 ABI."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path, help='matching writable OVMF VARS template')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / 'apps' / 'woven-editor'
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars
    if not qemu.is_file() or not firmware.is_file() or (firmware_vars is not None and not firmware_vars.is_file()):
        parser.error('Set --qemu and --firmware to existing QEMU and OVMF files.')
    build_env = dict(os.environ)
    build_env.pop('CARGO_BUILD_BUILD_DIR', None)
    build_env['CARGO_TARGET_DIR'] = str(app / 'target')
    if subprocess.run(['cargo', 'build', '--release', '--target', 'x86_64-unknown-none'], cwd=app, env=build_env).returncode:
        return 1
    image = app / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-editor'
    package = image.with_suffix('.wapp')
    package_env = dict(os.environ)
    package_env.pop('CARGO_BUILD_BUILD_DIR', None)
    package_env['CARGO_TARGET_DIR'] = str(root / 'target' / 'wapp-packager')
    if subprocess.run([
        'cargo', 'run', '--quiet', '--manifest-path', str(root / 'tools' / 'wapp-packager' / 'Cargo.toml'),
        '--', str(image), str(package),
    ], cwd=root, env=package_env).returncode:
        return 1
    env = dict(os.environ)
    env['WOVENHAT_APP_IMAGE'] = str(package)
    env.setdefault('CARGO_BUILD_BUILD_DIR', str(root / '.cargo-build'))
    env.setdefault('CARGO_TARGET_DIR', str(root / 'target'))
    boot = subprocess.run(
        ['cargo', 'run', '--quiet', '--features', 'stage16-1-test', '--', '--print-image'],
        cwd=root, env=env, capture_output=True, text=True,
    )
    if boot.returncode:
        print(boot.stdout, boot.stderr)
        return boot.returncode
    out = root / 'audit-artifacts' / f'woven-editor-{args.cpus}cpu'
    out.mkdir(parents=True, exist_ok=True)
    serial = out / 'serial.log'
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(firmware_vars, vars_copy)
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
        '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
        *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'] if vars_copy is not None else []),
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={boot.stdout.strip()}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
    ]
    try:
        with (out / 'qemu.log').open('w') as log:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log, timeout=args.timeout).returncode
    except subprocess.TimeoutExpired:
        print(f'QEMU timeout; evidence: {out}')
        return 1
    text = serial.read_text(errors='replace') if serial.exists() else ''
    if code != 33 or '[S16.1] out-of-kernel application: PASSED' not in text:
        print(f'WovenEditor failed (exit {code}); evidence: {out}')
        return 1
    print(f'WovenEditor: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
