# Device support and commands

## Supported devices

The table is generated from [`devices.toml`](../devices.toml). “Hardware verified”
records existing hardware evidence, not a guarantee for every firmware or region.
Unverified models have implementation support but still need real-device testing.
HS300 verification covers the US model, hardware 2.0 / firmware 1.1.2.

<!-- device-support:start -->
| Model | Kind | Protocol | Hardware verified | Features |
| --- | --- | --- | --- | --- |
| KL135 | Bulb | kasa | Yes | power, dim, color_temp, color, energy, specs, presets |
| LB130 | Bulb | kasa | No | power, dim, color_temp, color, energy, specs, presets |
| KL430 | LightStrip | kasa | No | power, dim, color_temp, color, energy, effects |
| KL420L5 | LightStrip | kasa | No | power, dim, color_temp, color, energy, effects |
| HS220 | Dimmer | kasa | No | power, dim, schedules, led, clock |
| KP115 | Plug | kasa | Yes | power, energy, schedules, led, clock |
| HS110 | Plug | kasa | Yes | power, energy, schedules, led, clock |
| HS105 | Plug | kasa | Yes | power, schedules, led, clock |
| HS300 | Strip | kasa | Yes | power, energy, schedules, led, clock, outlets |
| KP303 | Strip | kasa | No | power, schedules, led, clock, outlets |
| P125 | Tapo | klap | Yes | power |
| P125 | Tapo | tapo (auto TPAP/KLAP) | Partial; see notes below | power |
| P110 | Tapo | klap | No | power, energy |
| P115 | Tapo | klap | No | power, energy |
| KP125M | Tapo | klap | No | power, energy |
| P125M | Tapo | klap | No | power |
<!-- device-support:end -->

Tapo models use saved `--klap` aliases, or `--tapo` for the new P125 adapter. Power strips expose individual outlets;
energy requires an ENE-capable model. Feature names in the table match the registry;
`color_temp` means color temperature and `dim` means brightness.

> **Energy note:** Bulbs and light strips use `smartlife.iot.common.emeter`; ENE-capable plugs use `emeter`, and ENE-capable strips use `emeter` with the outlet argument for `energy` or `-o N` for daily/monthly reports. KL135 / LB130 report `power_mw` and `total_wh`; KP115 reports `voltage_mv`, `current_ma`, and `power_mw`; HS110 reports real units (`voltage`, `current`, `power`).

Devices marked `verified` in [`devices.toml`](../devices.toml) have been tested on real hardware.

## Everyday commands

### Discover devices

```bash
denki scan
```

`scan` auto-saves newly discovered aliases and also probes saved `--klap` and `--tapo` aliases.
It stores the device identity locally and reconciles subsequent discoveries by
identity, preserving your chosen name when DHCP changes the IP. It never moves an
alias merely because a different address reports the same name. Existing v1/v2
registries remain readable; aliases without identity learn it at their current
address on their next successful scan. Until then, a DHCP move cannot be identified
safely by name alone.

Kasa UDP discovery can locate moved Kasa devices. Tapo discovery still requires
known IPs: for the original KLAP path, use `denki scan --tapo-target 192.0.2.51` to probe a moved Tapo device
and recover its existing alias by identity. The option can be repeated. An old,
unreachable Tapo address is reported as a failed probe even if another probe
finds the device at its new address. Saved identity mismatches stop commands before
sending mutations. Device IDs identify peers; the legacy Kasa protocol does not
cryptographically authenticate them.

Malformed discovery replies and failed Tapo probes are reported individually;
partial scans return nonzero and retain their successful results. Tapo probes
are limited to four concurrent connections.

### Inspect and control power

```bash
denki info "desk lamp"
denki on "desk lamp"
denki off "desk lamp"
denki toggle "desk lamp"
denki group off "office"
denki group off "office" --dry-run
```

Use `denki group <on|off|toggle> <pattern>` when multiple aliases should be controlled together
(for example `denki group off "office"` for `Office Color 1`, `Office Color 2`, `Office Color 3`).
Group operations run concurrently (four devices at a time by default), continue when one device
fails, and finish with a success/failure summary. Use `--dry-run` to review every matched alias
without contacting devices, or `--concurrency N` to tune the limit.

### Bulbs and dimmers

```bash
denki dim "desk lamp" 50
denki color-temp "desk lamp" 2700
denki color "desk lamp" -H 275 -s 50 -v 80
```

### Light strip effects

```bash
denki effects "light strip"
denki effect "light strip" Aurora
denki effect "light strip" Off
```

`effects` lists built-in effect names and the active effect. `effect` activates one by name (for example `Aurora`, `Rainbow`, or `Off`).

KL420/KL430 strips also support the standard lighting commands:

```bash
denki on "light strip"
denki dim "light strip" 60
denki color-temp "light strip" 3000
denki color "light strip" -H 200 -s 80 -v 70
```

### Energy

```bash
denki energy "desk plug"
denki energy-daily "desk plug" 2026-03
denki energy-monthly "desk plug" 2026
```

`energy-daily` defaults to the current month, and `energy-monthly` defaults to the current year.
For KLAP-based Tapo energy plugs, `energy` shows current power plus today's and this month's
totals. Tapo's local API does not expose the same daily/monthly history used by Kasa devices,
so `energy-daily` and `energy-monthly` remain Kasa-only.

### Automation building blocks

Commands return a non-zero exit status when a device cannot be reached or does not
support an operation, making them suitable for scripts and scheduled jobs. Use
friendly aliases to keep automation readable:

```bash
denki group off "office"
denki dim "bedroom lamp" 20
denki color-temp "bedroom lamp" 2700
denki effect "living room strip" Aurora
```

Every command accepts `--json` before or after the subcommand. Data goes to stdout;
human-readable details and progress go to stderr. The `group` command supports
`--dry-run` so an automation can verify its targets before changing device state.
Missing or malformed success codes, invalid power state, rejected operations,
and missing energy measurements return errors. Toggle never assumes unknown state
means off.

### Diagnostics and structured output

```bash
denki doctor "desk plug"
denki doctor "desk plug" --json
denki info "desk plug" --json
```

`doctor` checks alias resolution, protocol connectivity, device-info parsing, model support,
firmware metadata, and advertised capabilities. Its versioned JSON report is suitable for bug
reports and automation. It intentionally excludes raw device IDs, MAC addresses, Wi-Fi names,
and credentials.

### Power strips

```bash
denki outlets "power strip"
denki on "power strip" 2
denki off "power strip" 2
denki toggle "power strip" 2
denki energy "power strip" 2
denki energy-daily "power strip" 2026-03 -o 2
denki energy-monthly "power strip" 2026 -o 2
denki outlet-rename "power strip" 2 "Coffee Maker"
```

Notes:

- `outlets` shows the strip's outlet numbers, names, and on/off state.
- Outlet numbers are `1`-based and match the order shown by `outlets`.
- Omit the outlet number to target the whole strip; include it to target one outlet.
- Per-outlet energy commands only work on strips with the `ENE` feature flag.
- `outlet-rename` changes the name shown by `outlets` and `info`.

### Device metadata

```bash
denki specs "desk lamp"
denki presets "desk lamp"
denki schedules "desk plug"
denki led "desk plug" on
denki clock "desk plug"
denki rename "desk plug" "Office Plug"
denki restart "desk plug"
```

## Aliases and Tapo setup

### Save a friendly name for a device

```bash
denki alias "floor lamp" 192.168.1.50
```

You must provide a valid IP address; invalid values are rejected immediately.

Aliasing is normalized for lookup, and duplicate aliases are detected using that same
normalized form. For example, `denki alias "Desk Lamp" ...` will not both be allowed
alongside an existing `denki alias "desk   lamp" ...`.

Use `--klap` for the original Tapo client:

```bash
denki alias "tapo plug" 192.168.1.51 --klap
```

For P125 plugs on recent firmware, use automatic TPAP/KLAP negotiation:

```sh
denki alias "P125 A" 192.0.2.10 --tapo
denki login "you@example.com"
denki info "P125 A"
denki info "P125 A" --json
denki on "P125 A"
denki off "P125 A"
denki toggle "P125 A"
```

`--tpap` is a synonym for `--tapo`. Both use upstream `tapo` 0.11.1 to
negotiate TPAP or KLAP; neither forces TPAP. `--klap` keeps Denki's original
KLAP client. The options are mutually exclusive. Saved `protocol: "tapo"`
identifies the connection policy, not the negotiated wire protocol.

On recent P125 firmware, Third-Party Compatibility **on selects KLAP; off selects
TPAP**, as described in the [upstream TPAP article](https://mihai.dinculescu.dev/posts/tapo-speaks-tpap/).
Denki does not change that app setting. The article explains why TPAP's SPAKE2+
login protects against offline password guessing; automatic mode does not provide
that benefit when the device selects KLAP. Other integrations may still need the
compatibility setting on.

**Supported today:** P125 information, doctor, on/off/toggle, and grouped power
commands. Writes check model, device type, saved identity, and resulting state.
Toggle is read-then-write, so another controller can race it. Energy, outlets,
lighting, schedules, timers, cameras, and hubs are not implemented by this adapter,
even where upstream supports them. P125 has no energy capability in Denki's registry.

**Hardware evidence:** two P125 plugs, HW 1.0, FW `1.4.0 Build 260803 Rel.232153`,
passed information reads while advertising TPAP. One passed off → on → off with
independent readbacks. This confirms reported relay state. The app setting was not
independently confirmed; toggle, other firmware, and this adapter's KLAP path remain
unverified. `devices.toml` tracks auto-mode evidence separately from original KLAP.

Re-save the same alias with `--tapo` to migrate it. At the same address, a known
identity survives KLAP ↔ auto migration; other aliases are preserved. Use the alias
for commands: raw IPs still select Kasa. Auto-mode discovery probes saved addresses;
`scan --tapo-target` remains KLAP-only, so it cannot discover a moved TPAP plug.

Credentials use the saved login or `TAPO_USER`/`TAPO_PASS`. Stop on
`TPAP_CREDENTIALS` or `TPAP_AUTH_ATTEMPTS_LIMIT`; repeated login attempts can lock
the device. Denki adds no retry loop and bounds operations to 30 seconds. After a
power timeout or readback failure, read `info` before retrying. For upstream failures,
report model, hardware/firmware, app setting, and the sanitized error to
[upstream issues](https://github.com/mihai-dinculescu/tapo/issues); never include
credentials or raw captures. Denki routing/output failures belong in Denki's tracker.

Then use the alias anywhere you would use an IP address:

```bash
denki info "tapo plug"
denki on "tapo plug"
```

Notes:

- Leading/trailing whitespace is trimmed when saving aliases.
- Empty and all-whitespace alias names are rejected.
- Alias names that normalize to an existing alias (case-insensitive, non-alphanumeric
  characters normalized to spaces, extra spaces collapsed) are rejected to avoid
  ambiguity in lookups.

### Remove or list aliases

```bash
denki aliases
denki unalias "tapo plug"
```

Aliases are stored in `hosts.json` inside the platform's Denki configuration directory:

| Platform | Configuration directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/denki`, or `~/.config/denki` when unset |
| macOS | `~/Library/Application Support/denki` |
| Windows | `%APPDATA%\denki` |

`denki aliases` prints the actual registry path. These files are local to your user
account; keep credentials and device identities out of bug reports.

### Save Tapo credentials locally

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"
```

Or save them once:

```bash
denki login "you@example.com"
```

You can also pass the password on the command line, but prompting is safer for day-to-day use.

Tapo credentials are stored in `credentials.json` in the same configuration directory.
Set both `TAPO_USER` and `TAPO_PASS` to override the saved file.

## How device lookup works

- Device names can come from `scan` output, a saved alias, or a raw IP address.
- Exact normalized alias matches win first, then unambiguous normalized substring matches.
- Raw IP addresses are treated as Kasa devices.
- Tapo devices require a saved `--klap` or `--tapo` alias.
- The alias registry parser accepts object entries with optional device identities, older object entries without identities, or legacy plain-string entries.
- If `hosts.json` is malformed, the CLI prints parser details for both formats to make
  recovery easier.

## Limitations

- daily and monthly history for Tapo energy devices (the local API exposes current/day/month totals)
- away mode (`anti_theft`) rule creation
- countdown timer creation
- schedule creation and deletion
- firmware updates
- strip-level energy monitoring for HS300/KP303 on non-ENE models


## JSON result contract

Single-result commands emit exactly one JSON object with this envelope:

```json
{"schema_version":2,"command":"energy","status":"ok","data":{"device":"desk plug","outlet":null,"source":"device","measurement":{"power_w":1.234,"voltage_v":null,"current_a":null,"energy_wh":null,"today_energy_wh":12.0,"month_energy_wh":345.0}},"error":null}
```

`status` is `ok` or `error`; the process exits nonzero on error. `error` is null
or an object with stable `code` and human-readable `message`. Codes include
`invalid_arguments`, `not_found`, `malformed_response`, `device_rejected`,
`unsupported_operation`, `identity_mismatch`, `connection_failed`, `timeout`,
`io_error`, `partial_failure`, `integrity_failed`, `registry_conflict`, and the
fallback `command_failed`.
Messages may change; branch on codes. Resolution and argument failures also emit
JSON. Help/version requests retain their normal output.

- `info`: `data.ip`, `data.protocol`, and `data.device` contain device information,
  with local device identifiers removed. Device-specific fields retain their API
  names and depend on model/firmware. `info --json` no longer calls `doctor`.
- `doctor`: `data` contains the diagnostic report, including `error_code` when a
  device check fails. It validates response status, model, and explicit power state.
- `energy`: normalized `data.measurement`; unavailable measurements are null,
  not zero. `power_w`, `voltage_v`, `current_a`, and `energy_wh` use W, V, A, and Wh.
  `energy_wh` is the device-reported cumulative total; Tapo today/month totals
  are separate fields. These are not integrated estimates.
- `energy-daily` / `energy-monthly`: `data.period` and sorted `data.entries` with
  `day` or `month` and `energy_wh`. An empty list is valid; a missing list is an error.
- `group`: `data.results` contains each alias's status, resulting power state, and
  error, plus `succeeded` / `failed` counts. A partial failure still emits every result.
  Dry runs instead include `data.targets` and `dry_run: true`.
- `scan`: `data.devices` contains each discovered/probed address's result, with
  `failed` and `registry_updated` summary fields.
- `aliases`: `data.aliases` includes `identity_known` without exposing device IDs.
- Other commands put their result under `data`; metadata APIs such as schedules,
  specs, and presets retain their device-specific namespace/method shape.

**Migration:** the former `info --json` / `doctor --json` output was a bare
schema-v1 diagnostic report. Single-result output now uses the schema-v2 envelope.
For doctor, move selectors such as `.model` to `.data.model`. Info now returns
actual device details under `.data.device` instead of a diagnostic report.

## Streaming energy measurements

```sh
denki energy watch "desk plug"
denki energy watch "desk plug" --interval 5 --count 120 --format jsonl > energy.jsonl
denki energy watch "power strip" --outlet 2 --interval 10 --format csv > energy.csv
denki energy watch "desk plug" --json --count 3
```

`energy DEVICE [OUTLET]` remains the one-shot command. `watch` is reserved as a
subcommand in this position; use the device's IP if an alias is literally `watch`.

The default interval is five seconds, with a minimum of one second. Each completed
sample is followed by that delay, so slow devices never cause overlapping requests
or a catch-up burst. Omit `--count` to run until Ctrl-C; interruption cancels an
in-flight read and exits cleanly. Polling only reads device information and energy;
it never changes power. The Tapo authenticated session is reused, and a failed
sample causes a reconnect on the next interval. Memory use is bounded; each sample
is flushed immediately. Redirect stdout to a file to collect history locally.

JSONL emits one schema-v1 sample per line, with `timestamp_unix_ms` (UTC Unix
epoch milliseconds at completion), `device`, `outlet`, `status`, `source: "device"`,
`measurement`, and `error`. `--json` is shorthand for JSONL here, not a final
schema-v2 envelope; it cannot be combined with `--format csv`.

CSV columns are `timestamp_unix_ms,device,outlet,status,power_w,voltage_v,current_a,
energy_wh,today_energy_wh,month_energy_wh,error_code,error_message`. Fields containing
commas, quotes, or newlines are quoted. Missing values are empty CSV cells.

A failed sample has `status: "error"`, null `measurement` in JSONL, and an error
code/message. It never becomes a zero-watt sample. Later samples continue, but
any failed sample makes the eventual process exit nonzero. Summaries go to stderr,
so exported files contain only records. Historical collection starts when you run
watch; it cannot reconstruct periods before collection or fill gaps while offline.

## Safe local updates

Credential saves and alias changes use atomic file replacement. On Unix, credential
files are created with mode 0600 before credentials are written. Concurrent Denki
alias edits are serialized through an OS file lock. A scan that detects another
process changed the registry returns `registry_conflict` and saves none of its
updates; rerun `denki scan` to reconcile against the latest aliases.

The empty `.lock` sidecar files are intentionally retained. Do not remove them to
unlock a process: locks are released automatically when the holding process exits.
KLAP responses that fail authentication return `integrity_failed` before decryption
or any state-dependent follow-up command.
