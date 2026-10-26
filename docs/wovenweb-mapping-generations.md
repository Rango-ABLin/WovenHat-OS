# Mapping-lifetime lineage tags (qualification pending)

Fresh anonymous and lazy-file constructors issue a checked NonZeroU64 lineage
tag from one boot-wide atomic authority. The tag is obtained before the leaf
constructor allocates pages or retains its backing descriptor. Private-file/TLS
mappings use the anonymous constructor; shared mappings use the lazy constructor.
Other wrapper preparation is not covered by a global transaction. Failed maps
may consume IDs. Zero and MAX are reserved; exhaustion is sticky and refuses new
issuance instead of wrapping. Memory ordering provides uniqueness only; existing
canonical mapping locks still publish the metadata.

AnonymousMapping copies retain their tag. Fork copies lineage into a different
owner/root, so the pager identity remains owner + root + tag + immutable binding.
Snapshots do not create operation leases. The pager's existing binding checks now
include the lineage tag, preventing a queued request from matching a same-file
replacement at the same virtual slot. This does not pin active work against
concurrent unmap/exec, make slot admission transactional, or recover uncertain I/O.
ABI, capabilities, mapping addresses and capacity limits stay unchanged.

The new real guest fixture queues a sibling fault while the test-only worker is
paused. Its leader then actually munmaps and remaps the same file/descriptor,
length, offset, rights and virtual address. A bounded acknowledgement file uses
r/x phases to synchronize without sleeps or extra VFS capacity. Only this test
child has FileWrite. The old request must be refused with the existing user page
fault status -11, zero resident replacement pages, one completed request and exact
resource cleanup. The controlled user fault is expected evidence, not a hidden
kernel panic. Tags of the old/replacement mapping must differ.

A Linux-only negative-control job uses a detached disposable checkout, removes
only the lineage comparison (all guest assertions remain), and must observe the
precise old behavior: exit 0 and one resident replacement page. Build failures,
timeouts and unrelated guest failures cannot satisfy this control. The primary
checkout is hashed before/after and never mutated. Source mutation and serial logs
are retained. This is a counterfactual guard test, not a byte-identical old build.

Four Rust issuer tests cover sequential allocation, 4096 IDs across eight host
threads, sticky exhaustion and racing exhaustion. These await hosted execution.
Four Python checks of the negative-evidence parser pass locally; syntax and diff
checks pass. Native generation qualification and its negative control are still
pending. Earlier three-file fixture runs failed before releasing the worker; their
cause was not established and they are not evidence of stale-work acceptance.

Windows Application Control now blocks clippy-driver.exe and dependency build
scripts (4551). No binary was renamed, copied or retried to evade policy. New
compilation/guest qualification is delegated to Linux CI, not claimed locally.
Full journal/owner retirement, active-work lifetime, fork/exec/unmap/cancellation,
thread-stack/wake/migration and larger capacity remain open. Native browser,
libc/C++ runtime, LibCore, dependencies/TLS, engine, UI, Tor and VPN are unfinished.
