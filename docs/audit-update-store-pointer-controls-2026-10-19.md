# WovenUpdate and WovenStore pointer-control audit — 2026-10-19

## Result

PASS. Both clients now expose a bounded read-only `Refresh` control. A
primary-button release delivered by WovenShell through `WPBT` re-reads the
client's bounded view, increments a visible counter, and repaints the shared
surface. The QEMU harness requires the serial activation marker and a
presented pixel delta.

## Geometry and semantics

- WovenUpdate surface: 360x200; button local x=250..329, y=145..164.
- WovenStore surface: 360x220; button local x=250..329, y=170..189.
- Update refreshes `/updates` preflight state; Store refreshes the installed
  `/share/applications` catalog.
- Neither control installs packages, mutates the catalog, or writes storage.

## QEMU evidence

The production harness passed each run:

```text
python scripts/test-files-desktop-qemu.py --app update --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app update --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app update --exercise-client-click --cpus 4
python scripts/test-files-desktop-qemu.py --app store --exercise-client-click --cpus 1
python scripts/test-files-desktop-qemu.py --app store --exercise-client-click --cpus 2
python scripts/test-files-desktop-qemu.py --app store --exercise-client-click --cpus 4
```

Evidence directories:

- `audit-artifacts/update-desktop-1cpu-1792445649734601500`
- `audit-artifacts/update-desktop-2cpu-1792445719209523000`
- `audit-artifacts/update-desktop-4cpu-1792445783243814900`
- `audit-artifacts/store-desktop-1cpu-1792445846593054300`
- `audit-artifacts/store-desktop-2cpu-1792445912873180400`
- `audit-artifacts/store-desktop-4cpu-1792445978619929300`

## Limits

This closes the functional pointer-control slice for all ten packaged clients.
USB HID qualification, physical hardware testing, and hardware GPU
presentation remain separate hardware-dependent gates.
