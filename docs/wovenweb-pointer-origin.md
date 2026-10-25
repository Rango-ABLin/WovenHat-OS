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

Windows Application Control blocked the local SDK unit binary before execution
(error 4551). Hosted Linux validation on b72a26d then passed 178 unit tests,
three compile-fail ownership checks and all-feature/all-target Clippy, including
the new full-buffer first/last-byte host regression. The signing policy was not
changed. The dedicated workflow retains logs for relevant branch/PR changes.

A test-only follow-up now centralizes explicit null rejection before unsafe
payload access and adds a negative test verifying null input panics. The expected
new unit count is 179; fresh hosted execution and a fresh scan are required.
Local all-target Clippy compiles the new test but does not run its assertions.
The production allocator prefix is byte-identical after LF normalization; no
native runtime code changed in this guard follow-up.

Native heap, TCP/SOCKS and C++ ABI fixture matrices are rerun using the updated
native adapter; results and hashes are in wovenweb-pointer-origin-evidence.json.
These fixtures do not execute the host-only System backend. Existing GUI default
allocator behavior is unchanged. Full VM/runtime/browser acceptance remains open.

The complete scan on b72a26d reported five high invalid-pointer alerts at host
test writes. Those findings remain open. The connector does not supply the full
dataflow paths, and the blocked browser URL-verification path was not retried.
No warning, query, path or test is disabled, and no finding is dismissed.

GitHub's [query source](https://github.com/github/codeql/blob/main/rust/ql/lib/codeql/rust/security/AccessInvalidPointerExtensions.qll)
uses pointer-invalidation sources; its [core models](https://github.com/github/codeql/blob/main/rust/ql/lib/codeql/rust/frameworks/stdlib/core.model.yml)
include null pointers, and its [guard logic](https://github.com/github/codeql/blob/main/rust/ql/lib/codeql/rust/security/Barriers.qll)
recognizes the false branch of an explicit is_null() check. This motivates the
explicit helper as an analyzer-model hypothesis, not proof that the old findings
are false positives. Non-nullness alone is not a lifetime/extent proof; backend
ownership and the unsafe GlobalAlloc caller contract still apply. A neutral scan
with a missing Rust configuration is not a clean scan.
