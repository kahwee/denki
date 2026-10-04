# Architecture and protocols

## Modules

| File / module | Purpose |
|---------------|---------|
| `src/main.rs` | Async runtime entry point (`denki::app::run()`) |
| `src/app/` | CLI dispatch, scan/info flows, shared helpers, integration tests |
| `src/cli.rs` | Clap argument definitions for all subcommands |
| `src/commands/` | Power, lighting, and energy command handlers |
| `src/admin/` | Aliases, device admin commands, login, shell completions |
| `src/resolve.rs` | Device name/IP resolution; `require_kasa` protocol guard |
| `src/devices/` | `DeviceKind`, `detect_kind`, capability guards (`can_*`), `devices.toml` registry |
| `src/cipher.rs` | XOR autokey cipher: `encode` (TCP, length-prefixed) / `encode_raw` (UDP) |
| `src/transport.rs` | Kasa TCP `send()` and UDP `broadcast_each()` |
| `src/klap.rs` | KLAP handshake + AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — maps friendly names to IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credentials from env vars or `denki login` |
| `src/fmt.rs` | Formatting helpers — `duration`, `on_time`, `parse_year_month`, `current_year_month` |
| `src/bulb.rs` | Bulb and light-strip sysinfo parsing |
| `src/plug.rs` | Plug sysinfo parsing + ENE feature detection |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo + per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/ops.rs` | All API calls — `bulb_set_*`, `relay_*`, `device_*`, `tapo_*`, `strip_*` |
| `src/effects.rs` | Light-strip effect helpers |
| `src/display/` | Colored terminal output for all device types |
| `src/lib.rs` | Library module graph |

## Extending device support

- add the API call in `src/ops.rs`
- add or update the parser in the matching device module (`bulb.rs`, `plug.rs`, etc.)
- add a capability guard in `src/devices/` and wire it through `src/commands/` or `src/admin/` via `src/app/dispatch.rs`
- update `devices.toml` to reflect the new capability
- update the README and inline docs so behavior and help text stay aligned
- add a regression test for the parser or capability guard (`src/app/tests/` for CLI/capability coverage)

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


## Protocol details

### Kasa — port 9999

XOR autokey cipher. Starting key `0xAB` (171); each output byte becomes the key for the next byte.

- **Encrypt:** `c = p ^ key;  key = c`
- **Decrypt:** `p = c ^ key;  key = c`
- **TCP:** `encode()` prepends a 4-byte big-endian length; receiver reads that many cipher bytes then calls `decode()`
- **UDP:** `encode_raw()` for send (no prefix), `decode()` for receive — adding a prefix causes garbage
- **Connect timeout:** 5 seconds
- **Exchange deadline:** 10 seconds for the complete write/read exchange after connecting
- **Response limit:** 1 MiB, checked before allocating the response body
- **Mutations:** validate the requested namespace/method's `err_code`; missing, malformed, or nonzero codes are errors

### KLAP (Tapo) — port 80

AES-128-CBC over plain HTTP. Uses raw `TcpStream` — some Tapo firmware returns 400 for standard HTTP clients. Each HTTP request uses one shared 10-second deadline across connection, writes, headers, and body. Response bodies are capped at 1 MiB before allocation.

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


## ops.rs Naming Conventions

| Prefix | Used for |
|--------|---------|
| `relay_on` / `relay_off` | `set_relay_state` — plugs, dimmers, strips |
| `device_*` | emeter / schedule / time / LED — spans all relay devices |
| `bulb_set_*` | brightness, color-temp, color via `smartlife.iot.smartbulb.lightingservice` |
| `strip_*` | per-outlet commands using `context.child_ids` |
| `tapo_*` | KLAP session operations |

## hosts.rs Public API

The scan command loads hosts.json once before discovery and stops if the registry cannot be loaded. It updates the map in memory as devices respond, preserving existing aliases when normalized names collide, then writes once at the end only if new aliases were added (1 read + 0 or 1 write).

| Function | Purpose |
|----------|---------|
| `load()` | Read hosts.json from disk; returns `BTreeMap<String, HostEntry>` |
| `save(map)` | Write hosts.json from in-memory map |
| `save_if_new_in(name, ip, map)` | Insert if IP not already present; returns `bool` (dirty flag) |
| `lookup_by_ip_in(ip, map)` | Reverse lookup alias name from an in-memory map |
| `lookup(name)` | Exact-then-substring match; errors on ambiguity |
| `normalize(s)` | Lowercase + collapse non-alphanumeric to spaces for fuzzy matching |

