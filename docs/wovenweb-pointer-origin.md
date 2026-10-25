# WovenWeb allocator pointer-origin follow-up

The bitmap heap still tracks numeric addresses, but its runtime adapter now asks
the VM backend to resolve a reserved extent into a payload pointer. The host
backend retains the original System pointer in AtomicPtr and derives suballocation
pointers from it after checked subtraction/addition and a full-buffer extent check.
The native backend uses an explicit exposed-address conversion for syscall-created
foreign memory. Zero-size requests return null instead of a fabricated low-address
sentinel. The GlobalAlloc contract still requires nonzero valid caller layouts.

This change makes the pointer-origin boundary explicit. It does not repair the
kernel's mapping transaction races or lift its sixteen 64 KiB mapping ceiling.
The allocator remains opt-in and requires coordinated VM use.

## Validation boundary

The SDK unit binary was blocked by Windows Application Control before execution
(error 4551). It was not renamed, copied, relaunched through a workaround or exempted
from signing policy. The newly added full-buffer first/last-byte host test therefore
requires hosted CI validation. A focused Linux workflow now runs SDK all-feature
unit/compile-fail tests and warning-denying all-target Clippy, retaining logs.
An all-target Clippy pass compiles the test code but does not run its assertions.

Native heap, TCP/SOCKS and C++ ABI fixture matrices are rerun using the updated
native adapter; results and hashes are in wovenweb-pointer-origin-evidence.json.
These fixtures do not execute the host-only System backend. Existing GUI default
allocator behavior is unchanged. Full VM/runtime/browser acceptance remains open.

The three previously reported high CodeQL pointer alerts remain unresolved until
a fresh scan confirms their status. The old report identified host test writes;
the connector did not supply full dataflow paths. This change is not an alert
suppression or a declaration that the earlier reports were false positives. A
neutral CodeQL result with a missing Rust configuration is not a clean scan.
