use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::dotenv::{parse, select, Selector};
use crate::error::{Result, ShhError};
use crate::profile::is_valid_profile_slug;
use crate::store::SecretStore;
use crate::tty::is_stdin_tty;

use super::CommandOutcome;

pub struct LoadArgs {
    pub path: PathBuf,
    pub profile: Option<String>,
    pub all: bool,
    pub only: Vec<String>,
    pub except: Vec<String>,
    pub dry_run: bool,
}

pub fn run(args: LoadArgs, store: &dyn SecretStore) -> Result<CommandOutcome> {
    let profile = args.profile.as_deref().unwrap_or("default");
    if !is_valid_profile_slug(profile) {
        return Err(ShhError::InvalidProfile(profile.to_string()));
    }

    let path = absolutize(&args.path)?;
    let body = std::fs::read_to_string(&path).map_err(|e| {
        ShhError::Io(std::io::Error::new(
            e.kind(),
            format!("{}: {}", path.display(), e),
        ))
    })?;
    let outcome = parse(&body);

    for rej in &outcome.rejected {
        eprintln!(
            "warning: rejected entry on line {}: {} ({})",
            rej.line, rej.name, rej.reason
        );
    }
    for err in &outcome.errors {
        eprintln!("error: parse error on line {}: {}", err.line, err.message);
    }

    let has_selector_flag = args.all || !args.only.is_empty() || !args.except.is_empty();

    // Decide which entries to import. Three exclusive paths:
    //   * an explicit selector flag,
    //   * an interactive checklist when stdin is a TTY,
    //   * non-interactive without flags is a usage error.
    let interactive_names: Vec<String>;
    let selector: Selector<'_> = if has_selector_flag {
        if args.all {
            Selector::All
        } else if !args.only.is_empty() {
            Selector::Only(&args.only)
        } else {
            Selector::Except(&args.except)
        }
    } else if is_stdin_tty() {
        let names: Vec<String> = outcome.entries.iter().map(|(n, _)| n.clone()).collect();
        if names.is_empty() {
            eprintln!("error: no valid entries found in {}", path.display());
            return Ok(CommandOutcome::ExitCode(2));
        }
        let picked = inquire::MultiSelect::new("Select entries to import:", names)
            .prompt()
            .map_err(|e| ShhError::Keychain(format!("prompt error: {e}")))?;
        interactive_names = picked;
        Selector::Interactive(&interactive_names)
    } else {
        eprintln!("error: non-interactive mode requires one of --all, --only, or --except");
        return Ok(CommandOutcome::ExitCode(2));
    };

    let resolved =
        select(&outcome.entries, selector).map_err(|e| ShhError::Keychain(e.to_string()))?;

    if args.dry_run {
        let existing: BTreeSet<String> = store.list_names(profile)?.into_iter().collect();
        for (name, _value) in &resolved {
            if existing.contains(name) {
                println!("would update {name}");
            } else {
                println!("would add {name}");
            }
        }
    } else {
        for (name, value) in &resolved {
            store.set(profile, name, value.as_str())?;
        }
    }

    Ok(CommandOutcome::Success)
}

fn absolutize(path: &Path) -> Result<PathBuf> {
    let s = path.to_str().ok_or_else(|| {
        ShhError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path is not valid UTF-8: {}", path.display()),
        ))
    })?;
    let expanded = shellexpand::tilde(s);
    Ok(std::path::absolute(expanded.as_ref())?)
}
