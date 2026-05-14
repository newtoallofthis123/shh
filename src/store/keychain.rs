//! macOS Keychain backend for `SecretStore`.
//!
//! Items are stored as generic password items where:
//! - `service = "<prefix>:<profile>"`
//! - `account = "<NAME>"`
//! - `password = "<value>"`
//!
//! Two backends are supported:
//! - `Mode::File`: legacy file-based keychain (default; uses `security_framework`'s
//!   plain helpers, which target the user's login keychain on macOS).
//! - `Mode::DataProtection`: data-protection keychain
//!   (`kSecUseDataProtectionKeychain`).
//!
//! The `list_names` and `list_profiles` paths MUST be attribute-only — they
//! never request `kSecReturnData`, so they do not trigger Keychain permission
//! prompts.

use security_framework::base::Error as SfError;
use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
use security_framework::passwords::{
    delete_generic_password, delete_generic_password_options, generic_password,
    get_generic_password, set_generic_password, set_generic_password_options, PasswordOptions,
};

use crate::error::{Result, ShhError};
use crate::store::SecretStore;

// `errSec*` constants (avoid pulling them via the -sys crate to keep deps lean).
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SEC_USER_CANCELED: i32 = -128;
const ERR_SEC_AUTH_FAILED: i32 = -25293;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    File,
    DataProtection,
}

pub struct KeychainStore {
    prefix: String,
    mode: Mode,
}

impl KeychainStore {
    /// Construct a `KeychainStore` from `SHH_SERVICE_PREFIX` and `SHH_KEYCHAIN`
    /// environment variables.
    ///
    /// - `SHH_SERVICE_PREFIX` defaults to `"shh"`.
    /// - `SHH_KEYCHAIN` is `"file"` (default) or `"data-protection"`.
    pub fn from_env() -> Self {
        let prefix = std::env::var("SHH_SERVICE_PREFIX").unwrap_or_else(|_| "shh".into());
        let mode = match std::env::var("SHH_KEYCHAIN").as_deref() {
            Ok("data-protection") => Mode::DataProtection,
            _ => Mode::File,
        };
        Self { prefix, mode }
    }

    pub fn new(prefix: impl Into<String>, mode: Mode) -> Self {
        Self { prefix: prefix.into(), mode }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn service(&self, profile: &str) -> String {
        format!("{}:{}", self.prefix, profile)
    }

    fn service_prefix(&self) -> String {
        format!("{}:", self.prefix)
    }

    fn make_options(&self, profile: &str, name: &str) -> PasswordOptions {
        let mut opts = PasswordOptions::new_generic_password(&self.service(profile), name);
        if matches!(self.mode, Mode::DataProtection) {
            opts.use_protected_keychain();
        }
        opts
    }
}

fn map_err(e: SfError) -> ShhError {
    match e.code() {
        ERR_SEC_ITEM_NOT_FOUND => ShhError::NotFound,
        ERR_SEC_USER_CANCELED | ERR_SEC_AUTH_FAILED => ShhError::KeychainDenied,
        _ => ShhError::Keychain(e.to_string()),
    }
}

impl SecretStore for KeychainStore {
    fn set(&self, profile: &str, name: &str, value: &str) -> Result<()> {
        let bytes = value.as_bytes();
        let res = match self.mode {
            Mode::File => set_generic_password(&self.service(profile), name, bytes),
            Mode::DataProtection => {
                set_generic_password_options(bytes, self.make_options(profile, name))
            }
        };
        res.map_err(map_err)
    }

    fn get(&self, profile: &str, name: &str) -> Result<Option<String>> {
        let res = match self.mode {
            Mode::File => get_generic_password(&self.service(profile), name),
            Mode::DataProtection => generic_password(self.make_options(profile, name)),
        };
        match res {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => Ok(Some(s)),
                Err(e) => Err(ShhError::Keychain(e.to_string())),
            },
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn delete(&self, profile: &str, name: &str) -> Result<bool> {
        let res = match self.mode {
            Mode::File => delete_generic_password(&self.service(profile), name),
            Mode::DataProtection => {
                delete_generic_password_options(self.make_options(profile, name))
            }
        };
        match res {
            Ok(()) => Ok(true),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(false),
            Err(e) => Err(map_err(e)),
        }
    }

    fn list_names(&self, profile: &str) -> Result<Vec<String>> {
        let want_service = self.service(profile);
        let dicts = search_generic_password_attrs(self.mode)?;
        let mut names = Vec::new();
        for d in dicts {
            let svc = d.get("svce").map(String::as_str);
            let acct = d.get("acct").map(String::as_str);
            if let (Some(s), Some(a)) = (svc, acct) {
                if s == want_service {
                    names.push(a.to_string());
                }
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    fn list_profiles(&self) -> Result<Vec<String>> {
        let prefix = self.service_prefix();
        let dicts = search_generic_password_attrs(self.mode)?;
        let mut profiles = Vec::new();
        for d in dicts {
            if let Some(svc) = d.get("svce") {
                if let Some(rest) = svc.strip_prefix(&prefix) {
                    profiles.push(rest.to_string());
                }
            }
        }
        profiles.sort();
        profiles.dedup();
        Ok(profiles)
    }
}

/// Attribute-only enumeration of generic password items. Never asks for the
/// password data — the call site must not request `kSecReturnData`.
fn search_generic_password_attrs(
    mode: Mode,
) -> Result<Vec<std::collections::HashMap<String, String>>> {
    let mut opts = ItemSearchOptions::new();
    opts.class(ItemClass::generic_password())
        .load_attributes(true)
        .limit(Limit::All);
    if matches!(mode, Mode::DataProtection) {
        opts.ignore_legacy_keychains();
    }
    let results = match opts.search() {
        Ok(r) => r,
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(vec![]),
        Err(e) => return Err(map_err(e)),
    };
    let mut out = Vec::new();
    for r in results {
        if let SearchResult::Dict(_) = r {
            if let Some(d) = r.simplify_dict() {
                out.push(d);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_string_format() {
        let s = KeychainStore::new("shh", Mode::File);
        assert_eq!(s.service("default"), "shh:default");
        assert_eq!(s.service_prefix(), "shh:");
    }

    #[test]
    fn service_string_respects_custom_prefix() {
        let s = KeychainStore::new("acme", Mode::DataProtection);
        assert_eq!(s.service("staging"), "acme:staging");
        assert_eq!(s.service_prefix(), "acme:");
        assert_eq!(s.mode(), Mode::DataProtection);
    }

    #[test]
    fn from_env_defaults() {
        // We don't mutate process env here; just make sure construction works
        // and falls back to expected defaults when the vars are absent. This is
        // racy across tests that mutate env, so we only assert the shape.
        let s = KeychainStore::from_env();
        assert!(!s.prefix().is_empty());
    }
}
