# Stage 19.13 transparency and shadow audit — 2026-10-13

The complete Stage 19.13 compositor gate passed on 1, 2, and 4 CPUs:

- 1 CPU: `target/hello-gui-alpha-1cpu`
- 2 CPUs: `target/hello-gui-alpha-2cpu`
- 4 CPUs: `target/hello-gui-alpha-4cpu`
- marker: `[S19.13] translucency and shadows: PASSED`

The gate verifies refusal of unprivileged blending, compositor-owned backdrop
and lease, client sharing, independently recomputed translucent/shadow regions,
clean client/compositor exits, and lease release. This is software compositor
qualification under QEMU; hardware GPU acceleration and multi-monitor output
remain outside the boundary.
