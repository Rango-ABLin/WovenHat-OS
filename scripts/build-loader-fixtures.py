"""Build unmodified Rust/LLD ELF fixtures for loader interoperability tests.

No Cargo/QEMU process is launched, no production image is rebuilt, and no
signing key is used. Every invocation has its own output directory. A manifest
is published only after all compiler/linker/inspection commands succeed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(output, rustc='rustc'):
    root = Path(__file__).resolve().parent.parent
    source = root / 'tools' / 'loader-fixtures'
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    commands = []
    inputs = ('library.rs', 'shared-main.rs', 'static-tls.rs', 'tls-threads.rs')
    input_hashes = {name: digest(source / name) for name in inputs}
    builder_hash = digest(Path(__file__).resolve())

    def run(command):
        command = [str(item) for item in command]
        print(json.dumps(command), flush=True)
        result = subprocess.run(command, cwd=root, capture_output=True, text=True)
        record = dict(command=command, exit_code=result.returncode,
                      stdout=result.stdout, stderr=result.stderr)
        commands.append(record)
        (output / 'commands.json').write_text(json.dumps(commands, indent=2), encoding='utf-8')
        if result.returncode:
            raise RuntimeError(f'command failed: {command!r}\n{result.stderr}')
        return result.stdout.strip()

    version = run([rustc, '-vV'])
    host = next(line.removeprefix('host: ') for line in version.splitlines()
                if line.startswith('host: '))
    sysroot = Path(run([rustc, '--print', 'sysroot']))
    binary = sysroot / 'lib' / 'rustlib' / host / 'bin'
    suffix = '.exe' if os.name == 'nt' else ''
    linker = binary / f'rust-lld{suffix}'
    readobj = binary / f'llvm-readobj{suffix}'
    objdump = binary / f'llvm-objdump{suffix}'
    for tool in (linker, readobj, objdump):
        if not tool.is_file():
            raise RuntimeError(f'missing bundled LLVM tool: {tool}')

    for name in inputs:
        command = [rustc, '--edition=2024', '--crate-type=lib', '--emit=obj',
                   '--target=x86_64-unknown-none', '-Dwarnings', '-Cpanic=abort',
                   '-Copt-level=2', '-Crelocation-model=pic', '-Ccode-model=large']
        if name in ('static-tls.rs', 'tls-threads.rs'):
            command.append('-Ztls-model=local-exec')
        run(command + [source / name, '-o', output / (Path(name).stem + '.o')])

    library = output / 'libwoven-demo.so'
    shared = output / 'shared-demo'
    tls = output / 'static-tls-demo'
    threads = output / 'tls-threads-demo'
    run([linker, '-flavor', 'gnu', '-shared', '--hash-style=sysv',
         '-soname', library.name, '-z', 'now', '-o', library, output / 'library.o'])
    run([linker, '-flavor', 'gnu', '-pie', '--no-dynamic-linker', '--hash-style=sysv',
         '-z', 'now', '--image-base=0x400000000000', '-e', '_start', '-L', output,
         '-lwoven-demo', '-o', shared, output / 'shared-main.o'])
    run([linker, '-flavor', 'gnu', '-static', '--image-base=0x400000000000',
         '-e', '_start', '-o', tls, output / 'static-tls.o'])
    run([linker, '-flavor', 'gnu', '-static', '--image-base=0x400000000000',
         '-e', '_start', '-o', threads, output / 'tls-threads.o'])
    outputs = (library, shared, tls, threads)
    report = run([readobj, '--file-headers', '--program-headers', '--dynamic-table',
                  '--relocations', '--symbols', *outputs])
    (output / 'elf-report.txt').write_text(report, encoding='utf-8')
    report = run([objdump, '--disassemble', *outputs])
    (output / 'disassembly.txt').write_text(report, encoding='utf-8')
    if input_hashes != {name: digest(source / name) for name in inputs}:
        raise RuntimeError('fixture sources changed during the build; manifest not published')
    if builder_hash != digest(Path(__file__).resolve()):
        raise RuntimeError('fixture builder changed during the build; manifest not published')
    manifest = dict(
        compiler=version, expected_exit=42,
        inputs=input_hashes, builder_sha256=builder_hash,
        tools={path.name: digest(path) for path in (linker, readobj, objdump)},
        outputs={path.name: dict(path=str(path), sha256=digest(path)) for path in outputs},
        commands_sha256=digest(output / 'commands.json'),
    )
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    return output / 'manifest.json'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--rustc', default='rustc')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    output = args.output or root / 'target' / 'loader-fixtures' / f'build-{time.time_ns()}'
    print('Fixture manifest:', build(output, args.rustc))


if __name__ == '__main__':
    main()
