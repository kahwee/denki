# Contributing to denki

Thanks for improving `denki`.

## Local setup

```bash
git clone https://github.com/kahwee/denki.git
cd denki
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

## Development workflow

1. Make a small, focused change.
2. Update the README, `src/cli.rs` help text, and inline docs when behavior changes.
3. Add or update tests for parser or CLI behavior.
4. Run the checks above before opening a PR.

## Code style

- Keep protocol logic in the device/module that owns it.
- Keep device-kind detection and command capability guards in `src/devices.rs`.
- Keep CLI argument definitions in `src/cli.rs` and command dispatch in `src/main.rs`.
- Prefer clear error messages over silent fallthrough.
- Avoid changing network behavior unless the change is verified on a real device.

## Good places to add tests

- CLI parser/argument validation (`src/cli.rs`, `src/devices.rs`)
- device-kind detection (`detect_kind` in `src/devices.rs`)
- capability guards for unsupported commands (`can_*` functions in `src/devices.rs`)
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

## Notes

`denki` is intentionally local-network-first. Please avoid adding cloud dependencies unless there is a clear reason and a minimal, documented fallback.
