use crate::error::{Result, ShhError};
use crate::profile::{is_valid_profile_slug, resolve_with_sources, Source};
use crate::store::SecretStore;

pub fn run(profile: Option<&str>, store: &dyn SecretStore) -> Result<()> {
    match profile {
        None => {
            let mut names = store.list_names("default")?;
            names.sort();
            for n in names {
                println!("{n}");
            }
        }
        Some(p) => {
            if !is_valid_profile_slug(p) {
                return Err(ShhError::InvalidProfile(p.to_string()));
            }
            let entries = resolve_with_sources(store, p)?;
            for (name, src) in entries {
                let label = match src {
                    Source::Default => "default".to_string(),
                    Source::Profile => p.to_string(),
                    Source::Override => format!("{p} overrides default"),
                };
                println!("{name}\t{label}");
            }
        }
    }
    Ok(())
}
