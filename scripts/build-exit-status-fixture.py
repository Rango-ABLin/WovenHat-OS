"""Build the actual libwoven Ring-3 status fixture with retained provenance."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def snapshot_inputs(root):
    app = root / 'tools/exit-status-fixture'
    inputs = [app / 'Cargo.toml', app / 'Cargo.lock', app / '.cargo/config.toml',
              app / 'src/main.rs', root / 'libwoven/src/lib.rs', root / 'libwoven/Cargo.toml',
              root / 'kernel/src/font.rs', root / 'rust-toolchain.toml',
              root / 'apps/hello/link.x', Path(__file__).resolve()]
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in inputs}


def build(output):
    root = Path(__file__).resolve().parents[1]
    app = root / 'tools/exit-status-fixture'
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ)
    env['CARGO_BUILD_BUILD_DIR'] = str(root / '.cargo-build')
    env['CARGO_TARGET_DIR'] = str(root / 'target')
    env['CARGO_INCREMENTAL'] = '0'
    before = snapshot_inputs(root)
    version = subprocess.run([env.get('RUSTC', 'rustc'), '-vV'], cwd=app, env=env,
                             capture_output=True, text=True)
    (output / 'compiler.txt').write_text(version.stdout + version.stderr, encoding='utf-8')
    if version.returncode:
        raise RuntimeError(f'Compiler version check failed; evidence: {output}')
    command = ['cargo', 'build', '--locked', '--release', '--message-format=json-render-diagnostics']
    result = subprocess.run(command, cwd=app, env=env, capture_output=True, text=True)
    (output / 'build.stdout').write_text(result.stdout, encoding='utf-8')
    (output / 'build.stderr').write_text(result.stderr, encoding='utf-8')
    if result.returncode:
        raise RuntimeError(f'Fixture build failed; evidence: {output}\n{result.stderr}')
    candidates = []
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if (record.get('reason') == 'compiler-artifact'
                and record.get('target', {}).get('name') == 'wovenhat-exit-status-fixture'
                and record.get('executable')):
            candidates.append(Path(record['executable']))
    if len(candidates) != 1:
        raise RuntimeError(f'Expected one fixture executable: {candidates}')
    binary = output / 'exit-status.elf'
    binary.write_bytes(candidates[0].read_bytes())
    if before != snapshot_inputs(root):
        raise RuntimeError('Fixture inputs changed during build; manifest not published')
    manifest = {'command': command, 'exit': result.returncode, 'compiler': version.stdout,
                'inputs': before,
                'elf_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    return binary


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[1]
    print(build(root / 'audit-artifacts' / f'exit-status-build-{time.time_ns()}'))
