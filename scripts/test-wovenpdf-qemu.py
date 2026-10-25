"""Smoke-test the WovenPDF native executable under the generic Ring-3 QEMU gate.

This is the first gate that exercises an application the ordinary Rust
toolchain produced. Every userspace program before it was `global_asm!` inside
`kernel/src/userspace.rs`, wrapped in a synthetic ELF at runtime, so there was
no way to write a program without editing the kernel.

The application is a separate crate with its own linker script and link flags,
excluded from the kernel workspace. Its path is handed to the kernel build
through WOVENHAT_APP_IMAGE, the kernel installs it into the filesystem, reads
it back, and loads those filesystem bytes through the same ELF loader that
serves every other executable.
"""
import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

APP = 'wovenhat-pdf'
MARKER = '[S16.1] out-of-kernel application: PASSED'

def app_build_env(app_dir):
    """Build environment for an application crate.

    The kernel's Cargo directory overrides are dropped and the app's own
    target directory pinned, so the binary lands where this harness reads it.
    Inheriting CARGO_TARGET_DIR would build into the kernel's tree and leave a
    stale image to be embedded.
    """
    env = dict(os.environ)
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    return env


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64')
                        or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing QEMU and OVMF files.')

    app_dir = root / 'apps' / 'woven-pdf'
    print('[1/4] Building the application outside the kernel...')
    # A separate cargo invocation on purpose: this crate has its own workspace
    # root, target directory, linker script and code model, none of which the
    # kernel build shares.
    build = subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=app_build_env(app_dir))
    if build.returncode != 0:
        print('Application build failed.', file=sys.stderr)
        return 1
    image = app_dir / 'target' / 'x86_64-unknown-none' / 'release' / APP
    if not image.is_file():
        print(f'Application image missing: {image}', file=sys.stderr)
        return 1
    print(f'      image: {image} ({image.stat().st_size} bytes)')

    package = app_dir / 'target' / 'x86_64-unknown-none' / 'release' / f'{APP}.wapp'
    print('[2/5] Signing the application into a WAPP package...')
    package_env = dict(os.environ)
    package_env.pop('CARGO_BUILD_BUILD_DIR', None)
    package_env['CARGO_TARGET_DIR'] = str(root / 'target' / 'wapp-packager')
    packaged = subprocess.run([
        'cargo', 'run', '--quiet', '--manifest-path',
        str(root / 'tools' / 'wapp-packager' / 'Cargo.toml'), '--',
        str(image), str(package),
    ], cwd=root, env=package_env)
    if packaged.returncode != 0 or not package.is_file():
        print('Application packaging failed.', file=sys.stderr)
        return 1
    print(f'      package: {package} ({package.stat().st_size} bytes)')

    print('[3/5] Building the kernel with the signed package embedded...')
    env = dict(os.environ)
    env['WOVENHAT_APP_IMAGE'] = str(package)
    env.setdefault('CARGO_BUILD_BUILD_DIR', str(root / '.cargo-build'))
    env.setdefault('CARGO_TARGET_DIR', str(root / 'target'))
    boot = subprocess.run(
        ['cargo', 'run', '--quiet', '--features', 'stage16-1-test', '--', '--print-image'],
        cwd=root, env=env, capture_output=True, text=True)
    if boot.returncode != 0:
        print(boot.stdout, boot.stderr, file=sys.stderr)
        return 1
    boot_image = boot.stdout.strip()

    out = root / 'target' / f'wovenpdf-ring3-{args.cpus}cpu'
    out.mkdir(parents=True, exist_ok=True)
    serial = out / 'serial.log'
    serial.write_text('')
    vars_copy = None
    if args.firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(args.firmware_vars, vars_copy)

    print(f'[4/5] Booting on {args.cpus} CPU(s)...')
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35',
        '-m', '256M', '-smp', str(args.cpus), '-display', 'none',
        '-serial', f'file:{serial}', '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}']
          if vars_copy is not None else []),
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={boot_image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    with (out / 'qemu.log').open('w') as log:
        try:
            code = subprocess.run(command, cwd=root, stdout=log, stderr=log,
                                  timeout=args.timeout,
                                  creationflags=flags).returncode
        except subprocess.TimeoutExpired:
            print(f'QEMU timed out. Evidence: {out}', file=sys.stderr)
            return 1

    print('[5/5] Checking the acceptance markers...')
    text = serial.read_text(errors='replace') if serial.exists() else ''
    for line in text.splitlines():
        if '[S16.1]' in line or '[USER]' in line or 'hello from wovenhat' in line:
            print('  ' + line.strip())

    if code != 33 or MARKER not in text:
        print(f'FAILED (exit {code}). Evidence: {out}', file=sys.stderr)
        print(text[-3000:], file=sys.stderr)
        return 1
    print(f'WovenPDF generic Ring-3 gate: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    print('WARNING: This does not validate --gui, WovenShell IPC, PDF drawing or page navigation.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
