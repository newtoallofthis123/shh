# Chapter E — Command handlers & wiring

**Type:** conscious
**Depends on:** A, B, C, D

## Executive summary

Implement every command, dispatch them from `main.rs`, and apply the PRD's TTY guards, prompts, and child-process semantics. This chapter touches the most files and integrates everything else.

Conscious because: TTY guards must be applied uniformly, interactive prompts have UX nuance, `run` involves OS-level child-process env handling, and `doctor` orchestrates a careful self-test that must not leak.

## Files touched

- `src/commands/mod.rs` (create)
- `src/commands/set.rs`, `get.rs`, `rm.rs`, `ls.rs`, `profiles.rs`, `load.rs`, `export.rs`, `unset.rs`, `run.rs`, `completions.rs`, `doctor.rs` (create)
- `src/tty.rs` (create — `is_stdin_tty`, `is_stdout_tty` wrappers around `std::io::IsTerminal`)
- `src/main.rs` (replace stubs with real dispatch)
- `src/lib.rs` (extend: `pub mod commands; pub mod tty;`)
- `tests/cli.rs` (create — integration tests against `MemoryStore` via a small test helper that injects a store)

## Success criteria

Each PRD acceptance criterion (lines 407–428 of `docs/prd-ticket.md`) is testable. In particular:

- `shh get NAME` to a TTY exits non-zero with guidance, never prints the value.
- `shh get NAME | cat` prints the raw value (no trailing newline appended unexpectedly — match PRD example `pbcopy`).
- `shh export` and `shh unset` refuse a TTY and produce shell-safe `export NAME='...'` / `unset NAME` lines otherwise. Values use `posix_quote`.
- `shh ls` and `shh profiles` never call `get` — they use only `list_names` / `list_profiles`.
- `shh ls -p work` output format: `NAME    <source>` per line, where source is `default`, `<profile>`, or `<profile> overrides default`.
- `shh load FILE` with no flags in a TTY shows an `inquire::MultiSelect` of parsed names; non-TTY without `--all|--only|--except` exits with usage error.
- `shh load --dry-run` prints `would add NAME` or `would update NAME` per resolved selection and writes nothing.
- `shh run -p work -- env` spawns child with inherited env + overlaid resolved vars; `--clean` builds a fresh env with only `PATH, HOME, USER, SHELL, TERM, LANG, LC_*, TMPDIR` plus resolved vars; exit code matches child.
- `shh completions zsh` writes a script to stdout.
- `shh doctor keychain` writes/reads/lists/deletes under reserved profile `__shh_doctor__`, prints active store + signing identity + result lines, and cleans up even on partial failure.
- `shh set NAME VALUE` warns about shell history on the positional form; piped or prompted forms do not warn.

## Phases

### E.1 — Dispatcher and store injection

- **Goal:** `run()` in `main.rs` constructs a `Box<dyn SecretStore>` (always `KeychainStore::from_env()` for the real binary; integration tests substitute via a `pub fn run_with_store(cli: Cli, store: &dyn SecretStore) -> Result<()>` shim exposed from `lib.rs`).
- **Files & changes:** `src/commands/mod.rs` exposes `pub fn dispatch(cmd: Command, store: &dyn SecretStore) -> Result<()>`. `main.rs` calls it. `tests/cli.rs` uses it directly with `MemoryStore`.

### E.2 — `set`

- **Goal:** Validate name + profile, resolve value source (positional, piped stdin, or hidden prompt), persist, print confirmation per PRD wording.
- **Files & changes:** `src/commands/set.rs`.
- **Code shape:**
  ```rust
  pub fn run(args: SetArgs, store: &dyn SecretStore) -> Result<()> {
      if !is_valid_env_name(&args.name) { bail!("invalid env name: {}", args.name); }
      let profile = args.profile.as_deref().unwrap_or("default");
      if !is_valid_profile_slug(profile) { bail!("invalid profile: {}", profile); }
      let value = match (args.value, is_stdin_tty()) {
          (Some(v), _) => { eprintln!("warning: values on the command line may be stored in shell history"); v }
          (None, false) => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; s }
          (None, true)  => inquire::Password::new(&format!("Value for {}:", args.name))
                              .without_confirmation()
                              .with_display_mode(inquire::PasswordDisplayMode::Hidden)
                              .prompt()?,
      };
      store.set(profile, &args.name, &value)?;
      println!("Stored {} in macOS Keychain under profile {}.", args.name, profile);
      println!("macOS may ask for permission the first time shh reads this item.");
      println!("Choose \"Always Allow\" to avoid repeated prompts for this signed shh binary.");
      Ok(())
  }
  ```

### E.3 — `get`

- **Files & changes:** `src/commands/get.rs`. TTY guard: if `stdout.is_terminal()`, exit non-zero with `error: refusing to print secret to a terminal. Pipe stdout (e.g. \`shh get FOO | pbcopy\`).`
- Resolve via `profile::resolve` (default + overlay) to honor overlay; if name missing → exit 1 with `not found`.
- Write the raw value to stdout via `std::io::stdout().write_all(value.as_bytes())` — no trailing newline.

### E.4 — `rm`

- **Files & changes:** `src/commands/rm.rs`. Validate, then `store.delete(profile, name)` — `Ok(true)` prints `Removed NAME from profile <p>.`; `Ok(false)` exits 1 with `not found in profile <p>`.

### E.5 — `ls`

- **Files & changes:** `src/commands/ls.rs`. With no `-p`, list `store.list_names("default")`. With `-p`, call `profile::resolve_with_sources` and format `{name}\t{source_label}` where source labels are: `default`, `<profile>`, `<profile> overrides default`. Never call `get`.

### E.6 — `profiles`

- **Files & changes:** `src/commands/profiles.rs`. Call `store.list_profiles()` and print one per line, sorted. If `default` entries exist (any name in `store.list_names("default")`), ensure `default` is included.

### E.7 — `load`

- **Files & changes:** `src/commands/load.rs`. Read file, `dotenv::parse`, surface rejected names and parse errors to stderr without values. Determine selector:
  - If `--all/--only/--except/--dry-run`-only is non-interactive: pick selector.
  - In TTY with no flags: `inquire::MultiSelect::new("Select entries to import:", names).with_filter(...).prompt()` → `Interactive` selector.
  - Non-interactive with no flags: error.
- For each resolved entry: if `--dry-run`, print `would add NAME` / `would update NAME` (decide by checking `store.list_names(profile)` once and bucketing). Otherwise call `store.set(profile, name, value)`. Never echo values.

### E.8 — `export`

- **Files & changes:** `src/commands/export.rs`. TTY-guard stdout. Resolve profile via overlay. For each name in sorted order, print `export {name}={posix_quote(value)}`.

### E.9 — `unset`

- **Files & changes:** `src/commands/unset.rs`. TTY-guard stdout. Resolve profile via overlay (so names align with what `export` would emit). For each resolved name, print `unset {name}`.

### E.10 — `run`

- **Files & changes:** `src/commands/run.rs`.
- Resolve profile env.
- Build child via `std::process::Command`. With `--clean`, start from an empty env, then insert the safe baseline from current process env (`PATH, HOME, USER, SHELL, TERM, LANG, TMPDIR`, plus every `LC_*`), then overlay resolved profile vars. Without `--clean`, inherit parent env then overlay resolved vars (`Command::envs(...)`).
- Spawn, wait, propagate exit code via `std::process::exit(code)`.

### E.11 — `completions`

- **Files & changes:** `src/commands/completions.rs`. Use `clap_complete::generate(shell, &mut Cli::command(), "shh", &mut std::io::stdout())`.

### E.12 — `doctor keychain`

- **Files & changes:** `src/commands/doctor.rs`.
- Print active store mode (read `SHH_KEYCHAIN`) and prefix.
- Detect signing identity by shelling out to `codesign -dvv` against `std::env::current_exe()?` — parse stderr for `Identifier=`, `Authority=`, `Signature=adhoc`. Classify as `unsigned`, `ad-hoc`, or `Developer ID`. If `data-protection` mode but not `Developer ID`, warn that the mode requires a signed binary.
- Run a write/read/list/delete cycle against profile `__shh_doctor__` with name `SHH_DOCTOR_PROBE` and value `ok`. Use `defer`-style cleanup: register a `scopeguard` (or a manual `let _guard = OnDrop(...)`) — actually just call `delete` in both `Ok` and `Err` paths. Never print the probe value back.
- Print clearly: `keychain access: ok`, `permission flow: not prompted | granted | denied | cancelled`.

### E.13 — Dispatch + main wiring

- **Files & changes:** rewrite `src/main.rs::run()` to: parse → construct `KeychainStore::from_env()` → call `commands::dispatch`. Map `ShhError::KeychainDenied` specifically to the PRD's `Keychain access was denied` guidance message before exiting.

### E.14 — Integration tests

- **Files & changes:** `tests/cli.rs`.
- Use `MemoryStore` directly via `commands::dispatch`. Cover:
  - `set` happy path + invalid name + invalid profile
  - `get` returns value via overlay; `get` missing → `NotFound`
  - `rm` of missing → returns `false` → handler exits non-zero (assert via capturing the `ShhError`)
  - `ls` source annotation for all three cases (`default`, `<profile>`, `<profile> overrides default`)
  - `load` with `--only`, `--except`, `--all`, `--dry-run` against a temp `.env` file
  - `export` output exactly matches `export NAME='value'` with quoting
  - `unset` output exactly matches `unset NAME`
  - `run` without a child available (skip OS-shell test or run `Command::new("true")` — assert exit 0)
- TTY guards are harder to test without a PTY; assert the guard functions exist and are called by reading the source via cargo test compilation (or factor out a `OutputSink` trait with a `is_tty()` for the test).
