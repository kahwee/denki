"""Offline evidence only; no real device connections."""
import asyncio
import importlib.util
import io
import json
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("probe", Path(__file__).with_name("probe-tpap.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class ProbeTests(unittest.TestCase):
    def test_discovery_drops_private_and_unknown_fields(self):
        result = probe.summarize_discovery({"error_code": 0, "result": {
            "mac": "private", "tpap": {"tls": 0, "port": 80, "pake": [2], "secret": "private"}}})
        self.assertNotIn("private", json.dumps(result))
        self.assertEqual(result["status"], "tpap_announced")

    def test_malformed_metadata_rejected(self):
        for info in ({"pake": "2"}, {"tls": "private"}, {"port": True}):
            with self.assertRaises(ValueError):
                probe.summarize_discovery({"error_code": 0, "result": {"tpap": info}})

    def test_read_only_calls_and_device_redaction(self):
        calls = []
        class Info:
            def to_dict(self):
                return {"model": "TEST", "hw_ver": "1", "fw_ver": "2", "device_id": "private"}
        class Device:
            async def get_device_info(self):
                calls.append("get_device_info")
                return Info()
        class Client:
            def __init__(self, *args):
                pass
            async def p100(self, host):
                calls.append("login")
                return Device()
        with patch.dict(sys.modules, {"tapo": types.SimpleNamespace(ApiClient=Client)}):
            result = asyncio.run(probe.read_info("unused", "unused", "unused"))
        self.assertEqual(calls, ["login", "get_device_info"])
        self.assertEqual(result, {"model": "TEST", "hw_ver": "1", "fw_ver": "2"})

    def run_main(self, discovery, error=None):
        async def read(*args):
            raise error
        output = io.StringIO()
        with patch.object(sys, "argv", ["probe", "--compatibility", "off", "--authenticate"]), \
             patch("builtins.input", return_value="192.0.2.1"), \
             patch.object(probe, "discover", return_value=discovery), \
             patch.object(probe.importlib.metadata, "version", return_value=probe.VERSION), \
             patch.object(probe.getpass, "getpass", return_value="private") as prompt, \
             patch.object(probe, "read_info", side_effect=read) as reader, \
             patch("sys.stdout", output), patch("sys.stderr", io.StringIO()):
            status = probe.main()
        return status, output.getvalue(), prompt.call_count, reader.call_count

    def test_lockout_stops_without_authentication(self):
        status, output, prompts, reads = self.run_main({"status": "device_error", "error_code": -2101})
        self.assertEqual((status, prompts, reads), (1, 0, 0))

    def test_bad_credentials_never_retried_or_printed(self):
        status, output, prompts, reads = self.run_main(
            {"status": "tpap_announced", "tpap": {"tls": 0, "port": 80, "pake": [2]}},
            RuntimeError("TPAP_CREDENTIALS private"))
        self.assertEqual((status, prompts, reads), (1, 2, 1))
        self.assertNotIn("private", output)
        self.assertEqual(json.loads(output)["authentication"], "tpap_credentials")


if __name__ == "__main__":
    unittest.main()
