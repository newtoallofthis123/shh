use anyhow::Result;
use clap::Parser;
use shh::cli::{Cli, Command};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Set { .. } => anyhow::bail!("set: unimplemented in this chapter"),
        Command::Get { .. } => anyhow::bail!("get: unimplemented in this chapter"),
        Command::Rm { .. } => anyhow::bail!("rm: unimplemented in this chapter"),
        Command::Ls { .. } => anyhow::bail!("ls: unimplemented in this chapter"),
        Command::Profiles => anyhow::bail!("profiles: unimplemented in this chapter"),
        Command::Load { .. } => anyhow::bail!("load: unimplemented in this chapter"),
        Command::Export { .. } => anyhow::bail!("export: unimplemented in this chapter"),
        Command::Unset { .. } => anyhow::bail!("unset: unimplemented in this chapter"),
        Command::Run { .. } => anyhow::bail!("run: unimplemented in this chapter"),
        Command::Doctor { .. } => anyhow::bail!("doctor: unimplemented in this chapter"),
        Command::Completions { .. } => {
            anyhow::bail!("completions: unimplemented in this chapter")
        }
    }
}
