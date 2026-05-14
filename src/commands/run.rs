use std::process::Command;

use crate::error::{Result, ShhError};
use crate::profile::{is_valid_profile_slug, resolve};
use crate::store::SecretStore;

use super::CommandOutcome;

/// Variables preserved across the env wipe when `--clean` is set.
const CLEAN_BASELINE: &[&str] = &["PATH", "HOME", "USER", "SHELL", "TERM", "LANG", "TMPDIR"];

pub struct RunArgs {
    pub profile: Option<String>,
    pub clean: bool,
    pub argv: Vec<String>,
}

pub fn run(args: RunArgs, store: &dyn SecretStore) -> Result<CommandOutcome> {
    if let Some(p) = args.profile.as_deref() {
        if !is_valid_profile_slug(p) {
            return Err(ShhError::InvalidProfile(p.to_string()));
        }
    }
    if args.argv.is_empty() {
        eprintln!(
            "error: missing child command. Usage: shh run [-p PROFILE] [--clean] -- CMD [ARGS...]"
        );
        return Ok(CommandOutcome::ExitCode(2));
    }

    let env = resolve(store, args.profile.as_deref())?;

    let (program, child_args) = args.argv.split_first().unwrap();
    let mut cmd = Command::new(program);
    cmd.args(child_args);

    if args.clean {
        cmd.env_clear();
        for key in CLEAN_BASELINE {
            if let Ok(v) = std::env::var(key) {
                cmd.env(key, v);
            }
        }
        for (k, v) in std::env::vars() {
            if k.starts_with("LC_") {
                cmd.env(k, v);
            }
        }
    }
    for (k, v) in &env.vars {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().map_err(ShhError::Io)?;
    let status = child.wait().map_err(ShhError::Io)?;
    match status.code() {
        Some(code) => Ok(CommandOutcome::ExitCode(code)),
        None => {
            eprintln!("error: child terminated by signal");
            Ok(CommandOutcome::ExitCode(1))
        }
    }
}
