//! The `check` command: environment diagnostics.

use crate::config::Config;
use crate::git::Git;
use crate::gpg::Gpg;

/// Runs all checks, printing one line per check. Returns the number of failures.
pub fn run(config: &Config) -> usize {
    let mut failures = 0;
    let mut report = |name: &str, result: Result<String, String>| match result {
        Ok(detail) => println!("[ ok ] {name}: {detail}"),
        Err(detail) => {
            failures += 1;
            println!("[FAIL] {name}: {detail}");
        }
    };

    let gpg = Gpg::new(
        &config.encryption.gpg_binary,
        &config.encryption.recipient,
        config.encryption.armor,
    );
    report("gpg binary", gpg.version().map_err(|e| e.to_string()));
    report(
        "gpg recipient key",
        gpg.check_recipient()
            .map(|()| config.encryption.recipient.clone())
            .map_err(|e| e.to_string()),
    );

    let source = &config.paths.source_dir;
    report(
        "source_dir",
        if source.is_dir() {
            Ok(source.display().to_string())
        } else {
            Err(format!("{} is not a directory", source.display()))
        },
    );

    let target = &config.paths.target_dir;
    report(
        "target_dir",
        if target.is_dir() {
            Ok(target.display().to_string())
        } else if target.parent().is_some_and(|p| p.is_dir()) {
            Ok(format!("{} (will be created)", target.display()))
        } else {
            Err(format!("parent of {} does not exist", target.display()))
        },
    );

    report("git binary", Git::version().map_err(|e| e.to_string()));

    if let Some(url) = &config.git.remote_url {
        let git = Git::new(target);
        if git.is_repo() {
            report(
                "git origin",
                match git.remote_url("origin") {
                    Ok(Some(existing)) if &existing == url => Ok(existing),
                    Ok(Some(existing)) => Err(format!("is {existing}, config says {url}")),
                    Ok(None) => Ok("not set yet (will be added on backup)".into()),
                    Err(e) => Err(e.to_string()),
                },
            );
        }
        report(
            "git remote reachable",
            Git::ls_remote(url)
                .map(|()| url.clone())
                .map_err(|e| e.to_string()),
        );
    }
    failures
}
