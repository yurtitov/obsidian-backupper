//! Configuration loading, resolution and path expansion.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

pub const CONFIG_ENV_VAR: &str = "OBSIDIAN_BACKUPPER_CONFIG";
pub const LOCAL_CONFIG_NAME: &str = ".obsidian-backupper.toml";

pub const EXAMPLE_CONFIG: &str = r#"[paths]
source_dir = "~/Documents/MyObsidianVault"
target_dir = "~/Backups/ObsidianVaultEncrypted"

[encryption]
recipient = "user@example.com"
armor = false
gpg_binary = "gpg"

[git]
auto_push = true
remote_url = "git@github.com:username/obsidian-vault-encrypted.git"
branch = "main"
commit_message = "chore(backup): auto-sync {datetime}"

[filters]
exclude_patterns = [
  ".git/**",
  ".trash/**",
  ".obsidian/workspace.json",
  ".obsidian/cache/**",
  "**/.DS_Store",
  "**/thumbs.db"
]
include_obsidian_config = true
"#;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub paths: Paths,
    pub encryption: Encryption,
    #[serde(default)]
    pub git: Git,
    #[serde(default)]
    pub filters: Filters,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Paths {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Encryption {
    pub recipient: String,
    #[serde(default)]
    pub armor: bool,
    #[serde(default = "default_gpg_binary")]
    pub gpg_binary: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Git {
    #[serde(default)]
    pub auto_push: bool,
    pub remote_url: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_commit_message")]
    pub commit_message: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Filters {
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_true")]
    pub include_obsidian_config: bool,
}

fn default_gpg_binary() -> String {
    "gpg".into()
}

fn default_branch() -> String {
    "main".into()
}

fn default_commit_message() -> String {
    "chore(backup): auto-sync {datetime}".into()
}

fn default_true() -> bool {
    true
}

impl Default for Git {
    fn default() -> Self {
        Git {
            auto_push: false,
            remote_url: None,
            branch: default_branch(),
            commit_message: default_commit_message(),
        }
    }
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            exclude_patterns: Vec::new(),
            include_obsidian_config: true,
        }
    }
}

impl Config {
    /// Parses a config from TOML text and expands `~` in its paths.
    pub fn parse(text: &str, origin: &Path, home: Option<&Path>) -> Result<Config> {
        let mut config: Config = toml::from_str(text).map_err(|source| Error::ConfigParse {
            path: origin.to_path_buf(),
            source,
        })?;
        config.paths.source_dir = expand_tilde(&config.paths.source_dir, home)?;
        config.paths.target_dir = expand_tilde(&config.paths.target_dir, home)?;
        config.validate()?;
        Ok(config)
    }

    /// Reads and parses the config file at `path`.
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Config::parse(&text, path, dirs::home_dir().as_deref())
    }

    fn validate(&self) -> Result<()> {
        if self.encryption.recipient.trim().is_empty() {
            return Err(Error::ConfigInvalid(
                "encryption.recipient must not be empty".into(),
            ));
        }
        if self.git.branch.trim().is_empty() {
            return Err(Error::ConfigInvalid("git.branch must not be empty".into()));
        }
        if self.paths.source_dir == self.paths.target_dir {
            return Err(Error::ConfigInvalid(
                "paths.source_dir and paths.target_dir must differ".into(),
            ));
        }
        Ok(())
    }
}

/// Expands a leading `~` or `~/` using `home`. Other paths are returned unchanged.
pub fn expand_tilde(path: &Path, home: Option<&Path>) -> Result<PathBuf> {
    let Ok(rest) = path.strip_prefix("~") else {
        return Ok(path.to_path_buf());
    };
    let home = home.ok_or(Error::NoHomeDir)?;
    Ok(home.join(rest))
}

/// Candidate config locations from the system directories, in priority order.
pub fn system_config_candidates(
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    const REL: &str = "obsidian-backupper/config.toml";
    let mut out = Vec::new();
    if cfg!(target_os = "macos")
        && let Some(home) = home
    {
        out.push(home.join("Library/Application Support").join(REL));
    }
    match (xdg_config_home.filter(|p| p.is_absolute()), home) {
        (Some(xdg), _) => out.push(xdg.join(REL)),
        (None, Some(home)) => out.push(home.join(".config").join(REL)),
        (None, None) => {}
    }
    out
}

/// Resolves the config file path: `--config`, then the env var, then system dirs,
/// then `.obsidian-backupper.toml` in the current directory.
pub fn resolve_config_path(cli_path: Option<&Path>) -> Result<PathBuf> {
    let env_path = std::env::var_os(CONFIG_ENV_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = dirs::home_dir();
    let cwd = std::env::current_dir().map_err(|e| Error::io(".", e))?;
    resolve_from(
        cli_path,
        env_path.as_deref(),
        &system_config_candidates(xdg.as_deref(), home.as_deref()),
        &cwd.join(LOCAL_CONFIG_NAME),
    )
}

fn resolve_from(
    cli_path: Option<&Path>,
    env_path: Option<&Path>,
    system: &[PathBuf],
    local: &Path,
) -> Result<PathBuf> {
    if let Some(explicit) = cli_path.or(env_path) {
        return if explicit.is_file() {
            Ok(explicit.to_path_buf())
        } else {
            Err(Error::ConfigMissing(explicit.to_path_buf()))
        };
    }
    let mut searched = Vec::new();
    for candidate in system.iter().map(PathBuf::as_path).chain([local]) {
        if candidate.is_file() {
            return Ok(candidate.to_path_buf());
        }
        searched.push(candidate.display().to_string());
    }
    Err(Error::ConfigNotFound(searched.join(", ")))
}

/// Where `init` writes a new config when no explicit path is given.
pub fn default_config_path() -> Result<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = dirs::home_dir();
    system_config_candidates(xdg.as_deref(), home.as_deref())
        .into_iter()
        .last()
        .ok_or(Error::NoHomeDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/tester";

    #[test]
    fn parses_example_config_and_expands_tilde() {
        let cfg = Config::parse(EXAMPLE_CONFIG, Path::new("c.toml"), Some(Path::new(HOME)))
            .expect("example config must parse");
        assert_eq!(
            cfg.paths.source_dir,
            PathBuf::from("/home/tester/Documents/MyObsidianVault")
        );
        assert_eq!(
            cfg.paths.target_dir,
            PathBuf::from("/home/tester/Backups/ObsidianVaultEncrypted")
        );
        assert_eq!(cfg.encryption.recipient, "user@example.com");
        assert!(!cfg.encryption.armor);
        assert!(cfg.git.auto_push);
        assert_eq!(cfg.git.branch, "main");
        assert_eq!(cfg.filters.exclude_patterns.len(), 6);
        assert!(cfg.filters.include_obsidian_config);
    }

    #[test]
    fn applies_defaults_for_optional_sections() {
        let text = r#"
            [paths]
            source_dir = "/a"
            target_dir = "/b"
            [encryption]
            recipient = "me"
        "#;
        let cfg = Config::parse(text, Path::new("c.toml"), None).unwrap();
        assert_eq!(cfg.encryption.gpg_binary, "gpg");
        assert_eq!(cfg.git, Git::default());
        assert_eq!(cfg.filters, Filters::default());
    }

    #[test]
    fn rejects_unknown_structure_and_bad_values() {
        let missing = "[paths]\nsource_dir = \"/a\"\n";
        assert!(matches!(
            Config::parse(missing, Path::new("c.toml"), None),
            Err(Error::ConfigParse { .. })
        ));
        let same = "[paths]\nsource_dir=\"/a\"\ntarget_dir=\"/a\"\n[encryption]\nrecipient=\"x\"\n";
        assert!(matches!(
            Config::parse(same, Path::new("c.toml"), None),
            Err(Error::ConfigInvalid(_))
        ));
        let empty =
            "[paths]\nsource_dir=\"/a\"\ntarget_dir=\"/b\"\n[encryption]\nrecipient=\" \"\n";
        assert!(matches!(
            Config::parse(empty, Path::new("c.toml"), None),
            Err(Error::ConfigInvalid(_))
        ));
    }

    #[test]
    fn tilde_expansion_rules() {
        let home = Some(Path::new(HOME));
        assert_eq!(
            expand_tilde(Path::new("~"), home).unwrap(),
            PathBuf::from(HOME)
        );
        assert_eq!(
            expand_tilde(Path::new("~/x/y"), home).unwrap(),
            PathBuf::from("/home/tester/x/y")
        );
        assert_eq!(
            expand_tilde(Path::new("/abs/~/x"), home).unwrap(),
            PathBuf::from("/abs/~/x")
        );
        assert_eq!(
            expand_tilde(Path::new("~user/x"), home).unwrap(),
            PathBuf::from("~user/x")
        );
        assert!(matches!(
            expand_tilde(Path::new("~/x"), None),
            Err(Error::NoHomeDir)
        ));
    }

    #[test]
    fn system_candidates_respect_xdg() {
        let with_xdg = system_config_candidates(Some(Path::new("/xdg")), Some(Path::new(HOME)));
        assert_eq!(
            with_xdg.last().unwrap(),
            &PathBuf::from("/xdg/obsidian-backupper/config.toml")
        );
        let without = system_config_candidates(None, Some(Path::new(HOME)));
        assert_eq!(
            without.last().unwrap(),
            &PathBuf::from("/home/tester/.config/obsidian-backupper/config.toml")
        );
    }

    #[test]
    fn resolution_priority() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, "").unwrap();
            p
        };
        let cli = write("cli.toml");
        let env = write("env.toml");
        let sys = write("sys.toml");
        let local = write("local.toml");
        let system = [dir.path().join("absent.toml"), sys.clone()];

        let pick = |c: Option<&Path>, e: Option<&Path>| resolve_from(c, e, &system, &local);
        assert_eq!(pick(Some(&cli), Some(&env)).unwrap(), cli);
        assert_eq!(pick(None, Some(&env)).unwrap(), env);
        assert_eq!(pick(None, None).unwrap(), sys);
        assert_eq!(resolve_from(None, None, &[], &local).unwrap(), local);
        assert!(matches!(
            pick(Some(&dir.path().join("nope.toml")), None),
            Err(Error::ConfigMissing(_))
        ));
        assert!(matches!(
            resolve_from(None, None, &[], &dir.path().join("nope")),
            Err(Error::ConfigNotFound(_))
        ));
    }
}
