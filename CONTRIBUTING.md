# Contributing

Thanks for helping improve `shh`. This project is intentionally small: a macOS
CLI that keeps environment-variable secrets in the login Keychain and loads
them only when needed.

## Before you start

- For bugs and one-file fixes, a pull request is fine.
- For larger behavior changes, open an issue first so the design can be
  discussed before code is written.
- Keep changes narrow. Avoid refactors, formatting churn, or new abstractions
  unless they are needed for the issue you are solving.
- Do not include real secrets, API keys, `.env` files, Keychain exports, or
  screenshots that reveal secret values.

## Development setup

Use either your local Rust toolchain or the Nix shell:

```sh
nix develop
```

Then check the local tools:

```sh
just setup
```

If you do not use `just`, the important commands are:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

## Local workflow

Run the full check before sending a pull request:

```sh
just ci
```

Use the Keychain smoke test only when you need to verify the real macOS
Keychain backend:

```sh
just smoke-keychain
```

The normal test suite uses an in-memory store and should not touch your real
Keychain.

## Pull requests

Please include:

- What changed and why.
- How you tested it.
- Any user-facing behavior changes.
- Any follow-up work that is deliberately left out.

PRs should keep public CLI behavior, profile resolution, and Keychain access
semantics explicit. If a change affects secret handling, document the security
impact in the PR description.
