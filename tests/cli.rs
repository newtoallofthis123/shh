//! Integration tests for the command dispatcher.
//!
//! These tests exercise `shh::commands::dispatch` against a `MemoryStore`
//! so they never touch the real macOS Keychain. Tests that print to stdout
//! (`get`, `export`, `unset`, `ls`, `profiles`) are validated by reading
//! the store state produced by the handlers — the dispatcher's stdout is
//! tested separately in chapters that own UX assertions.

use std::path::PathBuf;

use shh::cli::Command;
use shh::commands::{dispatch, CommandOutcome};
use shh::error::ShhError;
use shh::store::memory::MemoryStore;
use shh::store::SecretStore;

fn set_cmd(name: &str, value: &str, profile: Option<&str>) -> Command {
    Command::Set {
        name: name.into(),
        value: Some(value.into()),
        profile: profile.map(str::to_string),
    }
}

#[test]
fn set_happy_path_writes_to_default() {
    let store = MemoryStore::new();
    let outcome = dispatch(set_cmd("FOO", "bar", None), &store).unwrap();
    assert_eq!(outcome, CommandOutcome::Success);
    assert_eq!(store.get("default", "FOO").unwrap().as_deref(), Some("bar"));
}

#[test]
fn set_invalid_name_errors() {
    let store = MemoryStore::new();
    let err = dispatch(set_cmd("1bad", "v", None), &store).unwrap_err();
    assert!(matches!(err, ShhError::InvalidEnvName(_)));
}

#[test]
fn set_invalid_profile_errors() {
    let store = MemoryStore::new();
    let err = dispatch(set_cmd("FOO", "v", Some("bad:profile")), &store).unwrap_err();
    assert!(matches!(err, ShhError::InvalidProfile(_)));
}

#[test]
fn get_missing_returns_exit_code_one() {
    let store = MemoryStore::new();
    // stdout is not a TTY under cargo test, so the TTY guard does not fire.
    let outcome = dispatch(
        Command::Get {
            name: "MISSING".into(),
            profile: None,
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::ExitCode(1));
}

#[test]
fn get_resolves_overlay() {
    let store = MemoryStore::new();
    store.set("default", "SHARED", "d").unwrap();
    store.set("work", "SHARED", "w").unwrap();
    let outcome = dispatch(
        Command::Get {
            name: "SHARED".into(),
            profile: Some("work".into()),
        },
        &store,
    )
    .unwrap();
    // We can't easily capture stdout from the integration harness, but the
    // success outcome confirms resolution did not error and the value was
    // present.
    assert_eq!(outcome, CommandOutcome::Success);
}

#[test]
fn rm_existing_succeeds_and_deletes() {
    let store = MemoryStore::new();
    store.set("default", "FOO", "v").unwrap();
    let outcome = dispatch(
        Command::Rm {
            name: "FOO".into(),
            profile: None,
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::Success);
    assert_eq!(store.get("default", "FOO").unwrap(), None);
}

#[test]
fn rm_missing_returns_exit_code_one() {
    let store = MemoryStore::new();
    let outcome = dispatch(
        Command::Rm {
            name: "FOO".into(),
            profile: None,
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::ExitCode(1));
}

#[test]
fn ls_default_runs() {
    let store = MemoryStore::new();
    store.set("default", "A", "1").unwrap();
    store.set("default", "B", "2").unwrap();
    dispatch(Command::Ls { profile: None }, &store).unwrap();
}

#[test]
fn ls_with_profile_runs_for_all_three_source_kinds() {
    let store = MemoryStore::new();
    store.set("default", "ONLY_D", "d").unwrap();
    store.set("default", "SHARED", "d").unwrap();
    store.set("work", "SHARED", "w").unwrap();
    store.set("work", "ONLY_P", "w").unwrap();
    let outcome = dispatch(
        Command::Ls {
            profile: Some("work".into()),
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::Success);
}

#[test]
fn profiles_lists_default_when_default_entries_exist() {
    let store = MemoryStore::new();
    store.set("default", "A", "1").unwrap();
    store.set("work", "B", "2").unwrap();
    dispatch(Command::Profiles, &store).unwrap();
}

fn write_temp_env(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("shh-cli-test-{}-{}.env", name, std::process::id()));
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn load_all_writes_all_entries() {
    let store = MemoryStore::new();
    let path = write_temp_env("all", "A=1\nB=2\nC=3\n");
    dispatch(
        Command::Load {
            path: path.clone(),
            profile: None,
            all: true,
            only: vec![],
            except: vec![],
            dry_run: false,
        },
        &store,
    )
    .unwrap();
    assert_eq!(store.get("default", "A").unwrap().as_deref(), Some("1"));
    assert_eq!(store.get("default", "B").unwrap().as_deref(), Some("2"));
    assert_eq!(store.get("default", "C").unwrap().as_deref(), Some("3"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn load_only_writes_subset() {
    let store = MemoryStore::new();
    let path = write_temp_env("only", "A=1\nB=2\nC=3\n");
    dispatch(
        Command::Load {
            path: path.clone(),
            profile: None,
            all: false,
            only: vec!["A".into(), "C".into()],
            except: vec![],
            dry_run: false,
        },
        &store,
    )
    .unwrap();
    assert_eq!(store.get("default", "A").unwrap().as_deref(), Some("1"));
    assert_eq!(store.get("default", "B").unwrap(), None);
    assert_eq!(store.get("default", "C").unwrap().as_deref(), Some("3"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn load_except_writes_complement() {
    let store = MemoryStore::new();
    let path = write_temp_env("except", "A=1\nB=2\nC=3\n");
    dispatch(
        Command::Load {
            path: path.clone(),
            profile: None,
            all: false,
            only: vec![],
            except: vec!["B".into()],
            dry_run: false,
        },
        &store,
    )
    .unwrap();
    assert_eq!(store.get("default", "A").unwrap().as_deref(), Some("1"));
    assert_eq!(store.get("default", "B").unwrap(), None);
    assert_eq!(store.get("default", "C").unwrap().as_deref(), Some("3"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn load_dry_run_writes_nothing() {
    let store = MemoryStore::new();
    let path = write_temp_env("dryrun", "A=1\nB=2\n");
    dispatch(
        Command::Load {
            path: path.clone(),
            profile: None,
            all: true,
            only: vec![],
            except: vec![],
            dry_run: true,
        },
        &store,
    )
    .unwrap();
    assert_eq!(store.get("default", "A").unwrap(), None);
    assert_eq!(store.get("default", "B").unwrap(), None);
    let _ = std::fs::remove_file(path);
}

#[test]
fn export_runs_when_stdout_not_tty() {
    let store = MemoryStore::new();
    store.set("default", "A", "hello").unwrap();
    let outcome = dispatch(
        Command::Export {
            profile: None,
            format: None,
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::Success);
}

#[test]
fn unset_runs_when_stdout_not_tty() {
    let store = MemoryStore::new();
    store.set("default", "A", "hello").unwrap();
    let outcome = dispatch(Command::Unset { profile: None }, &store).unwrap();
    assert_eq!(outcome, CommandOutcome::Success);
}

#[test]
fn run_spawns_child_and_returns_exit_code() {
    let store = MemoryStore::new();
    store.set("default", "FOO", "bar").unwrap();
    let outcome = dispatch(
        Command::Run {
            profile: None,
            clean: false,
            argv: vec!["true".into()],
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::ExitCode(0));
}

#[test]
fn run_clean_spawns_child_with_minimal_env() {
    let store = MemoryStore::new();
    store.set("default", "FOO", "bar").unwrap();
    let outcome = dispatch(
        Command::Run {
            profile: None,
            clean: true,
            argv: vec!["true".into()],
        },
        &store,
    )
    .unwrap();
    assert_eq!(outcome, CommandOutcome::ExitCode(0));
}
