"""Check the persistent account fixture against the current login KDF contract."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "create-account-fixture.py"
SPEC = importlib.util.spec_from_file_location("account_fixture", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class AccountFixtureTests(unittest.TestCase):
    def test_persisted_account_uses_current_password_kdf(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "account.img"
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(image)], capture_output=True, check=False
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            with image.open("rb") as source:
                source.seek(MODULE.cluster_offset(5))
                record = source.read(MODULE.SECTOR).split(b"\0", 1)[0].strip()
            name, salt_hex, hash_hex = record.split(b":")
            self.assertEqual(name, b"persistent")
            self.assertEqual(salt_hex, b"1122334455667788")
            # Fixed PBKDF2-HMAC-SHA256 known answer for the public fixture's
            # PIN/salt and 100000 iterations, independent of generator code.
            self.assertEqual(
                hash_hex,
                b"1bfe0fe647a473f869594f41e675427e2fcc6fc3e1b42208c514a90530e552ef",
            )

    def test_existing_destination_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "existing.img"
            original = b"existing image must survive fixture generation"
            image.write_bytes(original)
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(image)], capture_output=True, check=False
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"Refusing to overwrite", result.stderr)
            self.assertEqual(image.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
