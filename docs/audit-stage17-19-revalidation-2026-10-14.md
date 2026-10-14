# Stages 17–19 compositor revalidation — 2026-10-14

After scheduler-driven termination cleanup was tightened, the exposed
compositor gates were rerun on 1/2/4 CPUs. Every run exited 33 and emitted its
required pass marker.

Stage 18.1:

- `audit-artifacts/stage18.1-1cpu-1791958078399045900`
- `audit-artifacts/stage18.1-2cpu-1791958154582784300`
- `audit-artifacts/stage18.1-4cpu-1791958224128481000`

Stage 19.1:

- `audit-artifacts/stage19.1-1cpu-1791958284631266200`
- `audit-artifacts/stage19.1-2cpu-1791958401331279900`
- `audit-artifacts/stage19.1-4cpu-1791958464093830200`

Stage 19.3:

- `audit-artifacts/stage19.3-1cpu-1791958520133014600`
- `audit-artifacts/stage19.3-2cpu-1791958674407996000`
- `audit-artifacts/stage19.3-4cpu-1791958727747729400`

Stage 19.10:

- `audit-artifacts/stage19.10-1cpu-1791958777408129100`
- `audit-artifacts/stage19.10-2cpu-1791958848733373300`
- `audit-artifacts/stage19.10-4cpu-1791958904921519500`

Dedicated compositor stages:

Stage 19.11:

- `target/hello-gui-wm-1cpu`
- `target/hello-gui-wm-2cpu`
- `target/hello-gui-wm-4cpu`

Stage 19.12:

- `target/hello-gui-pointer-1cpu`
- `target/hello-gui-pointer-2cpu`
- `target/hello-gui-pointer-4cpu`

Stage 19.13:

- `target/hello-gui-alpha-1cpu`
- `target/hello-gui-alpha-2cpu`
- `target/hello-gui-alpha-4cpu`

Stage 19.14:

- `target/hello-gui-scaled-1cpu`
- `target/hello-gui-scaled-2cpu`
- `target/hello-gui-scaled-4cpu`

All twelve dedicated runs exited 33 and emitted their required pass marker.
The dedicated harness is `scripts/test-hello-gui-compositor-qemu.py`; these
runs use the release `hello-gui` image and the local QEMU UEFI firmware.

These are software/QEMU gates. USB HID pointer decoding, multiple-monitor
hardware, GPU acceleration, and physical display qualification remain explicit
hardware-dependent limits.
