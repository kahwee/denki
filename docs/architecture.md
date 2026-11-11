# Architecture and protocols

## Modules

| File / module | Purpose |
|---------------|---------|
| `src/main.rs` | Async runtime entry point (`denki::app::run()`) |
| `src/app/doctor/report.rs` | Pure diagnostic report and next-step guidance, independent of device I/O and rendering |
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
| `src/tapo_vendor/` | Bundled upstream transport with streaming limits and response authentication |
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
| `save_if_new_in(name, ip, map)` | Legacy no-op; unauthenticated discovery cannot enroll targets |
| `require_group_approval(matches)` | Reject the whole group if a Kasa target lacks explicit enrollment |
| `lookup_by_ip_in(ip, map)` | Reverse lookup alias name from an in-memory map |
| `lookup(name)` | Exact-then-substring match; errors on ambiguity |
| `normalize(s)` | Lowercase + collapse non-alphanumeric to spaces for fuzzy matching |


## Automation and identity checks

Kasa read and write operations share response-envelope validation; each requested
namespace/method must report an integer zero `err_code`. Tapo operations require
integer zero `error_code`. Device info validates explicit state before toggle or
control. Sysinfo/Tapo info checks stored identities at the destination address.
Authenticated Tapo observations can reconcile identities after DHCP moves.
Kasa IDs are unauthenticated: a matching ID from a different address returns
`identity_rebind_required` without changing any alias. An operator must verify the
new address independently and explicitly update it with `denki alias NAME IP`.
Legacy aliases learn identity only at their saved address during scan. Kasa identity
checks detect mismatches but cannot authenticate a peer that copies a device ID.

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

`Protocol::Tapo` selects the bundled, hardened `tapo` 0.12.2 implementation; `Protocol::Klap` still
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


Every buffered automatic-mode HTTP control response uses the same 1 MiB streaming
cap. Excessive advertised length is rejected before reading, and actual chunks
are checked before extending the buffer, including responses without a length.
Oversized negotiation responses fail rather than trigger protocol fallback.
Peer-selected SHA-crypt work is restricted to 1,000–100,000 rounds (default 5,000);
malformed rounds and non-crypt salts are rejected before hashing. Credential
transforms and SPAKE2 proof work run off the async executor, with fixed work caps
because a started blocking task cannot be cancelled by the operation timeout.
Redirects and ambient HTTP proxies are disabled so authentication stays at the
selected device endpoint. Automatic-mode KLAP responses authenticate their MAC
against the current request sequence before decryption, just like explicit KLAP.

The upstream source is embedded under `src/tapo_vendor/`, with its MIT license,
provenance, and security regression tests. This is intentional: a versioned Cargo
path override would be discarded during registry packaging and silently restore
the vulnerable upstream behavior. Package preflight must compile the embedded
source. Remove the copy only once upstream offers equivalent protections. Device
TLS certificates remain self-signed; protocol-level authentication is required,
not a claim that the local endpoint has a publicly trusted certificate. These
hardening changes have offline coverage, not new hardware-verification evidence.

## Untrusted presentation and discovery

Human-readable device strings are escaped before applying trusted terminal styles.
The original values remain available in JSON and in protocol/configuration data.
Shell hints use POSIX quoting; controls and empty targets suppress the hint rather
than silently changing the command target. Unauthenticated Kasa scan observations
never generate physical-control hints. Discovery validates each reply before
reserving its source for deduplication, bounds retained results, and cannot create
an alias at an address saved with a different protocol. Neither a copied ID nor a
matching address authenticates Kasa; a trusted LAN is still required.

Malformed weekday arrays are reported as invalid instead of indexing outside the
seven-day label table or pretending the schedule is valid.


## Local trust and authentication policy

New Kasa observations never add aliases. Only explicit `set` / `denki alias`
sets `group_approved`; v1 and older v2 entries default to false. Identity learning
at a saved address never upgrades this flag. Group commands validate every Kasa
target before any device work; Tapo's authenticated targets are unchanged.
This protects local target selection, not the inherently unauthenticated Kasa wire.

`DENKI_REQUIRE_TPAP=1` rejects non-TPAP handshakes at the original and bundled
transport boundaries, including refresh. Default compatibility mode warns before
KLAP, whose captured fast-hash handshake can be checked against password guesses
offline. The protocol cannot be repaired unilaterally by this client. CLI login
uses hidden prompting or bounded redirected stdin, never a positional password.

## Configuration trust boundary

`src/storage/security.rs` centralizes bounded reads, Unix ownership/permission
checks, and safe file opening. Registry reads require integrity (no group/other
writes); credential reads additionally require confidentiality (no group/other
permissions). Both use the checked file descriptor, reject hard links and
non-regular files, and open with `O_NOFOLLOW | O_NONBLOCK` on Unix. Lock sidecars
use the same file validation before locking. New directories use mode 0700.

Ancestor directories must belong to the current effective user or root and must
not be group/other writable. Root-owned sticky temporary directories are allowed
as ancestors, but never as the immediate configuration directory. Root-managed
system symlinks (for example macOS `/var`) are checked through their targets;
user-owned symlink directories are rejected. Root and same-UID processes remain
trusted. These Unix checks do not constitute a Windows ACL audit.

The existing atomic replacement path can safely replace a permissive credential
file during login without reading it. It never follows a destination symlink.
Reads are limited to 16 KiB for credentials and 4 MiB for the alias registry.
