# Contributing to denki

Thanks for improving `denki`.

## Local setup

Development and CI use Rust 1.99.0, selected by `rust-toolchain.toml` when using
rustup. The minimum supported Rust version remains 1.97.

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

## Releases

Use the [Denki release skill](.agents/skills/denki-release/SKILL.md) to prepare
a release from green main, create an annotated tag matching the Cargo package
version, and publish and verify the GitHub release. The guide also covers
optional binary artifacts and separately requested crates.io publication.

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

## Offline automation regression coverage

`cargo test --locked` includes real CLI subprocesses against a local Kasa TCP
simulator (`tests/automation_cli.rs`, Linux). Scenarios cover normalized energy,
JSON purity, malformed/rejected replies, no mutation after invalid state or identity,
DHCP reconciliation, dry runs and partial group failures, JSONL/CSV, failed-sample
recovery, and Ctrl-C. The simulator binds only `127.0.0.1:9999` and never contacts
physical devices. Keep these scenarios in one test to avoid fixed-port races.

`src/klap_tests.rs` uses an ephemeral loopback HTTP peer to exercise KLAP handshakes,
request signatures, AES framing, advancing sequence numbers, strict response codes,
and rejection of malformed state before any mutation. Identity/energy unit tests
cover swapped addresses, legacy registry migration, collisions, unit normalization,
and missing data. These checks are offline evidence, not hardware verification.
