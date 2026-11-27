# B3 loader allocation failure boundary

Status: ACCEPTED bounded unit at runtime `952eda65`.

UEFI loader-data allocation now validates zero and arithmetic-overflow sizes,
then maps firmware allocation failure to static firmware-resource diagnostics
before kernel ELF parsing or `ExitBootServices`.

```text
[WBOOT] firmware resource failure: LoaderAllocation
[WBOOT] boot stopped: firmware resource failure
```

The existing disk-missing to TFTP fallback, UEFI mapper, BootInfo, kernel ABI,
capabilities and authority model are unchanged. This unit does not qualify
general disk/TFTP I/O, graphics-mode, page-table, or ExitBootServices failures.

The fixture producer creates a regular 256 MiB `kernel-x86_64`, the maximum
accepted raw kernel size. Under the unchanged 256 MiB QEMU budget it reaches
firmware allocation before ELF parsing. All 16 source-bound cases pass at
1/2/4 CPUs. Serial/QMP/framebuffer evidence is preserved intact at
`D:\WovenHat-OS-audit-boot-loader-b3-20261105`; it was moved from C: after the
first preservation attempt exposed temporary-disk exhaustion. Source hashes in
the manifests match `952eda65`.

Full Windows preservation passes in
`audit-artifacts/acceptance-20261105-074313-292/`: 215 Python tests (six
skips), host Rust tests, 60 repeated boot/memory runs, networking and isolated
Stage 10.4-10.7 at 1/2/4 CPUs. The earlier failed gate is retained as ENOSPC
environment evidence; it did not start preservation QEMU.
