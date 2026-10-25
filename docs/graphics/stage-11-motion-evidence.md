# Graphics roadmap Stage 11 motion and pacing evidence

Date: 2026-10-25  
Scope: bounded motion-token and animation-runtime increment

## Accepted slice

The existing shell animation and compositor pacing contracts were requalified
against the current renderer. The Stage 20.7 oracle captures the actual
pre-window framebuffer pixel as the animation background, isolating the spring
and alpha-blend contract from the separate wallpaper/projection geometry. It checks
that every presented sample is a valid monotonic blend, that at least three
intermediate opacities are observed, that the final frame is opaque, and that
the display lease and client teardown complete cleanly.

The Stage 19.14 gate independently covers integer display scaling, full-screen
repaint after rescaling, paced presentation, lease release, and restoration of
the next holder's default scale.

## Results

| Gate | 1 CPU | 2 CPUs | 4 CPUs |
|---|---|---|---|
| Stage 19.14 display scaling and frame pacing | PASS | PASS | PASS |
| Stage 20.7 spring animation | PASS | PASS | PASS |
| Stage 20.10 Quick Settings accessibility/motion path | PASS | PASS | PASS |

| Stage 20.11 lock screen and PIN isolation | PASS | PASS | PASS |

The full desktop screenshot/input qualification also passes on 1/2/4 CPUs
after the production catalog capacity repair. Each run captured the signed-in
desktop, WovenFiles open/click/close, WovenTerminal keyboard input/close, and
the restored desktop. Evidence is retained under
`audit-artifacts/desktop-qualification-1792952128648637100`,
`audit-artifacts/desktop-qualification-1792952312640236600`, and
`audit-artifacts/desktop-qualification-1792952447677142500`.

Stage 20.7 observed monotonic opacity sequences including
`[0, 48, 194, 236, 250, 253, 255]` on the 1-CPU run and completed with QEMU
exit 33. Warning-denying freestanding kernel Clippy passed for the affected
`stage20-7-test` feature.

The animation gate also injects neutral PS/2 pointer packets throughout the
fade. It delivered 108, 114, and 135 packets on the 4-, 2-, and 1-CPU runs
respectively while still observing multiple intermediate opacities and a
fully settled frame.

Evidence is retained under:

- `target/hello-gui-scaled-{1,2,4}cpu`
- `target/woven-shell-animation-{1,2,4}cpu`
- `target/woven-shell-quick-settings-{1,2,4}cpu`

The Stage 20.10 matrix exercises the production Quick Settings path,
including an actual Reduced Motion toggle with visible `On` state, Do Not
Disturb state, tiling over a live client, scale 1/2 transitions, panel close,
input routing, and teardown.

The shared WovenUI renderer also has golden pixel signatures for a focused
Button plus checked Checkbox composition in dark, light, and high-contrast
themes. The 183-test libwoven suite and warning-denying all-targets Clippy
pass.

## Boundary

This accepts the motion/pacing, Quick Settings accessibility, and desktop
screenshot/input runtime slices. A complete Stage 11 claim still requires
theme-variation screenshots and low-power runtime evidence; the sustained-input
starvation slice is covered by the Stage 20.7 matrix above.
