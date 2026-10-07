# Troubleshooting

Start with the same alias you used in the failing command:

```sh
denki aliases
denki doctor "desk plug"
```

`aliases` shows saved names and the configuration path. `doctor` checks resolution,
connectivity, device information, and supported capabilities. Its JSON output is
useful for reporting a bug:

```sh
denki doctor "desk plug" --json
```

Review reports before sharing them. Remove local addresses, aliases, and anything
private. Never include passwords or your credentials file.

## Device not found or alias is ambiguous

Copy the full name from `denki aliases`. Exact normalized matches take precedence;
a partial name must match only one alias. For Kasa, run `denki scan` to discover
new devices. For Tapo, add the address explicitly with `denki alias ... --klap`.

## Connection failed or timed out

Check that the device is powered and the computer can reach its local network.
Guest Wi-Fi, client isolation, VPN routing, or a changed DHCP address may prevent
access. Run `denki scan` for Kasa. For a moved Tapo device, use its current IP:

```sh
denki scan --tapo-target 192.0.2.50
```

Replace the placeholder IP. New targets negotiate TPAP/KLAP; saved targets use
their configured protocol. For explicit KLAP mode, save a `--klap` alias first.
Auto-mode identity reconciliation requires an existing `--tapo` alias.
A failed old-address probe may still make scan exit
nonzero even when a new-address probe succeeds. Inspect the individual results.

## Tapo authentication failed

For P125 firmware advertising TPAP, re-save the alias with `--tapo`; `--klap`
selects the original KLAP-only client. Use the account associated with the
device. Save credentials again with `denki login "you@example.com"`. If both
`TAPO_USER` and `TAPO_PASS` are set, they override the saved credentials; check for
stale values in your shell. Do not paste passwords into issue reports.

For auto-mode aliases, `tpap_credentials` means login was rejected and
`tpap_auth_attempts_limit` means the device has locked logins. Stop retries, check
the saved account credentials, and wait for lockout to clear. A `state_mismatch`
or a power timeout leaves the state uncertain; read `info` before another write.

## Unsupported operation or missing energy

Check the [support table](commands.md#supported-devices). Not every plug measures
energy, and Tapo daily/monthly history differs from Kasa. A missing measurement
is `null` in JSON or empty in CSV; it does not mean zero consumption.

## Identity mismatch

The saved address now reports a different device identity, or none. Denki stops
before sending control commands. Run scan to reconcile known identities and check
your router's device list. Do not delete the saved identity just to bypass the
check; first verify which physical device owns the address.

## Registry conflict or malformed configuration

`registry_conflict` means another process edited aliases during a scan. Run scan
again after the other command finishes. Do not remove `.lock` sidecars.

For malformed `hosts.json`, back up the file shown by `denki aliases` before
repairing its JSON. Denki preserves unreadable configuration instead of replacing
it silently. See [configuration paths](commands.md#remove-or-list-aliases).

## Report a reproducible problem

Open an [issue on GitHub](https://github.com/kahwee/denki/issues) with the device
model, region, hardware/firmware version, Denki version (`denki --version`), exact
command, error, and sanitized doctor output. Say whether the behavior happened on
a real device or only in a simulator.
