# shh — Product Brief

## What it is

`shh` is a macOS CLI for storing environment variables (especially sensitive API keys) in the system Keychain and injecting them into shells or child processes on demand. It replaces the common-but-bad habit of hardcoding API keys into `.zshrc`, `.env` files, or shell profiles.

## Why it exists

Two concrete pains:

1. **Secrets in dotfiles.** Keys like `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `AWS_SECRET_ACCESS_KEY` end up plain-text in `~/.zshrc` (or sourced from a `.env`). They leak via backups, screen shares, dotfile repos, and shoulder-surfers. They're also globally exported to every process the shell spawns.

2. **Over-broad env exposure to agent tools.** When a CLI agent (Claude Code, Cursor, etc.) runs, it inherits the entire shell environment — every key for every service, whether or not the agent needs them. There's no scoping.

`shh` fixes both by keeping secrets at rest in the Keychain (encrypted, OS-protected) and exposing them only when explicitly requested, scoped to the process that needs them.

## Core principles

- **At rest, always encrypted.** Values live in the macOS Keychain, never on disk in plaintext.
- **Pit of success.** The CLI refuses unsafe usage (e.g. printing values to a TTY) rather than relying on user discipline.
- **Scoped exposure.** Prefer giving a single child process the env it needs over exporting globally.
- **Single binary, zero config.** Install, set, run.

## Users

Primarily the author and developers who:
- Juggle multiple API keys across personal/work contexts.
- Use AI coding agents and don't want to hand them the whole keyring.
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
shh export              [-p <profile>]     # print `export K=V` lines for eval
shh run                 [-p <profile>] -- <cmd> [args...]   # spawn child with env injected
```

### Behaviors

- **`set`** creates or updates. If a value would be visible on the command line (shell history), `shh set <NAME>` with no value reads from stdin / prompts interactively. (TBD whether the positional form is allowed at all — leaning: allow but warn.)
- **`get`** and **`export`** refuse to write to a TTY. If stdout is a terminal, they exit with an error explaining how to use them (`eval "$(shh export -p work)"`). This makes accidental disclosure structurally hard.
- **`run`** spawns the target command with the current process env *inherited* and the resolved profile env *overlaid*. The parent shell is untouched. This is the recommended path for agent tools:

  ```
  shh run -p work -- claude
  ```

- **Profile merging:** `-p work` resolves as `default` ∪ `work`, with `work` winning on conflicts.
- **No `-p` flag** means just `default`.

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
- **Profile discovery.** Listing profiles requires enumerating Keychain items by service prefix. Need to verify performance and that we can filter cleanly.
- **Agent inheritance leakage.** Even with `shh run`, the child inherits the parent shell's existing env. If the user already has `OPENAI_API_KEY` exported in `.zshrc`, it leaks through. Mitigation: document this clearly, and consider a `--clean` flag for pristine env later.

## Success criteria for v1

- I can delete every `export FOO_KEY=…` line from my `.zshrc`.
- `shh run -p <ctx> -- <agent>` becomes my default way to launch coding agents.
- Adding a new secret takes one command and never touches a file I'd commit.
