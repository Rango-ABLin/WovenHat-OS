# WovenDev Stage 32

Stage 32 provides the developer-facing diagnostics and debugging foundation:

- bounded structured diagnostics, severity filtering, and a ring buffer;
- panic/crash-context dumps with preserved serial evidence;
- zero-cost tracing spans and a bounded performance profiler;
- GDB Remote Serial Protocol framing, COM2 transport, packet assembly,
  register context, memory-window reads, session lifecycle, trap-stop mapping,
  generation-bound ownership, trap-frame snapshots, and task binding;
- an integrated `WovenDev` coordinator that connects those boundaries while
  keeping ownership and memory access explicit.

The integrated gate is `stage32-integrated-test` and uses
`scripts/test-stage32-integrated.py`. The coordinator is deliberately bounded:
it does not dereference arbitrary guest addresses, retain interrupt-frame
references, or resume a task from an interrupt handler. A production trap
handler must hand off to the coordinator at a scheduler-safe point and retain
the existing teardown, cancellation, lock-order, and generation checks.

## Application developer path

Start from `templates/wovenhat-app-template`, then read
`docs/writing-wovenhat-applications.md`, `docs/userspace-abi.md`, and
`docs/syscall-api.md`. The template uses `libwoven` wrappers and the same
large-code-model/linker contract as `apps/hello`; it does not require kernel
knowledge or direct `int 0x80` assembly.
