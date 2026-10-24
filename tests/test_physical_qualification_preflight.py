#!/usr/bin/env python3
"""Contract tests for the read-only physical qualification preflight."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "physical-qualification-preflight.py"
SPEC = importlib.util.spec_from_file_location("physical_qualification_preflight", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load physical qualification preflight")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PhysicalQualificationPreflightTests(unittest.TestCase):
    def test_schema_shape_and_category_order_are_deterministic(self) -> None:
        first = MODULE.build_report()
        second = MODULE.build_report()
        self.assertEqual(list(first), list(second))
        self.assertEqual(first["required_categories"], list(MODULE.CATEGORIES))
        self.assertEqual(first["qualification"], second["qualification"])
        self.assertEqual({item["category"] for item in first["qualification"]}, set(MODULE.CATEGORIES))

    def test_no_native_claim_policy(self) -> None:
        report = MODULE.build_report()
        self.assertFalse(report["native_evidence"]["supplied"])
        self.assertFalse(report["native_evidence"]["accepted"])
        self.assertTrue(all(item["status"] in {"BLOCKED", "PHYSICAL_NOT_TESTED"} for item in report["qualification"]))
        self.assertTrue(all(item["native_claim"] is False for item in report["qualification"]))
        self.assertFalse(report["safety"]["physical_device_claim"])

    def test_output_path_safety(self) -> None:
        unsafe = (r"\\.\PhysicalDrive0", r"/dev/sda", r"/dev/nvme0n1", r"C:\raw-device\report")
        for path in unsafe:
            with self.subTest(path=path):
                self.assertFalse(MODULE.is_safe_output_path(path))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "preflight"
            self.assertEqual(MODULE.run(["--output-dir", str(output), "--json-only"]), 0)
            report = json.loads((output / "report.json").read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], MODULE.SCHEMA)


if __name__ == "__main__":
    unittest.main()
