"""Packet-evidence parser must not pass missing, malformed or empty captures."""
import importlib.util
from pathlib import Path
import struct
from unittest.mock import MagicMock
import unittest

SPEC = importlib.util.spec_from_file_location("wovenweb_harness", Path(__file__).resolve().parents[1] / "scripts/test-wovenweb-network-qemu.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def capture(frame=None):
    head = bytes.fromhex("d4c3b2a1") + struct.pack("<HHIIII", 2, 4, 0, 0, 65535, 1)
    return head if frame is None else head + struct.pack("<IIII", 0, 0, len(frame), len(frame)) + frame


def ipv4(protocol, source=1234, destination=80):
    frame = bytearray(14 + 20 + 8)
    frame[12:14] = bytes.fromhex("0800")
    frame[14] = 0x45
    frame[23] = protocol
    frame[34:38] = struct.pack("!HH", source, destination)
    return frame


class CaptureTests(unittest.TestCase):
    def parse(self, data):
        file = MagicMock()
        file.read_bytes.return_value = data
        return MODULE.dns_packets(file)

    def test_dns_query_and_reply_are_detected(self):
        for source, destination in [(1234, 53), (53, 1234)]:
            self.assertEqual(self.parse(capture(ipv4(17, source, destination)))["ipv4_dns_packets"], 1)

    def test_tcp_is_counted_separately(self):
        self.assertEqual(self.parse(capture(ipv4(6))), {"ipv4_dns_packets": 0, "ipv4_tcp_packets": 1})

    def test_empty_capture_has_no_positive_transport_evidence(self):
        self.assertEqual(self.parse(capture())["ipv4_tcp_packets"], 0)

    def test_incomplete_capture_records_are_rejected(self):
        for data in (b"", capture() + b"x", capture(ipv4(6))[:-1]):
            with self.subTest(length=len(data)), self.assertRaises(RuntimeError):
                self.parse(data)

    def test_non_ethernet_link_type_is_rejected(self):
        data = bytearray(capture())
        data[20:24] = struct.pack("<I", 101)
        with self.assertRaises(RuntimeError):
            self.parse(data)

    def test_unrelated_udp_does_not_count_as_dns(self):
        self.assertEqual(self.parse(capture(ipv4(17, 67, 68)))["ipv4_dns_packets"], 0)


if __name__ == "__main__":
    unittest.main()
