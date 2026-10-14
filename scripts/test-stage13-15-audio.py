"""Stage 13.15: WovenAudio playback from Ring 3.

Builds apps/hello-gui (its --audio mode plays a quarter second of a 500 Hz
square wave: 96-frame periods, 48 kHz 16-bit stereo), embeds it in a
stage13-15-test kernel and boots q35 with an Intel HDA controller and codec
whose output QEMU records to a WAV file. The kernel checks playback ends
through the controller's completion interrupt and that a client without
AudioPlayback is refused. This harness then requires the recording to hold
the wave: enough loud samples, and sign changes every ~48 frames.
"""
import argparse
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time

PASS_MARKER = '[S13.15] WovenAudio playback: PASSED'
WAVE_FRAMES = 12_000
HALF_PERIOD = 48


def app_build_env(app_dir):
    env = dict(os.environ)
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    return env


def read_wav(path):
    """(channels, sample rate, bits, interleaved samples) of a PCM WAV."""
    data = path.read_bytes()
    if data[:4] != b'RIFF' or data[8:12] != b'WAVE':
        raise ValueError('not a WAV file')
    offset, fmt, samples = 12, None, b''
    while offset + 8 <= len(data):
        chunk, size = data[offset:offset + 4], struct.unpack_from('<I', data, offset + 4)[0]
        body = data[offset + 8:offset + 8 + size]
        if chunk == b'fmt ':
            fmt = struct.unpack_from('<HHIIHH', body)
        elif chunk == b'data':
            # QEMU leaves the sizes at 0 when the guest exits through
            # isa-debug-exit: the data then runs to the end of the file.
            samples = data[offset + 8:] if size == 0 or offset + 8 + size > len(data) else body
            break
        offset += 8 + size + (size & 1)
    if fmt is None:
        raise ValueError('no fmt chunk')
    _, channels, rate, _, _, bits = fmt
    count = len(samples) // 2
    return channels, rate, bits, struct.unpack(f'<{count}h', samples[:count * 2])


def wave_errors(path):
    channels, rate, bits, samples = read_wav(path)
    if (channels, rate, bits) != (2, 48_000, 16):
        return [f'recording is {channels} ch {rate} Hz {bits}-bit, expected 2 ch 48000 Hz 16-bit']
    left = samples[0::2]
    loud = [i for i, value in enumerate(left) if abs(value) >= 0x400]
    if len(loud) < WAVE_FRAMES * 3 // 4:
        return [f'only {len(loud)} loud frames recorded, expected about {WAVE_FRAMES}']
    first, last = loud[0], loud[-1]
    signal = left[first:last + 1]
    crossings = [i for i in range(1, len(signal))
                 if (signal[i] >= 0) != (signal[i - 1] >= 0) and abs(signal[i]) >= 0x400]
    if len(crossings) < 10:
        return [f'only {len(crossings)} sign changes in the recording']
    gaps = [b - a for a, b in zip(crossings, crossings[1:])]
    near = sum(1 for gap in gaps if abs(gap - HALF_PERIOD) <= 2)
    if near < len(gaps) * 9 // 10:
        return [f'sign changes are not every {HALF_PERIOD} frames: {sorted(set(gaps))[:8]}']
    return []


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
    out = root / 'audit-artifacts' / f'stage13.15-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    gui_dir = root / 'apps' / 'hello-gui'
    if subprocess.run(['cargo', 'build', '--release'], cwd=gui_dir, env=app_build_env(gui_dir)).returncode:
        return 1
    env = dict(os.environ)
    env['WOVENHAT_HELLO_GUI_IMAGE'] = str(gui_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-hello-gui')
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', 'stage13-15-test', '--', '--print-image'],
                           cwd=root, env=env, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    wav = out / 'audio.wav'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-audiodev', f'wav,id=snd,path={wav},out.frequency=48000,out.channels=2,out.format=s16',
               '-device', 'intel-hda,id=hda', '-device', 'hda-duplex,bus=hda.0,audiodev=snd']
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
    if PASS_MARKER not in log:
        errors.append(f'missing {PASS_MARKER!r}')
    if wav.exists():
        try:
            errors += wave_errors(wav)
        except ValueError as error:
            errors.append(f'recording: {error}')
    else:
        errors.append('QEMU wrote no recording')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.15' in line or 'DRIVERS' in line), file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.15: PASS ({args.cpus} CPU, exit 33, waveform recorded). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
