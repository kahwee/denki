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
  `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo test --locked`. Use `cargo update --dry-run` to inspect compatible
  lockfile updates; keep `rust-version` and the lockfile compatible.
- Do not run power-changing commands against real devices as routine tests.
  `denki group ... --dry-run` previews matching targets without contacting them.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full local workflow and
[README.md](README.md) for user commands and support status.
