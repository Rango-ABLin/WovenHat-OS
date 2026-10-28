# Display HAL and modesetting — Stage 02 evidence

Status: **ACCEPTED for supported QEMU modes**
Date: 2026-10-25  
Prerequisite: Stage 01 accepted in commit `1f5ba9e3`

## Implemented boundary

The current display path has real, bounded implementations in:

- `kernel/src/graphics_abi.rs` for backend-neutral pixel formats, output IDs,
  validated display modes, presentation fences, and recovery errors.
- `kernel/src/display.rs` for UEFI framebuffer formats, stride-safe writes, and
  CPU fallback presentation.
- `kernel/src/compositor.rs` for framebuffer presentation, damage clipping,
  VirtIO-GPU output attachment, scanout resource presentation, and fallback.
- `kernel/src/virtio_gpu.rs` for the VirtIO-GPU transport and mode/resource
  operations.

Commit `4062f3eb` adds a bounded one-shot VirtIO reprobe/resource rebuild after
presentation failure. If reprobe fails, the compositor drops the GPU binding
and presents through the UEFI framebuffer CPU fallback while retaining pending
damage. This closes the source-level recovery/fallback design gap; the
fault-injected guest qualification remains open.

## Executed QEMU evidence

The existing real-QEMU VirtIO harnesses passed serially, with pixel captures
and serial logs retained:

- Single-output scanout: 1 CPU
  `audit-artifacts/stage20.14s-1cpu-1792922165001067900`
- Single-output scanout: 2 CPU
  `audit-artifacts/stage20.14s-2cpu-1792922220611005500`
- Single-output scanout: 4 CPU
  `audit-artifacts/stage20.14s-4cpu-1792922238078921800`
- Two-head scanout: 1 CPU
  `audit-artifacts/stage20.14-1cpu-1792922262079247000`
- Two-head scanout: 2 CPU
  `audit-artifacts/stage20.14-2cpu-1792922322133186600`
- Two-head scanout: 4 CPU
  `audit-artifacts/stage20.14-4cpu-1792922352145022100`

Those runs verify actual host-captured scanout and exact expected pixels. They
do not prove that every physical mode from 1024×768 through 1920×1080 is
available in this QEMU profile, and they do not inject a GPU reset/resource
loss and verify recovery to a known-good scanout.

Commit `5959b411` adds the Stage 02 recovery feature gate and serial mode
diagnostics. The fault-injected recovery gate passed at 1/2/4 CPUs:

- `audit-artifacts/stage20.14s-1cpu-1792923022824676600`
- `audit-artifacts/stage20.14s-2cpu-1792923037292569200`
- `audit-artifacts/stage20.14s-4cpu-1792923054166918300`

The implementation accepts device-advertised modes up to 1920x1080 per output
within the bounded compositor surface, and the current QEMU profile advertises
1280x800. The required QEMU-supported scanout and recovery gate is therefore
**accepted**. Physical EDID, native 1920x1080 hardware, and physical GPU
certification remain outside this emulator acceptance.

