# Stage 31 low-resource device-path evidence — 2026-10-22

The existing Stage 15.2 ATA/WovenFS QEMU gate was run against disposable 4
MiB scratch images:

```text
python scripts/test-wovenfs-device.py --cpus 1 --size-mib 4 --timeout 300
python scripts/test-wovenfs-device.py --cpus 4 --size-mib 4 --timeout 300
```

Both runs passed with exit 33. The gate completed device write/readback,
formatting, durable file creation, remount verification, VFS namespace and
authority checks, unlink, and the final WovenFS marker. Evidence is retained
under:

- `target/wovenfs-device-scratch-1cpu-1792676503835808200`
- `target/wovenfs-device-scratch-4cpu-1792676540559963400`

This is stronger than the host-only disk-full test because it exercises the
ATA path and QEMU device. The image is still disposable emulation; physical
media exhaustion, block-device fault injection, and whole-kernel low-RAM
qualification remain open.
