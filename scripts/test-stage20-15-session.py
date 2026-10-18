"""Stage 20.15: the WovenShell desktop as the session.

Boots the stage20-15-test kernel on q35. The kernel starts the session as a
system-apps boot does (session::start), with an application in the
catalog, and drives it with real i8042 input: the launcher lists the
application; F12 shows the diagnostic console (the session's frames stop,
the terminal owns the display, a typed key reaches the kernel shell) and
returns to the desktop; then Quick Settings' Power off turns the machine
off through syscall 209.

QEMU (without -no-shutdown) must exit by itself with status 0 after the
kernel's "clicking Power off" line, with every earlier step passed and no
FAIL line anywhere.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

CLICK = '[S20.15] clicking Power off'


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
    out = root / 'audit-artifacts' / f'stage20.15-{args.cpus}cpu-{time.time_ns()}'
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
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage20-15-test', '--', '--print-image'],
                           cwd=root, env=env, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        try:
            process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            errors.append('timeout: the machine did not turn off')
            process.kill()
            process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    steps = [line for line in log.splitlines() if line.startswith('[S20.15]')]
    if process.returncode != 0:
        errors.append(f'QEMU exit status {process.returncode}, expected 0 (turned off by the guest)')
    if CLICK not in log:
        errors.append(f'missing {CLICK!r}')
    # The desktop marker must not mask a kernel panic or an unrelated security failure.
    # Check the entire serial log, not just Stage 20.15 lines.
    if any(marker in log.lower() for marker in ('kernel panic', 'panicked at', 'security violation', 'security: failed')):
        errors.append('kernel panic or security failure found in serial evidence')
    if any('FAIL' in line for line in steps):
        errors.append('a step failed')
    passed = [line for line in steps if line.endswith(' PASS')]
    if len(passed) < 7:
        errors.append(f'{len(passed)} steps passed before the power-off, expected 7')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S20.15' in line or 'SESSION' in line or 'POWER' in line),
              file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print('\n'.join(steps))
    print(f'Stage 20.15: PASS ({args.cpus} CPU; turned off from the desktop). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
