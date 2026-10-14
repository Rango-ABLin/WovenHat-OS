# Stage 13.2 admission routing audit — 2026-10-13

PCI boot discovery now has an explicit discover-and-route admission phase.
`hal::init` publishes a discovery generation through `discover_and_route`; a
bridge-bearing generation is not driver-admissible until the complete
rollback-capable bridge transaction succeeds. A bridge-free generation is
admitted after discovery. Any discovery truncation or routing failure leaves
the admission flag false, and `bind_pci_with_bars` rejects claims fail-closed.

The existing hotplug path continues to route after ordered removal teardown,
so both initial admission and rescan admission use the same bridge transaction
boundary. This avoids exposing a device behind an unconfigured bridge window
to a driver.

Validation completed:

- warning-denying host and freestanding kernel Clippy;
- full Stage 13.2 preservation gate on 1, 2, and 4 CPUs, exit 33:
  `audit-artifacts/stage13.2-1cpu-1791886751085795900`,
  `audit-artifacts/stage13.2-2cpu-1791886761254233800`, and
  `audit-artifacts/stage13.2-4cpu-1791886770270981900`.

QEMU does not expose a valid mappable physical BAR/bridge topology for a
positive hardware routing qualification. This increment proves admission
ordering and fail-closed behavior; physical PCIe bridge-window interoperability
and hotplug remain hardware-dependent.
