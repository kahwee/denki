# CLAUDE.md — denki

Rust CLI for controlling TP-Link Kasa and Tapo smart devices over the local network. No cloud required.

## Build & Run

```bash
export PATH="/opt/homebrew/bin:$PATH"  # Homebrew Rust on macOS
cargo build --release
./target/release/denki --help
```

## Source Files

| File / module | Purpose |
|---------------|---------|
| `src/main.rs` | Async entry point — calls `denki::app::run()` |
| `src/app/` | CLI wiring: `dispatch`, `info`, `scan`, shared helpers; capability/alias tests under `app/tests/` |
| `src/cli.rs` | Clap `Cli`, `Command`, `LedAction` — all argument definitions and help text (hidden `completions` subcommand) |
| `src/commands/` | Command handlers for power, lighting, and energy |
| `src/admin/` | Alias registry commands, device admin (LED/clock/rename/…), login, shell completions |
| `src/resolve.rs` | `resolve()`, `resolve_quiet()`, `resolve_outlet()`, `require_kasa()` |
| `src/devices/` | `DeviceKind` / `DeviceEntry`, `detect_kind()`, `can_*` guards, `devices.toml` registry |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP, 4-byte length-prefixed) / `encode_raw` (UDP) |
| `src/transport.rs` | TCP `send()` with 5s timeout + UDP `broadcast_each()` for Kasa devices |
| `src/klap.rs` | KLAP two-phase handshake + AES-128-CBC `KlapSession`; all I/O wrapped with 10s timeouts |
| `src/hosts.rs` | Alias registry — friendly name → IP + protocol; `~/.config/denki/hosts.json`; v1/v2 compat |
| `src/creds.rs` | Tapo credentials — `TAPO_USER`/`TAPO_PASS` env vars take precedence over saved file |
| `src/fmt.rs` | `duration(secs)`, `on_time()`, `parse_year_month()`, and `current_year_month()` via Howard Hinnant civil_from_days (no chrono) |
| `src/ops.rs` | Every device API call — `bulb_*`, `relay_*`, `device_*`, `tapo_*`, `strip_*` |
| `src/effects.rs` | Light-strip effect list/activate helpers |
| `src/bulb.rs` | `Bulb` / `LightState` / `DftOnState` — KL135/LB130 bulbs and KL430; handles off-state field relocation |
| `src/plug.rs` | `Plug` — relay state, ENE energy flag, on-time |
| `src/dimmer.rs` | `Dimmer` — HS220 relay state, brightness |
| `src/strip.rs` | `Strip` + `StripChild` — outlet state; expands short child IDs for HS300 HW 2.0 |
| `src/tapo.rs` | `TapoDevice` — KLAP `get_device_info` response; base64-decodes nickname field |
| `src/display/` | Colored terminal output — summaries, energy, strip, Tapo, and hints |
| `src/lib.rs` | Library module graph (re-exports app, commands, admin, protocols, parsers, display) |

## Key Data Files

| File | Purpose |
|------|---------|
| `devices.toml` | Canonical device capability registry — model → kind + verified + supported features; embedded at compile time via `include_str!` |
| `~/.config/denki/hosts.json` | Saved aliases — `{"name": {"ip": "...", "protocol": "kasa"\|"klap"}}` (v2 format); plain string values read as Kasa for v1 compat |
| `~/.config/denki/credentials.json` | Saved Tapo credentials (mode 0600 on Unix); overridden by env vars |

## Protocols

### Kasa — port 9999

XOR autokey cipher. Starting key `0xAB` (171); each output byte becomes the key for the next byte.

- **Encrypt:** `c = p ^ key;  key = c`
- **Decrypt:** `p = c ^ key;  key = c`
- **TCP:** `encode()` prepends a 4-byte big-endian length; receiver reads that many cipher bytes then calls `decode()`
- **UDP:** `encode_raw()` for send (no prefix), `decode()` for receive — adding a prefix causes garbage
- **Connect timeout:** 5 seconds

### KLAP (Tapo) — port 80

AES-128-CBC over plain HTTP. Uses raw `TcpStream` — some Tapo firmware returns 400 for standard HTTP clients. All I/O is wrapped with a 10-second timeout.

**Auth hash:** `SHA256(SHA1(username) || SHA1(password))`

**Handshake:**
1. `POST /app/handshake1` — send 16 random bytes (`local_seed`); receive `remote_seed || server_hash`; verify `SHA256(local_seed || remote_seed || auth_hash) == server_hash`; save `TP_SESSIONID` cookie
2. `POST /app/handshake2` — send `SHA256(remote_seed || local_seed || auth_hash)`

**Key derivation:**
- `key     = SHA256("lsk" || local_seed || remote_seed || auth_hash)[..16]`
- `iv_base = SHA256("iv"  || local_seed || remote_seed || auth_hash)[..12]`
- `seq     = i32::from_be_bytes(iv_full[28..32])`
- `sig     = SHA256("ldk" || local_seed || remote_seed || auth_hash)[..28]`

**Per request:** `seq += 1; iv = iv_base || seq.to_be_bytes(); body = SHA256(sig || seq || cipher) || cipher`

**Response:** skip 32-byte signature prefix, then AES-CBC decrypt the rest.

## Device Resolution

Implemented in `src/resolve.rs`:

1. Input parses as an IP address → Kasa protocol, no alias lookup
2. Exact normalized match in `hosts.json` → stored protocol
3. Unambiguous substring match → stored protocol
4. Multiple substring matches → error listing all candidates
5. No match → fail fast with a clear help message (no UDP fallback)

Alias matching is case- and punctuation-insensitive (`normalize()` collapses non-alphanumeric to spaces). Raw IPs are always Kasa/XOR — Tapo devices must be registered with `denki alias <name> <ip> --klap`.

## Device Capability System

`devices.toml` is the single source of truth for what each model supports. It is embedded at compile time via `include_str!`. Two parts work together:

1. **Capability guards** (`src/devices/`) — `can_*` functions that accept or reject a `DeviceKind`. Called before any network I/O. Return a clear error naming the command and which models support it.

2. **Feature registry** (`devices.toml`) — lists which features each model supports. Capability tests in `src/app/tests/capabilities.rs` assert that every listed feature is permitted by the matching guard, and every unlisted guarded feature is denied.

**Adding a new feature:**
1. Add the `can_*` guard in `src/devices/`
2. Add the feature name to `devices.toml` for each model that supports it
3. Call the guard from the matching handler in `src/commands/` or `src/admin/` (wired via `src/app/dispatch.rs`)
4. Add a test in `src/app/tests/capabilities.rs` if it is a new guard feature string

## Device Detection (Kasa)

`detect_kind()` in `src/devices/` reads `mic_type` (newer firmware) or `type` (older firmware):

| Condition | Result |
|-----------|--------|
| `IOT.SMARTBULB` + `length` field present | `LightStrip` |
| `IOT.SMARTBULB` | `Bulb` |
| `IOT.SMARTPLUGSWITCH` + "Dimmer" in `dev_name` | `Dimmer` (checked before Strip) |
| `IOT.SMARTPLUGSWITCH` + `children` array present | `Strip` |
| `IOT.SMARTPLUGSWITCH` | `Plug` |
| anything else | `Unknown(type_str)` |

Tapo devices respond only on port 80 via KLAP and never appear in UDP scan results.

## Implementation Status

`✅` = works  `—` = not applicable  `❌` = guard blocks it (not yet implemented)

| Model | Verified | Power | Dim | CT | Color | Energy | Schedules | LED | Clock | Outlets | Specs | Presets |
|-------|:--------:|:-----:|:---:|:--:|:-----:|:------:|:---------:|:---:|:-----:|:-------:|:-----:|:-------:|
| KL135 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | — | — | — | ✅ | ✅ |
| LB130 | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | — | — | — | ✅ | ✅ |
| KL430 | — | ❌¹ | ❌¹ | ❌¹ | ❌¹ | ✅ | — | — | — | — | — | — |
| HS220 | — | ✅ | ✅ | — | — | — | ✅ | ✅ | ✅ | — | — | — |
| KP115 | ✅ | ✅ | — | — | — | ✅ | ✅ | ✅ | ✅ | — | — | — |
| HS110 | ✅ | ✅ | — | — | — | ✅ | ✅ | ✅ | ✅ | — | — | — |
| HS105 | ✅ | ✅ | — | — | — | — | ✅ | ✅ | ✅ | — | — | — |
| HS300 | ✅ | ✅ | — | — | — | ✅ | ✅ | ✅ | ✅ | ✅ | — | — |
| KP303 | — | ✅ | — | — | — | — | ✅ | ✅ | ✅ | ✅ | — | — |
| P125 (Tapo) | ✅ | ✅ | — | — | — | — | — | — | — | — | — | — |

CT = color temperature.

¹ KL430 uses `smartlife.iot.lightStrip` namespace — `smartbulb.lightingservice` commands are rejected by the device. Power, dim, color-temp, and color are not yet routed through the correct namespace.

**Energy notes:**
- Bulbs and light strips always use `smartlife.iot.common.emeter`; bare `emeter` returns error -2001
- ENE-capable plugs use `emeter`; ENE-capable strips use `emeter` with `context.child_ids` for per-outlet queries
- KP115 reports milli-unit fields (`power_mw`, `voltage_mv`, `current_ma`, `total_wh`)
- HS110 reports real-unit fields (`power`, `voltage`, `current`, `total` in kWh)
- Plugs and strips require the `ENE` flag in sysinfo `feature` field (e.g. `"TIM:ENE"`)

**HS300 HW 2.0 quirks:**
- Omits `relay_state` from sysinfo — strip toggle uses `is_any_on()` over child states instead
- Child IDs are short ("00"–"05") and must be prefixed with `deviceId` for per-outlet commands

## Design Principles

- **Return fast** — emit results as they arrive; never block to collect everything first
- **No sorting** — scan and list output preserves arrival/insertion order
- **Fail fast** — unsupported device/command combos error before any network I/O
- **Fail loudly** — unknown names error immediately with a helpful message; no silent UDP fallback

## Commands

```
denki scan [--timeout N]                          Scan LAN for Kasa devices; probe saved Tapo aliases concurrently
denki info <device>                               Detailed device info (Kasa + Tapo)
denki on <device> [N]                             Turn on; N = outlet number (strips, 1-based)
denki off <device> [N]                            Turn off; N = outlet number (strips, 1-based)
denki toggle <device> [N]                         Toggle; N = outlet number (strips, 1-based)
denki group <on|off|toggle> <pattern>             Apply power action to all matching aliases
denki dim <device> <0-100>                        Brightness — bulbs + HS220 dimmers
denki color-temp <device> <2500-9000>             Color temperature in Kelvin — bulbs only
denki color <device> -H <hue> -s <sat> -v <val>  HSV color — bulbs only
denki energy <device> [N]                         Real-time power — bulbs + ENE plugs/strips; N = outlet
denki energy-daily <device> [YYYY-MM] [-o N]      Daily energy — defaults to current month
denki energy-monthly <device> [YYYY] [-o N]       Monthly energy — defaults to current year
denki specs <device>                              Hardware specs — bulbs only
denki presets <device>                            Saved light presets — bulbs only
denki effects <device>                            List built-in effects + active effect — light strips only
denki effect <device> <name>                      Activate built-in effect (e.g. Aurora, Off) — light strips only
denki schedules <device>                          Schedule rules — plugs, dimmers, strips
denki led <device> on|off                         LED indicator — plugs, dimmers, strips
denki clock <device>                              Device clock — plugs, dimmers, strips
denki outlets <device>                            Per-outlet state — strips only
denki outlet-rename <device> <N> <name>           Rename one outlet — strips only
denki rename <device> <name>                      Rename device (Kasa only)
denki restart <device>                            Reboot device (Kasa only)
denki alias <name> <ip> [--klap]                  Save a friendly name for a device
denki unalias <name>                              Remove a saved alias
denki aliases                                     List all saved aliases
denki login <email> [password]                    Save Tapo credentials (prompts if password omitted)
```

## ops.rs Naming Conventions

| Prefix | Used for |
|--------|---------|
| `relay_on` / `relay_off` | `set_relay_state` — plugs, dimmers, strips |
| `device_*` | emeter / schedule / time / LED — spans all relay devices |
| `bulb_set_*` | brightness, color-temp, color via `smartlife.iot.smartbulb.lightingservice` |
| `strip_*` | per-outlet commands using `context.child_ids` |
| `tapo_*` | KLAP session operations |

## hosts.rs Public API

The scan command loads hosts.json once before the UDP broadcast, updates the map in memory as devices respond, and writes it once at the end only if new aliases were added (was N reads + N writes; now 1 read + 0 or 1 write).

| Function | Purpose |
|----------|---------|
| `load()` | Read hosts.json from disk; returns `BTreeMap<String, HostEntry>` |
| `save(map)` | Write hosts.json from in-memory map |
| `save_if_new_in(name, ip, map)` | Insert if IP not already present; returns `bool` (dirty flag) |
| `lookup_by_ip_in(ip, map)` | Reverse lookup alias name from an in-memory map |
| `lookup(name)` | Exact-then-substring match; errors on ambiguity |
| `normalize(s)` | Lowercase + collapse non-alphanumeric to spaces for fuzzy matching |

## Not Implemented

- Energy monitoring for Tapo devices (P125 does not expose emeter locally)
- KL430 power, dim, color-temp, color — uses `smartlife.iot.lightStrip`, not `smartbulb.lightingservice`
- Away mode (`anti_theft`) rule creation
- Countdown timer creation
- Schedule creation and deletion
- Firmware updates (intentionally excluded)
