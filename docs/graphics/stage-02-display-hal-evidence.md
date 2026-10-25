# Display HAL and modesetting — Stage 02 evidence

Status: **PARTIAL / NOT ACCEPTED**  
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

## Gate decision

Stage 02 remains **partial**. The CPU/UEFI fallback and QEMU VirtIO scanout
are working, but the required EDID/mode diagnostics and error-recovery
qualification are not yet implemented or evidenced. No physical display/GPU
certification is claimed.

