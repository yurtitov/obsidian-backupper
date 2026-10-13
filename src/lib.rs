//! `obsidian-backupper`: incremental GnuPG-encrypted backups of an Obsidian vault into Git.

pub mod backup;
pub mod check;
pub mod cli;
pub mod config;
pub mod diff;
pub mod error;
pub mod exec;
pub mod filter;
pub mod fsutil;
pub mod git;
pub mod gpg;
pub mod manifest;
pub mod restore;
pub mod scan;
