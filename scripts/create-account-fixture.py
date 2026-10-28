"""Create an isolated FAT32 image containing a persistent WovenHat account."""
import hashlib
import pathlib
import struct
import sys


SECTOR = 512
TOTAL_SECTORS = 70000
RESERVED = 32
FAT_SECTORS = 600
DATA_START = RESERVED + 2 * FAT_SECTORS
ACCOUNT_LINE = "persistent:1122334455667788:{}\n"


def directory_entry(name, attributes, cluster, size=0):
    entry = bytearray(32)
    short = name.ljust(11).encode("ascii")
    entry[:11] = short
    entry[11] = attributes
    struct.pack_into("<H", entry, 20, (cluster >> 16) & 0xFFFF)
    struct.pack_into("<H", entry, 26, cluster & 0xFFFF)
    struct.pack_into("<I", entry, 28, size)
    return entry


def cluster_offset(cluster):
    return (DATA_START + cluster - 2) * SECTOR


def main():
    path = pathlib.Path(sys.argv[1])
    if path.exists():
        raise SystemExit(f"Refusing to overwrite existing fixture: {path}")
    digest = hashlib.sha256(bytes.fromhex("1122334455667788") + b"2468").hexdigest()
    account = ACCOUNT_LINE.format(digest).encode("ascii")

    with path.open("xb") as disk:
        disk.truncate(TOTAL_SECTORS * SECTOR)
        boot = bytearray(SECTOR)
        boot[:3] = b"\xeb\x58\x90"
        boot[3:11] = b"WOVENHAT"
        struct.pack_into("<H", boot, 11, SECTOR)
        boot[13] = 1
        struct.pack_into("<H", boot, 14, RESERVED)
        boot[16] = 2
        boot[21] = 0xF8
        struct.pack_into("<I", boot, 32, TOTAL_SECTORS)
        struct.pack_into("<I", boot, 36, FAT_SECTORS)
        struct.pack_into("<I", boot, 44, 2)
        struct.pack_into("<H", boot, 48, 1)
        struct.pack_into("<H", boot, 50, 6)
        boot[64] = 0x80
        boot[66] = 0x29
        struct.pack_into("<I", boot, 67, 0x574F564E)
        boot[71:82] = b"WOVENHAT   "
        boot[82:90] = b"FAT32   "
        boot[510:512] = b"\x55\xaa"
        info = bytearray(SECTOR)
        struct.pack_into("<I", info, 0, 0x41615252)
        struct.pack_into("<I", info, 484, 0x61417272)
        struct.pack_into("<I", info, 488, TOTAL_SECTORS - DATA_START - 1)
        struct.pack_into("<I", info, 492, 6)
        struct.pack_into("<I", info, 508, 0xAA550000)
        fat = bytearray(SECTOR)
        for cluster in (0, 1, 2, 3, 4, 5):
            struct.pack_into("<I", fat, cluster * 4, 0x0FFFFFF8 if cluster == 0 else 0x0FFFFFFF)
        for lba, data in ((0, boot), (1, info), (6, boot), (7, info),
                          (RESERVED, fat), (RESERVED + FAT_SECTORS, fat)):
            disk.seek(lba * SECTOR)
            disk.write(data)

        root = directory_entry("ETC", 0x10, 3)
        etc = directory_entry("WOVENHAT", 0x10, 4)
        wovenhat = directory_entry("ACCOUNTS", 0x01, 5, len(account))
        disk.seek(cluster_offset(2))
        disk.write(root)
        disk.seek(cluster_offset(3))
        disk.write(etc)
        disk.seek(cluster_offset(4))
        disk.write(wovenhat)
        disk.seek(cluster_offset(5))
        disk.write(account)
    print(f"Created account fixture: {path} (persistent/2468)")


if __name__ == "__main__":
    main()
