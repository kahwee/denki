#!/usr/bin/env python3
"""Regenerate the support table from devices.toml; --check rejects drift."""

import argparse
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
START = "<!-- device-support:start -->"
END = "<!-- device-support:end -->"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    devices = tomllib.loads((ROOT / "devices.toml").read_text())["device"]
    rows = [
        START,
        "| Model | Kind | Protocol | Hardware verified | Features |",
        "| --- | --- | --- | --- | --- |",
    ]
    for device in devices:
        rows.append(
            f"| {device['model']} | {device['kind']} | "
            f"{device.get('protocol', 'kasa')} | "
            f"{'Yes' if device['verified'] else 'No'} | "
            f"{', '.join(device['supports'])} |"
        )
    rows.append(END)
    path = ROOT / "docs/commands.md"
    original = path.read_text()
    if original.count(START) != 1 or original.count(END) != 1:
        sys.exit("Expected exactly one pair of device support markers")
    before, rest = original.split(START)
    _, after = rest.split(END)
    updated = before + "\n".join(rows) + after
    if args.check:
        if updated != original:
            sys.exit("Device support docs are stale. Run: python3 scripts/update-device-docs.py")
    else:
        path.write_text(updated)


if __name__ == "__main__":
    main()
