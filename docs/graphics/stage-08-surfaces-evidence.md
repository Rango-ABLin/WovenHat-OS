# Graphics roadmap Stage 08 secure userspace surfaces and IPC

Date: 2026-10-25

## Implementation

Stage 08 uses the existing owner-bound, generation-tagged surface registry and
the compositor IPC boundary. Surface creation enforces dimensions, per-owner
and system pixel budgets; surface tokens are checked against owner and
generation; sharing is an explicit owner operation; and private surfaces are
not readable by a grantee. Client exit releases surfaces and grants together.
The compositor path copies shared pixels through a snapshot/region boundary,
so it does not retain a userspace pointer or read an unauthorized surface.

The new `stage08-graphics-test` feature provides a roadmap-native acceptance
gate. It runs two independent Ring-3 `hello-gui` processes: a compositor with
`DisplayIo | Ipc` and an ordinary client. The client shares one surface,
retains another private surface, and sends the handles over IPC. The gate
requires the shared surface to compose, the private surface to be refused,
pixel ownership checks to pass, and all client surfaces/grants to be released
on exit.

## Evidence

- `python scripts/test-hello-gui-compositor-qemu.py --stage 08 --cpus 1`:
  PASS, exit 33.
- `python scripts/test-hello-gui-compositor-qemu.py --stage 08 --cpus 2`:
  PASS, exit 33.
- `python scripts/test-hello-gui-compositor-qemu.py --stage 08 --cpus 4`:
  PASS, exit 33.
- Evidence: `target/stage08-graphics-{1,2,4}cpu`.
- Target warning-denying kernel Clippy with `stage08-graphics-test`: PASS.
- Generic host `--all-targets` kernel Clippy remains incompatible with the
  freestanding kernel's panic/allocator handlers when host-only WebP `std`
  dependencies are linked; this is recorded as a tooling limitation, not a
  Stage 08 guest acceptance failure.

## Gate status

Stage 08 is accepted for the secure userspace surface/IPC and QEMU 1/2/4-CPU
scope. Physical hardware display and input qualification remain outside this
guest gate.
