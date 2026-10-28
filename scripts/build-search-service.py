"""Build the signed least-privilege WovenSearch Ring-3 service image."""

import argparse
import os
from pathlib import Path
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / "apps" / "woven-search"
    output = (args.output or root / "target" / "services" / "woven-search.wapp").resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    build_env = dict(os.environ)
    build_env.pop("CARGO_BUILD_BUILD_DIR", None)
    build_env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(
        ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app,
        env=build_env,
        check=True,
    )
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-search"
    package_env = dict(os.environ)
    package_env.pop("CARGO_BUILD_BUILD_DIR", None)
    package_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    subprocess.run(
        [
            "cargo", "run", "--quiet", "--manifest-path",
            str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--",
            str(elf), str(output),
        ],
        cwd=root,
        env=package_env,
        check=True,
    )
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
