use std::io::Write;

use crate::env::is_valid_env_name;
use crate::error::{Result, ShhError};
use crate::profile::{is_valid_profile_slug, resolve};
use crate::store::SecretStore;
use crate::tty::is_stdout_tty;

use super::CommandOutcome;

pub fn run(name: String, profile: Option<&str>, store: &dyn SecretStore) -> Result<CommandOutcome> {
    if !is_valid_env_name(&name) {
        return Err(ShhError::InvalidEnvName(name));
    }
    if let Some(p) = profile {
        if !is_valid_profile_slug(p) {
            return Err(ShhError::InvalidProfile(p.to_string()));
        }
    }
    if is_stdout_tty() {
        eprintln!(
            "error: refusing to print secret to a terminal. Pipe stdout (e.g. `shh get {name} | pbcopy`).",
        );
        return Ok(CommandOutcome::ExitCode(2));
    }

    let env = resolve(store, profile)?;
    match env.vars.get(&name) {
        Some(value) => {
            std::io::stdout().write_all(value.as_bytes())?;
            std::io::stdout().flush()?;
            Ok(CommandOutcome::Success)
        }
        None => {
            let scope = profile.unwrap_or("default");
            eprintln!("error: {name} not found in profile {scope}");
            Ok(CommandOutcome::ExitCode(1))
        }
    }
}
