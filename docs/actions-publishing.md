# Releases and registry publishing

GitHub releases, crates.io and docs.rs are separate destinations. Confirm each
version before reporting it as available. Follow the [release skill](../.agents/skills/denki-release/SKILL.md)
for source preparation and exact-source CI; never move or reuse an existing tag.

## Checks before a release

- `python3 scripts/check-release.py v<version>` requires the stable tag, manifest
  and root lockfile versions to agree.
- Linux CI checks Cargo's actual package inventory and runs
  `cargo publish --locked --dry-run` to compile the packaged source.
- Require successful CI on the exact commit, including Linux/macOS builds and
  the dependency audit. A dry run does not authenticate or upload a crate.

## Inactive automation proposals

Both [publish-crate.yml](../.github/release-proposals/publish-crate.yml) and
[release-source.yml](../.github/release-proposals/release-source.yml) remain outside
`.github/workflows`; neither can run. Ordinary main pushes do not publish a crate
or create a GitHub release. Current uploads use separately authorized release steps.

Activating the crate proposal requires approval for job permissions
`contents: read`, `actions: read`, `id-token: write` and a crates.io trusted-publisher
binding for owner `kahwee`, repository `denki`, workflow `publish-crate.yml`, blank
environment. It would validate a user-pushed `v*` tag and exact-source main CI,
verify the package, then request a short-lived credential from the
[official authentication action](https://github.com/rust-lang/crates-io-auth-action).
No long-lived Cargo secret is proposed. First publication/name ownership and
registry trust are separate operations.

Activating the source-release proposal requires approval for `contents: write`
and `actions: read`. It would create a source-only GitHub release for a matching
existing tag after exact-source CI. Binary distribution requires a separately
supported build matrix and checksums.

Existing versions cannot be overwritten. Read back registry state and checksums
before retrying an uncertain upload; do not duplicate an in-progress release.
Historical tags without these workflows need a separately reviewed recovery path.
See [Actions maintenance](../.github/ACTIONS.md) for reviewed pins and workflow behavior.
