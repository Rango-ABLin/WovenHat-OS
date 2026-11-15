"""Exercise the actual bootloader build script with a recording Cargo child."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BootloaderBuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.directory = Path(cls.temporary.name)
        suffix = '.exe' if os.name == 'nt' else ''
        cls.builder = cls.directory / ('builder' + suffix)
        cls.cargo = cls.directory / ('record-cargo' + suffix)
        environment = dict(os.environ, CARGO_PKG_VERSION='0.11.17')
        subprocess.run([
            'rustc', '--edition=2024', '--cfg', 'feature="uefi"',
            str(ROOT / 'vendor/bootloader/build.rs'), '-o', str(cls.builder),
        ], env=environment, check=True, capture_output=True)
        recorder = cls.directory / 'record.rs'
        recorder.write_text(r'''
use std::{env, fs, path::PathBuf};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    fs::write(env::var("RECORD_ARGS").unwrap(), args.join("\n")).unwrap();
    fs::write(env::var("RECORD_BUILD_DIR").unwrap(),
        env::var("CARGO_BUILD_BUILD_DIR").unwrap()).unwrap();
    let root = args.windows(2).find(|pair| pair[0] == "--root").unwrap();
    let bin = PathBuf::from(&root[1]).join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("bootloader-x86_64-uefi.efi"), b"test").unwrap();
}
''', encoding='utf-8')
        subprocess.run(['rustc', str(recorder), '-o', str(cls.cargo)],
                       check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def check_source(self, mode):
        directory = self.directory / mode
        directory.mkdir()
        if mode != 'published':
            (directory / 'uefi').mkdir()
        if mode == 'local':
            (directory / 'uefi/Cargo.toml').write_text('[package]\n')
        output = directory / 'output'
        record = directory / 'arguments'
        build_record = directory / 'build-directory'
        environment = dict(os.environ, CARGO=str(self.cargo), OUT_DIR=str(output),
                           RECORD_ARGS=str(record), RECORD_BUILD_DIR=str(build_record),
                           CARGO_TARGET_DIR=str(directory / 'parent-target'),
                           CARGO_BUILD_BUILD_DIR=str(directory / 'parent-build'))
        subprocess.run([str(self.builder)], cwd=directory, env=environment,
                       check=True, capture_output=True)
        arguments = record.read_text().splitlines()
        self.assertEqual(arguments.count('--target-dir'), 1)
        self.assertEqual(arguments[arguments.index('--target-dir') + 1],
                         str(output / 'uefi-target'))
        self.assertEqual(build_record.read_text(), str(output / 'uefi-build'))
        self.assertIn('--locked', arguments)
        if mode == 'local':
            self.assertEqual(arguments[arguments.index('--path') + 1], 'uefi')
            self.assertNotIn('--version', arguments)
        else:
            self.assertEqual(arguments[arguments.index('--version') + 1], '0.11.17')
            self.assertNotIn('--path', arguments)

    def test_published_source(self):
        self.check_source('published')

    def test_empty_directory_uses_published_source(self):
        self.check_source('leftover')

    def test_local_manifest_uses_single_isolated_target(self):
        self.check_source('local')


if __name__ == '__main__':
    unittest.main()
