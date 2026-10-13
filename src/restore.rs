//! The `restore` command: decrypt the backup into a directory.

use std::path::Path;

use indicatif::ProgressBar;
use tracing::{info, warn};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::fsutil;
use crate::gpg::Gpg;
use crate::manifest::Manifest;

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub force: bool,
}

/// Decrypts every manifest entry into `dest_dir`, verifying SHA-256 of the result.
/// Returns the number of restored files.
pub fn run(config: &Config, dest_dir: &Path, opts: Options) -> Result<usize> {
    let target_dir = &config.paths.target_dir;
    let manifest = Manifest::load(target_dir)?;
    if manifest.entries.is_empty() {
        info!("manifest is empty, nothing to restore");
        return Ok(0);
    }

    for (rel, entry) in &manifest.entries {
        fsutil::ensure_safe_rel(rel)?;
        fsutil::ensure_safe_rel(&entry.target_rel_path)?;
    }

    // Fail early, before decrypting anything, if existing files would be clobbered.
    let mut pending = Vec::new();
    for (rel, entry) in &manifest.entries {
        let dest = dest_dir.join(rel);
        if dest.exists() && !opts.force {
            if fsutil::sha256_file(&dest)? == entry.plain_sha256 {
                continue;
            }
            return Err(Error::WouldOverwrite(dest));
        }
        pending.push((rel, entry, dest));
    }

    let gpg = Gpg::new(
        &config.encryption.gpg_binary,
        &config.encryption.recipient,
        config.encryption.armor,
    );
    let progress = ProgressBar::new(pending.len() as u64);
    let mut restored = 0;
    for (rel, entry, dest) in pending {
        fsutil::create_parent_dirs(&dest)?;
        let tmp = fsutil::tmp_path(&dest);
        let result = gpg
            .decrypt(&target_dir.join(&entry.target_rel_path), &tmp)
            .and_then(|()| {
                let actual = fsutil::sha256_file(&tmp)?;
                if actual == entry.plain_sha256 {
                    Ok(())
                } else {
                    Err(Error::ChecksumMismatch {
                        path: rel.clone(),
                        expected: entry.plain_sha256.clone(),
                        actual,
                    })
                }
            })
            .and_then(|()| std::fs::rename(&tmp, &dest).map_err(|e| Error::io(&dest, e)));
        if let Err(err) = result {
            let _ = std::fs::remove_file(&tmp);
            warn!("failed to restore {rel}");
            return Err(err);
        }
        restored += 1;
        progress.inc(1);
    }
    progress.finish_and_clear();
    Ok(restored)
}
