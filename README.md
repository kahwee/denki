# denki (電気)

Command-line tool for controlling TP-Link Kasa and Tapo smart devices over your local network. No cloud required.

*denki* means electricity in Japanese.

## Support Status

| Model | Type | Protocol | Current CLI status |
|-------|------|----------|--------------------|
| KL135 | Smart bulb | Kasa (XOR) | Info, power, brightness, color temperature, HSV color, energy, specs, presets |
| KP115 | Smart plug mini | Kasa (XOR) | Info, power, energy, schedules, clock, LED |
| HS110 | Smart plug | Kasa (XOR) | Info, power, energy |
| HS105 | Smart plug mini | Kasa (XOR) | Info, power, schedules, clock, LED; no energy chip |
| P125/P125M | Tapo plug mini | KLAP (HTTP) | Experimental info and power via saved `--klap` alias |

The parser can identify a few more Kasa device families, but their controls are not fully wired yet:

| Model | Current limitation |
|-------|--------------------|
| KL430 light strip | Scan/info only. Power, color, effects, and energy need the `smartlife.iot.lightStrip` namespace and are not implemented yet. |
| HS220 dimmer | Info, power, and brightness (`denki dim`) via `smartlife.iot.dimmer`. Schedules and clock work. |
| HS300/KP303 power strip | Info, outlet listing (`denki outlets`), per-outlet control (`denki outlet N on\|off\|toggle`), per-outlet energy (`denki outlet-energy N`), per-outlet rename (`denki outlet-rename N name`), strip-level LED. Strip-level energy via `denki energy` on ENE-capable models. |

> **Note on energy units:** KP115 returns milli-units (`voltage_mv`, `current_ma`, `power_mw`). HS110 returns real units (`voltage`, `current`, `power` in V/A/W). Both share the same command.

Devices marked `verified = true` in `devices.toml` have been live-tested on real hardware. Some entries in `devices.toml` are capability notes for future work, not a promise that every command is already routed in the CLI.

## Install

**Prerequisites:** Rust. If you don't have it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Build from source:**

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --release
./target/release/denki --help
```

**Install to PATH:**

```bash
cargo install --path .
# binary lands in ~/.cargo/bin/denki (already on PATH if you used rustup)
# re-run this after pulling updates to replace the installed binary
```

Or copy manually:

```bash
cp target/release/denki /usr/local/bin/
```

## Usage

### Discover Kasa Devices

```bash
denki scan
denki scan --timeout 5
```

Broadcasts a Kasa UDP discovery packet on port 9999. Devices must be on the same subnet. Tapo/KLAP devices do not respond to this scan; save them manually with `denki alias <name> <ip> --klap`.

```
Found 3 device(s)

== Office Bulb == [192.168.1.42] on  good signal
   KL135 HW:2.6  FW:1.0.9  2700K  80%

[192.168.1.55] on  good signal
   KP115  relay: on  feature: TIM:ENE
```

### Save device aliases

Instead of typing IP addresses, save a friendly name:

```bash
denki alias "desk lamp" 192.168.1.42
denki alias "tapo plug" 192.168.1.50 --klap   # Tapo devices need --klap
denki aliases                                   # list all saved aliases
denki unalias "desk lamp"                       # remove an alias
```

Aliases are stored in `~/Library/Application Support/denki/hosts.json` (macOS) or `~/.config/denki/hosts.json` (Linux).

### Control power

```bash
denki power "desk lamp" on
denki power "desk lamp" off
denki power "desk lamp" toggle
denki power 192.168.1.42 on        # raw IP also works
```

Name matching is case-insensitive and partial — `"desk"` resolves `"Desk Lamp"` if it's the only match. Raw IP addresses are treated as Kasa/XOR devices. For Tapo/KLAP devices, use a saved alias with `--klap`.

### KL135 Bulb Controls

```bash
denki dim "desk lamp" 50           # brightness 0–100% (KL135 bulbs and HS220 dimmers)
denki warmth "desk lamp" 2700      # color temperature 2500–9000 K (KL135 bulbs only)
denki color "desk lamp" 275 50 80  # hue (0–360) saturation (0–100) value (0–100) (KL135 bulbs only)
```

`denki dim` works for both KL135-style bulbs (via `smartlife.iot.smartbulb.lightingservice`) and HS220 dimmers (via `smartlife.iot.dimmer`). Color temperature and HSV color are KL135-only — running them on a non-bulb device prints a clear error naming the command and supported models. Setting saturation > 0 activates color mode and disables color temperature mode (they are mutually exclusive on the device).

### Device info

```bash
denki info "desk lamp"
```

### Energy monitoring

```bash
denki energy "desk plug"                  # real-time watts (and V/A for plugs)
denki energy-daily "desk plug" 2025-03    # daily usage for a month
denki energy-monthly "desk plug" 2025     # monthly totals for a year
```

`denki energy` checks the device's feature string at runtime and reports an error if the device has no energy chip (e.g. HS105 with `feature: TIM`).

### Other commands

```bash
denki specs "desk lamp"            # hardware specs: lumens, CRI, wattage (bulbs)
denki presets "desk lamp"          # saved light presets (bulbs)
denki schedules "desk plug"        # scheduled on/off rules (plugs)
denki led "desk plug" off          # turn off the status LED (plugs, dimmers, strips)
denki clock "desk plug"            # show device clock (plugs)
denki outlets "power strip"        # list outlets with state and on-time (strips)
denki outlet "power strip" 2 on    # turn outlet 2 on (strips)
denki outlet "power strip" 2 off   # turn outlet 2 off (strips)
denki outlet "power strip" 2 toggle
denki outlet-energy "power strip" 2          # real-time energy for one outlet (strips with ENE)
denki outlet-energy-daily "power strip" 2 2025-03
denki outlet-energy-monthly "power strip" 2 2025
denki outlet-rename "power strip" 2 "Coffee Maker"
denki rename "desk plug" "new name"
denki restart "desk lamp"
```

## Tapo devices (KLAP protocol)

P125 and other Tapo devices use the KLAP protocol — an AES-128-CBC encrypted session over plain HTTP on port 80.

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"

denki alias "tapo plug" 192.168.1.50 --klap
denki info "tapo plug"
denki power "tapo plug" on
```

You can also save credentials locally:

```bash
denki login "you@example.com" "your-tapo-password"
```

`TAPO_USER` and `TAPO_PASS` take precedence over saved credentials. The `--klap` flag tells denki to use KLAP instead of the legacy XOR protocol. Without it, power and info commands fall back to Kasa/XOR, which will time out on Tapo devices.

## How it works

### Kasa (legacy) protocol

All classic Kasa devices communicate over **TCP port 9999** with an XOR autokey cipher:

- **Cipher:** key starts at 171; each output byte = input XOR previous output byte
- **TCP:** 4-byte big-endian length prefix before the ciphertext
- **UDP discovery:** same cipher, no length prefix — sends to broadcast address, collects replies

### KLAP protocol (Tapo)

Newer Tapo devices (P125, L530, etc.) use a two-phase handshake over plain HTTP on port 80:

1. **Handshake 1** — POST `/app/handshake1` with 16 random bytes; device responds with `remote_seed | server_hash`; verify `SHA256(local + remote + auth_hash) == server_hash`
2. **Handshake 2** — POST `/app/handshake2` with `SHA256(remote + local + auth_hash)`
3. **Requests** — POST `/app/request?seq=N` with AES-128-CBC ciphertext; IV derived from seed material + sequence number

`auth_hash = SHA256(SHA1(username) + SHA1(password))`

denki implements this over raw `tokio::net::TcpStream` rather than an HTTP client library, because some Tapo firmware returns HTTP 400 for requests from standard HTTP clients (reqwest/hyper).

## Project structure

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI (clap) — subcommand definitions, device-type detection, command dispatch |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP, length-prefixed) / `encode_raw` (UDP) |
| `src/transport.rs` | TCP `send()` + UDP `broadcast()` for Kasa device communication |
| `src/klap.rs` | KLAP handshake + AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — maps friendly names to IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credential loading/saving for env vars and `denki login` |
| `src/bulb.rs` | KL135/KL430 sysinfo parsing |
| `src/plug.rs` | Plug sysinfo parsing + feature detection (KP115, HS110, HS105) |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo + per-outlet state |
| `src/tapo.rs` | Tapo device info parsing (P125 `get_device_info` response) |
| `src/ops.rs` | All API calls, namespaced by device type: `bulb_*`, `plug_*`, `tapo_*`, shared |
| `src/display.rs` | Colored terminal output for all device types |
| `src/lib.rs` | Library crate exports for reuse from Rust |
| `devices.toml` | Machine-readable device capability map — supported commands, verified hardware |

## Library usage

denki exposes a library crate alongside the binary. You can use it from other Rust projects:

```toml
[dependencies]
denki = { git = "https://github.com/kahwee/denki" }
```

```rust
use denki::{klap, ops, transport};

// Kasa device
let json = ops::sysinfo("192.168.1.42").await?;

// Tapo device (KLAP)
let mut session = klap::handshake("192.168.1.50", "user@example.com", "pass").await?;
let info = ops::tapo_device_info(&mut session).await?;
ops::tapo_on(&mut session).await?;
```

## Not implemented

- Energy monitoring for Tapo devices (P125 does not expose emeter via KLAP locally)
- Away mode (`anti_theft`) rule creation
- Countdown timer creation
- Schedule creation and deletion
- Firmware updates (intentionally excluded)
- KL430 light-strip control/effects routing
- Strip-level energy monitoring for HS300/KP303 without ENE feature flag

## License

MIT
