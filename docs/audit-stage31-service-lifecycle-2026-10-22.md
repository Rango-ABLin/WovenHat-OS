# Stage 31 service-lifecycle repeat evidence — 2026-10-22

The Stage 31 repeated-runner now accepts `recovery` and invokes the existing
Stage 27.5 durable-recovery QEMU gate sequentially. Each run copies the
child output, result metadata, and corresponding Stage 27.5 evidence into its
own directory.

Command:

```text
python scripts/test-stage31-repeated-boots.py --suite recovery --cpus 1 --count 2 --timeout 300
```

Result: PASS. Both runs exited 0; the first took 40.5 seconds and the second
54.1 seconds. Evidence is retained in
`audit-artifacts/stage31-repeated/recovery-1cpu/run-0001` and `run-0002`.

This proves repeatable startup, durable-recovery execution, and bounded
teardown for this existing gate at 1 CPU. It does not implement or qualify a
generic crash supervisor, automatic service restart, physical power-loss
recovery, driver restart, packet corruption, or native hardware behavior.
The runner currently limits this suite to the Stage 27.5-supported 1/2/4 CPU
profiles.
