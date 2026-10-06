# Your first device

Go from an installed CLI to your first energy reading. Denki runs on your computer
and talks to devices on your local network. Keep the computer and device on a
network that allows them to reach each other; guest Wi-Fi often blocks this.

## 1. Install Denki

Install [Rust with rustup](https://rustup.rs/) first. Denki requires Rust 1.99 or
newer; the repository selects 1.99.0 automatically.

```sh
git clone https://github.com/kahwee/denki.git
cd denki
cargo install --path . --locked
denki --help
```

Cargo installs `denki` into `~/.cargo/bin` by default. If your shell cannot find
it, restart the shell after installing Rust or add that directory to `PATH`.

## 2. Connect a Kasa device

Scan your network, then use a name from the results:

```sh
denki scan
denki aliases
denki info "desk plug"
```

Replace `desk plug` with your own alias. Scan saves discovered devices and their
identities. Later scans can update their addresses after DHCP changes while
preserving your chosen names.

If discovery does not find a device, look up its IP in your router and add it:

```sh
denki alias "desk plug" 192.0.2.42
denki info "desk plug"
```

The `192.0.2.x` addresses in this guide are placeholders. Replace them with your
device's actual local IP.

## 2. Connect a Tapo device instead

Tapo requires account credentials. For P125 plugs using TPAP, add the IP with
`--tapo` (`--tpap` is a synonym); the client negotiates TPAP/KLAP automatically.
For other registered Tapo models, use the original `--klap` client. Save the
credentials for the TP-Link account associated with the device. `login` prompts
for the password so it does not go into your shell history.

```sh
denki alias "tapo plug" 192.0.2.50 --tapo
denki login "you@example.com"
denki info "tapo plug"
```

Use the saved alias for Tapo commands: a raw IP is treated as Kasa. Scan probes
saved Tapo aliases; it does not automatically discover unknown Tapo IPs. See the
[command guide](commands.md#discover-devices) for finding a moved Tapo device.

## 3. Read energy

Use an energy-capable model from the [support table](commands.md#supported-devices).
HS105 and P125, for example, support power control but do not measure energy.

```sh
denki energy "desk plug"
denki energy "desk plug" --json
```

Watts describe power right now. Watt-hours describe accumulated energy. Missing
measurements are unavailable, not zero. Read the [energy guide](energy.md) before
comparing readings or collecting a history.

## 4. Collect a short recording

```sh
denki energy watch "desk plug" --interval 5 --count 12 --format csv > energy.csv
```

This takes 12 readings with a five-second delay after each completed sample.
Open `energy.csv` in a spreadsheet. Omit `--count` to keep recording until Ctrl-C.

## 5. Control power when you are ready

These commands change the device's power state:

```sh
denki on "desk plug"
denki off "desk plug"
```

For a group, preview the matches first. The first command below does not contact
devices; the second switches matching devices off.

```sh
denki group off "office" --dry-run
denki group off "office"
```

If anything fails, start with the [troubleshooting guide](troubleshooting.md).
