"""Verify production WAPP packaging enforces the custody boundary."""
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

        seed_environment = dict(base)
        seed_environment["WOVENHAT_APP_SIGNING_SEED_HEX"] = TEST_SEED
        packaged = run(root, source, output, seed_environment)
        if packaged.returncode != 0 or not output.is_file():
            raise SystemExit(packaged.stderr or "production packager rejected matching release keys")
        reference_signature = directory / "signature.bin"
        reference_signature.write_bytes(output.read_bytes()[16:80])

        mismatched = dict(seed_environment)
        mismatched["WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX"] = "00" * 32
        output.unlink(missing_ok=True)
        rejected_mismatch = run(root, source, output, mismatched)
        if (
            rejected_mismatch.returncode == 0
            or "does not match the trusted public key" not in rejected_mismatch.stderr
        ):
            raise SystemExit("production packager accepted a mismatched trusted public key")

        signer = directory / "external-signer.py"
        signer.write_text(
            "import os\n"
            "from pathlib import Path\n"
            "import sys\n"
            "if sys.argv[1] != 'sign' or sys.argv[2] != 'app':\n"
            "    raise SystemExit(2)\n"
            "Path(sys.argv[4]).write_bytes(Path(os.environ['WOVENHAT_TEST_SIGNATURE']).read_bytes())\n",
            encoding="utf-8",
        )
        external = dict(base)
        external["WOVENHAT_APP_EXTERNAL_SIGNER"] = str(signer)
        external["WOVENHAT_TEST_SIGNATURE"] = str(reference_signature)
        output.unlink(missing_ok=True)
        hardware_signed = run(root, source, output, external)
        if hardware_signed.returncode != 0 or not output.is_file():
            raise SystemExit(
                hardware_signed.stderr or "production packager rejected an external signer"
            )

        source.write_bytes(b"different-release-key-fixture")
        output.unlink(missing_ok=True)
        invalid_signature = run(root, source, output, external)
        if invalid_signature.returncode == 0 or "invalid signature" not in invalid_signature.stderr:
            raise SystemExit("production packager accepted an invalid external signature")

        ambiguous = dict(seed_environment)
        ambiguous["WOVENHAT_APP_EXTERNAL_SIGNER"] = str(signer)
        ambiguous["WOVENHAT_TEST_SIGNATURE"] = str(reference_signature)
        rejected_ambiguous = run(root, source, output, ambiguous)
        if rejected_ambiguous.returncode == 0 or "cannot both be set" not in rejected_ambiguous.stderr:
            raise SystemExit("production packager accepted ambiguous signing authority")
    print("Release-key provisioning boundary: PASS")


if __name__ == "__main__":
    main()
