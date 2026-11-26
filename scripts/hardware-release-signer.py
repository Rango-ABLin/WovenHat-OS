"""PKCS#11 Ed25519 signing adapter for the WovenHat release tools."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


PURPOSES = {
    "app": ("WOVENHAT_APP_PKCS11_KEY_ID", "WovenHat release application v1"),
    "boot": ("WOVENHAT_BOOT_PKCS11_KEY_ID", "WovenHat release boot v1"),
    "sdk": ("WOVENHAT_SDK_PKCS11_KEY_ID", "WovenHat release SDK provenance v1"),
}
SPKI_ED25519_PREFIX = bytes.fromhex("302a300506032b6570032100")


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def required_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        fail(f"{name} is required")
    return value


def tool_path() -> str:
    configured = os.environ.get("WOVENHAT_PKCS11_TOOL")
    if configured:
        return configured
    discovered = shutil.which("pkcs11-tool")
    if not discovered:
        fail("pkcs11-tool is required (set WOVENHAT_PKCS11_TOOL to its path)")
    return discovered


def common_arguments() -> list[str]:
    arguments = ["--module", required_env("WOVENHAT_PKCS11_MODULE")]
    slot = os.environ.get("WOVENHAT_PKCS11_SLOT")
    if slot:
        arguments += ["--slot", slot]
    return arguments


def key_id(purpose: str) -> str:
    name, _ = PURPOSES[purpose]
    return required_env(name)


def run(command: list[str]) -> subprocess.CompletedProcess[bytes]:
    if Path(command[0]).suffix.lower() == ".py":
        command = [sys.executable, *command]
    return subprocess.run(command, check=False)


def read_public_bytes(path: Path) -> bytes:
    data = path.read_bytes()
    if len(data) == 32:
        return data
    if len(data) == len(SPKI_ED25519_PREFIX) + 32 and data.startswith(SPKI_ED25519_PREFIX):
        return data[-32:]
    fail("token returned an unsupported Ed25519 public-key encoding")


def public_key(purpose: str, output: Path | None = None) -> bytes:
    with tempfile.TemporaryDirectory(prefix="wovenhat-public-key-") as directory:
        encoded = Path(directory) / "public.der"
        command = [
            tool_path(),
            *common_arguments(),
            "--read-object",
            "--type",
            "pubkey",
            "--id",
            key_id(purpose),
            "--output-file",
            str(encoded),
        ]
        result = run(command)
        if result.returncode != 0 or not encoded.is_file():
            fail(f"failed to export the {purpose} public key")
        raw = read_public_bytes(encoded)
        if output:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(raw)
        return raw


def sign(purpose: str, input_path: Path, output_path: Path, trusted_hex: str) -> None:
    trusted = bytes.fromhex(trusted_hex)
    if len(trusted) != 32:
        fail("trusted public key must contain exactly 32 bytes")
    actual = public_key(purpose)
    if actual != trusted:
        fail(f"token public key does not match the pinned {purpose} release key")

    with tempfile.TemporaryDirectory(prefix="wovenhat-token-sign-") as directory:
        signature = Path(directory) / "signature.bin"
        command = [
            tool_path(),
            *common_arguments(),
            "--login",
            "--sign",
            "--mechanism",
            "EDDSA",
            "--id",
            key_id(purpose),
            "--input-file",
            str(input_path),
            "--output-file",
            str(signature),
        ]
        result = run(command)
        if result.returncode != 0 or not signature.is_file():
            fail(f"hardware {purpose} signature failed")
        data = signature.read_bytes()
        if len(data) != 64:
            fail(f"hardware {purpose} signer returned {len(data)} bytes, expected 64")
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(data)


def provision(purpose: str, output_path: Path) -> None:
    try:
        public_key(purpose)
    except SystemExit:
        pass
    else:
        fail(f"token object already exists for {purpose}; refusing to overwrite it")

    command = [
        tool_path(),
        *common_arguments(),
        "--login",
        "--keypairgen",
        "--key-type",
        "EC:edwards25519",
        "--id",
        key_id(purpose),
        "--label",
        PURPOSES[purpose][1],
        "--usage-sign",
    ]
    result = run(command)
    if result.returncode != 0:
        fail(f"failed to generate the {purpose} key on the token")
    raw = public_key(purpose)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(
            {
                "schema": "wovenhat.release-public-key/v1",
                "purpose": purpose,
                "algorithm": "Ed25519",
                "key_id": key_id(purpose),
                "public_key_hex": raw.hex(),
                "private_material_exported": False,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("sign", "public", "provision"))
    parser.add_argument("purpose", choices=PURPOSES)
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path, nargs="?")
    parser.add_argument("trusted_public_hex", nargs="?")
    args = parser.parse_args()
    if args.command == "sign":
        if args.second is None or args.trusted_public_hex is None:
            parser.error("sign requires INPUT OUTPUT TRUSTED_PUBLIC_HEX")
        sign(args.purpose, args.first, args.second, args.trusted_public_hex)
    elif args.command == "public":
        if args.second is not None or args.trusted_public_hex is not None:
            parser.error("public requires OUTPUT")
        public = public_key(args.purpose)
        args.first.parent.mkdir(parents=True, exist_ok=True)
        args.first.write_bytes(public)
    else:
        if args.second is not None or args.trusted_public_hex is not None:
            parser.error("provision requires OUTPUT")
        provision(args.purpose, args.first)
    return 0


if __name__ == "__main__":
    sys.exit(main())
