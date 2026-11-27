"""Package only the image and source state recorded by the successful release gate."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

def require_validated_source(root, manifest):
    source_commit = manifest.get('source_commit')
    if not isinstance(source_commit, str) or len(source_commit) != 40:
        raise ValueError('Validation manifest has no exact source commit; rerun the release gate.')
    current_commit = subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=root, text=True
    ).strip()
    if current_commit != source_commit:
        raise ValueError('Source commit changed after validation; rerun the release gate.')
    status = subprocess.check_output(
        ['git', 'status', '--porcelain', '--untracked-files=all'], cwd=root, text=True
    )
    if status:
        raise ValueError('Refusing to package a release from a dirty working tree.')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64')
                        or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    validation = root / 'target' / 'release-validation'
    manifest = json.loads((validation / 'validated-image.json').read_text())
    report = validation / 'results.json'
    checks = json.loads(report.read_text())
    apps = ('woven-calculator', 'woven-terminal', 'woven-files', 'woven-monitor',
            'woven-settings', 'woven-security', 'woven-editor', 'woven-images',
            'woven-update', 'woven-store', 'woven-installer')
    required = manifest.get('required_checks')
    if not isinstance(required, list) or not required or len(required) != len(set(required)):
        raise SystemExit('Validation manifest has no complete required check list.')
    if {item.get('check') for item in checks} != set(required) or len(checks) != len(required):
        raise SystemExit('Release report does not match the validated gate set.')
    if not all(item.get('passed') is True for item in checks):
        raise SystemExit('Release gates are incomplete or failed.')
    try:
        require_validated_source(root, manifest)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    if sha(report) != manifest['checks_sha256']:
        raise SystemExit('Validation report changed; rerun the release gate.')
    current_sources = [root / name for name in ('Cargo.toml', 'Cargo.lock', 'kernel/Cargo.toml', 'build.rs', 'rust-toolchain.toml', '.cargo/config.toml')]
    current_sources += list((root / 'kernel' / 'src').rglob('*.rs')) + list((root / 'kernel' / 'src').rglob('*.S'))
    current_sources += list((root / 'src').rglob('*.rs'))
    current_sources += list((root / 'libwoven' / 'src').rglob('*.rs'))
    current_sources += list((root / 'tools' / 'wapp-packager').rglob('*.rs'))
    for app in apps + ('woven-package', 'woven-ai', 'woven-shell'):
        app_root = root / 'apps' / app
        current_sources += [path for path in app_root.rglob('*') if path.is_file() and path.suffix in {'.rs', '.toml', '.x', '.S'}]
    current_sources += list((root / 'scripts').glob('*.py'))
    if {str(path.relative_to(root)) for path in current_sources} != set(manifest['sources']):
        raise SystemExit('Source file inventory changed; rerun the release gate.')
    for name, digest in manifest['sources'].items():
        if sha(root / name) != digest:
            raise SystemExit(f'Source changed after validation: {name}')
    image = Path(manifest['image'])
    if sha(image) != manifest['image_sha256']:
        raise SystemExit('Image changed after validation; rerun the release gate.')
    out = root / 'target' / 'releases' / '0.8.0'
    out.mkdir(parents=True, exist_ok=True)
    packaged = out / 'wovenhat-os-0.8.0-uefi.img'
    shutil.copyfile(image, packaged)
    iso_builder_spec = importlib.util.spec_from_file_location(
        'wovenhat_uefi_iso', root / 'scripts' / 'build-uefi-iso.py')
    if iso_builder_spec is None or iso_builder_spec.loader is None:
        raise SystemExit('Unable to load the Stage 28 UEFI ISO builder.')
    iso_builder = importlib.util.module_from_spec(iso_builder_spec)
    iso_builder_spec.loader.exec_module(iso_builder)
    packaged_iso = out / 'wovenhat-os-0.8.0-uefi.iso'
    iso_builder.build_iso(packaged, packaged_iso, 'WOVENHAT')
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file():
        raise SystemExit('Set --qemu and --firmware to regular QEMU/OVMF paths for ISO acceptance.')
    subprocess.run([
        sys.executable, str(root / 'scripts' / 'test-uefi-iso-qemu.py'),
        '--iso', str(packaged_iso),
        '--firmware', str(firmware),
        '--qemu', str(qemu),
        '--serial-log', str(out / 'iso-boot-serial.log'),
    ], cwd=root, check=True)
    shutil.copyfile(root / 'docs' / 'release-0.8.0.md', out / 'RELEASE-NOTES.md')
    shutil.copyfile(report, out / 'results.json')
    system_apps = root / 'target' / 'system-apps' / '0.8.0'
    subprocess.run([sys.executable, str(root / 'scripts' / 'package-system-apps.py'),
                    '--version', '0.8.0', '--production'], cwd=root, check=True)
    release_apps = out / 'system-apps'
    if release_apps.exists():
        shutil.rmtree(release_apps)
    shutil.copytree(system_apps, release_apps)
    manifest['image'] = packaged.name
    manifest['iso'] = packaged_iso.name
    manifest['iso_sha256'] = sha(packaged_iso)
    manifest['base_commit'] = manifest['source_commit']
    manifest['working_tree_dirty'] = False
    manifest['system_apps_manifest'] = 'system-apps/manifest.json'
    manifest['system_apps_manifest_sha256'] = sha(release_apps / 'manifest.json')
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    release_files = [packaged, packaged_iso, out / 'RELEASE-NOTES.md', out / 'results.json', out / 'manifest.json']
    release_files += sorted(path for path in release_apps.rglob('*') if path.is_file())
    (out / 'SHA256SUMS').write_text(
        ''.join(f"{sha(path)}  {path.relative_to(out).as_posix()}\n" for path in release_files),
        encoding='utf-8',
    )
    try:
        require_validated_source(root, manifest)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    archive = out / 'wovenhat-os-0.8.0.zip'
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as bundle:
        for path in release_files + [out / 'SHA256SUMS']:
            bundle.write(path, path.relative_to(out).as_posix())
        for log in sorted(validation.glob('*.log')):
            bundle.write(log, 'validation/' + log.name)
    print('Packaged validated release:', archive)
    print('UEFI image:', packaged)
    print('SHA256:', sha(packaged))


if __name__ == '__main__':
    main()
