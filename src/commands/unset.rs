use crate::error::{Result, ShhError};
use crate::profile::{is_valid_profile_slug, resolve};
use crate::store::SecretStore;
use crate::tty::is_stdout_tty;

use super::CommandOutcome;

pub fn run(profile: Option<&str>, store: &dyn SecretStore) -> Result<CommandOutcome> {
    if let Some(p) = profile {
        if !is_valid_profile_slug(p) {
            return Err(ShhError::InvalidProfile(p.to_string()));
        }
    }
    if is_stdout_tty() {
        eprintln!(
            "error: refusing to print unset lines to a terminal. Use: eval \"$(shh unset{})\"",
            profile.map(|p| format!(" -p {p}")).unwrap_or_default()
        );
        return Ok(CommandOutcome::ExitCode(2));
    }
    let env = resolve(store, profile)?;
    for name in env.vars.keys() {
        println!("unset {name}");
    }
    Ok(CommandOutcome::Success)
}
