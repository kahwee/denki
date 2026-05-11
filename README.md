# denki (電気)

I built this to turn my lights on and off from the terminal. That's it.

I have a few KL135 bulbs, some Kasa switches, and a Tapo plug. I got tired of reaching for my phone, so I wrote a CLI. *denki* means "electricity" in Japanese.

My day-to-day usage looks like this:

```bash
denki on "desk lamp"
denki off "desk lamp"
denki dim "desk lamp" 40
denki scan
```

That's what I actively maintain and use. Everything else in here — energy monitoring, power strips, color control — I've written and it works on my hardware, but I'm not adding more to it myself.

If you have a device or use case that isn't covered, contributions are welcome. The code is structured to make that straightforward.

## My devices

These work on hardware I own:

- **KL135 bulbs** — power, dim, color temp, HSV color, energy
- **KP115 plugs** — power, energy
- **HS105 plugs** — power (no energy chip)
- **HS300 power strips** — power per outlet, energy per outlet
- **P125 Tapo plug** — power via KLAP

Also implemented but I don't actively use:

- **HS110 plugs** — power, energy
- **HS220 dimmers** — power, dim
- **KP303 strips** — same as HS300
- **KL430 light strips** — scan and energy only; power/color not implemented

## Install

```bash
cargo install --path .
```

Or build directly:

```bash
cargo build --release
./target/release/denki --help
```

## Usage

```bash
denki scan                              # find devices on the network
denki info "desk lamp"                  # detailed info
denki on "desk lamp"
denki off "desk lamp"
denki toggle "desk lamp"
denki dim "desk lamp" 50               # 0–100
denki color-temp "desk lamp" 2700      # Kelvin
denki color "desk lamp" --hue 275 --sat 50 --val 80
```

Aliases are saved on first scan. You can also set them manually:

```bash
denki alias "desk lamp" 192.168.1.42
denki aliases
denki unalias "desk lamp"
```

### Energy

```bash
denki energy "desk plug"
denki energy-daily "desk plug" 2025-03
denki energy-monthly "desk plug" 2025
```

### Power strips

```bash
denki outlets "garage strip"           # list outlets
denki on "garage strip" 2             # outlet 2 on
denki off "garage strip" 2
denki energy "garage strip" 2
denki outlet-rename "garage strip" 2 "Coffee Maker"
```

Outlet numbers are 1-based and match the order from `outlets`.

### Tapo devices

```bash
export TAPO_USER="you@example.com"
export TAPO_PASS="your-tapo-password"

denki alias "tapo plug" 192.168.1.50 --klap
denki on "tapo plug"
```

Or save credentials with `denki login` so you don't have to export every time.

## Contributing

I don't plan to expand the feature set beyond what I personally use, but I'll review and merge contributions that add support for other devices or use cases. The code is reasonably well structured for that.

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the local setup and workflow.

A few useful entry points:

- New API calls go in `src/ops.rs`
- New device parsers go in their own module (see `src/plug.rs` for a simple example)
- Add the device to `devices.toml` with its capabilities
- Gate the CLI command with a `devices::can_*` guard in `src/main.rs`

## Architecture

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point — command dispatch and device-type routing |
| `src/cli.rs` | clap command definitions |
| `src/resolve.rs` | Device name/IP resolution, outlet lookup, protocol guards |
| `src/devices.rs` | Capability registry (`devices.toml`), `detect_kind`, command guards |
| `src/ops.rs` | All API calls — `bulb_*`, `relay_*`, `device_*`, `strip_*`, `tapo_*` |
| `src/transport.rs` | Kasa TCP `send()` and UDP `broadcast_each()` |
| `src/cipher.rs` | XOR autokey cipher — `encode` (TCP) / `encode_raw` (UDP) |
| `src/klap.rs` | KLAP handshake and AES-128-CBC session for Tapo devices |
| `src/hosts.rs` | Alias registry — friendly names → IP + protocol, stored as JSON |
| `src/creds.rs` | Tapo credential load/save |
| `src/fmt.rs` | Shared formatting helpers |
| `src/bulb.rs` | KL135/KL430 sysinfo parsing |
| `src/plug.rs` | KP115/HS110/HS105 sysinfo parsing |
| `src/dimmer.rs` | HS220 dimmer sysinfo parsing |
| `src/strip.rs` | HS300/KP303 power strip sysinfo and per-outlet state |
| `src/tapo.rs` | Tapo `get_device_info` response parsing |
| `src/display.rs` | Colored terminal output |
| `src/lib.rs` | Library re-exports |

## Protocol notes

**Kasa (port 9999):** XOR autokey cipher, key starts at `171`. TCP adds a 4-byte big-endian length prefix; UDP does not.

**KLAP / Tapo (port 80):** Two-step handshake, then AES-128-CBC. Uses raw `tokio::net::TcpStream` because some Tapo firmware rejects standard HTTP clients.

## License

MIT
