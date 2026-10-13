//! Diff engine: compares the current source state with the manifest.

use std::collections::BTreeMap;
use std::fmt;

use crate::manifest::Entry;

/// Snapshot of a source file as seen during scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub sha256: String,
    pub modified_at_epoch: i64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Create,
    Update,
    Delete,
    Skip,
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ActionKind::Create => "CREATE",
            ActionKind::Update => "UPDATE",
            ActionKind::Delete => "DELETE",
            ActionKind::Skip => "SKIP",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub rel_path: String,
}

/// Computes actions for every path in either side, sorted by path.
pub fn compute(
    source: &BTreeMap<String, SourceFile>,
    manifest: &BTreeMap<String, Entry>,
) -> Vec<Action> {
    let mut actions = Vec::with_capacity(source.len().max(manifest.len()));
    for (path, file) in source {
        let kind = match manifest.get(path) {
            None => ActionKind::Create,
            Some(entry) if entry.plain_sha256 != file.sha256 => ActionKind::Update,
            Some(_) => ActionKind::Skip,
        };
        actions.push(Action {
            kind,
            rel_path: path.clone(),
        });
    }
    for path in manifest.keys().filter(|p| !source.contains_key(*p)) {
        actions.push(Action {
            kind: ActionKind::Delete,
            rel_path: path.clone(),
        });
    }
    actions.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(hash: &str) -> SourceFile {
        SourceFile {
            sha256: hash.into(),
            modified_at_epoch: 0,
            size_bytes: 0,
        }
    }

    fn entry(hash: &str) -> Entry {
        Entry {
            plain_sha256: hash.into(),
            target_rel_path: "vault/x.gpg".into(),
            modified_at_epoch: 0,
            size_bytes: 0,
        }
    }

    fn kinds(actions: &[Action]) -> Vec<(&str, ActionKind)> {
        actions
            .iter()
            .map(|a| (a.rel_path.as_str(), a.kind))
            .collect()
    }

    #[test]
    fn classifies_all_four_cases() {
        let source = BTreeMap::from([
            ("new.md".to_string(), src("1")),
            ("changed.md".to_string(), src("2")),
            ("same.md".to_string(), src("3")),
        ]);
        let manifest = BTreeMap::from([
            ("changed.md".to_string(), entry("old")),
            ("same.md".to_string(), entry("3")),
            ("gone.md".to_string(), entry("4")),
        ]);
        assert_eq!(
            kinds(&compute(&source, &manifest)),
            vec![
                ("changed.md", ActionKind::Update),
                ("gone.md", ActionKind::Delete),
                ("new.md", ActionKind::Create),
                ("same.md", ActionKind::Skip),
            ]
        );
    }

    #[test]
    fn empty_inputs() {
        assert!(compute(&BTreeMap::new(), &BTreeMap::new()).is_empty());
        let only_manifest = BTreeMap::from([("a".to_string(), entry("1"))]);
        assert_eq!(
            kinds(&compute(&BTreeMap::new(), &only_manifest)),
            vec![("a", ActionKind::Delete)]
        );
        let only_source = BTreeMap::from([("a".to_string(), src("1"))]);
        assert_eq!(
            kinds(&compute(&only_source, &BTreeMap::new())),
            vec![("a", ActionKind::Create)]
        );
    }

    #[test]
    fn mtime_change_alone_is_a_skip() {
        let mut file = src("h");
        file.modified_at_epoch = 999;
        let source = BTreeMap::from([("a".to_string(), file)]);
        let manifest = BTreeMap::from([("a".to_string(), entry("h"))]);
        assert_eq!(compute(&source, &manifest)[0].kind, ActionKind::Skip);
    }
}
