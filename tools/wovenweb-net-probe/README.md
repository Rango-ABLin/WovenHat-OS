# Native WovenWeb network platform probe

A freestanding Ring-3 executable exercising libwoven::net and libwoven::socks5.
This is an acceptance fixture, not a GUI browser, Tor daemon, TLS stack or VPN.

Run from the repository root with the pinned Rust toolchain and QEMU/OVMF:

```powershell
python scripts/test-wovenweb-network-qemu.py --cpus 1
```

The harness builds this crate from its own directory (so its linker config is
used), embeds its ELF in a feature-isolated kernel, and boots it against a local
SOCKS/HTTP fixture at host loopback port 18085. Do not run concurrent chains using
that port. It writes the boot-image path to the printed evidence directory's
image.txt. Reuse that exact image with --image PATH for the 2- and 4-CPU runs.

Assertions cover denied NetworkIo, remote-name CONNECT, fragmented proxy/HTTP
replies, HTTP body/DOM verification, 256 cancellations, short output refusal,
authentication refusal, an intentionally stalled method reply/deadline, child exit,
and socket/async/completion-port/process cleanup. Packet evidence checks IPv4 UDP
DNS absence in this fixture only. No external Tor relay is contacted. Captures,
logs, checksums and result JSON are retained under audit-artifacts/wovenweb-network-*.

The static native executable is under target/x86_64-unknown-none/release after a
build; kernel and production application catalogs are separate. The gate grants
only Console and NetworkIo to the positive probe and only Console to its denied
probe, using existing user-domain sandbox ceilings. No capability policy or
syscall number is changed. See docs/wovenweb-native-port.md for the full browser
port and remaining platform limits.

The probe opts into `reclaiming-allocator`. `--heap` runs the isolated memory
fixture through `scripts/test-wovenweb-memory-qemu.py`; it needs no NIC. This
allocator is experimental and bounded by the existing VM window. See
`docs/wovenweb-heap-acceptance.md` for validation gaps and ownership limits.
