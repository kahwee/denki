#!/usr/bin/env python3
"""Opt-in, read-only Tapo compatibility evidence; never changes device settings."""
import argparse
import asyncio
import getpass
import http.client
import importlib.metadata
import ipaddress
import json
import sys

LIMIT = 1024 * 1024
VERSION = "0.11.1"


def discover(host):
    # Direct LAN connection: do not send this request through an HTTP proxy.
    connection = http.client.HTTPConnection(host, 80, timeout=10)
    try:
        connection.request("POST", "/", json.dumps({
            "method": "login", "params": {"sub_method": "discover"}
        }), {"Content-Type": "application/json"})
        response = connection.getresponse()
        body = response.read(LIMIT + 1)
        if len(body) > LIMIT:
            raise ValueError("oversized response")
        if response.status != 200:
            return {"status": "http_error", "http_status": response.status}
        return summarize_discovery(json.loads(body))
    finally:
        connection.close()


def summarize_discovery(body):
    if not isinstance(body, dict) or type(body.get("error_code")) is not int:
        raise ValueError("malformed discovery")
    code = body["error_code"]
    if code != 0:
        return {"status": "device_error", "error_code": code}
    result = body.get("result")
    if not isinstance(result, dict):
        raise ValueError("malformed result")
    info = result.get("tpap")
    if info is None:
        return {"status": "no_tpap_announcement"}
    if not isinstance(info, dict):
        raise ValueError("malformed TPAP metadata")
    clean = {}
    for key in ("tls", "port"):
        if key in info:
            if type(info[key]) is not int:
                raise ValueError("malformed TPAP metadata")
            clean[key] = info[key]
    pake = info.get("pake", [])
    if not isinstance(pake, list) or any(type(x) is not int for x in pake):
        raise ValueError("malformed PAKE metadata")
    clean["pake"] = pake
    return {"status": "tpap_announced", "tpap": clean}


def error_category(error):
    # Never publish exception text: upstream errors can contain private payloads.
    for code in ("TPAP_CREDENTIALS", "TPAP_AUTH_ATTEMPTS_LIMIT"):
        if code in str(error):
            return code.lower()
    if isinstance(error, TimeoutError):
        return "timeout"
    return "upstream_error"


async def read_info(host, username, password):
    from tapo import ApiClient
    # p100 selects the generic plug handler, not a claimed P125 model match.
    device = await ApiClient(username, password).p100(host)
    info = (await device.get_device_info()).to_dict()
    return {key: info[key] for key in ("model", "hw_ver", "fw_ver")
            if isinstance(info.get(key), str)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compatibility", required=True, choices=("on", "off", "unknown"),
                        help="Current app setting; the probe does not change it")
    parser.add_argument("--authenticate", action="store_true",
                        help="Attempt one upstream login and read device info")
    args = parser.parse_args()
    report = {"schema": 1, "compatibility_setting": args.compatibility,
              "hardware_verification": "pending", "authentication": "not_attempted"}
    try:
        host = str(ipaddress.IPv4Address(input("Plug IPv4 address (not saved): ").strip()))
    except ValueError:
        parser.error("Enter a valid IPv4 address")
    try:
        report["discovery"] = discover(host)
    except (OSError, ValueError, http.client.HTTPException):
        report["discovery"] = {"status": "discovery_failed"}
    if args.authenticate:
        discovery = report["discovery"]
        info = discovery.get("tpap", {})
        # Restrict this spike to the known plug TPAP path; never fall back after failure.
        if (discovery["status"] != "tpap_announced" or info.get("tls", 0) != 0
                or info.get("pake") != [2] or info.get("port", 80) != 80):
            report["authentication"] = "skipped_unsupported_or_absent_announcement"
        else:
            try:
                installed = importlib.metadata.version("tapo")
            except importlib.metadata.PackageNotFoundError:
                installed = None
            if installed != VERSION:
                report["authentication"] = "requires_tapo_" + VERSION
            else:
                report["upstream_version"] = installed
                report["handler"] = "p100_generic_plug_experiment"
                username = getpass.getpass("Tapo account email (hidden): ")
                password = getpass.getpass("Tapo password (hidden): ")
                try:
                    report["device"] = asyncio.run(asyncio.wait_for(
                        read_info(host, username, password), timeout=30))
                    report["authentication"] = "device_info_read_succeeded"
                    report["hardware_verification"] = "read_only_upstream_probe_only"
                except Exception as error:
                    report["authentication"] = error_category(error)
                    print("Stopped. Do not loop or retry bad credentials/lockout.", file=sys.stderr)
    print(json.dumps(report, indent=2))
    return 0 if (not args.authenticate and report["discovery"]["status"] in
                 ("tpap_announced", "no_tpap_announcement")) or report["authentication"] == "device_info_read_succeeded" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyboardInterrupt, EOFError):
        print("Probe cancelled.", file=sys.stderr)
        raise SystemExit(130)
