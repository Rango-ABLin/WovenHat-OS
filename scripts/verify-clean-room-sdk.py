"""Verify a clean-room, locked SDK view and emit deterministic provenance.

The check is intentionally read-only: it runs Cargo metadata offline, rejects
absolute/path dependencies outside this tree, and hashes only tracked source
files. It does not claim a reproducible binary until the caller also supplies
the same toolchain and build environment.
"""

import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path


def tracked_files(root: Path):
    result = subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True)
    return [root / item for item in result.stdout.decode().split("\0") if item]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("target/sdk-provenance.json"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    env = dict(os.environ, CARGO_NET_OFFLINE="true")
    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=root, env=env, check=False, capture_output=True, text=True,
    )
    if metadata.returncode != 0:
        print(metadata.stderr, end="")
        return metadata.returncode
    document = json.loads(metadata.stdout)
    outside = []
    root_resolved = root.resolve()
    for package in document["packages"]:
        manifest = Path(package["manifest_path"]).resolve()
        # Registry sources are expected to live in Cargo's cache. Only an
        # unsourced path package outside the tree is a clean-room violation.
        if package.get("source") is None and not str(manifest).startswith(str(root_resolved)):
            outside.append(str(manifest))
    if outside:
        print("outside-tree manifests:", *outside, sep="\n")
        return 2
    digest = hashlib.sha256()
    files = []
    for path in sorted(tracked_files(root)):
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            data = path.read_bytes()
            digest.update(relative.encode() + b"\0" + data)
            files.append({"path": relative, "sha256": hashlib.sha256(data).hexdigest()})
    output = root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({
        "schema": "wovenhat.sdk-provenance/v1",
        "source_sha256": digest.hexdigest(),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
        "packages": sorted({package["name"] + "@" + package["version"] for package in document["packages"]}),
        "tracked_files": files,
        "offline_locked_metadata": True,
    }, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
