# GitHub Actions maintenance

This guide describes the checked-in workflows. Follow each linked YAML file for branch filters, job dependencies, permissions, and release conditions.

## Workflows

| Workflow | Events | Jobs |
| --- | --- | --- |
| [ci.yml](workflows/ci.yml) | `workflow_dispatch`, `schedule`, `push`, `pull_request` | `test`, `audit`, `build` |
| [pages.yml](workflows/pages.yml) | `push`, `pull_request`, `workflow_dispatch` | `build`, `deploy` |

## Parallel steps

Independent checks use GitHub Actions [native parallel steps](https://github.blog/changelog/2026-06-25-actions-steps-can-now-be-run-in-parallel/). The following groups run concurrently within a job:

- [ci.yml](workflows/ci.yml), job `test`: `Test`; `Check formatting`; `Check generated device documentation`.
- [pages.yml](workflows/pages.yml), job `build`: `Check generated device documentation`; `Build documentation and check links`; `Install browser`.

Steps after a group wait for it to finish. Keep prerequisites before the group and dependent build, package, or deployment work afterward. Do not run commands that overwrite the same build directory or coverage output together. Parallel steps share the job workspace; they do not provide separate machines.

## Action versions

Verified against upstream stable releases on 2026-10-28. Every external action is
pinned to the resolved commit SHA; comments name the release. The Rust toolchain
action is pinned to a commit in upstream `master` history, as required by its
[pinning guidance](https://github.com/dtolnay/rust-toolchain#choice-of-full-length-commit-sha). The explicit
`toolchain` input selects Rust 1.99.1.

### Active workflows and composite actions

| Action | Version | Commit |
| --- | --- | --- |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) | [v2.9.3](https://github.com/Swatinem/rust-cache/releases/tag/v2.9.3) | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
| [actions/checkout](https://github.com/actions/checkout) | [v7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1) | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| [actions/deploy-pages](https://github.com/actions/deploy-pages) | [v5.0.1](https://github.com/actions/deploy-pages/releases/tag/v5.0.1) | `368f82528645a54fb793d4d04e342629a3f51346` |
| [actions/setup-python](https://github.com/actions/setup-python) | [v7.1.0](https://github.com/actions/setup-python/releases/tag/v7.1.0) | `6d4fd22b20465ccc0f7354476588e8df7c9ca0e0` |
| [actions/upload-pages-artifact](https://github.com/actions/upload-pages-artifact) | [v5.0.0](https://github.com/actions/upload-pages-artifact/releases/tag/v5.0.0) | `fc324d3547104276b827a68afc52ff2a11cc49c9` |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) | [master](https://github.com/dtolnay/rust-toolchain/commit/683b588c487a69d01f6c6ddf40f036ff682cc2c8) | `683b588c487a69d01f6c6ddf40f036ff682cc2c8` |

### Inactive release proposals

The YAML files in `release-proposals/` are outside `.github/workflows` and do not
run. Their shared action references match the table above. The additional action is:

| Action | Version | Commit |
| --- | --- | --- |
| [rust-lang/crates-io-auth-action](https://github.com/rust-lang/crates-io-auth-action) | [v1.0.6](https://github.com/rust-lang/crates-io-auth-action/releases/tag/v1.0.6) | `3268eb413133e2960fa4ae0e963ca76380c54cd9` |

Updating these pins does not activate a proposal or configure registry trust.
The active JavaScript actions use Node.js 24; keep runners compatible with each
upstream action's runner requirements. The checked-in jobs use GitHub-hosted
runners.

## Greenkeeping

[Dependabot configuration](dependabot.yml) checks GitHub Actions weekly and groups their updates. Review the upstream release notes, runtime requirements, permissions, and changes to inputs or artifact behavior before merging. Update SHA pins to the release commit, retaining the version comment where present.

1. Update every reference to the affected action, including local composite actions under `.github/actions/`.
2. Keep frozen dependency installation and the repository’s declared toolchain versions aligned. Cache package downloads with a lockfile-based key; a cache hit does not replace installation or verification.
3. Check workflow YAML and review shell commands. Older workflow linters may not understand native `parallel`; GitHub execution must verify those groups.
4. Run the affected checks and inspect the resulting Actions run. Preserve matrix coverage, build dependencies, artifact paths, and release gates.
5. Refresh this guide when workflows, action references, or parallel groups change.

Use workflow concurrency to cancel superseded check runs where appropriate. Deployment and release cancellation have different consequences: preserve the workflow’s existing policy rather than copying check-run settings blindly. Repository permissions and job-level overrides are defined in the linked YAML files.

Linux CI also checks the Cargo package inventory and runs a locked publish dry run.
This catches missing bundled files and packaged-source compilation failures before
release tagging. It uses the existing read-only permissions, without registry
credentials. The crate publisher remains an inactive proposal outside workflows.
