set shell := ["bash", "-cu"]
set positional-arguments := true

default: ci

setup:
    @command -v cargo >/dev/null || { echo "cargo not on PATH — run 'nix develop' first"; exit 1; }
    @command -v just  >/dev/null
    @echo "ok — rustc $(rustc --version | awk '{print $2}'), just $(just --version | awk '{print $2}')"

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test --all

build:
    cargo build

run *ARGS:
    cargo run -- "$@"

install:
    cargo install --path .

install-signed:
    @if [ -z "${SHH_SIGN_IDENTITY:-}" ]; then echo "SHH_SIGN_IDENTITY not set (e.g. 'Developer ID Application: Your Name (TEAMID)')"; exit 1; fi
    cargo build --release
    codesign --force --timestamp --options runtime --sign "$SHH_SIGN_IDENTITY" target/release/shh
    install -m 0755 target/release/shh "$HOME/.cargo/bin/shh"
    @echo "installed signed shh to $HOME/.cargo/bin/shh"

smoke-keychain:
    SHH_SERVICE_PREFIX=shh-smoke cargo run -- doctor keychain

ci: fmt-check lint test
