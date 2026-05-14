pub fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn posix_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for c in value.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_env_name(""));
    }

    #[test]
    fn accepts_simple() {
        assert!(is_valid_env_name("FOO"));
    }

    #[test]
    fn accepts_leading_underscore() {
        assert!(is_valid_env_name("_FOO"));
    }

    #[test]
    fn rejects_leading_digit() {
        assert!(!is_valid_env_name("1FOO"));
    }

    #[test]
    fn rejects_hyphen() {
        assert!(!is_valid_env_name("FOO-BAR"));
    }

    #[test]
    fn rejects_whitespace() {
        assert!(!is_valid_env_name("FOO BAR"));
    }

    #[test]
    fn rejects_equals() {
        assert!(!is_valid_env_name("FOO="));
    }

    #[test]
    fn rejects_unicode_letters() {
        assert!(!is_valid_env_name("FOOÄ"));
    }

    #[test]
    fn quote_empty() {
        assert_eq!(posix_quote(""), "''");
    }

    #[test]
    fn quote_plain() {
        assert_eq!(posix_quote("hello"), "'hello'");
    }

    #[test]
    fn quote_single_quote() {
        assert_eq!(posix_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn quote_dollar_var() {
        assert_eq!(posix_quote("$VAR"), "'$VAR'");
    }

    #[test]
    fn quote_newline() {
        assert_eq!(posix_quote("a\nb"), "'a\nb'");
    }

    #[test]
    fn quote_double_quote() {
        assert_eq!(posix_quote("a\"b"), "'a\"b'");
    }

    #[test]
    fn quote_backtick() {
        assert_eq!(posix_quote("a`b"), "'a`b'");
    }

    #[test]
    fn quote_backslash() {
        assert_eq!(posix_quote("a\\b"), "'a\\b'");
    }

    #[test]
    fn quote_shell_round_trip() {
        use std::process::Command;
        let cases = ["", "hello", "a'b", "$VAR", "a\nb", "a\"b", "a`b", "a\\b"];
        for original in cases {
            let q = posix_quote(original);
            let script = format!("printf %s {}", q);
            let out = Command::new("sh").args(["-c", &script]).output();
            let Ok(out) = out else { continue };
            assert!(out.status.success(), "sh failed for {:?}", original);
            assert_eq!(
                String::from_utf8(out.stdout).unwrap(),
                original,
                "round-trip mismatch for {:?}",
                original
            );
        }
    }
}
