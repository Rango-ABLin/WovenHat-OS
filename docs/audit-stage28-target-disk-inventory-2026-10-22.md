# Stage 28 target-disk inventory audit — 2026-10-22

This slice adds `scripts/inventory-target-disks.py` and focused host tests in
`tests/test_target_disk_inventory.py`. It parses caller-supplied JSON fixtures
representing normalized records, Windows PowerShell `Get-Disk` output, or
Linux `lsblk -J` output. It identifies a uniquely marked system/boot disk and
marks removable, USB, external, and backup-labelled devices as protected
candidates when those signals are present.

The classifier is fail-closed. Missing path/number, serial, or positive
capacity; duplicate serials; or zero/multiple system disks prevent
`safe_target_selection`. The report is informational only and has no operation
that opens a device, executes a discovery command, mounts, initializes,
formats, partitions, writes, or erases media.

Validation for this bounded slice:

- five focused fixture tests pass;
- Python compilation checks pass;
- CLI smoke output is generated from a temporary fixture only;
- no physical disk or command output was collected by the implementation.

This does not implement target selection, partitioning, installation,
rollback, ISO generation, or hardware qualification.
