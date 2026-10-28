# Static compiler thread-local storage fixture

Here TLS means thread-local storage, not HTTPS transport security.

The C++ fixture adds a real PT_TLS image and explicitly uses the static local-exec
model. One 64-byte-aligned volatile initialized word and one aligned zero word
exercise .tdata/.tbss and compiler FS-relative addressing. The main task mutates
its values before creating two native sibling threads; both must receive the
immutable initial template rather than copies of the parent's changed values.

Both siblings publish distinct aligned addresses and synchronize before repeated
sets/checks across 64 yields each. Joins must succeed and the main task's values
and TLS address remain unchanged. The existing integer/aggregate calls, guarded
static, 512 virtual objects/destructors, heap checks and exact kernel cleanup are
all retained. Guard/static-object tests run only after joins, so this does not
qualify concurrent guards. Thread creation uses the existing Console-only child
profile and adds no kernel ABI, capability or catalog entry.

The controlled fixture passes hosted Linux 1/2/4-CPU guest qualification on
92328129, including all prior assertions and exact cleanup. Windows
Application Control still blocks compilation/lints; no blocked tool is retried
or replaced. Fourteen local Python packet/parser/firmware checks and source
format/diff checks pass. The hosted jobs provide the actual C++ execution proof,
and also pass heap/network/Stage19.2 GUI regressions plus kernel/probe Clippy.
Artifacts/source/serial/compiler hashes are in wovenweb-cxx-static-tls-evidence.json.
Complete Rust analysis and the PR CodeQL check also succeeded on 92328129,
with no new alerts in changed code and zero PR annotations.

Sibling placement remains the existing default; these checks do not establish
cross-CPU migration, dynamic TLS, nontrivial thread-local initialization or
thread-local destructors, global initializer/finalizer arrays, exception/RTTI
runtime, FP/SIMD context isolation, libc++ or a Ladybird engine. VM transaction
admission/lifetimes and capacity remain independent prerequisites. No full
browser, Tor/VPN or anonymity claim follows.

The preceding native baseline 58938b7e passed heap/TCP-SOCKS/C++ ABI/Stage19.2 GUI
and selected lints on Linux 1/2/4 CPUs. SDK/pager/generation controls and the PR
CodeQL check also succeeded, with no new alerts and zero annotations. Artifact
source/serial/compiler hashes are archived in wovenweb-linux-native-evidence.json;
those results are historical for this newly expanded C++ fixture.
