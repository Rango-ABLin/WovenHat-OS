"""Static contracts for the bounded WovenWeb loader environment primitive."""
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class ProcessAbiContracts(unittest.TestCase):
    def setUp(self):
        self.userspace = (ROOT / "kernel/src/userspace.rs").read_text()
        self.linker = (ROOT / "kernel/src/userspace/linker.rs").read_text()

    def test_existing_argv_entrypoint_preserves_empty_environment(self):
        self.assertIn("setup_argv_env_stack(address_space, stack, args, &[])", self.userspace)

    def test_environment_stack_is_bounded_and_not_inherited(self):
        self.assertIn("pub const MAX_ENV: usize = 8;", self.userspace)
        self.assertIn("if args.len() > MAX_ARGV || env.len() > MAX_ENV", self.userspace)
        self.assertIn("args.iter().chain(env.iter())", self.userspace)
        self.assertIn("neither inherits the parent's environment", self.userspace)

    def test_envp_is_emitted_after_the_argv_terminator(self):
        self.assertIn("let envp = argv_null + 8;", self.userspace)
        self.assertIn("skip(args.len()).take(env.len())", self.userspace)
        self.assertIn("let envp_null = envp + (env.len() as u64) * 8;", self.userspace)

    def test_only_an_explicit_loader_api_can_opt_into_environment_delivery(self):
        self.assertIn("pub fn load_elf_with_argv_env(", self.userspace)
        self.assertIn("linker::load_with_env(bytes, args, env)", self.userspace)
        self.assertIn("if env.is_empty()", self.linker)
        self.assertIn("setup_argv_stack(page_table, stack, args)?", self.linker)
        self.assertIn("setup_argv_env_stack(page_table, stack, args, env)", self.linker)


if __name__ == "__main__":
    unittest.main()
