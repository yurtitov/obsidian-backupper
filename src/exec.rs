//! Thin wrapper around `std::process::Command` with typed errors.

use std::process::{Command, Output};

use crate::error::{Error, Result};

/// Runs a command to completion and fails with the captured stderr on a non-zero exit.
pub fn run(cmd: &mut Command) -> Result<Output> {
    let output = run_unchecked(cmd)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(Error::Command {
            command: describe(cmd),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Runs a command to completion and returns its output regardless of the exit status.
pub fn run_unchecked(cmd: &mut Command) -> Result<Output> {
    cmd.output().map_err(|source| Error::Spawn {
        program: cmd.get_program().to_string_lossy().into_owned(),
        source,
    })
}

fn describe(cmd: &Command) -> String {
    let mut parts = vec![cmd.get_program().to_string_lossy().into_owned()];
    parts.extend(cmd.get_args().map(|a| a.to_string_lossy().into_owned()));
    parts.join(" ")
}
