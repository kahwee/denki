"""Validate Cargo's actual package inventory; no credentials or uploads."""
import subprocess
import sys

REQUIRED = {
    'Cargo.lock',
    'Cargo.toml',
    'Cargo.toml.orig',
    'LICENSE',
    'README.md',
    'devices.toml',
    'docs/library.md',
    'src/lib.rs',
    'src/main.rs',
    'tests/fixtures/hs300_hw2_sysinfo.json',
}
ALLOWED_FILES = REQUIRED | {'devices.toml', 'rustfmt.toml', 'docs/library.md', 'rust-toolchain.toml', 'CONTRIBUTING.md'} | {".cargo_vcs_info.json"}
ALLOWED_DIRS = ('src/', 'tests/')


def validate(files):
    files = set(files)
    missing = REQUIRED - files
    unexpected = {p for p in files if p not in ALLOWED_FILES and not p.startswith(ALLOWED_DIRS)}
    # Cargo's source allowlist must never admit personal output or runner files.
    unexpected |= {p for p in files if p.startswith(("local/", "infra/", ".github/", ".agents/", "target/")) and p != "local/README.md"}
    if missing or unexpected:
        raise ValueError(f"Missing package files: {sorted(missing)}; unexpected package files: {sorted(unexpected)}")


if __name__ == "__main__":
    result = subprocess.run(["cargo", "package", "--locked", "--list"], capture_output=True, text=True)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    validate(result.stdout.splitlines())
