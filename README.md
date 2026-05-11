# denki (電気)

`denki` is a command-line tool for controlling TP-Link Kasa and Tapo devices over your local network — no cloud required.

*denki* means "electricity" in Japanese.

## What works today

### Verified on real hardware

- **KL135 smart bulbs** — power, dimming, color temperature, HSV color, energy, specs, presets
- **KP115 smart plugs** — power, energy, schedules, clock, LED
- **HS110 smart plugs** — power, energy, schedules, clock, LED
- **HS105 smart plugs** — power, schedules, clock, LED (no energy chip)
- **HS300 power strips** — info, outlet listing, per-outlet power/energy, outlet rename, LED, schedules, clock
- **P125 / P125M Tapo plugs** — info and power via KLAP

### Partial support

- **KL430 light strips** — scan/info and energy; power/color not yet implemented
- **HS220 dimmers** — info, power, dimming, schedules, LED, clock
- **KP303 power strips** — same feature set as HS300

> **Energy note:** KL135 reports `power_mw` and `total_wh`; KP115 reports `voltage_mv`, `current_ma`, and `power_mw`; HS110 reports `voltage`, `current`, `power`. All use the same `energy` command.

## Quick start

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --release
./target/release/denki --help
```

```bash
cargo install --path .
```

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

### Energy

```bash
denki energy "desk plug"
denki energy-daily "desk plug" 2025-03
denki energy-monthly "desk plug" 2025
```

### Power strips

```bash
denki outlets "garage strip"
denki on "garage strip" 2
denki off "garage strip" 2
denki toggle "garage strip" 2
denki energy "garage strip" 2
denki energy-daily "garage strip" --outlet 2 2025-03
denki energy-monthly "garage strip" --outlet 2 2025
denki outlet-rename "garage strip" 2 "Coffee Maker"
```

Outlet numbers are `1`-based and match the order shown by `outlets`. Omit the outlet number to target the whole strip.

### Tapo devices

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"

denki alias "tapo plug" 192.168.1.50 --klap
denki info "tapo plug"
denki on "tapo plug"
```

Or save credentials locally with `denki login`.

## Architecture

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point — command dispatch and device-type routing |
| `src/cli.rs` | clap command definitions |
| `src/resolve.rs` | Device name/IP resolution, outlet lookup, protocol guards |
| `src/devices.rs` | Capability registry (`devices.toml`), `detect_kind`, command guards |
| `src/ops.rs` | All API calls — `bulb_*`, `relay_*`, `device_*`, `strip_*`, `tapo_*` |
| `src/transport.rs` | Kasa TCP `send()` and UDP `broadcast_each()` |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP) / `encode_raw` (UDP) |
| `src/klap.rs` | KLAP handshake and AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — friendly names → IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credential load/save |
| `src/fmt.rs` | Shared formatting helpers |
| `src/bulb.rs` | KL135/KL430 sysinfo parsing |
| `src/plug.rs` | KP115/HS110/HS105 sysinfo parsing |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo and per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/display.rs` | Colored terminal output |
| `src/lib.rs` | Library re-exports |

`src/resolve.rs` is binary-only (not part of the library API). All other `src/` files except `main.rs` are exported as library modules.

### How to extend

1. Add the protocol/request in `src/ops.rs`
2. Add or update the parser in the matching device module
3. Add the device to `devices.toml` with its supported features
4. Gate the CLI command with the right `devices::can_*` guard in `src/main.rs`
5. Update the README and inline docs together

## Protocol notes

### Kasa (legacy) — port 9999

XOR autokey cipher. Key starts at `171`; each output byte is `input XOR previous_output_byte`. TCP adds a 4-byte big-endian length prefix; UDP does not.

### KLAP (Tapo) — port 80

Two-step handshake over plain HTTP, then AES-128-CBC for all requests. Uses raw `tokio::net::TcpStream` because some Tapo firmware rejects standard HTTP clients.

## Library usage

```toml
[dependencies]
denki = { git = "https://github.com/kahwee/denki" }
```

```rust
use denki::{klap, ops};

let json = ops::sysinfo("192.168.1.42").await?;
let mut session = klap::handshake("192.168.1.50", "user@example.com", "pass").await?;
ops::tapo_on(&mut session).await?;
```

## Not implemented

- Energy monitoring for Tapo devices (P125 doesn't expose emeter locally)
- Away mode (`anti_theft`) and countdown timer creation
- Schedule creation and deletion
- Firmware updates
- KL430 light-strip power/color/effects control

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md).

## License

MIT
