# denki (電気)

`denki` is a command-line tool for controlling TP-Link Kasa and Tapo devices over your local network — no cloud required.

*denki* means “electricity” in Japanese.

## Why this repo exists

This project is intentionally small, local-network-first, and easy to extend. The core ideas are:

- fast control of smart devices from the terminal
- clear device capability checks before sending requests
- support for both classic Kasa/XOR and newer Tapo/KLAP devices
- a clean split between protocol code, device parsing, and CLI orchestration

## What works today

### Verified support

- **KL135 smart bulbs** — power, dimming, color temperature, HSV color, energy, specs, presets
- **KP115 smart plugs** — power, energy, schedules, clock, LED
- **HS110 smart plugs** — power, energy, schedules, clock, LED
- **HS105 smart plugs** — power, schedules, clock, LED; no energy chip
- **P125 / P125M Tapo plugs** — experimental info and power through a saved `--klap` alias

### Partial support

- **KL430 light strips** — scan/info plus energy monitoring; power/color control not yet implemented
- **HS220 dimmers** — info, power, dimming, schedules, LED, and clock
- **HS300 / KP303 / KP400 power strips** — info, outlet listing, per-outlet power control, per-outlet energy, outlet rename, LED, schedules, and clock for ENE-capable models

> **Energy note:** KL135 reports `power_mw` and `total_wh`; KP115 reports `voltage_mv`, `current_ma`, and `power_mw`; HS110 reports real units (`voltage`, `current`, `power`). All use the same `energy` command.

Devices listed above have been tested on real hardware.

## Quick start

### Build

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --release
./target/release/denki --help
```

### Install

```bash
cargo install --path .
```

That installs the binary to `~/.cargo/bin/denki`.

### Common commands

```bash
denki scan
denki info "desk lamp"
denki on "desk lamp"
denki off "desk lamp"
denki toggle "desk lamp"
denki dim "desk lamp" 50
denki color-temp "desk lamp" 2700
denki color "desk lamp" --hue 275 --sat 50 --val 80
```

### Energy and power strip commands

```bash
denki energy "desk plug"
denki energy-daily "desk plug" 2025-03
denki energy-monthly "desk plug" 2025
```

```bash
denki outlets "power strip"
denki on "power strip" 2
denki off "power strip" 2
denki toggle "power strip" 2
denki energy "power strip" 2
denki energy-daily "power strip" --outlet 2 2025-03
denki energy-monthly "power strip" --outlet 2 2025
denki outlet-rename "power strip" 2 "Coffee Maker"
```

Notes:

- `outlets` shows the strip's outlet numbers, names, and on/off state.
- Outlet numbers are `1`-based and match the order shown by `outlets`.
- Omit the outlet number to target the whole strip; include it to target one outlet.
- Per-outlet energy commands only work on strips with the `ENE` feature flag.
- `outlet-rename` changes the name shown by `outlets` and `info`.

### Tapo setup

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"

denki alias "tapo plug" 192.168.1.50 --klap
denki info "tapo plug"
denki on "tapo plug"
```

Or save credentials locally:

```bash
denki login "you@example.com" "your-tapo-password"
```

## Developer notes

This repo is a good fit for local-network device development because it has a narrow scope and a strong test surface.

### Architecture at a glance

- `src/main.rs` — CLI, command dispatch, device-type detection
- `src/cipher.rs` — XOR autokey cipher helpers
- `src/transport.rs` — Kasa TCP/UDP transport
- `src/klap.rs` — Tapo handshake and encrypted session
- `src/hosts.rs` — alias storage and lookup
- `src/creds.rs` — Tapo credential loading/saving
- `src/fmt.rs` — shared formatting helpers
- `src/bulb.rs` — bulb-specific parsing
- `src/plug.rs` — plug-specific parsing
- `src/dimmer.rs` — HS220 dimmer parsing
- `src/strip.rs` — power strip parsing and outlet control
- `src/tapo.rs` — Tapo device info parsing
- `src/ops.rs` — device operations
- `src/display.rs` — terminal output formatting
- `src/lib.rs` — library exports

### How to extend it

- add the protocol/request in `src/ops.rs`
- add or update the parser in the matching device module
- gate the CLI command with the right device-kind check in `src/main.rs`
- update the README and inline docs together so behavior and help text stay aligned
- add a regression test for the CLI parser or device-kind guard when possible

## Protocol notes

### Kasa (legacy)

Classic Kasa devices use TCP port `9999` with an XOR autokey cipher:

- the key starts at `171`
- each output byte is `input XOR previous_output_byte`
- TCP adds a 4-byte big-endian length prefix before the ciphertext
- UDP discovery uses the same cipher without the length prefix

### KLAP (Tapo)

Tapo devices use a two-step handshake over plain HTTP on port `80`:

1. `POST /app/handshake1` with 16 random bytes
2. `POST /app/handshake2` with the client proof
3. `POST /app/request?seq=N` for encrypted requests

`denki` uses raw `tokio::net::TcpStream` rather than a higher-level HTTP client because some Tapo firmware rejects standard clients.

## Library usage

`denki` can also be used as a library from another Rust project:

```toml
[dependencies]
denki = { git = "https://github.com/kahwee/denki" }
```

```rust
use denki::{klap, ops};

let json = ops::sysinfo("192.168.1.42").await?;
let mut session = klap::handshake("192.168.1.50", "user@example.com", "pass").await?;
let info = ops::tapo_device_info(&mut session).await?;
ops::tapo_on(&mut session).await?;
```

## Limitations

- energy monitoring for Tapo devices
- away mode (`anti_theft`) rule creation
- countdown timer creation
- schedule creation and deletion
- firmware updates
- KL430 light-strip control/effects routing
- strip-level energy monitoring for HS300/KP303 on non-ENE models

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the local development flow, checks, and contribution rules.

## License

MIT
