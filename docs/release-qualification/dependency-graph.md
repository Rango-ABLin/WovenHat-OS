# Release qualification dependency graph

```text
M00 inventory/checkpoint
  -> M01 known CI regressions and storage mutation
    -> M02 boot/session reliability
      -> M03 kernel safety/concurrency
        -> M04 persistent storage/filesystems
          -> M11 installer/recovery/rollback
            -> M14 reproducible release artifacts
              -> M15 final release decision
      -> M05 drivers/HID/display device matrix
        -> M07 compositor/display
          -> M08 desktop shell/UX
            -> M09 applications/services
  -> M06 networking/radio
  -> M10 identity/security/trust chain
  -> M12 performance/soak/resource limits
  -> M13 physical compatibility and certification
```

Edges are release dependencies, not claims that every earlier source feature
is absent. A downstream gate cannot be called accepted when its upstream
evidence is stale, failed, or blocked.
