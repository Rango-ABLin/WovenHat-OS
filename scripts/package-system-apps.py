"""Build, sign, and catalog the accepted WovenHat system applications.

The output is an install plan, not a boot image.  The kernel currently accepts
one WAPP package per Stage 16.1 acceptance boot; keeping this packaging step
separate makes that limitation explicit while giving release tooling a
reproducible multi-application artifact to consume when multi-package install
is enabled.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


APPS = (
    {
        "id": "woven-calculator",
        "directory": "woven-calculator",
        "binary": "wovenhat-calculator",
        "display_name": "WovenCalculator",
        "install_path": "/bin/wovenhat-calculator",
        "exec": "/bin/wovenhat-calculator",
    },
    {
        "id": "woven-terminal",
        "directory": "woven-terminal",
        "binary": "wovenhat-terminal",
        "display_name": "WovenTerminal",
        "install_path": "/bin/wovenhat-terminal",
        "exec": "/bin/wovenhat-terminal --interactive",
    },
    {
        "id": "woven-files",
        "directory": "woven-files",
        "binary": "wovenhat-files",
        "display_name": "WovenFiles",
        "install_path": "/bin/wovenhat-files",
        "exec": "/bin/wovenhat-files",
    },
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(command, *, cwd: Path, env: dict[str, str]) -> None:
    print("+", " ".join(str(part) for part in command))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def validate_wapp(path: Path) -> dict[str, int | str]:
    data = path.read_bytes()
    header_length = 4 + 2 + 2 + 8 + 64
    if len(data) < header_length or data[:4] != b"WAPP":
        raise RuntimeError(f"{path} is not a WAPP package")
    version = int.from_bytes(data[4:6], "little")
    declared_header = int.from_bytes(data[6:8], "little")
    payload_length = int.from_bytes(data[8:16], "little")
    if version != 1 or declared_header != header_length:
        raise RuntimeError(f"{path} has an unsupported WAPP header")
    if payload_length != len(data) - header_length or payload_length == 0:
        raise RuntimeError(f"{path} has an invalid WAPP payload length")
    if data[header_length:header_length + 4] != b"\x7fELF":
        raise RuntimeError(f"{path} does not contain an ELF payload")
    return {
        "format": "WAPP",
        "version": version,
        "header_bytes": header_length,
        "payload_bytes": payload_length,
        "package_bytes": len(data),
        "sha256": sha256(path),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="output directory (default: target/system-apps/<version>)")
    parser.add_argument("--version", default="0.8.0", help="catalog version")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = args.output or root / "target" / "system-apps" / args.version
    output = output.resolve()
    package_dir = output / "packages"
    catalog_dir = output / "catalog"
    package_dir.mkdir(parents=True, exist_ok=True)
    catalog_dir.mkdir(parents=True, exist_ok=True)

    packager_target = root / "target" / "wapp-packager"
    packager_manifest = root / "tools" / "wapp-packager" / "Cargo.toml"
    records = []
    with tempfile.TemporaryDirectory(prefix="wovenhat-system-apps-") as temporary:
        temporary_root = Path(temporary)
        for app in APPS:
            app_root = root / "apps" / app["directory"]
            env = dict(os.environ)
            env.pop("CARGO_BUILD_BUILD_DIR", None)
            env["CARGO_TARGET_DIR"] = str(app_root / "target")
            run(
                ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
                cwd=app_root,
                env=env,
            )
            elf = app_root / "target" / "x86_64-unknown-none" / "release" / app["binary"]
            if not elf.is_file():
                raise RuntimeError(f"missing release ELF: {elf}")
            package = temporary_root / f"{app['id']}.wapp"
            package_env = dict(os.environ)
            package_env.pop("CARGO_BUILD_BUILD_DIR", None)
            package_env["CARGO_TARGET_DIR"] = str(packager_target)
            run(
                ["cargo", "run", "--quiet", "--manifest-path", str(packager_manifest), "--", str(elf), str(package)],
                cwd=root,
                env=package_env,
            )
            package_info = validate_wapp(package)
            final_package = package_dir / package.name
            shutil.copyfile(package, final_package)
            descriptor = catalog_dir / f"{app['id']}.app"
            descriptor.write_text(
                f"Name={app['display_name']}\nExec={app['exec']}\n",
                encoding="ascii",
                newline="\n",
            )
            records.append({
                "id": app["id"],
                "name": app["display_name"],
                "binary": app["binary"],
                "install_path": app["install_path"],
                "exec": app["exec"],
                "package": f"packages/{final_package.name}",
                "package_sha256": package_info["sha256"],
                "package_bytes": package_info["package_bytes"],
                "catalog": f"catalog/{descriptor.name}",
                "catalog_sha256": sha256(descriptor),
            })

    manifest = {
        "format": "WovenHat system-app install plan",
        "format_version": 1,
        "version": args.version,
        "package_signature_format": "WAPP-v1",
        "kernel_install_boundary": "stage16.1-single-package-acceptance",
        "apps": records,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Packaged {len(records)} signed system applications in {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
