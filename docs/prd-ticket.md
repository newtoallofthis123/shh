# PRD Ticket: Build `shh` v1 macOS CLI

## Summary

Build `shh`, a Rust CLI that stores environment-variable secrets in the macOS Keychain and loads selected profiles into shells or child processes on demand. The v1 product should let a macOS developer remove plaintext API keys from shell profiles and `.env` files while preserving the convenience of env vars.

## Problem

Developers commonly keep secrets in `.zshrc`, `.env`, or other plaintext files. Those secrets leak through backups, dotfile repos, screen shares, shell history, and process inheritance. Existing `.env` workflows also mix real secrets with non-secret local config, making blind import unsafe.

`shh` should make the safer path easy:

- Store secrets at rest in Keychain.
- Activate secrets explicitly per shell or command.
- Import only chosen `.env` entries.
- Avoid printing secret values to a terminal.

## Users

Primary user: a macOS developer who juggles API keys across personal, work, and tool-specific contexts.

Secondary user: a developer running agents, CLIs, scripts, or local apps that need a focused set of credentials for one command.

## Goals

- Provide a single installable Rust binary for macOS.
- Store, retrieve, list, delete, import, export, and run with profile-scoped secrets.
- Use macOS Keychain directly through Security.framework bindings.
- Keep values off disk and avoid accidental TTY disclosure.
- Support an implicit `default` profile and named profile overlays.

## Non-Goals

- Linux, Windows, or cross-platform secret backends.
- Team secret sharing, RBAC, audit logs, or rotation workflows.
- File secrets such as certs, kubeconfigs, and SSH keys.
- GUI, TUI, or long-running daemon.
- Automatic cleanup of already-exported parent shell secrets.

## Assumptions

- v1 targets macOS only.
- Rust is the implementation language.
- `clap` is used for command parsing.
- Nix is used for a reproducible local development shell.
- `just` is used as the task runner for common build, test, lint, and smoke-test commands.
- Use `nixpkgs-unstable` for the flake input.
- The Keychain storage identity is:
  - `service = shh:<profile>`
  - `account = <ENV_NAME>`
  - `password = <secret value>`
- No `-p` flag means `default` only.
- `-p <profile>` means resolve `default` first, then overlay `<profile>`.
- Profile names use a slug format: ASCII letters, numbers, `.`, `_`, and `-`, with no `:` because `:` separates the Keychain service prefix from the profile name.
- `set <NAME> <VALUE>` ships in v1, but stdin and hidden prompt input are also supported.
- `ls -p <profile>` annotates each effective name with its source profile and override status.
- `load --only` and `load --except` accept both comma-separated names and repeated flags.

## Command Scope

Implement these commands:

```text
shh set <NAME> [VALUE] [-p <profile>]
shh get <NAME> [-p <profile>]
shh rm <NAME> [-p <profile>]
shh ls [-p <profile>]
shh profiles
shh load <PATH> [-p <profile>] [--all] [--only <names>...] [--except <names>...] [--dry-run]
shh export [-p <profile>]
shh run [-p <profile>] -- <cmd> [args...]
```

## Functional Requirements

### `set`

- Creates or updates one entry in the target profile.
- Accepts `VALUE` as an optional positional argument.
- If `VALUE` is omitted:
  - If stdin is piped, read the secret from stdin.
  - If attached to a TTY, prompt for a hidden value.
- Validate that `NAME` is a valid env identifier before writing.
- Validate that `profile` is a valid profile slug before writing.
- Never echo the value back to stdout or stderr.
- If a positional value is provided, emit a concise warning that command-line values may be stored in shell history.

### `get`

- Resolves the requested profile using default-plus-overlay rules.
- Prints the secret value only when stdout is not a TTY.
- Refuses TTY output with usage guidance.
- Returns a non-zero exit code if the name is missing.

### `rm`

- Deletes the entry from the target profile only.
- Does not delete matching names from `default` or other profiles.
- Succeeds with a clear message when deletion occurs.
- Returns a non-zero exit code when the entry does not exist.

### `ls`

- Lists entry names for the resolved profile.
- Never prints values.
- With no `-p`, lists names from `default`.
- With `-p <profile>`, shows effective merged names after applying `default` plus the named profile.
- With `-p <profile>`, annotates each name with source information:
  - `default` when the value comes only from `default`
  - `<profile>` when the value comes only from the named profile
  - `<profile> overrides default` when the same name exists in both profiles
- If the same name exists in both profiles, list it once.

### `profiles`

- Lists discovered profile names.
- Includes `default` when default entries exist.
- Discovers profiles by enumerating Keychain items with `service` prefix `shh:`.

### `load`

- Parses a `.env` file.
- Supports v1 dotenv syntax:
  - blank lines
  - comments
  - `NAME=value`
  - `export NAME=value`
  - single-quoted values
  - double-quoted values with common escapes
  - unquoted values trimmed of surrounding whitespace
- Rejects invalid env names and reports them without storing them.
- In TTY mode with no selection flags, shows an interactive checklist of parsed names.
- In non-interactive mode, requires one of `--all`, `--only`, or `--except`.
- `--all` imports every valid parsed entry.
- `--only` imports only the names provided. It accepts comma-separated names and repeated flags.
- `--except` imports every valid parsed entry except the names provided. It accepts comma-separated names and repeated flags.
- `--dry-run` prints what would be added or updated by name only and writes nothing.
- Existing selected names in the target profile are updated.
- Existing profile entries absent from the file are not deleted.
- Values are never printed.

### `export`

- Resolves the requested profile using default-plus-overlay rules.
- Prints shell-safe `export NAME=VALUE` lines only when stdout is not a TTY.
- Refuses TTY output with guidance:

```text
eval "$(shh export -p work)"
```

- Quotes values so the output is safe for POSIX-like shells used on macOS.

### `run`

- Requires `--` before the child command.
- Resolves the requested profile using default-plus-overlay rules.
- Spawns the child command with the current process env inherited and resolved profile vars overlaid.
- Does not mutate the parent shell.
- Returns the child command's exit code.

## Technical Plan

Plan:
1. Create Rust crate structure in `Cargo.toml` and `src/main.rs` - establishes the single-binary CLI entrypoint.
2. Add CLI command model in `src/cli.rs` - centralizes `clap` parsing and keeps command dispatch testable.
3. Add env-name validation and shell quoting in `src/env.rs` - shared by `set`, `load`, `export`, and `run`.
4. Add Keychain adapter in `src/keychain.rs` - isolates macOS Security.framework calls behind a small storage trait.
5. Add profile resolver in `src/profile.rs` - implements `default` plus named-profile overlay behavior.
6. Add dotenv parser/import selector in `src/dotenv.rs` - supports selective `.env` loading without leaking values.
7. Add command handlers in `src/commands/` - keeps each command narrow and independently testable.
8. Add Nix development environment in `flake.nix` - provides Rust, cargo tooling, Security.framework access on macOS, and `just`.
9. Add `justfile` commands - gives contributors a small command surface for setup, build, test, lint, format, run, and Keychain smoke tests.
10. Add integration tests or mocked storage command tests in `tests/` - verifies CLI behavior without depending on a real user's Keychain.

Decisions:
- Ship positional `set <NAME> <VALUE>` plus stdin and hidden prompt input.
- Warn when a positional secret value is provided.
- Annotate `ls -p <profile>` output with source and override status.
- Use slug-style profile names rather than env-var identifier rules.
- Accept repeated `--only` and `--except` flags as well as comma-separated names.
- Use `nixpkgs-unstable` for the Nix flake input.
- Use the Rust toolchain from `nixpkgs-unstable` directly unless a future reproducibility issue requires an overlay.
- Use `dialoguer` for v1 interactive prompts unless a small implementation spike exposes a blocker.

Open questions:
- None for the current PRD pass.

Risk:
- Keychain ACL behavior may prompt more often than expected, especially after binary updates or unsigned local builds.
- Keychain profile enumeration may be slower or less filterable than desired.
- Shell quoting mistakes could break `eval "$(shh export)"` or expose malformed values.
- Dotenv parsing can sprawl; v1 should intentionally avoid interpolation and multiline values unless required.
- `run` still inherits unrelated secrets already present in the parent environment.

## Suggested Internal Interfaces

```rust
trait SecretStore {
    fn set(&self, profile: &str, name: &str, value: &str) -> Result<()>;
    fn get(&self, profile: &str, name: &str) -> Result<Option<String>>;
    fn delete(&self, profile: &str, name: &str) -> Result<bool>;
    fn list_names(&self, profile: &str) -> Result<Vec<String>>;
    fn list_profiles(&self) -> Result<Vec<String>>;
}
```

```rust
struct ProfileEnv {
    vars: BTreeMap<String, String>,
}

fn resolve_profile(store: &dyn SecretStore, profile: Option<&str>) -> Result<ProfileEnv>;
```

Keep command handlers dependent on `SecretStore` rather than directly on Keychain so behavior can be tested with an in-memory store.

## Developer Tooling

### Nix

Add a `flake.nix` that provides a macOS-focused development shell with:

- Rust toolchain.
- `cargo`, `rustfmt`, and `clippy`.
- `just`.
- `pkg-config` or other build helpers only if needed by the selected crates.
- Darwin framework availability for Security.framework integration.

The Nix shell should optimize for contributor setup, not cross-platform packaging. v1 remains macOS-only, and Linux support in the dev shell is not required.

Expected usage:

```text
nix develop
```

### Justfile

Add a `justfile` that wraps the commands a developer will actually run:

```text
just setup
just fmt
just lint
just test
just build
just run -- <args>
just smoke-keychain
just ci
```

Expected command behavior:

- `setup` verifies required local tools are available.
- `fmt` runs Rust formatting.
- `lint` runs clippy with warnings treated as errors.
- `test` runs the test suite.
- `build` creates a debug binary.
- `run -- <args>` runs the CLI through cargo.
- `smoke-keychain` performs a local macOS-only set/get/list/delete test against a clearly test-scoped profile.
- `ci` runs formatting, linting, and tests.

The smoke test must use a reserved profile such as `__shh_smoke__` and clean up after itself.

## Prompt Dependency Research

`shh` needs two interactive primitives in v1: hidden secret input for `set NAME` and a checklist for `load`.

Good current Rust options:

- `dialoguer` - mature and focused. It directly supports password input, input validation, single and multi-select prompts, and fuzzy select. This is the lowest-risk v1 choice because it matches the required surface without pulling the project toward a richer TUI.
- `inquire` - feature-rich and modern. It supports text, password, select, multi-select, validators, autocomplete, and richer prompt customization. Good fit if `.env` selection needs filtering, help text, or more polished validation behavior.
- `cliclack` - modern Clack-style UX. It supports password, select, multi-select, filter mode, progress bars, logging helpers, and themes. Good fit if the CLI should feel more designed, but it is more opinionated than `dialoguer`.
- `requestty` - Inquirer.js-style prompts. It supports password and multi-select builders, but it is a heavier abstraction than v1 needs unless the command flow becomes more questionnaire-like.

Recommendation: use `dialoguer` for v1. It covers hidden input and checklist import cleanly, keeps implementation small, and leaves room to switch to `inquire` or `cliclack` later if the interactive flow becomes a product differentiator.

## Acceptance Criteria

- `shh set OPENAI_API_KEY` stores a hidden prompted value in the default profile.
- `printf '%s' "$VALUE" | shh set OPENAI_API_KEY` stores a piped value without echoing it.
- `shh get OPENAI_API_KEY` refuses to print to an interactive terminal.
- `shh get OPENAI_API_KEY | pbcopy` prints the value to the pipe.
- `eval "$(shh export -p work)"` exports default values plus work overrides.
- `shh run -p work -- env` includes resolved profile variables in the child process.
- `shh load .env -p work` lets an interactive user select names before import.
- `shh load .env -p work --only OPENAI_API_KEY --only ANTHROPIC_API_KEY --dry-run` reports add/update names and writes nothing.
- `shh load .env -p work --except PORT,DEBUG --dry-run` excludes both names and writes nothing.
- `shh ls -p work` lists effective names without values and annotates default/profile/override source.
- `shh profiles` lists profiles discovered from Keychain services.
- `nix develop` opens a shell with the Rust toolchain and `just` available.
- `just ci` runs format checks, lint checks, and tests.
- `just smoke-keychain` verifies the real Keychain adapter on macOS without leaving test entries behind.
- Unit tests cover env-name validation, shell quoting, profile overlay behavior, dotenv parsing, and load selection behavior.
- Command tests cover TTY refusal paths for `get` and `export`.

## Implementation Milestones

1. CLI skeleton and in-memory storage
   - Verify: command parser tests pass and handlers work against a mock store.

2. Validation, quoting, and profile overlay
   - Verify: unit tests cover invalid names, override collisions, and shell-special values.

3. Keychain storage adapter
   - Verify: local macOS smoke test can set, get, list, and delete one entry.

4. Safe output commands
   - Verify: `get` and `export` refuse TTY output and work through pipes.

5. `.env` import flow
   - Verify: parser tests pass, `--all`, `--only`, `--except`, and `--dry-run` behave as specified.

6. `run` command
   - Verify: child process receives overlaid env and returns the child exit code.

7. Packaging polish
   - Verify: release build produces one binary and README quickstart matches actual behavior.

8. Nix and Justfile developer workflow
   - Verify: `nix develop`, `just ci`, and `just smoke-keychain` work on macOS.

## Future Follow-Ups

- `run --clean` to avoid inheriting the parent environment.
- `unset` helper output for deactivating loaded secrets in the current shell.
- Optional shell completions.
- Broader dotenv syntax if real `.env` files require it.
