use crate::env::is_valid_env_name;
use crate::error::{Result, ShhError};
use crate::profile::is_valid_profile_slug;
use crate::store::SecretStore;

use super::CommandOutcome;

pub fn run(name: String, profile: Option<&str>, store: &dyn SecretStore) -> Result<CommandOutcome> {
    if !is_valid_env_name(&name) {
        return Err(ShhError::InvalidEnvName(name));
    }
    let profile = profile.unwrap_or("default");
    if !is_valid_profile_slug(profile) {
        return Err(ShhError::InvalidProfile(profile.to_string()));
    }
    if store.delete(profile, &name)? {
        println!("Removed {name} from profile {profile}.");
        Ok(CommandOutcome::Success)
    } else {
        eprintln!("error: {name} not found in profile {profile}");
        Ok(CommandOutcome::ExitCode(1))
    }
}
