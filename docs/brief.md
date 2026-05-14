# shh — Product Brief

## What it is

`shh` is a macOS CLI for storing environment variables (especially sensitive API keys) in the system Keychain and loading them into shells or child processes on demand. It replaces the common-but-bad habit of hardcoding API keys into `.zshrc`, `.env` files, or shell profiles.

## Why it exists

Three concrete pains:

1. **Secrets in dotfiles.** Keys like `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `AWS_SECRET_ACCESS_KEY` end up plain-text in `~/.zshrc` (or sourced from a `.env`). They leak via backups, screen shares, dotfile repos, and shoulder-surfers. They're also globally exported to every process the shell spawns.

2. **Ad hoc `.env` handling.** `.env` files are convenient, but they mix real secrets with local config (`PORT`, `DEBUG`, `NODE_ENV`, local URLs). Moving from `.env` to Keychain should not mean blindly importing every variable.

3. **Over-broad env exposure to child processes.** When a tool or agent runs, it inherits the shell environment — every key currently loaded, whether or not that process needs them.

`shh` fixes this by keeping secrets at rest in the Keychain (encrypted, OS-protected) and making secret-bearing shell sessions explicit. The primary flow is loading chosen secrets into the current shell with `export`; process-scoped execution with `run` is available when the user wants a one-off command instead.

## Core principles

- **At rest, always encrypted.** Values live in the macOS Keychain, never on disk in plaintext.
- **Pit of success.** The CLI refuses unsafe usage (e.g. printing values to a TTY) rather than relying on user discipline.
- **Explicit activation.** Secrets are loaded into a shell only when the user asks for them.
- **Selective import.** Loading a `.env` should let the user choose which variables deserve Keychain storage.
- **Single binary, zero config.** Install, set, export.

## Users

Primarily the author and developers who:
- Juggle multiple API keys across personal/work contexts.
- Want clean shell profiles without losing the convenience of env vars.
- Use tools or agents that sometimes need a focused set of secrets.
- Are on macOS (Keychain-specific; portability is not a goal v1).

## Concepts

### Profile

A named group of env vars. Profiles let you separate contexts (e.g. `work`, `personal`, `openai`, `aws`). There is always an implicit `default` profile.

When a profile is used, `shh` merges `default` as the baseline and overlays the named profile on top. Named-profile values override `default` on key collisions.

### Entry

A single `(name, value)` pair within a profile. Stored as a Keychain generic-password item:

- `service`: `shh:<profile>` (e.g. `shh:default`, `shh:work`)
- `account`: env var name (e.g. `OPENAI_API_KEY`)
- `password`: the secret value

Access Control List is configured so the `shh` binary can read its own entries without prompting on every call.

## Command surface (v1)

```
shh set <NAME> <VALUE> [-p <profile>]      # store / update an entry
shh get <NAME>          [-p <profile>]     # print a single value (pipe-only, see below)
shh rm  <NAME>          [-p <profile>]     # delete an entry
shh ls                  [-p <profile>]     # list entry names (never values)
shh profiles                                # list profiles
shh load <PATH>         [-p <profile>]     # import selected entries from a .env file
shh export              [-p <profile>]     # print `export K=V` lines for eval
shh run                 [-p <profile>] -- <cmd> [args...]   # spawn child with env injected
```

### Behaviors

- **`set`** creates or updates. If a value would be visible on the command line (shell history), `shh set <NAME>` with no value reads from stdin / prompts interactively. (TBD whether the positional form is allowed at all — leaning: allow but warn.)
- **`load`** parses a `.env` file and stores selected variables in the target profile. When attached to a TTY, it shows an interactive checklist so the user can unselect non-secrets or unwanted entries before saving. It prints names and a summary only, never values.
- **`load --all`** skips the checklist and imports every valid entry. This is useful for scripts or trusted `.env` files.
- **`load --only NAME,NAME`** imports an explicit subset. **`load --except NAME,NAME`** imports all valid entries except the listed names. (Exact flag shape TBD.)
- **`load --dry-run`** reports what would be added or updated without writing to Keychain.
- **`get`** and **`export`** refuse to write to a TTY. If stdout is a terminal, they exit with an error explaining how to use them (`eval "$(shh export -p work)"`). This makes accidental disclosure structurally hard.
- **`export`** is the primary activation flow for interactive shells:

  ```
  eval "$(shh export -p work)"
  ```

  This intentionally creates a secret-bearing shell session without requiring persistent `export FOO=...` lines in `.zshrc`.

- **`run`** spawns the target command with the current process env *inherited* and the resolved profile env *overlaid*. The parent shell is untouched. This is useful for one-off commands, scripts, and cases where the user does not want to mutate the current shell:

  ```
  shh run -p work -- claude
  ```

- **Profile merging:** `-p work` resolves as `default` ∪ `work`, with `work` winning on conflicts.
- **No `-p` flag** means just `default`.
- **Storage conflicts:** the same env var name can exist in multiple profiles because the profile is part of the Keychain item identity:

  ```
  service: shh:work
  account: OPENAI_API_KEY

  service: shh:personal
  account: OPENAI_API_KEY
  ```

  `OPENAI_API_KEY` in `work` and `OPENAI_API_KEY` in `personal` are separate entries. Do not rewrite names to `WORK_OPENAI_API_KEY`; keep the name the consuming program expects and let profiles choose the value.
- **Load conflicts:** if `shh load .env -p work` sees a key that already exists in `work`, it updates that entry after selection. It does not delete existing profile entries that are absent from the `.env` file.

## Out of scope (v1)

- Cross-machine sync (iCloud Keychain handles this for free if the user enables it; we don't ship a sync layer).
- Non-macOS platforms (Linux Secret Service, Windows Credential Manager).
- Team/shared secrets, RBAC, audit logs.
- Secret rotation, expiry, or reminders.
- File-based secrets (certs, kubeconfigs). Values are strings only.
- A TUI or GUI. CLI only.

## Tech choices

- **Language:** Rust. Single static binary, fast startup (matters for `run`).
- **Keychain access:** Apple's `Security.framework` via a Rust binding (`security-framework` crate is the obvious candidate). Avoid shelling out to the `security` CLI — slower, weaker control over ACLs.
- **CLI framework:** `clap` with derive macros.

## Risks / open questions

- **First-use ACL prompts.** Need to confirm that setting the ACL at creation time to allow the `shh` binary results in silent reads across runs, surviving binary updates (signature-based ACL behavior on macOS is finicky).
- **`set` from positional arg vs stdin.** Positional is convenient but lands the value in shell history. Stdin-only is safer but worse UX. Probably: support both, document the risk, maybe warn if a positional value is detected.
- **`.env` parser scope.** Need to decide how much dotenv syntax v1 supports: comments, blank lines, quoted values, `export NAME=...`, multiline values, interpolation, and invalid identifiers.
- **Interactive checklist implementation.** Need a lightweight prompt/checklist dependency or a simple built-in terminal selector. Non-interactive modes (`--all`, `--only`, `--except`) should work without prompts.
- **Profile discovery.** Listing profiles requires enumerating Keychain items by service prefix. Need to verify performance and that we can filter cleanly.
- **Process inheritance leakage.** Even with `shh run`, the child inherits the parent shell's existing env. If the user already has unrelated secrets loaded, they leak through. Mitigation: document this clearly, and consider a `--clean` flag for pristine env later.

## Success criteria for v1

- I can delete every `export FOO_KEY=…` line from my `.zshrc`.
- `eval "$(shh export -p <ctx>)"` becomes my default way to activate secrets for the current shell.
- I can import a `.env` without blindly saving every variable in it.
- Adding a new secret takes one command and never touches a file I'd commit.
