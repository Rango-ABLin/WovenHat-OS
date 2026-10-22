"""Show the WovenGraphics 2D/3D compositor frame (Stage 16/19.2 graphics-track
work: gradients, rounded alpha panel, two depth-tested 3D triangles, an
antialiased path badge, text, a transformed image) in a real QEMU window.

This builds the same `stage19-2-test` self-test image the headless harness
(scripts/test-hello-gui-compositor-qemu.py) uses, where hello-gui draws its
frame, presents it through the compositor to the framebuffer, and the kernel
self-test then calls qemu_test_exit_success() -- which writes to the QEMU
isa-debug-exit port and halts. The headless harness attaches that device, so
the write ends the VM a fraction of a second after the frame is drawn, before
a human could look at it. This script deliberately does NOT attach
isa-debug-exit: the write becomes a no-op to an unassigned I/O port, so the
kernel instead parks forever in its `hlt` loop with the last presented frame
still in the real framebuffer, visible in the QEMU window until you close it.

Not an acceptance harness; produces no stage evidence.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--windowed', action='store_true',
                         help='keep the default-sized window instead of full-screen zoom')
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing files.')

    app_dir = root / 'apps' / 'hello-gui'
    app_env = os.environ.copy()
    app_env.pop('CARGO_BUILD_BUILD_DIR', None)
    app_env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=app_env, check=True)
    hello_gui_image = app_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-hello-gui'

    build_env = os.environ.copy()
    build_env['WOVENHAT_HELLO_GUI_IMAGE'] = str(hello_gui_image)
    build_env.setdefault('CARGO_BUILD_BUILD_DIR', str(root / '.cargo-build'))
    build_env.setdefault('CARGO_TARGET_DIR', str(root / 'target'))
    image = subprocess.check_output(
        ['cargo', 'run', '--quiet', '--features', 'stage19-2-test', '--', '--print-image'],
        cwd=root, env=build_env, text=True).strip()

    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', '1',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-serial', 'mon:stdio', '-no-reboot',
        # Deliberately no isa-debug-exit device: see module docstring.
    ]
    if not args.windowed:
        # hello-gui only draws a 200x120 window in the corner of the
        # 1280x800 framebuffer; zoom-to-fit (GTK's default) stretches that
        # whole frame to fill the screen in full-screen mode, which is the
        # only way QEMU's display backends offer to make it legible without
        # touching the tested drawing coordinates. Alt+Enter (or the window's
        # View menu) toggles back to windowed.
        command.append('-full-screen')
    print('Image: ' + image)
    print('The kernel halts (not shuts down) once the frame is drawn; close the window when done.')
    if not args.windowed:
        print('Opening full-screen so the small drawn window is legible (Alt+Enter toggles windowed).')
    print(' '.join(command))
    subprocess.run(command, cwd=root)


if __name__ == '__main__':
    main()
