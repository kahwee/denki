# Device support and commands

## Supported devices

### Verified support

- **KL135 smart bulbs** — power, dimming, color temperature, HSV color, energy, specs, presets
- **KP115 smart plugs** — power, energy, schedules, clock, LED
- **HS110 smart plugs** — power, energy, schedules, clock, LED
- **HS105 smart plugs** — power, schedules, clock, LED; no energy chip
- **P125 Tapo plugs** — info and on/off/toggle power through a saved `--klap` alias

### Supported but unverified

- **LB130 smart bulbs** — same bulb commands as KL135; unverified
- **KL420L5 / KL430 light strips** — scan/info, power, dimming, color temperature, HSV color, energy monitoring, and effects
- **P110 / P115 / KP125M Tapo plugs** — power and real-time/today/month energy usage through a saved `--klap` alias
- **P125M Tapo plugs** — info and power through a saved `--klap` alias
- **HS220 dimmers** — info, power, dimming, schedules, LED, and clock
- **HS300 / KP303 power strips** — info, outlet listing, per-outlet on/off/toggle power control, outlet rename, LED, schedules, and clock; energy only on ENE-capable models (verified on HS300 HW 2.0)

> **Energy note:** Bulbs and light strips use `smartlife.iot.common.emeter`; ENE-capable plugs use `emeter`, and ENE-capable strips use `emeter` with the outlet argument for `energy` or `-o N` for daily/monthly reports. KL135 / LB130 report `power_mw` and `total_wh`; KP115 reports `voltage_mv`, `current_ma`, and `power_mw`; HS110 reports real units (`voltage`, `current`, `power`).

Devices marked `verified` in [`devices.toml`](../devices.toml) have been tested on real hardware.

## Everyday commands

### Discover devices

```bash
denki scan
```

`scan` auto-saves newly discovered aliases and also probes saved `--klap` Tapo aliases.

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

For machine-readable device and diagnostic data, add `--json` to `info` or `doctor`.
The `group` command supports `--dry-run` so an automation can verify its targets
before changing device state.

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

Add `--klap` for Tapo devices:

```bash
denki alias "tapo plug" 192.168.1.51 --klap
```

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

Aliases are stored in `~/.config/denki/hosts.json`.

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

Tapo credentials are stored in `~/.config/denki/credentials.json`, and `TAPO_USER` / `TAPO_PASS` override the saved file.

## How device lookup works

- Device names can come from `scan` output, a saved alias, or a raw IP address.
- Exact normalized alias matches win first, then unambiguous normalized substring matches.
- Raw IP addresses are treated as Kasa devices.
- Tapo devices must be added with `denki alias <name> <ip> --klap`.
- The alias registry parser accepts either the current v2 JSON shape or legacy v1 shape.
- If `hosts.json` is malformed, the CLI prints parser details for both formats to make
  recovery easier.

## Limitations

- daily and monthly history for Tapo energy devices (the local API exposes current/day/month totals)
- away mode (`anti_theft`) rule creation
- countdown timer creation
- schedule creation and deletion
- firmware updates
- strip-level energy monitoring for HS300/KP303 on non-ENE models

