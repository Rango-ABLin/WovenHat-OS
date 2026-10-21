"""Package only the image and source state recorded by the successful release gate."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

root = Path(__file__).resolve().parents[1]
validation = root / 'target' / 'release-validation'
manifest = json.loads((validation / 'validated-image.json').read_text())
report = validation / 'results.json'
checks = json.loads(report.read_text())
required = {'lint-kernel', 'lint-host', 'legacy-pic', 'build-release', 'shell-smoke',
            'test-buffer_cache', 'test-file_mapping', 'test-page_cache'}
required |= {f'{suite}-{cpus}-debug' for suite in ('memory', 'storage') for cpus in (1, 2, 4)}
required |= {f'{suite}-4-release' for suite in ('memory', 'storage')}
apps = ('woven-calculator', 'woven-terminal', 'woven-files', 'woven-monitor',
        'woven-settings', 'woven-security', 'woven-editor', 'woven-images',
        'woven-update', 'woven-store')
required |= {f'lint-app-{app}' for app in apps}
required |= {f'{app.removeprefix("woven-")}-{cpus}'
             for app in apps if app not in ('woven-calculator', 'woven-terminal', 'woven-files')
             for cpus in (1, 2, 4)}
required |= {f'system-app-catalog-{cpus}' for cpus in (1, 2, 4)}
required.add('system-apps-boot')
if not required <= {item['check'] for item in checks} or not all(item['passed'] for item in checks):
    raise SystemExit('Release gates are incomplete or failed.')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
if sha(report) != manifest['checks_sha256']:
    raise SystemExit('Validation report changed; rerun the release gate.')
current_sources = [root / name for name in ('Cargo.toml', 'Cargo.lock', 'kernel/Cargo.toml', 'build.rs', 'rust-toolchain.toml', '.cargo/config.toml')]
current_sources += list((root / 'kernel' / 'src').rglob('*.rs')) + list((root / 'kernel' / 'src').rglob('*.S'))
current_sources += list((root / 'src').rglob('*.rs'))
current_sources += list((root / 'libwoven' / 'src').rglob('*.rs'))
current_sources += list((root / 'tools' / 'wapp-packager').rglob('*.rs'))
for app in apps:
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
shutil.copyfile(root / 'docs' / 'release-0.8.0.md', out / 'RELEASE-NOTES.md')
shutil.copyfile(report, out / 'results.json')
system_apps = root / 'target' / 'system-apps' / '0.8.0'
if not (system_apps / 'manifest.json').is_file():
    subprocess.run([sys.executable, str(root / 'scripts' / 'package-system-apps.py'),
                    '--version', '0.8.0', '--production'], cwd=root, check=True)
release_apps = out / 'system-apps'
if release_apps.exists():
    shutil.rmtree(release_apps)
shutil.copytree(system_apps, release_apps)
manifest['image'] = packaged.name
manifest['base_commit'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
manifest['working_tree_dirty'] = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=root, text=True).strip())
manifest['system_apps_manifest'] = 'system-apps/manifest.json'
manifest['system_apps_manifest_sha256'] = sha(release_apps / 'manifest.json')
(out / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
release_files = [packaged, out / 'RELEASE-NOTES.md', out / 'results.json', out / 'manifest.json']
release_files += sorted(path for path in release_apps.rglob('*') if path.is_file())
(out / 'SHA256SUMS').write_text(
    ''.join(f"{sha(path)}  {path.relative_to(out).as_posix()}\n" for path in release_files),
    encoding='utf-8',
)
archive = out / 'wovenhat-os-0.8.0.zip'
with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as bundle:
    for path in release_files + [out / 'SHA256SUMS']:
        bundle.write(path, path.relative_to(out).as_posix())
    for log in sorted(validation.glob('*.log')):
        bundle.write(log, 'validation/' + log.name)
print('Packaged validated release:', archive)
print('UEFI image:', packaged)
print('SHA256:', sha(packaged))
