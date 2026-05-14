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
}
