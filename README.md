# denki (電気)

Control TP-Link Kasa and Tapo devices over your local network. Switch supported
plugs and lights, inspect energy usage and record measurements for scripts.
電気 means “electricity” in Japanese.

[Documentation & demo](https://kahwee.github.io/denki/) ·
[Rust API](https://docs.rs/denki/) · [Supported devices](docs/commands.md#supported-devices)

## Install

With [Rust](https://rustup.rs/) 1.99 or newer:

```sh
cargo install denki --locked --version 0.1.1
denki --version
denki --help
```

Cargo installs into `~/.cargo/bin`; keep it on `PATH`. To upgrade, run
`cargo install denki --locked`. GitHub releases contain source archives, not
prebuilt executables. See [getting started](docs/getting-started.md) for an exact
tagged-source install and [contributing](CONTRIBUTING.md) for development setup.

## Connect a device

For Kasa, discover devices and use an alias from the results:

```sh
denki scan
denki info "desk plug"
denki doctor "desk plug"
denki on "desk plug"
denki off "desk plug"
```

For a Tapo P125, save its address and TP-Link account credentials:

```sh
denki alias "tapo plug" 192.0.2.50 --tapo
denki login "you@example.com"
denki info "tapo plug"
```

Replace names and example addresses with your devices. `login` prompts for the
password. `--tapo` (`--tpap`) negotiates TPAP/KLAP; existing `--klap` aliases keep
the original client. Scan probes saved Tapo addresses. Unknown IPs default to Kasa.
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

TPAP/KLAP auto-negotiation uses [Mihai Dinculescu's tapo library](https://github.com/mihai-dinculescu/tapo).
Denki is [MIT licensed](LICENSE); dependencies retain their own licenses.
