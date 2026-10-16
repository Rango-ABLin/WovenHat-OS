"""Verify the real IPC probe's evidence-directory creation in a fresh checkout."""
import ast
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/test-ipc-handle-generation.py"


class IpcProbeEvidenceTests(unittest.TestCase):
    def test_evidence_directory_creation_is_nested_and_exclusive(self):
        tree = ast.parse(SCRIPT.read_text(encoding="utf-8"))
        main = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "main")
        calls = [
            node for node in ast.walk(main)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "output"
            and node.func.attr == "mkdir"
        ]
        self.assertEqual(len(calls), 1, "Expected exactly one IPC evidence mkdir call")
        call = calls[0]
        self.assertEqual(len(call.args), 0)
        kwargs = {keyword.arg: keyword.value for keyword in call.keywords}
        self.assertIsInstance(kwargs.get("parents"), ast.Constant)
        self.assertIs(kwargs["parents"].value, True)
        self.assertNotIn("exist_ok", kwargs, "Evidence directories must not silently reuse prior runs")
        statement = ast.fix_missing_locations(ast.Expression(body=call))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "audit-artifacts" / "ipc-generation-probe-123"
            eval(compile(statement, str(SCRIPT), "eval"), {"output": output})
            self.assertTrue(output.is_dir())
            with self.assertRaises(FileExistsError):
                eval(compile(statement, str(SCRIPT), "eval"), {"output": output})


if __name__ == "__main__":
    unittest.main()
