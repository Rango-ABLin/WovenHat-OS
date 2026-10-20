"""Build and sign the production Ring-3 WovenPackage service image."""

import argparse
import os
from pathlib import Path
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    app = root / "apps" / "woven-package"
    output = args.output or root / "target" / "system-apps" / "0.8.0" / "packages" / "woven-package.wapp"
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.pop("CARGO_BUILD_BUILD_DIR", None)
    env["CARGO_TARGET_DIR"] = str(app / "target")
    subprocess.run(
        ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
        cwd=app,
        env=env,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    elf = app / "target" / "x86_64-unknown-none" / "release" / "wovenhat-package"
    packager_env = dict(os.environ)
    packager_env.pop("CARGO_BUILD_BUILD_DIR", None)
    packager_env["CARGO_TARGET_DIR"] = str(root / "target" / "wapp-packager")
    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "tools" / "wapp-packager" / "Cargo.toml"),
            "--",
            str(elf),
            str(output),
        ],
        cwd=root,
        env=packager_env,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
