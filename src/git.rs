//! Git operations through the system `git` binary (keeps SSH agent integration).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::Result;
use crate::exec;
use crate::fsutil;

const GITIGNORE: &str = ".gitignore";

#[derive(Debug, Clone)]
pub struct Git {
    dir: PathBuf,
}

impl Git {
    pub fn new(dir: &Path) -> Self {
        Git {
            dir: dir.to_path_buf(),
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.dir);
        cmd
    }

    pub fn is_repo(&self) -> bool {
        self.dir.join(".git").exists()
    }

    pub fn version() -> Result<String> {
        let out = exec::run(Command::new("git").arg("--version"))?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Initialises a repository on `branch` and writes a base `.gitignore` if absent.
    pub fn init(&self, branch: &str) -> Result<()> {
        exec::run(self.cmd().args(["init", "-b", branch]))?;
        let ignore = self.dir.join(GITIGNORE);
        if !ignore.exists() {
            fsutil::write_atomic(&ignore, b"*.tmp\n")?;
        }
        Ok(())
    }

    pub fn remote_url(&self, name: &str) -> Result<Option<String>> {
        let out = exec::run_unchecked(self.cmd().args(["remote", "get-url", name]))?;
        Ok(out
            .status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    /// Adds `origin` when it does not exist yet. Returns the existing URL if it differs.
    pub fn ensure_origin(&self, url: &str) -> Result<Option<String>> {
        match self.remote_url("origin")? {
            None => {
                exec::run(self.cmd().args(["remote", "add", "origin", url]))?;
                Ok(None)
            }
            Some(existing) if existing != url => Ok(Some(existing)),
            Some(_) => Ok(None),
        }
    }

    /// Checks that a remote URL is reachable.
    pub fn ls_remote(url: &str) -> Result<()> {
        exec::run(
            Command::new("git")
                .args(["ls-remote", "--quiet", url])
                .env("GIT_TERMINAL_PROMPT", "0"),
        )
        .map(drop)
    }

    pub fn has_changes(&self) -> Result<bool> {
        let out = exec::run(self.cmd().args(["status", "--porcelain"]))?;
        Ok(!out.stdout.iter().all(u8::is_ascii_whitespace))
    }

    pub fn add_all(&self) -> Result<()> {
        exec::run(self.cmd().args(["add", "-A"])).map(drop)
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        exec::run(self.cmd().args(["commit", "-m", message])).map(drop)
    }

    pub fn push(&self, branch: &str) -> Result<()> {
        exec::run(self.cmd().args(["push", "-u", "origin", branch])).map(drop)
    }
}
