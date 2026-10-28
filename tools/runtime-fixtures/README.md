# Native runtime contract fixtures

This directory describes the deliberately small native runtime boundary that
WovenHat currently supports. It is not a claim of POSIX or hosted-libc
compatibility.

`c-abi.c` is a freestanding C fixture. It uses only the WovenHat entry ABI and
the native `int 0x80` exit convention; it does not include a system header or
call a hosted libc. A target-qualified C compiler must produce an ELF64
executable before this fixture can be used in a QEMU acceptance gate.

The existing Rust loader fixtures remain the reference for Rust ELF, shared
object, TLS and thread behavior. The C fixture is intentionally separate so a
future libc implementation cannot silently broaden the supported ABI.

Run `python scripts/check-runtime-contract.py` to inspect the source contract
and record whether an ELF-capable C compiler is available. An unavailable
compiler is reported as `NOT_AVAILABLE`; it is never converted into a passing
runtime result.
