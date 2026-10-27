"""Build an x86-64 UEFI USB installer. Never accesses physical disks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import uuid
import zlib
import zipfile


def header(current, alternate, last_usable, guid, table_lba, table_crc):
    data = bytearray(512)
    struct.pack_into('<8sIIIIQQQQ16sQIII', data, 0, b'EFI PART', 0x10000,
                     92, 0, 0, current, alternate, 34, last_usable, guid,
                     table_lba, 128, 128, table_crc)
    struct.pack_into('<I', data, 16, zlib.crc32(data[:92]))
    return data


def make_usb(efi, destination):
    """GPT + FAT16 ESP with EFI/BOOT/BOOTX64.EFI fallback loader."""
    size = efi.stat().st_size
    clusters = max(16000, (size + 4095) // 4096 + 4096)
    if clusters > 65000:
        raise ValueError('Installer exceeds bounded FAT16 image capacity')
    fat_sectors = ((clusters + 2) * 2 + 511) // 512
    volume_sectors = 1 + 2 * fat_sectors + 32 + clusters * 8
    first = 2048
    total = first + volume_sectors + 33
    table = bytearray(16384)
    struct.pack_into('<16s16sQQQ', table, 0,
                     uuid.UUID('c12a7328-f81f-11d2-ba4b-00a0c93ec93b').bytes_le,
                     uuid.uuid4().bytes_le, first, first + volume_sectors - 1, 0)
    name = 'WovenInstaller'.encode('utf-16le')
    table[56:56 + len(name)] = name
    guid = uuid.uuid4().bytes_le
    crc = zlib.crc32(table)
    boot = bytearray(512)
    boot[:3] = b'\xeb\x3c\x90'
    boot[3:11] = b'WOVENHAT'
    struct.pack_into('<HBHBHHBHHHII', boot, 11, 512, 8, 1, 2, 512, 0,
                     0xf8, fat_sectors, 63, 255, first, volume_sectors)
    struct.pack_into('<BBBI11s8s', boot, 36, 0x80, 0, 0x29, 0x574f564e,
                     b'WOVENSETUP ', b'FAT16   ')
    boot[510:] = b'\x55\xaa'
    fat = bytearray(fat_sectors * 512)
    struct.pack_into('<HHHH', fat, 0, 0xfff8, 0xffff, 0xffff, 0xffff)
    file_clusters = (size + 4095) // 4096
    for i in range(file_clusters):
        struct.pack_into('<H', fat, (4 + i) * 2,
                         0xffff if i == file_clusters - 1 else 5 + i)

    def entry(name, attr, cluster, length=0):
        data = bytearray(32)
        data[:11] = name
        data[11] = attr
        struct.pack_into('<H', data, 26, cluster)
        struct.pack_into('<I', data, 28, length)
        return data

    with destination.open('xb') as disk:
        disk.truncate(total * 512)
        mbr = bytearray(512)
        mbr[450] = 0xee
        struct.pack_into('<II', mbr, 454, 1, min(total - 1, 0xffffffff))
        mbr[510:] = b'\x55\xaa'
        disk.write(mbr)
        disk.write(header(1, total - 1, total - 34, guid, 2, crc))
        disk.write(table)
        disk.seek((total - 33) * 512)
        disk.write(table)
        disk.write(header(total - 1, 1, total - 34, guid, total - 33, crc))
        disk.seek(first * 512)
        disk.write(boot)
        disk.write(fat)
        disk.write(fat)
        disk.write(entry(b'EFI        ', 0x10, 2))
        data_start = (first + 1 + 2 * fat_sectors + 32) * 512
        disk.seek(data_start)
        disk.write(entry(b'.          ', 0x10, 2) + entry(b'..         ', 0x10, 0)
                   + entry(b'BOOT       ', 0x10, 3))
        disk.seek(data_start + 4096)
        disk.write(entry(b'.          ', 0x10, 3) + entry(b'..         ', 0x10, 2)
                   + entry(b'BOOTX64 EFI', 0x20, 4, size))
        disk.seek(data_start + 8192)
        with efi.open('rb') as source:
            shutil.copyfileobj(source, disk)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True, help='New directory; never overwritten')
    parser.add_argument('--uefi-linker', type=Path, help='Optional Microsoft link.exe for UEFI linking')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env['CARGO_BUILD_BUILD_DIR'] = str(root / '.cargo-build')
    env['CARGO_TARGET_DIR'] = str(root / 'target')
    image = Path(subprocess.check_output(
        ['cargo', 'run', '--offline', '--quiet', '--', '--print-image'],
        cwd=root, env=env, text=True).strip())
    payload = out / 'wovenhat-system.img'
    shutil.copyfile(image, payload)
    env['WOVENHAT_INSTALL_IMAGE'] = str(payload)
    command = ['cargo', 'build', '--offline', '--release', '--manifest-path',
               'tools/woven-installer/Cargo.toml', '--target', 'x86_64-unknown-uefi']
    if args.uefi_linker:
        env['CARGO_TARGET_X86_64_UNKNOWN_UEFI_LINKER'] = str(args.uefi_linker.resolve(strict=True))
        command[1] = 'rustc'
        command += ['--bin', 'woven-installer', '--', '-C', 'link-arg=/NXCOMPAT:NO']
    subprocess.run(command,
                   cwd=root, env=env, check=True)
    efi = out / 'BOOTX64.EFI'
    shutil.copyfile(root / 'target/x86_64-unknown-uefi/release/woven-installer.efi', efi)
    usb = out / 'wovenhat-installer-usb.img'
    make_usb(efi, usb)
    readme = out / 'README.md'
    shutil.copyfile(root / 'docs/installer.md', readme)
    files = {}
    for path in (payload, efi, usb, readme):
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        files[path.name] = {'bytes': path.stat().st_size, 'sha256': digest}
    manifest = {'kind': 'experimental UEFI installer', 'architecture': 'x86_64',
                'secure_boot': False, 'target_logical_sector_size': 512,
                'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
                'uncommitted_changes': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=root)),
                'installer_sources': {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                                      for p in sorted((root / 'tools/woven-installer/src').glob('*.rs'))},
                'files': files}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    with zipfile.ZipFile(out / 'wovenhat-installer.zip', 'x', zipfile.ZIP_DEFLATED) as archive:
        for path in (usb, readme, out / 'manifest.json'):
            archive.write(path, path.name)
    print(usb)


if __name__ == '__main__':
    main()
