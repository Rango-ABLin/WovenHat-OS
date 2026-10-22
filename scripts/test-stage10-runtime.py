"""Run isolated completion-port/timer/runtime acceptance with durable logs."""
import argparse
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import time

# Explicit acceptance contract; do not derive expectations from the tested source at runtime.
WIFI_REQUIRED_MARKERS = (
    '[S13.9A] WovenWiFi framework + PCI classification: PASSED',
    '[S13.9B] IEEE 802.11 frame/IE core: PASSED',
    '[S13.9C] beacon/probe scan pipeline: PASSED',
    '[S13.9D] Open System authentication + association: PASSED',
    '[S13.9E] RSN + EAPOL-Key protocol foundation: PASSED',
    '[S13.9F] WPA2 cryptographic foundation: PASSED',
    '[S13.9G] WPA2 4-way handshake integration: PASSED',
    '[S13.9H] GTK + encrypted key data: PASSED',
    '[S13.9I] CCMP protected data path: PASSED',
    '[S13.9J] WovenWiFi <-> WovenNet integration: PASSED',
    '[S13.9X] GTK/group-addressed CCMP data path: PASSED',
    '[S13.9Y] WPA2 reconnect/rekey lifecycle: PASSED',
    '[S13.9Z] WPA2 live group-key rekey: PASSED',
    '[S13.10A] physical PCI Wi-Fi backend boundary: PASSED',
    '[S13.10B] Wi-Fi MMIO/DMA/interrupt scaffolding: PASSED',
    '[S13.10C] Wi-Fi real MMIO + DMA memory ownership: PASSED',
    '[S13.10D] Wi-Fi DMA descriptor ownership + queue lifecycle: PASSED',
    '[S13.10E] Wi-Fi supported chipset binding boundary: PASSED',
    '[S13.10F] Wi-Fi DMA buffer ownership binding: PASSED',
    '[S13.10G] Wi-Fi activation authority hardening: PASSED',
    '[S13.10H] Intel AX200 CSR boundary: PASSED',
    '[S13.10I] Intel reset/readiness state machine: PASSED',
    '[S13.10J] Intel MAC access/device initialization: PASSED',
    '[S13.10K] Wi-Fi firmware validation/lifecycle foundation: PASSED',
    '[S13.10L] Intel TLV firmware parser: PASSED',
    '[S13.10M] Intel firmware DMA staging/transfer boundary: PASSED',
    '[S13.10N] Intel firmware transfer executor boundary: PASSED',
    '[S13.10O] Intel 22000 firmware transport contract: PASSED',
    '[S13.10P] AX200 context-info self-load manifest: PASSED',
    '[S13.10Q] AX200 DMA-backed context-info construction: PASSED',
    '[S13.10R] AX200 context-info ABI + publication: PASSED',
    '[S13.10S] AX200 CPU_INIT_RUN startup sequencing: PASSED',
    '[S13.10T] AX200 ALIVE notification validation: PASSED',
    '[S13.10U] contiguous firmware DMA ownership: PASSED',
    '[S13.10V] version-aware AX200 ALIVE ABI: PASSED',
    '[S13.10W] Intel RX notification delivery: PASSED',
    '[S13.10X] Intel RX DMA completion boundary: PASSED',
    '[S13.10Y] Intel RX interrupt service boundary: PASSED',
    '[S13.10Z] PCI device interrupt foundation: PASSED',
    '[S13.10AA] PCI MSI programming contract: PASSED',
    '[S13.10AB] AX200 PCI MSI binding contract: PASSED',
    '[S13.10AC] deferred AX200 IRQ -> CSR RX service: PASSED',
    '[S13.9K] Wi-Fi transport/backend contract: PASSED',
    '[S13.9L] reconnect/timeout/lifecycle hardening: PASSED',
    '[S13.9M] cross-module integration closure: PASSED',
    '[S13.9N] WPA2 security lifecycle integration: PASSED',
    '[S13.9O] kernel entropy/secure SNonce boundary: PASSED',
    '[S13.9P] live WPA2 GTK/KRACK integration: PASSED',
    '[S13.9Q] WPA2 association + RSN integration: PASSED',
    '[S13.9R] entropy/network randomness split: PASSED',
    '[S13.9S] WPA2 supplicant/session key handoff: PASSED',
    '[S13.9T] backend RX/CCMP/Ethernet integration: PASSED',
    '[S13.9U] WovenWiFi/smoltcp transport adapter: PASSED',
    '[S13.9V] selectable WovenNet transport integration: PASSED',
)

def required_markers(stage, cpus):
    required = [f'[SMP] online={cpus} expected={cpus}',
                '[S10.3] userspace async completion ABI + cancellation/teardown: PASSED',
                ({'10.8': '[S10.8] completion ports + batch/cancel/timeout/teardown/SMP: PASSED',
                  '10.9': '[S10.9] timers/events/deadlines/cancellation/teardown: PASSED',
                  '11.1': '[S11.1] production process model: PASSED',
                  '11.2': '[S11.2] threads/TLS/join: PASSED',
                  '11.2-status': '[S11.STATUS] libwoven signed exit status/ownership/teardown: PASSED',
                  '11.2-tls': '[S11.TLS] compiler TLS lifecycle: PASSED',
                  '11.2-aslr': '[S11.ASLR] arena and stack guard: PASSED',
                  '11.3': '[S11.3] notifications: PASSED',
                  '11.4': '[S11.4] libwoven runtime boundary: PASSED',
                  '11.5': '[S11.5] loader hardening: PASSED',
                  '11.5-compiler': '[S11.COMPILER] signed toolchain PIE: PASSED',
                  '12.1': '[S12.1] VFS boundary: PASSED', '12.2': '[S12.2] WovenFS: PASSED',
                  '12.3': '[S12.3] encryption: PASSED', '12.4': '[S12.4] snapshots: PASSED',
                  '12.5': '[S12.5] storage management: PASSED', '13.1': '[S13.1] driver framework: PASSED',
                  '13.2': '[S13.2] PCIe topology + generation-safe ownership/teardown: PASSED',
                  '13.3': '[S13.3] NVMe controller/queue foundation: PASSED',
                  '13.4': '[S13.4] AHCI/SATA DMA block I/O: PASSED',
                  '13.5': '[S13.5] xHCI USB core: PASSED',
                  '13.6': '[S13.6] USB HID keyboard: PASSED',
                  '13.7': '[S13.7] WovenInput unified event framework: PASSED',
                  '13.8': '[S13.8] WovenAudio stream/API integration: PASSED',
                  '13.9': '[S13.9H] GTK + encrypted key data: PASSED',
                  '13.10': '[S13.10T] Bluetooth LE notification minimum key size: PASSED',
                  '13.11': '[S13.11-AUDIT] BLE SMP persistent authentication authority: PASSED',
                  '1-5': '[S1-5] storage journal: PASSED',
                  '15.1': '[S15.1] WovenFS native filesystem: PASSED',
                  '15.3': '[S15.3] permission broker: PASSED',
                  '15.4': '[S15.4] signed application verification: PASSED',
                  '15.5': '[S15.5] secure boot handoff: PASSED',
                  '15.6': '[S15.6] software measurement: PASSED',
                  '15.7': '[S15.7] WovenVault handles: PASSED',
                  '15.8': '[S15.8] security ledger ABI: PASSED',
                  '15.9': '[S15.9] exploit mitigations: PASSED',
                  '17.1': '[S17.1] WovenHat font coverage: PASSED',
                  '18.1': '[S18.1] client surface registry: PASSED',
                  '19.1': '[S19.1] compositor surface: PASSED',
                  '19.3': '[S19.3] compositor presentation: PASSED',
                  '19.10': '[S19.10] display geometry and single writer: PASSED'}[stage])]
    if stage == '11.2-aslr':
        required.extend([
            '[S11.2] scheduler-backed thread: PASSED',
            '[S11.2] threads/TLS/join: PASSED',
        ])
    if stage == '11.2-tls':
        required.extend([
            '[S11.TLS] static compiler TLS: PASSED',
            '[S11.TLS] fresh child TLS: PASSED',
            '[S11.TLS] relocated pointer initializer: PASSED',
            '[S11.TLS] biased PIE initializer relocation: PASSED',
            '[S11.TLS] compiled fork state and fresh descendant: PASSED',
            '[S11.TLS] compiled capacity cancellation and stale joins: PASSED',
            '[S11.TLS] compiled owner-exit sibling armed: PASSED',
            '[S11.TLS] nonzero congruence TLS: PASSED',
            '[S11.TLS] page-edge TCB: PASSED',
            '[S11.TLS] template/TCB rollback and fork protection: PASSED',
            '[S11.2] scheduler-backed thread: PASSED',
            '[S11.2] threads/TLS/join: PASSED',
        ])
        required.extend(f'[S11.TLS] lifecycle CPU {cpu}: PASSED' for cpu in range(cpus))
    if stage == '11.2-status':
        required.extend([
            '[S11.PLACEMENT] fixed ET_EXEC at nonzero ASLR candidate: PASSED',
            '[S11.PLACEMENT] PIE executes with nonzero ASLR: PASSED',
            '[S11.STATUS] libwoven signed process waits and double-reap: PASSED',
            '[S11.STATUS] libwoven signed natural/canceled joins and stale IDs: PASSED',
            '[S11.STATUS] ownership and genuine wait errors: PASSED',
            '[S11.2] scheduler-backed thread: PASSED',
            '[S11.2] threads/TLS/join: PASSED',
        ])
    if stage == '11.5-compiler':
        required.append('[S11.COMPILER] shared-library PT_TLS relocation and per-process template: PASSED')
    if stage == '11.5-compiler':
        required.extend([
            '[S11.COMPILER] actual shared import and exact owner teardown: PASSED',
            '[S11.COMPILER] nonzero-bias shared PIE and teardown: PASSED',
            '[S11.5] loader hardening: PASSED',
        ])
    if stage in ('11.5', '11.5-compiler'):
        required.extend([
            '[S11.5] shared libraries unsigned dependency rejected: PASSED',
            '[S11.5] shared libraries writable dependency rejected: PASSED',
            '[S11.5] shared libraries tampered signature rejected: PASSED',
            '[S11.5] shared libraries SONAME mismatch rejected: PASSED',
            '[S11.5] shared libraries path traversal rejected: PASSED',
            '[S11.5] shared libraries transitive graph and cross-image relocation: PASSED',
            '[S11.5] shared libraries W^X RELRO and fork preservation: PASSED',
            '[S11.5] shared libraries Ring-3 import call and owner teardown: PASSED',
            '[S11.5] shared libraries lazy PLT slot remains unresolved before first call: PASSED',
            '[S11.5] shared libraries lazy PLT first-call resolution and teardown: PASSED',
            '[S11.5] shared libraries lazy PLT load: PASSED',
            '[S11.5] shared libraries DT_REL runtime relocation: PASSED',
            '[S11.5] shared libraries DT_REL Ring-3 execution and teardown: PASSED',
            '[S11.5] shared libraries DT_JMPREL DT_REL runtime relocation: PASSED',
            '[S11.5] shared libraries DT_JMPREL DT_REL Ring-3 execution and teardown: PASSED',
            '[S11.5] shared libraries weak symbol runtime resolution: PASSED',
            '[S11.5] shared libraries weak symbol Ring-3 execution and teardown: PASSED',
            '[S11.5] shared libraries versioned symbol runtime resolution: PASSED',
            '[S11.5] shared libraries versioned symbol Ring-3 execution and teardown: PASSED',
            '[S11.5] shared libraries constructors scheduled before Ring-3 entry: PASSED',
            '[S11.5] shared libraries constructors/destructors Ring-3 execution and teardown: PASSED',
            '[S11.5] shared libraries unresolved import rollback: PASSED',
            '[S11.5] shared libraries argv failure rollback: PASSED',
        ])
    if stage == '13.11':
        required.extend([
            '[S13.11G] BLE SMP identity/privacy foundation: PASSED',
            '[S13.11H] BLE SMP lifecycle/hardening closure: PASSED',
        ])
    if stage == '13.9':
        required.extend(WIFI_REQUIRED_MARKERS)
    return list(dict.fromkeys(required))


def validation_errors(returncode, log, required):
    errors = [f'Missing marker: {marker}' for marker in required if marker not in log]
    if returncode != 33:
        errors.append(f'QEMU exit {returncode}; expected 33')
    if re.search(r'KERNEL PANIC|\[S[^\]\n]+\][^\n]*(?:FAILED|TIMEOUT)', log):
        errors.append('Serial log contains a failure, timeout, or panic')
    return errors

def copy_firmware_vars(template: Path | None, evidence: Path) -> Path | None:
    """Copy writable OVMF state into this run's isolated evidence directory."""
    if template is None:
        return None
    destination = evidence / 'OVMF_VARS.fd'
    shutil.copyfile(template, destination)
    return destination


def firmware_drives(code: Path, vars_copy: Path | None) -> list[str]:
    """Keep immutable CODE and per-run writable VARS on distinct pflash units."""
    drives = ['-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={code}']
    if vars_copy is not None:
        drives.extend(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'])
    return drives


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stage', choices=('10.8', '10.9', '11.1', '11.2', '11.2-status', '11.2-tls', '11.2-aslr', '11.3', '11.4', '11.5', '11.5-compiler', '12.1', '12.2', '12.3', '12.4', '12.5', '13.1', '13.2', '13.3', '13.4', '13.5', '13.6', '13.7', '13.8', '13.9', '13.10', '13.11', '1-5', '15.1', '15.3', '15.4', '15.5', '15.6', '15.7', '15.8', '15.9', '17.1', '18.1', '19.1', '19.3', '19.10'), default='10.8')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4), default=1)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path, help='matching OVMF VARS template, copied per run')
    parser.add_argument('--timeout', type=float, default=180)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    if not qemu.is_file() or not firmware.is_file() or (args.firmware_vars is not None and not args.firmware_vars.is_file()):
        parser.error('QEMU, firmware CODE, and optional firmware VARS files must exist')
    out = root / 'audit-artifacts' / f'stage{args.stage}-{args.cpus}cpu-{time.time_ns()}'
    out.mkdir(parents=True, exist_ok=False)
    vars_copy = copy_firmware_vars(args.firmware_vars, out)
    feature = 'stage13-10-test' if args.stage == '13.11' else f'stage{args.stage.replace(".", "-")}-test'
    if args.stage == '11.2-aslr':
        feature = 'stage11-aslr-test'
    env = dict(os.environ)
    if args.stage == '11.2-status':
        import importlib.util
        spec = importlib.util.spec_from_file_location('exit_status_builder', root / 'scripts/build-exit-status-fixture.py')
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        try:
            env['WOVENHAT_EXIT_STATUS_ELF'] = str(builder.build(out / 'fixture'))
        except RuntimeError as error:
            print(str(error), file=sys.stderr)
            return 1
        feature = 'stage11-exit-status-test'
    if args.stage in ('11.5-compiler', '11.2-tls'):
        import importlib.util
        spec = importlib.util.spec_from_file_location('loader_builder', root / 'scripts/build-loader-fixtures.py')
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        try:
            manifest_path = builder.build(out / 'fixture')
            manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
            env['WOVENHAT_COMPILER_MAIN_ELF'] = manifest['outputs']['shared-demo']['path']
            env['WOVENHAT_COMPILER_LIBRARY_ELF'] = manifest['outputs']['libwoven-demo.so']['path']
            env['WOVENHAT_TLS_STATIC_ELF'] = manifest['outputs']['static-tls-demo']['path']
            env['WOVENHAT_TLS_THREADS_ELF'] = manifest['outputs']['tls-threads-demo']['path']
        except RuntimeError as error:
            print(str(error), file=sys.stderr)
            return 1
        feature = 'stage11-tls-test' if args.stage == '11.2-tls' else 'stage11-compiler-test'
    build = subprocess.run(['cargo', 'run', '--quiet', '--features', feature, '--', '--print-image'],
                           cwd=root, env=env, text=True, capture_output=True)
    (out / 'build.log').write_text(build.stdout + build.stderr, encoding='utf-8')
    if build.returncode:
        print(build.stderr, file=sys.stderr)
        return 1
    serial = out / 'serial.log'
    command = [str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
               '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
               *firmware_drives(firmware, vars_copy),
               '-drive', f'if=none,id=boot,format=raw,readonly=on,file={build.stdout.strip()}',
               '-device', 'virtio-blk-pci,drive=boot,bootindex=1']
    if args.stage in ('13.5', '13.6'):
        command.extend(['-device', 'qemu-xhci,id=xhci',
                        '-device', 'usb-kbd,bus=xhci.0'])
    if args.stage == '13.8':
        command.extend(['-audiodev', 'none,id=wh_audio', '-device', 'intel-hda,id=hda', '-device', 'hda-duplex,bus=hda.0,audiodev=wh_audio'])
    if args.stage == '13.4':
        ahci_image = out / 'ahci.img'
        with ahci_image.open('wb') as image:
            image.truncate(16 * 1024 * 1024)
        command.extend(['-drive', f'if=none,id=sata0,format=raw,file={ahci_image}',
                        '-device', 'ich9-ahci,id=ahci',
                        '-device', 'ide-hd,drive=sata0,bus=ahci.0'])
    if args.stage in ('13.2', '13.3'):
        nvme_image = out / 'nvme.img'
        with nvme_image.open('wb') as image:
            image.truncate(16 * 1024 * 1024)
        command.extend(['-drive', f'if=none,id=nvme0,format=raw,file={nvme_image}',
                        '-device', 'nvme,drive=nvme0,serial=WOVENHAT133'])
    if args.stage == '13.2':
        command.extend(['-netdev', 'user,id=stage132net',
                        '-device', 'e1000,netdev=stage132net'])
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    with (out / 'qemu.log').open('w') as output:
        try:
            result = subprocess.run(command, cwd=root, stdout=output, stderr=output,
                                    timeout=args.timeout, creationflags=flags)
        except subprocess.TimeoutExpired:
            print('QEMU timeout; evidence:', out, file=sys.stderr)
            return 1
    log = serial.read_text(errors='replace') if serial.exists() else ''
    errors = validation_errors(result.returncode, log, required_markers(args.stage, args.cpus))
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        print('FAILED; evidence:', out, file=sys.stderr)
        return 1
    print(f'Stage {args.stage}: PASS ({args.cpus} CPU, exit 33). Evidence: {out}')
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
