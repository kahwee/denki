# Actions publishing

`python3 scripts/check-release.py v<version>` verifies the package version, tag
and root Cargo.lock entry without building or publishing.
`python3 scripts/test_check_release.py` covers version mismatch regressions.
Keep the repository release skill and existing required CI as the authority.

## Approval proposal

[The complete crates.io workflow](../.github/release-proposals/publish-crate.yml)
is outside `.github/workflows` and cannot run. Activation requires approval to
move it to `.github/workflows/publish-crate.yml`, grant its publish job
`contents: read`, `actions: read`, `id-token: write`, and bind crate `denki` to:

- owner `kahwee`, repository `denki`
- GitHub Actions workflow filename `publish-crate.yml`
- environment blank

Use [crates.io trusted publishing](https://crates.io/docs/trusted-publishing) and
the [official temporary authentication action](https://github.com/rust-lang/crates-io-auth-action).
It checks the human-pushed stable version tag and locked metadata, requires exact
source CI, verifies packaging, then authenticates and publishes. GitHub publication
and registry publication remain separate. No long-lived Cargo credential is added.
Existing versions cannot be overwritten: an already published version requires
read-back source/checksum reconciliation rather than automatically declaring success.

No registry trust or workflow access was enabled by this change. Current local
Cargo credential file existence does not validate publishing authentication.
If the crate does not yet exist, first publication/name ownership is a separate
approved owner action before a trusted-publisher binding can be established.

## GitHub source releases

Denki currently has no automatic GitHub release creator. Approval is also needed
for a tag-triggered job with `contents: write` that checks tag/locked versions,
requires successful exact-source CI, and creates a source-only GitHub release.
Binary artifacts require an explicitly supported target matrix and built checksums;
this proposal does not invent binary support or touch devices.

[The source-release workflow](../.github/release-proposals/release-source.yml) is
also inactive. Approval must cover moving it to `.github/workflows/release-source.yml`
and granting its job `contents: write` and `actions: read`. It creates a GitHub
source release for a matching existing tag after exact-source CI. It never moves
a tag; an existing release makes creation fail safely and requires read-back.

If activated, the crate publisher proposal would be triggered directly by human-pushed `v*` tags; no `release: published` cascade from `GITHUB_TOKEN` is assumed. It would run independently of GitHub source/binary publication. Existing immutable historical tags without these new guard files need a separately reviewed recovery procedure; this proposal does not move or republish them.

CI runs the actual metadata CLI against the checked-out manifest/lock and rejects
a mismatched tag. Linux CI also validates the actual Cargo package inventory and
runs `cargo publish --locked --dry-run` to compile the packaged source. These
checks do not prove registry authentication or publication. The publisher proposal
remains inactive; its registry exchange and upload have not been exercised.

See [GitHub Actions maintenance](../.github/ACTIONS.md) for the verified action
versions, including the pins in the inactive proposals.
