use anyhow::Result;
use clap::Parser;
use shh::cli::Cli;
use shh::commands::{dispatch, CommandOutcome};
use shh::error::ShhError;
use shh::store::KeychainStore;

fn main() {
    let code = match run() {
        Ok(CommandOutcome::Success) => 0,
        Ok(CommandOutcome::ExitCode(c)) => c,
        Err(e) => {
            // Map specific typed errors to PRD-prescribed guidance.
            if let Some(shh_err) = e.downcast_ref::<ShhError>() {
                if matches!(shh_err, ShhError::KeychainDenied) {
                    eprintln!("Keychain access was denied.");
                    eprintln!(
                        "Rerun the command and approve the macOS prompt, or inspect the item in Keychain Access."
                    );
                    std::process::exit(1);
                }
            }
            eprintln!("error: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<CommandOutcome> {
    let cli = Cli::parse();
    let store = KeychainStore::from_env();
    Ok(dispatch(cli.command, &store)?)
}
