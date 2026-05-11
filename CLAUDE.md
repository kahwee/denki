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
| `src/transport.rs` | TCP `send()` + UDP `broadcast_each()` / `broadcast()` for Kasa devices |
| `src/klap.rs` | KLAP handshake + AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — maps friendly names → IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credentials from env vars or `denki login` |
| `src/fmt.rs` | Shared formatting helpers (duration, etc.) |
| `src/bulb.rs` | KL135/KL430 sysinfo parsing |
| `src/plug.rs` | Plug sysinfo parsing + feature detection (KP115, HS110, HS105) |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo + per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/ops.rs` | All API calls — `bulb_set_*`, `relay_*`, `device_*`, `tapo_*`, `strip_*` |
| `src/display.rs` | Colored terminal output for all device types |
| `src/lib.rs` | Re-exports all modules as pub for library use |

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
1. Looks like an IP → Kasa (default)
2. Saved alias in `hosts.json` → uses stored protocol
3. Unknown name → fail fast with a clear error (no UDP fallback)

Raw IPs are always treated as Kasa/XOR. Tapo devices must be saved as aliases with `--klap`.

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

| Device kind | Power | Dim | Color-temp | Color | Energy | Schedules | LED | Clock | Outlets |
|-------------|-------|-----|------------|-------|--------|-----------|-----|-------|---------|
| KL135 Bulb | ✅ | ✅ | ✅ | ✅ | ✅ | — | — | — | — |
| KL430 Strip | ❌ not implemented | ❌ | ❌ | ❌ | ✅ (unverified) | — | — | — | — |
| HS220 Dimmer | ✅ `set_relay_state` | ✅ | — | — | — | ✅ | ✅ | ✅ | — |
| KP115/HS110 Plug | ✅ `set_relay_state` | — | — | — | ✅ `emeter` (ENE flag) | ✅ | ✅ | ✅ | — |
| HS300/KP303 Strip | ✅ `set_relay_state` | — | — | — | ✅ per outlet (ENE flag) | ✅ | — | ✅ | ✅ per child_id |
| Tapo (P125 etc.) | ✅ KLAP `set_device_info` | — | — | — | — | — | — | — | — |

Commands guard unsupported combinations before issuing any network request and
return a clear message naming the command, the actual device kind, and which
device models support it.

## Design Principles

- **Return fast** — commands should start outputting immediately when possible; never block the full result to sort or batch
- **No sorting** — scan and list output preserves arrival/insertion order; do not sort results
- **Partial returns** — if a command can yield partial data (e.g. multi-device scan), emit each result as it arrives rather than collecting everything first
- **Fail fast** — unsupported device/command combos return a clear error before any network I/O

## Commands

```
denki scan [--timeout N]                          Discover all Kasa devices on the network
denki info <device>                               Detailed device info (Kasa + Tapo)
denki on <device> [N]                             Turn on (Kasa + Tapo); N = outlet (strips only, 1-based)
denki off <device> [N]                            Turn off (Kasa + Tapo); N = outlet (strips only, 1-based)
denki toggle <device> [N]                         Toggle (Kasa + Tapo); N = outlet (strips only, 1-based)
denki dim <device> <0-100>                        Brightness — KL135 bulbs + HS220 dimmers
denki color-temp <device> <2500-9000>             Color temperature in Kelvin — KL135 bulbs only
denki color <device> -H <hue> -s <sat> -v <val>  HSV color — KL135 bulbs only
denki energy <device> [N]                         Real-time power usage — bulbs + ENE-capable plugs/strips; N = outlet (strips)
denki energy-daily <device> [YYYY-MM] [-o N]      Daily energy stats — bulbs + ENE-capable plugs/strips; -o = outlet (strips)
denki energy-monthly <device> [YYYY] [-o N]       Monthly energy stats — bulbs + ENE-capable plugs/strips; -o = outlet (strips)
denki specs <device>                              Hardware specs — KL135 bulbs only
denki presets <device>                            Saved light presets — KL135 bulbs only
denki schedules <device>                          Schedule rules — plugs, dimmers, strips
denki led <device> on|off                         LED indicator — plugs, dimmers, and strips
denki clock <device>                              Device clock — plugs, dimmers, strips
denki outlets <device>                            Per-outlet state — strips only
denki outlet-rename <device> <N> <name>           Rename one outlet — strips only
denki rename <device> <name>                      Rename device (Kasa only)
denki restart <device>                            Reboot device (Kasa only)
denki alias <name> <ip> [--klap]                  Save a friendly name for a device
denki unalias <name>                              Remove a saved alias
denki aliases                                     List all saved aliases
denki login <email> <password>                    Save Tapo credentials for KLAP commands
```

## ops.rs naming conventions

- `relay_on` / `relay_off` — `set_relay_state` commands (plugs, dimmers, strips)
- `device_*` — emeter/schedule/time/led operations that span all relay devices
- `bulb_set_*` — bulb-specific lighting operations (brightness, color-temp, color)
- `strip_*` — per-outlet strip operations using `context.child_ids`
- `tapo_*` — KLAP session operations

## Not Implemented

- Energy monitoring for Tapo devices (P125 does not expose emeter locally)
- Away mode (`anti_theft`) rule creation
- Countdown timer creation
- Schedule creation/deletion
- Firmware updates (intentionally excluded)
- KL430 light-strip control/effects routing

## Docs to update together

If you change behavior, update the README plus the inline comments/doc comments in the affected source file.