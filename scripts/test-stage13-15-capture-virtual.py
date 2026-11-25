#!/usr/bin/env python3
"""Stage 13.15 capture ABI with a bounded virtual input source.

Builds the real hello-gui capture client and the production HDA driver with
``stage13-15-capture-virtual-test``. The driver fills its owned DMA page with a
deterministic PCM pattern instead of starting a hardware stream, so the gate
verifies the 4096-byte copy-out, the Microphone capability check and the denial
path without a physical input device. It does not claim device capture.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

REQUIRED = (
    '[S13.15] capture client exit=0',
    '[S13.15] capture denial exit=3',
    '[S13.15] WovenAudio playback: PASSED',
)


def app_build_env(app_dir: Path) -> dict[str, str]:
    env = dict(os.environ)
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    return env


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4, 8, 16), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('QEMU and firmware files must exist')
    out = root / 'audit-artifacts' / f'stage13.15-capture-virtual-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    gui_dir = root / 'apps' / 'hello-gui'
    if subprocess.run(['cargo', 'build', '--release'], cwd=gui_dir, env=app_build_env(gui_dir)).returncode:
        return 1
    env = dict(os.environ)
    env['WOVENHAT_HELLO_GUI_IMAGE'] = str(gui_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-hello-gui')
    build = subprocess.run(
        ['cargo', 'run', '--quiet', '--features', 'stage13-15-capture-virtual-test', '--', '--print-image'],
        cwd=root, env=env, text=True, capture_output=True,
    )
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
        '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-audiodev', f'wav,id=snd,path={out / "audio.wav"},out.frequency=48000,out.channels=2,out.format=s16',
        '-device', 'intel-hda,id=hda', '-device', 'hda-duplex,bus=hda.0,audiodev=snd',
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        try:
            result = subprocess.run(command, cwd=root, stdout=output, stderr=output, timeout=args.timeout,
                                    creationflags=flags)
            if result.returncode != 33:
                errors.append(f'exit code {result.returncode}, expected 33')
        except subprocess.TimeoutExpired:
            errors.append('timeout')
    log = serial.read_text(errors='replace') if serial.exists() else ''
    for marker in REQUIRED:
        if marker not in log:
            errors.append(f'missing {marker!r}')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.15' in line), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Virtual capture gate: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
