//! Atomic file helpers and path utilities.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Returns `path` with `.tmp` appended to its file name.
pub fn tmp_path(path: &Path) -> PathBuf {
    let mut name: OsString = path.file_name().unwrap_or_default().to_owned();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Writes `bytes` to `path` atomically via a sibling `.tmp` file and `rename`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path(path);
    std::fs::write(&tmp, bytes).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::io(path, e)
    })
}

pub fn create_parent_dirs(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    Ok(())
}

/// Streams a file through SHA-256 and returns the lowercase hex digest.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Converts a relative path to POSIX form (`/` separators). Returns `None` for non-UTF-8 paths.
pub fn to_posix(rel: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    parts.map(|p| p.join("/"))
}

/// Validates that a manifest-provided relative path stays inside its base directory.
pub fn ensure_safe_rel(rel: &str) -> Result<()> {
    let unsafe_path = rel.is_empty()
        || rel.contains('\\')
        || Path::new(rel)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)));
    if unsafe_path {
        Err(Error::UnsafePath(rel.to_string()))
    } else {
        Ok(())
    }
}

/// Removes empty directories below `root` (never `root` itself).
pub fn prune_empty_dirs(root: &Path) -> Result<()> {
    fn walk(dir: &Path, is_root: bool) -> Result<bool> {
        let mut empty = true;
        for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
            let entry = entry.map_err(|e| Error::io(dir, e))?;
            let file_type = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
            if file_type.is_dir() && walk(&entry.path(), false)? {
                continue;
            }
            empty = false;
        }
        if empty && !is_root {
            std::fs::remove_dir(dir).map_err(|e| Error::io(dir, e))?;
        }
        Ok(empty)
    }
    if root.is_dir() {
        walk(root, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_path_appends_suffix() {
        assert_eq!(
            tmp_path(Path::new("/a/b/file.md.gpg")),
            PathBuf::from("/a/b/file.md.gpg.tmp")
        );
    }

    #[test]
    fn sha256_of_known_input() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty");
        std::fs::write(&p, b"").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn posix_conversion() {
        assert_eq!(to_posix(Path::new("a/b/c.md")).as_deref(), Some("a/b/c.md"));
        assert_eq!(to_posix(Path::new("/abs")), None);
    }

    #[test]
    fn safe_rel_paths() {
        assert!(ensure_safe_rel("Notes/a.md").is_ok());
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "a\\b", "./a"] {
            assert!(ensure_safe_rel(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn prunes_only_empty_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("a/b/c")).unwrap();
        std::fs::create_dir_all(root.join("keep/inner")).unwrap();
        std::fs::write(root.join("keep/file"), "x").unwrap();
        prune_empty_dirs(root).unwrap();
        assert!(!root.join("a").exists());
        assert!(root.join("keep/file").exists());
        assert!(!root.join("keep/inner").exists());
        assert!(root.exists());
    }
}
