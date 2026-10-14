"""Warning-denying kernel Clippy for every feature in kernel/Cargo.toml, plus
production (no feature) and host --all-targets.

Features that embed application images are linted with whatever images are
provided through WOVENHAT_*_IMAGE / key variables in the environment. A
feature that fails only because such a variable is missing is reported as
`needs-image`, never as a pass. Exit status is non-zero if any lint fails.

  python scripts/lint-all-features.py [--only PREFIX]
"""
import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def features():
    text = (ROOT / 'kernel' / 'Cargo.toml').read_text(encoding='utf-8')
    section = text.split('[features]', 1)[1].split('\n[', 1)[0]
    names = re.findall(r'^([a-z0-9][a-z0-9-]*)\s*=', section, re.M)
    return [n for n in names if n not in ('default',)]


def clippy(args):
    command = ['cargo', 'clippy', '-q', '-p', 'wovenhat-kernel', '--target', 'x86_64-unknown-none', *args,
               '--', '-D', 'warnings']
    r = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    missing = re.findall(r'environment variable `(WOVENHAT_[A-Z_]+)` not defined', r.stderr)
    if r.returncode and missing and not re.search(r'^error(\[E\d+\])?: (?!environment variable|could not compile)', r.stderr, re.M):
        return 'needs-image', sorted(set(missing))
    errors = [l for l in r.stderr.splitlines() if l.startswith('error')][:3]
    return ('ok' if r.returncode == 0 else 'FAIL'), errors


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--only', default='')
    args = parser.parse_args()
    failed = 0
    configs = [('production', [])] + [(f, ['--features', f]) for f in features() if f.startswith(args.only)]
    for name, extra in configs:
        status, detail = clippy(extra)
        failed += status == 'FAIL'
        print(f'{name}: {status} {detail if status != "ok" else ""}'.rstrip(), flush=True)
    r = subprocess.run(['cargo', 'clippy', '-q', '--all-targets', '--', '-D', 'warnings'], cwd=ROOT,
                       capture_output=True, text=True)
    failed += r.returncode != 0
    print(f'host --all-targets: {"ok" if r.returncode == 0 else "FAIL " + r.stderr[-800:]}', flush=True)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
