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
        self.assertEqual(len(plan["jobs"]), 60)
        self.assertEqual(MODULE.validate_plan(plan), [])

    def test_eight_and_sixteen_cpu_jobs_are_pending(self):
        plan = MODULE.build_plan()
        expanded = [job for job in plan["jobs"] if job["cpus"] in (8, 16)]
        self.assertEqual(len(expanded), 24)
        self.assertEqual(sum(job["status"] == "pending" for job in expanded), 24)
        self.assertEqual(sum(job["status"] == "blocked" for job in expanded), 0)
        self.assertEqual(sum(isinstance(job["command"], list) for job in expanded), 24)

    def test_lifecycle_gates_are_pending_across_8_and_16_cpus(self):
        plan = MODULE.build_plan()
        lifecycle = [
            job for job in plan["jobs"]
            if job["suite"] in ("recovery", "update-service", "service-supervisor")
        ]
        self.assertEqual(len(lifecycle), 15)
        self.assertTrue(all(job["status"] == "pending" for job in lifecycle if job["cpus"] <= 4))
        self.assertTrue(all(job["status"] == "pending" for job in lifecycle))

    def test_supported_jobs_are_pending_with_commands(self):
        plan = MODULE.build_plan((1, 4))
        jobs = plan["jobs"]
        self.assertEqual(len(jobs), 24)
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

    def test_fault_scenario_map_is_explicit_and_fail_closed(self):
        plan = MODULE.build_plan((1,))
        scenarios = plan["fault_scenarios"]
        self.assertEqual(
            plan["randomized_fault_injection"],
            "partial-wovenfs-host-stage14-checksum-and-seeded-scenario-orchestration-1-2-4-8-16cpu",
        )
        self.assertEqual(
            scenarios["whole-kernel-low-ram"],
            "covered-stage31-disposable-256m-boot-matrix-1-2-4-8-16cpu",
        )
        self.assertEqual(scenarios["filesystem-disk-full"], "covered-wovenfs-host")
        self.assertEqual(
            scenarios["filesystem-read-fault"],
            "covered-qemu-blkdebug-read-aio-eio-1-2-4-8-16cpu",
        )
        self.assertEqual(scenarios["service-lifecycle"], "covered-signed-update-service-and-supervisor-recovery-1-2-4-8-16cpu")
        self.assertEqual(scenarios["packet-corruption"], "partial-stage31-checksum-boundary")
        self.assertEqual(scenarios["driver-crash"], "covered-stage31-controller-owned-restart-1-2-4-8-16cpu")
        self.assertEqual(scenarios["process-crash"], "covered-stage31-qemu-process-crash")
        self.assertEqual(
            scenarios["sudden-shutdown"],
            "covered-qemu-abrupt-termination-and-reboot-1-2-4cpu",
        )
