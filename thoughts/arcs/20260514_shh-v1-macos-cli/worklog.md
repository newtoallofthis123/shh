---
type: enact-worklog
created: 2026-05-14
status: enacting
arc: thoughts/arcs/20260514_shh-v1-macos-cli/
pr: not yet
---

# shh v1 — macOS Keychain-backed env-var secrets CLI

## Phase log

### 2026-05-14 — Phase 0: read arc

- Prologue + 6 chapters read. All chapters declare a Type.
- Wave 0 (F) confirmed bootstrapped on disk: `flake.nix`, `justfile`, `README.md`, `.gitignore` present.
- Prologue Status was `draft`; flipped to `enacting` after confirming F's bootstrap and the locked answers section. Surfaced to user.
- Dispatch plan:
  - Wave 1: A (strict, no deps).
  - Wave 2 (parallel): B, C, D — all depend on A.
  - Wave 3: E — depends on A, B, C, D.

### 2026-05-14 — Phase 1: dispatching wave 1

- Dispatching chapter A (strict-executor).

### 2026-05-14 — Wave 1 outcome

#### Chapter A — Foundation [complete]
- Files: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/main.rs`, `src/lib.rs`, `src/cli.rs`, `src/env.rs`, `src/error.rs`, `src/store/mod.rs`, `src/store/memory.rs`.
- Verification: `RUSTFLAGS='-D warnings' cargo build` clean; `cargo test` 9 passed; `cargo run -- --help` lists all PRD subcommands.
- Commit: 515fa92

### 2026-05-14 — Phase 1: dispatching wave 2

- Dispatching B (strict), C (conscious), D (conscious) in parallel — all depend on A which is committed.

### 2026-05-14 — Wave 2 outcomes (combined `cargo build` + `cargo test` clean: 70 passed)

#### Chapter B — Core utilities [complete]
- Files: `src/env.rs` (extend; +`posix_quote` + 9 tests), `src/profile.rs` (new; resolver + 13 tests), `src/lib.rs` (`+pub mod profile;`).
- Commit: 897c6cd

#### Chapter C — Dotenv parser & selector [complete]
- Files: `src/dotenv.rs` (new; parser + Selector + `SecretValue` newtype + 35 tests), `src/lib.rs` (`+pub mod dotenv;`).
- Commit: 8d2397c
- Judgment calls flagged by executor:
  - Entries typed as `Vec<(String, SecretValue)>` end-to-end (chapter showed `String` in C.1 but C.3 mandated SecretValue) — resolved in favor of C.3.
  - Unsupported escapes inside `"..."` are errors, not literal passthrough — matches "safer behavior" guidance.
  - Allows raw newlines inside `"..."` in addition to backslash-newline continuation — flagged for review.
  - Selector returns its own `SelectionError` rather than `anyhow` to keep the module dependency-light.

#### Chapter D — Keychain adapter [complete]
- Files: `src/store/keychain.rs` (new), `src/store/mod.rs` (macOS-gated `pub mod keychain` + re-export), `Cargo.toml` (enable `security-framework` `OSX_10_15` feature so `use_protected_keychain` compiles).
- Commit: 5395280
- Judgment calls flagged by executor:
  - Added `OSX_10_15` feature on security-framework — required for data-protection mode to compile.
  - errSec constants inlined as `i32` (-25300/-128/-25293) to avoid pulling `security-framework-sys`.
  - Used `simplify_dict()` for attribute enumeration (no typed accessor on `SearchResult::Dict`).

### 2026-05-14 — Phase 1: dispatching wave 3

- Dispatching E (conscious). All deps (A/B/C/D) committed.

### 2026-05-14 — Wave 3 outcome

#### Chapter E — Command handlers & wiring [complete]
- Files: `src/tty.rs` (new), `src/commands/{mod,set,get,rm,ls,profiles,load,export,unset,run,completions,doctor}.rs` (new), `src/lib.rs` (extend), `src/main.rs` (real dispatch via `KeychainStore::from_env()`), `tests/cli.rs` (new — 18 integration tests against `MemoryStore`).
- Verification: `RUSTFLAGS='-D warnings' cargo build` clean; `cargo test` 70 unit + 18 integration passing; `cargo run -- --help` lists all 11 subcommands.
- Commit: 2503a18
- Judgment calls flagged by executor:
  - `SecretValue` exposes value via `as_str()`; used that.
  - inquire prompt failures map to `ShhError::Keychain("prompt error: …")` (no dedicated variant in A's enum).
  - `get`/`export`/`unset` use exit code 2 for TTY-guard refusal vs 1 for not-found (PRD silent on the split).
  - `load` interactive selector path not exercised by integration tests (no PTY); flag/dry-run paths covered.
  - `doctor` reports `permission flow: not prompted` on success; cannot distinguish `granted | denied | cancelled` without OS-level introspection beyond `KeychainDenied`.

### 2026-05-14 — Phase 2: Review

- Diff vs pre-arc: 27 files, +3023 lines. Every changed file falls inside its declared chapter file set; no out-of-bounds drift.
- `RUSTFLAGS='-D warnings' cargo build` clean.
- `cargo test` 70 unit + 18 integration passing.
- `just ci` initially failed on `fmt-check` (a few `rustfmt` re-flows in chapters C/D/E). Fixed with `cargo fmt --all`. Commit dcea5f2.
- Clippy + tests pass under `just ci` after fmt.
- Re-flagged for user testing:
  - `doctor keychain` — exercises the real Keychain (only verifiable by hand on the user's macOS).
  - `set` interactive hidden prompt and `load`'s filterable MultiSelect — not exercised by automated tests (no PTY).
  - `just install` / `just install-signed` — F's recipes; PRD calls install-signed "ships but not tested".
  - Selector behaviors flagged in C (raw newlines inside `"..."`, custom `SelectionError`) and D (Cargo.toml feature add, errSec inlining) — confirm acceptable.
  - Exit-code split for TTY-guard refusal (2 vs 1) chosen by E — confirm.

Handing off to user for firefight.
