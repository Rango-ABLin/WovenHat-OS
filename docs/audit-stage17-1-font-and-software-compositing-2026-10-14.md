# Stage 17.1 font and software compositing — accepted (2026-10-14)

The WovenHat font coverage and bounded software compositor gate passed on 1,
2, and 4 CPUs with exit 33:

- `audit-artifacts/stage17.1-1cpu-1791957821262520800`
- `audit-artifacts/stage17.1-2cpu-1791957966098533500`
- `audit-artifacts/stage17.1-4cpu-1791958016323174300`

The gate covers the expanded font table, clipping, stride, pixel formats,
damage coalescing, and framebuffer presentation. GPU acceleration and
multi-monitor hardware qualification remain outside this software gate.
