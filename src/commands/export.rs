use std::collections::BTreeMap;

use crate::cli::ExportFormat;
use crate::error::{Result, ShhError};
use crate::profile::{is_valid_profile_slug, resolve};
use crate::store::SecretStore;
use crate::tty::is_stdout_tty;

use super::CommandOutcome;

pub fn run(
    profile: Option<&str>,
    format: Option<ExportFormat>,
    store: &dyn SecretStore,
) -> Result<CommandOutcome> {
    if let Some(p) = profile {
        if !is_valid_profile_slug(p) {
            return Err(ShhError::InvalidProfile(p.to_string()));
        }
    }
    if is_stdout_tty() {
        eprintln!("{}", tty_refusal_message(profile, format));
        return Ok(CommandOutcome::ExitCode(2));
    }
    let env = resolve(store, profile)?;
    print!("{}", render(&env.vars, format)?);
    Ok(CommandOutcome::Success)
}

fn render(vars: &BTreeMap<String, String>, format: Option<ExportFormat>) -> Result<String> {
    match format {
        Some(ExportFormat::Json) => render_json(vars),
        Some(ExportFormat::Dotenv) => render_dotenv(vars),
        None => Ok(render_shell(vars)),
    }
}

fn render_json(vars: &BTreeMap<String, String>) -> Result<String> {
    let mut out = serde_json::to_string_pretty(vars)
        .map_err(|e| ShhError::Keychain(format!("json serialization error: {e}")))?;
    out.push('\n');
    Ok(out)
}

fn render_dotenv(vars: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::new();
    for (name, value) in vars {
        out.push_str(name);
        out.push('=');
        out.push_str(
            &dotenv_quote(value).ok_or_else(|| ShhError::DotenvExport { name: name.clone() })?,
        );
        out.push('\n');
    }
    Ok(out)
}

fn render_shell(vars: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (name, value) in vars {
        out.push_str("export ");
        out.push_str(name);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

fn dotenv_quote(value: &str) -> Option<String> {
    if is_unquoted_dotenv_value(value) {
        return Some(value.to_string());
    }
    if !value.contains('\'') && !value.contains('\n') && !value.contains('\r') {
        return Some(format!("'{value}'"));
    }
    if value.contains("${") || value.contains("$(") || value.contains('`') {
        return None;
    }

    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out.push('"');
    Some(out)
}

fn is_unquoted_dotenv_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@'))
}

fn tty_refusal_message(profile: Option<&str>, format: Option<ExportFormat>) -> String {
    let profile_arg = profile.map(|p| format!(" -p {p}")).unwrap_or_default();
    match format {
        None => format!(
            "error: refusing to print export lines to a terminal. Use: eval \"$(shh export{profile_arg})\""
        ),
        Some(ExportFormat::Json) => format!(
            "error: refusing to print JSON secrets to a terminal. Redirect stdout: shh export{profile_arg} --format json > secrets.json"
        ),
        Some(ExportFormat::Dotenv) => format!(
            "error: refusing to print dotenv secrets to a terminal. Redirect stdout: shh export{profile_arg} --format dotenv > .env.shh"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dotenv;

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn renders_shell_default() {
        let v = vars(&[("A", "hello"), ("B", "two words")]);
        assert_eq!(
            render(&v, None).unwrap(),
            "export A=hello\nexport B=two words\n"
        );
    }

    #[test]
    fn renders_json() {
        let v = vars(&[("A", "hello"), ("B", "line\nquote\"")]);
        assert_eq!(
            render(&v, Some(ExportFormat::Json)).unwrap(),
            "{\n  \"A\": \"hello\",\n  \"B\": \"line\\nquote\\\"\"\n}\n"
        );
    }

    #[test]
    fn renders_dotenv_round_trips_common_values() {
        let v = vars(&[
            ("A", "hello"),
            ("B", "two words"),
            ("C", "line\nquote\""),
            ("D", "${LITERAL}"),
        ]);
        let out = render(&v, Some(ExportFormat::Dotenv)).unwrap();
        assert_eq!(
            out,
            "A=hello\nB='two words'\nC=\"line\\nquote\\\"\"\nD='${LITERAL}'\n"
        );

        let parsed = dotenv::parse(&out);
        assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
        let parsed_vars: BTreeMap<String, String> = parsed
            .entries
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().to_string()))
            .collect();
        assert_eq!(parsed_vars, v);
    }

    #[test]
    fn dotenv_rejects_values_that_would_interpolate_in_double_quotes() {
        let v = vars(&[("A", "it's ${TOKEN}")]);
        assert!(matches!(
            render(&v, Some(ExportFormat::Dotenv)),
            Err(ShhError::DotenvExport { name }) if name == "A"
        ));
    }

    #[test]
    fn dotenv_rejects_values_that_parser_rejects_in_double_quotes() {
        for value in ["it's $(cmd)", "it's `cmd`"] {
            let v = vars(&[("A", value)]);
            assert!(matches!(
                render(&v, Some(ExportFormat::Dotenv)),
                Err(ShhError::DotenvExport { name }) if name == "A"
            ));
        }
    }

    #[test]
    fn tty_refusal_mentions_selected_format() {
        assert!(tty_refusal_message(None, None).contains("eval \"$(shh export)\""));
        assert!(tty_refusal_message(Some("work"), Some(ExportFormat::Json))
            .contains("shh export -p work --format json"));
        assert!(
            tty_refusal_message(Some("work"), Some(ExportFormat::Dotenv))
                .contains("shh export -p work --format dotenv")
        );
    }
}
