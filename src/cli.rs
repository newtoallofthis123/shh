use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "shh", version, about = "macOS Keychain-backed env-var secrets")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Set {
        name: String,
        value: Option<String>,
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Get {
        name: String,
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Rm {
        name: String,
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Ls {
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Profiles,
    Load {
        path: PathBuf,
        #[arg(short = 'p', long)]
        profile: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        except: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    Export {
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Unset {
        #[arg(short = 'p', long)]
        profile: Option<String>,
    },
    Run {
        #[arg(short = 'p', long)]
        profile: Option<String>,
        #[arg(long)]
        clean: bool,
        #[arg(last = true)]
        argv: Vec<String>,
    },
    Doctor {
        #[command(subcommand)]
        which: DoctorCmd,
    },
    Completions {
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum DoctorCmd {
    Keychain,
}
