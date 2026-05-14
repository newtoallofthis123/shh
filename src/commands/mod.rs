//! Command handlers and the central dispatcher.
//!
//! Each handler is narrow and depends only on [`SecretStore`] so it can be
//! exercised against the in-memory store from integration tests. The
//! dispatcher returns a [`CommandOutcome`] rather than calling
//! `std::process::exit` directly — only `main.rs` exits the process.

use crate::cli::{Command, DoctorCmd};
use crate::error::Result;
use crate::store::SecretStore;

pub mod completions;
pub mod doctor;
pub mod export;
pub mod get;
pub mod load;
pub mod ls;
pub mod profiles;
pub mod rm;
pub mod run;
pub mod set;
pub mod unset;

/// Resolved exit behavior for a single command invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    Success,
    ExitCode(i32),
}

/// Dispatch a parsed command against the given store.
pub fn dispatch(cmd: Command, store: &dyn SecretStore) -> Result<CommandOutcome> {
    match cmd {
        Command::Set {
            name,
            value,
            profile,
        } => {
            set::run(
                set::SetArgs {
                    name,
                    value,
                    profile,
                },
                store,
            )?;
            Ok(CommandOutcome::Success)
        }
        Command::Get { name, profile } => get::run(name, profile.as_deref(), store),
        Command::Rm { name, profile } => rm::run(name, profile.as_deref(), store),
        Command::Ls { profile } => {
            ls::run(profile.as_deref(), store)?;
            Ok(CommandOutcome::Success)
        }
        Command::Profiles => {
            profiles::run(store)?;
            Ok(CommandOutcome::Success)
        }
        Command::Load {
            path,
            profile,
            all,
            only,
            except,
            dry_run,
        } => load::run(
            load::LoadArgs {
                path,
                profile,
                all,
                only,
                except,
                dry_run,
            },
            store,
        ),
        Command::Export { profile } => export::run(profile.as_deref(), store),
        Command::Unset { profile } => unset::run(profile.as_deref(), store),
        Command::Run {
            profile,
            clean,
            argv,
        } => run::run(
            run::RunArgs {
                profile,
                clean,
                argv,
            },
            store,
        ),
        Command::Doctor { which } => match which {
            DoctorCmd::Keychain => doctor::run_keychain(store),
        },
        Command::Completions { shell } => {
            completions::run(shell);
            Ok(CommandOutcome::Success)
        }
    }
}
