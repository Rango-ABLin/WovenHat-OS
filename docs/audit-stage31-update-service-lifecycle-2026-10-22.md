# Stage 31 update-service lifecycle repeat evidence — 2026-10-22

The Stage 31 repeated-runner now accepts `update-service` and invokes the
existing signed Stage 27.5 `woven-update --service` QEMU gate sequentially.
Each iteration retains the service gate's serial, QEMU, and package evidence
under its own repeated-run directory.

Command:

```text
python scripts/test-stage31-repeated-boots.py --suite update-service --cpus 1 --count 2 --timeout 300
```

Result: PASS. Both runs exited 0; the first took 87.9 seconds and the second
56.3 seconds. Evidence is retained in
`audit-artifacts/stage31-repeated/update-service-1cpu/run-0001` and
`run-0002`.

This boundary is intended to prove repeatable signed service publication,
request handling, malformed-request rejection, and orderly teardown. It does
not implement automatic restart after a crashed service, driver restart,
physical power-loss recovery, or native hardware qualification.
