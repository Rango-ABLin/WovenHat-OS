# Controlled C++ global lifecycle fixture

The WovenWeb C++ ABI probe now has one header-free C++ global object. Clang
emits its initializer in `.init_array`; the fixture linker retains that section
and exposes explicit start/end symbols. After its controlled static TLS workers
have joined, the Rust adapter validates that the array contains exactly one
entry, invokes it, and confirms the object registered one destructor through a
fixed four-entry private `__cxa_atexit` registry. It invokes the registered
callback in reverse order after the existing C++ ABI and heap checks, then
verifies the destructor marker.

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

The scope is intentionally narrow: the kernel's dynamic-image lifecycle path is
not exercised by this fixed ET_EXEC fixture, and this does not qualify general
startup/exit ordering, dynamic/shared objects, fork/exec behavior, cancellation,
concurrent initialization, exceptional unwinding, dynamic TLS destructors,
libc++, LibCore, Ladybird, Tor, VPN, or privacy properties.
