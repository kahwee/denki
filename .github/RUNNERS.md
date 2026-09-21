# CI runners

Main pushes and manual main runs use `stash-denki-ci` for Linux tests, audit and
release builds. PRs use GitHub-hosted Linux; macOS always uses GitHub-hosted runners.
The jobs retain their existing names and checks, with locked Cargo dependencies.
Obsolete runs are cancelled when a newer commit arrives.

The dedicated container keeps Cargo toolchains, downloaded crates, the pinned audit
binary and compiled artifacts between jobs. `CARGO_TARGET_DIR` is outside checkout
cleanup. Cargo validates fingerprints and all tests still execute; audit advisories
refresh on every run. Linux jobs are serialized by the single repository runner.

Infrastructure and maintenance live in
[Thermark's runner directory](https://github.com/kahwee/thermark/tree/main/infra/github-runner).
The runner rejects non-main and PR events before checkout. All external fork
contributors require workflow approval; never approve a PR that targets the NAS.

Run `gh workflow run ci.yml --ref main` and compare step timings with a rerun of the
same commit. First runs include compilation; warm runs reuse unchanged artifacts.

## Verified timings

[Run 35550739631](https://github.com/kahwee/denki/actions/runs/35550739631) passed
all jobs on commit `76fd756`, both initially and on rerun (2026-09-20 Pacific).

| Job | Initial | Warm rerun |
| --- | ---: | ---: |
| Linux tests | 176s | 21s |
| Linux release build | 76s | 9s |
| Dependency audit | 640s | 11s |
| Hosted macOS release build | 58s | 21s |

Times include job setup and cleanup, but not queueing. The initial audit job spent
615 seconds installing the pinned tool. Warm Linux jobs completed serially within
44 seconds. These are single-run observations on a shared NAS, not medians or a
controlled provider comparison. All test, audit and build commands still execute.
