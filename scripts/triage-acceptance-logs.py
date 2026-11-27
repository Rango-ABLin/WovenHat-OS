#!/usr/bin/env python3
"""Summarize WovenHat's acceptance failures from retained CI/QEMU logs.

Usage: python3 scripts/triage-acceptance-logs.py audit-artifacts/ target/
This is diagnostic only: it never changes acceptance results or claims closure.
"""
import argparse
from pathlib import Path
import re
import sys

FAILURE = re.compile(r"(?P<marker>\[S\d+(?:\.\d+[A-Za-z]?)?\])[^\r\n]*\b(?:FAIL|FAILED)\b", re.I)
MISSING = re.compile(r"Missing marker:\s*(.+)")
FATAL = re.compile(r"\b(?:kernel panic|panicked at|RuntimeError:|Traceback \(most recent call last\))\b", re.I)
SUFFIXES = {".log", ".txt"}

def scan_file(path: Path):
    try:
        if path.stat().st_size > 32 * 1024 * 1024:
            return []
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except (OSError, UnicodeError):
        return []
    findings = []
    for number, line in enumerate(lines, 1):
        if FAILURE.search(line) or MISSING.search(line) or FATAL.search(line):
            findings.append((number, line.strip()[:500]))
    return findings

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args(argv)
    seen = set()
    count = 0
    for path in args.paths:
        candidates = (p for p in path.rglob("*") if p.is_file() and p.suffix in SUFFIXES) if path.is_dir() else (path,)
        for candidate in candidates:
            if candidate in seen or not candidate.is_file():
                continue
            seen.add(candidate)
            for number, line in scan_file(candidate):
                print(f"{candidate}:{number}: {line}")
                count += 1
    print(f"Diagnostic findings: {count}; results are not acceptance evidence.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
