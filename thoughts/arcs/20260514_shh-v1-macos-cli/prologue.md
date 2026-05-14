# shh v1 — macOS Keychain-backed env-var secrets CLI

**Date:** 2026-05-14
**Status:** draft
**Worklog:** worklog.md

## Context

Greenfield Rust crate at `/Users/noob/Projects/shh`. Only `docs/` exists today. The PRD (`docs/prd-ticket.md`) is exhaustive: it locks command surface, dotenv subset, profile overlay rules (`default` + named overlay), Keychain item identity (`service = shh:<profile>`, `account = <ENV_NAME>`), runtime knobs (`SHH_KEYCHAIN`, `SHH_SERVICE_PREFIX`), interactive UX (hidden prompt + filterable multi-select), TTY guards, and acceptance criteria. v1 targets macOS only via Security.framework; the in-memory store is `#[cfg(test)]`-only and not reachable from the shipped binary. Default install path is unsigned/ad-hoc via `just install`; `just install-signed` ships but is not tested.

## Locked answers from preflight

- Q: Keychain crate vs raw FFI? → A: Use the `security-framework` crate (mature wrapper over Security.framework). Use its data-protection keychain APIs for the `data-protection` mode and legacy generic-password APIs for the `file` mode.
- Q: Prompt crate? → A: `inquire` — filterable multi-select for `load`, `Password` for `set` hidden input (locked by PRD).
- Q: Dotenv parser source? → A: Write our own in `src/dotenv.rs` — PRD specifies a precise subset that excludes `$(...)`, backticks, and `${VAR:-default}`. No off-the-shelf crate matches exactly.
- Q: Error strategy? → A: `thiserror` for the library/store layer; handlers return `anyhow::Result` so command code stays terse.
- Q: Shell quoting? → A: Implement POSIX single-quote escaping in `src/env.rs` (`'` → `'\''`). Don't pull `shell-escape` for one function.
- Q: `clap` style? → A: derive API + `clap_complete` for `completions`. One enum of subcommands dispatched from `main.rs`.
- Q: In-memory store reachability? → A: `#[cfg(test)]`-gated; never compiled into release. No runtime backend toggle.
- Q: Service-prefix env var? → A: Honored only by the real Keychain adapter at construction. Profile parsing (`profiles` command) filters by the active prefix.
- Q: `run --clean` baseline list? → A: Exactly `PATH, HOME, USER, SHELL, TERM, LANG, LC_*, TMPDIR` (per PRD).
- Q: Reserved profiles for tests/doctor? → A: `__shh_smoke__` (justfile smoke) and `__shh_doctor__` (`doctor keychain`). Adapters must allow these names through profile slug validation.

## Chapters

- **F — Dev tooling** [conscious] — `flake.nix`, `justfile`, install/install-signed/smoke-keychain recipes, `.gitignore` updates. **Bootstrapped before the arc is enacted** — every other chapter assumes `nix develop` provides Rust + `just`.
- **A — Foundation & storage trait** [strict] — Cargo crate, clap subcommand enum, `SecretStore` trait, `#[cfg(test)]` in-memory store, error types, main dispatcher skeleton.
- **B — Core utilities** [strict] — env-name validation, profile slug validation, POSIX shell quoting, profile resolver (`default` + overlay).
- **C — Dotenv parser & selector** [conscious] — PRD-spec dotenv subset, validation errors, `--all`/`--only`/`--except` selection.
- **D — Keychain adapter** [conscious] — `security-framework`-backed `SecretStore`, file vs data-protection modes, `SHH_SERVICE_PREFIX` namespacing, attribute-only enumeration for `ls`/`profiles`.
- **E — Command handlers & wiring** [conscious] — every command implemented and wired into the dispatcher, TTY guards, hidden prompts, interactive load checklist, child-process `run`, completions, doctor.

## Parallelism plan

- Wave 0 (already done): **F** — repo has working `nix develop` + `just` surface.
- Wave 1: **A** — depends on F's toolchain.
- Wave 2 (parallel, all depend on A): **B**, **C**, **D**.
- Wave 3: **E** (depends on A, B, C, D).
