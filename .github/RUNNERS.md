# CI runners

The [CI workflow](workflows/ci.yml) uses GitHub-hosted runners for every event:

| Job | Runner | Checks |
| --- | --- | --- |
| Test | `ubuntu-latest` | Generated support docs, locked tests (including library examples), Clippy, formatting, rustdoc |
| Dependency Audit | `ubuntu-latest` | `cargo audit` with refreshed advisories |
| Build | `ubuntu-latest`, `macos-latest` | Locked release build |

Development and CI use only Rust 1.99.0, configured by `rust-toolchain.toml` and
[setup-rust](actions/setup-rust/action.yml). The minimum supported version is 1.99.
Cargo caches may speed up compilation; checks still execute on every applicable run.
There is no self-hosted or NAS runner requirement.

Pushes to main, pull requests targeting main, and manual runs execute all jobs.
Every Monday at 17:23 UTC, a scheduled run executes only the dependency audit.
GitHub may delay scheduled runs. Concurrency groups include the event type, so a
scheduled audit cannot cancel a full push/PR/manual run. Newer runs of the same
event and ref cancel older ones.

The workflow has read-only repository permissions. Action dependencies are pinned
to commit SHAs; Dependabot opens weekly updates for actions and Cargo dependencies.
See [dependency maintenance](../CONTRIBUTING.md#dependency-maintenance) for grouping
and review policy.

To run all checks manually:

```sh
gh workflow run ci.yml --ref main
```

Use the Actions job logs to inspect timings. Cache misses and runner availability
make historical timings unsuitable as performance guarantees.
