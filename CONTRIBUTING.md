# Contributing to denki

Thanks for improving `denki`.

## Local setup

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build --locked
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo doc --locked --no-deps --document-private-items
```

## Development workflow

1. Make a small, focused change.
2. Update the README, `src/cli.rs` help text, and inline docs when behavior changes.
3. Add or update tests for parser or CLI behavior.
4. Run the checks above before opening a PR.

## Code style

- Keep protocol logic in the device/module that owns it.
- Keep device-kind detection and command capability guards in `src/devices/`.
- Keep CLI argument definitions in `src/cli.rs`, command handlers in `src/commands/` / `src/admin/`, and dispatch in `src/app/`.
- Prefer clear error messages over silent fallthrough.
- Avoid changing network behavior unless the change is verified on a real device.

## Good places to add tests

- CLI parser/argument validation (`src/cli.rs`, `src/devices/`)
- device-kind detection (`detect_kind` in `src/devices/`)
- capability guards for unsupported commands (`can_*` functions in `src/devices/`; coverage in `src/app/tests/capabilities.rs`)
- response parsing for Kasa/Tapo payloads

## Documentation

If you change command names, supported devices, or output format, update:

- `README.md`
- `src/cli.rs` clap help text
- relevant inline doc comments in `src/`
- any examples in `CLAUDE.md` if they are affected

## Reporting a bug

Include:

- the device model
- whether it is Kasa or Tapo
- the exact command you ran
- the error output
- whether the device was discovered by scan or added with `denki alias`
- sanitized output from `denki doctor "<device>" --json`

## Notes

`denki` is intentionally local-network-first. Please avoid adding cloud dependencies unless there is a clear reason and a minimal, documented fallback.

## Live Smoke Harness

The repository includes an ignored integration harness that runs the real `denki`
binary against live devices. It is useful for verifying actual on/off behavior
without turning the main test suite into a network-dependent job.

```bash
cargo test --test live_smoke -- --ignored --nocapture
```

Environment variables:

- `DENKI_SMOKE_POWER_TARGETS`: comma-separated aliases to power cycle
- `DENKI_SMOKE_LIGHTSTRIP`: optional alias for a light-strip effect cycle

If `DENKI_SMOKE_POWER_TARGETS` is unset, the harness defaults to
`Living Room Right Lamp`.


See [architecture and protocols](docs/architecture.md) for module ownership and
[device commands](docs/commands.md) for support and usage.
