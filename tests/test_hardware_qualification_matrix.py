#!/usr/bin/env python3
"""Unit tests for the bounded hardware qualification matrix runner."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER_PATH = ROOT / "scripts" / "run-hardware-qualification-matrix.py"
MATRIX_PATH = ROOT / "tests" / "hardware-matrix.json"


def load_runner():
    spec = importlib.util.spec_from_file_location("wovenhat_hardware_matrix_runner", RUNNER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load matrix runner")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load_runner()


class HardwareQualificationMatrixTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))

    def test_plan_determinism(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            first = runner.build_plan(self.matrix, 1234, [4, 1, 4], [4096, 256], None, output)
            second = runner.build_plan(self.matrix, 1234, [4, 1, 4], [4096, 256], None, output)
            self.assertEqual(first, second)
            self.assertEqual([job["index"] for job in first["jobs"]], list(range(first["job_count"])))

    def test_default_profile_cpu_ram_expansion(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            plan = runner.build_plan(self.matrix, 1, None, None, None, Path(directory))
        expected = len(self.matrix["profiles"]) * len(self.matrix["cpu_counts"]) * len(self.matrix["ram_mib"])
        self.assertEqual(plan["job_count"], expected)
        self.assertEqual(plan["jobs"][0]["profile"], self.matrix["profiles"][0]["id"])
        self.assertEqual(sorted({job["cpu_count"] for job in plan["jobs"]}), self.matrix["cpu_counts"])
        self.assertEqual(sorted({job["ram_mib"] for job in plan["jobs"]}), self.matrix["ram_mib"])

    def test_plan_only_writes_only_inside_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "matrix"
            outside = Path(directory) / "outside"
            code = runner.run(["--plan-only", "--profile", "graphics.framebuffer", "--cpus", "1", "--ram-mib", "256", "--output-dir", str(output)])
            self.assertEqual(code, 0)
            self.assertEqual(sorted(path.relative_to(output).as_posix() for path in output.rglob("*")), ["plan.json", "summary.json"])
            self.assertFalse(outside.exists())
            self.assertEqual(json.loads((output / "summary.json").read_text(encoding="utf-8"))["status"], "NOT_RUN")

    def test_physical_arguments_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "should-not-exist"
            for option in ("--native", "--tier1", "--raw-device", "--physical-device"):
                with self.subTest(option=option):
                    with mock.patch.object(runner.sys, "argv", [str(RUNNER_PATH), option, "x", "--output-dir", str(output)]):
                        self.assertEqual(runner.main(), 2)
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
