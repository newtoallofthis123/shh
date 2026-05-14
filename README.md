# shh

A macOS CLI that keeps env-var secrets in the Keychain and loads them into shells or child processes on demand. See `docs/prd-ticket.md` for the v1 spec.

## Quickstart

```sh
nix develop          # rustc, cargo, just, Security.framework
just setup           # sanity check
just ci              # fmt-check + lint + test
just install         # cargo install --path .  (unsigned / ad-hoc)
```

After install:

```sh
shh set OPENAI_API_KEY                  # hidden prompt
eval "$(shh export -p work)"            # load profile into shell
shh run -p work --clean -- env          # run a child with only profile vars + safe baseline
```

See `just --list` for the full contributor surface.
