"""Create a bounded, privacy-filtered diagnostic evidence bundle.

Only explicitly selected text evidence is copied. Credentials, key material,
tokens, and binary files are rejected or redacted; no source-tree traversal or
ambient user-data collection occurs.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_BUNDLE_BYTES = 16 * 1024 * 1024
TEXT_SUFFIXES = {".log", ".txt", ".json", ".md"}
SENSITIVE_NAME = re.compile(r"(secret|password|passwd|token|credential|seed|private|\.pem$|\.key$|\.p12$)", re.I)
SENSITIVE_VALUE = re.compile(
    r"(?i)(WOVENHAT_[A-Z0-9_]+|bearer\s+[^\s,;]+|(?:password|passwd|token|secret|seed|private[_ -]?key|pin)\s*[=:]\s*)[^\s,;]+"
)


def redact(line: str) -> str:
    def replacement(match: re.Match[str]) -> str:
        value = match.group(0)
        if value.lower().startswith("bearer"):
            return "Bearer=<redacted>"
        prefix = re.split(r"[=:]", value, maxsplit=1)[0].strip()
        return f"{prefix}=<redacted>"

    return SENSITIVE_VALUE.sub(replacement, line)


def collect(root: Path, source: Path) -> tuple[str, bytes]:
    resolved = source.resolve()
    allowed_roots = [(root / "target").resolve(), (root / "audit-artifacts").resolve()]
    if not any(resolved == allowed or str(resolved).startswith(str(allowed) + "\\") for allowed in allowed_roots):
        raise SystemExit(f"diagnostic source is outside target/audit-artifacts: {source}")
    if not resolved.is_file():
        raise SystemExit(f"diagnostic source is not a file: {source}")
    if SENSITIVE_NAME.search(resolved.name) or resolved.suffix.lower() not in TEXT_SUFFIXES:
        raise SystemExit(f"diagnostic source is sensitive or non-text: {source}")
    data = resolved.read_bytes()
    if len(data) > MAX_FILE_BYTES or b"\x00" in data:
        raise SystemExit(f"diagnostic source exceeds text bounds: {source}")
    text = data.decode("utf-8", errors="strict")
    redacted = "".join(redact(line) for line in text.splitlines(keepends=True))
    relative = resolved.relative_to(root).as_posix()
    return relative, redacted.encode("utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    entries = [collect(root, source) for source in args.source]
    entries.sort(key=lambda item: item[0])
    if len({name for name, _ in entries}) != len(entries):
        raise SystemExit("duplicate diagnostic source")
    total = sum(len(data) for _, data in entries)
    if total > MAX_BUNDLE_BYTES:
        raise SystemExit("diagnostic bundle exceeds 16 MiB")
    manifest = {
        "schema": "wovenhat.diagnostic-bundle/v1",
        "redaction": "line-oriented credential/key/token filtering",
        "entries": [
            {"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            for name, data in entries
        ],
    }
    manifest_bytes = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode("utf-8")
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", ZIP_DEFLATED) as bundle:
        for name, data in entries:
            info = ZipInfo(name)
            info.date_time = (1980, 1, 1, 0, 0, 0)
            info.compress_type = ZIP_DEFLATED
            bundle.writestr(info, data)
        info = ZipInfo("manifest.json")
        info.date_time = (1980, 1, 1, 0, 0, 0)
        info.compress_type = ZIP_DEFLATED
        bundle.writestr(info, manifest_bytes)
    if output.stat().st_size > MAX_BUNDLE_BYTES:
        output.unlink()
        raise SystemExit("compressed diagnostic bundle exceeds 16 MiB")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
