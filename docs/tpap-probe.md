# Read-only TPAP compatibility probe

This experimental probe collects evidence for TPAP support. Denki itself still
uses KLAP for Tapo. No P125 TPAP hardware verification has been completed.

Run on a computer on the plug's local network with Python 3.10 or newer. Record
the model, hardware revision and firmware from the Tapo app first. Note the
current **Me → Third-Party Services → Third-Party Compatibility** setting.
The probe does not change that setting, power, schedules, or Denki's registry.

## Discover without credentials

```sh
python3 scripts/probe-tpap.py --compatibility off
```

Use `on` or `unknown` if appropriate. The address is prompted locally, not saved.
This sends one HTTP discovery request directly to that address on port 80, with
no account credentials. An absent announcement does not prove KLAP support;
HTTP errors do not prove bad credentials. This is direct-IP probing, not a scan.

## Try one authenticated device-info read

Only after discovery announces TPAP with account-password authentication over
HTTP port 80:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install 'tapo==0.11.1'
.venv/bin/python scripts/probe-tpap.py --compatibility off --authenticate
```

On Windows use `.venv\Scripts\python.exe` instead. Credentials are entered with
hidden prompts, kept in memory, and never written by this script. Do not enable
upstream debug logging. The probe allows one login flow and one information read,
with a 30-second overall authentication/read timeout and no retry loop. Stop on
`tpap_credentials` or `tpap_auth_attempts_limit`; repeated wrong passwords can
lock the device. Other failures are deliberately reported without raw exception
text, which could contain private device data.

The upstream Python package wraps its Rust implementation. Its `p100` constructor
selects the generic plug handler and negotiates authentication; using it with a
P125 is an experiment, not an assertion that P125 is officially supported upstream.
A parsing failure can occur after a successful login. The script currently reports
such failures as `upstream_error`, so that result does not isolate the login stage.
The public API does not expose the selected transport: the report records the
TPAP announcement and read outcome, not independently verified wire traffic.

Share the final JSON block after reviewing it. It includes only allowlisted
protocol metadata, model/hardware/firmware, and outcome categories; it excludes
addresses, account details, device IDs, MACs, names and raw responses. Do not
commit real diagnostic reports. A successful read is evidence for this device
and setting only, not for power control or Denki's production integration.

For a useful baseline, run credential-free discovery with the current setting.
If you later choose to change the setting in the app, record it and run again.
The goal is a successful read with compatibility **off**. Do not change settings
if other integrations depend on them without accounting for that impact.

Reference: [TPAP announcement and release notes](https://mihai.dinculescu.dev/posts/tapo-speaks-tpap/).
