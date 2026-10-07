# denki (電気)

Control TP-Link Kasa and Tapo devices on your local network. Inspect energy
usage, operate lights and plugs, and combine commands in shell scripts.

[**Documentation & interactive demo →**](https://kahwee.github.io/denki/)

Try sample energy output and copy commands, or follow the
[getting-started guide](docs/getting-started.md).

## Install

Requires Rust 1.99 or newer:

```sh
git clone https://github.com/kahwee/denki.git
cd denki
cargo install --path . --locked
denki --help
```

Cargo installs the binary in `~/.cargo/bin`.

## Use

```sh
denki scan
denki info "desk plug"
denki energy "desk plug" --json
denki energy watch "desk plug" --interval 5 --format jsonl > energy.jsonl
denki group off "office" --dry-run
denki on "desk lamp"
```

Save a Tapo P125 alias with `denki alias "tapo plug" 192.0.2.50 --tapo`, then
run `denki login <email>`. Automatic mode negotiates TPAP/KLAP; existing `--klap`
aliases retain the original client. Direct-IP commands reuse a saved protocol;
unknown IPs default to Kasa.

`scan` preserves aliases and reconciles known device identities after address
changes. Use `scan --tapo-target IP` to probe an additional Tapo address.
`group --dry-run` previews targets without contacting devices. Use `--json` for
structured command results or energy watch's JSONL/CSV output for streaming.

Support depends on the model and connection mode. P125 automatic-mode info and
on/off readbacks have hardware evidence; toggle remains unverified and energy
is unsupported. See the [command and support guide](docs/commands.md) for exact
capabilities and [devices.toml](devices.toml) for verification status.

## Documentation

- [Getting started with Kasa or Tapo](docs/getting-started.md)
- [Energy readings, units, and recording](docs/energy.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Commands, aliases, credentials, and limitations](docs/commands.md)
- [Rust library usage](docs/library.md)
- [Architecture and protocols](docs/architecture.md)
- [Development checks and live smoke tests](CONTRIBUTING.md)
- [Agent guidance](AGENTS.md)

## License

MIT
