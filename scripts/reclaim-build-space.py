"""Reclaim disk space from regenerable build output in WovenHat checkouts.

Only build caches are removed:
  - incremental compilation caches under `.cargo-build/` and `target/`;
  - with `--deep`, the whole `.cargo-build/` of a checkout and every
    `apps/*/target/` (rebuilt on the next build).
Never removed: `audit-artifacts/`, gate evidence under `target/<gate>-*cpu/`,
sources, or anything outside the named checkouts. A checkout in which a
cargo, rustc or QEMU process is running is skipped, so a build or gate in
progress (ours or Codex's) is never disturbed.

Usage:
  python scripts/reclaim-build-space.py [--min-free-gb 8] [--deep] CHECKOUT...
With --min-free-gb, nothing is removed while the drive already has that
much free space.
"""
import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


def free_gb(path):
    return shutil.disk_usage(path).free / 2**30


def busy_paths():
    """Command lines of running cargo/rustc/QEMU processes (lower-cased)."""
    if os.name == 'nt':
        command = ['powershell.exe', '-NoProfile', '-Command',
                   "Get-CimInstance Win32_Process | Where-Object { $_.Name -match "
                   "'^(cargo|rustc|qemu-system-x86_64|clippy-driver)' } | "
                   "ForEach-Object { $_.CommandLine }"]
    else:
        command = ['ps', '-eo', 'args']
    output = subprocess.run(command, capture_output=True, text=True).stdout
    return [line.lower().replace('\\', '/') for line in output.splitlines()
            if any(name in line for name in ('cargo', 'rustc', 'qemu-system', 'clippy-driver'))]


def size_of(path):
    total = 0
    for root, _, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(root, name)).st_size
            except OSError:
                pass
    return total


def targets(checkout, deep):
    found = []
    for cache in ('.cargo-build', 'target'):
        base = checkout / cache
        if base.is_dir():
            found += [p for p in base.rglob('incremental') if p.is_dir()]
    if deep:
        if (checkout / '.cargo-build').is_dir():
            found.append(checkout / '.cargo-build')
        found += [p for p in (checkout / 'apps').glob('*/target') if p.is_dir()]
    # Drop paths nested inside another selected path.
    found = sorted(set(found), key=lambda p: len(p.parts))
    kept = []
    for path in found:
        if not any(parent in path.parents for parent in kept):
            kept.append(path)
    return kept


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('checkouts', nargs='+', type=Path)
    parser.add_argument('--min-free-gb', type=float, default=0)
    parser.add_argument('--deep', action='store_true')
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    drive = args.checkouts[0].resolve().anchor
    before = free_gb(drive)
    if args.min_free_gb and before >= args.min_free_gb:
        print(f'{before:.1f} GB free (>= {args.min_free_gb}); nothing to do')
        return 0
    busy = busy_paths()
    for checkout in args.checkouts:
        checkout = checkout.resolve()
        if not (checkout / '.git').exists():
            print(f'skip {checkout}: not a checkout', file=sys.stderr)
            continue
        key = str(checkout).lower().replace('\\', '/')
        if any(key in line for line in busy):
            print(f'skip {checkout}: a build or QEMU is running there')
            continue
        for path in targets(checkout, args.deep):
            size = size_of(path) / 2**30
            print(f'{"would remove" if args.dry_run else "remove"} {path} ({size:.2f} GB)')
            if not args.dry_run:
                shutil.rmtree(path, ignore_errors=True)
    print(f'free: {before:.1f} GB -> {free_gb(drive):.1f} GB')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
