# Contributing to denki

Thanks for improving `denki`.

## Local setup

Development and CI use Rust 1.99.0, selected by `rust-toolchain.toml` when using
rustup. The minimum supported Rust version is 1.99. Rust 1.99 is the sole
tested toolchain; language and standard-library features stabilized through
1.99 may be used.

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

## CI concurrency

Workflows use GitHub Actions `parallel` groups for independent steps. Rust tests
run alongside formatting and generated-doc checks; Clippy and rustdoc follow
sequentially to reuse Cargo build outputs. Documentation builds and link checks
run alongside browser installation and the support-table check. Each group must
succeed before dependent steps run, including browser tests and artifact upload.

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

The support table in `docs/commands.md` is generated from `devices.toml`. After
changing the registry, run `python3 scripts/update-device-docs.py` (Python 3.11+).
CI runs the same script with `--check` to reject stale tables. Keep hardware
verification claims tied to actual device evidence.

`docs/library.md` is also the crate's rustdoc page. Its `no_run` examples compile
as part of `cargo test --locked` without connecting to any devices. Keep complete
imports, dependencies, and async entry points in those examples.

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

The repository includes ignored integration tests that call Denki handlers and
the CLI against explicitly selected live devices. It is useful for verifying actual on/off behavior
without turning the main test suite into a network-dependent job.

```bash
cargo test --test live_smoke -- --ignored --nocapture
```

Environment variables:

- `DENKI_SMOKE_POWER_TARGETS`: comma-separated aliases to power cycle
- `DENKI_SMOKE_LIGHTSTRIP`: optional alias for a light-strip effect cycle

Power tests require `DENKI_SMOKE_POWER_TARGETS` explicitly; there is no default
target. They read the initial state, verify both states, and restore the initial
state using the shared harness in `tests/support/power_cycle.rs`. Power strips
are refused because restoring a single state would lose individual outlet states.

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

Hardening regressions additionally cover an independently generated KLAP ciphertext
vector, bit flips throughout its tag and ciphertext, replay/cross-session rejection,
and a tampered response from the local HTTP peer with no subsequent power mutation.
Storage tests inject write/rename failures, verify temporary permissions before the
first write, and test stale scan snapshots. Linux CLI subprocess tests race alias
adds/removals while reading JSON, check credentials under umask 000, and kill a lock
holder to verify automatic OS lock release. No live devices are used.

## CI

[CI](.github/workflows/ci.yml) runs on GitHub-hosted runners: tests, Clippy,
formatting, rustdoc, and dependency audit on Linux, plus Linux/macOS release
builds. Pushes to `main`, pull requests, and manual runs execute these checks.
To trigger a manual run, use `gh workflow run ci.yml --ref main`.

## Dependency maintenance

Dependabot checks Cargo dependencies every Monday at 09:00 America/Los_Angeles.
Minor and patch updates are grouped in one PR; major updates remain individual
PRs, with at most five Cargo update PRs open at once. GitHub Actions dependencies
continue to receive weekly grouped updates. Dependency PRs run the normal Rust
1.99.0 checks and require review; these settings do not enable automatic merging.

CI also runs the dependency audit every Monday at 17:23 UTC so new advisories are
checked even when no code changes. Scheduled runs skip tests and release builds;
push, pull-request, and manual runs retain the full checks. Separate concurrency
groups keep a scheduled audit from cancelling a full CI run. GitHub may delay
scheduled jobs; the cron time is the requested start, not a delivery guarantee.

The pinned Tapo dependency brings `rsa`, whose timing advisory
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) has no
patched release. `.cargo/audit.toml` records the scoped exception: Tapo 0.11.1
only generates a discovery key and exports the public key, with no RSA decryption
or signing. Reassess the call sites whenever Tapo/RSA changes or another RSA
consumer is introduced. All other advisories remain checked.

## Documentation website

GitHub Pages serves the guides at https://kahwee.github.io/denki/. Edit the Markdown
files in `docs/`; the website and repository share the same guide content. The
landing page, styles, and interactive sample demo live in `docs/site/`.

Build and preview with Python 3.11+:

```sh
python3 -m venv .venv
.venv/bin/pip install -r docs/site/requirements.txt
.venv/bin/python scripts/build-docs.py
.venv/bin/python scripts/check-docs.py
python3 -m http.server 8000 --directory _site
```

For browser regression checks, run `.venv/bin/playwright install chromium` and
`.venv/bin/python scripts/test-docs.py`. CI runs these checks before deployment.

Open http://localhost:8000. Code blocks get copy buttons automatically. Keep sample
data clearly labeled; the demo must never contact real devices or ask for account
credentials. Commands generated by the demo target POSIX shells.

The Documentation workflow checks internal links and builds every PR. Main pushes
publish the tested artifact with GitHub Pages. In repository Settings → Pages,
the build source must be **GitHub Actions**. No custom domain is required.

### P125 adapter power round trip

This separate ignored test exercises the upstream Tapo adapter. Select a plug
on a suitable load explicitly; there is no default address or saved-alias update:

```sh
DENKI_TAPO_TEST_IP='<plug-ip>' cargo test --locked --test live_tapo p125_power_round_trip -- --ignored --exact --nocapture
```

Optionally set `DENKI_TAPO_TEST_NAME` to the exact device nickname to check the
selected target before power changes.

It reads the initial state, sets the opposite state, verifies it with separate
information reads (at most three), and restores and verifies the initial state.
It uses Denki's saved credentials or `TAPO_USER`/`TAPO_PASS`. Errors fail the test,
even if cleanup succeeds. Cleanup first checks device identity and state, then
attempts restoration once if needed. Authentication errors skip cleanup login;
restoration cannot be guaranteed if the device disconnects or the process is
terminated. The result verifies the device's reported relay state; observe the
physical load separately. Do not commit real addresses or diagnostic output.

The ordinary `cargo test --locked` suite runs this harness's offline failure and
restoration tests, while leaving the hardware test ignored.

### Sharing harness code

`tests/support/power_cycle.rs` owns the common `Plug` interface, bounded readback
polling, identity checks, and best-effort restoration. `tests/power_cycle.rs`
exercises failure handling offline. `tests/live_tapo.rs` selects an explicit P125
and adapts the upstream client; `tests/live_smoke.rs` reuses the same cycle for
selected Kasa, KLAP, or auto-mode aliases. New hardware tests should implement the
small interface instead of duplicating switching or cleanup logic. Authentication
and identity failures use structured error codes to stop further writes/login
attempts. Live tests stay ignored in CI; never add a real address as a default.
