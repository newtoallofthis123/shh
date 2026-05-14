# Chapter B — Core utilities

**Type:** strict
**Depends on:** A

## Executive summary

Pure functions used by every command after the foundation validator exists: profile-slug validation (including reserved slugs), POSIX single-quote escaping, and the `default`-plus-overlay profile resolver. All exhaustively unit-tested.

## Files touched

- `src/env.rs` (extend)
- `src/profile.rs` (create)
- `src/lib.rs` (extend with `pub mod env; pub mod profile;`)

## Success criteria

- `is_valid_profile_slug` accepts ASCII letters/digits/`.`/`_`/`-`. Rejects names containing `:` (PRD constraint), empty strings, and pure whitespace. Accepts reserved `__shh_doctor__` and `__shh_smoke__`.
- `posix_quote` wraps any value in single quotes and escapes embedded `'` as `'\''`. Round-trips through `sh -c "echo $(printf %s "$quoted")"`.
- `resolve_profile(store, Some("work"))` returns the union of `default` keys and `work` keys, with `work` overriding `default`.
- `resolve_profile(store, None)` returns only the `default` map.
- `resolve_with_sources(store, "work")` returns each name annotated with `Source::Default`, `Source::Profile`, or `Source::Override` for use by `shh ls -p work`.
- 100% of profile validation, quoting, and resolver behaviors covered by tests using `MemoryStore`.

## Phases

### B.1 — Profile slug validation

- **Goal:** Single function used everywhere a `--profile` is consumed plus `set`, `rm`.
- **Files & changes:** `src/profile.rs::is_valid_profile_slug(p: &str) -> bool`.
- **Code:**
  ```rust
  pub fn is_valid_profile_slug(p: &str) -> bool {
      if p.is_empty() { return false; }
      p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
  }
  ```
  - Note: ASCII alphanumeric + `.`, `_`, `-` only — explicitly forbids `:` per PRD (Keychain prefix delimiter).
  - Tests: `"work"`, `"default"`, `"__shh_doctor__"`, `"shh:work"` (reject — colon), `""`, `"a/b"`, `"a b"`.

### B.2 — POSIX shell quoting

- **Goal:** Safe value emission for `export` and `unset`.
- **Files & changes:** `src/env.rs::posix_quote(value: &str) -> String`.
- **Code:**
  ```rust
  pub fn posix_quote(value: &str) -> String {
      let mut out = String::with_capacity(value.len() + 2);
      out.push('\'');
      for c in value.chars() {
          if c == '\'' { out.push_str("'\\''"); } else { out.push(c); }
      }
      out.push('\'');
      out
  }
  ```
  - Tests: empty, plain text, value containing `'`, value containing `$VAR`, `\n`, `"`, backtick, `\`.
  - Optional shell-round-trip test (`Command::new("sh").args(["-c", &format!("printf %s {}", q)])`) — skip if test env can't shell out.

### B.3 — Profile resolver

- **Goal:** Compute the effective env map for a command and (separately) the annotated view used by `ls -p`.
- **Files & changes:**
  - `src/profile.rs::ProfileEnv` (struct with `BTreeMap<String, String>`).
  - `src/profile.rs::resolve(store: &dyn SecretStore, profile: Option<&str>) -> Result<ProfileEnv>`.
  - `src/profile.rs::Source` enum: `Default`, `Profile`, `Override`.
  - `src/profile.rs::resolve_with_sources(store: &dyn SecretStore, profile: &str) -> Result<Vec<(String, Source)>>`.
- **Code:**
  ```rust
  pub struct ProfileEnv { pub vars: std::collections::BTreeMap<String, String> }

  pub fn resolve(store: &dyn SecretStore, profile: Option<&str>) -> Result<ProfileEnv> {
      let mut vars = BTreeMap::new();
      for name in store.list_names("default")? {
          if let Some(v) = store.get("default", &name)? { vars.insert(name, v); }
      }
      if let Some(p) = profile {
          if !is_valid_profile_slug(p) { return Err(ShhError::InvalidProfile(p.into())); }
          if p != "default" {
              for name in store.list_names(p)? {
                  if let Some(v) = store.get(p, &name)? { vars.insert(name, v); }
              }
          }
      }
      Ok(ProfileEnv { vars })
  }
  ```
  - `resolve_with_sources` collects sets of names from `default` and `<profile>`, deduplicates, classifies as `Default`/`Profile`/`Override`. No value fetch (so `ls -p` does not trigger Keychain prompts — see chapter D for backing claim).
  - Tests against `MemoryStore`:
    - default-only resolution
    - profile-only resolution (no `default` keys present)
    - overlay where `default` and `work` share keys
    - sources annotation: distinct, shared, and only-default
    - invalid profile slug rejected before any store call
