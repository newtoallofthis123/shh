//! Dotenv subset parser and import selector.
//!
//! Implements precisely the dotenv syntax declared in the PRD: blank lines,
//! `#` comments (full-line and trailing-outside-quotes), `NAME=value` and
//! `export NAME=value`, single-quoted literal values, double-quoted values
//! with `\n \t \r \\ \"` escapes plus `${VAR}` interpolation against entries
//! parsed earlier in the same file, unquoted values trimmed of surrounding
//! whitespace with `${VAR}` interpolation, and backslash-newline continuation
//! inside double-quoted values.
//!
//! Explicitly rejects (with a parse error, not silent passthrough): command
//! substitution `$(...)`, backtick substitution, and default-value expansion
//! `${VAR:-default}`.
//!
//! Secret values are wrapped in [`SecretValue`] whose `Debug` impl redacts
//! the contents. Parser errors and rejected entries never carry the value.

use std::collections::HashMap;

use crate::env::is_valid_env_name;

/// Newtype around a secret value with a redacting `Debug` impl.
#[derive(Clone)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

#[derive(Debug, Clone)]
pub struct RejectedEntry {
    pub line: usize,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct ParseOutcome {
    pub entries: Vec<(String, SecretValue)>,
    pub rejected: Vec<RejectedEntry>,
    pub errors: Vec<ParseError>,
}

/// Parse a dotenv file body. Never panics on malformed input; collects errors.
pub fn parse(input: &str) -> ParseOutcome {
    let mut outcome = ParseOutcome::default();
    let mut interpolation: HashMap<String, String> = HashMap::new();

    let lines: Vec<&str> = input.split('\n').collect();
    let mut idx = 0;
    while idx < lines.len() {
        let line_no = idx + 1;
        let raw = lines[idx];
        let trimmed = raw.trim_start();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            idx += 1;
            continue;
        }

        // Strip optional `export ` prefix.
        let after_export = if let Some(rest) = strip_export_prefix(trimmed) {
            rest
        } else {
            trimmed
        };

        // Split on first `=`.
        let Some(eq_pos) = after_export.find('=') else {
            outcome.errors.push(ParseError {
                line: line_no,
                message: "missing '=' in assignment".to_string(),
            });
            idx += 1;
            continue;
        };

        let name = after_export[..eq_pos].trim().to_string();
        let value_region = &after_export[eq_pos + 1..];

        // Parse value, possibly consuming additional lines for "..." continuation.
        let value_result = parse_value(value_region, &lines, &mut idx, line_no, &interpolation);

        match value_result {
            Ok(value) => {
                if !is_valid_env_name(&name) {
                    outcome.rejected.push(RejectedEntry {
                        line: line_no,
                        name,
                        reason: "invalid env name".to_string(),
                    });
                } else {
                    interpolation.insert(name.clone(), value.clone());
                    outcome.entries.push((name, SecretValue::new(value)));
                }
            }
            Err(err) => {
                outcome.errors.push(err);
            }
        }

        idx += 1;
    }

    outcome
}

fn strip_export_prefix(s: &str) -> Option<&str> {
    let rest = s.strip_prefix("export")?;
    // Require at least one whitespace char after `export`.
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if c.is_whitespace() => Some(rest.trim_start()),
        _ => None,
    }
}

/// Parse the right-hand side of an assignment.
///
/// `value_region` is the substring after the first `=` on the starting line.
/// `idx` is advanced past any continuation lines consumed for `"..."` values.
fn parse_value(
    value_region: &str,
    lines: &[&str],
    idx: &mut usize,
    start_line: usize,
    interpolation: &HashMap<String, String>,
) -> Result<String, ParseError> {
    let leading_trimmed = value_region.trim_start();

    if leading_trimmed.is_empty() {
        return Ok(String::new());
    }

    let first = leading_trimmed.as_bytes()[0];
    match first {
        b'\'' => parse_single_quoted(&leading_trimmed[1..], start_line),
        b'"' => parse_double_quoted(&leading_trimmed[1..], lines, idx, start_line, interpolation),
        _ => parse_unquoted(leading_trimmed, start_line, interpolation),
    }
}

fn parse_single_quoted(rest: &str, line: usize) -> Result<String, ParseError> {
    // Single-quoted: literal until next '. No escapes, no interpolation, no
    // multi-line.
    let Some(end) = rest.find('\'') else {
        return Err(ParseError {
            line,
            message: "unterminated single-quoted value".to_string(),
        });
    };
    let value = rest[..end].to_string();
    let after = rest[end + 1..].trim_start();
    if !after.is_empty() && !after.starts_with('#') {
        return Err(ParseError {
            line,
            message: "unexpected text after closing single quote".to_string(),
        });
    }
    Ok(value)
}

fn parse_double_quoted(
    initial_rest: &str,
    lines: &[&str],
    idx: &mut usize,
    start_line: usize,
    interpolation: &HashMap<String, String>,
) -> Result<String, ParseError> {
    // Build a buffer; when we hit end-of-line without a closing quote, advance
    // to the next line. Backslash-newline is a continuation that drops the
    // newline.
    let mut buf = String::new();
    let mut current = initial_rest;
    let mut current_line = start_line;

    loop {
        let mut chars = current.char_indices().peekable();
        let mut closed = false;
        let mut consumed_to: Option<usize> = None;

        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    closed = true;
                    consumed_to = Some(i + c.len_utf8());
                    break;
                }
                '\\' => {
                    let Some((_, esc)) = chars.next() else {
                        // Backslash at EOL inside "..." => line continuation.
                        // Drop the backslash, do not append newline.
                        consumed_to = Some(current.len());
                        break;
                    };
                    match esc {
                        'n' => buf.push('\n'),
                        't' => buf.push('\t'),
                        'r' => buf.push('\r'),
                        '\\' => buf.push('\\'),
                        '"' => buf.push('"'),
                        other => {
                            return Err(ParseError {
                                line: current_line,
                                message: format!("unsupported escape sequence \\{other}"),
                            });
                        }
                    }
                }
                '$' => {
                    let next = chars.peek().map(|(_, c)| *c);
                    match next {
                        Some('{') => {
                            // consume '{'
                            chars.next();
                            // Collect until matching '}'.
                            let mut name = String::new();
                            let mut found_close = false;
                            for (_, nc) in chars.by_ref() {
                                if nc == '}' {
                                    found_close = true;
                                    break;
                                }
                                if nc == ':' {
                                    return Err(ParseError {
                                        line: current_line,
                                        message:
                                            "default-value expansion '${VAR:-...}' is not supported"
                                                .to_string(),
                                    });
                                }
                                name.push(nc);
                            }
                            if !found_close {
                                return Err(ParseError {
                                    line: current_line,
                                    message: "unterminated '${' interpolation".to_string(),
                                });
                            }
                            let resolved = interpolation.get(&name).cloned().unwrap_or_default();
                            buf.push_str(&resolved);
                        }
                        Some('(') => {
                            return Err(ParseError {
                                line: current_line,
                                message: "command substitution '$(...)' is not supported"
                                    .to_string(),
                            });
                        }
                        _ => {
                            // Lone `$` — keep literal.
                            buf.push('$');
                        }
                    }
                }
                '`' => {
                    return Err(ParseError {
                        line: current_line,
                        message: "backtick substitution is not supported".to_string(),
                    });
                }
                _ => buf.push(c),
            }
        }

        if closed {
            // Validate trailing text after closing quote.
            let after = &current[consumed_to.unwrap_or(current.len())..];
            let after_trim = after.trim_start();
            if !after_trim.is_empty() && !after_trim.starts_with('#') {
                return Err(ParseError {
                    line: current_line,
                    message: "unexpected text after closing double quote".to_string(),
                });
            }
            return Ok(buf);
        }

        // Did not close on this line — need a continuation line.
        if *idx + 1 >= lines.len() {
            return Err(ParseError {
                line: start_line,
                message: "unterminated double-quoted value".to_string(),
            });
        }
        // If we did not end with a backslash continuation, we still allow
        // raw newline inside "..." (common dotenv behavior). Append \n only
        // if the previous line did not consume itself entirely via backslash
        // continuation.
        let was_continuation = consumed_to == Some(current.len()) && current.ends_with('\\');
        if !was_continuation {
            buf.push('\n');
        }
        *idx += 1;
        current_line = *idx + 1;
        current = lines[*idx];
    }
}

fn parse_unquoted(
    region: &str,
    line: usize,
    interpolation: &HashMap<String, String>,
) -> Result<String, ParseError> {
    // Walk to terminating `#` (preceded by whitespace or at start) outside of
    // any quoting. Unquoted values do not carry quotes mid-string.
    let mut buf = String::new();
    let mut chars = region.char_indices().peekable();
    let mut last_kept_byte: usize = 0;

    while let Some((i, c)) = chars.next() {
        if c == '#' {
            // A `#` terminates the value if it is at the very start (already
            // handled upstream as a comment line) or preceded by whitespace.
            // Since `region` has been left-trimmed, `i == 0` cannot happen
            // here unless the value is genuinely empty — and that was caught
            // in `parse_value`. Check the previous char in `region`.
            let prev_is_ws = region[..i]
                .chars()
                .next_back()
                .map(|p| p.is_whitespace())
                .unwrap_or(true);
            if prev_is_ws {
                break;
            }
            buf.push('#');
            last_kept_byte = i + 1;
            continue;
        }
        if c == '$' {
            let next = chars.peek().map(|(_, c)| *c);
            match next {
                Some('{') => {
                    chars.next();
                    let mut name = String::new();
                    let mut found_close = false;
                    for (_, nc) in chars.by_ref() {
                        if nc == '}' {
                            found_close = true;
                            break;
                        }
                        if nc == ':' {
                            return Err(ParseError {
                                line,
                                message: "default-value expansion '${VAR:-...}' is not supported"
                                    .to_string(),
                            });
                        }
                        name.push(nc);
                    }
                    if !found_close {
                        return Err(ParseError {
                            line,
                            message: "unterminated '${' interpolation".to_string(),
                        });
                    }
                    let resolved = interpolation.get(&name).cloned().unwrap_or_default();
                    buf.push_str(&resolved);
                    last_kept_byte = i + 2; // approximate; only used for trim below
                    continue;
                }
                Some('(') => {
                    return Err(ParseError {
                        line,
                        message: "command substitution '$(...)' is not supported".to_string(),
                    });
                }
                _ => {
                    buf.push('$');
                    last_kept_byte = i + 1;
                    continue;
                }
            }
        }
        if c == '`' {
            return Err(ParseError {
                line,
                message: "backtick substitution is not supported".to_string(),
            });
        }
        buf.push(c);
        last_kept_byte = i + c.len_utf8();
    }

    let _ = last_kept_byte; // reserved for future diagnostics
    Ok(buf.trim_end().to_string())
}

// ---------------------------------------------------------------------------
// Selector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Selector<'a> {
    All,
    Only(&'a [String]),
    Except(&'a [String]),
    /// Same as `Only` but missing names are tolerated silently (the names came
    /// from the parsed set the user just picked from interactively).
    Interactive(&'a [String]),
}

#[derive(Debug, Clone)]
pub struct SelectionError {
    pub unknown: Vec<String>,
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown name(s) requested via --only: {}",
            self.unknown.join(", ")
        )
    }
}

impl std::error::Error for SelectionError {}

/// Apply a selector to a parsed entry list.
pub fn select<'a>(
    entries: &'a [(String, SecretValue)],
    selector: Selector<'_>,
) -> Result<Vec<&'a (String, SecretValue)>, SelectionError> {
    match selector {
        Selector::All => Ok(entries.iter().collect()),
        Selector::Only(names) => {
            let known: std::collections::HashSet<&str> =
                entries.iter().map(|(n, _)| n.as_str()).collect();
            let unknown: Vec<String> = names
                .iter()
                .filter(|n| !known.contains(n.as_str()))
                .cloned()
                .collect();
            if !unknown.is_empty() {
                return Err(SelectionError { unknown });
            }
            let wanted: std::collections::HashSet<&str> =
                names.iter().map(|s| s.as_str()).collect();
            Ok(entries
                .iter()
                .filter(|(n, _)| wanted.contains(n.as_str()))
                .collect())
        }
        Selector::Except(names) => {
            let excluded: std::collections::HashSet<&str> =
                names.iter().map(|s| s.as_str()).collect();
            Ok(entries
                .iter()
                .filter(|(n, _)| !excluded.contains(n.as_str()))
                .collect())
        }
        Selector::Interactive(names) => {
            let wanted: std::collections::HashSet<&str> =
                names.iter().map(|s| s.as_str()).collect();
            Ok(entries
                .iter()
                .filter(|(n, _)| wanted.contains(n.as_str()))
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries_map(o: &ParseOutcome) -> std::collections::HashMap<String, String> {
        o.entries
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().to_string()))
            .collect()
    }

    // ----- Happy paths -----

    #[test]
    fn parses_simple_assignment() {
        let o = parse("FOO=bar\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "bar");
    }

    #[test]
    fn parses_export_prefix() {
        let o = parse("export FOO=bar\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "bar");
    }

    #[test]
    fn ignores_blank_lines_and_full_line_comments() {
        let o = parse("\n# hello\n\nFOO=bar\n# trailing\n");
        assert!(o.errors.is_empty());
        assert_eq!(o.entries.len(), 1);
    }

    #[test]
    fn trims_unquoted_whitespace() {
        let o = parse("FOO=  bar baz   \n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "bar baz");
    }

    #[test]
    fn empty_value_is_empty_string() {
        let o = parse("FOO=\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "");
    }

    #[test]
    fn trailing_comment_outside_quotes_unquoted() {
        let o = parse("FOO=bar # a comment\n");
        assert!(o.errors.is_empty(), "errors: {:?}", o.errors);
        assert_eq!(entries_map(&o)["FOO"], "bar");
    }

    #[test]
    fn trailing_comment_outside_quotes_double() {
        let o = parse("FOO=\"bar\" # a comment\n");
        assert!(o.errors.is_empty(), "errors: {:?}", o.errors);
        assert_eq!(entries_map(&o)["FOO"], "bar");
    }

    // ----- Single-quoted -----

    #[test]
    fn single_quoted_literal_no_escape() {
        let o = parse("FOO='a\\nb'\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "a\\nb");
    }

    #[test]
    fn single_quoted_no_interpolation() {
        let o = parse("A=hi\nFOO='${A}'\n");
        assert_eq!(entries_map(&o)["FOO"], "${A}");
    }

    #[test]
    fn single_quoted_unterminated_errors() {
        let o = parse("FOO='oops\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("unterminated"));
    }

    // ----- Double-quoted escapes -----

    #[test]
    fn double_quoted_escape_n() {
        let o = parse("FOO=\"a\\nb\"\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["FOO"], "a\nb");
    }

    #[test]
    fn double_quoted_escape_t() {
        let o = parse("FOO=\"a\\tb\"\n");
        assert_eq!(entries_map(&o)["FOO"], "a\tb");
    }

    #[test]
    fn double_quoted_escape_r() {
        let o = parse("FOO=\"a\\rb\"\n");
        assert_eq!(entries_map(&o)["FOO"], "a\rb");
    }

    #[test]
    fn double_quoted_escape_backslash() {
        let o = parse("FOO=\"a\\\\b\"\n");
        assert_eq!(entries_map(&o)["FOO"], "a\\b");
    }

    #[test]
    fn double_quoted_escape_quote() {
        let o = parse("FOO=\"a\\\"b\"\n");
        assert_eq!(entries_map(&o)["FOO"], "a\"b");
    }

    #[test]
    fn double_quoted_unsupported_escape_errors() {
        let o = parse("FOO=\"a\\zb\"\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("unsupported escape"));
    }

    #[test]
    fn double_quoted_unterminated_errors() {
        let o = parse("FOO=\"oops\n");
        assert_eq!(o.errors.len(), 1);
    }

    // ----- Interpolation -----

    #[test]
    fn double_quoted_interpolation_uses_prior_entries() {
        let o = parse("A=hello\nB=\"${A} world\"\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["B"], "hello world");
    }

    #[test]
    fn unquoted_interpolation_uses_prior_entries() {
        let o = parse("A=hello\nB=${A}_world\n");
        assert!(o.errors.is_empty());
        assert_eq!(entries_map(&o)["B"], "hello_world");
    }

    #[test]
    fn interpolation_order_forward_only() {
        // B references A which is defined before; C references D which is later → empty.
        let o = parse("A=x\nB=${A}\nC=${D}\nD=y\n");
        assert_eq!(entries_map(&o)["B"], "x");
        assert_eq!(entries_map(&o)["C"], "");
        assert_eq!(entries_map(&o)["D"], "y");
    }

    // ----- Forbidden constructs -----

    #[test]
    fn rejects_default_expansion() {
        let o = parse("FOO=\"${BAR:-x}\"\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("default-value"));
    }

    #[test]
    fn rejects_command_substitution_double() {
        let o = parse("FOO=\"$(echo hi)\"\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("command substitution"));
    }

    #[test]
    fn rejects_command_substitution_unquoted() {
        let o = parse("FOO=$(echo hi)\n");
        assert_eq!(o.errors.len(), 1);
    }

    #[test]
    fn rejects_backticks_double() {
        let o = parse("FOO=\"`echo hi`\"\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("backtick"));
    }

    // ----- Continuation -----

    #[test]
    fn double_quoted_backslash_newline_continuation() {
        let o = parse("FOO=\"a\\\nb\"\n");
        assert!(o.errors.is_empty(), "errors: {:?}", o.errors);
        assert_eq!(entries_map(&o)["FOO"], "ab");
    }

    // ----- Bad name -----

    #[test]
    fn rejects_invalid_env_name() {
        let o = parse("1FOO=bar\n");
        assert!(o.entries.is_empty());
        assert_eq!(o.rejected.len(), 1);
        assert_eq!(o.rejected[0].name, "1FOO");
    }

    #[test]
    fn missing_equals_is_error() {
        let o = parse("FOOBAR\n");
        assert_eq!(o.errors.len(), 1);
        assert!(o.errors[0].message.contains("missing '='"));
    }

    // ----- Redaction -----

    #[test]
    fn debug_of_outcome_redacts_values() {
        let o = parse("FOO=supersecret\nBAR=alsosecret\n");
        let dbg = format!("{:?}", o);
        assert!(!dbg.contains("supersecret"));
        assert!(!dbg.contains("alsosecret"));
        // every entry's value is rendered as <redacted>
        assert!(dbg.matches("<redacted>").count() >= 2);
    }

    #[test]
    fn rejected_entry_carries_no_value() {
        let o = parse("1FOO=supersecret\n");
        let dbg = format!("{:?}", o.rejected);
        assert!(!dbg.contains("supersecret"));
    }

    // ----- Selector -----

    fn sample_entries() -> Vec<(String, SecretValue)> {
        vec![
            ("A".into(), SecretValue::new("1".into())),
            ("B".into(), SecretValue::new("2".into())),
            ("C".into(), SecretValue::new("3".into())),
        ]
    }

    #[test]
    fn select_all_returns_all() {
        let e = sample_entries();
        let r = select(&e, Selector::All).unwrap();
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn select_only_filters() {
        let e = sample_entries();
        let names = vec!["A".to_string(), "C".to_string()];
        let r = select(&e, Selector::Only(&names)).unwrap();
        assert_eq!(
            r.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["A", "C"]
        );
    }

    #[test]
    fn select_only_errors_on_unknown() {
        let e = sample_entries();
        let names = vec!["A".to_string(), "Z".to_string()];
        let err = select(&e, Selector::Only(&names)).unwrap_err();
        assert_eq!(err.unknown, vec!["Z"]);
    }

    #[test]
    fn select_except_filters_out() {
        let e = sample_entries();
        let names = vec!["B".to_string()];
        let r = select(&e, Selector::Except(&names)).unwrap();
        assert_eq!(
            r.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["A", "C"]
        );
    }

    #[test]
    fn select_except_ignores_unknown_silently() {
        let e = sample_entries();
        let names = vec!["NOPE".to_string()];
        let r = select(&e, Selector::Except(&names)).unwrap();
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn select_interactive_tolerates_unknown() {
        let e = sample_entries();
        let names = vec!["A".to_string(), "ghost".to_string()];
        let r = select(&e, Selector::Interactive(&names)).unwrap();
        assert_eq!(
            r.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["A"]
        );
    }
}
