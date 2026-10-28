import importlib.util
import json
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).parents[1] / "scripts" / "inventory-target-disks.py"
SPEC = importlib.util.spec_from_file_location("inventory_target_disks", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class TargetDiskInventoryTests(unittest.TestCase):
    def test_powershell_fixture_marks_windows_system_and_usb_backup(self):
        disks = MODULE.parse_fixture(json.dumps([
            {"Number": 0, "Path": "\\\\.\\PHYSICALDRIVE0", "SerialNumber": "WIN-001",
             "Size": 512 * 1024**3, "FriendlyName": "Windows NVMe", "BusType": "NVMe",
             "IsSystem": True, "IsBoot": True},
            {"Number": 1, "Path": "\\\\.\\PHYSICALDRIVE1", "SerialNumber": "BACKUP-001",
             "Size": 16 * 1024**3, "FriendlyName": "Backup USB", "BusType": "USB",
             "IsRemovable": True},
        ]), "powershell")
        report = MODULE.classify(disks)
        self.assertEqual(report["system_disk"], 0)
        self.assertEqual(report["protected_candidates"], [1])
        self.assertEqual(report["protected_candidate_details"][0]["reasons"], ["backup", "removable", "usb"])
        self.assertTrue(report["safe_target_selection"])
        self.assertTrue(report["read_only"])

    def test_lsblk_fixture_ignores_partitions_and_identifies_removable(self):
        disks = MODULE.parse_fixture(json.dumps({"blockdevices": [
            {"name": "nvme0n1", "path": "/dev/nvme0n1", "type": "disk", "serial": "N-1",
             "size": "512G", "tran": "nvme", "mountpoints": ["/"]},
            {"name": "sdb", "path": "/dev/sdb", "type": "disk", "serial": "U-1",
             "size": "16G", "tran": "usb", "rm": 1, "model": "Backup Stick"},
            {"name": "sdb1", "path": "/dev/sdb1", "type": "part", "size": "16G"},
        ]}), "lsblk")
        self.assertEqual(len(disks), 2)
        self.assertEqual(MODULE.classify(disks)["protected_candidates"], [1])

    def test_missing_serial_or_capacity_fails_closed(self):
        disks = MODULE.parse_fixture(json.dumps([
            {"path": "/dev/a", "serial": "A", "size": 100, "system": True},
            {"path": "/dev/b", "serial": "", "size": 100},
        ]))
        report = MODULE.classify(disks)
        self.assertFalse(report["safe_target_selection"])
        self.assertIn("missing-serial", report["identity"][1]["errors"])

    def test_multiple_system_disks_and_duplicate_serial_fail_closed(self):
        disks = MODULE.parse_fixture(json.dumps([
            {"number": 0, "serial": "DUP", "size_bytes": 100, "system": True},
            {"number": 1, "serial": "DUP", "size_bytes": 200, "boot": True},
        ]))
        report = MODULE.classify(disks)
        self.assertFalse(report["safe_target_selection"])
        self.assertEqual(report["system_disk"], None)
        self.assertEqual(report["duplicate_serials"], ["dup"])

    def test_parser_never_invokes_a_command(self):
        with self.assertRaises(ValueError):
            MODULE.parse_fixture("not command output")


if __name__ == "__main__":
    unittest.main()
