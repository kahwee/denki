# denki agent guide

denki controls Kasa and Tapo devices on the local network. `devices.toml` is
the support registry: do not label a path hardware-verified based on mocks or
compilation. Avoid changing live network behavior without device evidence.
Never put real device addresses, credentials, or diagnostic captures in Git.

- Preserve unrelated working-tree changes. Keep protocol logic with its device
  module, device-kind detection and capability guards in `src/devices/`, CLI
  arguments in `src/cli.rs`, handlers in `src/commands/` or `src/admin/`, and
  dispatch in `src/app/`.
- Update the README and CLI help when command names, support, or output change.
  Prefer offline parser, capability, and fixture tests; use live hardware only
  for an explicit device question.
- For Rust changes or dependency updates run `cargo fmt --check`,
  `cargo clippy --locked --all-targets --all-features -- -D warnings`, and
  `cargo test --locked`. Use `cargo update --dry-run` to inspect compatible
  lockfile updates; keep `rust-version` and the lockfile compatible.
- Do not run power-changing commands against real devices as routine tests.
  `denki group ... --dry-run` previews matching targets without contacting them.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full local workflow and
[README.md](README.md) for user commands and support status.

For release preparation, follow [the release skill](.agents/skills/denki-release/SKILL.md).
Keep manifest, root lockfile and tag versions aligned; require exact-source CI,
including Linux/macOS builds, audit and package preflight. Keep action pins and
comments aligned with [.github/ACTIONS.md](.github/ACTIONS.md). For docs, run
`python3 scripts/update-device-docs.py --check`, build/check the site, and keep
README, getting-started and site installation examples consistent.

GitHub releases, crates.io and docs.rs have separate verification. New user-pushed
`v*` tags start the registry publisher after metadata, exact-source CI, package
and audit checks. Its OIDC trust binding is external; ordinary main pushes do
not publish. See [publishing](docs/actions-publishing.md) before tagging.
Never move existing tags or duplicate an in-progress release/upload. Do not
claim registry availability until the version is visible in the official index.
