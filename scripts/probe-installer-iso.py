"""Probe whether the local toolchain can safely build a bootable WovenHat ISO.

This probe is deliberately fail-closed.  It never mounts an image, accesses a
physical disk, writes an ISO, or claims bootability.  A future ISO builder may
use the reported prerequisites, but support is only established by generating
an artifact and booting that exact artifact in QEMU.
"""

import argparse
import json
import shutil
from pathlib import Path


SECTOR_SIZE = 512
GPT_SIGNATURE = b"EFI PART"
ISO_BUILDERS = ("xorriso", "mkisofs", "genisoimage", "oscdimg")


def find_builder() -> dict[str, str] | None:
    for name in ISO_BUILDERS:
        path = shutil.which(name)
        if path:
            return {"name": name, "path": path}
    return None


def validate_source_image(path: Path) -> list[str]:
    blockers: list[str] = []
    if not path.is_file() or path.is_symlink():
        return [f"source image is not a regular non-symlink file: {path}"]
    size = path.stat().st_size
    if size == 0 or size % SECTOR_SIZE:
        blockers.append(f"source image is not a non-zero {SECTOR_SIZE}-byte sector multiple")
        return blockers
    with path.open("rb") as stream:
        mbr = stream.read(SECTOR_SIZE)
        stream.seek(SECTOR_SIZE)
        gpt = stream.read(SECTOR_SIZE)
    if mbr[510:512] != b"\x55\xaa":
        blockers.append("source image lacks a protective-MBR signature")
    if gpt[:8] != GPT_SIGNATURE:
        blockers.append("source image lacks a primary GPT header")
    return blockers


def find_bootloader(efi_tree: Path | None) -> tuple[str | None, list[str]]:
    if efi_tree is None:
        return None, [
            "no file-backed EFI system-partition tree was supplied",
            "required ISO boot file EFI/BOOT/BOOTX64.EFI cannot be verified",
        ]
    if not efi_tree.is_dir() or efi_tree.is_symlink():
        return None, [f"EFI tree is not a regular directory: {efi_tree}"]
    candidates = list(efi_tree.rglob("*"))
    for candidate in candidates:
        if (candidate.parent.name.upper() == "BOOT"
                and candidate.parent.parent.name.upper() == "EFI"
                and candidate.name.upper() == "BOOTX64.EFI"
                and candidate.is_file()
                and not candidate.is_symlink()
                and candidate.stat().st_size > 0):
            return str(candidate), []
    return None, [
        "EFI tree does not contain a non-empty EFI/BOOT/BOOTX64.EFI",
    ]


def probe(source_image: Path | None, efi_tree: Path | None) -> dict[str, object]:
    blockers: list[str] = []
    if source_image is None:
        blockers.append("no file-backed WovenHat UEFI disk image was supplied")
    else:
        blockers.extend(validate_source_image(source_image))
    builder = find_builder()
    if builder is None:
        blockers.append(
            "no supported ISO builder is installed (searched: "
            + ", ".join(ISO_BUILDERS)
            + ")"
        )
    bootloader, bootloader_blockers = find_bootloader(efi_tree)
    blockers.extend(bootloader_blockers)
    return {
        "format": "WovenHat ISO capability probe",
        "format_version": 1,
        "iso_supported": False,
        "status": "blocked" if blockers else "ready-for-build-validation",
        "source_image": str(source_image.resolve()) if source_image else None,
        "iso_builder": builder,
        "efi_boot_file": bootloader,
        "blockers": blockers,
        "safety": {
            "physical_disk_access": "none",
            "mounts_filesystems": False,
            "writes_artifacts": False,
            "qemu_boot_validation": "not-run",
        },
        "qualification_rule": (
            "ISO support remains unclaimed until an artifact is generated and "
            "that exact artifact boots in QEMU"
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-image", type=Path, help="file-backed GPT UEFI image")
    parser.add_argument("--efi-tree", type=Path, help="file-backed EFI tree containing EFI/BOOT/BOOTX64.EFI")
    args = parser.parse_args()
    result = probe(args.source_image.resolve() if args.source_image else None,
                   args.efi_tree.resolve() if args.efi_tree else None)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "ready-for-build-validation" else 2


if __name__ == "__main__":
    raise SystemExit(main())
