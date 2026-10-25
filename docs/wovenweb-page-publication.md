# Queued pager page-state publication

The production worker now refreshes a request from the canonical process mapping
before page I/O. It checks the retained process owner, address-space root and file
binding against admission metadata. A second queued fault for a page already
resident completes without mapping that page again. Otherwise page I/O remains
outside PROCESS_TABLE, and publication updates only the completed page's resident,
dirty and swap fields under the process lock. Other pages' newer metadata survives.
The target page must still match its pre-operation metadata before publication;
root or binding mismatches refuse publication. ABI, capabilities and VM ceilings
are unchanged. These checks are not mapping-generation tokens.

The new guest fixture pauses the feature-only worker before spawning a real
process with two native sibling threads. Both faults must actually enqueue with
zero completed requests and zero canonical resident pages before the worker is
released. Each thread then reads the expected file data, joins and exits. Two
rounds cover disjoint pages (two residents) and duplicate faults (one resident),
with two completed requests each. After each round the file is removed, unused
cache pages reclaimed, and frame/mapping/process baselines must match exactly.
The original snapshot-overwrite worker fails the disjoint case with one resident
page despite two completed requests; that serial and source provenance are kept.

A feature-only metadata fixture invokes the production delta publisher on local
copies of a real file mapping. It checks unrelated resident/dirty bits survive,
stale target and wrong binding are refused without overwriting those bits, and
out-of-range/readonly-write resident checks fail. It performs no fake page I/O.
The sibling tasks use existing default placement; the surrounding real pager boot
also exercises remote leader faults on SMP. This is not a simultaneously paused
cross-CPU write/eviction test. The worker pause and metadata fixture are absent
from production configurations.

Full VM transactions remain open. Root/backing checks do not pin an old space
against exec or distinguish same-file remap ABA. Raw unmap/fork, fast dirty/COW
faults, eviction, cancellation and uncertain physical application still require
mapping generations, durable intent/recovery and coherent operation admission.
Refusing a stale metadata result is not proof of physical rollback. The separate
host journal models have not been wired into all production mapping operations.
Thread-stack/wake/migration lifetime work and larger capacity also remain open.
No complete browser or general native libc/C++ runtime, LibCore, engine, UI, Tor
or VPN is claimed.

Qualification and hashes are recorded in wovenweb-page-publication-evidence.json.
Root Windows host checks remain signing-policy blocked; no policy is bypassed.
Unresolved desktop pixel assertions still prevent complete release acceptance.

Current-source queued/disjoint/duplicate, prior pager/reaping/handoff and sibling
fixtures pass on 1/2/4 CPUs, as do heap, TCP/SOCKS, C++ ABI and legacy GUI
regressions. Default kernel, all four affected WovenWeb feature configurations
and the Rust probe pass -D warnings Clippy. New hosted guest qualification is
pending. Previous 830d8886 guest/SDK/CodeQL/core-foundation success is historical.
