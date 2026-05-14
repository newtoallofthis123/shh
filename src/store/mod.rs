use crate::error::Result;

pub trait SecretStore: Send + Sync {
    fn set(&self, profile: &str, name: &str, value: &str) -> Result<()>;
    fn get(&self, profile: &str, name: &str) -> Result<Option<String>>;
    fn delete(&self, profile: &str, name: &str) -> Result<bool>;
    fn list_names(&self, profile: &str) -> Result<Vec<String>>;
    fn list_profiles(&self) -> Result<Vec<String>>;
}

pub mod memory;

#[cfg(target_os = "macos")]
pub mod keychain;
#[cfg(target_os = "macos")]
pub use keychain::KeychainStore;
