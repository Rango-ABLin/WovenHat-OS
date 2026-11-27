import socket
import unittest
from pathlib import Path
from unittest.mock import patch, MagicMock

import wovenweb


class LauncherTests(unittest.TestCase):
    def test_proxy_environment_removes_mixed_case_bypasses(self):
        env = wovenweb.launch_environment({"PATH": "tools", "No_PrOxY": "*", "HTTP_PROXY": "http://bad", "https_proxy": "bad"})
        self.assertNotIn("No_PrOxY", env)
        self.assertNotIn("HTTP_PROXY", env)
        self.assertEqual(env["http_proxy"], wovenweb.PROXY)
        self.assertEqual(env["https_proxy"], wovenweb.PROXY)
        self.assertEqual(env["all_proxy"], wovenweb.PROXY)
        self.assertEqual(env["NO_PROXY"], "")
        self.assertEqual(env["no_proxy"], "")
        self.assertEqual(env["WOVENWEB_TOR_ONLY"], "1")
        self.assertEqual(env["PATH"], "tools")

    def test_onion_and_https_are_accepted(self):
        for url in ("https://example.com/", "http://example.onion/", "https://[::1]:443/"):
            self.assertEqual(wovenweb.validate_url(url), url)

    def test_unsafe_inputs_are_rejected(self):
        for url in ("--disable-sandbox", "javascript:alert(1)", "file:///etc/passwd", "http://user:pass@example.com/", "https://example.com:0", "https://example.com:65536", "https://example.com/a b", "https://example.com/\n", "https://"):
            with self.subTest(url=url), self.assertRaises(ValueError):
                wovenweb.validate_url(url)

    def test_fragmented_socks_reply(self):
        stream = MagicMock()
        stream.recv.side_effect = [bytes([5]), bytes([0])]
        self.assertEqual(wovenweb.receive_exact(stream, 2), bytes([5, 0]))

    def test_truncated_socks_reply(self):
        stream = MagicMock()
        stream.recv.side_effect = [bytes([5]), b""]
        with self.assertRaises(ConnectionError):
            wovenweb.receive_exact(stream, 2)

    def test_handshake_wire_bytes(self):
        stream = MagicMock()
        stream.recv.return_value = bytes([5, 0])
        with patch("wovenweb.socket.create_connection") as connect:
            connect.return_value.__enter__.return_value = stream
            wovenweb.check_proxy()
        stream.sendall.assert_called_once_with(bytes([5, 1, 0]))

    def test_invalid_handshake(self):
        stream = MagicMock()
        stream.recv.return_value = bytes([5, 255])
        with patch("wovenweb.socket.create_connection") as connect:
            connect.return_value.__enter__.return_value = stream
            with self.assertRaises(ConnectionError):
                wovenweb.check_proxy()

    def test_no_launch_on_proxy_failure(self):
        binary = Path(__file__).resolve()
        with patch("sys.argv", ["wovenweb", "--binary", str(binary), "https://example.com"]), patch("wovenweb.check_proxy", side_effect=ConnectionRefusedError), patch("wovenweb.subprocess.run") as run:
            with self.assertRaises(ConnectionRefusedError):
                wovenweb.main()
            run.assert_not_called()

    def test_browser_exit_code_propagates(self):
        binary = Path(__file__).resolve()
        with patch("sys.argv", ["wovenweb", "--binary", str(binary), "https://example.com"]), patch("wovenweb.check_proxy"), patch("wovenweb.subprocess.run") as run:
            run.return_value.returncode = 7
            self.assertEqual(wovenweb.main(), 7)
            self.assertFalse(run.call_args.kwargs.get("shell", False))

    def test_protection_flags_cannot_be_overridden_by_urls(self):
        cmd = wovenweb.command(Path("ladybird"), ["https://example.com"])
        for flag in ("--temporary-profile", "--force-new-process", "--disable-scripting", "--disable-http-disk-cache", "--default-time-zone=UTC"):
            self.assertIn(flag, cmd)
        self.assertNotIn("--disable-sandbox", cmd)

    def test_real_closed_socks_endpoint_refuses(self):
        # Use an OS-assigned bound, non-listening local port, not the user's Tor.
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            port = reserved.getsockname()[1]
            original = socket.create_connection
            with patch("wovenweb.socket.create_connection", side_effect=lambda address, timeout: original(("127.0.0.1", port), timeout=timeout)):
                with self.assertRaises(OSError):
                    wovenweb.check_proxy()


if __name__ == "__main__":
    unittest.main()
