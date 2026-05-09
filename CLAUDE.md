# CLAUDE.md — denki

Rust CLI for controlling TP-Link smart bulbs and plugs over the local network.

## Build & Run

```bash
cargo build
./target/debug/denki --help
```

## Project Structure

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI (clap) + command dispatch + device-type detection |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP, has length prefix) / `encode_raw` (UDP, no prefix) |
| `src/transport.rs` | TCP `send()` + UDP `broadcast()` for discovery |
| `src/bulb.rs` | KL135 sysinfo struct — handles on/off state difference (dft_on_state vs inline) |
| `src/plug.rs` | Plug sysinfo struct + feature detection (supports KP115, HS110, HS105) |
| `src/tapo.rs` | Tapo device info parsing |
| `src/klap.rs` | Experimental KLAP transport for newer Tapo devices |
| `src/ops.rs` | All device operations, split into bulb/plug/shared namespaces |
| `src/display.rs` | Colored terminal output for both device types |

## Protocol

All legacy Kasa devices use **XOR autokey cipher on TCP port 9999**:
- Encrypt: key starts at 171, each output byte = input XOR previous output; TCP adds 4-byte big-endian length prefix
- UDP broadcast (discovery): same cipher but NO length prefix — `encode_raw()` for send, `decode()` for receive
- Newer Tapo devices (P125, L530) use KLAP on port 80. Basic info/power support is experimental and uses `TAPO_USER` / `TAPO_PASS`.

## Supported Devices

### KL135 Smart Bulb (IOT.SMARTBULB)
- Power: `smartlife.iot.smartbulb.lightingservice/transition_light_state`
- Color: HSV or color temp (mutually exclusive — sat > 0 disables CCT mode)
- Energy: `smartlife.iot.common.emeter` (NOT the standard `emeter` module)
- NO schedule/countdown/time support
- HW 2.6 adds: `fade_on_off`, `get_default_behavior`, `re_power_type`
- Specs: `get_light_details` — lumens (800), wattage (10W), CRI (90), beam (220°)
- Presets: `get_preferred_state` — 4 saved slots

### KP115 Smart Plug (IOT.SMARTPLUGSWITCH)
- Power: `system/set_relay_state`
- Energy: standard `emeter` module — realtime, daily, monthly (full V/A/W data)
- LED indicator: `system/set_led_off`
- Schedule: `schedule/get_rules`
- Time: `time/get_time` and `time/get_timezone`
- NO countdown support (returns -1)

### HS110 Smart Plug with Energy Monitoring
- Energy: `emeter` module — real units (W/V/A/kWh), NOT milli-units like KP115
- Day/month stat field: `energy` (kWh), NOT `energy_wh` like KP115

### HS105 Smart Plug Mini (no energy chip)
- Feature string: `TIM` only (no `ENE`) — energy commands will fail gracefully
- Supports: countdown timer, away mode (`anti_theft`), schedule, time, LED, cloud info
- GPS coordinates stored in sysinfo (`longitude_i`, `latitude_i` × 0.0001 = degrees)

### Tapo P125/P125M Smart Plug (experimental KLAP)
- Info: `get_device_info`
- Power: `set_device_info` with `device_on`
- Requires `TAPO_USER` and `TAPO_PASS`

## Device Detection

`detect_kind()` in `main.rs` reads `mic_type` (newer devices) or `type` (older devices like HS105/HS110):
- `IOT.SMARTBULB` → Bulb
- `IOT.SMARTPLUGSWITCH` or `IOT.SMARTPLUG` → Plug

Most legacy commands accept either an IP address or a device name from `denki scan`.
Name matching is case-insensitive and partial, so `denki info "living room"` can
resolve `Living Room Right Lamp` when that is the only match. IP addresses still
bypass discovery and connect directly.

## Commands

```
denki scan [--timeout N]              Discover all devices on the network
denki info <device>                   Detailed device info
denki power <device> on|off|toggle    Power control (auto-detects bulb vs plug)
denki dim <device> <0-100>            Brightness (bulbs only)
denki warmth <device> <2500-9000>     Color temperature in Kelvin (bulbs only)
denki color <device> <H> <S> <V>      HSV color (bulbs only)
denki energy <device>                 Real-time power usage
denki energy-daily <device> [YYYY-MM] Daily energy stats
denki energy-monthly <device> [YYYY]  Monthly energy stats
denki specs <device>                  Hardware specs (bulbs only)
denki presets <device>                Saved light presets (bulbs only)
denki schedules <device>              Schedule rules (plugs only)
denki led <device> on|off             LED indicator (plugs only)
denki clock <device>                  Device clock (plugs only)
denki rename <device> <name>          Rename device
denki restart <device>                Reboot device
denki tapo <host>                     Tapo info via KLAP
denki tapo-power <host> on|off|toggle Tapo power via KLAP
```

## Not Implemented

- Full Tapo feature coverage beyond basic info and power
- Away mode (`anti_theft`) rule creation
- Countdown timer creation
- Schedule creation/deletion
- Firmware updates (intentionally excluded)
- Effect animations (not available via local API on KL135)
