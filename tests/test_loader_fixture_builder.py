"""The fixture builder must preserve evidence and never publish failed output."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    'loader_fixture_builder',
    Path(__file__).resolve().parents[1] / 'scripts/build-loader-fixtures.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class LoaderFixtureBuilderTests(unittest.TestCase):
    def test_existing_output_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            sentinel = output / 'manifest.json'
            sentinel.write_text('earlier evidence', encoding='utf-8')
            with patch.object(builder.subprocess, 'run') as run:
                with self.assertRaises(FileExistsError):
                    builder.build(output)
                run.assert_not_called()
            self.assertEqual(sentinel.read_text(encoding='utf-8'), 'earlier evidence')

    def test_failed_command_is_retained_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'failed'
            failure = subprocess.CompletedProcess(['rustc', '-vV'], 7, 'partial output', 'compiler error')
            with patch.object(builder.subprocess, 'run', return_value=failure):
                with self.assertRaisesRegex(RuntimeError, 'command failed'):
                    builder.build(output)
            record = json.loads((output / 'commands.json').read_text(encoding='utf-8'))
            self.assertEqual(len(record), 1)
            self.assertEqual(record[0]['exit_code'], 7)
            self.assertEqual(record[0]['stdout'], 'partial output')
            self.assertEqual(record[0]['stderr'], 'compiler error')
            self.assertFalse((output / 'manifest.json').exists())

    def test_missing_llvm_tools_fail_before_compilation(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'missing-tools'
            responses = [
                subprocess.CompletedProcess([], 0, 'host: test-host\n', ''),
                subprocess.CompletedProcess([], 0, str(Path(directory) / 'empty-sysroot'), ''),
            ]
            with patch.object(builder.subprocess, 'run', side_effect=responses) as run:
                with self.assertRaisesRegex(RuntimeError, 'missing bundled LLVM tool'):
                    builder.build(output)
                self.assertEqual(run.call_count, 2)
            self.assertFalse((output / 'manifest.json').exists())
            self.assertTrue((output / 'commands.json').is_file())


if __name__ == '__main__':
    unittest.main()
