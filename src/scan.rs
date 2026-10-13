//! Source directory scanning.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use tracing::{debug, warn};
use walkdir::WalkDir;

use crate::diff::SourceFile;
use crate::error::{Error, Result};
use crate::filter::PathFilter;
use crate::fsutil;

/// Walks `source_dir`, applies the filter and hashes every remaining regular file.
///
/// Keys are vault-relative POSIX paths. Symlinks and non-UTF-8 names are skipped.
/// `skip_dir` (typically the backup target) is never descended into.
pub fn scan(
    source_dir: &Path,
    filter: &PathFilter,
    skip_dir: Option<&Path>,
) -> Result<BTreeMap<String, SourceFile>> {
    let mut files = BTreeMap::new();
    let walker = WalkDir::new(source_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| skip_dir.is_none_or(|skip| e.path() != skip));

    for entry in walker {
        let entry = entry.map_err(|source| Error::Walk {
            path: source_dir.to_path_buf(),
            source,
        })?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            debug!("skipping symlink {}", entry.path().display());
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(source_dir)
            .expect("walkdir yields paths under its root");
        let Some(rel) = fsutil::to_posix(rel) else {
            warn!(
                "skipping path with non-UTF-8 name: {}",
                entry.path().display()
            );
            continue;
        };
        if filter.is_excluded(&rel) {
            debug!("excluded {rel}");
            continue;
        }
        let meta = entry.metadata().map_err(|source| Error::Walk {
            path: entry.path().to_path_buf(),
            source,
        })?;
        let modified_at_epoch = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        let sha256 = fsutil::sha256_file(entry.path())?;
        files.insert(
            rel,
            SourceFile {
                sha256,
                modified_at_epoch,
                size_bytes: meta.len(),
            },
        );
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Filters;

    #[test]
    fn scans_filters_and_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (rel, body) in [
            ("Notes/Daily.md", "abc"),
            (".git/config", "x"),
            (".DS_Store", "x"),
            ("Attachments/img.png", "png"),
        ] {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let filter = PathFilter::new(&Filters {
            exclude_patterns: vec![".git/**".into(), "**/.DS_Store".into()],
            include_obsidian_config: true,
        })
        .unwrap();

        let files = scan(root, &filter, None).unwrap();
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["Attachments/img.png", "Notes/Daily.md"]
        );
        let daily = &files["Notes/Daily.md"];
        assert_eq!(
            daily.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(daily.size_bytes, 3);
    }

    #[test]
    fn skips_symlinks_and_nested_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("real.md"), "r").unwrap();
        std::os::unix::fs::symlink(root.join("real.md"), root.join("link.md")).unwrap();
        std::fs::create_dir_all(root.join("backup")).unwrap();
        std::fs::write(root.join("backup/x.gpg"), "g").unwrap();

        let filter = PathFilter::new(&Filters::default()).unwrap();
        let files = scan(root, &filter, Some(&root.join("backup"))).unwrap();
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["real.md"]
        );
    }
}
