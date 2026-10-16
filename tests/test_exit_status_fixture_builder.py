"""The Ring-3 status fixture must retain failures and reject ambiguous evidence."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'exit_status_builder', Path(__file__).resolve().parents[1] / 'scripts/build-exit-status-fixture.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class ExitStatusBuilderTests(unittest.TestCase):
    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            sentinel = output / 'manifest.json'
            sentinel.write_text('retained', encoding='utf-8')
            with patch.object(builder.subprocess, 'run') as run:
                with self.assertRaises(FileExistsError):
                    builder.build(output)
                run.assert_not_called()
            self.assertEqual(sentinel.read_text(encoding='utf-8'), 'retained')

    def test_failed_compile_retains_logs_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'new'
            replies = [subprocess.CompletedProcess([], 0, 'compiler version', ''),
                       subprocess.CompletedProcess([], 1, 'failed stdout', 'failed stderr')]
            with patch.object(builder.subprocess, 'run', side_effect=replies):
                with self.assertRaisesRegex(RuntimeError, 'Fixture build failed'):
                    builder.build(output)
            self.assertEqual((output / 'build.stderr').read_text(), 'failed stderr')
            self.assertFalse((output / 'manifest.json').exists())

    def test_source_change_prevents_manifest_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            binary = base / 'compiled.elf'
            binary.write_bytes(b'compiler output')
            record = {'reason': 'compiler-artifact',
                      'target': {'name': 'wovenhat-exit-status-fixture'},
                      'executable': str(binary)}
            replies = [subprocess.CompletedProcess([], 0, 'compiler version', ''),
                       subprocess.CompletedProcess([], 0, json.dumps(record), '')]
            output = base / 'new'
            with patch.object(builder.subprocess, 'run', side_effect=replies), \
                    patch.object(builder, 'snapshot_inputs', side_effect=[{'input': 'before'}, {'input': 'after'}]):
                with self.assertRaisesRegex(RuntimeError, 'inputs changed'):
                    builder.build(output)
            self.assertFalse((output / 'manifest.json').exists())


if __name__ == '__main__':
    unittest.main()
