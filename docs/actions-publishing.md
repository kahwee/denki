# Releases and registry publishing

GitHub releases, crates.io and docs.rs are separate destinations. Verify each
before reporting it as available. Denki 0.1.1 is published on
[crates.io](https://crates.io/crates/denki/0.1.1), and
[docs.rs build 5044276](https://docs.rs/crate/denki/0.1.1/builds/5044276) succeeded.
That initial upload used immutable `v0.1.1` at commit
`2c93521152da2a15fd4d2a8f27e33e916f096ba9`; workflow activation does not republish it.

## Future releases

1. Follow the [release skill](../.agents/skills/denki-release/SKILL.md). Update the
   manifest/root-lock versions and relevant documentation together.
2. Run the local checks and package preflight, then push main. Require successful
   CI on that exact source, including tests, Linux builds and audit.
3. Confirm the external crates.io trusted-publisher binding, then push one new
   stable `v<version>` tag matching the manifest and root lockfile. Never reuse
   or move an existing tag.
4. Monitor the tag's Publish crate run. Verify the registry version and docs.rs
   build separately. Create a GitHub source release separately when authorized;
   the source-release proposal remains inactive and no prebuilt binaries are promised.

## Tag-only publisher

[Publish crate](../.github/workflows/publish-crate.yml) is active for future `v*`
tag pushes from user accounts. Branch pushes, pull requests and manual dispatches
do not invoke it. It checks stable tag/manifest/root-lock agreement, confirms the
tagged SHA is on main with successful complete push CI, validates Cargo's actual
package inventory, runs a locked publish dry run and audits locked dependencies.
These checks and all repository/build code run in the read-only `verify` job,
which has `contents: read` and `actions: read` but no OIDC permission. It saves the
verified `.crate` plus a record of its package version, source SHA and SHA-256 digest
as a short-lived artifact. Checkout does not retain GitHub credentials.

A separate `publish` job starts on a fresh runner with only `id-token: write`.
It never checks out the repository, executes repository scripts or local actions,
restores repository caches, or compiles package code. It downloads the exact
immutable artifact ID produced by `verify`. Inline workflow code checks its
identity, digest, packaged manifest/lock/VCS metadata, member count and sizes.
Unexpected files, non-normal or escaping paths, duplicates, file/directory conflicts,
symlinks, hardlinks, devices and sparse files are rejected. Compressed artifacts
are limited to 16 MiB, decompressed tar data to 64 MiB, individual files to 8 MiB,
and member count to 4096. Extraction creates ordinary files without honoring
archive ownership or executable bits.

Only then does the [official authentication action](https://github.com/rust-lang/crates-io-auth-action)
request a short-lived credential. The final command uses explicit Rust 1.99.1,
`--no-verify --locked --registry crates-io`, an empty isolated `CARGO_HOME` and a
clean working directory outside the extracted source. Packaged build scripts,
`.cargo` configuration and toolchain overrides cannot execute in this job. OIDC
permission is job-wide, so moving the authentication step later in a build job
would not provide this separation.

There is no GitHub environment or long-lived Cargo secret. The separate registry
binding must match owner `kahwee`, repository `denki`, workflow `publish-crate.yml`,
blank environment. The matching binding was configured and read back on 2026-10-28.
This workflow change neither configures nor re-verifies it. No tag-triggered OIDC
upload has been exercised; the initial 0.1.1 upload used existing Cargo authentication.

## Integrity and offline checks

Run `python3 scripts/test_publish_workflow.py -v` to test the exact inline archive
validator with ordinary and malicious fixtures and check the publisher's structural
boundary. Ordinary CI and tag verification run this stdlib-only suite. It performs
no authentication, network publication or compilation. After its explicit-registry
publish dry run, CI also runs `--real-preflight` to validate the actual clean
Cargo artifact through the recorder and publisher validator. This catches output
layout or metadata changes before a release tag is pushed.

The stored SHA-256 verifies the artifact transferred between jobs. Cargo publishes
from its extracted source and repackages it. The extractor omits Cargo's generated
`Cargo.toml.orig`, a reserved input filename that Cargo recreates from the normalized
manifest. Archive bytes and generated manifest metadata can change, and packaging
outside Git omits `.cargo_vcs_info.json`. Do not claim that the
registry archive must have the preflight digest. Verify the registry's own checksum
and compare source contents and meaningful normalized manifest fields separately.
The workflow and pinned actions remain trusted code; the digest and recorded source
SHA are transfer/provenance checks, not an independent proof that arbitrary build
code produced the reviewed source. Protect and review tag/workflow changes and
require the exact-source main CI gate.

## Recovery and GitHub releases

Published versions and tags are immutable. Read back registry state and checksums
before retrying an uncertain upload; never duplicate an in-progress publication.
Tags created before activation do not acquire publisher runs retroactively and
need separately reviewed recovery. Do not repush them to trigger an upload.

The [source-release proposal](../.github/release-proposals/release-source.yml)
remains inactive. Activation needs separate approval for `contents: write` and
`actions: read`. Binary distribution needs a supported build matrix and checksums.
See [Actions maintenance](../.github/ACTIONS.md) for reviewed pins and workflow behavior.
