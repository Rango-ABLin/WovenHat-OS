# AX201 provider and queue-owner audit — 2026-10-17

## Implemented

- AX201 provider sources are identified by the exact API-77 filename
  `iwlwifi-QuZ-a0-hr-b0-77.ucode`; a mismatched provider name is rejected
  before parsing or DMA allocation.
- The driver copies validated firmware sections into owned DMA. The external
  firmware file is ignored by Git and is not embedded in the repository.
- `Ax201QueueOwner` owns contiguous DMA for the 512-entry 16-byte MQ RX free
  transfer ring and 512-entry 32-byte used-completion ring, a status page, a
  32-entry command ring, and 512 4 KiB RX buffers. Free descriptors contain
  the rbid and physical buffer address; used completions contain the bounded
  rbid/flags record defined by the 22000 transport.
- `Ax201Driver` attaches this owner, publishes its addresses through
  context-info, and refuses startup without it. Queue storage is retained
  until the caller explicitly marks hardware quiesced.

## Evidence

- `cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features
  stage13-9-test -- -D warnings`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `run-stage13-9-acceptance.ps1`: passed on 1, 2 and 4 CPUs; exit 33.
- The post-command-path acceptance rerun passed on 1, 2 and 4 CPUs; evidence
  is in `stage13.9-1cpu-1792249718248983200`,
  `stage13.9-2cpu-1792249737034990100`, and
  `stage13.9-4cpu-1792249755856501300`.
- The post-ALIVE command-probe acceptance rerun passed on 1, 2 and 4 CPUs;
  evidence is in `stage13.9-1cpu-1792250404222125300`,
  `stage13.9-2cpu-1792250423036706000`, and
  `stage13.9-4cpu-1792250444352634200`.
- The MSI/DMA lifecycle acceptance rerun passed on 1, 2 and 4 CPUs; evidence
  is in `stage13.9-1cpu-1792251276402667200`,
  `stage13.9-2cpu-1792251347003211700`, and
  `stage13.9-4cpu-1792251417786768400`.
- The four-command ownership acceptance rerun passed on 1, 2 and 4 CPUs;
  evidence is in `stage13.9-1cpu-1792252270222258500`,
  `stage13.9-2cpu-1792252302535149100`, and
  `stage13.9-4cpu-1792252335365257400`.
- Dedicated `physical-wifi` kernel Clippy passed with the external firmware
  path set to `firmware\iwlwifi-QuZ-a0-hr-b0-77.ucode`.
- The concrete 22000 MMIO adapter rings the MQ RX shadow write-index doorbell
  at PRPH `0x1c80` with the 512-entry, 8-aligned initial index before
  `CPU_INIT_RUN`; synthetic startup I/O remains a no-op boundary.
- The rebuilt firmware-backed image was written only to the verified USB
identity (VendorCo ProductCode USB Device, serial
  `4045141272666606378`, 15,726,735,360 bytes). The rebuilt 42,008,576-byte
  candidate image SHA-256 is
  `c341342e6e5acce321279c0bd6d69094e26655412fbcc8d41a93c69ba037e0e1`, and
  the Administrator writer's raw read-back matched. The RX path rejects a
  completion whose device-written rbid is outside the owned buffer table
  before copying from the selected buffer.
- Physical context-info startup now reads the Intel `CSR_HW_REV` register at
  BAR offset `0x28` and supplies that hardware revision as `version.mac_id`,
  matching the upstream 22000 context-info contract rather than using the PCI
  revision byte.
- The separate read-only inventory probe now reports that same CSR word as
  `HW_REV`, without enabling DMA, MSI, reset or radio operation.
- The physical provider hashes the external firmware before PCI activation and
  rejects anything other than the approved API-77 SHA-256
  `f866c78d5bc3b8f53964bd4c2fcfdef5b06ea94f6657e00adc570ca1b4904104`.
- The activation probe reads and logs the AX201 CSR MAC address pair at
  offsets `0x380` and `0x384` without treating the value as radio acceptance.
- The AX201 command queue now allocates the upstream 22000 256-byte TFD
  stride; its 32-entry ring therefore has 8 KiB of descriptor storage. The
  context-info command-queue size is the upstream `ilog2(entries) - 3`
  encoding (`2`), not
  the literal entry count.
- The concrete physical MMIO adapter now programs the 22000 RFH free/used/status
  queue base registers and resets the queue cursors before the RX write-index
  doorbell.
- The physical RX poller now consumes the device's closed-RBD cursor in order,
  validates each completion rbid, copies the owned buffer, writes the 16-byte
  free descriptor back, and advances the RFH doorbell on the required
  eight-descriptor cadence. The queue-owner self-test covers this requeue.
- The command path now has a bounded four-outstanding-command owner. It
  writes short or wide command headers, retains each copied payload in DMA,
  builds the 256-byte `iwl_tfh_tfd` at queue 9, rings the command queue, and
  releases each payload only after its exact response sequence matches.
  Unsolicited or stale RX notifications cannot reclaim command storage.
- The command descriptor is built through a shared checked TFD encoder. It
  bounds the list to 25 transfer buffers, rejects zero lengths and addresses
  above the 36-bit AX201 transport limit, and is covered by the queue-owner
  self-test before publication.
  The firmware layer now has typed scan-control wire contracts for scan
  configuration, abort, and scan-complete notifications. It rejects invalid
  chains, zero UIDs, wrong notification group/command, and wrong payload
  lengths before interpreting fields; this is not yet a complete scan request
  or radio qualification.
  The scan-control layer now includes the explicit API-17 passive UMAC request
  encoder (maximum 67 validated channels) and a per-iteration completion
  parser whose length check covers every 8-byte channel-result record. It
  rejects direct-SSID active probing until probe-frame and MAC/PHY context
  setup are implemented.
- Scan command ownership now retains one active UID per driver instance.
  Overlapping requests, mismatched aborts, and stale unsolicited notifications
  are rejected; matching completion retires the session. Stage 13.10AG covers
  this lifecycle in the AX201 driver self-test.
- Physical activation now runs after IDT/APIC initialization, enables PCI
  memory decoding and bus mastering, programs the identity-specific AX201 MSI
  vector, enables CSR RX causes, services deferred interrupt work while
  polling the owned RX ring, and disables MSI/CSR delivery before restoring
  the saved PCI command state.
- After a real ALIVE, the physical probe submits the upstream statistics
  command (`0x9c`) with periodic notifications disabled and waits for its
  sequence-matched response. This distinguishes firmware startup from a
  functioning host-command round trip without issuing scan or association.
- A follow-up audit against the upstream 22000 transport confirms that
  `RFH_QUEUE_CONFIG_CMD` is only needed for additional RX queues. This driver
  intentionally owns one default RX queue, which is described by context-info;
  it does not fabricate a post-ALIVE command response path for a command that
  the single-queue configuration does not require. The upstream command is
  `WIDE_ID(DATA_PATH_GROUP, RFH_QUEUE_CONFIG_CMD)` (`0x050D`) and its payload
  layout is retained as a prerequisite if multi-queue RSS is added later.
- The image was written and raw-read back again on the guarded USB identity
  after the four-command lifecycle correction. The 42,008,576-byte image
  SHA-256 is
  `c341342e6e5acce321279c0bd6d69094e26655412fbcc8d41a93c69ba037e0e1`; the
  transcript is
  `audit-artifacts/ax201-physical-activation-usb-write-four-command-20261017.txt`.
- The scan-control increment passed the Stage 13.9 1/2/4-CPU matrix again.
  Its 42,008,576-byte physical image SHA-256 is
  `11b20a98a11fbb76be39f38a522836986678ac6f65000ca0446abd0511dd5458`; the
  guarded write and raw read-back passed in
  `audit-artifacts/ax201-physical-activation-usb-write-scan-control-20261017.txt`.
- The follow-up API-17 passive scan-request/iteration-parser increment passed
  the Stage 13.9 1/2/4-CPU matrix; evidence is in
  `stage13.9-1cpu-1792254320382147300`,
  `stage13.9-2cpu-1792254348951409300`, and
  `stage13.9-4cpu-1792254377249682400`. Its 42,008,576-byte physical image
  SHA-256 is
  `a0938b0135fb0284a9dcf63601d0e9cd0e91ba953c84ba4031fb8e914d5b27ee`; the
  guarded write and raw read-back passed in
  `audit-artifacts/ax201-physical-activation-usb-write-scan-request-20261017.txt`.
- The scan-session ownership increment passed the Stage 13.9 1/2/4-CPU
  matrix; evidence is in `stage13.9-1cpu-1792254782439869700`,
  `stage13.9-2cpu-1792254811748979300`, and
  `stage13.9-4cpu-1792254842497801100`. Its 42,008,576-byte image SHA-256 is
  `5b5fd671fc67b48cc3e55cb778c5cab3cdd56cc3e92512d67657857f8eacfaf9`; the
  guarded write and raw read-back passed in
  `audit-artifacts/ax201-physical-activation-usb-write-scan-session-20261017.txt`.
- The MAC/binding and command-status increment passed the Stage 13.9 matrix;
  evidence is in `stage13.9-1cpu-1792256208913421100`,
  `stage13.9-2cpu-1792256236492856300`, and
  `stage13.9-4cpu-1792256264078602300`. The physical image candidate is
  42,008,576 bytes with SHA-256
  `bde089b3b692fc585ff6d9cda73a81a765f69aca321f9f0aed5cf8722665c312`.
  It has not been written to USB from the non-Administrator shell.
- The final command-response reclamation correction passed the complete
  Stage 13.9 1/2/4-CPU chain; evidence is in
  `stage13.9-1cpu-1792256412816237100`,
  `stage13.9-2cpu-1792256436910132400`, and
  `stage13.9-4cpu-1792256460770673800`.
- The physical passive-scan sequence increment passed the complete Stage 13.9
  matrix; evidence is in `stage13.9-1cpu-1792256636696324900`,
  `stage13.9-2cpu-1792256661182426300`, and
  `stage13.9-4cpu-1792258543111313700`. This is still synthetic evidence;
  no returned laptop boot log has yet verified the scan.
- The API-1 PHY-context wire-boundary increment passed host/all-target
  warning-denying Clippy and the Stage 13.9 1/2/4-CPU matrix. Evidence is in
  `stage13.9-1cpu-1792255478429817700`,
  `stage13.9-2cpu-1792255523334399800`, and
  `stage13.9-4cpu-1792255551838581300`. The encoder is checked and callable
  through `Ax201Driver`, but the physical probe does not issue it yet.
- The matching physical candidate image built successfully with SHA-256
  `8774dc764c9042acbf7f9b94258014707ee91b63a8e3addb5d35a26f89bcd180`.
  The guarded write was refused by the non-Administrator shell, so the
  prior successfully written scan-session image remains the hardware-test
  artifact. The refusal transcript is
  `audit-artifacts/ax201-physical-activation-usb-write-phy-context-20261017.txt`.
- The active-scan increment adds a checked API-17 broadcast probe template,
  directed-SSID array support, and per-channel SSID selection. The physical
  path now submits the active broadcast scan after PHY/MAC/binding setup. Host
  and all-target warning-denying Clippy passed, and the Stage 13.9 matrix
  passed with evidence in `stage13.9-1cpu-1792259066847956700`,
  `stage13.9-2cpu-1792259146012605800`, and
  `stage13.9-4cpu-1792259181589741900`. This remains synthetic until the
  rebuilt image is written and a laptop boot log is returned.
- The RX increment adds AX201 `REPLY_RX_MPDU_CMD` descriptor validation,
  CRC/length checks, RSSI/channel extraction, and physical routing of valid
  beacon/probe-response frames into `Manager`. Host/all-target warning-denying
  Clippy and the full host test suite pass; physical returned-frame evidence
  remains outstanding.
- The current physical image after RX MPDU ingestion is 42,008,576 bytes with
  SHA-256 `f0c1d430652071c8f917169da37acc281d2493218db75390ee9253a14f286073`.
  The corresponding Stage 13.9 evidence is
  `stage13.9-1cpu-1792259384946552100`,
  `stage13.9-2cpu-1792259451253593400`, and
  `stage13.9-4cpu-1792259489563484400`.
  
- The TX boundary increment adds a bounded 22000 TX_CMD v9 management-MPDU
  encoder. It intentionally does not route TX_CMD through the host-command
  queue; a dedicated data-TX queue owner is still required. The matrix passed
  with evidence in `stage13.9-1cpu-1792259723636441300`,
  `stage13.9-2cpu-1792259774430287500`, and
  `stage13.9-4cpu-1792259815230643000`. The current image SHA-256 is
  `bddc86b900dfd7525cf59f7ba9917e8a7e71f6de1cd387574c20c7194828d75b`.

- The dedicated data-TX increment adds a separate 32-entry TFD ring and
  retains each encoded TX payload in owned DMA until an exact queue/index
  completion. It accepts only Intel management queues 5–8 or data queues
  10–30, rejecting queue 9 and the RX queue as TX destinations, and
  does not route TX_CMD through the host-command path. The driver boundary is
  covered by the queue-owner self-test; physical DQA queue configuration and
  TX notification decoding remain open. The Stage 13.9 matrix passed with
  evidence in `stage13.9-1cpu-1792260873010961800`,
  `stage13.9-2cpu-1792260907652843800`, and
  `stage13.9-4cpu-1792260941804531300`. The candidate image is
  43,057,152 bytes with SHA-256
  `3c8220bd850e82171863c13daed3861b92184fdeb1db74109019dec645c727bc`.

- The latest wire-ABI increment corrects the TX_CMD v9 offsets, adds checked
  DQA-enable and API-v3 scheduler-queue encoders, allocates the required
  byte-count table, and decodes bounded single-frame `REPLY_TX` responses by
  the queue/index sequence in the RX header. The physical probe now attempts
  DQA enable and queue allocation after ALIVE and remains fail-closed unless
  the firmware returns a valid queue. It does not yet transmit an arbitrary
  management frame. Stage 13.9 evidence is in
  `stage13.9-1cpu-1792261608259229200`,
  `stage13.9-2cpu-1792261642569914800`, and
  `stage13.9-4cpu-1792261677212040100`. The current candidate image is
  43,057,152 bytes with SHA-256
  `60c7323329d7eeb11407384c087cd40a4cada154186036041660416202cefd29`.

- The physical probe now has a bounded host-owned management-TX exercise:
  after a successful scan it builds Open-System authentication for the first
  scan result, submits corrected TX_CMD v9 through the firmware-configured
  queue, waits for `REPLY_TX`, reclaims the exact queue/index slot, and resets
  link state without attempting association or credentials. Stage 13.9 passed
  with evidence in `stage13.9-1cpu-1792265662237988600`,
  `stage13.9-2cpu-1792265696327245000`, and
  `stage13.9-4cpu-1792265730806233400`. The candidate image is
  43,057,152 bytes with SHA-256
  `20bc396f058d6b4a120fbbad5008c2c35b227d585ad11b8ba38b6df57dd233b4`.

## Open gates

This is not physical activation. Physical BAR mapping/MSI attachment,
device-produced ALIVE, interrupt recovery, radio qualification, scan and
association remain unaccepted. The authorized USB physical probe is still
awaiting a returned boot photograph/log; the repeat write/read-back itself is
media evidence, not device execution evidence.

## Successful physical image write

The current 43,057,152-byte candidate was subsequently written from an
elevated PowerShell to the uniquely identified USB target
(`VendorCo ProductCode USB Device`, 15,726,735,360 bytes,
serial `4045141272666606378`). The writer verified the image-region
read-back SHA-256 as
`ad6f4de25a53fddafee14ed30b0133a391035b081ff933d74c68139fc444bb3f`,
created the FAT32 `WOVENLOG` partition, and reported `DONE`. The complete
transcript is
`audit-artifacts/ax201-physical-write-20261017.txt`.

This closes the media-preparation gate only. Device-produced ALIVE, interrupt,
scan, and TX completion evidence still requires booting the laptop from the
USB and returning `LOGS/BOOT*.TXT`.
