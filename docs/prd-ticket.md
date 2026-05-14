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
- Make Keychain permission behavior understandable with clear first-use messaging and diagnostics.

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
- v1 uses normal macOS Keychain permission behavior first. Do not require explicit ACL customization for the initial implementation.
- v1 is a personal tool. Default install (`just install` → `cargo install --path .`) produces an unsigned / ad-hoc-signed binary; the user accepts that macOS will re-prompt for Keychain access after rebuilds and reinstalls. **This is the only install path covered by v1 testing and acceptance criteria.**
- `just install-signed` is provided for users with a Developer ID who want stable Keychain trust across rebuilds, and can be paired with `SHH_KEYCHAIN=data-protection` to use the modern keychain. Shipped as a convenience; not exercised by v1 tests.
- Development uses a dev-only Keychain service prefix (`SHH_SERVICE_PREFIX=shh-dev`) to avoid touching real user secrets. An in-memory backend exists only behind `#[cfg(test)]` for unit tests and is not reachable from the shipped binary.

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
shh unset [-p <profile>]
shh run [-p <profile>] [--clean] -- <cmd> [args...]
shh doctor keychain
shh completions <shell>
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
- Discovers profiles by enumerating `kSecClassGenericPassword` items with `kSecReturnAttributes = true`, `kSecReturnData = false`, `kSecMatchLimit = kSecMatchLimitAll`, then filtering client-side for `service` strings prefixed with `shh:` and extracting the suffix. `SecItemCopyMatching` does not support prefix queries on `kSecAttrService`, so the broad-fetch + filter is required.
- Attribute-only enumeration does not trigger Keychain permission prompts and is not filtered by per-item ACLs for items the calling binary itself wrote. `profiles` and `ls` therefore run silently. Permission prompts are scoped to commands that read secret values (`get`, `export`, `run`).

### `load`

- Parses a `.env` file.
- Supports v1 dotenv syntax:
  - blank lines
  - comments (full-line `#` and trailing `#` outside quotes)
  - `NAME=value`
  - `export NAME=value`
  - single-quoted values (literal, no escapes, no interpolation)
  - double-quoted values with common escapes (`\n`, `\t`, `\r`, `\\`, `\"`) and `${VAR}` interpolation against already-parsed entries in the same file
  - unquoted values trimmed of surrounding whitespace, with `${VAR}` interpolation
  - backslash-newline line continuation inside double-quoted values
- Explicitly out of scope for v1: command substitution (`$(...)`), backtick substitution, default-value expansion (`${VAR:-default}`).
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
- With `--clean`, spawns the child with only resolved profile vars plus a minimal safe baseline (`PATH`, `HOME`, `USER`, `SHELL`, `TERM`, `LANG`, `LC_*`, `TMPDIR`) — does not inherit other parent env.
- Does not mutate the parent shell.
- Returns the child command's exit code.

### `unset`

- Resolves the requested profile using default-plus-overlay rules.
- Prints shell-safe `unset NAME` lines for every resolved name only when stdout is not a TTY.
- Refuses TTY output with guidance:

```text
eval "$(shh unset -p work)"
```

- Intended as the inverse of `shh export` for deactivating a previously loaded profile in the current shell.

### `completions`

- Prints shell completion script for `bash`, `zsh`, or `fish` to stdout.
- Generated via `clap_complete`.

### `doctor keychain`

- Performs a local macOS-only Keychain smoke test.
- Reports which Keychain store is active (file-based vs data-protection) per `SHH_KEYCHAIN`.
- Reports the signing identity of the running binary (unsigned, ad-hoc, Developer ID) and whether it is suitable for the active store.
- Writes, reads, lists, and deletes a test-scoped entry against the active store.
- Uses a reserved profile such as `__shh_doctor__`.
- Never touches non-test profiles.
- Reports whether Keychain access works and whether macOS permission was denied or cancelled.
- Explains that macOS may prompt for access and that choosing "Always Allow" permits the current signed `shh` binary to read its own items without repeated prompts.

## Technical Plan

Plan:
1. Create Rust crate structure in `Cargo.toml` and `src/main.rs` - establishes the single-binary CLI entrypoint.
2. Add CLI command model in `src/cli.rs` - centralizes `clap` parsing and keeps command dispatch testable.
3. Add env-name validation and shell quoting in `src/env.rs` - shared by `set`, `load`, `export`, and `run`.
4. Add storage adapters in `src/store/` - isolates the real macOS Keychain adapter and the in-memory dev/test adapter behind a small storage trait.
5. Add profile resolver in `src/profile.rs` - implements `default` plus named-profile overlay behavior.
6. Add dotenv parser/import selector in `src/dotenv.rs` - supports selective `.env` loading without leaking values.
7. Add command handlers in `src/commands/` - keeps each command narrow and independently testable.
8. Add Keychain diagnostics in `doctor keychain` - gives users and development builds a safe way to verify real Keychain behavior.
9. Add Nix development environment in `flake.nix` - provides Rust, cargo tooling, Security.framework access on macOS, and `just`.
10. Add `justfile` commands - gives contributors a small command surface for setup, build, test, lint, format, run, and Keychain smoke tests.
11. Add integration tests or mocked storage command tests in `tests/` - verifies CLI behavior without depending on a real user's Keychain.

Decisions:
- Ship positional `set <NAME> <VALUE>` plus stdin and hidden prompt input.
- Warn when a positional secret value is provided.
- Annotate `ls -p <profile>` output with source and override status.
- Use slug-style profile names rather than env-var identifier rules.
- Accept repeated `--only` and `--except` flags as well as comma-separated names.
- Use `nixpkgs-unstable` for the Nix flake input.
- Use the Rust toolchain from `nixpkgs-unstable` directly unless a future reproducibility issue requires an overlay.
- Use `inquire` for v1 interactive prompts (hidden password input and filterable multi-select for `load`).
- Use standard Keychain access first. Explicit ACL customization is not a v1 requirement.
- Ship `run --clean`, `unset`, and shell completions in v1.
- Do not ship an env-var-toggleable storage backend in the release binary. The in-memory store exists only for tests.
- Support a development-only service prefix override with `SHH_SERVICE_PREFIX=shh-dev`.
- Add `doctor keychain` as the user-facing way to validate Keychain access and explain macOS prompts.

Open questions:
- None for the current PRD pass.

Risk:
- macOS will prompt on every secret-value read (`get`, `export`, `run`) after each binary rebuild or reinstall, because v1 is unsigned / ad-hoc signed and "Always Allow" trust does not survive an identity change. Accepted as the personal-tool tradeoff.
- macOS 26.4 introduced stricter authorization checks (CVE-2026-28864) that can return `errSecAuthFailed` from `SecItemCopyMatching` against the legacy file-based login keychain under some conditions. Default v1 backend is the file-based keychain (works unsigned); users with a Developer ID signed build can opt into the data-protection backend via `SHH_KEYCHAIN=data-protection` to sidestep this.
- Shell quoting mistakes could break `eval "$(shh export)"` or `eval "$(shh unset)"` or expose malformed values.
- Dotenv parsing must stop at the syntax declared in `load` — no command substitution, no `${VAR:-default}`.
- `run` without `--clean` inherits unrelated secrets already present in the parent environment. The user should prefer `--clean` when isolation matters.
- Development builds can accidentally touch real Keychain entries unless tests use the `#[cfg(test)]` in-memory store and smoke tests use reserved profiles or `SHH_SERVICE_PREFIX`.

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

## Keychain and ACL Policy

`shh` stores secrets as macOS generic password items through public Security.framework APIs. The intended item identity is:

```text
service: shh:<profile>
account: <ENV_NAME>
value: <secret>
```

For v1, implement normal Keychain storage and retrieval before attempting explicit ACL customization. macOS already mediates access to Keychain items. When `shh` reads an item, the system may prompt the user to allow access. If the user chooses "Always Allow", macOS can trust the current `shh` binary for future reads of that item.

Do not make explicit ACL mutation a hard v1 dependency. The older macOS ACL APIs are finicky around signing identity, iCloud/data-protection Keychain behavior, and binary updates. The v1 success path is:

- Use Security.framework item APIs for add, update, lookup, list, and delete.
- Let macOS handle first-use permission prompts.
- Sign release builds so the trusted application identity is stable.
- Add diagnostics and clear messaging when access is denied, cancelled, or blocked.

### Safety Guarantees

- `shh` must not modify system Keychain settings.
- `shh` must not change global macOS security settings.
- `shh` must not install a daemon, login item, kernel extension, or background agent.
- Deletes must be scoped to `service = shh:<profile>` and a specific account name, except reserved smoke-test cleanup.
- Test and diagnostic profiles must use reserved names such as `__shh_smoke__` and `__shh_doctor__`.
- User-facing output must never include secret values unless the command is explicitly designed to write to a non-TTY pipe.

### Development Modes

Most development and tests should avoid the real Keychain.

Supported runtime knobs:

```text
SHH_KEYCHAIN=file | data-protection      # default: file
SHH_SERVICE_PREFIX=shh-dev               # optional dev namespace
```

- `SHH_KEYCHAIN=file` uses the legacy file-based login keychain. Works with unsigned and ad-hoc-signed binaries. Default.
- `SHH_KEYCHAIN=data-protection` uses the modern data-protection keychain via `kSecUseDataProtectionKeychain = true`. Requires the running binary to be Developer ID signed; uses the team ID as the default access group, no custom provisioning profile needed for items shh owns. Sidesteps the macOS 26.4 file-based regression and gives stable trust across rebuilds with the same signing identity.
- The two keychains are separate stores. Items written under one mode are invisible in the other. v1 does not provide automatic migration — switching mode means starting fresh or running `load` again. A `shh migrate` command is a possible follow-up.
- `SHH_SERVICE_PREFIX=shh-dev` uses the configured Keychain but stores entries under a dev prefix instead of `shh:<profile>`. Useful for iterating on the real adapter without polluting `shh:` entries.
- The in-memory `SecretStore` is gated behind `#[cfg(test)]` and is not reachable from the shipped binary — no env-var or flag toggles the storage backend at runtime. This avoids the footgun of a stale shell-rc env silently redirecting writes to /dev/null.
- Real Keychain integration is tested through `just smoke-keychain` and `shh doctor keychain`.

### User Messaging

After storing a value, print a concise explanation without revealing the value:

```text
Stored OPENAI_API_KEY in macOS Keychain under profile work.
macOS may ask for permission the first time shh reads this item.
Choose "Always Allow" to avoid repeated prompts for this signed shh binary.
```

When access is denied or cancelled, show actionable guidance:

```text
Keychain access was denied.
Rerun the command and approve the macOS prompt, or inspect the item in Keychain Access:
service shh:work, account OPENAI_API_KEY.
```

### Future ACL Investigation

Only investigate explicit ACL customization after the standard signed-binary flow is implemented and smoke-tested. The investigation should answer:

- Does creating an item with explicit trusted-app access reduce prompts reliably?
- Does that trust survive release binary updates?
- Does it behave differently for unsigned dev builds, ad-hoc signed builds, and Developer ID signed builds?
- Does it interact poorly with iCloud Keychain or data-protection Keychain behavior?

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
just install
just install-signed
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
- `install` runs `cargo install --path .` to produce an unsigned/ad-hoc-signed binary in `~/.cargo/bin/shh`. This is the v1 supported install path.
- `install-signed` runs a release build, codesigns the binary with the identity from `SHH_SIGN_IDENTITY` (`Developer ID Application: ...`) using `--timestamp --options runtime`, and copies it to `~/.cargo/bin/shh`. Provided for users with a Developer ID who want stable Keychain trust across rebuilds; not exercised by v1 tests.
- `smoke-keychain` performs a local macOS-only set/get/list/delete test against a clearly test-scoped profile.
- `ci` runs formatting, linting, and tests.

The smoke test must use a reserved profile such as `__shh_smoke__` and clean up after itself.

For v1, only the unsigned `just install` path is tested. `just install-signed` ships as a convenience but is not covered by acceptance criteria — signed-install verification is a follow-up if the personal-use workflow shifts toward frequent rebuilds.

## Prompt Dependency Research

`shh` needs two interactive primitives in v1: hidden secret input for `set NAME` and a checklist for `load`.

Good current Rust options:

- `inquire` - feature-rich and modern. Supports text, password, select, multi-select with filter/search, validators, autocomplete, and help text. Best fit for `.env` import where the user may be scrolling and toggling 20+ names — filter-as-you-type matters.
- `dialoguer` - mature and focused. Directly supports password input and multi-select but lacks built-in filter on the multi-select. Lower polish for `load` UX.
- `cliclack` - modern Clack-style UX. More opinionated than needed.
- `requestty` - Inquirer.js-style. Heavier abstraction than v1 needs.

Recommendation: use `inquire` for v1. The filterable multi-select pays for itself in `load`, and the hidden password prompt covers `set`.

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
- `shh ls -p work` lists effective names without values and annotates default/profile/override source, and runs without triggering Keychain permission prompts.
- `shh profiles` lists profiles discovered from Keychain services without triggering Keychain permission prompts.
- `shh run -p work --clean -- env` shows resolved profile vars plus only the minimal safe baseline (`PATH`, `HOME`, `USER`, `SHELL`, `TERM`, `LANG`, `LC_*`, `TMPDIR`).
- `eval "$(shh unset -p work)"` removes the previously exported names from the current shell.
- `shh completions zsh > _shh` writes a working zsh completion script.
- `shh doctor keychain` writes, reads, lists, and deletes only a reserved diagnostic entry.
- Keychain denial and user-cancelled permission flows produce clear non-secret error messages.
- `nix develop` opens a shell with the Rust toolchain and `just` available.
- `just ci` runs format checks, lint checks, and tests.
- `just install` produces a working `shh` binary in `~/.cargo/bin/` via the unsigned/ad-hoc path. Subsequent commands (`shh doctor keychain`, `shh set`, `shh get`) work against the file-based keychain.
- `just smoke-keychain` verifies the real Keychain adapter on macOS without leaving test entries behind.
- Unit tests cover env-name validation, shell quoting, profile overlay behavior, dotenv parsing, and load selection behavior.
- Command tests cover TTY refusal paths for `get` and `export`.

## Implementation Milestones

1. CLI skeleton and in-memory storage
   - Verify: command parser tests pass and handlers work against a mock store.

2. Validation, quoting, and profile overlay
   - Verify: unit tests cover invalid names, override collisions, and shell-special values.

3. Keychain storage adapter
   - Verify: local macOS smoke test can set, get, list, and delete one reserved entry.

4. Keychain diagnostics and messaging
   - Verify: `shh doctor keychain` uses only a reserved profile and reports permission failures without leaking values.

5. Safe output commands
   - Verify: `get` and `export` refuse TTY output and work through pipes.

6. `.env` import flow
   - Verify: parser tests pass, `--all`, `--only`, `--except`, and `--dry-run` behave as specified.

7. `run` command
   - Verify: child process receives overlaid env and returns the child exit code.

8. Packaging polish
   - Verify: release build produces one binary and README quickstart matches actual behavior.

9. Nix and Justfile developer workflow
   - Verify: `nix develop`, `just ci`, and `just smoke-keychain` work on macOS.

## Future Follow-Ups

- Explicit Keychain ACL customization if standard signed-binary Keychain behavior causes too many prompts.
- `shh migrate` to copy items between the file-based and data-protection keychain stores when a user upgrades to a signed build.
- Broader dotenv syntax (e.g. `${VAR:-default}`) if real `.env` files require it.
