"""Run live DHCP/DNS/ICMP/UDP/TCP regressions against QEMU user networking."""
import argparse
import json
import os
from pathlib import Path
import random
import shutil
import socket
import subprocess
import sys
from qemu_validation import validation_errors
import time


def reserve_tcp_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def reserve_udp_port():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for_marker(serial_path, marker, process, deadline, failure_markers=()):
    while time.monotonic() < deadline:
        log = serial_path.read_text(errors="replace")
        if marker in log:
            return log
        for failure in failure_markers:
            if failure in log:
                raise RuntimeError(
                    f"Kernel reported {failure!r} while waiting for {marker!r}"
                )
        if process.poll() is not None:
            raise RuntimeError(f"QEMU exited before {marker!r}")
        time.sleep(0.05)
    raise RuntimeError(f"Timed out waiting for {marker!r}")


def udp_round_trip(port):
    payload = b"wovenhat-stage5-udp"
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.settimeout(4)
        sock.sendto(payload, ("127.0.0.1", port))
        echoed, _ = sock.recvfrom(2048)
    if echoed != payload:
        raise RuntimeError(f"UDP echo mismatch: {echoed!r}")


def tcp_round_trip(port):
    payload = b"wovenhat-stage5-tcp"
    deadline = time.monotonic() + 6
    last_error = None
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=2) as sock:
                sock.settimeout(4)
                sock.sendall(payload)
                echoed = b""
                while len(echoed) < len(payload):
                    chunk = sock.recv(len(payload) - len(echoed))
                    if not chunk:
                        break
                    echoed += chunk
                if echoed != payload:
                    raise RuntimeError(f"TCP echo mismatch: {echoed!r}")
                return
        except OSError as error:
            last_error = error
            time.sleep(0.1)
    raise RuntimeError(f"TCP host-forward connection failed: {last_error}")


def qmp_command(port, execute, arguments=None):
    """Execute one QMP command and fail closed on an error response."""
    with socket.create_connection(("127.0.0.1", port), timeout=4) as sock:
        sock.settimeout(4)
        # QEMU sends a JSON greeting before accepting commands.
        greeting = b""
        while b"\n" not in greeting:
            greeting += sock.recv(4096)
        sock.sendall(b'{"execute":"qmp_capabilities"}\n')
        capabilities = json.loads(_qmp_line(sock))
        if "error" in capabilities:
            raise RuntimeError(f"QMP capabilities failed: {capabilities}")
        request = {"execute": execute}
        if arguments is not None:
            request["arguments"] = arguments
        sock.sendall((json.dumps(request) + "\n").encode())
        response = json.loads(_qmp_line(sock))
        if "error" in response:
            raise RuntimeError(f"QMP {execute} failed: {response}")
        return response


def _qmp_line(sock):
    response = b""
    while b"\n" not in response:
        response += sock.recv(4096)
    line, _, _ = response.partition(b"\n")
    return line.decode("utf-8")


def build_link_fault_plan(seed, count):
    if count < 1 or count > 8:
        raise ValueError('link-loss count must be between 1 and 8')
    rng = random.Random(seed)
    return [rng.randint(100, 500) for _ in range(count)]


def flap_link(port, downtime_ms):
    qmp_command(port, "set_link", {"name": "net0", "up": False})
    time.sleep(downtime_ms / 1000.0)
    qmp_command(port, "set_link", {"name": "net0", "up": True})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default=shutil.which('qemu-system-x86_64') or r'C:\Program Files\qemu\qemu-system-x86_64.exe')
    parser.add_argument('--firmware', type=Path)
    parser.add_argument('--firmware-vars', type=Path, help='matching writable OVMF VARS template')
    parser.add_argument('--cpus', type=int, choices=(1, 2, 4, 8, 16), default=4)
    parser.add_argument('--timeout', type=float, default=240)
    parser.add_argument('--release', action='store_true')
    parser.add_argument('--feature', default='network-test')
    parser.add_argument(
        '--link-loss',
        action='store_true',
        help='flap the QEMU virtio-net link after UDP and require TCP recovery',
    )
    parser.add_argument(
        '--link-loss-count',
        type=int,
        default=1,
        help='number of deterministic link flaps (1-8; requires --link-loss)',
    )
    parser.add_argument(
        '--fault-seed',
        type=lambda value: int(value, 0),
        default=0x51A7E,
        help='seed for bounded link-loss downtime selection',
    )
    args = parser.parse_args()
    if args.link_loss_count < 1 or args.link_loss_count > 8:
        parser.error('--link-loss-count must be between 1 and 8')
    if args.link_loss_count != 1 and not args.link_loss:
        parser.error('--link-loss-count requires --link-loss')

    root = Path(__file__).resolve().parents[1]
    qemu = Path(args.qemu)
    firmware = args.firmware or qemu.parent / 'share' / 'edk2-x86_64-code.fd'
    firmware_vars = args.firmware_vars
    if not qemu.is_file() or not firmware.is_file() or (firmware_vars is not None and not firmware_vars.is_file()):
        parser.error('Set --qemu and --firmware to existing QEMU and OVMF files.')

    build = ['cargo', 'run', '--quiet']
    if args.release:
        build.append('--release')
    build += ['--features', args.feature, '--', '--print-image']
    print(f"[1/6] Building WovenHat network-test image ({'release' if args.release else 'debug'})...", flush=True)
    image = subprocess.check_output(build, cwd=root, text=True).strip()
    print(f"      image: {image}", flush=True)

    mode = 'release' if args.release else 'debug'
    out = root / 'target' / f'network-regression-{args.cpus}-{mode}'
    out.mkdir(parents=True, exist_ok=True)
    serial = out / 'serial.log'
    serial.write_text('')
    vars_copy = None
    if firmware_vars is not None:
        vars_copy = out / 'OVMF_VARS.fd'
        shutil.copyfile(firmware_vars, vars_copy)
    tcp_port = reserve_tcp_port()
    udp_port = reserve_udp_port()
    qmp_port = reserve_tcp_port() if args.link_loss else None
    link_fault_plan = build_link_fault_plan(args.fault_seed, args.link_loss_count) if args.link_loss else []
    (out / 'link-fault-plan.json').write_text(
        json.dumps({
            'schema': 'wovenhat.stage31.network-link-fault/v1',
            'seed': args.fault_seed,
            'flaps': [
                {'index': index, 'downtime_ms': downtime_ms}
                for index, downtime_ms in enumerate(link_fault_plan, 1)
            ],
            'physical_access': 'none',
        }, indent=2) + '\n',
        encoding='utf-8',
    )

    netdev = (
        f'user,id=net0,'
        f'hostfwd=udp:127.0.0.1:{udp_port}-10.0.2.15:7000,'
        f'hostfwd=tcp:127.0.0.1:{tcp_port}-10.0.2.15:8080'
    )
    command = [
        str(qemu), '-accel', 'tcg,tb-size=128', '-machine', 'q35', '-m', '256M', '-smp', str(args.cpus),
        '-display', 'none', '-serial', f'file:{serial}', '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={firmware}',
               *(['-drive', f'if=pflash,unit=1,format=raw,file={vars_copy}'] if vars_copy is not None else []),
        '-drive', f'if=none,id=boot,format=raw,readonly=on,file={image}',
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-device', 'virtio-net-pci,netdev=net0,disable-modern=on',
        '-netdev', netdev,
        *(['-qmp', f'tcp:127.0.0.1:{qmp_port},server=on,wait=off'] if qmp_port is not None else []),
    ]
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    qemu_log = out / 'qemu.log'
    deadline = time.monotonic() + args.timeout

    print(f"[2/6] Starting QEMU with {args.cpus} CPU(s)...", flush=True)
    print(f"      UDP host:{udp_port} -> guest:7000", flush=True)
    print(f"      TCP host:{tcp_port} -> guest:8080", flush=True)

    early_failures = (
        '[NETTEST] init failed:',
        '[NETTEST] DHCP: TIMEOUT',
        '[NETTEST] DNS: START FAILED',
        '[NETTEST] DNS: FAILED',
        '[NETTEST] DNS: TIMEOUT',
        '[NETTEST] ICMP: START FAILED',
        '[NETTEST] ICMP: FAILED',
        '[NETTEST] ICMP: TIMEOUT',
        '[NETTEST] runtime regression: FAILED',
    )

    with qemu_log.open('w') as errors:
        process = subprocess.Popen(command, cwd=root, stdout=errors, stderr=errors, creationflags=flags)
        try:
            print('[3/6] Waiting for DHCP + DNS + ICMP...', flush=True)
            wait_for_marker(
                serial,
                '[NETTEST] UDP READY port=7000',
                process,
                deadline,
                early_failures,
            )
            print('      DHCP + DNS + ICMP: PASS', flush=True)

            print('[4/6] Verifying host <-> WovenHat UDP round trip...', flush=True)
            udp_round_trip(udp_port)
            wait_for_marker(
                serial,
                '[NETTEST] UDP: PASSED',
                process,
                deadline,
                ('[NETTEST] UDP: TIMEOUT', '[NETTEST] runtime regression: FAILED'),
            )
            print('      UDP: PASS', flush=True)

            if args.link_loss:
                print(
                    f'      Injecting {len(link_fault_plan)} seeded QEMU virtio-net link loss/recovery fault(s)...',
                    flush=True,
                )
                for index, downtime_ms in enumerate(link_fault_plan, 1):
                    flap_link(qmp_port, downtime_ms)
                    print(f'      Link flap {index}: PASS ({downtime_ms} ms down)', flush=True)
                print('      Link loss/recovery: PASS', flush=True)

            print('[5/6] Verifying host <-> WovenHat TCP round trip...', flush=True)
            wait_for_marker(
                serial,
                '[NETTEST] TCP READY port=8080',
                process,
                deadline,
                ('[NETTEST] TCP: OPEN FAILED', '[NETTEST] TCP: LISTEN FAILED',
                 '[NETTEST] runtime regression: FAILED'),
            )
            tcp_round_trip(tcp_port)
            wait_for_marker(
                serial,
                '[NETTEST] TCP: PASSED',
                process,
                deadline,
                ('[NETTEST] TCP: RECEIVE FAILED', '[NETTEST] TCP: RECEIVE TIMEOUT',
                 '[NETTEST] TCP: SEND FAILED', '[NETTEST] TCP: SEND TIMEOUT',
                 '[NETTEST] runtime regression: FAILED'),
            )
            print('      TCP: PASS', flush=True)

            remaining = max(1.0, deadline - time.monotonic())
            result = process.wait(timeout=remaining)
        except Exception as error:
            print(error, file=sys.stderr)
            log = serial.read_text(errors='replace')
            print(log[-12000:], file=sys.stderr)
            return 1
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)

    print('[6/6] Verifying final SMP/TLB/release markers...', flush=True)
    log = serial.read_text(errors='replace')
    required = [
        f'[SMP] online={args.cpus} expected={args.cpus}',
        '[NETTEST] DHCP: PASSED',
        '[NETTEST] DNS: PASSED',
        '[NETTEST] ICMP: PASSED',
        '[NETTEST] UDP: PASSED',
        '[NETTEST] TCP: PASSED',
        '[NETTEST] DHCP/DNS/ICMP/UDP/TCP: PASSED',
        '[SMP] acknowledged TLB shootdowns: PASSED',
        '[BOOT] ALL VALIDATIONS PASSED',
    ]
    if args.feature in ('stage14-1-test', 'stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test'):
        required.extend([
            '[S14.1D] WovenNet DHCP transition/recovery: PASSED',
            '[S14.1S] WovenNet socket/ephemeral stress: PASSED',
            '[S14.1G] WovenNet DNS generation tokens: PASSED',
            '[S14.1H] WovenNet resource/lifecycle hardening: PASSED',
            '[S14.1] WovenNet IPv4 core: PASSED',
            *(['[S14.2A] WovenNet IPv6 address foundation: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2B] WovenNet IPv6 neighbor foundation: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2C] WovenNet ICMPv6 neighbor parser: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2D] WovenNet Router Advertisement state: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2E] WovenNet ICMPv6 checksum boundary: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2F] WovenNet neighbor state and DAD: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2G] WovenNet SLAAC lifecycle: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2H] WovenNet DHCPv6 protocol foundation: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2I] WovenNet DHCPv6 client state machine: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2J] WovenNet IPv6 interface ingress: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2K] WovenNet runtime IPv6 integration: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.2L] WovenNet NDP ingress hardening: PASSED'] if args.feature in ('stage14-2-test', 'stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.3] WovenNet socket API boundary: PASSED'] if args.feature in ('stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.3A] WovenNet socket authority lifecycle: PASSED'] if args.feature in ('stage14-3-test', 'stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.4] WovenNet routing table: PASSED'] if args.feature in ('stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.4A] WovenNet live route integration: PASSED'] if args.feature in ('stage14-4-test', 'stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.5A] firewall policy foundation PASSED'] if args.feature in ('stage14-5-test', 'stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.5B] live packet enforcement PASSED'] if args.feature in ('stage14-5b-test', 'stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.5C] firewall policy authority PASSED'] if args.feature in ('stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.5C] refused mutation auditing PASS'] if args.feature in ('stage14-5c-test', 'stage14-5d-test') else []),
            *(['[S14.5D] firewall parser/transport hardening PASSED'] if args.feature == 'stage14-5d-test' else []),
        ])
    errors = validation_errors(result, log, required)
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print(log[-12000:], file=sys.stderr)
        print(qemu_log.read_text(errors='replace')[-4000:], file=sys.stderr)
        return 1

    print(f'QEMU live network suite: PASS ({args.cpus} CPU, {mode}, exit 33)')
    print('DHCP + DNS + ICMP + host-verified UDP/TCP round trips passed.')
    print('Serial log:', serial)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
