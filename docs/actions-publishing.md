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
   CI on that exact source, including tests, Linux/macOS builds and audit.
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
Only then does the [official authentication action](https://github.com/rust-lang/crates-io-auth-action)
request a short-lived OIDC credential for one locked publish.

The job has `contents: read`, `actions: read`, `id-token: write`, and no GitHub
environment or long-lived Cargo secret. The separate registry binding must match
owner `kahwee`, repository `denki`, workflow `publish-crate.yml`, blank environment.
The matching binding was configured and read back on 2026-10-28. No tag-triggered
OIDC upload has been exercised; the initial 0.1.1 upload used existing Cargo authentication.

## Recovery and GitHub releases

Published versions and tags are immutable. Read back registry state and checksums
before retrying an uncertain upload; never duplicate an in-progress publication.
Tags created before activation do not acquire publisher runs retroactively and
need separately reviewed recovery. Do not repush them to trigger an upload.

The [source-release proposal](../.github/release-proposals/release-source.yml)
remains inactive. Activation needs separate approval for `contents: write` and
`actions: read`. Binary distribution needs a supported build matrix and checksums.
See [Actions maintenance](../.github/ACTIONS.md) for reviewed pins and workflow behavior.
