# Controlled C++ global lifecycle fixture

The WovenWeb C++ ABI probe has two header-free C++ global objects in separate
translation units. Clang emits their initializers in `.init_array`; the fixture
linker retains that section and exposes explicit start/end symbols. After its
controlled static TLS workers have joined, the Rust adapter validates the array,
invokes its callbacks, and confirms the objects registered destructors through a
fixed four-entry private `__cxa_atexit` registry. It invokes registered callbacks
in reverse order after the existing C++ ABI and heap checks, then verifies the
destructor marker.

Hosted Linux native jobs at `99fa46dc762321a46055899c18f9b4cc349c5284` pass on
1/2/4 CPUs with QEMU exit 33, every established ABI/heap/cleanup marker, and
the new global-lifecycle serial marker. The SDK job reports 179 tests and three
compile-fail tests passing; queue, handoff, generation and selected warning-
denying lint gates pass. The PR CodeQL summary reports no new alerts and zero
PR annotations. Immutable job, artifact, canonical-source, compiler, image and
serial hashes are recorded in [the lifecycle evidence index](wovenweb-cxx-global-lifecycle-evidence.json).

Seventeen Python harness/parser contracts and direct compilation also pass
locally. Windows Application Control still prevents the fresh local Cargo
build/lint chain; no policy bypass was attempted.

## Two-object ordering follow-up (unqualified)

The evidence revision above covers the initial one-object fixture. The current
source extends it to two independently compiled global objects with Clang
`init_priority(200)` and `init_priority(300)`. The linker sorts their two
`.init_array` entries; the adapter demands exactly two registrations and checks
late-constructor/late-destructor-before-early-destructor ordering through a
single phase value. Local static contracts and direct C++ compilation pass.
Hosted Linux 1/2/4-CPU guest and warning-denying lint qualification is pending.

The scope is intentionally narrow: the kernel's dynamic-image lifecycle path is
not exercised by this fixed ET_EXEC fixture, and this does not qualify general
startup/exit ordering, dynamic/shared objects, fork/exec behavior, cancellation,
concurrent initialization, exceptional unwinding, dynamic TLS destructors,
libc++, LibCore, Ladybird, Tor, VPN, or privacy properties.
