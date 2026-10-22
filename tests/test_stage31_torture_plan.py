import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "stage31-torture-plan.py"
SPEC = importlib.util.spec_from_file_location("stage31_torture_plan", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class Stage31TorturePlanTests(unittest.TestCase):
    def test_default_plan_covers_all_suites_and_cpu_profiles(self):
        plan = MODULE.build_plan()
        self.assertEqual(len(plan["jobs"]), 25)
        self.assertEqual(MODULE.validate_plan(plan), [])

    def test_eight_and_sixteen_cpu_jobs_are_explicitly_blocked(self):
        plan = MODULE.build_plan()
        blocked = [job for job in plan["jobs"] if job["cpus"] in (8, 16)]
        self.assertEqual(len(blocked), 10)
        self.assertTrue(all(job["status"] == "blocked" for job in blocked))
        self.assertTrue(all(job["command"] is None for job in blocked))

    def test_supported_jobs_are_pending_with_commands(self):
        plan = MODULE.build_plan((1, 4))
        jobs = plan["jobs"]
        self.assertEqual(len(jobs), 10)
        self.assertTrue(all(job["status"] == "pending" for job in jobs))
        self.assertTrue(all(isinstance(job["command"], list) for job in jobs))

    def test_tampering_cpu_status_is_rejected(self):
        plan = MODULE.build_plan((1,))
        plan["jobs"][0]["status"] = "blocked"
        self.assertTrue(MODULE.validate_plan(plan))

    def test_unknown_suite_is_rejected(self):
        plan = MODULE.build_plan((1,))
        plan["jobs"][0]["suite"] = "unknown"
        self.assertTrue(MODULE.validate_plan(plan))
