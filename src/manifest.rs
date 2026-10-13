//! The `.sync_manifest.json` state file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fsutil;

pub const MANIFEST_FILE: &str = ".sync_manifest.json";
pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub plain_sha256: String,
    pub target_rel_path: String,
    pub modified_at_epoch: i64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub last_sync_timestamp: i64,
    pub entries: BTreeMap<String, Entry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest {
            version: MANIFEST_VERSION,
            last_sync_timestamp: 0,
            entries: BTreeMap::new(),
        }
    }
}

impl Manifest {
    /// Loads the manifest from `target_dir`; a missing file yields an empty manifest.
    pub fn load(target_dir: &Path) -> Result<Manifest> {
        let path = target_dir.join(MANIFEST_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Manifest::default()),
            Err(e) => return Err(Error::io(&path, e)),
        };
        let manifest: Manifest = serde_json::from_str(&text).map_err(|e| Error::Manifest {
            path: path.clone(),
            message: e.to_string(),
        })?;
        if manifest.version != MANIFEST_VERSION {
            return Err(Error::Manifest {
                path,
                message: format!(
                    "unsupported version {} (expected {MANIFEST_VERSION})",
                    manifest.version
                ),
            });
        }
        Ok(manifest)
    }

    /// Atomically writes the manifest into `target_dir`.
    pub fn save(&self, target_dir: &Path) -> Result<()> {
        let path = target_dir.join(MANIFEST_FILE);
        let mut json = serde_json::to_string_pretty(self).map_err(|e| Error::Manifest {
            path: path.clone(),
            message: e.to_string(),
        })?;
        json.push('\n');
        fsutil::write_atomic(&path, json.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        let mut m = Manifest {
            last_sync_timestamp: 1_774_000_000,
            ..Manifest::default()
        };
        m.entries.insert(
            "Notes/Daily.md".into(),
            Entry {
                plain_sha256: "ab".repeat(32),
                target_rel_path: "vault/Notes/Daily.md.gpg".into(),
                modified_at_epoch: 1_773_999_900,
                size_bytes: 1024,
            },
        );
        m
    }

    #[test]
    fn missing_manifest_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap(), Manifest::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let m = sample();
        m.save(dir.path()).unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap(), m);
        assert!(!dir.path().join(".sync_manifest.json.tmp").exists());
    }

    #[test]
    fn parses_spec_example() {
        let json = r#"{
          "version": 1,
          "last_sync_timestamp": 1774000000,
          "entries": {
            "Notes/Daily.md": {
              "plain_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
              "target_rel_path": "vault/Notes/Daily.md.gpg",
              "modified_at_epoch": 1773999900,
              "size_bytes": 1024
            }
          }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MANIFEST_FILE), json).unwrap();
        let m = Manifest::load(dir.path()).unwrap();
        assert_eq!(m.entries["Notes/Daily.md"].size_bytes, 1024);
    }

    #[test]
    fn rejects_corrupt_and_future_versions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MANIFEST_FILE), "{ nope").unwrap();
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(Error::Manifest { .. })
        ));
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            r#"{"version":2,"last_sync_timestamp":0,"entries":{}}"#,
        )
        .unwrap();
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(Error::Manifest { .. })
        ));
    }
}
