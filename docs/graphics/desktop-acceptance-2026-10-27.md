# Desktop software graphics acceptance ? 2026-10-27

Status: final matrix in progress; not yet accepted or closed.

## Boundary

This gate covers the existing software desktop, secure compositor, input,
window management, emulated VirtIO GPU scanout/recovery, and interactive demo
paths. It does not certify native GPUs, native HID, GPU-accelerated 3D, or the
complete graphics modernization mandate. WHPX accelerates the guest CPU.

## Corrections

| Gate | Correction and retained/strengthened assertion |
| --- | --- |
| 20.2 | Explicit flat application contract; current wallpaper/taskbar palette; exact taskbar pixels and actual toast border, text and translucency replace background-only tolerance. |
| 20.3 | Explicit flat launcher contract; independent version-1 generic icon masks; the entire panel, including icons, selection, header and border, is compared. Failed launches require the exact notification text. |
| 20.4 | Flat drag/close contract and matching independent background formula; drag and client teardown assertions retained. |
| 20.6 | Flat translucency contract and current panel-derived toast colour; window/overlay/pointer comparisons retained. |
| 20.8 | Flat resize/tiling contract and matching cleanup background; geometry, protocol and owner teardown assertions retained. |
| 20.9 | Flat notifications contract and current toast colour; complete Notification Center panels replace partial interior sampling. |
| 20.13 | Flat workspace contract and matching exposed background; switching, moving, focus and cleanup assertions retained. |
| 20.18 | Existing perspective title-bar checks retained; added an independent full-pixel projected-toast comparison and a visible-text requirement. The old blank-toast claim cannot pass this assertion. |

The flat behavior gates are separate from the dedicated 3D gates. Unused local
projection calculations were removed from the flat fixtures; the independent
3D oracle remains active. No feature prerequisites, capabilities, production
ABI, timeouts, stress counts or warning policies were relaxed. Runtime code is
unchanged by the fixture repairs. A stale positive normal-boot serial fixture
was also corrected; a new regression rejects the obsolete marker that lacks
licensed-wallpaper VFS readback evidence.

## Planned validation

All QEMU cases run sequentially on 1, 2 and 4 CPUs.

| Coverage | Existing runner | Cases per CPU |
| --- | --- | --- |
| Secure surfaces/compositor and prior input/rendering contracts | `scripts/test-hello-gui-compositor-qemu.py` | 08, 09, 19.12, 19.13, 19.14, 20.16 |
| Desktop behavior and dedicated 3D | `scripts/test-woven-shell-qemu.py` | 20.1?20.13, 20.17, 20.18, 24.59 |
| GPU scanout and recovery | `scripts/test-stage20-14-single-output.py` | default and `--recovery` |
| Two GPU heads | `scripts/test-stage20-14-monitors.py` | default |
| Desktop across monitors | `scripts/test-stage20-14b-desktop.py` | default |
| Session/focus/console/power-off | `scripts/test-stage20-15-session.py` | default |
| Real desktop screenshots/input/performance | `scripts/test-desktop-qualification.py` | default TCG and `--accelerator whpx` |

Total planned: 87 runs, all using the runners' existing assertions and limits.
Kernel build, warning-denying host/kernel/app Clippy, Rust host tests and the
full Python suite are required as well. On this Windows host, creating real
symbolic-link fixtures requires an elevated test process; those security tests
are run with the needed OS privilege, not mocked, skipped or weakened.

## Evidence

- Static checks: `audit-artifacts/graphics-close-static-20261027-163001/`.
- Focused repairs and strengthened 3D-toast check: `audit-artifacts/graphics-close-focus-*`.
- Full matrix: `audit-artifacts/graphics-close-matrix-20261027-164141/manifest.json`.
- Previous failures remain in the earlier `desktop-ux-*` and `graphics-close-focus-*` evidence.

Final results, accepted source revision and commit/push information will be
recorded after the gates complete.
