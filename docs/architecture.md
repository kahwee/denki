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
| `src/hosts.rs` | Alias registry — maps friendly names to IP, protocol, and optional stable identity |
| `src/error.rs` | Typed error categories for library and automation |
| `src/output.rs` | Task-local CLI result collection and versioned JSON envelope |
| `src/energy.rs` | Measurement normalization and validated history |
| `src/commands/watch.rs` | Bounded streaming energy polling with CSV/JSONL output |
| `src/creds.rs` | Tapo credentials from env vars or `denki login` |
| `src/fmt.rs` | Formatting helpers — `duration`, `on_time`, `parse_year_month`, `current_year_month` |
| `src/bulb.rs` | Bulb and light-strip sysinfo parsing |
| `src/plug.rs` | Plug sysinfo parsing + ENE feature detection |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo + per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/tapo_client.rs` | Upstream TPAP/KLAP plug adapter and identity-checked operations |
| `src/tapo_client/` | Redacted error translation and offline adapter tests |
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

**Per request:** `POST /app/request?seq=N`; `seq += 1; iv = iv_base || seq.to_be_bytes(); body = SHA256(sig || seq || cipher) || cipher`

**Response:** require a 32-byte tag followed by nonempty, block-aligned ciphertext.
Verify `SHA256(sig || current_sequence || ciphertext)` in constant time before
AES-CBC decryption. Reject tampered, replayed, or cross-session frames with
`integrity_failed`. Handshake server-proof comparisons also use constant time.


## ops.rs Naming Conventions

| Prefix | Used for |
|--------|---------|
| `relay_on` / `relay_off` | `set_relay_state` — plugs, dimmers, strips |
| `device_*` | emeter / schedule / time / LED — spans all relay devices |
| `bulb_set_*` | brightness, color-temp, color via `smartlife.iot.smartbulb.lightingservice` |
| `strip_*` | per-outlet commands using `context.child_ids` |
| `tapo_*` | KLAP session operations |

## hosts.rs Public API

The scan command loads hosts.json before discovery and stops if it cannot be loaded.
It reconciles observations in memory, preserving existing aliases. When changes
need saving, it acquires the registry lock and reloads the file. If that snapshot
differs from the original, it returns `registry_conflict` without writing; otherwise
it atomically replaces the file. Network discovery never holds the file lock.

| Function | Purpose |
|----------|---------|
| `load()` | Read hosts.json from disk; returns `BTreeMap<String, HostEntry>` |
| `save(map)` | Explicit full replacement under a lock; atomic write |
| `save_if_unchanged(previous, map)` | Locked comparison and atomic commit; rejects stale scan snapshots |
| `save_if_new_in(name, ip, map)` | Insert if IP not already present; returns `bool` (dirty flag) |
| `lookup_by_ip_in(ip, map)` | Reverse lookup alias name from an in-memory map |
| `lookup(name)` | Exact-then-substring match; errors on ambiguity |
| `normalize(s)` | Lowercase + collapse non-alphanumeric to spaces for fuzzy matching |


## Automation and identity checks

Kasa read and write operations share response-envelope validation; each requested
namespace/method must report an integer zero `err_code`. Tapo operations require
integer zero `error_code`. Device info validates explicit state before toggle or
control. Sysinfo/Tapo info checks stored identities at the destination address.
Discovery uses validated observations to reconcile identities before normal command
checks, permitting DHCP moves without rebinding an alias to a different device.
Legacy aliases learn identity only at their saved address during scan.

CLI handlers record structured results in a task-local output context. Human
rendering routes to stderr under `--json`; dispatch emits the schema-v2 envelope
once, including failures. Group results are collected before returning partial
failure. Info uses its normal data fetch and rendering path in both modes.
Energy text, JSON, and streaming share normalized `Measurement` values; watch
retains an `EnergyReader` between samples to reuse KLAP sessions. Streaming has
its own schema-v1 records and never accumulates the complete sample history.

## Local storage

`src/storage.rs` provides private same-directory temporary files, file sync,
atomic replacement, and parent-directory sync on Unix. A write or rename failure
before publication leaves the previous destination intact and removes the temporary
file. A directory-sync failure after publication is reported, but the replacement
may already be visible. Readers need no lock and see complete snapshots.

Registry edits acquire an OS advisory lock on `hosts.json.lock` before reading and
hold it through replacement. Credential saves similarly use `credentials.json.lock`.
Lock sidecars must never be unlinked: the stable inode coordinates all writers.
The OS releases the lock when a process exits or is killed. `save(map)` explicitly
replaces a registry; callers modifying an earlier `load()` snapshot should use
`save_if_unchanged` to avoid overwriting another process's edits.

Temporary files start with mode 0600 on Unix, before any credential bytes are
written; atomic replacement also tightens permissions on previously permissive
credential files. Other platforms inherit their filesystem's access controls.
This provides local file protection, not encryption at rest.

## Tapo automatic mode

`Protocol::Tapo` selects the pinned upstream `tapo` crate; `Protocol::Klap` still
selects Denki's original KLAP implementation. The upstream `p100` constructor
selects a generic plug handler and negotiates TPAP or KLAP; it is not a P125 model
assertion or a forced TPAP transport. Denki's adapter converts already-decoded
upstream fields directly, without running the KLAP base64 decoder again.

The adapter reuses `creds::load`, sets 10-second upstream request timeouts and a
30-second overall operation deadline, and makes no automatic retries. Upstream
errors become stage-specific, redacted messages. P125 power writes require the
model/type guard in `src/devices/tapo.rs`, then verify state and device identity
in the same session. `devices.toml` tracks `tapo_auto_supports` and
`tapo_auto_verified` separately from original-client capabilities and verification.
