# Packaged Calculator launch regression audit — 2026-10-20

The production WovenCalculator package was launched from the hardware-demo
WovenShell launcher after the SMP lock-screen repaint fix. The harness signed
in, opened the client window, verified a changed presented frame, and
exercised the bounded Calculator control through forwarded pointer input.

Evidence:

- 1 CPU: `audit-artifacts/calculator-desktop-1cpu-1792477296412921900`
- 2 CPUs: `audit-artifacts/calculator-desktop-2cpu-1792477368736701600`
- 4 CPUs: `audit-artifacts/calculator-desktop-4cpu-1792477427654653200`

All three runs passed. No client code changed in the shell repaint fix; the
other nine packaged client gates remain covered by their existing 1/2/4-CPU
audits.
