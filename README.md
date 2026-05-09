# denki (電気)

Command-line tool for controlling TP-Link smart bulbs and plugs over your local network. No cloud required.

*denki* means electricity in Japanese.

## Supported Devices

| Device | Type | Energy | Color |
|--------|------|--------|-------|
| KL135 | Smart bulb | Yes | Yes (HSV + color temp) |
| KP115 | Smart plug | Yes (V/A/W) | — |
| HS110 | Smart plug | Yes (V/A/W) | — |
| HS105 | Smart plug mini | No | — |
| P125/P125M | Tapo smart plug | Not yet exposed | — |

> Tapo/KLAP support is experimental. Set `TAPO_USER` and `TAPO_PASS` before using Tapo commands.

## Install

### Prerequisites

You need Rust. If you don't have it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from source

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --release
```

The binary will be at `./target/release/denki`.

### Install with cargo

```bash
cargo install --path .
```

This places the binary in `~/.cargo/bin/denki`, which is already on your PATH if you installed Rust with rustup.

### Or copy manually

```bash
cp target/release/denki /usr/local/bin/
```

On macOS with Homebrew's prefix:

```bash
cp target/release/denki /opt/homebrew/bin/
```

## Project Structure

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI (clap) + command dispatch + device-type detection |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP, has length prefix) / `encode_raw` (UDP, no prefix) |
| `src/transport.rs` | TCP `send()` + UDP `broadcast()` for discovery |
| `src/bulb.rs` | KL135 sysinfo struct — handles on/off state difference (dft_on_state vs inline) |
| `src/plug.rs` | Plug sysinfo struct + feature detection (supports KP115, HS110, HS105) |
| `src/ops.rs` | All device operations, split into bulb/plug/shared namespaces |
| `src/display.rs` | Colored terminal output for both device types |

## Usage

### Find your devices

```bash
denki scan
denki scan --timeout 5    # custom timeout in seconds
```

This broadcasts a UDP discovery packet and lists everything it finds. Devices must be on the same network as your computer.

```
== Office Bulb == [192.168.1.42] on  good signal
   KL135 HW:2.6  FW:1.0.9  2700K  80%
```

### Control power

```bash
denki power "Office Bulb" on
denki power "Office Bulb" off
denki power "Office Bulb" toggle
```

You can use the device name shown by `denki scan`, or pass an IP address if you
prefer. Name matching is case-insensitive and partial, so
`denki info "office"` can resolve `Office Bulb` when it is the only match.

### Bulb: brightness, color temperature, color

```bash
denki dim "Office Bulb" 50          # 0–100%
denki warmth "Office Bulb" 2700     # 2500–9000 Kelvin (warm to cool white)
denki color "Office Bulb" 275 50 80 # hue (0–360), saturation (0–100), value (0–100)
```

### Device info

```bash
denki info "Office Bulb"
```

### Energy usage

```bash
denki energy "Office Bulb"                   # real-time watts
denki energy-daily "Office Bulb" 2025-03     # daily breakdown for a month
denki energy-monthly "Office Bulb" 2025      # monthly totals for a year
```

### Other commands

```bash
denki specs "Office Bulb"          # hardware specs — lumens, CRI, wattage (bulbs)
denki presets "Office Bulb"        # saved light presets (bulbs)
denki schedules "Desk Plug"        # scheduled on/off rules (plugs)
denki led "Desk Plug" off          # turn off the status LED (plugs)
denki clock "Desk Plug"            # show device clock (plugs)
denki rename "Desk Plug" "Desk Lamp"
denki restart "Desk Lamp"
```

### Tapo devices

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-password"
denki tapo 192.168.1.50
denki tapo-power 192.168.1.50 on
```

## How it works

All legacy Kasa devices communicate over TCP port 9999 using a simple XOR autokey cipher. denki implements this cipher directly — no cloud, no account, no app required.

Discovery uses UDP broadcast to the same port. The device responds with its full sysinfo JSON.

### Protocol details

- **Encrypt:** key starts at 171; each output byte = input XOR previous output
- **TCP:** adds a 4-byte big-endian length prefix (`encode`)
- **UDP:** no length prefix (`encode_raw` for send, `decode` for receive)
- **Newer Tapo devices** (P125, L530) use KLAP on port 80 — experimental support is available through `denki tapo` and `denki tapo-power`

### Device capabilities

**KL135 Smart Bulb (`IOT.SMARTBULB`)**
- Power: `smartlife.iot.smartbulb.lightingservice/transition_light_state`
- Color: HSV or color temp (mutually exclusive — sat > 0 disables CCT mode)
- Energy: `smartlife.iot.common.emeter` (not the standard `emeter` module)
- No schedule/countdown/time support
- HW 2.6 adds: `fade_on_off`, `get_default_behavior`, `re_power_type`

**KP115 Smart Plug (`IOT.SMARTPLUGSWITCH`)**
- Power: `system/set_relay_state`
- Energy: standard `emeter` module — realtime, daily, monthly (full V/A/W data)
- Supports: LED indicator, schedule, time

**HS110 Smart Plug with Energy Monitoring**
- Energy values in real units (W/V/A/kWh), not milli-units like KP115
- Day/month stat field: `energy` (kWh), not `energy_wh`

**HS105 Smart Plug Mini (no energy chip)**
- Feature string: `TIM` only (no `ENE`) — energy commands will fail gracefully
- Supports: countdown timer, away mode, schedule, time, LED, cloud info

## Not Implemented

- Full Tapo feature coverage beyond basic info and power
- Away mode rule creation
- Countdown timer creation
- Schedule creation/deletion
- Firmware updates (intentionally excluded)
- Effect animations (not available via local API on KL135)

## License

MIT
