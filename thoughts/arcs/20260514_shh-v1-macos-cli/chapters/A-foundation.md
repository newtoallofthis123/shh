# Chapter A — Foundation & storage trait

**Type:** strict
**Depends on:** none

## Executive summary

Stand up the Rust crate, define the `SecretStore` trait, ship the `#[cfg(test)]` in-memory adapter, define error types, and lay out the clap subcommand enum with stub dispatch. No command logic, no Keychain. After this chapter, `cargo build`, `cargo test`, and `cargo run -- --help` all succeed.

## Files touched

- `Cargo.toml` (create)
- `Cargo.lock` (generated)
- `rust-toolchain.toml` (create, stable channel)
- `src/main.rs` (create)
- `src/lib.rs` (create — internal use; binary re-uses it)
- `src/cli.rs` (create)
- `src/error.rs` (create)
- `src/store/mod.rs` (create)
- `src/store/memory.rs` (create, `#[cfg(test)]`)
- `.gitignore` (extend with `/target`, `Cargo.lock` kept since this is a binary crate)

## Success criteria

- `cargo build` succeeds with no warnings under `-D warnings`.
- `cargo test` runs (no tests yet beyond a trivial in-memory store smoke test) and passes.
- `cargo run -- --help` prints help listing every subcommand from the PRD: `set`, `get`, `rm`, `ls`, `profiles`, `load`, `export`, `unset`, `run`, `doctor`, `completions`.
- Every subcommand stub returns a clear `unimplemented in this chapter` error so dispatch wiring is provable.
- In-memory store is gated behind `#[cfg(test)]` — `rg "memory::MemoryStore" src/` outside `#[cfg(test)]` blocks finds nothing in shipped code.

## Phases

### A.1 — Cargo crate scaffold

- **Goal:** Single binary crate `shh` with declared dependencies, stable Rust toolchain pin.
- **Files & changes:**
  - `Cargo.toml`: `[package] name="shh" version="0.1.0" edition="2021"`. `[[bin]] name="shh" path="src/main.rs"`. `[lib] path="src/lib.rs"`.
  - Dependencies: `clap = { version = "4", features = ["derive"] }`, `clap_complete = "4"`, `inquire = "0.7"`, `anyhow = "1"`, `thiserror = "1"`, `security-framework = "2"` (target-gated to `cfg(target_os = "macos")` — define as a normal dep since v1 is macOS-only; gate at use-site if any docs build on other OS).
  - `rust-toolchain.toml`: `[toolchain] channel = "stable"`.
- **Code:**
  ```toml
  [package]
  name = "shh"
  version = "0.1.0"
  edition = "2021"

  [[bin]]
  name = "shh"
  path = "src/main.rs"

  [lib]
  path = "src/lib.rs"

  [dependencies]
  clap = { version = "4", features = ["derive"] }
  clap_complete = "4"
  inquire = "0.7"
  anyhow = "1"
  thiserror = "1"
  security-framework = "2"
  ```

### A.2 — Error types

- **Goal:** Library-level error enum with `thiserror`; handlers consume via `anyhow`.
- **Files & changes:** `src/error.rs` exposes `ShhError` covering: invalid env name, invalid profile slug, not found, keychain access, keychain denied/cancelled, dotenv parse, io. `Result<T> = std::result::Result<T, ShhError>`.
- **Code:**
  ```rust
  use thiserror::Error;

  #[derive(Debug, Error)]
  pub enum ShhError {
      #[error("invalid env name: {0}")]
      InvalidEnvName(String),
      #[error("invalid profile slug: {0}")]
      InvalidProfile(String),
      #[error("not found")]
      NotFound,
      #[error("keychain access denied or cancelled")]
      KeychainDenied,
      #[error("keychain error: {0}")]
      Keychain(String),
      #[error("dotenv parse error at line {line}: {message}")]
      DotenvParse { line: usize, message: String },
      #[error(transparent)]
      Io(#[from] std::io::Error),
  }

  pub type Result<T> = std::result::Result<T, ShhError>;
  ```

### A.3 — `SecretStore` trait + `#[cfg(test)]` memory store

- **Goal:** Trait that command handlers depend on. Test-only `MemoryStore`.
- **Files & changes:**
  - `src/store/mod.rs`: trait + re-exports.
  - `src/store/memory.rs`: `MemoryStore` behind `#[cfg(test)]` — back with `Mutex<BTreeMap<(String,String), String>>`.
- **Code:**
  ```rust
  // src/store/mod.rs
  use crate::error::Result;

  pub trait SecretStore: Send + Sync {
      fn set(&self, profile: &str, name: &str, value: &str) -> Result<()>;
      fn get(&self, profile: &str, name: &str) -> Result<Option<String>>;
      fn delete(&self, profile: &str, name: &str) -> Result<bool>;
      fn list_names(&self, profile: &str) -> Result<Vec<String>>;
      fn list_profiles(&self) -> Result<Vec<String>>;
  }

  #[cfg(test)]
  pub mod memory;
  ```
  - Memory store: simple `BTreeMap`, returns `NotFound`-equivalent semantics correctly (`delete` returns `false` when missing; `get` returns `Ok(None)`).
  - Include a smoke unit test in `src/store/memory.rs` exercising set/get/delete/list_names/list_profiles.

### A.4 — CLI model

- **Goal:** Clap derive enum mirroring the PRD command surface. No handler logic.
- **Files & changes:** `src/cli.rs` exports `Cli` and `Command` enums.
- **Code:**
  ```rust
  use clap::{Parser, Subcommand};
  use std::path::PathBuf;

  #[derive(Parser, Debug)]
  #[command(name = "shh", version, about = "macOS Keychain-backed env-var secrets")]
  pub struct Cli {
      #[command(subcommand)]
      pub command: Command,
  }

  #[derive(Subcommand, Debug)]
  pub enum Command {
      Set {
          name: String,
          value: Option<String>,
          #[arg(short = 'p', long)]
          profile: Option<String>,
      },
      Get { name: String, #[arg(short = 'p', long)] profile: Option<String> },
      Rm  { name: String, #[arg(short = 'p', long)] profile: Option<String> },
      Ls  { #[arg(short = 'p', long)] profile: Option<String> },
      Profiles,
      Load {
          path: PathBuf,
          #[arg(short = 'p', long)] profile: Option<String>,
          #[arg(long)] all: bool,
          #[arg(long, value_delimiter = ',')] only: Vec<String>,
          #[arg(long, value_delimiter = ',')] except: Vec<String>,
          #[arg(long)] dry_run: bool,
      },
      Export { #[arg(short = 'p', long)] profile: Option<String> },
      Unset  { #[arg(short = 'p', long)] profile: Option<String> },
      Run {
          #[arg(short = 'p', long)] profile: Option<String>,
          #[arg(long)] clean: bool,
          #[arg(last = true)] argv: Vec<String>,
      },
      Doctor { #[command(subcommand)] which: DoctorCmd },
      Completions { shell: clap_complete::Shell },
  }

  #[derive(Subcommand, Debug)]
  pub enum DoctorCmd { Keychain }
  ```
  - Note `value_delimiter = ','` plus repeatable `--only/--except` satisfies the PRD requirement.
  - Note `argv: Vec<String>` after `last = true` requires `--` before the child command — matches PRD.

### A.5 — `main.rs` and `lib.rs` dispatch skeleton

- **Goal:** `lib.rs` exposes modules; `main.rs` parses CLI and dispatches via a single `match` to stub handlers that return `Err(anyhow!("unimplemented in chapter A"))`. Print error to stderr and exit non-zero on `Err`.
- **Files & changes:**
  - `src/lib.rs`: `pub mod cli; pub mod error; pub mod store;`
  - `src/main.rs`: parse, match, dispatch stubs; map errors to `eprintln!` + `std::process::exit(1)`.
- **Code:**
  ```rust
  // src/main.rs
  use anyhow::Result;
  use clap::Parser;
  use shh::cli::{Cli, Command};

  fn main() {
      if let Err(e) = run() {
          eprintln!("error: {e:#}");
          std::process::exit(1);
      }
  }

  fn run() -> Result<()> {
      let cli = Cli::parse();
      match cli.command {
          Command::Set { .. } => anyhow::bail!("set: not yet implemented"),
          // ...one arm per command, all bail.
          _ => anyhow::bail!("not yet implemented"),
      }
  }
  ```
