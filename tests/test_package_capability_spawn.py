"""Static guard for atomic package capability installation during user spawn."""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class PackageCapabilitySpawnContracts(unittest.TestCase):
    def setUp(self):
        self.task = (ROOT / "kernel/src/task.rs").read_text()

    def test_package_capabilities_are_installed_before_the_task_is_published(self):
        start = self.task.index("fn spawn_user_process_with_sandbox_and_capabilities(")
        end = self.task.index("/// Launch a signed package", start)
        helper = self.task[start:end]
        initialized = helper.index(".initialize_user(")
        capabilities = helper.index(".capabilities = capabilities;")
        published = helper.index("scheduler.task_count += 1;")
        self.assertLess(initialized, capabilities)
        self.assertLess(capabilities, published)
        self.assertIn(".capability_lineages = [None; Capability::COUNT];", helper)

    def test_package_launcher_uses_the_atomic_spawn_helper(self):
        start = self.task.index("pub fn spawn_user_process_with_package_capabilities(")
        end = self.task.index("/// Redirect one normal user exit", start)
        launcher = self.task[start:end]
        self.assertIn("spawn_user_process_with_sandbox_and_capabilities", launcher)
        self.assertNotIn("let result = spawn_user_process_with_sandbox", launcher)


if __name__ == "__main__":
    unittest.main()
