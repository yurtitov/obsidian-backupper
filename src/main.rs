use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::Parser;
use tracing::Level;

use obsidian_backupper::cli::{Cli, Command};
use obsidian_backupper::config::{self, Config};
use obsidian_backupper::{backup, check, restore};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let verbose = matches!(cli.command, Command::Backup { verbose: true, .. });
    tracing_subscriber::fmt()
        .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .init();

    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn load_config(cli_path: Option<&std::path::Path>) -> anyhow::Result<Config> {
    let path = config::resolve_config_path(cli_path)?;
    tracing::debug!("using config {}", path.display());
    Config::load(&path).with_context(|| format!("loading config {}", path.display()))
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Init { force } => {
            let path = match cli.config {
                Some(p) => p,
                None => config::default_config_path()?,
            };
            if path.exists() && !force {
                bail!(
                    "{} already exists (use --force to overwrite)",
                    path.display()
                );
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            std::fs::write(&path, config::EXAMPLE_CONFIG)
                .with_context(|| format!("writing {}", path.display()))?;
            println!(
                "Created {}. Edit it, then run `obsidian-backupper check`.",
                path.display()
            );
        }
        Command::Backup { dry_run, .. } => {
            let config = load_config(cli.config.as_deref())?;
            backup::run(&config, backup::Options { dry_run })?;
        }
        Command::Restore { target, force } => {
            let config = load_config(cli.config.as_deref())?;
            let dest = target.unwrap_or_else(|| config.paths.source_dir.clone());
            let count = restore::run(&config, &dest, restore::Options { force })?;
            println!("Restored {count} file(s) into {}", dest.display());
        }
        Command::Check => {
            let config = load_config(cli.config.as_deref())?;
            if check::run(&config) > 0 {
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
