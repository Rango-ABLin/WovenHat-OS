# Stage 22.5 WovenPackage transaction authority audit

Date: 2026-10-20  
Owner: Codex  
Scope: signed Ring-3 WovenPackage install/remove/rollback authority

## Result

Accepted as a bounded Stage 22.5 increment. The required QEMU transaction
gate passed on 1, 2, and 4 CPUs, each with exit code 33 and the serial marker
`[S22.5] WovenPackage transactions: PASSED`.

## Implementation boundary

Syscall 217 is exposed through `libwoven::package_transaction` and dispatched
by `apps/woven-package`. The kernel validates the operation and source path,
requires `FileRead`, `FileWrite`, and `StorageIo`, and then issues the private
WovenFS writer authority. Install reads a broker-owned VFS source into bounded
heap storage, verifies the WPKG envelope, and publishes authenticated,
chunked payloads with durable catalog and journal records. Remove and rollback
reuse recovered catalog state and durable rollback intent. Package bytes do
not cross the IPC control frame.

## Commands and results

The following commands were run from `C:\WovenHat-OS`:

```text
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage22-5-test -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo test -p libwoven --lib
python scripts/test-stage22-5-package-transactions.py --cpus 1
python scripts/test-stage22-5-package-transactions.py --cpus 2
python scripts/test-stage22-5-package-transactions.py --cpus 4
python -m py_compile scripts/test-stage22-5-package-transactions.py
```

The focused transaction script rebuilt and signed the Ring-3 broker and WPKG
fixture for each run. The 66-test `libwoven` suite passed. Both Clippy runs
passed with `-D warnings`. The QEMU evidence is retained at:

```text
audit-artifacts/stage22.5-1cpu-1792521732550569700
audit-artifacts/stage22.5-2cpu-1792521764182836500
audit-artifacts/stage22.5-4cpu-1792521793861888600
```

## Limits and open prerequisites

The syscall path is bounded by the normal 512-byte path limit. A single source
package is limited by the 64 KiB VFS node capacity; native WovenFS publication
is chunked by its file limit. The service-enabled kernel task stack is 256 KiB
and remains protected by the task canary. This gate does not establish larger
source ingress, signing-key rotation, a complete update daemon, power-loss
timing qualification, or physical USB behavior. No physical USB write was
performed; the attached flash drive remains outside this accepted scope.
