"""Build and verify the WovenHat multicore foundation release; fail on any gate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def build_woven_shell(root):
    """Build WovenShell, the desktop session a system-apps image embeds
    (Stage 20.15), in its own target directory; returns the image path."""
    app_dir = root / 'apps' / 'woven-shell'
    env = os.environ.copy()
    env.pop('CARGO_BUILD_BUILD_DIR', None)
    env['CARGO_TARGET_DIR'] = str(app_dir / 'target')
    subprocess.run(['cargo', 'build', '--release'], cwd=app_dir, env=env, check=True)
    return app_dir / 'target' / 'x86_64-unknown-none' / 'release' / 'wovenhat-woven-shell'


def resolve_fault_gate_firmware(args):
    """Return QEMU/OVMF inputs required by the storage-fault children.

    The release chain must reject a missing OVMF pair before running dozens of
    unrelated checks.  CI supplies both paths explicitly; local runs may use
    the conventional QEMU installation layout.
    """
    explicit_qemu = getattr(args, 'qemu', None)
    qemu = explicit_qemu or shutil.which('qemu-system-x86_64')
    firmware = getattr(args, 'firmware', None)
    firmware_vars = getattr(args, 'firmware_vars', None)
    if explicit_qemu and not Path(explicit_qemu).is_file():
        raise SystemExit(
            f'Release acceptance requires the supplied QEMU executable: {explicit_qemu}'
        )
    if not firmware and qemu:
        qemu_path = Path(qemu)
        candidate = qemu_path.parent / 'share' / 'edk2-x86_64-code.fd'
        if candidate.is_file():
            firmware = str(candidate)
    if not qemu:
        raise SystemExit(
            'Release acceptance requires QEMU. Supply --qemu or install '
            'qemu-system-x86_64.'
        )
    if not firmware or not Path(firmware).is_file():
        raise SystemExit(
            'Release acceptance requires an OVMF CODE firmware file for the '
            'storage-fault gates. Supply --firmware (and a matching '
            '--firmware-vars for the full UEFI matrix).'
        )
    if firmware_vars and not Path(firmware_vars).is_file():
        raise SystemExit(
            f'Release acceptance requires the supplied OVMF variables file: {firmware_vars}'
        )
    return qemu, firmware

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu')
    parser.add_argument('--firmware')
    parser.add_argument('--firmware-vars')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fault_qemu, fault_firmware = resolve_fault_gate_firmware(args)
    out = root / 'target' / 'release-validation'
    out.mkdir(parents=True, exist_ok=True)
    report = []
    # Bind every recorded software acceptance gate to the exact source revision.
    # A passing marker from another tree must never be treated as closure proof.
    source_sha = subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=root, text=True
    ).strip()
    if len(source_sha) != 40 or any(c not in '0123456789abcdef' for c in source_sha):
        raise SystemExit('Release acceptance could not identify an exact source SHA')
    # A commit hash does not describe uncommitted tracked source changes.
    # Ignore untracked build outputs, but reject staged and unstaged edits.
    tracked_changes = subprocess.check_output(
        ['git', 'status', '--porcelain=v1', '--untracked-files=no'],
        cwd=root, text=True,
    ).strip()
    if tracked_changes:
        raise SystemExit(
            'Release acceptance requires a clean tracked source tree. '
            'Commit or revert changes before testing: ' + tracked_changes
        )
    # Release acceptance is serialized and must not depend on Cargo's
    # incremental artifact graph. On Windows, a stale/missing incremental
    # object directory can make an otherwise valid image build fail before
    # QEMU starts. Keep the documented project-local target tree, but make the
    # acceptance chain deterministic and safe to rerun.
    acceptance_env = os.environ.copy()
    acceptance_env['CARGO_INCREMENTAL'] = '0'

    def run(name, command, run_cwd=root, env=None):
        print(f'[{name}]', flush=True)
        start = time.monotonic()
        flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
        result = subprocess.run(command, cwd=run_cwd, env=env or acceptance_env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                creationflags=flags)
        log_path = out / f'{name}.log'
        log_bytes = result.stdout.encode('utf-8')
        log_path.write_bytes(log_bytes)
        print(result.stdout, flush=True)
        report.append(dict(check=name, passed=result.returncode == 0,
                           source_sha=source_sha, command=command,
                           exit_code=result.returncode,
                           log_path=str(log_path.relative_to(root)),
                           log_sha256=hashlib.sha256(log_bytes).hexdigest(),
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
    fault_options = []
    fault_options.extend(['--qemu', fault_qemu, '--firmware', fault_firmware])
    for cpus in (1, 2, 4):
        for suite in ('memory', 'storage'):
            run(f'{suite}-{cpus}-debug', [sys.executable, f'scripts/test-{suite}-qemu.py', '--cpus', str(cpus)] + options)
        run(
            f'storage-read-fault-{cpus}',
            [sys.executable, 'scripts/test-stage31-storage-read-fault.py', '--cpus', str(cpus)] + fault_options,
        )
        run(
            f'storage-write-fault-{cpus}',
            [sys.executable, 'scripts/test-stage31-storage-write-fault.py', '--cpus', str(cpus)] + fault_options,
        )
        run(f'network-{cpus}-debug', [sys.executable, 'scripts/test-network-qemu.py', '--cpus', str(cpus)] + options)
    run('legacy-pic', [sys.executable, 'scripts/test-memory-qemu.py', '--cpus', '1', '--legacy-irq'] + options)
    for suite in ('memory', 'storage', 'network'):
        run(f'{suite}-4-release', [sys.executable, f'scripts/test-{suite}-qemu.py', '--cpus', '4', '--release'] + options)
    run('build-release', ['cargo', 'build', '--release'])
    run('shell-smoke', [sys.executable, 'scripts/test-shell-qemu.py'] + options)
    for cpus in (1, 2, 4):
        run(
            f'service-supervisor-{cpus}',
            [
                sys.executable,
                'scripts/test-shell-qemu.py',
                '--supervisor-test',
                '--cpus',
                str(cpus),
            ]
            + options,
        )
    for cpus in (1, 2, 4):
        run(
            f'driver-restart-{cpus}',
            [
                sys.executable,
                'scripts/test-stage13-12-drivers.py',
                '--mode',
                'restart',
                '--cpus',
                str(cpus),
            ]
            + options,
        )
    app_directories = ('woven-calculator', 'woven-terminal', 'woven-files', 'woven-monitor', 'woven-settings', 'woven-security', 'woven-editor', 'woven-images', 'woven-update', 'woven-store', 'woven-installer', 'woven-package', 'woven-ai')
    for app in app_directories:
        run(
            f'lint-app-{app}',
            ['cargo', 'clippy', '--release', '--target', 'x86_64-unknown-none', '--', '-D', 'warnings'],
            root / 'apps' / app,
        )
    for cpus in (1, 2, 4):
        run(f'monitor-{cpus}', [sys.executable, 'scripts/test-monitor-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'settings-{cpus}', [sys.executable, 'scripts/test-settings-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'security-{cpus}', [sys.executable, 'scripts/test-security-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'editor-{cpus}', [sys.executable, 'scripts/test-editor-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'images-{cpus}', [sys.executable, 'scripts/test-images-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'update-{cpus}', [sys.executable, 'scripts/test-update-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(f'store-{cpus}', [sys.executable, 'scripts/test-store-qemu.py', '--cpus', str(cpus)] + options)
    for cpus in (1, 2, 4):
        run(
            f'system-app-catalog-{cpus}',
            [sys.executable, 'scripts/test-system-app-catalog-qemu.py', '--cpus', str(cpus)] + options,
        )
    transaction_options = []
    for key in ('qemu', 'firmware'):
        if getattr(args, key):
            transaction_options.extend([f'--{key}', getattr(args, key)])
    for cpus in (1, 2, 4):
        run(
            f'woven-package-transactions-{cpus}',
            [
                sys.executable,
                'scripts/test-stage22-5-package-transactions.py',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-broker-{cpus}',
            [sys.executable, 'scripts/test-stage23-1-ai-broker.py', '--cpus', str(cpus)]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-local-provider-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '2',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-cloud-provider-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '3',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-capability-security-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '4',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-semantic-search-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '5',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-assistant-actions-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '6',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    for cpus in (1, 2, 4):
        run(
            f'woven-ai-security-monitor-{cpus}',
            [
                sys.executable,
                'scripts/test-stage23-1-ai-broker.py',
                '--stage',
                '7',
                '--cpus',
                str(cpus),
            ]
            + transaction_options,
        )
    # Normal system-apps boot is a separate release path from the Stage 20.15
    # instrumented fixture; qualify session start and F12 recovery on every
    # supported CPU count instead of relying on the default 4-CPU smoke.
    for cpus in (1, 2, 4):
        run(f'system-apps-boot-{cpus}', [
            sys.executable, 'scripts/test-shell-qemu.py',
            '--system-apps', '--cpus', str(cpus),
        ] + options)
    system_app_output = root / 'target' / 'system-apps' / '0.8.0'
    system_app_manifest = json.loads((system_app_output / 'manifest.json').read_text(encoding='utf-8'))
    system_app_env = acceptance_env.copy()
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
            'woven-settings': 'WOVENHAT_SETTINGS_IMAGE',
            'woven-security': 'WOVENHAT_SECURITY_IMAGE',
            'woven-editor': 'WOVENHAT_EDITOR_IMAGE',
            'woven-images': 'WOVENHAT_IMAGES_IMAGE',
            'woven-update': 'WOVENHAT_UPDATE_IMAGE',
            'woven-store': 'WOVENHAT_STORE_IMAGE',
            'woven-installer': 'WOVENHAT_INSTALLER_IMAGE',
        }[item['id']]
        system_app_env[variable] = str(system_app_output / item['package'])
    system_app_env['WOVENHAT_SHELL_IMAGE'] = str(build_woven_shell(root))
    system_app_env['WOVENHAT_PACKAGE_IMAGE'] = str(Path(subprocess.check_output(
        [sys.executable, 'scripts/build-package-service.py'],
        cwd=root,
        env=system_app_env,
        text=True,
    ).strip()))
    system_app_env['WOVENHAT_SEARCH_IMAGE'] = str(Path(subprocess.check_output(
        [sys.executable, 'scripts/build-search-service.py'],
        cwd=root,
        env=system_app_env,
        text=True,
    ).strip()))
    run(
        'lint-kernel-system-apps',
        [
            'cargo', 'clippy', '-p', 'wovenhat-kernel',
            '--target', 'x86_64-unknown-none', '--features', 'system-apps',
            '--', '-D', 'warnings',
        ],
        env=system_app_env,
    )
    image = Path(subprocess.check_output(
        ['cargo', 'run', '--quiet', '--release', '--features', 'system-apps', '--', '--print-image'],
        cwd=root, env=system_app_env, text=True).strip())
    sources = [root / name for name in ('Cargo.toml', 'Cargo.lock', 'kernel/Cargo.toml', 'build.rs', 'rust-toolchain.toml', '.cargo/config.toml')]
    sources += sorted((root / 'kernel' / 'src').rglob('*.rs'))
    sources += sorted((root / 'kernel' / 'src').rglob('*.S'))
    sources += sorted((root / 'src').rglob('*.rs'))
    sources += sorted((root / 'libwoven' / 'src').rglob('*.rs'))
    sources += sorted((root / 'tools' / 'wapp-packager').rglob('*.rs'))
    for app in app_directories + ('woven-shell',):
        app_root = root / 'apps' / app
        sources += sorted(path for path in app_root.rglob('*') if path.is_file() and path.suffix in {'.rs', '.toml', '.x', '.S'})
    sources += sorted((root / 'scripts').glob('*.py'))
    # Recheck the source after all builds: tests against a moving tree are not
    # evidence for the initial commit, even when every individual gate passed.
    final_sha = subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=root, text=True
    ).strip()
    final_changes = subprocess.check_output(
        ['git', 'status', '--porcelain=v1', '--untracked-files=no'],
        cwd=root, text=True,
    ).strip()
    if final_sha != source_sha or final_changes:
        raise SystemExit(
            'Source revision or tracked files changed during acceptance; '
            'discard the release result and rerun from a clean immutable tree.'
        )
    manifest = dict(source_sha=source_sha, image=str(image), image_sha256=hashlib.sha256(image.read_bytes()).hexdigest(),
                    checks_sha256=hashlib.sha256((out / 'results.json').read_bytes()).hexdigest(),
                    sources={str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources})
    (out / 'validated-image.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print('All release gates passed. Report:', out / 'results.json')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
