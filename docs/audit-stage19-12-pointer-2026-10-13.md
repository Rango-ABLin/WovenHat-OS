# Stage 19.12 pointer routing audit — 2026-10-13

The previously recorded Stage 19.11 open item for pointer-driven focus is now
validated. The complete Stage 19.12 gate passed on 1, 2, and 4 CPUs:

- 1 CPU: `target/hello-gui-pointer-1cpu`
- 2 CPUs: `target/hello-gui-pointer-2cpu`
- 4 CPUs: `target/hello-gui-pointer-4cpu`
- marker: `[S19.12] pointer input: PASSED`

The gate exercises IRQ12 mouse packet decoding, IRQ1 keyboard decoding, the
event-driven window-manager sleep, click-to-focus on each window, key routing
to the focused client, and lease/focus teardown. The USB HID mouse path and
multi-monitor pointer semantics remain separate hardware/product work.
