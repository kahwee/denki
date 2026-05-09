# CLAUDE.md — denki

Rust CLI for controlling TP-Link Kasa and Tapo smart devices over the local network.

## Build & Run

```bash
export PATH="/opt/homebrew/bin:$PATH"  # Homebrew Rust on macOS
cargo build --release
./target/release/denki --help
```

## Project Structure

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI (clap) — subcommand definitions, device-type detection, command dispatch |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP, length-prefixed) / `encode_raw` (UDP) |
| `src/transport.rs` | TCP `send()` + UDP `broadcast()` for Kasa device communication |
| `src/klap.rs` | KLAP handshake + AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — maps friendly names → IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credentials from env vars or `denki login` |
| `src/bulb.rs` | KL135/KL430 sysinfo parsing |
| `src/plug.rs` | Plug sysinfo parsing + feature detection (KP115, HS110, HS105) |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo + per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/ops.rs` | All API calls — `bulb_*`, `plug_*`, `tapo_*`, shared |
| `src/display.rs` | Colored terminal output for all device types |
| `src/lib.rs` | Re-exports all modules as pub for library use |
| `devices.toml` | Machine-readable device capability map — commands, verified hardware |

## Protocols

### Kasa (legacy) — port 9999
XOR autokey cipher. Key starts at 171; each output byte = input XOR previous output byte.
- TCP: 4-byte big-endian length prefix before ciphertext
- UDP: no length prefix — `encode_raw()` for send, `decode()` for receive

### KLAP (Tapo) — port 80
AES-128-CBC over plain HTTP on port 80. Two-phase handshake:
1. POST `/app/handshake1` — send 16 random bytes, receive `remote_seed | server_hash`, verify auth
2. POST `/app/handshake2` — send client proof
3. POST `/app/request?seq=N` — encrypted requests with sequence-numbered IVs

`auth_hash = SHA256(SHA1(username) + SHA1(password))`

Implemented via raw `tokio::net::TcpStream` (not reqwest) because some Tapo firmware returns 400 for standard HTTP clients.

## Device Resolution

`resolve()` in `main.rs` resolves a name or IP to `(ip, protocol)`:
1. Looks like an IP or contains `.` → Kasa (default)
2. Saved alias in `hosts.json` → uses stored protocol
3. Falls back to UDP scan → Kasa

Raw IPs are always treated as Kasa/XOR. Tapo devices must be saved as aliases with `--klap`; otherwise the CLI will try port 9999 and time out.

For Tapo devices, save the alias with `--klap`:
```bash
denki alias "tapo plug" 192.168.1.50 --klap
```

## Tapo Credentials

Set `TAPO_USER` and `TAPO_PASS` for all commands that resolve to a KLAP device:
```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"
```

Or save credentials locally:
```bash
denki login "you@example.com" "your-tapo-password"
```

Environment variables take precedence over saved credentials.

## Device Detection (Kasa)

`detect_kind()` reads `mic_type` (newer devices) or `type` (older devices):
- `IOT.SMARTBULB` + `length` field → LightStrip
- `IOT.SMARTBULB` → Bulb
- `IOT.SMARTPLUGSWITCH` + "Dimmer" in `dev_name` → Dimmer
- `IOT.SMARTPLUGSWITCH` + `children` array → Strip
- `IOT.SMARTPLUGSWITCH` → Plug

Implementation status per device kind:

| Device kind | Power | Dim | Warmth | Color | Energy | Schedules | LED | Clock | Outlets |
|-------------|-------|-----|--------|-------|--------|-----------|-----|-------|---------|
| KL135 Bulb | ✅ `smartbulb.lightingservice` | ✅ | ✅ | ✅ | ✅ `smartlife.iot.common.emeter` | — | — | — | — |
| KL430 Strip | ❌ not implemented | ❌ not implemented | ❌ | ❌ | ✅ (unverified) | — | — | — | — |
| HS220 Dimmer | ✅ `set_relay_state` | ✅ `smartlife.iot.dimmer` | — | — | — | ✅ | ✅ | ✅ | — |
| KP115/HS110 Plug | ✅ `set_relay_state` | — | — | — | ✅ `emeter` (ENE flag required) | ✅ | ✅ | ✅ | — |
| HS300/KP303 Strip | ✅ `set_relay_state` | — | — | — | — | ✅ | — | ✅ | ✅ per child_id |
| Tapo (P125 etc.) | ✅ KLAP `set_device_info` | — | — | — | — | — | — | — | — |

Commands guard unsupported combinations before issuing any network request and
return a clear message naming the command, the actual device kind, and which
device models support it (e.g. `` `warmth` is only supported on KL135-style
color bulbs, not plug ``).

## Design Principles

- **Return fast** — commands should start outputting immediately when possible; never block the full result to sort or batch
- **No sorting** — scan and list output preserves arrival/insertion order; do not sort results
- **Partial returns** — if a command can yield partial data (e.g. multi-device scan), emit each result as it arrives rather than collecting everything first

## Commands

```
denki scan [--timeout N]                Discover all Kasa devices on the network
denki info <device>                     Detailed device info (Kasa + Tapo)
denki on <device>                       Turn a device on (Kasa + Tapo)
denki off <device>                      Turn a device off (Kasa + Tapo)
denki toggle <device>                   Toggle a device on/off (Kasa + Tapo)
denki dim <device> <0-100>              Brightness — KL135 bulbs + HS220 dimmers; turns on if off
denki warmth <device> <2500-9000>       Color temperature in Kelvin — KL135 bulbs only
denki color <device> <H> <S> <V>        HSV color — KL135 bulbs only
denki energy <device>                   Real-time power usage — bulbs + ENE-capable plugs
denki energy-daily <device> [YYYY-MM]   Daily energy stats
denki energy-monthly <device> [YYYY]    Monthly energy stats
denki specs <device>                    Hardware specs — KL135 bulbs only
denki presets <device>                  Saved light presets — KL135 bulbs only
denki schedules <device>                Schedule rules — plugs, dimmers, strips
denki led <device> on|off               LED indicator — plugs and dimmers
denki clock <device>                    Device clock — plugs, dimmers, strips
denki outlets <device>                  Per-outlet state — strips only
denki outlet <device> <N> on|off|toggle Control one outlet — strips only (1-based)
denki rename <device> <name>            Rename device (all types)
denki restart <device>                  Reboot device (all types)
denki alias <name> <ip> [--klap]        Save a friendly name for a device
denki unalias <name>                    Remove a saved alias
denki aliases                           List all saved aliases
denki login <email> <password>          Save Tapo credentials for KLAP commands
```

## Not Implemented

- Energy monitoring for Tapo devices (P125 does not expose emeter locally)
- Away mode (`anti_theft`) rule creation
- Countdown timer creation
- Schedule creation/deletion
- Firmware updates (intentionally excluded)
- KL430 light-strip control/effects routing
