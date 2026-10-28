---
name: denki-release
description: Prepare, tag, publish, and verify a Denki GitHub release from green main. Use when asked to release Denki, cut a version, create a release tag, or prepare its release process.
---

# Release Denki

Follow the repository's AGENTS.md. Complete an explicitly requested release through
publication and verification. A request to prepare or document a release authorizes
preparation only; leave tags and releases untouched in that case.

## Inspect the current source

- Resolve the repository from `git remote get-url origin`; do not assume an owner.
- Read Cargo.toml, rust-toolchain.toml, Cargo.lock, and .github/workflows/.
- Fetch main and tags. Preserve unrelated working-tree changes.
- Inspect existing remote tags and GitHub releases with `gh`. Treat an existing
  tag as immutable; compare its commit before reusing it, and never force-retag.
- Read the package version and minimum Rust version from Cargo.toml. Match tags
  to `v<package-version>`. If the current version is unreleased, keep that version.
  For a new version, follow the requested version or determine the SemVer change
  from the changes since the last release; clarify when that cannot be determined.
- Resolve the full commit SHA to release. Require it to be present on remote main.
  If release preparation changes source or version files, commit and publish them
  to main first when authorized, then use that new SHA.

## Prepare and validate

When changing the package version, update Cargo.toml and the root package entry in
Cargo.lock together. Do not upgrade unrelated dependencies as part of a release.

Run the repository's checks on the exact source being released:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --document-private-items
cargo build --locked --release
cargo publish --dry-run --locked
```

Verify the minimum supported Rust version when the source or dependencies changed;
select it from Cargo.toml, not a hardcoded value. Never run ignored live smoke tests
or contact physical devices for routine release validation. Tests and compilation
must not change hardware-verification claims in devices.toml.

Require a successful CI workflow run for the exact release commit. Inspect every
required job, including dependency audit and Linux/macOS builds. If a check fails,
fix it within the authorized scope, publish the correction, and verify the new
commit before tagging. Do not treat a queued run or a different commit as green.

## Prepare release notes and artifacts

Write release notes to a temporary file outside the repository. Describe supported
behavior, meaningful changes, installation, and any actual validation limits. Do
not include credentials, real device addresses, or diagnostic captures.

Inspect the current release automation. New stable version tags start the active
crates.io publisher, but the GitHub source-release creator remains an inactive
proposal. A GitHub source release is a separate step; no prebuilt binaries are
promised. See docs/actions-publishing.md for the current workflow and recovery rules.

If the user asks for binary distribution or release automation, implement or use
a workflow that builds the exact tagged SHA, verifies the tag against Cargo.toml,
runs required checks, and packages the executable plus LICENSE for each explicitly
supported OS/architecture. Use the actual build target in archive names, generate
SHA-256 checksums, and attach the archives and checksum file to the release. Label
platform support from evidence, not from runner names alone.

## Tag and publish

Before tagging, confirm the external crates.io trusted-publisher binding matches
owner `kahwee`, repository `denki`, workflow `publish-crate.yml`, blank environment.
For a release request, create an annotated `v<version>` tag at the verified full SHA
and push that tag. Use task-specific shell variables and proper shell quoting.
Use the installed `gh` authentication normally; never print tokens or inspect secret
files. If HTTPS git authentication fails, first try `gh auth setup-git`.

If normal git transport still fails but the GitHub API has write access, use the
Git-data API instead:

1. POST `/repos/{owner}/{repo}/git/tags` with `tag`, `message`, `object` set to the
   full release commit SHA, and `type` set to `commit`.
2. Copy the returned tag-object SHA into POST `/repos/{owner}/{repo}/git/refs`
   with `ref` set to `refs/tags/v<version>` and `sha` set to that tag-object SHA.
3. Fetch the tag and verify `git rev-parse v<version>^{commit}` equals the intended
   release SHA. A conflict requires checking the existing tag, not overwriting it.

Create the GitHub release from the existing verified tag using `gh release create`
with `--verify-tag`, `--title`, and `--notes-file`. Respect an explicit draft or
prerelease request; otherwise publish the requested release. If a release already
exists, inspect it and complete only missing requested work.

A new stable tag also starts `.github/workflows/publish-crate.yml`. It validates
metadata, exact-source complete main CI, package inventory/dry run and a fresh
audit before requesting a short-lived OIDC credential. Monitor that tag's run,
then verify the registry version and docs.rs build. Do not duplicate its upload
with a manual publish. Activation does not backfill existing tags or prove the
external trust binding. Never publish credentials or start an interactive login
to discover access; report registry publication as pending when trust is unavailable.

## Verify and report

- Verify the remote tag resolves to the intended commit.
- Read back the GitHub release, its draft/prerelease status, and any uploaded assets.
- If tag-triggered release automation exists, wait for its terminal successful result.
- Confirm local main and remote main are aligned after publishing source changes,
  and report any preserved unrelated changes accurately.
- Return the version, full or abbreviated commit SHA, release URL, CI result, and
  actual artifacts. Distinguish source releases from binary and crates.io releases.
