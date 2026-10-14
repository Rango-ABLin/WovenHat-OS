"""Build and verify the WovenHat multicore foundation release; fail on any gate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu')
    parser.add_argument('--firmware')
    parser.add_argument('--firmware-vars')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    out = root / 'target' / 'release-validation'
    out.mkdir(parents=True, exist_ok=True)
    report = []

    def run(name, command, run_cwd=root):
        print(f'[{name}]', flush=True)
        start = time.monotonic()
        flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
        result = subprocess.run(command, cwd=run_cwd, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, creationflags=flags)
        (out / f'{name}.log').write_text(result.stdout, encoding='utf-8')
        print(result.stdout, flush=True)
        report.append(dict(check=name, passed=result.returncode == 0,
                           seconds=round(time.monotonic() - start, 2)))
        (out / 'results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        if result.returncode:
            raise SystemExit(result.returncode)

    run('lint-kernel', ['cargo', 'clippy', '-p', 'wovenhat-kernel', '--target', 'x86_64-unknown-none', '--', '-D', 'warnings'])
    run('lint-host', ['cargo', 'clippy', '-p', 'wovenhat-os', '--', '-D', 'warnings'])
    # Cargo resolves dev dependencies in the configured build directory. Do
    # not guess an rlib location or select a stale hash from another build.
    for source in sorted((root / 'tests').glob('*.rs')):
        run(f'compile-{source.stem}', ['cargo', 'test', '--test', source.stem, '--no-run'])
        run(f'test-{source.stem}', ['cargo', 'test', '--test', source.stem])
    options = []
    for key in ('qemu', 'firmware', 'firmware_vars'):
        if getattr(args, key):
            options.extend([f'--{key.replace("_", "-")}', getattr(args, key)])
    for cpus in (1, 2, 4):
        for suite in ('memory', 'storage'):
            run(f'{suite}-{cpus}-debug', [sys.executable, f'scripts/test-{suite}-qemu.py', '--cpus', str(cpus)] + options)
        run(f'network-{cpus}-debug', [sys.executable, 'scripts/test-network-qemu.py', '--cpus', str(cpus)] + options)
    run('legacy-pic', [sys.executable, 'scripts/test-memory-qemu.py', '--cpus', '1', '--legacy-irq'] + options)
    for suite in ('memory', 'storage', 'network'):
        run(f'{suite}-4-release', [sys.executable, f'scripts/test-{suite}-qemu.py', '--cpus', '4', '--release'] + options)
    run('build-release', ['cargo', 'build', '--release'])
    run('shell-smoke', [sys.executable, 'scripts/test-shell-qemu.py'] + options)
    app_directories = ('woven-calculator', 'woven-terminal', 'woven-files', 'woven-monitor')
    for app in app_directories:
        run(
            f'lint-app-{app}',
            ['cargo', 'clippy', '--release', '--target', 'x86_64-unknown-none', '--', '-D', 'warnings'],
            root / 'apps' / app,
        )
    for cpus in (1, 2, 4):
        run(f'monitor-{cpus}', [sys.executable, 'scripts/test-monitor-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(
            f'system-app-catalog-{cpus}',
            [sys.executable, 'scripts/test-system-app-catalog-qemu.py', '--cpus', str(cpus)] + options,
        )
    run('system-apps-boot', [sys.executable, 'scripts/test-shell-qemu.py', '--system-apps'] + options)
    system_app_output = root / 'target' / 'system-apps' / '0.8.0'
    system_app_manifest = json.loads((system_app_output / 'manifest.json').read_text(encoding='utf-8'))
    system_app_env = os.environ.copy()
    system_app_env.setdefault(
        'WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX',
        'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a',
    )
    for item in system_app_manifest['apps']:
        variable = {
            'woven-calculator': 'WOVENHAT_CALCULATOR_IMAGE',
            'woven-terminal': 'WOVENHAT_TERMINAL_IMAGE',
            'woven-files': 'WOVENHAT_FILES_IMAGE',
            'woven-monitor': 'WOVENHAT_MONITOR_IMAGE',
        }[item['id']]
        system_app_env[variable] = str(system_app_output / item['package'])
    image = Path(subprocess.check_output(
        ['cargo', 'run', '--quiet', '--release', '--features', 'system-apps', '--', '--print-image'],
        cwd=root, env=system_app_env, text=True).strip())
    sources = [root / name for name in ('Cargo.toml', 'Cargo.lock', 'kernel/Cargo.toml', 'build.rs', 'rust-toolchain.toml', '.cargo/config.toml')]
    sources += sorted((root / 'kernel' / 'src').rglob('*.rs'))
    sources += sorted((root / 'kernel' / 'src').rglob('*.S'))
    sources += sorted((root / 'src').rglob('*.rs'))
    sources += sorted((root / 'libwoven' / 'src').rglob('*.rs'))
    sources += sorted((root / 'tools' / 'wapp-packager').rglob('*.rs'))
    for app in app_directories:
        app_root = root / 'apps' / app
        sources += sorted(path for path in app_root.rglob('*') if path.is_file() and path.suffix in {'.rs', '.toml', '.x', '.S'})
    sources += sorted((root / 'scripts').glob('*.py'))
    manifest = dict(image=str(image), image_sha256=hashlib.sha256(image.read_bytes()).hexdigest(),
                    checks_sha256=hashlib.sha256((out / 'results.json').read_bytes()).hexdigest(),
                    sources={str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources})
    (out / 'validated-image.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print('All release gates passed. Report:', out / 'results.json')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
