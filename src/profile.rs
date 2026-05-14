use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Result, ShhError};
use crate::store::SecretStore;

pub fn is_valid_profile_slug(p: &str) -> bool {
    if p.is_empty() {
        return false;
    }
    p.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[derive(Debug)]
pub struct ProfileEnv {
    pub vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Default,
    Profile,
    Override,
}

pub fn resolve(store: &dyn SecretStore, profile: Option<&str>) -> Result<ProfileEnv> {
    let mut vars = BTreeMap::new();
    for name in store.list_names("default")? {
        if let Some(v) = store.get("default", &name)? {
            vars.insert(name, v);
        }
    }
    if let Some(p) = profile {
        if !is_valid_profile_slug(p) {
            return Err(ShhError::InvalidProfile(p.into()));
        }
        if p != "default" {
            for name in store.list_names(p)? {
                if let Some(v) = store.get(p, &name)? {
                    vars.insert(name, v);
                }
            }
        }
    }
    Ok(ProfileEnv { vars })
}

pub fn resolve_with_sources(
    store: &dyn SecretStore,
    profile: &str,
) -> Result<Vec<(String, Source)>> {
    if !is_valid_profile_slug(profile) {
        return Err(ShhError::InvalidProfile(profile.into()));
    }
    let default_names: BTreeSet<String> = store.list_names("default")?.into_iter().collect();
    let profile_names: BTreeSet<String> = if profile == "default" {
        BTreeSet::new()
    } else {
        store.list_names(profile)?.into_iter().collect()
    };

    let mut all: BTreeSet<String> = BTreeSet::new();
    all.extend(default_names.iter().cloned());
    all.extend(profile_names.iter().cloned());

    let mut out = Vec::with_capacity(all.len());
    for name in all {
        let in_default = default_names.contains(&name);
        let in_profile = profile_names.contains(&name);
        let src = match (in_default, in_profile) {
            (true, true) => Source::Override,
            (false, true) => Source::Profile,
            (true, false) => Source::Default,
            (false, false) => unreachable!(),
        };
        out.push((name, src));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::memory::MemoryStore;

    #[test]
    fn slug_accepts_work() {
        assert!(is_valid_profile_slug("work"));
    }

    #[test]
    fn slug_accepts_default() {
        assert!(is_valid_profile_slug("default"));
    }

    #[test]
    fn slug_accepts_reserved_doctor() {
        assert!(is_valid_profile_slug("__shh_doctor__"));
    }

    #[test]
    fn slug_accepts_reserved_smoke() {
        assert!(is_valid_profile_slug("__shh_smoke__"));
    }

    #[test]
    fn slug_rejects_colon() {
        assert!(!is_valid_profile_slug("shh:work"));
    }

    #[test]
    fn slug_rejects_empty() {
        assert!(!is_valid_profile_slug(""));
    }

    #[test]
    fn slug_rejects_slash() {
        assert!(!is_valid_profile_slug("a/b"));
    }

    #[test]
    fn slug_rejects_space() {
        assert!(!is_valid_profile_slug("a b"));
    }

    #[test]
    fn resolve_default_only() {
        let s = MemoryStore::new();
        s.set("default", "FOO", "1").unwrap();
        s.set("default", "BAR", "2").unwrap();
        let env = resolve(&s, None).unwrap();
        assert_eq!(env.vars.get("FOO"), Some(&"1".to_string()));
        assert_eq!(env.vars.get("BAR"), Some(&"2".to_string()));
        assert_eq!(env.vars.len(), 2);
    }

    #[test]
    fn resolve_profile_only_no_defaults() {
        let s = MemoryStore::new();
        s.set("work", "API", "x").unwrap();
        let env = resolve(&s, Some("work")).unwrap();
        assert_eq!(env.vars.get("API"), Some(&"x".to_string()));
        assert_eq!(env.vars.len(), 1);
    }

    #[test]
    fn resolve_overlay() {
        let s = MemoryStore::new();
        s.set("default", "FOO", "d").unwrap();
        s.set("default", "SHARED", "d").unwrap();
        s.set("work", "SHARED", "w").unwrap();
        s.set("work", "BAR", "w").unwrap();
        let env = resolve(&s, Some("work")).unwrap();
        assert_eq!(env.vars.get("FOO"), Some(&"d".to_string()));
        assert_eq!(env.vars.get("SHARED"), Some(&"w".to_string()));
        assert_eq!(env.vars.get("BAR"), Some(&"w".to_string()));
        assert_eq!(env.vars.len(), 3);
    }

    #[test]
    fn resolve_invalid_slug() {
        let s = MemoryStore::new();
        let err = resolve(&s, Some("bad:slug")).unwrap_err();
        assert!(matches!(err, ShhError::InvalidProfile(_)));
    }

    #[test]
    fn sources_distinct_shared_default_only() {
        let s = MemoryStore::new();
        s.set("default", "ONLY_D", "1").unwrap();
        s.set("default", "SHARED", "d").unwrap();
        s.set("work", "SHARED", "w").unwrap();
        s.set("work", "ONLY_P", "p").unwrap();
        let v = resolve_with_sources(&s, "work").unwrap();
        let map: BTreeMap<String, Source> = v.into_iter().collect();
        assert_eq!(map.get("ONLY_D"), Some(&Source::Default));
        assert_eq!(map.get("ONLY_P"), Some(&Source::Profile));
        assert_eq!(map.get("SHARED"), Some(&Source::Override));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn sources_invalid_slug() {
        let s = MemoryStore::new();
        let err = resolve_with_sources(&s, "bad:slug").unwrap_err();
        assert!(matches!(err, ShhError::InvalidProfile(_)));
    }
}
