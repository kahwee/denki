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
