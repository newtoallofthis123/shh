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
    @identity=$(security find-identity -v -p codesigning | awk -F'"' '/Developer ID Application|Apple Development|shh-codesign/ {print $2; exit}'); \
    if [ -z "$identity" ]; then \
        echo "error: no code-signing identity found in your login keychain."; \
        echo "       run 'just setup-codesign' to create a self-signed one,"; \
        echo "       or set up an Apple Development / Developer ID identity in Xcode."; \
        exit 1; \
    fi; \
    echo "signing with: $identity"; \
    cargo build --release && \
    if codesign -dv target/release/shh 2>&1 | grep -qF "Authority=$identity"; then \
        echo "binary already signed with this identity, skipping codesign"; \
    else \
        codesign --force --sign "$identity" --identifier "io.invideo.shh" target/release/shh; \
    fi && \
    install -m 0755 target/release/shh "$HOME/.cargo/bin/shh" && \
    echo "installed signed shh to $HOME/.cargo/bin/shh"

setup-codesign:
    @if security find-identity -v -p codesigning | grep -q '"shh-codesign"'; then \
        echo "shh-codesign identity already exists in login keychain — nothing to do."; \
        exit 0; \
    fi; \
    if security find-identity -v -p codesigning | awk -F'"' '/Developer ID Application|Apple Development/ {found=1} END {exit !found}'; then \
        echo "you already have an Apple-issued code-signing identity:"; \
        security find-identity -v -p codesigning | grep -E "Developer ID Application|Apple Development"; \
        echo "'just install' will pick it up automatically — no setup needed."; \
        exit 0; \
    fi; \
    workdir=$(mktemp -d); \
    trap "rm -rf $workdir" EXIT; \
    echo "generating self-signed code-signing cert 'shh-codesign'..."; \
    openssl req -newkey rsa:2048 -nodes -keyout "$workdir/k.key" -x509 -days 3650 \
        -out "$workdir/c.crt" -subj "/CN=shh-codesign" \
        -addext "extendedKeyUsage=codeSigning" >/dev/null 2>&1; \
    openssl pkcs12 -export -inkey "$workdir/k.key" -in "$workdir/c.crt" \
        -out "$workdir/c.p12" -passout pass: -name shh-codesign >/dev/null 2>&1; \
    echo "importing into login keychain (you'll be prompted for your login password)..."; \
    security import "$workdir/c.p12" -k "$HOME/Library/Keychains/login.keychain-db" \
        -T /usr/bin/codesign -P "" >/dev/null; \
    security add-trusted-cert -r trustAsRoot \
        -k "$HOME/Library/Keychains/login.keychain-db" "$workdir/c.crt" >/dev/null; \
    echo "done. run 'just install' next."

smoke-keychain:
    SHH_SERVICE_PREFIX=shh-smoke cargo run -- doctor keychain

ci: fmt-check lint test
