"""Host tests for the PKCS#11 release-signing adapter."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "hardware-release-signer.py"
PUBLIC_KEY = bytes.fromhex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")


class HardwareReleaseSignerTests(unittest.TestCase):
    def fake_environment(self, directory: Path) -> dict[str, str]:
        state = directory / "token-state.txt"
        fake_tool = directory / "pkcs11-tool.py"
        fake_tool.write_text(
            "import os\n"
            "from pathlib import Path\n"
            "import sys\n"
            "args = sys.argv[1:]\n"
            "key_id = args[args.index('--id') + 1]\n"
            "state = Path(os.environ['WOVENHAT_TEST_PKCS11_STATE'])\n"
            "present = state.read_text().splitlines() if state.exists() else []\n"
            "output = Path(args[args.index('--output-file') + 1]) if '--output-file' in args else None\n"
            "if '--read-object' in args:\n"
            "    if key_id not in present:\n"
            "        raise SystemExit(1)\n"
            "    output.write_bytes(bytes.fromhex(os.environ['WOVENHAT_TEST_PKCS11_SPKI']))\n"
            "elif '--keypairgen' in args:\n"
            "    state.write_text('\\n'.join(present + [key_id]) + '\\n')\n"
            "elif '--sign' in args:\n"
            "    output.write_bytes(bytes(64))\n"
            "else:\n"
            "    raise SystemExit(2)\n",
            encoding="utf-8",
        )
        environment = dict(os.environ)
        environment.update(
            {
                "WOVENHAT_PKCS11_MODULE": str(directory / "module.dll"),
                "WOVENHAT_PKCS11_SLOT": "1",
                "WOVENHAT_PKCS11_TOOL": sys.executable,
                "WOVENHAT_BOOT_PKCS11_KEY_ID": "01",
                "WOVENHAT_TEST_PKCS11_STATE": str(state),
                "WOVENHAT_TEST_PKCS11_SPKI": (SPKI_PREFIX + PUBLIC_KEY).hex(),
            }
        )
        environment["PYTHONFAKE"] = str(fake_tool)
        return environment

    def run_signer(
        self, environment: dict[str, str], *arguments: str
    ) -> subprocess.CompletedProcess[str]:
        command = [
            sys.executable,
            str(SCRIPT),
            *arguments,
        ]
        environment = dict(environment)
        tool = environment["PYTHONFAKE"]
        environment["WOVENHAT_PKCS11_TOOL"] = tool
        return subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            capture_output=True,
            text=True,
        )

    def test_provision_export_and_sign_use_expected_contract(self):
        with tempfile.TemporaryDirectory(prefix="wovenhat-pkcs11-test-") as temporary:
            directory = Path(temporary)
            environment = self.fake_environment(directory)
            record = directory / "boot-public-key.json"
            provisioned = self.run_signer(
                environment, "provision", "boot", str(record)
            )
            self.assertEqual(provisioned.returncode, 0, provisioned.stderr)
            self.assertEqual(
                json.loads(record.read_text(encoding="utf-8"))["public_key_hex"],
                PUBLIC_KEY.hex(),
            )

            public = directory / "boot-public.der"
            exported = self.run_signer(
                environment, "public", "boot", str(public)
            )
            self.assertEqual(exported.returncode, 0, exported.stderr)
            self.assertEqual(public.read_bytes(), PUBLIC_KEY)

            payload = directory / "payload.bin"
            signature = directory / "signature.bin"
            payload.write_bytes(b"release payload")
            signed = self.run_signer(
                environment,
                "sign",
                "boot",
                str(payload),
                str(signature),
                PUBLIC_KEY.hex(),
            )
            self.assertEqual(signed.returncode, 0, signed.stderr)
            self.assertEqual(len(signature.read_bytes()), 64)

            duplicate = self.run_signer(
                environment, "provision", "boot", str(directory / "duplicate.json")
            )
            self.assertNotEqual(duplicate.returncode, 0)
            self.assertIn("refusing to overwrite", duplicate.stderr)

    def test_rejects_a_public_key_different_from_the_pin(self):
        with tempfile.TemporaryDirectory(prefix="wovenhat-pkcs11-test-") as temporary:
            directory = Path(temporary)
            environment = self.fake_environment(directory)
            record = directory / "boot-public-key.json"
            self.run_signer(environment, "provision", "boot", str(record)).check_returncode()
            payload = directory / "payload.bin"
            payload.write_bytes(b"release payload")
            rejected = self.run_signer(
                environment,
                "sign",
                "boot",
                str(payload),
                str(directory / "signature.bin"),
                "00" * 32,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("does not match the pinned", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
