# Make sense of your energy use

Check an appliance's current draw, collect readings for a spreadsheet, or feed
structured data into your own scripts. Denki reads measurements reported by your
device. It does not infer missing measurements or estimate historical usage.

## Take a reading

```sh
denki energy "desk plug"
```

For a power strip, specify the outlet number shown by `denki outlets`:

```sh
denki outlets "power strip"
denki energy "power strip" 2
```

Energy support depends on the model. Consult the [device table](commands.md#supported-devices);
“unverified” means the implementation still needs confirmation on real hardware.

## Understand the units

| Field | Unit | Meaning |
| --- | --- | --- |
| `power_w` | W | Power at the time of the reading |
| `voltage_v` | V | Voltage, when reported |
| `current_a` | A | Current, when reported |
| `energy_wh` | Wh | Device-reported cumulative energy |
| `today_energy_wh` | Wh | Today's energy, when reported by Tapo |
| `month_energy_wh` | Wh | This month's energy, when reported by Tapo |

1,000 Wh equals 1 kWh. A device drawing 100 W steadily for ten hours would use
1 kWh, but a single 100 W reading does not tell you how much it used earlier.
Counters and their reset periods depend on device firmware. Compare equivalent
periods and fields; do not treat cumulative totals as today's usage.

## Export CSV for a spreadsheet

```sh
denki energy watch "desk plug" --interval 5 --count 120 --format csv > energy.csv
```

The example takes 120 samples, approximately ten minutes plus device response
time. Each request finishes before the next delay begins. Keep Denki running to
collect data; it cannot fill gaps while the computer or device is offline.

Missing values are empty cells. A failed sample has `status=error` and error
columns; it is not a zero-watt reading. Recording continues after a failed sample,
but the final process exit status is nonzero if any sample failed.

## Stream JSONL for automation

```sh
denki energy watch "desk plug" --interval 5 --format jsonl > energy.jsonl
```

Each line is a separate JSON object with a timestamp in UTC Unix milliseconds.
Press Ctrl-C to stop cleanly. Progress and summaries go to stderr, leaving the
file with only data. `--json` is shorthand for JSONL when used with `energy watch`.

One-shot `energy --json` uses a schema-v2 result envelope. Streaming samples use
schema v1 and are not wrapped in that envelope. See the
[JSON contract](commands.md#json-result-contract) and
[streaming reference](commands.md#streaming-energy-measurements) for exact fields.

## Read device history

Kasa energy devices can expose daily and monthly history:

```sh
denki energy-daily "desk plug" 2026-10
denki energy-monthly "desk plug" 2026
```

Omit the period to use the current month or year. Strip history accepts `-o 2`
to select an outlet. Tapo's local API exposes current power and today/month totals
through `energy`; Denki does not support Tapo daily or monthly history commands.

## Practical recording tips

- Give the device a descriptive alias before starting a long recording.
- Compare similar time windows and note changes in how the appliance is used.
- Check `status` before using a row in calculations; preserve gaps as missing data.
- Start a new output file for each recording. Shell `>` replaces an existing file.
- Energy watch reads information only; it never switches the device on or off.
