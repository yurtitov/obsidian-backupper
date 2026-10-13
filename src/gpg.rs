//! GnuPG invocation through the system binary (keeps `gpg-agent` integration).

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};
use crate::exec;

#[derive(Debug, Clone)]
pub struct Gpg {
    binary: String,
    recipient: String,
    armor: bool,
}

impl Gpg {
    pub fn new(binary: &str, recipient: &str, armor: bool) -> Self {
        Gpg {
            binary: binary.to_string(),
            recipient: recipient.to_string(),
            armor,
        }
    }

    /// Verifies the gpg binary runs; returns its first version line.
    pub fn version(&self) -> Result<String> {
        let out = exec::run(Command::new(&self.binary).arg("--version"))?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string())
    }

    /// Verifies a public key for the recipient exists in the keyring.
    pub fn check_recipient(&self) -> Result<()> {
        let out = exec::run_unchecked(
            Command::new(&self.binary)
                .args(["--batch", "--list-keys"])
                .arg(&self.recipient),
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(Error::KeyNotFound(self.recipient.clone()))
        }
    }

    /// Encrypts `src` to `dest` for the configured recipient.
    pub fn encrypt(&self, src: &Path, dest: &Path) -> Result<()> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(["--batch", "--yes", "--encrypt"]);
        if self.armor {
            cmd.arg("--armor");
        }
        cmd.arg("-r")
            .arg(&self.recipient)
            .arg("-o")
            .arg(dest)
            .arg(src);
        exec::run(&mut cmd).map(drop)
    }

    /// Decrypts `src` to `dest`.
    pub fn decrypt(&self, src: &Path, dest: &Path) -> Result<()> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(["--batch", "--yes", "--decrypt", "-o"])
            .arg(dest)
            .arg(src);
        exec::run(&mut cmd).map(drop)
    }
}
