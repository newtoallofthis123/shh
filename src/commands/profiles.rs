use std::collections::BTreeSet;

use crate::error::Result;
use crate::store::SecretStore;

pub fn run(store: &dyn SecretStore) -> Result<()> {
    let mut profiles: BTreeSet<String> = store.list_profiles()?.into_iter().collect();
    if !store.list_names("default")?.is_empty() {
        profiles.insert("default".to_string());
    }
    for p in profiles {
        println!("{p}");
    }
    Ok(())
}
