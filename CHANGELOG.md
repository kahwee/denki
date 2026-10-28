# Changelog

Notable user-facing changes are grouped by release. Changes under Unreleased
are on `main` and are not part of the published tags below.

## [Unreleased]

### Changed

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
