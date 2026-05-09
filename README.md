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
- **HS110 smart plugs** — power and energy
- **HS105 smart plugs** — power, schedules, clock, LED; no energy chip
- **P125 / P125M Tapo plugs** — experimental info and power through a saved `--klap` alias

### Partial support

- **KL430 light strips** — scan/info only for now
- **HS220 dimmers** — info, power, dimming, schedules, and clock
- **HS300 / KP303 power strips** — info, outlet listing, per-outlet control, and outlet energy for ENE-capable models

> **Energy note:** KP115 reports milli-units (`voltage_mv`, `current_ma`, `power_mw`), while HS110 reports real units (`voltage`, `current`, `power`). Both use the same `energy` command.

Devices marked `verified = true` in `devices.toml` have been tested on real hardware.

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
denki warmth "desk lamp" 2700
denki color "desk lamp" 275 50 80
```

### Energy and strip commands

```bash
denki energy "desk plug"
denki energy-daily "desk plug" 2025-03
denki energy-monthly "desk plug" 2025
denki outlets "power strip"
denki outlet "power strip" 2 on
denki outlet-energy "power strip" 2
denki outlet-energy-daily "power strip" 2 2025-03
denki outlet-energy-monthly "power strip" 2 2025
denki outlet-rename "power strip" 2 "Coffee Maker"
```

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
- `src/bulb.rs` — bulb-specific parsing
- `src/plug.rs` — plug-specific parsing
- `src/dimmer.rs` — HS220 dimmer parsing
- `src/strip.rs` — power strip parsing and outlet control
- `src/tapo.rs` — Tapo device info parsing
- `src/ops.rs` — device operations
- `src/display.rs` — terminal output formatting
- `src/lib.rs` — library exports
- `devices.toml` — machine-readable device capability map

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
