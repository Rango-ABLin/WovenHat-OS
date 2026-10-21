"""Verify production WAPP packaging rejects development-key fallback."""
import os
from pathlib import Path
import subprocess
import tempfile


TEST_SEED = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
TEST_PUBLIC_KEY = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"


def run(root, input_path, output_path, environment):
    return subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path",
         str(root / "tools" / "wapp-packager" / "Cargo.toml"), "--",
         str(input_path), str(output_path)],
        cwd=root, env=environment, capture_output=True, text=True,
    )


def main():
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="wovenhat-release-key-") as directory:
        directory = Path(directory)
        source = directory / "fixture.elf"
        output = directory / "fixture.wapp"
        source.write_bytes(b"release-key-fixture")
        base = os.environ.copy()
        base.pop("WOVENHAT_APP_SIGNING_SEED_HEX", None)
        base["WOVENHAT_APP_RELEASE_MODE"] = "1"
        base["WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX"] = TEST_PUBLIC_KEY
        rejected = run(root, source, output, base)
        if rejected.returncode == 0 or "requires WOVENHAT_APP_SIGNING_SEED_HEX" not in rejected.stderr:
            raise SystemExit("production packager accepted a missing signing seed")

        accepted = dict(base)
        accepted["WOVENHAT_APP_SIGNING_SEED_HEX"] = TEST_SEED
        packaged = run(root, source, output, accepted)
        if packaged.returncode != 0 or not output.is_file():
            raise SystemExit(packaged.stderr or "production packager rejected matching release keys")
    print("Release-key provisioning boundary: PASS")


if __name__ == "__main__":
    main()
