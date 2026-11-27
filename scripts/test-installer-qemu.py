"""Install to disposable file-backed disks, verify GPT/content, boot without USB.

Only this harness's QEMU processes are stopped. Each run keeps its own evidence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import struct
import subprocess
import time
import zlib


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


class VM:
    def __init__(self, args, folder, drives, cpus):
        self.folder = folder
        folder.mkdir()
        self.serial = folder / 'serial.log'
        self.log = self.serial
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        command = [str(args.qemu), '-accel', 'tcg', '-machine', 'pc', '-m', '512M',
                   '-smp', str(cpus), '-display', 'none', '-no-reboot',
                   '-serial', f'file:{self.serial}',
                   '-qmp', f'tcp:127.0.0.1:{port},server=on,wait=off',
                   '-drive', f'if=pflash,format=raw,readonly=on,file={args.firmware}']
        if args.firmware_vars:
            variables = folder / 'OVMF_VARS.fd'
            shutil.copyfile(args.firmware_vars, variables)
            command += ['-drive', f'if=pflash,format=raw,file={variables}']
        for i, drive in enumerate(drives):
            path, bootindex = drive[:2]
            readonly = len(drive) > 2 and drive[2]
            blocksize = drive[3] if len(drive) > 3 else 512
            command += ['-drive', f'if=none,id=d{i},format=raw,file={path},readonly={"on" if readonly else "off"}',
                        '-device', f'virtio-blk-pci,drive=d{i},bootindex={bootindex},logical_block_size={blocksize},physical_block_size={blocksize}']
        (folder / 'command.json').write_text(json.dumps(command, indent=2))
        self.errors = (folder / 'qemu.log').open('w')
        flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
        self.process = subprocess.Popen(command, stdout=self.errors, stderr=self.errors, creationflags=flags)
        self.socket = None
        try:
            deadline = time.monotonic() + 15
            while self.socket is None:
                try:
                    self.socket = socket.create_connection(('127.0.0.1', port), timeout=2)
                except OSError:
                    if time.monotonic() > deadline or self.process.poll() is not None:
                        raise RuntimeError(f'QEMU startup failed: {folder}')
                    time.sleep(.1)
            self.socket.settimeout(10)
            self.stream = self.socket.makefile('rwb')
            json.loads(self.stream.readline())
            self.execute('qmp_capabilities')
        except BaseException:
            self.close()
            raise

    def execute(self, name, arguments=None):
        request = {'execute': name}
        if arguments is not None:
            request['arguments'] = arguments
        self.stream.write((json.dumps(request) + '\n').encode())
        self.stream.flush()
        while True:
            response = json.loads(self.stream.readline())
            if 'error' in response:
                raise RuntimeError(response['error'])
            if 'return' in response:
                return response['return']

    def wait(self, marker, serial=False):
        path = self.serial if serial else self.log
        deadline = time.monotonic() + 180
        while True:
            data = path.read_text(errors='replace') if path.exists() else ''
            if marker in data:
                return data
            if self.process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError(f'Missing {marker!r}; inspect {self.folder}')
            if not serial and '[INSTALL] STOPPED' in data:
                raise RuntimeError(f'Installer stopped; inspect {self.folder}')
            time.sleep(.1)

    def type(self, text):
        for char in text:
            code = {' ': 'spc', '\n': 'ret'}.get(char, char.lower())
            keys = ([{'type': 'qcode', 'data': 'shift'}] if char.isupper() else [])
            keys += [{'type': 'qcode', 'data': code}]
            self.execute('send-key', {'keys': keys, 'hold-time': 50})
            time.sleep(.12)

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        if getattr(self, 'stream', None):
            self.stream.close()
        if self.socket:
            self.socket.close()
        self.errors.close()


def check_install(target, payload):
    original = payload.read_bytes()
    with target.open('rb') as disk:
        prefix = disk.read(len(original) - 33 * 512)
        # Only MBR length, primary GPT addresses and CRC may change.
        expected = bytearray(original[:len(prefix)])
        for start, end in ((458, 462), (512 + 16, 512 + 20),
                           (512 + 32, 512 + 40), (512 + 48, 512 + 56)):
            expected[start:end] = prefix[start:end]
        assert prefix == expected, 'Installed kernel/ESP differs from payload'
        table = original[1024:1024 + 16384]
        last = target.stat().st_size // 512 - 1
        for lba, alternate, table_lba in ((1, last, 2), (last, 1, last - 32)):
            disk.seek(lba * 512)
            h = bytearray(disk.read(512))
            assert h[:8] == b'EFI PART'
            crc = struct.unpack_from('<I', h, 16)[0]
            h[16:20] = bytes(4)
            assert zlib.crc32(h[:92]) == crc, 'Bad header CRC'
            assert struct.unpack_from('<QQ', h, 24) == (lba, alternate)
            assert struct.unpack_from('<Q', h, 48)[0] == last - 33
            assert struct.unpack_from('<Q', h, 72)[0] == table_lba
            disk.seek(table_lba * 512)
            assert disk.read(16384) == table
            assert zlib.crc32(table) == struct.unpack_from('<I', h, 88)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--qemu', type=Path, default=Path(r'C:\Program Files\qemu\qemu-system-x86_64.exe'))
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path)
    parser.add_argument('--cpus', type=int, nargs='+', choices=(1, 2, 4), default=[1, 2, 4])
    args = parser.parse_args()
    args.firmware = args.firmware or args.qemu.parent / 'share/edk2-x86_64-code.fd'
    args.firmware_vars = args.firmware_vars or args.qemu.parent / 'share/edk2-i386-vars.fd'
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = out / 'installer-usb.img'
    shutil.copyfile(args.bundle.resolve() / 'wovenhat-installer-usb.img', source)
    payload = args.bundle.resolve() / 'wovenhat-system.img'
    source_hash = digest(source)
    # No usable target: read-only, too small, and unsupported logical sectors.
    invalid = []
    for name, size, readonly, blocksize in [('readonly', 192, True, 512),
                                           ('tiny', 4, False, 512),
                                           ('4kn', 256, False, 4096)]:
        path = out / f'{name}.img'
        with path.open('xb') as stream:
            stream.truncate(size * 1024 * 1024)
        invalid.append((path, len(invalid) + 2, readonly, blocksize))
    hashes = {drive[0]: digest(drive[0]) for drive in invalid}
    vm = VM(args, out / 'no-eligible-target', [(source, 1)] + invalid, 1)
    try:
        vm.wait('No eligible target disks. Nothing written.')
    finally:
        vm.close()
    assert digest(source) == source_hash
    assert all(digest(path) == value for path, value in hashes.items())
    print('PASS: no eligible target; read-only, undersized and 4Kn targets excluded without writes', flush=True)
    for cpus in args.cpus:
        target = out / f'target-{cpus}.img'
        decoy = out / f'decoy-{cpus}.img'
        for path, size in ((target, 256), (decoy, 192)):
            with path.open('xb') as stream:
                stream.truncate(size * 1024 * 1024)
                stream.write(b'PRESERVE-UNLESS-SELECTED')
        target_hash, decoy_hash = digest(target), digest(decoy)
        drives = [(source, 1), (decoy, 3), (target, 2)]
        # Wrong confirmation must leave every disk unchanged.
        vm = VM(args, out / f'cancel-{cpus}', drives, cpus)
        try:
            listing = vm.wait('[INSTALL] SELECT DISK')
            matches = re.findall(r'Disk (\d+): 256 MiB', listing)
            assert len(matches) == 1
            assert len(re.findall(r'Disk \d+:', listing)) == 2, 'Installer source exposed as target'
            number = matches[0]
            vm.type(number + '\n')
            vm.wait('to install:')
            vm.type('NO\n')
            vm.wait('Cancelled. Nothing written.')
        finally:
            vm.close()
        assert digest(target) == target_hash
        assert digest(decoy) == decoy_hash
        assert digest(source) == source_hash
        vm = VM(args, out / f'install-{cpus}', drives, cpus)
        try:
            listing = vm.wait('[INSTALL] SELECT DISK')
            number = re.findall(r'Disk (\d+): 256 MiB', listing)[0]
            vm.type(number + '\n')
            vm.wait('to install:')
            vm.type(f'ERASE {number}\n')
            vm.wait('[INSTALL] VERIFIED')
        finally:
            vm.close()
        assert digest(source) == source_hash, 'Installer source changed'
        assert digest(decoy) == decoy_hash, 'Unselected disk changed'
        check_install(target, payload)
        # A new VM and fresh firmware state, with no installer attached.
        vm = VM(args, out / f'boot-installed-{cpus}', [(target, 1)], cpus)
        try:
            vm.wait('[BOOT] shell-first runtime ready', serial=True)
            vm.type('smptest\n')
            vm.wait('[SMP] acknowledged TLB shootdowns: PASSED', serial=True)
        finally:
            vm.close()
        print(f'PASS: {cpus} CPUs: cancel, source exclusion, selected-disk install, GPT/readback, standalone boot and keyboard/SMP regression', flush=True)
    (out / 'result.txt').write_text('PASS: installer acceptance on ' + ','.join(map(str, args.cpus)) + ' CPUs\n')


if __name__ == '__main__':
    main()
