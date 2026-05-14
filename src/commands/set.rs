use std::io::Read;

use crate::env::is_valid_env_name;
use crate::error::{Result, ShhError};
use crate::profile::is_valid_profile_slug;
use crate::store::SecretStore;
use crate::tty::is_stdin_tty;

pub struct SetArgs {
    pub name: String,
    pub value: Option<String>,
    pub profile: Option<String>,
}

pub fn run(args: SetArgs, store: &dyn SecretStore) -> Result<()> {
    if !is_valid_env_name(&args.name) {
        return Err(ShhError::InvalidEnvName(args.name));
    }
    let profile = args.profile.as_deref().unwrap_or("default");
    if !is_valid_profile_slug(profile) {
        return Err(ShhError::InvalidProfile(profile.to_string()));
    }

    let value = match (args.value, is_stdin_tty()) {
        (Some(v), _) => {
            eprintln!("warning: values on the command line may be stored in shell history");
            v
        }
        (None, false) => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
        (None, true) => inquire::Password::new(&format!("Value for {}:", args.name))
            .without_confirmation()
            .with_display_mode(inquire::PasswordDisplayMode::Hidden)
            .prompt()
            .map_err(|e| ShhError::Keychain(format!("prompt error: {e}")))?,
    };

    store.set(profile, &args.name, &value)?;

    println!(
        "Stored {} in macOS Keychain under profile {}.",
        args.name, profile
    );
    println!("macOS may ask for permission the first time shh reads this item.");
    println!("Choose \"Always Allow\" to avoid repeated prompts for this signed shh binary.");
    Ok(())
}
