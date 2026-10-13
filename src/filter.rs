//! Path filtering via glob patterns.

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::Filters;
use crate::error::{Error, Result};

/// Decides which vault-relative POSIX paths are excluded from the backup.
#[derive(Debug)]
pub struct PathFilter {
    exclude: GlobSet,
}

impl PathFilter {
    pub fn new(filters: &Filters) -> Result<Self> {
        let mut patterns: Vec<&str> = filters
            .exclude_patterns
            .iter()
            .map(String::as_str)
            .collect();
        if !filters.include_obsidian_config {
            patterns.push(".obsidian/**");
        }
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            let glob = Glob::new(pattern).map_err(|source| Error::Glob {
                pattern: pattern.to_string(),
                source,
            })?;
            builder.add(glob);
        }
        let exclude = builder.build().map_err(|source| Error::Glob {
            pattern: "<set>".into(),
            source,
        })?;
        Ok(PathFilter { exclude })
    }

    pub fn is_excluded(&self, rel_posix_path: &str) -> bool {
        self.exclude.is_match(rel_posix_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_filters(include_obsidian: bool) -> Filters {
        Filters {
            exclude_patterns: [
                ".git/**",
                ".trash/**",
                ".obsidian/workspace.json",
                ".obsidian/cache/**",
                "**/.DS_Store",
                "**/thumbs.db",
            ]
            .map(String::from)
            .to_vec(),
            include_obsidian_config: include_obsidian,
        }
    }

    #[test]
    fn excludes_configured_patterns() {
        let f = PathFilter::new(&default_filters(true)).unwrap();
        for excluded in [
            ".git/config",
            ".git/objects/ab/cdef",
            ".trash/old.md",
            ".obsidian/workspace.json",
            ".obsidian/cache/x/y",
            ".DS_Store",
            "Notes/.DS_Store",
            "a/b/c/thumbs.db",
        ] {
            assert!(f.is_excluded(excluded), "{excluded} should be excluded");
        }
    }

    #[test]
    fn keeps_regular_files() {
        let f = PathFilter::new(&default_filters(true)).unwrap();
        for kept in [
            "Notes/Daily.md",
            "Attachments/diagram.png",
            ".obsidian/app.json",
            "notes.git/readme.md",
            "workspace.json",
        ] {
            assert!(!f.is_excluded(kept), "{kept} should be kept");
        }
    }

    #[test]
    fn include_obsidian_config_false_excludes_whole_dir() {
        let f = PathFilter::new(&default_filters(false)).unwrap();
        assert!(f.is_excluded(".obsidian/app.json"));
        assert!(f.is_excluded(".obsidian/plugins/x/main.js"));
        assert!(!f.is_excluded("Notes/.obsidian.md"));
    }

    #[test]
    fn invalid_pattern_is_reported() {
        let filters = Filters {
            exclude_patterns: vec!["[".into()],
            include_obsidian_config: true,
        };
        assert!(matches!(PathFilter::new(&filters), Err(Error::Glob { .. })));
    }
}
