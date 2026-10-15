"""Stage 13.19: the boot's log on the WovenHat USB stick.

The stick is built the way scripts/write-live-usb.ps1 leaves a real one: a
64 MiB GPT disk with an EFI system partition, then a FAT32 basic-data
partition labelled WOVENLOG. It is attached as usb-storage on QEMU's xHCI
controller. Modes:

  test        the stage13-19-test kernel starts the log, adds a tester's
              note, a reported failure and a 200-line burst, and flushes
  production  a normal kernel boots to its shell; the harness waits one
              flush period longer, then stops it
  unlabelled  as production, but the FAT32 partition is labelled NO NAME:
              nothing on the stick may change

The harness then reads LOGS/BOOT0001.TXT back from the image with its own
FAT32 reader and checks its summary and contents, and requires every byte
before the log partition (the GPT and the EFI partition) to be unchanged.
"""
import argparse
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time
import uuid
import zlib

PASS_MARKER = '[S13.19] live-test log on the USB stick: PASSED'
SHELL_MARKER = '[BOOT] shell-first runtime ready'
SECTOR = 512
DISK_SECTORS = 64 * 1024 * 1024 // SECTOR
ESP_START, ESP_SECTORS = 2048, 8192
LOG_START = ESP_START + ESP_SECTORS
LOG_SECTORS = DISK_SECTORS - 34 - LOG_START
CAPACITY = 512 * 1024
EFI_SYSTEM = bytes([0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b])
BASIC_DATA = bytes([0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26, 0x99, 0xc7])


def fat32_volume(sectors, label):
    """A blank FAT32 volume of `sectors` 512-byte sectors, one per cluster."""
    reserved = 32
    clusters_guess = sectors
    fat_sectors = (clusters_guess * 4 + SECTOR - 1) // SECTOR
    first_data = reserved + 2 * fat_sectors
    clusters = sectors - first_data
    assert clusters >= 65525
    boot = bytearray(SECTOR)
    boot[:3] = b'\xeb\x58\x90'
    boot[3:11] = b'MSWIN4.1'
    struct.pack_into('<H', boot, 11, SECTOR)
    boot[13] = 1
    struct.pack_into('<H', boot, 14, reserved)
    boot[16] = 2
    boot[21] = 0xF8
    struct.pack_into('<I', boot, 28, LOG_START)
    struct.pack_into('<I', boot, 32, sectors)
    struct.pack_into('<I', boot, 36, fat_sectors)
    struct.pack_into('<I', boot, 44, 2)
    struct.pack_into('<H', boot, 48, 1)
    struct.pack_into('<H', boot, 50, 6)
    boot[64] = 0x80
    boot[66] = 0x29
    struct.pack_into('<I', boot, 67, 0x13192026)
    boot[71:82] = label
    boot[82:90] = b'FAT32   '
    boot[510:512] = b'\x55\xaa'
    info = bytearray(SECTOR)
    struct.pack_into('<I', info, 0, 0x41615252)
    struct.pack_into('<I', info, 484, 0x61417272)
    struct.pack_into('<I', info, 488, clusters - 1)
    struct.pack_into('<I', info, 492, 3)
    struct.pack_into('<I', info, 508, 0xAA550000)
    fat = bytearray(SECTOR)
    struct.pack_into('<III', fat, 0, 0x0FFFFFF8, 0xFFFFFFFF, 0x0FFFFFFF)
    return [(0, boot), (1, info), (6, boot), (7, info), (reserved, fat), (reserved + fat_sectors, fat)]


def gpt_disk(path, label):
    disk = bytearray(DISK_SECTORS * SECTOR)
    # Protective MBR.
    mbr = bytearray(SECTOR)
    mbr[446 + 4] = 0xEE
    struct.pack_into('<II', mbr, 446 + 8, 1, DISK_SECTORS - 1)
    mbr[510:512] = b'\x55\xaa'
    disk[0:SECTOR] = mbr
    entries = bytearray(128 * 128)
    for index, (kind, first, count, name) in enumerate(((EFI_SYSTEM, ESP_START, ESP_SECTORS, 'EFI'),
                                                         (BASIC_DATA, LOG_START, LOG_SECTORS, 'WOVENLOG'))):
        entry = bytearray(128)
        entry[0:16] = kind
        entry[16:32] = uuid.uuid4().bytes_le
        struct.pack_into('<QQ', entry, 32, first, first + count - 1)
        entry[56:56 + 2 * len(name)] = name.encode('utf-16-le')
        entries[index * 128:(index + 1) * 128] = entry
    for header_lba, entries_lba, backup in ((1, 2, DISK_SECTORS - 1), (DISK_SECTORS - 1, DISK_SECTORS - 33, 1)):
        header = bytearray(92)
        header[0:8] = b'EFI PART'
        struct.pack_into('<IIIIQQQQ', header, 8, 0x00010000, 92, 0, 0, header_lba, backup, 34, DISK_SECTORS - 34)
        header[56:72] = uuid.UUID('5ad1e5f0-1319-4c0f-9e55-0123456789ab').bytes_le
        struct.pack_into('<QIII', header, 72, entries_lba, 128, 128, zlib.crc32(entries))
        struct.pack_into('<I', header, 16, zlib.crc32(header))
        disk[header_lba * SECTOR:header_lba * SECTOR + 92] = header
        disk[entries_lba * SECTOR:entries_lba * SECTOR + len(entries)] = entries
    # The EFI partition: recognisable bytes that must survive untouched.
    for lba in range(ESP_START, ESP_START + ESP_SECTORS):
        disk[lba * SECTOR:lba * SECTOR + 16] = b'EFI-PART-%07d' % lba
    for lba, block in fat32_volume(LOG_SECTORS, label):
        start = (LOG_START + lba) * SECTOR
        disk[start:start + SECTOR] = block
    path.write_bytes(disk)


class Fat32:
    """Just enough FAT32 to read a file back from the log partition."""

    def __init__(self, image, start):
        self.image, self.start = image, start
        boot = image[start * SECTOR:(start + 1) * SECTOR]
        self.per_cluster = boot[13]
        reserved, fats, fat_size = struct.unpack_from('<H', boot, 14)[0], boot[16], struct.unpack_from('<I', boot, 36)[0]
        self.fat = start + reserved
        self.data = start + reserved + fats * fat_size
        self.root = struct.unpack_from('<I', boot, 44)[0]

    def chain(self, cluster):
        while 2 <= cluster < 0x0FFFFFF8:
            yield cluster
            offset = self.fat * SECTOR + cluster * 4
            cluster = struct.unpack_from('<I', self.image, offset)[0] & 0x0FFFFFFF

    def read(self, cluster, size=None):
        out = bytearray()
        for c in self.chain(cluster):
            lba = self.data + (c - 2) * self.per_cluster
            out += self.image[lba * SECTOR:(lba + self.per_cluster) * SECTOR]
            if size is not None and len(out) >= size:
                break
        return bytes(out if size is None else out[:size])

    def find(self, directory, name):
        data = self.read(directory)
        for offset in range(0, len(data), 32):
            entry = data[offset:offset + 32]
            if entry[0] == 0:
                break
            if entry[0] != 0xE5 and entry[11] != 0x0F and entry[:11] == name:
                cluster = struct.unpack_from('<H', entry, 20)[0] << 16 | struct.unpack_from('<H', entry, 26)[0]
                return cluster, struct.unpack_from('<I', entry, 28)[0]
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mode', choices=('test', 'production', 'unlabelled'), default='test')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars or qemu.parent / 'share' / 'edk2-i386-vars.fd'
    if not qemu.is_file() or not firmware.is_file() or not firmware_vars.is_file():
        parser.error('QEMU, firmware and firmware variable files must exist')
    out = root / 'audit-artifacts' / f'stage13.19-{args.mode}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True)
    production = args.mode != 'test'
    build_command = ['cargo', 'run', '--quiet'] + ([] if production else ['--features', 'stage13-19-test'])
    build = subprocess.run(build_command + ['--', '--print-image'], cwd=root, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    stick = out / 'stick.img'
    gpt_disk(stick, b'NO NAME    ' if args.mode == 'unlabelled' else b'WOVENLOG   ')
    original = stick.read_bytes()
    # The firmware gets its own variable store, so it never writes NvVars to
    # a FAT volume on the stick.
    vars_copy = out / 'OVMF_VARS.fd'
    shutil.copyfile(firmware_vars, vars_copy)
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
               '-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}',
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
               '-device', 'qemu-xhci,id=xhci',
               '-drive', f'if=none,id=stick,format=raw,file={stick}',
               '-device', 'usb-storage,bus=xhci.0,drive=stick,id=stick0']
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    errors = []
    with (out / 'qemu.log').open('w') as output:
        process = subprocess.Popen(command, cwd=root, stdout=output, stderr=output, creationflags=flags)
        started = time.time()
        try:
            while process.poll() is None:
                log = serial.read_text(errors='replace') if serial.exists() else ''
                if production and SHELL_MARKER in log:
                    # One flush period (3 s) and a margin.
                    time.sleep(8)
                    break
                if time.time() - started > args.timeout:
                    errors.append('timeout')
                    break
                time.sleep(0.5)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    image = stick.read_bytes()
    if image[:LOG_START * SECTOR] != original[:LOG_START * SECTOR]:
        errors.append('the GPT or the EFI partition changed')
    if args.mode == 'unlabelled':
        if image != original:
            errors.append('the stick changed although its partition is not labelled WOVENLOG')
        if '[LIVELOG] no live-test stick: NoLogPartition' not in log:
            errors.append('the boot did not report that there is no log partition')
        if SHELL_MARKER not in log:
            errors.append('production boot did not reach the shell')
    else:
        if not production:
            if process.returncode != 33:
                errors.append(f'exit code {process.returncode}, expected 33')
            if PASS_MARKER not in log:
                errors.append(f'missing {PASS_MARKER!r}')
        elif SHELL_MARKER not in log:
            errors.append('production boot did not reach the shell')
        volume = Fat32(image, LOG_START)
        logs = volume.find(volume.root, b'LOGS       ')
        found = logs and volume.find(logs[0], b'BOOT0001TXT')
        if not found:
            errors.append('no LOGS/BOOT0001.TXT on the stick')
        else:
            cluster, size = found
            text = volume.read(cluster, size)
            (out / 'BOOT0001.TXT').write_bytes(text)
            if size != CAPACITY:
                errors.append(f'the log file is {size} bytes, expected {CAPACITY}')
            wanted = [b'WovenHat OS live-test log, boot 0001', b'---- kernel log follows ----',
                      b'[INVENTORY] cpu', b'[INVENTORY] pci', b'[INVENTORY] usb disk', b'[DRIVERS] xhci0']
            if production:
                wanted.append(SHELL_MARKER.encode())
            else:
                wanted += [b'[NOTE] S13.19 tester note: the screen flickers on resume',
                           b'[S13.19] burst line 199 of the multi-sector log']
            for item in wanted:
                if item not in text:
                    errors.append(f'the log lacks {item!r}')
            summary = text[:4096]
            if not production and b'  [S13.19] simulated driver FAILURE for the summary' not in summary:
                errors.append('the summary does not list the reported failure')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print('\n'.join(line for line in log.splitlines() if 'S13.19' in line or 'LIVELOG' in line or '[USB]' in line),
              file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage 13.19 {args.mode}: PASS ({args.cpus} CPU). Evidence: {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
