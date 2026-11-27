# Address-space retention through physical CPU handoff

Production reaping now checks a scheduler-owned outgoing process pin for every
prepared context switch. The existing Switching state covers outgoing Running
tasks, but an exiting task can already be Dead and a blocking task already
Blocked while still executing the instructions before the assembly switch.
At that point current_slot and CURRENT_PROCESS_IDS already name the incoming
task. Those fields alone cannot establish that the old address space is unused.

Under SCHEDULER, prepare_switch records the outgoing process identity before
publishing incoming ownership. The pin lives independently of the task's mutable
state and is released only by finalize_switch on the incoming stack. This also
covers the existing first-dispatch task and fork trampolines. A second prepared
handoff on one CPU asserts instead of replacing a live pin. Kernel owner zero
still participates in nested-handoff detection but never retains a user process.
No extra lock, page operation, blocking call or context switch is added under an
IRQ guard. The internal nonblocking process_exited observer now requires the same
scheduler handoff acknowledgement after releasing PROCESS_TABLE. The external
process-info state and syscall ABI are unchanged; pager drain remains separate. Process reaping first checks the scheduler's live tasks plus physical
handoff pins, then the existing pager owner drain, then rechecks the zombie under
PROCESS_TABLE. ABI, capabilities, allocator defaults and VM ceilings are unchanged.

The allocation-free production module has six host tests, including all 24
ordered two-CPU/finish-order schedules, duplicate-owner pins, nested refusal,
wrong CPU, zero capacity and bootstrap/kernel behavior. A feature-isolated local
Scheduler fixture verifies Dead/Blocked/Sleeping metadata continues to retain an
owner through the pin until incoming finalization. It injects no fake task into
the live scheduler and performs no synthetic stack switch. The real guest boot,
pager, sibling faults and cleanup exercise production switching paths separately.

This is a reaping lifetime boundary, not a complete scheduler or VM proof. There
is no deterministic cross-CPU stall-and-reap race injection in this fixture.
Wake/migration races for outgoing blocked tasks, thread-stack lifetime, operation
cancellation, VM generations/reservations, exec/raw-unmap/fork and the independent
VM journal integration remain separate work. The browser runtime/engine/UI/Tor
are unfinished and no VPN gateway exists. Windows application policy is preserved.

Qualification and hashes are recorded in wovenweb-handoff-evidence.json. The
current-source handoff/pager/sibling guest gate, heap, TCP/SOCKS, C++ ABI and
legacy GUI matrices pass on 1/2/4 CPUs. Default kernel and all four affected
WovenWeb feature Clippy gates pass with -D warnings. Six exact-module host tests
pass. Hosted SDK/ownership checks, all three guest jobs, PR CodeQL and core-foundation
succeeded on 830d8886. Later page-publication changes are qualified separately. Root host
checks remain signing-policy blocked and were not retried. Focused validation
never substitutes for the unresolved full release gates.

The first two-CPU qualification exposed an existing observation race: Stage7.4
saw Exited metadata, then immediately tried reaping before physical departure.
The new guard correctly returned StillRunning and the unchanged assertion failed.
Its serial is retained. The observer was strengthened to wait for acknowledgement;
no acceptance assertion was removed and no deadline was increased. This does not
turn the metadata fixture into a deterministic physical cross-CPU race injection.
