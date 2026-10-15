# Stages 13.17–13.19: live hardware testing audit — 2026-10-15

Three stages that make a WovenHat USB stick safe to boot on someone else's
machine, and make that boot report back.
- **13.17:** disks WovenHat does not own are never written.
- **13.18:** USB mass storage.
- **13.19:** the boot's log kept on the stick.

They extend the Stage 13 driver closure (roadmap 13.5, USB transfers) and
prepare roadmap Stage 29, real hardware qualification.

## Stage 13.17 — disks WovenHat does not own are never written

**The defect.** Without an ATA disk, a normal boot mounted the first
ready NVMe or AHCI disk's FAT32 volume at `/mnt`, writable.
- On a Windows PC that volume is the EFI system partition.
- Any file written, changed or deleted under `/mnt` went through to the
  other system's boot partition.

**The rule.**
- A root device is writable only when the FAT32 volume found on it is
  labelled `WOVENHAT` (WovenHat's data disks) or `WOVENLOG` (the
  live-test log partition).
- `storage::mount_ata_root` reads the label (`fat32_label`) before
  mounting and records ownership (`block_io::set_root_owned`).
- `block_io::with_root`, the path of every root writer, wraps the device
  in a guard:
  - `write_sector` returns `Error::ReadOnly`;
  - `is_read_only` is true;
  - `flush` writes nothing.
- This covers file write-back, journal recovery, swap and raw block
  syscalls alike, and a foreign volume imports read-only.
- Test boots keep the ATA primary master writable, because several gates
  write raw sectors on scratch disks that hold no file system.
- WovenFS writes only volumes carrying a valid WovenFS superblock, which
  only WovenHat creates.

**AHCI takeover from firmware (bug found by this gate).**
- With a firmware variable store, OVMF uses the AHCI controller during
  boot and stops the port. The disk then reads signature `0xFFFFFFFF`
  with an active link, and the driver skipped it (`NoSataPort`).
- Real firmware always uses the controller, so every real SATA machine
  was likely affected.
- The driver now takes each port over in this order:
  1. stop the port;
  2. install its own buffers;
  3. start FIS receive;
  4. COMRESET if the link is not active or the signature is not ATA;
  5. wake a link left in a power-saving state.
- In QEMU, steps 1–3 are enough: once FIS receive runs, the disk reports a
  fresh signature. The gate therefore does not exercise COMRESET or the
  link wake; they remain fallbacks for hardware that needs them.

**Diagnostics.** Driver bring-up now logs every failing driver's specific
error (`[DRIVERS] <name> init error: …`). The outcome alone gives a field
tester nothing to act on.

**Gate** (`scripts/test-stage13-17-foreign-disk.py`). The data disk is a
FAT32 volume formatted as another system would format it: OEM
`MSDOS5.0`, label `NO NAME`, one file.
- Modes:
  - `nvme` and `ahci` (stage13-17-test kernel): the volume mounts and its
    file reads. Changing the file, creating a file, a raw sector write
    and a flush are then attempted, and every write must be refused.
  - `production`: a normal kernel mounts the disk and reaches the shell.
- The host requires the image to be byte-for-byte unchanged.
- OVMF gets its own variable store. The first run showed that without
  one the firmware writes `NvVars` to the first FAT volume it finds.

## Stage 13.18 — USB mass storage

**Protocol (`kernel/src/usb_msc.rs`, host tests `tests/usb_msc.rs`).**
- Bulk-Only SCSI interface discovery.
- Command Block Wrappers, and Command Status Wrapper validation:
  signature, tag, residue and status.
- SCSI commands: INQUIRY, TEST UNIT READY, REQUEST SENSE, READ CAPACITY
  (10), READ/WRITE (10) and SYNCHRONIZE CACHE.

**Transport (`kernel/src/xhci.rs`).**
- Enumeration reads each device's configuration once; a mass-storage
  interface becomes the USB disk.
- Attach sequence:
  1. SET_CONFIGURATION, then bulk endpoint contexts.
  2. INQUIRY.
  3. TEST UNIT READY until ready, with REQUEST SENSE between attempts. A
     reader reporting no medium stops early.
  4. READ CAPACITY. Only 512-byte blocks are accepted.
- Stalls are cleared with Reset Endpoint, Set TR Dequeue Pointer and
  CLEAR_FEATURE(ENDPOINT_HALT).
- `UsbDisk` implements `BlockDevice`. Each block takes the controller lock
  for one command, so interrupts are never masked for longer than one
  block.

**Shared event ring.**
- Storage transfers and the USB input worker share the controller's event
  ring.
- While a storage command waits, input reports it drains are routed: the
  device is re-armed and the report stashed. The input worker delivers
  them on its next pass.

**Ring wrap fix.**
- `Ring::push` used to flip its cycle bit before rewriting the link TRB,
  so the link carried the new cycle.
- A controller that had not yet fetched the link would stop at it, which
  could freeze a USB keyboard after its ring wrapped (every 255 reports).
- QEMU prefetches the link, so the gates never showed it.
- The link is now written with the consumer's cycle first.

**Gate** (`scripts/test-stage13-18-usb-storage.py`). A patterned 32 MiB
image is attached as `usb-storage` beside a `usb-kbd`. The kernel must:
1. identify QEMU's disk and its capacity;
2. read the pattern back, first and last blocks;
3. write 64 blocks, flush, and read them back;
4. refuse a block past the end;
5. verify 300 or more further reads, wrapping both bulk rings, while
   "usb" plus Enter typed over QMP arrives in order.

The host requires exactly the 64 written sectors to have changed, each
holding the kernel's record.

## Stage 13.19 — the boot's log on the WovenHat USB stick

**Kernel log.** `serial.rs` keeps every byte written to COM1 in a 256 KiB
lock-free ring (`log_position`, `read_log`).

**`kernel/src/livelog.rs`.**
- It finds the FAT32 partition labelled `WOVENLOG` on the USB disk
  (`gpt::fat_partitions`). That volume is the only one it writes.
- It creates `LOGS/BOOTnnnn.TXT` per boot, preallocated to 512 KiB:
  - Updates rewrite data sectors in place, with no FAT or directory
    changes after creation.
  - The first 4 KiB is a summary rewritten on every flush: boot number,
    machine clock, kernel version, and every log line reporting a
    failure, up to 40.
  - The kernel log follows, padded with spaces.
- It writes a hardware inventory: CPU vendor and brand string, logical
  CPUs, memory, display mode, every PCI function, and the USB disk.
- A worker flushes every 3 s. The panic handler flushes once more if the
  log and the controller lock are free.

**Kernel shell commands.**
- `note <text>` records a tester's observation and saves it at once.
- `savelog` flushes on demand.

**The stick.** `scripts/write-live-usb.ps1` writes the release-validated
image, verifies it by read-back SHA-256, then:
1. creates the `WOVENLOG` FAT32 partition in the rest of the stick;
2. checks the label in its boot sector;
3. adds a README.

**Gate** (`scripts/test-stage13-19-livelog.py`). It builds a stick as the
writer leaves a real one: GPT, an EFI partition, then the FAT32
`WOVENLOG` partition. It reads the log back with its own FAT32 reader.
- Modes:
  - `test`: a tester's note, a reported failure that must reach the
    summary, and a 200-line burst.
  - `production`: a normal boot reaches the shell, and the log holds the
    inventory and the boot messages within one flush period.
  - `unlabelled`: the partition is labelled `NO NAME`, and the stick must
    not change at all.
- In every mode, everything before the log partition must be unchanged.

## Evidence (`audit-artifacts/`)

All gates on 1/2/4 CPUs:
- **Stage 13.17:** `stage13.17-nvme-1cpu-1792065092555576200`, `stage13.17-nvme-2cpu-1792064222063722500`, `stage13.17-nvme-4cpu-1792064675450888300`, `stage13.17-ahci-1cpu-1792070090765846700`, `stage13.17-ahci-2cpu-1792064293120539100`, `stage13.17-ahci-4cpu-1792064755571914200`, `stage13.17-production-1cpu-1792063844090949600`, `stage13.17-production-2cpu-1792064338888488100`, `stage13.17-production-4cpu-1792064804937379700`
- **Stage 13.18:** `stage13.18-1cpu-1792065576766775400`, `stage13.18-2cpu-1792064417807942400`, `stage13.18-4cpu-1792064863140471100`
- **Stage 13.19:** `stage13.19-test-1cpu-1792065740723695600`, `stage13.19-test-2cpu-1792064505542364800`, `stage13.19-test-4cpu-1792064929004817200`, `stage13.19-production-1cpu-1792064101415762000`, `stage13.19-production-2cpu-1792064590889875700`, `stage13.19-production-4cpu-1792064994848484100`, `stage13.19-unlabelled-1cpu-1792065674471097500`, `stage13.19-unlabelled-2cpu-1792064637008146600`, `stage13.19-unlabelled-4cpu-1792065039369906000`

Chain results (lint, host tests, gates, negative controls, regression):

```
lint-all-features: exit 0 []
host tests: ok
libwoven tests: ok
13.17 nvme 1cpu: PASS (72s) 
13.17 ahci 1cpu: PASS (39s) 
13.17 production 1cpu: PASS (44s) 
13.18 1cpu: PASS (85s) 
13.19 test 1cpu: PASS (117s) 
13.19 production 1cpu: PASS (69s) 
13.19 unlabelled 1cpu: PASS (42s) 
13.17 nvme 2cpu: PASS (66s) 
13.17 ahci 2cpu: PASS (43s) 
13.17 production 2cpu: PASS (75s) 
13.18 2cpu: PASS (80s) 
13.19 test 2cpu: PASS (82s) 
13.19 production 2cpu: PASS (44s) 
13.19 unlabelled 2cpu: PASS (35s) 
13.17 nvme 4cpu: PASS (75s) 
13.17 ahci 4cpu: PASS (46s) 
13.17 production 4cpu: PASS (56s) 
13.18 4cpu: PASS (61s) 
13.19 test 4cpu: PASS (63s) 
13.19 production 4cpu: PASS (42s) 
13.19 unlabelled 4cpu: PASS (51s) 
negative control 13.17 guard lets writes through: caught ['the foreign disk image changed: ccf8de5dcf0f4c3043f781a1ce1e5bdb1842f0ed8c9250850f4ca7a467d6675a -> ebf83bc6351cc0570b65b5e9037c8053898500f9104e2fefb1f8f45672c24b01', '[DRIVERS] ahci0 init error: NoSataPort', '[S13.17] changing its file is refused FAIL', '[S13.17] creating a file is refused FAIL']
negative control 13.17 AHCI port taken only if already active with an ATA signature: NOT CAUGHT ['Stage 13.17 ahci: PASS (1 CPU); disk unchanged. Evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.17-ahci-1cpu-1792065159614502500']
negative control 13.18 no keys typed: caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] verified 105386 reads; keys []', '[S13.18] typed keys arrive once each, in order, during storage I/O FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.18 input events dropped during storage transfers: caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] verified 95120 reads; keys []', '[S13.18] typed keys arrive once each, in order, during storage I/O FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.18 writes land one block late: caught ['changed sectors [4097, 4098, 4099, 4100, 4101, 4102, 4103, 4104]... (64), expected 4096..4159', '[DRIVERS] ahci0 init error: NoSataPort', '[S13.18] 64 blocks written, flushed and read back FAIL', '[S13.18] USB mass storage: FAILED']
negative control 13.19 any FAT32 label accepted: caught ['the stick changed although its partition is not labelled WOVENLOG', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-unlabelled-1cpu-1792065674471097500']
negative control 13.19 issues never reach the summary: caught ['[S13.19] simulated driver FAILURE for the summary', 'FAILED; evidence: C:\\WovenHat-OS-wt\\audit-artifacts\\stage13.19-test-1cpu-1792065740723695600']
13.14 nvme 1cpu: PASS (77s) 
13.14 ahci 1cpu: PASS (58s) 
13.14 production 1cpu: PASS (62s) 
13.12 production 1cpu: PASS (23s) 
13.13 1cpu: PASS (83s) 
13.16 1cpu: PASS (106s) 
13.3 1cpu: PASS (66s) 
13.4 1cpu: PASS (51s) 
13.5 1cpu: PASS (54s) 
13.6 1cpu: PASS (54s) 
storage 1cpu: PASS (73s) 
13.14 nvme 2cpu: PASS (54s) 
13.14 ahci 2cpu: PASS (50s) 
13.14 production 2cpu: PASS (28s) 
13.12 production 2cpu: PASS (22s) 
13.13 2cpu: PASS (50s) 
13.16 2cpu: PASS (52s) 
13.3 2cpu: PASS (39s) 
13.4 2cpu: PASS (40s) 
13.5 2cpu: PASS (48s) 
13.6 2cpu: PASS (43s) 
storage 2cpu: PASS (57s) 
13.14 nvme 4cpu: PASS (76s) 
13.14 ahci 4cpu: PASS (47s) 
13.14 production 4cpu: PASS (28s) 
13.12 production 4cpu: PASS (23s) 
13.13 4cpu: PASS (52s) 
13.16 4cpu: PASS (59s) 
13.3 4cpu: PASS (44s) 
13.4 4cpu: PASS (44s) 
13.5 4cpu: PASS (52s) 
13.6 4cpu: PASS (38s) 
storage 4cpu: PASS (57s) 
20.1 1cpu: PASS (61s) 
20.14b 1cpu: PASS (93s) 
release: PASS (2278s) 
DONE
negative control 13.17 AHCI judged before takeover (old order): caught ['[DRIVERS] ahci0 init error: NoSataPort', '[S13.17] a storage driver is ready FAIL']
```

The release gate passed 100/100 (`audit-artifacts/release-validation-1792070090/`); its validated image is the one written to the live-test stick.

**Negative controls.** Each ran on 1 CPU with the source restored byte for
byte afterwards:
- **The guard lets writes through:** the host saw the image change, and
  three kernel steps failed.
- **AHCI port judged before the takeover** (the old order): `NoSataPort`.
  A first version of this control removed only the COMRESET, and it was
  not caught, for the reason given under Stage 13.17. The corrected
  control was run after the chain.
- **No keys typed during storage I/O.**
- **Input events dropped during storage transfers:** the keys never
  arrived.
- **Writes landing one block late:** the host sector check failed.
- **Any FAT32 label accepted:** the unlabelled stick changed.
- **Issues never reaching the summary.**

## Limits

- **13.17.**
  - Ownership is by volume label. A foreign volume that happens to be
    labelled WOVENHAT or WOVENLOG is treated as WovenHat's.
  - Test boots exempt the ATA primary master.
- **13.18.**
  - One disk, one LUN, on a root port. Hubs and UAS are not supported.
  - Only 512-byte blocks addressable by 10-byte commands, up to 2 TiB.
  - Polled completion, one block per command, so transfers are slow.
  - No Bulk-Only reset recovery after a phase error.
- **13.19.**
  - The kernel log ring holds 256 KiB, and earlier bytes are lost if the
    first flush comes later.
  - The file holds 508 KiB of log per boot.
  - A panic flush is skipped if the panicking code holds the USB or log
    lock.
  - Logs are written only when a USB disk is attached at boot.
