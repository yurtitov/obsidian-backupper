//! Command-line interface definition.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "obsidian-backupper", version, about)]
pub struct Cli {
    /// Path to the config file (overrides OBSIDIAN_BACKUPPER_CONFIG and system locations).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create an example config file.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
    /// Encrypt changed notes and sync them to the Git repository.
    Backup {
        /// Print the planned actions without touching the disk or Git.
        #[arg(long)]
        dry_run: bool,
        /// Enable debug logging.
        #[arg(long)]
        verbose: bool,
    },
    /// Decrypt files from the backup.
    Restore {
        /// Directory to restore into (default: paths.source_dir).
        #[arg(long, value_name = "DIR")]
        target: Option<PathBuf>,
        /// Overwrite existing files that differ from the backup.
        #[arg(long)]
        force: bool,
    },
    /// Verify the environment: gpg key, git remote, directories.
    Check,
}
