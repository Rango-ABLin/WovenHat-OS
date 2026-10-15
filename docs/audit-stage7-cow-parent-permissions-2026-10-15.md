# User page-table ancestor permissions — correction integrated, acceptance pending

## Executable finding

The real libwoven status fixture passed six process-status cases and twelve
natural/cancelled joins, then faulted writing its forked user stack. Retained
failure: `audit-artifacts/stage11.2-status-1cpu-1792086619756210600/` (QEMU 35,
not success). Original paging/task/userspace snapshots are preserved there.
The diagnostic counterpart is `stage11.2-status-1cpu-1792086799166937200/`.
Its trace shows a shared stack frame at reference count two, the remaining
owner at count one, then another write fault with the leaf already writable.
The latter was rejected and the child died. Object/process/frame baselines
were restored; that does not make the failed fork behavior acceptable.

The local primary source for x86_64 0.15.5 confirms `Mapper::map_to` derives
ancestor flags from the leaf's PRESENT/WRITABLE/USER bits, while `update_flags`
updates only the leaf. `MappedPageTable::translate` reports the leaf flags,
not the effective ancestor intersection. New COW roots therefore acquired
read-only ancestors when read-only shared leaves were installed. The sole-owner
COW branch subsequently made only the leaf writable; the CPU still denied
the access. This explains why leaf-only checks and synthetic/kernel-physical
access tests did not prove actual user stack writes work.

## Integrated correction and scope

`page_permissions::ancestor_bits` keeps present user ancestors writable while
preserving the original leaf policy. It never adds USER to supervisor mappings,
does not propagate leaf-only NX/GLOBAL bits to ancestors and leaves kernel
ancestor write policy unchanged. Compile-time assertions bind its numeric
PRESENT/WRITABLE/USER masks to the paging library's flag definitions. Host
regressions cover read-only/NX user leaves and all eight low-bit combinations.

The existing recorded user-map path, COW share/remap/restore, private-file and
file-frame installation paths now explicitly use those ancestor flags. This
addresses the common failure mechanism, not just the single test's fork path.
Leaf W^X, RELRO, logical-writability authorization, reference retention, failed
map rollback, allocator limits, lock order and TLB synchronization are unchanged.
Existing unsafe mapping calls use the explicit-parent-flags variant; no new
unsafe block, kernel object, lock, capability, scheduler wait or pointer lifetime
is introduced. Only existing user-mapping parents gain the write bit; a read-only
leaf remains read-only and cannot be made writable through this helper.

The next run, `stage11.2-status-1cpu-1792087130955323000/`, had no stack exception
and reached the separate foreign-wait assertion (codes 92/93). That is evidence
that this failure was removed, not a passing full gate. The following run after
the independent wait-ownership correction passed the full one-CPU libwoven
fixture, original thread gate and exact resource baselines:
`stage11.2-status-1cpu-1792087223023180400/`.

The final revised focused matrix now passes on 1/2/4 CPUs, including all
resource and original thread assertions. Session 94658 is running full combined
acceptance under the new 500-file freeze in `audit-artifacts/exit-status-20261015/`.
Full inherited preservation remains required. This correction is **not accepted**
from focused passes. Keep all
failed evidence, run warning-denying host/kernel/affected-feature checks and
the unchanged Stage 10.7/10.8–11.5 preservation gates before accepting it.
