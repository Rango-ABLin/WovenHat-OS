"""Package and validate a WovenHat UEFI disk image without touching media.

This is the bounded Stage 28 artifact boundary.  It consumes the image and
digest recorded by the existing release-validation manifest (or an explicitly
provided image), copies the image into a new output directory, and writes
machine-readable metadata and checksums.  It never opens a physical disk.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile


SECTOR_SIZE = 512
GPT_SIGNATURE = b"EFI PART"
MANIFEST_NAME = "installer-manifest.json"


def build_iso_artifact(source: Path, output: Path, volume_id: str) -> dict[str, object]:
    """Build the ISO through the reviewed file-only UEFI builder."""
    builder_path = Path(__file__).with_name("build-uefi-iso.py")
    spec = importlib.util.spec_from_file_location("wovenhat_uefi_iso", builder_path)
    if spec is None or spec.loader is None:
        raise ValueError("unable to load the UEFI ISO builder")
    builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(builder)
    return builder.build_iso(source, output, volume_id)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_uefi_disk_image(path: Path) -> dict[str, int | bool]:
    """Validate the file-level invariants needed by a UEFI disk artifact."""
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"image is not a regular non-symlink file: {path}")
    size = path.stat().st_size
    if size == 0 or size % SECTOR_SIZE:
        raise ValueError(f"image size is not a non-zero sector multiple: {size}")
    with path.open("rb") as stream:
        first_sector = stream.read(SECTOR_SIZE)
        stream.seek(SECTOR_SIZE)
        gpt_sector = stream.read(SECTOR_SIZE)
    protective_mbr = first_sector[510:512] == b"\x55\xaa"
    gpt_header = gpt_sector[:8] == GPT_SIGNATURE
    if not protective_mbr or not gpt_header:
        raise ValueError("image does not contain a protective MBR and GPT header")
    return {
        "sector_size": SECTOR_SIZE,
        "sector_count": size // SECTOR_SIZE,
        "bytes": size,
        "protective_mbr": protective_mbr,
        "gpt_header": gpt_header,
    }


def source_image(manifest: Path | None, image: Path | None) -> tuple[Path, dict[str, object]]:
    if (manifest is None) == (image is None):
        raise ValueError("provide exactly one of --validated-manifest or --image")
    if image is not None:
        resolved = image.resolve(strict=True)
        return resolved, {"input": "explicit-image", "recorded_sha256": None}
    record = json.loads(manifest.read_text(encoding="utf-8"))
    recorded = record.get("image")
    expected = record.get("image_sha256")
    if not isinstance(recorded, str) or not isinstance(expected, str):
        raise ValueError("validated manifest must contain image and image_sha256")
    candidate = Path(recorded)
    if not candidate.is_absolute():
        candidate = manifest.parent / candidate
    resolved = candidate.resolve(strict=True)
    actual = sha256(resolved)
    if actual != expected.lower():
        raise ValueError(f"validated image digest mismatch: {actual} != {expected}")
    return resolved, {
        "input": "release-validation",
        "validated_manifest": str(manifest.resolve()),
        "recorded_sha256": expected.lower(),
    }


def package(source: Path, output: Path, source_record: dict[str, object], version: str,
            *, include_iso: bool = False, volume_id: str = "WOVENHAT") -> Path:
    if output.exists():
        raise FileExistsError(f"refusing to overwrite existing output: {output}")
    checks = validate_uefi_disk_image(source)
    digest = sha256(source)
    output_parent = output.parent.resolve()
    output_parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f".{output.name}-", dir=output_parent) as temporary:
        staging = Path(temporary)
        artifact = staging / f"wovenhat-os-{version}-uefi.img"
        shutil.copyfile(source, artifact)
        if sha256(artifact) != digest:
            raise ValueError("copied artifact digest does not match source")
        iso_record: dict[str, object] | None = None
        if include_iso:
            iso_path = staging / f"wovenhat-os-{version}-uefi.iso"
            iso_record = build_iso_artifact(artifact, iso_path, volume_id)
            iso_record = {
                "file": iso_path.name,
                "sha256": sha256(iso_path),
                **iso_record,
            }
        metadata = {
            "format": "WovenHat installer artifact",
            "format_version": 1,
            "stage": "28-bounded",
            "version": version,
            "artifact": {
                "file": artifact.name,
                "kind": "uefi-disk-image",
                "sha256": digest,
                **checks,
            },
            "media": {
                "qemu": "supported as a file-backed UEFI disk image",
                "usb": "file image is ready for a separately reviewed writer",
                "iso": "included and requires exact-artifact UEFI QEMU acceptance"
                if iso_record is not None else "not-requested",
            },
            **({"iso_artifact": iso_record} if iso_record is not None else {}),
            "source": source_record,
            "physical_disk_access": "none",
            "destructive_partitioning": "not-implemented",
        }
        (staging / MANIFEST_NAME).write_text(
            json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        (staging / "SHA256SUMS").write_text(
            f"{digest}  {artifact.name}\n"
            f"{sha256(staging / MANIFEST_NAME)}  {MANIFEST_NAME}\n",
            encoding="ascii",
        )
        staging.rename(output)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--validated-manifest", type=Path,
                        help="release-validation/validated-image.json")
    inputs.add_argument("--image", type=Path, help="file-backed UEFI disk image")
    parser.add_argument("--output", type=Path, required=True,
                        help="new output directory; existing directories are refused")
    parser.add_argument("--version", default="0.8.0")
    parser.add_argument("--include-iso", action="store_true",
                        help="also build the file-backed UEFI ISO artifact")
    parser.add_argument("--iso-volume-id", default="WOVENHAT")
    args = parser.parse_args()
    try:
        source, record = source_image(
            args.validated_manifest.resolve() if args.validated_manifest else None,
            args.image,
        )
        result = package(source, args.output.resolve(), record, args.version,
                         include_iso=args.include_iso, volume_id=args.iso_volume_id)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    print(f"Packaged file-backed UEFI artifact: {result}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
