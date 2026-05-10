# CLAUDE.md — denki

Developer notes for the `denki` repo.

`denki` is a Rust CLI for controlling TP-Link Kasa and Tapo smart devices over the local network.

## Local build loop

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

## High-level architecture

- `src/main.rs` — CLI, command dispatch, device-kind detection, guard rails
- `src/ops.rs` — protocol calls grouped by device family
- `src/transport.rs` — Kasa TCP/UDP transport
- `src/klap.rs` — Tapo KLAP handshake/session logic
- `src/hosts.rs` — alias storage and lookup
- `src/fmt.rs` — shared formatting helpers
- `src/display.rs` — terminal formatting
- `src/*` device modules — parsing and per-device helpers

## Behavior to preserve

- Keep commands local-network-first; no cloud dependency.
- Fail fast on unsupported device/command combinations with a clear error.
- Keep scan/list output readable and close to arrival order.
- Prefer one small command-specific function over large multi-purpose branches.

## Command mapping

The CLI surface is:

- `scan`, `info`, `on`, `off`, `toggle`
- `dim`, `warmth`, `color`
- `energy`, `energy-daily`, `energy-monthly`
- `specs`, `presets`, `schedules`, `led`, `clock`
- `outlets`, `outlet`, `outlet-energy`, `outlet-energy-daily`, `outlet-energy-monthly`, `outlet-rename`, `rename`, `restart`
- `alias`, `unalias`, `aliases`, `login`

## Docs to update together

If you change behavior, update the README plus the inline comments/doc comments in the affected source file.

## Contributing

See `CONTRIBUTING.md` for the local workflow and checklist.
