# Chapter D — Keychain adapter

**Type:** conscious
**Depends on:** A

## Executive summary

Real macOS Keychain implementation of `SecretStore` using `security-framework` 3.x. Honors `SHH_KEYCHAIN` (`file` | `data-protection`, default `file`) and `SHH_SERVICE_PREFIX` (default `shh`). Items are stored as generic password items with `service = <prefix>:<profile>`, `account = <NAME>`, `password = <value>`.

This is conscious because:
- Two backends with different APIs (legacy `SecKeychain*` vs `kSecUseDataProtectionKeychain`).
- `ls` and `profiles` MUST use attribute-only enumeration that does not trigger Keychain permission prompts (PRD acceptance criterion).
- Mapping `errSec*` codes to clean `ShhError` variants (`NotFound`, `KeychainDenied`, `Keychain`) requires care.

## Files touched

- `src/store/keychain.rs` (create)
- `src/store/mod.rs` (extend to re-export `KeychainStore` on macOS)

## Success criteria

- `KeychainStore::from_env()` constructs the active backend by reading `SHH_KEYCHAIN` and `SHH_SERVICE_PREFIX` once.
- `set/get/delete` correctly round-trip values for both backends in a manual smoke run against a reserved profile.
- `list_names(profile)` and `list_profiles()` use `SecItemCopyMatching` with `kSecReturnAttributes = true` and `kSecReturnData = false` — verified by code review of the call sites: no `kSecReturnData = true` path is taken from these two functions.
- `list_profiles` enumerates all `kSecClassGenericPassword` items, filters service strings starting with `<prefix>:`, extracts and dedups the suffix.
- `update` semantics: `set` on an existing `(service, account)` updates the password — does not duplicate. For the helper path, verify `set_generic_password_options` preserves the documented create-or-update behavior in both file and data-protection modes.
- `delete` returns `Ok(false)` when nothing matched (not `Err`), so handler can map to non-zero exit per PRD `rm`.
- `errSecUserCanceled` and `errSecAuthFailed` map to `ShhError::KeychainDenied`; `errSecItemNotFound` maps to `Ok(None)`/`Ok(false)`/empty list as appropriate; everything else maps to `ShhError::Keychain(string)`.
- No panics on any error path.
- Reserved profile names `__shh_doctor__` and `__shh_smoke__` are valid via `is_valid_profile_slug` (verified — the underscore-only chars pass slug rules).

## Phases

### D.1 — Service prefix + mode resolution

- **Goal:** Read env once, expose `KeychainStore { prefix: String, mode: Mode }`.
- **Files & changes:** `src/store/keychain.rs`.
- **Code:**
  ```rust
  pub enum Mode { File, DataProtection }

  pub struct KeychainStore {
      prefix: String,
      mode: Mode,
  }

  impl KeychainStore {
      pub fn from_env() -> Self {
          let prefix = std::env::var("SHH_SERVICE_PREFIX").unwrap_or_else(|_| "shh".into());
          let mode = match std::env::var("SHH_KEYCHAIN").as_deref() {
              Ok("data-protection") => Mode::DataProtection,
              _ => Mode::File,
          };
          Self { prefix, mode }
      }

      fn service(&self, profile: &str) -> String { format!("{}:{}", self.prefix, profile) }
  }
  ```

### D.2 — `set` / `get` / `delete`

- **Goal:** Implement the three value operations using `security-framework`.
- **Files & changes:** continue in `src/store/keychain.rs`.
- **For file mode:** use `security_framework::passwords::{set_generic_password, get_generic_password, delete_generic_password}`. These convert macOS errors to `security_framework::base::Error` — match on `code()` against `errSecItemNotFound (-25300)`, `errSecUserCanceled (-128)`, `errSecAuthFailed (-25293)`.
- **For data-protection mode:** use `security_framework::passwords::PasswordOptions::new_generic_password(...)`, call `use_protected_keychain()`, then route through `set_generic_password_options`, `generic_password`, and `delete_generic_password_options`. These are the documented 3.x helpers for generic passwords in the data-protection keychain. Do not use a nonexistent `set_use_data_protection_keychain(true)` method.
- **Code (file mode, get):**
  ```rust
  fn get_file(&self, profile: &str, name: &str) -> Result<Option<String>> {
      use security_framework::passwords::get_generic_password;
      match get_generic_password(&self.service(profile), name) {
          Ok(bytes) => Ok(Some(String::from_utf8(bytes).map_err(|e| ShhError::Keychain(e.to_string()))?)),
          Err(e) if e.code() == -25300 => Ok(None),
          Err(e) if matches!(e.code(), -128 | -25293) => Err(ShhError::KeychainDenied),
          Err(e) => Err(ShhError::Keychain(e.to_string())),
      }
  }
  ```
- Tests: cannot unit-test against real Keychain in CI. Provide a conditional integration test gated on `SHH_RUN_KEYCHAIN_TESTS=1` that writes/reads/deletes under `__shh_smoke__`.

### D.3 — Attribute-only enumeration for `list_names` and `list_profiles`

- **Goal:** Implement without ever requesting `kSecReturnData`.
- **Files & changes:** continue in `src/store/keychain.rs`.
- **Code (using `security-framework`):**
  ```rust
  fn list_names(&self, profile: &str) -> Result<Vec<String>> {
      use security_framework::item::{ItemClass, ItemSearchOptions, SearchResult};
      let want_service = self.service(profile);
      let mut opts = ItemSearchOptions::new();
      opts.class(ItemClass::generic_password()).load_attributes(true).limit(i32::MAX as i64);
      if matches!(self.mode, Mode::DataProtection) {
          opts.ignore_legacy_keychains();
      }
      let results = match opts.search() {
          Ok(r) => r,
          Err(e) if e.code() == -25300 => return Ok(vec![]),
          Err(e) => return Err(map_err(e)),
      };
      let mut names = Vec::new();
      for r in results {
          if let SearchResult::Dict(d) = r {
              // d contains `svce` and `acct` attribute keys
              let svc = d.get("svce").and_then(|v| v.as_string());
              let acct = d.get("acct").and_then(|v| v.as_string());
              if let (Some(s), Some(a)) = (svc, acct) {
                  if s == want_service { names.push(a.into()); }
              }
          }
      }
      names.sort(); names.dedup();
      Ok(names)
  }
  ```
  - **Critical:** verify by compiling against `security-framework` 3.x before writing command handlers. `ItemSearchOptions::load_attributes` and `ignore_legacy_keychains` exist in current 3.x docs; `set_use_data_protection_keychain` does not.
  - `list_profiles` is the same shape but filters by service-prefix `<prefix>:` and extracts the suffix.
- Both functions MUST NOT call any API that asks for the password data. Adding such a call would trigger a Keychain prompt and break the PRD acceptance criteria for `ls` and `profiles`.

### D.4 — Error mapping helper

- **Goal:** One `map_err` so the four operations classify denials and not-found consistently.
- **Files & changes:** continue in `src/store/keychain.rs`.
- **Code:**
  ```rust
  fn map_err(e: security_framework::base::Error) -> ShhError {
      match e.code() {
          -25300 => ShhError::NotFound,
          -128 | -25293 => ShhError::KeychainDenied,
          _ => ShhError::Keychain(e.to_string()),
      }
  }
  ```
