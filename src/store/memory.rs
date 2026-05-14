use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::error::Result;
use crate::store::SecretStore;

#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<BTreeMap<(String, String), String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn set(&self, profile: &str, name: &str, value: &str) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.insert((profile.to_string(), name.to_string()), value.to_string());
        Ok(())
    }

    fn get(&self, profile: &str, name: &str) -> Result<Option<String>> {
        let g = self.inner.lock().unwrap();
        Ok(g.get(&(profile.to_string(), name.to_string())).cloned())
    }

    fn delete(&self, profile: &str, name: &str) -> Result<bool> {
        let mut g = self.inner.lock().unwrap();
        Ok(g.remove(&(profile.to_string(), name.to_string())).is_some())
    }

    fn list_names(&self, profile: &str) -> Result<Vec<String>> {
        let g = self.inner.lock().unwrap();
        Ok(g.keys()
            .filter(|(p, _)| p == profile)
            .map(|(_, n)| n.clone())
            .collect())
    }

    fn list_profiles(&self) -> Result<Vec<String>> {
        let g = self.inner.lock().unwrap();
        let mut profiles: Vec<String> = g.keys().map(|(p, _)| p.clone()).collect();
        profiles.sort();
        profiles.dedup();
        Ok(profiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let s = MemoryStore::new();
        assert_eq!(s.get("default", "FOO").unwrap(), None);
        s.set("default", "FOO", "bar").unwrap();
        assert_eq!(s.get("default", "FOO").unwrap(), Some("bar".to_string()));
        s.set("default", "BAZ", "qux").unwrap();
        s.set("other", "ALPHA", "beta").unwrap();

        let mut names = s.list_names("default").unwrap();
        names.sort();
        assert_eq!(names, vec!["BAZ".to_string(), "FOO".to_string()]);

        let profiles = s.list_profiles().unwrap();
        assert_eq!(profiles, vec!["default".to_string(), "other".to_string()]);

        assert!(s.delete("default", "FOO").unwrap());
        assert!(!s.delete("default", "FOO").unwrap());
        assert_eq!(s.get("default", "FOO").unwrap(), None);
    }
}
