import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "qualify-hardware-report.py"
SPEC = importlib.util.spec_from_file_location("qualify_hardware_report", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def observation(category, *, result="passed", environment="native", tier=None):
    item = {
        "id": f"obs-{category}",
        "category": category,
        "device": f"fixture-{category}",
        "environment": environment,
        "result": result,
        "evidence": f"audit-artifacts/hardware/{category}.json",
    }
    if tier is not None:
        item["tier"] = tier
    return item


def report(observations):
    return {
        "schema": "wovenhat.hardware-qualification/v1",
        "host": {"host_id": "lab-fixture-01", "environment": "native"},
        "observations": observations,
    }


class HardwareQualificationTests(unittest.TestCase):
    def test_partial_native_coverage_is_tier2(self):
        result = MODULE.validate(report([observation("nvme")]))
        self.assertEqual(result, [])
        self.assertEqual(MODULE.classify(report([observation("nvme")]))["tier"], "tier2")

    def test_failed_native_observation_is_tier3(self):
        item = observation("wifi", result="failed")
        self.assertEqual(MODULE.classify(report([item]))["tier"], "tier3")

    def test_qemu_evidence_cannot_claim_tier1(self):
        errors = MODULE.validate(report([observation("nvme", environment="qemu", tier="tier1")]))
        self.assertIn("observations[0].tier1 requires passed native evidence", errors)

    def test_failed_native_evidence_cannot_claim_tier1(self):
        errors = MODULE.validate(report([observation("nvme", result="failed", tier="tier1")]))
        self.assertIn("observations[0].tier1 requires passed native evidence", errors)

    def test_absolute_evidence_path_is_rejected(self):
        item = observation("usb")
        item["evidence"] = "C:/outside/report.json"
        self.assertIn(
            "observations[0].evidence must be a repository-relative path",
            MODULE.validate(report([item])),
        )

    def test_evidence_path_traversal_is_rejected(self):
        item = observation("usb")
        item["evidence"] = "audit-artifacts/../outside/report.json"
        self.assertIn(
            "observations[0].evidence must be a repository-relative path",
            MODULE.validate(report([item])),
        )

    def test_cross_platform_absolute_and_unsafe_evidence_paths_are_rejected(self):
        # These must fail on Linux runners too, not just on Windows.
        for evidence in (
            r"C:\outside\report.json",
            r"C:outside\report.json",
            r"\\server\share\report.json",
            r"audit-artifacts\..\outside.json",
            "/tmp/outside.json",
            "../outside.json",
            "audit-artifacts/../../outside.json",
            "audit-artifacts/report.json\x00suffix",
        ):
            with self.subTest(evidence=evidence):
                item = observation("usb")
                item["evidence"] = evidence
                self.assertIn(
                    "observations[0].evidence must be a repository-relative path",
                    MODULE.validate(report([item])),
                )

    def test_repository_relative_evidence_path_is_accepted(self):
        item = observation("usb")
        item["evidence"] = "audit-artifacts/native/usb-report.json"
        self.assertEqual(MODULE.validate(report([item])), [])

    def test_duplicate_observation_ids_are_rejected(self):
        first = observation("audio")
        second = observation("bluetooth")
        second["id"] = first["id"]
        self.assertIn("observations[1].id is duplicated", MODULE.validate(report([first, second])))
