# Changelog

Notable user-facing changes are grouped by release. Changes under Unreleased
are on `main` and are not part of the published tags below.

## [Unreleased]

### Fixed

- Reject fleet-only concurrency arguments for single-device diagnostics.
- Replace redundant tests with independent crypto vectors and argument-boundary
  checks; consolidate CLI executables and remove duplicate wall-clock waits.


### Security

- Validate Unix configuration files, lock sidecars, and ancestor directories before
  use; reject unsafe permissions, ownership, symlinks, hard links, and special files.
  Create new directories privately and bound configuration reads.
- Reject partial/empty credential environment overrides and sanitize malformed
  credential errors without retaining secret-bearing parser details.


## [0.2.0] - 2026-11-11

### Added

- Read-only `doctor --all` fleet diagnostics with bounded concurrency, per-device
  deadlines, address deduplication, partial failure results, and JSON summaries.
- `identify NAME --confirm` / `--clear` records explicit physical identification
  separately from saved aliases, authentication, and model hardware verification.
  Changing an alias address or protocol clears its physical confirmation.
- Wi-Fi signal diagnostics and actionable suggestions for connectivity,
  authentication, lockout, protocol, and identity failures.

### Changed

- Add physical confirmation to alias output and the public `HostEntry` structure.
  Existing registry files default to unconfirmed; discovery never grants it.
- Keep established connectivity evidence when later diagnostic operations fail.


## [0.1.2] - 2026-11-11

### Fixed

- Separate release verification/builds from fresh-runner OIDC publication; no
  repository scripts or build scripts run with registry authority.
- Stop automatically enrolling new Kasa aliases. Legacy Kasa group targets require
  explicit re-enrollment after independently verifying each address.
- Remove positional login passwords; use the hidden prompt or bounded
  `--password-stdin`. Rejected login arguments are not echoed.
- Add `DENKI_REQUIRE_TPAP=1` to reject non-TPAP login before authentication, and
  warn in compatibility mode about KLAP offline password guessing.
- Replace the HS300 fixture's stable-looking identity and household labels with
  explicitly synthetic data. This does not establish provenance or erase history.

- Upgrade RSA to the patched 0.10.0 release and remove the timing-advisory exception.
- Bound every automatic Tapo HTTP control response before buffering, authenticate
  automatic-mode KLAP replies, and keep authentication off redirects and proxies.
  Bundle the reviewed upstream transport so registry packaging retains the fixes.
- Escape untrusted terminal text and shell hints, including Tapo, effects, outlet
  names and errors. Suppress unsafe or unauthenticated discovery control hints.
  Preserve JSON values with safe Unicode escapes and protect CSV formula strings.
- Validate UDP discovery before deduplication, bound retained sources, and reject
  discovery that conflicts with a saved protocol. Report malformed weekday data
  without panicking.

- Reject Kasa discovery replies that try to move a saved device ID to another IP.
  Unauthenticated discovery can no longer persistently redirect existing aliases.
  Verify legitimate DHCP changes independently and update each affected alias with
  `denki alias NAME IP`; authenticated Tapo reconciliation remains available.

### Changed

- Refresh compatible locked crypto, TLS and TOML dependencies, including AES
  0.9.4 and TOML 1.2.0. Use the patched RSA stable release without an advisory exception.
- Enable future tag-only crates.io publishing with metadata, exact-source CI,
  package and audit checks before short-lived OIDC authentication. Activating the
  workflow does not republish existing tags or prove registry availability.
- Refresh pinned GitHub Actions and the development toolchain to Rust 1.99.1;
  validate documentation with Python 3.15.2. The minimum Rust version remains 1.99.
- Shorten the README and align installation, contributor and release guides with
  verified crates.io availability.

## [0.1.1] - 2026-10-28

### Changed

- Add a starting sequence, Tapo P125 setup guidance and output-format guidance
  to top-level CLI help. Explain that `--json` produces one result for ordinary
  commands and JSONL samples for `energy watch`; CSV uses `--format csv`.
- Put tagged installation, local-network requirements, diagnostics, common device
  commands and model-specific support limits in the README and getting-started guide.
- Define the Cargo source-package contents explicitly: include the device registry,
  compile-time library documentation and offline fixtures; exclude site infrastructure
  and local output. Linux CI validates the inventory and compiles the packaged source
  with a locked publish dry run.

### Distribution

- First crates.io publication of the CLI and library, from `v0.1.1` at
  `2c93521152da2a15fd4d2a8f27e33e916f096ba9`. API documentation is available on
  [docs.rs](https://docs.rs/denki/0.1.1/denki/).
- GitHub provides source archives; prebuilt executables are not included.
  Source builds require Rust 1.99 or newer.
- No device-control protocol or hardware-support claim changes in this patch.
  Capabilities and hardware evidence remain in [devices.toml](devices.toml).

## [0.1.0] - 2026-10-28

### Added

- First GitHub source release of the local-network CLI and library for supported
  TP-Link Kasa and Tapo devices.
- Device discovery, saved aliases, model-specific power and lighting commands,
  energy readings and CSV/JSONL recording, diagnostics and group-operation previews.
- TPAP/KLAP negotiation for the Tapo P125, with capability guards and offline
  protocol, CLI, credential-storage and recovery regression tests.

[Unreleased]: https://github.com/kahwee/denki/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/kahwee/denki/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kahwee/denki/releases/tag/v0.1.0
