# Foundation release-validator hardening — integration pending acceptance

User priority is stages 1–10 release preparation for desktops and laptops.
The ordered requirements and unresolved delivery conditions are in
`stage1-10-release-readiness.md`; this increment does not waive any blocker.

## Finding and correction

Memory, storage, network and Stage 6 hotplug final validators previously
accepted exit 33 plus marker substrings. A log containing all those markers
and a separate tagged failure or kernel panic satisfied that final condition.
Live-network early-failure checks only covered named network failures, not
every contradictory outcome. There is no evidence here that historical runs
actually contained such a contradiction.

The four consumers now call the same `qemu_validation.validation_errors`.
It requires exact exit 33 and line-anchored markers with a token boundary,
rejects tagged FAILED/TIMEOUT outcomes anywhere in the serial log, and rejects
kernel panic signatures. A detailed success may append whitespace-separated
fields; marker-shaped text in another line or a longer token cannot certify.
Deliberate user faults remain allowed because they are part of isolation tests.
All existing required markers, CPU profiles, assertions and timeouts remain.

Six added host tests exercise the actual shared validator and each consumer's
imported function, both failure/success orderings, missing/prefixed/extended
markers, wrong CPU/exit, detailed success and intentional user faults. The full
Python suite passes all 28 tests. A read-only check of 27 retained serial logs
from `audit-artifacts/acceptance-20261015-234437-300/` found no contradictory
fatal outcomes under the new checks. Fresh QEMU preservation is still required.

No kernel ABI, unsafe code, kernel object, capability, owner lifecycle, lock
order, preemption, IRQ or cross-CPU synchronization is changed by this increment.
Build/lint/host/focused and inherited preservation must pass on frozen inputs
before acceptance; documentation of a result is not release qualification.
