#!/usr/bin/env python3
"""Disposable DHCP/TFTP fixture: size succeeds, kernel transfer fails.

The QEMU socket netdev carries Ethernet frames prefixed by a big-endian u32.
It is deliberately tiny and implements only the DHCP, ARP and TFTP subset
needed by the WovenHat PXE acceptance fixture. No host interface is opened.
"""
import argparse, socket, struct
from pathlib import Path

SERVER_MAC = b"\x02\x00\x00\x00\x00\x02"
SERVER_IP = b"\x0a\x00\x02\x02"       # 10.0.2.2
CLIENT_IP = b"\x0a\x00\x02\x0f"       # 10.0.2.15
MASK = b"\xff\xff\xff\x00"
TFTP_TRANSFER_PORT = 1069

def checksum(data):
    if len(data) & 1: data += b"\0"
    total = sum(struct.unpack("!%dH" % (len(data)//2), data))
    total = (total >> 16) + (total & 0xffff)
    total += total >> 16
    return (~total) & 0xffff

def frame(dst, src, kind, payload): return dst + src + struct.pack("!H", kind) + payload

def ipv4(dst, payload, proto=17):
    header = b"\x45\0" + struct.pack("!H", 20 + len(payload)) + b"\0\0\0\0\x40" + bytes([proto]) + b"\0\0" + SERVER_IP + dst
    header = header[:10] + struct.pack("!H", checksum(header)) + header[12:]
    return header + payload

def udp(dst, sport, dport, payload):
    # UDP checksum zero is valid for IPv4.
    return struct.pack("!HHHH", sport, dport, 8 + len(payload), 0) + payload

def dhcp(request, kind):
    xid, flags = request[4:8], request[10:12]
    client_mac = request[28:34]
    fixed = b"\x02\x01\x06\0" + xid + b"\0\0" + flags + b"\0\0\0\0" + CLIENT_IP + SERVER_IP + b"\0\0\0\0" + client_mac + b"\0"*10
    fixed += b"\0"*64 + b"bootloader\0".ljust(128, b"\0") + b"\x63\x82\x53\x63"
    options = bytes([53,1,kind,54,4]) + SERVER_IP + bytes([1,4]) + MASK + bytes([3,4]) + SERVER_IP + bytes([51,4]) + struct.pack("!I", 3600) + b"\xff"
    return fixed + options

def parse_options(payload):
    parts = payload[2:].split(b"\0")
    return parts[0].decode("ascii", "replace"), {parts[i].decode("ascii", "replace").lower(): parts[i+1].decode("ascii", "replace") for i in range(2, len(parts)-1, 2) if parts[i]}

def send(sock, packet):
    sock.sendall(struct.pack("!I", len(packet)) + packet)

def recv_exact(sock, count):
    out = b""
    while len(out) < count:
        part = sock.recv(count-len(out))
        if not part: raise EOFError
        out += part
    return out

def recv(sock): return recv_exact(sock, struct.unpack("!I", recv_exact(sock, 4))[0])

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--connect", required=True, help="QEMU socket listener, HOST:PORT")
    ap.add_argument("--tftp-root", required=True)
    ap.add_argument("--log", required=True)
    args = ap.parse_args()
    host, port = args.connect.rsplit(":", 1)
    bootloader = Path(args.tftp_root, "bootloader").read_bytes()
    log = Path(args.log)
    def note(message):
        print(message, flush=True)
        with log.open("a", encoding="utf-8") as out: out.write(message + "\n")
    sock = socket.create_connection((host, int(port)), timeout=20)
    sock.settimeout(30)
    client_mac = None
    boot_transfers = {}
    def send_boot_block(mac, port, block, block_size):
        data = bootloader[(block - 1) * block_size:block * block_size]
        answer = b"\x00\x03" + struct.pack("!H", block) + data
        send(sock, frame(mac, SERVER_MAC, 0x0800, ipv4(CLIENT_IP, udp(CLIENT_IP, TFTP_TRANSFER_PORT, port, answer))))
    while True:
        packet = recv(sock)
        if len(packet) < 14: continue
        dst, src, kind, body = packet[:6], packet[6:12], struct.unpack("!H", packet[12:14])[0], packet[14:]
        if kind == 0x0806 and len(body) >= 28 and body[24:28] == SERVER_IP:
            client_mac = src
            arp = b"\0\x01\x08\0\x06\x04\0\x02" + SERVER_MAC + SERVER_IP + src + body[14:18]
            send(sock, frame(src, SERVER_MAC, 0x0806, arp)); continue
        if kind != 0x0800 or len(body) < 28 or body[9] != 17: continue
        ihl = (body[0] & 15) * 4
        sport, dport = struct.unpack("!HH", body[ihl:ihl+4]); payload = body[ihl+8:]
        client_mac = src
        if sport != 68:
            note(f"UDP {sport}->{dport} opcode={struct.unpack('!H', payload[:2])[0] if len(payload) >= 2 else 'short'}")
        if sport == 68 and dport == 67 and len(payload) >= 240:
            message_type = next((payload[i+2] for i in range(240, len(payload)-2) if payload[i] == 53 and payload[i+1] == 1), 0)
            if message_type in (1, 3):
                answer = dhcp(payload, 2 if message_type == 1 else 5)
                send(sock, frame(b"\xff"*6, SERVER_MAC, 0x0800, ipv4(b"\xff"*4, udp(b"\xff"*4, 67, 68, answer))))
                note("DHCP " + ("OFFER" if message_type == 1 else "ACK")); continue
        if len(payload) < 2: continue
        opcode = struct.unpack("!H", payload[:2])[0]
        if opcode == 4 and (src, sport) in boot_transfers:
            acknowledged = struct.unpack("!H", payload[2:4])[0]
            expected, block_size = boot_transfers[(src, sport)]
            if acknowledged == expected:
                next_block = acknowledged + 1
                if (next_block - 1) * block_size < len(bootloader):
                    boot_transfers[(src, sport)] = (next_block, block_size)
                    send_boot_block(src, sport, next_block, block_size)
            continue
        if dport != 69:
            continue
        if opcode != 1: continue
        name, options = parse_options(payload)
        if name == "bootloader":
            note(f"TFTP bootloader requested options={options}")
            block_size = int(options.get("blksize", "512"))
            if not 8 <= block_size <= 65464:
                raise RuntimeError("invalid requested TFTP block size")
            oack = b"\x00\x06blksize\x00" + str(block_size).encode() + b"\x00"
            if "tsize" in options:
                oack += b"tsize\x00" + str(len(bootloader)).encode() + b"\x00"
            boot_transfers[(src, sport)] = (0, block_size)
            send(sock, frame(src, SERVER_MAC, 0x0800, ipv4(CLIENT_IP, udp(CLIENT_IP, TFTP_TRANSFER_PORT, sport, oack))))
            continue
        if name in ("boot.json", "ramdisk"):
            note(f"TFTP optional {name} absent")
            error = b"\0\x05\0\x01not-found\0"
            send(sock, frame(src, SERVER_MAC, 0x0800, ipv4(CLIENT_IP, udp(CLIENT_IP, TFTP_TRANSFER_PORT, sport, error))))
            continue
        if name == "kernel-x86_64" and "tsize" in options:
            note("TFTP kernel tsize acknowledged")
            oack = b"\x00\x06tsize\x004096\x00"
            send(sock, frame(src, SERVER_MAC, 0x0800, ipv4(CLIENT_IP, udp(CLIENT_IP, TFTP_TRANSFER_PORT, sport, oack))))
            continue
        if name == "kernel-x86_64":
            note("TFTP kernel read deliberately rejected")
            error = b"\0\x05\0\x00read-fixture\0"
            send(sock, frame(src, SERVER_MAC, 0x0800, ipv4(CLIENT_IP, udp(CLIENT_IP, TFTP_TRANSFER_PORT, sport, error))))

if __name__ == "__main__": main()
