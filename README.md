# denki (電気)

Control TP-Link Kasa and Tapo devices from your terminal over the local network.
Switch plugs, adjust supported lights, read energy usage, and record measurements
for shell scripts or spreadsheets. 電気 means “electricity” in Japanese.

[Documentation & interactive demo](https://kahwee.github.io/denki/) ·
[Supported devices](docs/commands.md#supported-devices) ·
[Troubleshooting](docs/troubleshooting.md)

## Install

Requires [Rust](https://rustup.rs/) 1.99 or newer. The checkout selects Rust 1.99.0.

```sh
git clone https://github.com/kahwee/denki.git
cd denki
cargo install --path . --locked
denki --help
```

Cargo installs `denki` into `~/.cargo/bin`. Run it from a computer that can reach
your devices on the local network.

## Connect your first device

### Kasa

Discover devices and use an alias from the results:

```sh
denki scan
denki aliases
denki info "desk plug"
denki on "desk plug"
denki off "desk plug"
```

Replace `desk plug` with your device's alias. Scan saves discovered Kasa devices
and their identities, preserving your names when a later scan finds a changed IP.

### Tapo P125

Add the plug's IP and save the TP-Link account credentials associated with it.
The example IP is a placeholder; `login` prompts for your password.

```sh
denki alias "tapo plug" 192.0.2.50 --tapo
denki login "you@example.com"
denki info "tapo plug"
denki on "tapo plug"
denki off "tapo plug"
```

`--tapo` (also spelled `--tpap`) automatically negotiates TPAP or KLAP. It does
not force TPAP or change the app's Third-Party Compatibility setting. Existing
`--klap` aliases keep Denki's original KLAP client; use that mode for other
registered Tapo models. See [Tapo setup and limitations](docs/commands.md#aliases-and-tapo-setup).

Tapo scan probes saved addresses. Use `denki scan --tapo-target 192.0.2.50` to
probe an additional address. Commands accept aliases or IPs: a saved IP uses its
registered protocol, while an unknown IP defaults to Kasa.

## Energy and automation

On an energy-capable device, inspect a reading or record 12 samples as CSV:

```sh
denki energy "desk plug"
denki energy "desk plug" --json
denki energy watch "desk plug" --interval 5 --count 12 --format csv > energy.csv
```

Use `--format jsonl` for streaming JSON, or omit `--count` to record until Ctrl-C.
All commands accept `--json` for structured results. The [energy guide](docs/energy.md)
explains units, supported readings, and failed samples. P125 does not support energy
monitoring in Denki.

Preview a group before switching its matching devices:

```sh
denki group off "office" --dry-run
denki group off "office"
```

The dry run lists targets without contacting devices.

## Supported today

Support depends on the model, firmware, and connection mode.

| Connection | Available in Denki | Hardware evidence |
| --- | --- | --- |
| Kasa | Power, lighting, energy, outlets, and device settings, depending on model | Selected models; see the support table |
| Tapo `--klap` | Power; energy on registered energy-capable models | P125 power verified; other registered models unverified |
| Tapo `--tapo` / `--tpap` | P125 info, doctor, on/off/toggle, and group power | P125 info and on/off readbacks verified; toggle and this adapter's KLAP path unverified |

Automatic-mode P125 tests used hardware 1.0 and firmware
`1.4.0 Build 260803 Rel.232153`. That adapter does not expose energy, schedules,
timers, lighting, cameras, or hubs. Broader upstream support does not imply Denki
support. Exact capabilities and verification status live in
[devices.toml](devices.toml) and the [support guide](docs/commands.md#supported-devices).

## Documentation and development

- [Getting started](docs/getting-started.md)
- [Command reference, aliases, and credentials](docs/commands.md)
- [Rust library examples](docs/library.md)
- [Architecture and protocols](docs/architecture.md)
- [Contributing and opt-in hardware tests](CONTRIBUTING.md)

The normal test suite runs offline. Live power tests require explicit targets and
verify restoration of the starting state.

## Credits and license

Automatic TPAP/KLAP communication uses [Mihai Dinculescu's `tapo` library](https://github.com/mihai-dinculescu/tapo).
Denki adds CLI commands, aliases, capability checks, and integration tests.
His [TPAP article](https://mihai.dinculescu.dev/posts/tapo-speaks-tpap/) explains
the protocol and compatibility setting.

Denki is [MIT licensed](LICENSE). Dependencies retain their respective licenses.

## CI maintenance

[GitHub Actions maintenance](.github/ACTIONS.md) covers workflows, parallel checks, action versions, and weekly updates.
