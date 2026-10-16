# Real-toolchain loader fixtures

Run `python scripts/build-loader-fixtures.py` from the repository. This uses
the pinned Rust toolchain's bundled LLD and LLVM tools, compiles with warnings
denied, and writes a unique directory below `target/loader-fixtures/`.
No Cargo acceptance chain, QEMU, signing key or physical device is involved.

`manifest.json` pins source and ELF hashes, the compiler version and the full
command record. The ELF files are not patched after linking. The shared PIE
imports `woven_answer` from `libwoven-demo.so`; the static TLS executable uses
a real volatile compiler-generated local-exec access. Both should exit 42 when
executed by the ordinary WovenHat loader. This is the test's required outcome,
not a claim that the current kernel passes it. `tls-threads-demo` additionally
checks zero-fill, 64-byte alignment, two freshly initialized child threads,
private mutations and preservation of the main thread's changed values.
Thread join uses the existing event-driven blocking syscall, not busy waiting.

The TLS-thread fixture is now linked as a PIE and includes a relocated TLS
pointer initializer. It checks forked mutable state/fresh descendants, natural
and canceled joins, stale handles, the established eight stack/TLS-pair limit,
and owner exit with an initialized sleeping sibling. Timer sleeps coordinate
readiness. The dedicated kernel TLS gate loads it at a nonzero bias and runs
on every online test CPU. Compiling this fixture alone is not runtime proof.

At the initial 2026-10-15 probe, the shared PIE is rejected for LLD's null
`DT_DEBUG`, while the static TLS image parses but exposes the missing TCB and
incorrect TLS-template placement. The kernel acceptance gate still needs to
integrate these unmodified fixtures after correcting those defects. The
malformed-input/signed-graph fixtures remain necessary independent checks.
