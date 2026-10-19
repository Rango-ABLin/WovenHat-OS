"""Boot the Stage 19.2 hello-gui compositor self-test headlessly, wait for its
PASSED marker, and save a PNG screenshot of the framebuffer via QMP
screendump -- a fast, scaling-independent way to inspect the WovenGraphics
2D/3D compositor frame without an interactive window.

hello-gui only draws a 200x120 window in the corner of the 1280x800
framebuffer (see apps/hello-gui/src/main.rs); pass --crop to also save a
cropped, nearest-neighbor-upscaled PNG of just that region.

Not an acceptance harness; produces no stage evidence.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import time
import zlib

PASSED_MARKER = '[S19.2] hello-gui through the compositor: PASSED'


def build_image(root, qemu_path, firmware):
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
    return subprocess.check_output(
        ['cargo', 'run', '--quiet', '--features', 'stage19-2-test', '--', '--print-image'],
        cwd=root, env=build_env, text=True).strip()


def qmp_connect(port, deadline):
    sock = None
    while time.monotonic() < deadline:
        try:
            sock = socket.create_connection(('127.0.0.1', port), timeout=2)
            break
        except OSError:
            time.sleep(0.2)
    if sock is None:
        raise RuntimeError('could not connect to QMP')
    stream = sock.makefile('rwb')
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
    return execute


def read_ppm(path):
    data = path.read_bytes()
    assert data[:2] == b'P6'
    idx = 2
    fields = []
    while len(fields) < 3:
        while data[idx] in b' \t\r\n':
            idx += 1
        start = idx
        while data[idx] not in b' \t\r\n':
            idx += 1
        fields.append(int(data[start:idx]))
    width, height, _maxval = fields
    idx += 1
    pixels = data[idx:idx + width * height * 3]
    return width, height, pixels


def write_png(path, width, height, pixels, region=None, scale=1):
    x0, y0, x1, y1 = region or (0, 0, width, height)
    cw, ch = x1 - x0, y1 - y0
    ow, oh = cw * scale, ch * scale

    def get_px(x, y):
        off = (y * width + x) * 3
        return pixels[off:off + 3]

    raw = bytearray()
    for oy in range(oh):
        raw.append(0)
        sy = y0 + oy // scale
        for ox in range(ow):
            sx = x0 + ox // scale
            raw.extend(get_px(sx, sy))

    def chunk(tag, payload):
        return struct.pack('>I', len(payload)) + tag + payload + struct.pack('>I', zlib.crc32(tag + payload) & 0xffffffff)

    ihdr = struct.pack('>IIBBBBB', ow, oh, 8, 2, 0, 0, 0)
    idat = zlib.compress(bytes(raw), 6)
    with path.open('wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(chunk(b'IHDR', ihdr))
        f.write(chunk(b'IDAT', idat))
        f.write(chunk(b'IEND', b''))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--qemu', default=__import__('shutil').which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--out', type=Path, help='PNG output path (default: target/hello-gui-screenshot.png)')
    parser.add_argument('--crop', action='store_true', help='also save a 4x-upscaled crop of just the drawn window')
    parser.add_argument('--timeout', type=float, default=60)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        parser.error('Set --qemu and --firmware to existing files.')

    image = build_image(root, qemu, firmware)
    out_dir = root / 'target' / 'hello-gui-screenshot'
    out_dir.mkdir(parents=True, exist_ok=True)
    serial = out_dir / 'serial.log'
    serial.write_text('')
    ppm = out_dir / 'screendump.ppm'

    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]

    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', '1',
        '-display', 'none', '-vga', 'std',
        '-serial', f'file:{serial}', '-no-reboot',
        '-qmp', f'tcp:127.0.0.1:{port},server=on,wait=off',
        '-drive', f'if=pflash,format=raw,readonly=on,file={firmware}',
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
    ]
    process = subprocess.Popen(command, cwd=root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + args.timeout
        execute = qmp_connect(port, deadline)
        while PASSED_MARKER not in serial.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() > deadline:
                print('Timed out or process exited before the PASSED marker.', file=sys.stderr)
                print(serial.read_text(errors='replace')[-2000:], file=sys.stderr)
                return 1
            time.sleep(0.2)
        time.sleep(0.5)
        execute('screendump', {'filename': str(ppm)})
    finally:
        if process.poll() is None:
            process.kill()

    width, height, pixels = read_ppm(ppm)
    out = args.out or (root / 'target' / 'hello-gui-screenshot.png')
    write_png(out, width, height, pixels)
    print(f'Screenshot: {out} ({width}x{height})')
    if args.crop:
        crop_out = out.with_name(out.stem + '-crop.png')
        write_png(crop_out, width, height, pixels, region=(55, 35, 265, 165), scale=4)
        print(f'Cropped 4x view of the drawn window: {crop_out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
