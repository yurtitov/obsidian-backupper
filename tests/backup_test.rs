//! End-to-end backup/restore test using a throwaway GnuPG home and a local bare Git remote.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use obsidian_backupper::backup::{self, Options};
use obsidian_backupper::config::Config;
use obsidian_backupper::diff::ActionKind;
use obsidian_backupper::manifest::Manifest;
use obsidian_backupper::restore;

const RECIPIENT: &str = "backupper-test@example.com";

fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn sh(program: &str, args: &[&str], cwd: &Path) {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Env {
    _tmp: tempfile::TempDir,
    source: PathBuf,
    target: PathBuf,
    remote: PathBuf,
    config: Config,
}

/// Builds a temp environment: gpg wrapper bound to a fresh keyring, bare remote, git identity.
fn setup() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let (source, target, remote, gnupg) = (
        root.join("vault-src"),
        root.join("backup"),
        root.join("remote.git"),
        root.join("gnupg"),
    );
    for dir in [&source, &target, &remote, &gnupg] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::set_permissions(&gnupg, fs::Permissions::from_mode(0o700)).unwrap();

    let wrapper = root.join("gpg-wrapper.sh");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexport GNUPGHOME='{}'\nexec gpg \"$@\"\n",
            gnupg.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    sh(
        wrapper.to_str().unwrap(),
        &[
            "--batch",
            "--passphrase",
            "",
            "--quick-generate-key",
            RECIPIENT,
            "default",
            "default",
            "never",
        ],
        root,
    );

    sh(
        "git",
        &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
        root,
    );
    sh("git", &["init", "-b", "main"], &target);
    sh("git", &["config", "user.name", "Test"], &target);
    sh("git", &["config", "user.email", RECIPIENT], &target);

    let toml = format!(
        r#"
[paths]
source_dir = "{source}"
target_dir = "{target}"
[encryption]
recipient = "{RECIPIENT}"
gpg_binary = "{wrapper}"
[git]
auto_push = true
remote_url = "{remote}"
branch = "main"
[filters]
exclude_patterns = [".trash/**", "**/.DS_Store"]
"#,
        source = source.display(),
        target = target.display(),
        wrapper = wrapper.display(),
        remote = remote.display(),
    );
    let config = Config::parse(&toml, Path::new("test.toml"), None).unwrap();
    Env {
        _tmp: tmp,
        source,
        target,
        remote,
        config,
    }
}

fn write(base: &Path, rel: &str, body: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn kinds(actions: &[obsidian_backupper::diff::Action]) -> Vec<(String, ActionKind)> {
    actions
        .iter()
        .map(|a| (a.rel_path.clone(), a.kind))
        .collect()
}

fn commit_count(remote: &Path) -> usize {
    let out = Command::new("git")
        .args([
            "-C",
            remote.to_str().unwrap(),
            "rev-list",
            "--count",
            "main",
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

#[test]
fn full_backup_and_restore_cycle() {
    if !tool_available("gpg") || !tool_available("git") {
        eprintln!("skipping: gpg or git not installed");
        return;
    }
    let env = setup();
    write(&env.source, "Notes/Daily.md", "daily v1");
    write(&env.source, "Attachments/diagram.png", "png-bytes");
    write(&env.source, ".trash/old.md", "trashed");
    write(&env.source, "Notes/.DS_Store", "junk");

    // Dry run touches nothing.
    let plan = backup::run(&env.config, Options { dry_run: true }).unwrap();
    assert_eq!(
        kinds(&plan),
        vec![
            ("Attachments/diagram.png".into(), ActionKind::Create),
            ("Notes/Daily.md".into(), ActionKind::Create),
        ]
    );
    assert!(!env.target.join(".sync_manifest.json").exists());
    assert_eq!(commit_count(&env.remote), 0);

    // First backup: encrypted files, manifest, one pushed commit.
    backup::run(&env.config, Options::default()).unwrap();
    let encrypted = env.target.join("vault/Notes/Daily.md.gpg");
    assert!(encrypted.is_file());
    assert_ne!(fs::read(&encrypted).unwrap(), b"daily v1");
    assert!(!env.target.join("vault/.trash").exists());
    let manifest = Manifest::load(&env.target).unwrap();
    assert_eq!(manifest.entries.len(), 2);
    assert_eq!(commit_count(&env.remote), 1);

    // Nothing changed: all SKIP, no new commit, ciphertext untouched.
    let before = fs::read(&encrypted).unwrap();
    let plan = backup::run(&env.config, Options::default()).unwrap();
    assert!(plan.iter().all(|a| a.kind == ActionKind::Skip));
    assert_eq!(fs::read(&encrypted).unwrap(), before);
    assert_eq!(commit_count(&env.remote), 1);

    // Update one file, delete another, add a third.
    write(&env.source, "Notes/Daily.md", "daily v2");
    fs::remove_file(env.source.join("Attachments/diagram.png")).unwrap();
    write(&env.source, "Inbox/new.md", "new");
    let plan = backup::run(&env.config, Options::default()).unwrap();
    assert_eq!(
        kinds(&plan),
        vec![
            ("Attachments/diagram.png".into(), ActionKind::Delete),
            ("Inbox/new.md".into(), ActionKind::Create),
            ("Notes/Daily.md".into(), ActionKind::Update),
        ]
    );
    assert!(
        !env.target.join("vault/Attachments").exists(),
        "empty dirs are pruned"
    );
    assert_eq!(commit_count(&env.remote), 2);
    assert!(
        fs::read_dir(env.target.join("vault/Notes"))
            .unwrap()
            .all(|e| { !e.unwrap().file_name().to_string_lossy().ends_with(".tmp") })
    );

    // Restore into a fresh directory and compare contents.
    let restored = env.target.parent().unwrap().join("restored");
    let count = restore::run(&env.config, &restored, restore::Options::default()).unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        fs::read_to_string(restored.join("Notes/Daily.md")).unwrap(),
        "daily v2"
    );
    assert_eq!(
        fs::read_to_string(restored.join("Inbox/new.md")).unwrap(),
        "new"
    );
    assert!(!restored.join("Attachments/diagram.png").exists());

    // Restoring over a modified file is refused without --force.
    write(&restored, "Notes/Daily.md", "local edit");
    let err = restore::run(&env.config, &restored, restore::Options::default()).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    restore::run(&env.config, &restored, restore::Options { force: true }).unwrap();
    assert_eq!(
        fs::read_to_string(restored.join("Notes/Daily.md")).unwrap(),
        "daily v2"
    );
}

#[test]
fn backup_fails_for_unknown_recipient() {
    if !tool_available("gpg") || !tool_available("git") {
        return;
    }
    let mut env = setup();
    write(&env.source, "a.md", "a");
    env.config.encryption.recipient = "nobody@example.invalid".into();
    let err = backup::run(&env.config, Options::default()).unwrap_err();
    assert!(err.to_string().contains("was not found"), "{err}");
    assert!(!env.target.join(".sync_manifest.json").exists());
}
