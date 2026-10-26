# Controlled C++ global lifecycle fixture

The WovenWeb C++ ABI probe now has one header-free C++ global object. Clang
emits its initializer in `.init_array`; the fixture linker retains that section
and exposes explicit start/end symbols. After its controlled static TLS workers
have joined, the Rust adapter validates that the array contains exactly one
entry, invokes it, and confirms the object registered one destructor through a
fixed four-entry private `__cxa_atexit` registry. It invokes the registered
callback in reverse order after the existing C++ ABI and heap checks, then
verifies the destructor marker.

This is source integration only. Seventeen Python harness/parser contracts pass,
and the new C++ translation unit compiles locally under the existing guest
flags. Windows Application Control still prevents the fresh Cargo build/lint
chain, so hosted Linux 1/2/4-CPU guest tests and warning-denying lints are the
required acceptance evidence. No policy bypass was attempted.

The scope is intentionally narrow: the kernel's dynamic-image lifecycle path is
not exercised by this fixed ET_EXEC fixture, and this does not qualify general
startup/exit ordering, dynamic/shared objects, fork/exec behavior, cancellation,
concurrent initialization, exceptional unwinding, dynamic TLS destructors,
libc++, LibCore, Ladybird, Tor, VPN, or privacy properties.
