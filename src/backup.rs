//! The `backup` command: scan, diff, encrypt, update manifest, sync with Git.

use std::path::{Path, PathBuf};

use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::diff::{self, Action, ActionKind};
use crate::error::{Error, Result};
use crate::filter::PathFilter;
use crate::git::Git;
use crate::gpg::Gpg;
use crate::manifest::{Entry, Manifest};
use crate::{fsutil, scan};

pub const VAULT_DIR: &str = "vault";
const ENCRYPTED_SUFFIX: &str = ".gpg";

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub dry_run: bool,
}

/// Relative path of the encrypted copy of `rel` inside the target directory.
pub fn target_rel_path(rel: &str) -> String {
    format!("{VAULT_DIR}/{rel}{ENCRYPTED_SUFFIX}")
}

/// Runs a backup and returns the computed plan.
pub fn run(config: &Config, opts: Options) -> Result<Vec<Action>> {
    let source_dir = &config.paths.source_dir;
    let target_dir = &config.paths.target_dir;

    if !source_dir.is_dir() {
        return Err(Error::SourceMissing(source_dir.clone()));
    }
    let filter = PathFilter::new(&config.filters)?;

    if !opts.dry_run {
        prepare_environment(config)?;
    }

    let skip_dir = target_dir.canonicalize().ok();
    let source_canon = source_dir
        .canonicalize()
        .map_err(|e| Error::io(source_dir, e))?;
    let nested_target = skip_dir.as_deref().filter(|t| t.starts_with(&source_canon));
    let source = scan::scan(&source_canon, &filter, nested_target)?;
    info!(
        "scanned {} file(s) in {}",
        source.len(),
        source_dir.display()
    );

    let mut manifest = Manifest::load(target_dir)?;
    let actions = diff::compute(&source, &manifest.entries);

    if opts.dry_run {
        print_plan(&actions);
        return Ok(actions);
    }

    let gpg = Gpg::new(
        &config.encryption.gpg_binary,
        &config.encryption.recipient,
        config.encryption.armor,
    );
    let failures = apply(
        &actions,
        &source_canon,
        target_dir,
        &gpg,
        &source,
        &mut manifest,
    );

    manifest.last_sync_timestamp = Utc::now().timestamp();
    manifest.save(target_dir)?;
    fsutil::prune_empty_dirs(&target_dir.join(VAULT_DIR))?;

    if let Some(first) = failures.first() {
        return Err(Error::Partial {
            failed: failures.len(),
            first: first.to_string(),
        });
    }

    if config.git.auto_push {
        sync_git(config)?;
    }
    Ok(actions)
}

/// Creates the target directory, initialises Git when needed and verifies the GPG key.
fn prepare_environment(config: &Config) -> Result<()> {
    let target_dir = &config.paths.target_dir;
    std::fs::create_dir_all(target_dir).map_err(|e| Error::io(target_dir, e))?;

    let git = Git::new(target_dir);
    if !git.is_repo() {
        info!("initialising git repository in {}", target_dir.display());
        git.init(&config.git.branch)?;
    }
    if let Some(url) = &config.git.remote_url
        && let Some(existing) = git.ensure_origin(url)?
    {
        warn!("git remote 'origin' is {existing}, which differs from configured {url}");
    }

    Gpg::new(
        &config.encryption.gpg_binary,
        &config.encryption.recipient,
        config.encryption.armor,
    )
    .check_recipient()
}

fn apply(
    actions: &[Action],
    source_dir: &Path,
    target_dir: &Path,
    gpg: &Gpg,
    source: &std::collections::BTreeMap<String, diff::SourceFile>,
    manifest: &mut Manifest,
) -> Vec<Error> {
    let work: Vec<&Action> = actions
        .iter()
        .filter(|a| a.kind != ActionKind::Skip)
        .collect();
    let progress = ProgressBar::new(work.len() as u64);
    progress.set_style(
        ProgressStyle::with_template("{bar:40} {pos}/{len} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let mut failures = Vec::new();
    for action in work {
        progress.set_message(format!("{} {}", action.kind, action.rel_path));
        let result = match action.kind {
            ActionKind::Create | ActionKind::Update => {
                encrypt_one(action, source_dir, target_dir, gpg, source, manifest)
            }
            ActionKind::Delete => delete_one(action, target_dir, manifest),
            ActionKind::Skip => Ok(()),
        };
        if let Err(err) = result {
            warn!("{} {} failed: {err}", action.kind, action.rel_path);
            failures.push(err);
        }
        progress.inc(1);
    }
    progress.finish_and_clear();
    failures
}

fn encrypt_one(
    action: &Action,
    source_dir: &Path,
    target_dir: &Path,
    gpg: &Gpg,
    source: &std::collections::BTreeMap<String, diff::SourceFile>,
    manifest: &mut Manifest,
) -> Result<()> {
    let file = &source[&action.rel_path];
    let rel_target = target_rel_path(&action.rel_path);
    let dest: PathBuf = target_dir.join(&rel_target);
    let tmp = fsutil::tmp_path(&dest);

    fsutil::create_parent_dirs(&dest)?;
    debug!("{} {}", action.kind, action.rel_path);
    if let Err(err) = gpg.encrypt(&source_dir.join(&action.rel_path), &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::io(&dest, e)
    })?;

    manifest.entries.insert(
        action.rel_path.clone(),
        Entry {
            plain_sha256: file.sha256.clone(),
            target_rel_path: rel_target,
            modified_at_epoch: file.modified_at_epoch,
            size_bytes: file.size_bytes,
        },
    );
    Ok(())
}

fn delete_one(action: &Action, target_dir: &Path, manifest: &mut Manifest) -> Result<()> {
    if let Some(entry) = manifest.entries.get(&action.rel_path) {
        fsutil::ensure_safe_rel(&entry.target_rel_path)?;
        let path = target_dir.join(&entry.target_rel_path);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(&path, e)),
        }
    }
    debug!("DELETE {}", action.rel_path);
    manifest.entries.remove(&action.rel_path);
    Ok(())
}

fn sync_git(config: &Config) -> Result<()> {
    let git = Git::new(&config.paths.target_dir);
    git.add_all()?;
    if !git.has_changes()? {
        info!("no changes to commit");
        return Ok(());
    }
    let datetime = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let message = config.git.commit_message.replace("{datetime}", &datetime);
    git.commit(&message)?;
    info!("committed: {message}");
    git.push(&config.git.branch)?;
    info!("pushed to origin/{}", config.git.branch);
    Ok(())
}

/// Prints the planned actions as a table on stdout.
pub fn print_plan(actions: &[Action]) {
    println!("{:<8} PATH", "ACTION");
    for action in actions {
        println!("{:<8} {}", action.kind.to_string(), action.rel_path);
    }
    let count = |kind| actions.iter().filter(|a| a.kind == kind).count();
    println!(
        "\n{} create, {} update, {} delete, {} skip",
        count(ActionKind::Create),
        count(ActionKind::Update),
        count(ActionKind::Delete),
        count(ActionKind::Skip),
    );
}
