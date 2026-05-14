# Chapter C — Dotenv parser & selector

**Type:** conscious
**Depends on:** A

## Executive summary

Implement the precise dotenv subset declared in the PRD and the selection logic used by `shh load`. Pure module — no IO of the secret store, no terminal interaction. Values must never escape to logs.

Parser tradeoffs require judgment around edge cases (quoting, line continuation, interpolation order). This chapter is conscious because subtle parser bugs would corrupt secret import silently.

## Files touched

- `src/dotenv.rs` (create)
- `src/lib.rs` (extend: `pub mod dotenv;`)

## Success criteria

- Parses each PRD-specified syntax case correctly:
  - blank lines
  - full-line `#` comments
  - trailing `#` outside quotes
  - `NAME=value` and `export NAME=value`
  - single-quoted values: literal, no escape, no interpolation
  - double-quoted values: support escapes `\n`, `\t`, `\r`, `\\`, `\"` and `${VAR}` interpolation against previously parsed entries
  - unquoted values: trimmed of surrounding whitespace, `${VAR}` interpolation
  - backslash-newline continuation inside double-quoted values
- Rejects names invalid per `is_valid_env_name` (returns them in a separate `rejected` list rather than panicking).
- Explicitly does **not** evaluate `$(...)`, backticks, or `${VAR:-default}`. Leaves the literal text or returns a parse error — pick the safer behavior (error) so users notice.
- `select(parsed, selector) -> Vec<(String, String)>` implements `--all`, `--only`, `--except` semantics; rejects unknown names referenced by `--only` (warn or error — error).
- 25+ unit tests covering happy paths, each escape, interpolation order, malformed lines (unterminated quote, bad name), the `__shh_doctor__` / `__shh_smoke__` profile reserved names should round-trip if used in a `.env` file (not applicable — profile names don't appear in `.env`; names in `.env` are env identifiers).
- Test that secret values are never written via `Display`/`Debug` of intermediate structs (use `Debug` derives only on types not holding values, or wrap values in a newtype with redacted `Debug`).

## Phases

### C.1 — Parser

- **Goal:** `parse(input: &str) -> ParseOutcome` where outcome has `entries: Vec<(String, String)>`, `rejected: Vec<RejectedEntry>`, `errors: Vec<ParseError>`.
- **Files & changes:** all in `src/dotenv.rs`.
- **Code shape:**
  ```rust
  pub struct ParseOutcome {
      pub entries: Vec<(String, String)>,
      pub rejected: Vec<RejectedEntry>,    // name failed is_valid_env_name
      pub errors: Vec<ParseError>,         // unterminated quote, bad syntax
  }
  pub struct RejectedEntry { pub line: usize, pub name: String, pub reason: String }
  pub struct ParseError { pub line: usize, pub message: String }

  pub fn parse(input: &str) -> ParseOutcome { /* ... */ }
  ```
  - Walk line-by-line, maintaining an `already_parsed: HashMap<&str, &str>` for `${VAR}` interpolation.
  - On `"..."` opener, consume across newlines (backslash-newline continuation) until matching unescaped `"`.
  - On `'...'`, no interpolation, no escapes, no multi-line.
  - Trailing `#` outside quotes terminates the value; trim trailing whitespace before the `#`.
  - Reject `$(...)` and backticks at interpolation time (look ahead during `$` handling) — emit `ParseError`.
  - Reject `${VAR:-default}` — when scanning a `${...}` body, fail if a `:` appears.

### C.2 — Selector

- **Goal:** Apply `--all` / `--only` / `--except` to a parsed entry list. Selector is mode-exclusive (validated at CLI layer too).
- **Files & changes:** `src/dotenv.rs::select`.
- **Code:**
  ```rust
  pub enum Selector<'a> {
      All,
      Only(&'a [String]),
      Except(&'a [String]),
      Interactive(&'a [String]),   // populated by chapter E from the inquire result
  }

  pub fn select<'a>(
      entries: &'a [(String, String)],
      selector: Selector<'_>,
  ) -> Result<Vec<&'a (String, String)>> { /* ... */ }
  ```
  - `Only`: error if a requested name is not in `entries`.
  - `Except`: ignore unknown names silently (PRD doesn't require error — be lenient).
  - `Interactive`: same as `Only` semantics but no error on missing (the names come from the parsed set the user picked from).

### C.3 — Redaction discipline

- **Goal:** Make it hard to accidentally print values.
- **Files & changes:** wrap the value strings in `SecretValue(String)` with a custom `Debug` impl returning `"<redacted>"`. Use this type in `ParseOutcome.entries` and `RejectedEntry` (rejected names should never carry value at all). Provide `as_str(&self) -> &str` for the importer.
- **Code:**
  ```rust
  pub struct SecretValue(String);
  impl SecretValue {
      pub fn as_str(&self) -> &str { &self.0 }
  }
  impl std::fmt::Debug for SecretValue {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          f.write_str("<redacted>")
      }
  }
  ```
  - Tests assert `format!("{:?}", outcome)` contains `"<redacted>"` for every entry value.
