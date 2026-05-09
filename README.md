# denki (電気)

Command-line tool for controlling TP-Link smart bulbs and plugs over your local network. No cloud required.

*denki* means electricity in Japanese.

## Supported Devices

| Device | Type | Energy | Color |
|--------|------|--------|-------|
| KL135 | Smart bulb | Yes | Yes (HSV + color temp) |
| KP115 | Smart plug | Yes (V/A/W) | — |
| HS110 | Smart plug | Yes (V/A/W) | — |
| HS105 | Smart plug mini | No | — |

> Newer Tapo devices (P125, L530) use a different protocol and are not yet supported.

## Install

### Prerequisites

You need Rust. If you don't have it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from source

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --release
```

The binary will be at `./target/release/denki`.

### Put it on your PATH (optional)

```bash
cp target/release/denki /usr/local/bin/
```

Or on macOS with Homebrew's prefix:

```bash
cp target/release/denki /opt/homebrew/bin/
```

## Usage

### Find your devices

```bash
denki scan
```

This broadcasts a UDP discovery packet and lists everything it finds. Devices must be on the same network as your computer.

```
== Office Bulb == [192.168.1.42] on  good signal
   KL135 HW:2.6  FW:1.0.9  2700K  80%
```

### Control power

```bash
denki power 192.168.1.42 on
denki power 192.168.1.42 off
denki power 192.168.1.42 toggle
```

### Bulb: brightness, color temperature, color

```bash
denki dim 192.168.1.42 50          # 0–100%
denki warmth 192.168.1.42 2700     # 2500–9000 Kelvin (warm to cool white)
denki color 192.168.1.42 275 50 80 # hue (0–360), saturation (0–100), value (0–100)
```

### Device info

```bash
denki info 192.168.1.42
```

### Energy usage

```bash
denki energy 192.168.1.42                   # real-time watts
denki energy-daily 192.168.1.42 2025-03     # daily breakdown for a month
denki energy-monthly 192.168.1.42 2025      # monthly totals for a year
```

### Other commands

```bash
denki specs 192.168.1.42          # hardware specs — lumens, CRI, wattage (bulbs)
denki presets 192.168.1.42        # saved light presets (bulbs)
denki schedules 192.168.1.42      # scheduled on/off rules (plugs)
denki led 192.168.1.42 off        # turn off the status LED (plugs)
denki clock 192.168.1.42          # show device clock (plugs)
denki rename 192.168.1.42 "Desk Lamp"
denki restart 192.168.1.42
```

## How it works

All legacy Kasa devices communicate over TCP port 9999 using a simple XOR autokey cipher. denki implements this cipher directly — no cloud, no account, no app required.

Discovery uses UDP broadcast to the same port. The device responds with its full sysinfo JSON.

## License

MIT
