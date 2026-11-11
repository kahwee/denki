# denki (電気)

Control TP-Link Kasa and Tapo devices over your local network. Switch supported
plugs and lights, inspect energy usage and record measurements for scripts.
電気 means “electricity” in Japanese.

[Documentation & demo](https://kahwee.github.io/denki/) ·
[Rust API](https://docs.rs/denki/) · [Supported devices](docs/commands.md#supported-devices)

## Install

With [Rust](https://rustup.rs/) 1.99 or newer:

```sh
cargo install denki --locked --version 0.2.0
denki --version
denki --help
```

Cargo installs into `~/.cargo/bin`; keep it on `PATH`. To upgrade, run
`cargo install denki --locked`. GitHub releases contain source archives, not
prebuilt executables. See [getting started](docs/getting-started.md) for an exact
tagged-source install and [contributing](CONTRIBUTING.md) for development setup.

## Connect a device

For Kasa, scan, independently verify the device address, then enroll your own alias:

```sh
denki scan
denki alias "desk plug" 192.0.2.42
denki info "desk plug"
denki doctor "desk plug"
denki on "desk plug"
denki off "desk plug"
```

Scan does not create new Kasa aliases. Legacy Kasa aliases must be explicitly
re-saved with `denki alias NAME IP` before group commands can control them.
Scan never moves a saved Kasa alias based on an unauthenticated device ID. After a
DHCP change, verify the new address independently (for example, in your router),
then run `denki alias "desk plug" NEW_IP`. Kasa scan results are unauthenticated
and do not include physical-control command hints. Verify a device independently
before controlling it. See [discovery](docs/commands.md#discover-devices).

For a Tapo P125, save its address and TP-Link account credentials:

```sh
denki alias "tapo plug" 192.0.2.50 --tapo
denki login "you@example.com"
denki info "tapo plug"
```

Replace names and example addresses with your devices. `login` prompts for the
password. `--tapo` (`--tpap`) negotiates TPAP/KLAP; existing `--klap` aliases keep
the original client. Scan probes saved Tapo addresses. Unknown IPs default to Kasa.
Set `DENKI_REQUIRE_TPAP=1` to refuse non-TPAP authentication, including KLAP
fallback. KLAP handshake captures permit offline password guessing; the default
compatibility mode warns when KLAP is selected.
See [Tapo setup and limits](docs/commands.md#aliases-and-tapo-setup).

## Energy and groups

On an energy-capable model:

```sh
denki energy "desk plug" --json
denki energy watch "desk plug" --interval 5 --count 12 --format csv > energy.csv
```

P125 does not provide energy monitoring in Denki. The [energy guide](docs/energy.md)
covers units, JSONL, unavailable readings and failed samples. Preview a group
before switching it: `denki group off "office" --dry-run` lists matches without
contacting devices.

## Support and development

Capabilities depend on model, firmware and connection mode. P125 automatic-mode
info and on/off readbacks are hardware-verified; toggle and its KLAP fallback
remain unverified. Upstream support alone does not establish Denki support.
[devices.toml](devices.toml) and the [support guide](docs/commands.md#supported-devices)
record exact capabilities and evidence.

- [Commands, aliases and credentials](docs/commands.md) · [troubleshooting](docs/troubleshooting.md)
- [Library examples](docs/library.md) · [architecture](docs/architecture.md)
- [Contributing and offline tests](CONTRIBUTING.md) · [agent guide](AGENTS.md)
- [Publishing](docs/actions-publishing.md) · [changelog](CHANGELOG.md) · [CI maintenance](.github/ACTIONS.md)

Device responses are limited to 1 MiB on every control transport. Human-readable
output escapes terminal control characters; unsafe aliases never become command
hints. JSON escapes terminal-active Unicode reversibly and preserves values for programs.

TPAP/KLAP auto-negotiation uses a [bundled, hardened copy](src/tapo_vendor/README.md)
of [Mihai Dinculescu's tapo library](https://github.com/mihai-dinculescu/tapo).
Denki is [MIT licensed](LICENSE); dependencies retain their own licenses.

`denki doctor "device"` shows Wi-Fi signal and suggests next steps for connection,
authentication, and protocol failures. `--json` includes a `next_steps` array;
weak signal is a warning, not a failed connectivity check. Doctor never switches power.

Use `denki identify "desk plug" --confirm` to record physical identification,
and `denki doctor --all` for read-only fleet health checks. See the
[command guide](docs/commands.md#physical-identification-and-fleet-health).

Denki checks Unix configuration ownership and permissions before use. Unsafe
registries, credentials, or parent directories are rejected; see
[configuration troubleshooting](docs/troubleshooting.md#unsafe-local-configuration).
