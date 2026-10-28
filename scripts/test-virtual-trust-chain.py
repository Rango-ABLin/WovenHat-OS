#!/usr/bin/env python3
"""Fail-closed virtual Secure Boot/TPM trust-chain qualification.

This command qualifies only a disposable QEMU/OVMF/swtpm boundary.  It never
opens a host device, changes host firmware or TPM state, or writes a caller's
source image or OVMF variable store.  A result is ``QUALIFIED`` only when:

* QEMU, OVMF CODE, OVMF VARS, and swtpm are available;
* the caller explicitly supplies a development private key and certificate;
* OpenSSL proves that the key and certificate match;
* the certificate DER is enrolled in the supplied OVMF VARS template; and
* QEMU, with Secure Flash and a software TPM attached, boots the copied
  file-backed image and emits the expected signed-kernel marker.

Missing tools, malformed inputs, an unenrolled certificate, a failed guest,
or an unobservable marker produce a machine-readable ``NOT_QUALIFIED`` result
and a non-zero exit code.  Private key contents are never copied to the
result or passed to QEMU; only a SHA-256 fingerprint is recorded.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from typing import Any, Iterable


SCHEMA = "WOVENHAT-VIRTUAL-TRUST-QUALIFICATION-V1"
DEFAULT_MARKER = "[S15.5] secure boot handoff: PASSED"
DEFAULT_OUTPUT = Path("target") / "virtual-trust-qualification" / "result.json"
ACCEPTABLE_QEMU_EXIT_CODES = (0, 33)
MAX_INPUT_BYTES = 8 * 1024 * 1024 * 1024
MAX_KEY_BYTES = 16 * 1024 * 1024

# Deliberately conservative device-path rejection.  A normal file named
# ``disk.img`` is acceptable; Windows/Linux raw-device spellings are not.
DEVICE_PATH_RE = re.compile(
    r"(?i)(?:^|[/\\])dev[/\\]|(?:^|[/\\])physicaldrive\d*$|"
    r"(?:^|[/\\])(?:nvme\d+n\d+|sd[a-z]+|vd[a-z]+)$|"
    r"\\\\[.?]\\physicaldrive\d+"
)


class TrustInputError(ValueError):
    """A caller input cannot be accepted at the trust boundary."""


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _path_text(path: Path | str | None) -> str | None:
    return None if path is None else str(path)


def _is_device_path(path: Path | str) -> bool:
    return bool(DEVICE_PATH_RE.search(str(path)))


def _regular_file(path: Path | None, label: str, *, max_bytes: int = MAX_INPUT_BYTES) -> Path:
    if path is None:
        raise TrustInputError(f"{label} was not supplied")
    if _is_device_path(path):
        raise TrustInputError(f"{label} is a physical or raw-device path")
    try:
        stat = path.lstat()
    except OSError as exc:
        raise TrustInputError(f"{label} does not exist or is unreadable") from exc
    if path.is_symlink() or not path.is_file():
        raise TrustInputError(f"{label} must be an existing regular non-symlink file")
    if stat.st_size <= 0 or stat.st_size > max_bytes:
        raise TrustInputError(f"{label} size is outside the bounded file-image range")
    return path.resolve()


def _tool(path: Path | None, name: str) -> Path:
    if path is not None:
        if _is_device_path(path) or not path.is_file():
            raise TrustInputError(f"{name} must be an existing executable file")
        return path.resolve()
    discovered = shutil.which(name)
    if discovered is None:
        raise TrustInputError(f"{name} is unavailable")
    return Path(discovered).resolve()


def _find_ovmf_code(qemu: Path | None) -> Path | None:
    if qemu is None:
        return None
    candidates = (
        qemu.parent / "share" / "edk2-x86_64-code.fd",
        qemu.parent / "share" / "edk2" / "x64" / "OVMF_CODE.fd",
        qemu.parent / "share" / "OVMF_CODE.fd",
    )
    return next((item.resolve() for item in candidates if item.is_file()), None)


def _openssl_public_key(openssl: Path, key: Path) -> bytes:
    result = subprocess.run(
        [str(openssl), "pkey", "-in", str(key), "-pubout", "-outform", "DER"],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0 or not result.stdout:
        raise TrustInputError("development private key is not a readable OpenSSL key")
    return result.stdout


def _openssl_certificate_public_key(openssl: Path, certificate: Path) -> tuple[bytes, bytes]:
    certificate_der = subprocess.run(
        [str(openssl), "x509", "-in", str(certificate), "-outform", "DER"],
        capture_output=True,
        check=False,
    )
    public_pem = subprocess.run(
        [str(openssl), "x509", "-in", str(certificate), "-pubkey", "-noout"],
        capture_output=True,
        check=False,
    )
    if certificate_der.returncode != 0 or not certificate_der.stdout:
        raise TrustInputError("development certificate is not a readable X.509 certificate")
    if public_pem.returncode != 0 or not public_pem.stdout:
        raise TrustInputError("development certificate has no readable public key")
    public_der = subprocess.run(
        [str(openssl), "pkey", "-pubin", "-outform", "DER"],
        input=public_pem.stdout,
        capture_output=True,
        check=False,
    )
    if public_der.returncode != 0 or not public_der.stdout:
        raise TrustInputError("development certificate public key is not readable")
    return certificate_der.stdout, public_der.stdout


def validate_development_key_material(
    key: Path | None, certificate: Path | None, openssl: Path | None = None
) -> dict[str, Any]:
    """Validate explicit development key material without exposing its bytes."""

    key_path = _regular_file(key, "development private key", max_bytes=MAX_KEY_BYTES)
    certificate_path = _regular_file(
        certificate, "development certificate", max_bytes=MAX_KEY_BYTES
    )
    openssl_path = _tool(openssl, "openssl")
    key_public = _openssl_public_key(openssl_path, key_path)
    certificate_der, certificate_public = _openssl_certificate_public_key(
        openssl_path, certificate_path
    )
    if key_public != certificate_public:
        raise TrustInputError("development private key and certificate public keys do not match")
    return {
        "private_key_sha256": _sha256(key_path),
        "certificate_sha256": hashlib.sha256(certificate_der).hexdigest(),
        "certificate_der": certificate_der,
        "openssl": str(openssl_path),
    }


def _reserve_tcp_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def build_qemu_command(
    qemu: Path,
    ovmf_code: Path,
    ovmf_vars: Path,
    disk_image: Path,
    serial_log: Path,
    tpm_port: int,
    *,
    marker: str = DEFAULT_MARKER,
) -> list[str]:
    """Build the isolated QEMU command; ``marker`` documents the contract."""

    del marker  # The marker is checked in the serial evidence after boot.
    return [
        str(qemu),
        "-accel",
        "tcg",
        "-machine",
        "q35,smm=on",
        "-m",
        "512M",
        "-smp",
        "1",
        "-display",
        "none",
        "-serial",
        f"file:{serial_log}",
        "-no-reboot",
        "-no-shutdown",
        "-global",
        "driver=cfi.pflash01,property=secure,value=on",
        "-drive",
        f"if=pflash,format=raw,readonly=on,file={ovmf_code}",
        "-drive",
        f"if=pflash,format=raw,file={ovmf_vars}",
        "-chardev",
        f"socket,id=chrtpm,host=127.0.0.1,port={tpm_port}",
        "-tpmdev",
        "emulator,id=tpm0,chardev=chrtpm",
        "-device",
        "tpm-tis,tpmdev=tpm0",
        "-drive",
        f"if=virtio,format=raw,readonly=on,file={disk_image}",
    ]


def _copy_read_only_source(source: Path, destination: Path) -> None:
    before = _sha256(source)
    shutil.copyfile(source, destination)
    if _sha256(source) != before:
        raise TrustInputError(f"source changed while copied: {source.name}")
    os.chmod(destination, 0o600)


def _wait_for_process(process: subprocess.Popen[bytes], seconds: float = 3.0) -> bool:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            return False
        time.sleep(0.05)
    return True


def _stop_process(process: subprocess.Popen[bytes] | None) -> int | None:
    if process is None:
        return None
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    return process.returncode


def run_virtual_guest(
    qemu: Path,
    swtpm: Path,
    ovmf_code: Path,
    ovmf_vars: Path,
    disk_image: Path,
    output: Path,
    *,
    marker: str = DEFAULT_MARKER,
    timeout: float = 180.0,
) -> dict[str, Any]:
    """Run only copied file images with a disposable software TPM state."""

    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="wovenhat-virtual-trust-") as temporary:
        work = Path(temporary)
        disk_copy = work / "guest-disk.img"
        vars_copy = work / "OVMF_VARS.fd"
        tpm_state = work / "swtpm-state"
        tpm_state.mkdir()
        serial_log = work / "qemu-serial.log"
        swtpm_log = work / "swtpm.log"
        _copy_read_only_source(disk_image, disk_copy)
        _copy_read_only_source(ovmf_vars, vars_copy)
        tpm_port = _reserve_tcp_port()
        swtpm_command = [
            str(swtpm),
            "socket",
            "--tpm2",
            "--tpmstate",
            f"dir={tpm_state}",
            "--ctrl",
            f"type=tcp,port={tpm_port}",
        ]
        qemu_command = build_qemu_command(
            qemu,
            ovmf_code,
            vars_copy,
            disk_copy,
            serial_log,
            tpm_port,
            marker=marker,
        )
        with swtpm_log.open("wb") as log:
            tpm_process = subprocess.Popen(
                swtpm_command,
                cwd=work,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
        qemu_process: subprocess.Popen[bytes] | None = None
        qemu_returncode: int | None = None
        timed_out = False
        try:
            if not _wait_for_process(tpm_process):
                raise TrustInputError("swtpm exited before QEMU could connect")
            qemu_process = subprocess.Popen(
                qemu_command,
                cwd=work,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            try:
                qemu_process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                qemu_process.kill()
                qemu_process.communicate()
            qemu_returncode = qemu_process.returncode
        finally:
            swtpm_returncode = _stop_process(tpm_process)
        serial = serial_log.read_text(encoding="utf-8", errors="replace") if serial_log.exists() else ""
        swtpm_output = swtpm_log.read_text(encoding="utf-8", errors="replace") if swtpm_log.exists() else ""
        # Keep bounded evidence, not private key material or an unbounded guest log.
        (output / "qemu-serial.log").write_text(serial[-256 * 1024 :], encoding="utf-8")
        (output / "swtpm.log").write_text(swtpm_output[-64 * 1024 :], encoding="utf-8")
        marker_seen = marker in serial
        return {
            "attempted": True,
            "qemu_returncode": qemu_returncode,
            "swtpm_returncode": swtpm_returncode,
            "timed_out": timed_out,
            "expected_marker": marker,
            "marker_seen": marker_seen,
            "qemu_command": qemu_command,
            "swtpm_command": swtpm_command,
            "serial_log": str(output / "qemu-serial.log"),
            "swtpm_log": str(output / "swtpm.log"),
            "passed": (
                not timed_out
                and qemu_returncode in ACCEPTABLE_QEMU_EXIT_CODES
                and marker_seen
            ),
        }


def _blocker(code: str, detail: str) -> dict[str, str]:
    return {"code": code, "detail": detail}


def _result(
    blockers: Iterable[dict[str, str]],
    *,
    checks: list[dict[str, Any]],
    inputs: dict[str, Any],
    execution: dict[str, Any],
    output: Path,
) -> dict[str, Any]:
    blocker_list = list(blockers)
    return {
        "schema": SCHEMA,
        "generated_utc": _utc_now(),
        "status": "QUALIFIED" if not blocker_list else "NOT_QUALIFIED",
        "qualified": not blocker_list,
        "blockers": blocker_list,
        "checks": checks,
        "inputs": inputs,
        "execution": execution,
        "safety": {
            "disposable_file_images_only": True,
            "source_images_written": False,
            "raw_device_opened": False,
            "host_firmware_changed": False,
            "host_tpm_changed": False,
            "private_key_recorded": False,
        },
        "result_path": str(output),
    }


def _write_result(output: Path, result: dict[str, Any]) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp")
    temporary.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, output)


def qualify(args: argparse.Namespace) -> tuple[int, dict[str, Any]]:
    output = args.output.resolve()
    blockers: list[dict[str, str]] = []
    checks: list[dict[str, Any]] = []
    inputs: dict[str, Any] = {}
    execution: dict[str, Any] = {"attempted": False}
    resolved: dict[str, Path] = {}

    if _is_device_path(args.output):
        result = _result(
            [_blocker("unsafe-output-path", "result output cannot be a physical or raw-device path")],
            checks=[],
            inputs={},
            execution=execution,
            output=output,
        )
        return 2, result

    def file_input(name: str, path: Path | None, label: str, *, max_bytes: int = MAX_INPUT_BYTES) -> None:
        try:
            value = _regular_file(path, label, max_bytes=max_bytes)
        except TrustInputError as exc:
            prefix = "missing" if path is None else "invalid"
            blockers.append(_blocker(f"{prefix}-{name.replace('_', '-')}", str(exc)))
            checks.append({"name": name, "status": "BLOCKED", "detail": str(exc)})
            return
        resolved[name] = value
        inputs[name] = {"path": str(value), "sha256": _sha256(value), "bytes": value.stat().st_size}
        checks.append({"name": name, "status": "PASS"})

    def tool_input(name: str, path: Path | None, executable: str) -> None:
        try:
            value = _tool(path, executable)
        except TrustInputError as exc:
            blockers.append(_blocker(f"missing-{name}", str(exc)))
            checks.append({"name": name, "status": "BLOCKED", "detail": str(exc)})
            return
        resolved[name] = value
        checks.append({"name": name, "status": "PASS", "path": str(value)})

    tool_input("qemu", args.qemu, "qemu-system-x86_64")
    if args.ovmf_code is not None:
        file_input("ovmf_code", args.ovmf_code, "OVMF CODE", max_bytes=MAX_KEY_BYTES)
    elif "qemu" in resolved:
        code = _find_ovmf_code(resolved["qemu"])
        if code is None:
            blockers.append(_blocker("missing-ovmf-code", "OVMF CODE firmware was not found adjacent to QEMU"))
            checks.append({"name": "ovmf_code", "status": "BLOCKED", "detail": "OVMF CODE firmware not found"})
        else:
            file_input("ovmf_code", code, "OVMF CODE", max_bytes=MAX_KEY_BYTES)
    else:
        blockers.append(_blocker("missing-ovmf-code", "OVMF CODE cannot be resolved without QEMU"))
        checks.append({"name": "ovmf_code", "status": "BLOCKED", "detail": "OVMF CODE cannot be resolved"})
    file_input("ovmf_vars", args.ovmf_vars, "OVMF VARS template", max_bytes=MAX_KEY_BYTES)
    tool_input("swtpm", args.swtpm, "swtpm")
    file_input("disk_image", args.disk_image, "guest disk image")
    file_input("development_key", args.development_key, "development private key", max_bytes=MAX_KEY_BYTES)
    file_input("development_certificate", args.development_certificate, "development certificate", max_bytes=MAX_KEY_BYTES)

    key_material: dict[str, Any] | None = None
    if "development_key" in resolved and "development_certificate" in resolved:
        try:
            key_material = validate_development_key_material(
                resolved["development_key"],
                resolved["development_certificate"],
                args.openssl,
            )
            inputs["development_key"] = {
                "path": str(resolved["development_key"]),
                "sha256": key_material["private_key_sha256"],
                "bytes": resolved["development_key"].stat().st_size,
            }
            inputs["development_certificate"] = {
                "path": str(resolved["development_certificate"]),
                "sha256": key_material["certificate_sha256"],
                "bytes": resolved["development_certificate"].stat().st_size,
            }
            checks.append({"name": "development_key_matches_certificate", "status": "PASS"})
        except (TrustInputError, OSError) as exc:
            code = "missing-openssl" if str(exc) == "openssl is unavailable" else "invalid-development-key-material"
            blockers.append(_blocker(code, str(exc)))
            checks.append({"name": "development_key_matches_certificate", "status": "BLOCKED", "detail": str(exc)})

    if key_material is not None and "ovmf_vars" in resolved:
        try:
            vars_bytes = resolved["ovmf_vars"].read_bytes()
            enrolled = key_material["certificate_der"] in vars_bytes
        except OSError:
            enrolled = False
        if not enrolled:
            blockers.append(
                _blocker(
                    "development-certificate-not-enrolled",
                    "the supplied certificate DER was not found in the supplied OVMF VARS template",
                )
            )
            checks.append({"name": "certificate_enrolled_in_ovmf_vars", "status": "BLOCKED"})
        else:
            checks.append({"name": "certificate_enrolled_in_ovmf_vars", "status": "PASS"})

    if args.preflight_only:
        blockers.append(_blocker("qemu-execution-not-requested", "preflight-only mode cannot qualify a trust chain"))
    elif not blockers:
        try:
            execution = run_virtual_guest(
                resolved["qemu"],
                resolved["swtpm"],
                resolved["ovmf_code"],
                resolved["ovmf_vars"],
                resolved["disk_image"],
                output.parent,
                marker=args.marker,
                timeout=args.timeout,
            )
            if not execution["passed"]:
                blockers.append(_blocker("virtual-guest-verification-failed", "QEMU did not return an accepted code and emit the expected trust marker"))
        except (OSError, TrustInputError, subprocess.SubprocessError) as exc:
            blockers.append(_blocker("virtual-execution-error", str(exc)))
            execution = {"attempted": True, "passed": False}
    else:
        execution = {"attempted": False, "passed": False}

    result = _result(
        blockers,
        checks=checks,
        inputs=inputs,
        execution=execution,
        output=output,
    )
    _write_result(output, result)
    return (0 if result["qualified"] else 2), result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", type=Path, help="explicit qemu-system-x86_64 executable")
    parser.add_argument("--ovmf-code", "--firmware", dest="ovmf_code", type=Path, help="explicit OVMF CODE firmware")
    parser.add_argument("--ovmf-vars", "--firmware-vars", dest="ovmf_vars", type=Path, help="explicit Secure-Boot-enrolled OVMF VARS template")
    parser.add_argument("--swtpm", type=Path, help="explicit swtpm executable")
    parser.add_argument("--disk-image", "--image", dest="disk_image", type=Path, help="existing signed, file-backed guest image")
    parser.add_argument("--development-key", "--dev-key", dest="development_key", type=Path, help="caller-supplied development private key")
    parser.add_argument("--development-certificate", "--dev-certificate", "--dev-cert", dest="development_certificate", type=Path, help="caller-supplied development X.509 certificate")
    parser.add_argument("--openssl", type=Path, help="explicit OpenSSL executable")
    parser.add_argument("--marker", default=DEFAULT_MARKER, help="exact serial marker proving the signed handoff")
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--preflight-only", action="store_true", help="validate the contract but deliberately do not qualify")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT, help="machine-readable result path")
    args = parser.parse_args(argv)
    if args.timeout <= 0 or args.timeout > 1800:
        parser.error("--timeout must be between 0 and 1800 seconds")
    try:
        code, result = qualify(args)
    except (OSError, ValueError) as exc:
        output = args.output.resolve()
        result = _result(
            [_blocker("qualification-command-error", str(exc))],
            checks=[],
            inputs={},
            execution={"attempted": False},
            output=output,
        )
        _write_result(output, result)
        code = 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
