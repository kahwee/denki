# denki repository guidance

Follow [AGENTS.md](AGENTS.md) for code boundaries, hardware evidence, and checks.

- [Commands and device support](docs/commands.md)
- [Architecture, protocol framing, and module ownership](docs/architecture.md)
- [Rust library usage](docs/library.md)
- [Development and optional live smoke harness](CONTRIBUTING.md)

Use the locked Cargo graph and retain the minimum Rust version in Cargo.toml.
Tests, mocks, and compilation do not establish hardware support. Live smoke
tests change device power and require an explicit hardware-testing task.
