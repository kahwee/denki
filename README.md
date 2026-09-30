# denki (電気)

Control TP-Link Kasa and Tapo devices on your local network. Inspect energy
usage, operate lights and plugs, and combine commands in shell scripts.

## Install

Requires Rust 1.97 or newer:

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
denki energy "desk plug"
denki group off "office" --dry-run
denki on "desk lamp"
```

`scan` saves discovered Kasa aliases and probes saved Tapo aliases. Add Tapo
devices with `denki alias "tapo plug" 192.168.1.51 --klap`, then run `denki login`
with your account email. `group --dry-run` lists matches without contacting devices.

Only models marked verified in [devices.toml](devices.toml) have hardware evidence.
Commands depend on each device's capabilities; Tapo daily/monthly history differs
from Kasa. See the [command and support guide](docs/commands.md).

## Documentation

- [Commands, aliases, credentials, and limitations](docs/commands.md)
- [Rust library usage](docs/library.md)
- [Architecture and protocols](docs/architecture.md)
- [Development checks and live smoke tests](CONTRIBUTING.md)
- [Agent guidance](AGENTS.md)

## License

MIT
