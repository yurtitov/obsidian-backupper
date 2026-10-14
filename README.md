# obsidian-backupper

A command-line tool that makes incremental, GnuPG-encrypted backups of an [Obsidian](https://obsidian.md) vault and pushes them to a private Git repository (for example on GitHub).

- **Encrypted at rest.** Every note and attachment is encrypted with GnuPG to your public key before it touches the Git repository.
- **Incremental.** Only files whose content changed are re-encrypted. Because GnuPG output is different on every run, change detection uses the SHA-256 of the *plaintext* file, so unchanged files never produce spurious commits.
- **Config-first.** All settings live in one TOML file; `obsidian-backupper backup` needs no arguments.
- **Safe by default.** Every file write is atomic, `restore` verifies checksums and refuses to overwrite your local edits, and `--dry-run` shows exactly what would happen.

Supported platforms: Linux and macOS.

## Table of contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Commands](#commands)
- [How it works](#how-it-works)
- [Backup repository layout](#backup-repository-layout)
- [Restoring](#restoring)
- [Scheduling](#scheduling)
- [Security considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [License](#license)

## Requirements

- Rust (stable, edition 2024) to build from source.
- `gpg` (GnuPG 2.x) with a public key for the recipient you want to encrypt to.
- `git`, and credentials for the remote (an SSH key loaded in `ssh-agent` is recommended).
- An empty **private** repository on your Git host.

`gpg` and `git` are executed as external programs, so the tool automatically uses your `gpg-agent` (including passphrase prompts via pinentry) and your SSH agent.

## Installation

### Prebuilt binary (Linux and macOS)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/yurtitov/obsidian-backupper/releases/latest/download/obsidian-backupper-installer.sh | sh
```

The installer detects your OS and CPU architecture (Linux/macOS, x86_64/aarch64), downloads the matching archive from the latest GitHub Release, verifies it and installs `obsidian-backupper` into `~/.cargo/bin`. Make sure that directory is in your `PATH`. You still need `gpg` and `git` installed.

### From source

```sh
git clone <this repository>
cd obsidian-backupper
cargo install --path .
```

This installs the `obsidian-backupper` binary into `~/.cargo/bin`.

### Publishing a release (maintainers)

Releases are built by [cargo-dist](https://github.com/axodotdev/cargo-dist) via `.github/workflows/release.yml`. Bump `version` in `Cargo.toml`, commit, then push a matching tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds all targets, creates the GitHub Release and uploads the archives, checksums and the installer script. After changing `dist-workspace.toml`, regenerate the workflow with `dist generate`.

## Quick start

1. Create a GPG key if you do not have one (`gpg --full-generate-key`) and note the email or key ID.
2. Create a private, empty repository on GitHub and make sure `git ls-remote <url>` works from your machine.
3. Generate a config file and edit it:

   ```sh
   obsidian-backupper init
   $EDITOR ~/.config/obsidian-backupper/config.toml
   ```

4. Verify the environment:

   ```sh
   obsidian-backupper check
   ```

5. Preview, then run the first backup:

   ```sh
   obsidian-backupper backup --dry-run
   obsidian-backupper backup
   ```

## Configuration

### Where the config file is looked up

The first match wins:

1. `--config <PATH>` on the command line.
2. The `OBSIDIAN_BACKUPPER_CONFIG` environment variable.
3. The system config directory:
   - Linux: `$XDG_CONFIG_HOME/obsidian-backupper/config.toml` (default `~/.config/obsidian-backupper/config.toml`).
   - macOS: `~/Library/Application Support/obsidian-backupper/config.toml`, then `~/.config/obsidian-backupper/config.toml`.
4. `.obsidian-backupper.toml` in the current working directory.

If a path given by option 1 or 2 does not exist, the command fails instead of falling through to the next option.

`obsidian-backupper init` writes an example file to the last location of step 3 (or to `--config <PATH>`), and refuses to overwrite an existing file unless `--force` is passed.

### Reference

```toml
[paths]
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
```

| Key | Required | Default | Description |
| --- | --- | --- | --- |
| `paths.source_dir` | yes | | The Obsidian vault to back up. Must exist. A leading `~` is expanded. |
| `paths.target_dir` | yes | | Local working copy of the backup repository. Created if missing. Must differ from `source_dir`. |
| `encryption.recipient` | yes | | Email, key ID or fingerprint of the GPG public key to encrypt to. |
| `encryption.armor` | no | `false` | Produce ASCII-armored output. Files still use the `.gpg` extension. |
| `encryption.gpg_binary` | no | `gpg` | Name or path of the GnuPG executable. |
| `git.auto_push` | no | `false` | When `true`, commit and push after a successful backup. When `false`, no Git command is run on the working copy after the backup (you can still commit manually). |
| `git.remote_url` | no | none | URL for the `origin` remote. Added automatically if `origin` does not exist. |
| `git.branch` | no | `main` | Branch to create and push. |
| `git.commit_message` | no | `chore(backup): auto-sync {datetime}` | Commit message. `{datetime}` is replaced with the current UTC time, e.g. `2026-10-13T12:30:00Z`. |
| `filters.exclude_patterns` | no | empty | Glob patterns, matched against vault-relative paths with `/` separators. |
| `filters.include_obsidian_config` | no | `true` | When `false`, the whole `.obsidian/` directory is excluded. |

### Exclude patterns

Patterns use [globset](https://docs.rs/globset) syntax and are matched against the path relative to `source_dir`, always with `/` separators:

- With the default globset semantics, `*` also matches `/`, so `*` and `**` behave alike; prefer `**` for clarity.
- `.trash/**` excludes everything under `.trash/`.
- `**/.DS_Store` matches a `.DS_Store` file at any depth, including the vault root.

Symbolic links are never followed or backed up.

## Commands

All commands accept `--config <PATH>`.

### `obsidian-backupper init [--force]`

Writes an example config file. See [Configuration](#configuration).

### `obsidian-backupper backup [--dry-run] [--verbose]`

Encrypts new and changed files, removes backups of deleted files, and (if `git.auto_push = true`) commits and pushes.

- `--dry-run` scans the vault and prints a table of planned actions without changing anything on disk, without creating `target_dir`, and without running Git or checking the GPG key.
- `--verbose` enables debug logging (one line per processed file).

Example dry-run output:

```
ACTION   PATH
CREATE   Inbox/new.md
DELETE   Attachments/old.png
SKIP     Notes/Archive.md
UPDATE   Notes/Daily.md

1 create, 1 update, 1 delete, 1 skip
```

### `obsidian-backupper restore [--target <DIR>] [--force]`

Decrypts every file listed in the manifest into `--target` (default: `paths.source_dir`). See [Restoring](#restoring).

### `obsidian-backupper check`

Checks that:

- the `gpg` binary runs and a public key exists for `recipient`;
- `source_dir` exists and `target_dir` exists or can be created;
- `git` is installed;
- if `git.remote_url` is set: the existing `origin` matches it, and the remote is reachable (`git ls-remote`).

Prints one `[ ok ]` or `[FAIL]` line per check and exits with a non-zero status if any check failed.

### Exit status

`0` on success; non-zero if any error occurred (errors are printed to stderr as `error: ...`).

## How it works

A backup run performs these steps:

1. **Prepare.** Expand `~`, verify `source_dir`, create `target_dir`, run `git init` and add `origin` if needed (and write a `.gitignore` containing `*.tmp`), and verify that the recipient key exists.
2. **Scan.** Walk `source_dir`, drop excluded paths, and compute the SHA-256 of each file.
3. **Load the manifest** (`.sync_manifest.json` in `target_dir`; a missing file means an empty manifest).
4. **Diff.** Classify every path:

   | Action | Condition |
   | --- | --- |
   | `CREATE` | in the vault, not in the manifest |
   | `UPDATE` | in both, SHA-256 differs |
   | `DELETE` | in the manifest, no longer in the vault |
   | `SKIP` | in both, SHA-256 identical |

   Modification time and size are recorded in the manifest but never used for the decision, so touching a file without changing it does nothing.
5. **Apply.**
   - `CREATE`/`UPDATE`: run `gpg --batch --yes --encrypt -r <recipient> -o <file>.gpg.tmp <source>`, then atomically rename the `.tmp` file into place.
   - `DELETE`: remove the encrypted file and prune empty directories under `vault/`.
6. **Save.** Atomically rewrite the manifest.
7. **Sync Git** (only if `git.auto_push = true`): `git add -A`; if `git status --porcelain` is empty, stop without committing; otherwise commit and `git push -u origin <branch>`.

### Error handling

- If encrypting or deleting one file fails, the remaining files are still processed, the manifest is saved with everything that succeeded, Git sync is skipped, and the command exits with an error that reports how many operations failed. Re-running `backup` retries only what is still outstanding.
- If the push fails, the commit stays in the local working copy; fix the problem (network, credentials) and run `backup` again, or `git push` manually in `target_dir`.
- A corrupt manifest, or one with an unsupported `version`, aborts the run before anything is modified.

## Backup repository layout

```
target_dir/
├── .git/
├── .gitignore               # contains "*.tmp"
├── .sync_manifest.json
└── vault/
    ├── Notes/
    │   └── Daily.md.gpg
    └── Attachments/
        └── diagram.png.gpg
```

`.sync_manifest.json` is committed alongside the encrypted files:

```json
{
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
}
```

Do not edit the manifest by hand. Deleting it makes the next backup re-encrypt everything (which is safe, but creates a large commit).

## Restoring

```sh
obsidian-backupper restore --target ~/Restored
```

- The restore reads the manifest, decrypts each file with your private key (your `gpg-agent` will ask for the passphrase), and verifies the result against the recorded SHA-256 before moving it into place. A mismatch aborts the restore.
- Before decrypting anything, restore checks the destination. A file that already exists with identical content is left alone; a file with different content makes the command fail without changes, unless `--force` is given.
- Without `--target`, files are restored into `paths.source_dir`, i.e. on top of the live vault. Use a separate directory first if you are unsure.

### Restoring on a new machine

1. Install the tool, `gpg` and `git`; import your **private** key (`gpg --import`).
2. Clone the backup repository to the `target_dir` location from your config.
3. Run `obsidian-backupper restore --target <DIR>`.

You can also decrypt a single file without this tool: `gpg --decrypt vault/Notes/Daily.md.gpg > Daily.md`.

### Restoring with a YubiKey (touch policy)

Every file is encrypted separately, so `restore` performs one decryption on the card per file. If the decryption key has the touch policy `on`, you must touch the YubiKey for every single file. To avoid this, set the touch policy of the decryption key to `cached`:

```bash
ykman openpgp keys set-touch dec cached
```

A single touch is then accepted for 15 seconds, so a restore of many small notes needs only a few touches. Notes:

- If the restore runs longer than 15 seconds, you will be asked to touch again once the window expires.
- While the cache is active, any process that can reach your `gpg-agent` can decrypt without a new touch. Set the policy back with `ykman openpgp keys set-touch dec on` after restoring if you want per-operation confirmation again.
- `cached` requires a YubiKey 4 or 5; check the current policy with `ykman openpgp info`.

## Scheduling

The command is non-interactive as long as `gpg-agent` has your passphrase cached or the key has no passphrase, and SSH credentials are available without prompting.

Cron example (hourly):

```
0 * * * * /home/me/.cargo/bin/obsidian-backupper backup >> /home/me/.local/state/obsidian-backupper.log 2>&1
```

For cron or launchd, make sure the job environment can reach `gpg-agent` and `ssh-agent` (for example `SSH_AUTH_SOCK`), otherwise the job will fail on the push or on decryption prompts.

## Security considerations

Encryption protects **file contents** only. Be aware that the backup repository exposes:

- **File and folder names**, and the directory structure of your vault (both in `vault/` and in the manifest).
- **Approximate file sizes** and modification times (the manifest records them).
- **SHA-256 hashes of the plaintext files** in the manifest. Anyone who can guess a file's exact content can confirm the guess by hashing it.
- **When you change things**, through commit timestamps.

Other points:

- Keep the repository private, and keep your GPG private key and its backup safe. Without the private key the backup cannot be restored.
- Git history retains every previously committed version of an encrypted file, including files you later deleted from the vault. Deleting a note removes it from the current tree only.
- Encryption uses `recipient`'s public key only; the tool never handles your passphrase or private key.

## Troubleshooting

| Symptom | Likely cause and fix |
| --- | --- |
| `no config file found; searched: ...` | Run `obsidian-backupper init`, or pass `--config`. |
| `GPG key for recipient "..." was not found` | The key is not in the keyring used by this user/environment. Check `gpg --list-keys <recipient>`. |
| `source directory ... does not exist` | `paths.source_dir` is wrong, or `~` points to a different home than you expect. |
| Commit fails with "Please tell me who you are" | Set `git config --global user.name` and `user.email`; the tool does not configure a Git identity. |
| Push is rejected or asks for credentials | Load your SSH key into `ssh-agent`, and verify with `obsidian-backupper check`. |
| `git remote 'origin' is X, which differs from configured Y` (warning) | `origin` already exists in `target_dir` with another URL. The existing remote is kept; fix with `git remote set-url origin <url>`. |
| `refusing to overwrite existing file ... (use --force)` | A local file differs from the backup. Restore into an empty `--target`, or pass `--force`. |
| `restore` asks to touch the YubiKey for every file | The touch policy of the decryption key is `on`. See [Restoring with a YubiKey](#restoring-with-a-yubikey-touch-policy). |
| Decryption in cron fails | `gpg-agent` has no cached passphrase and no terminal. Use a cached passphrase or a dedicated key without one. |

## Development

```sh
cargo build
cargo test                                   # unit + integration tests
cargo test --test backup_test                # end-to-end backup/restore only
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

The integration test generates a throwaway GPG keyring and uses a local bare Git repository as the remote, so it needs `gpg` and `git` installed but no network access or personal keys. It skips itself if either is missing.

Code layout (`src/`):

| Module | Responsibility |
| --- | --- |
| `cli`, `main` | Argument parsing and command dispatch |
| `config` | Config discovery, parsing, `~` expansion |
| `filter`, `scan` | Exclude globs, vault walk and hashing |
| `diff` | Pure CREATE/UPDATE/DELETE/SKIP computation |
| `manifest` | `.sync_manifest.json` model and atomic persistence |
| `backup`, `restore`, `check` | The commands |
| `gpg`, `git`, `exec` | External process wrappers |
| `fsutil`, `error` | Atomic writes, path safety; typed errors |

The behavioural specification is in [`docs/tech-task.md`](docs/tech-task.md) (in Russian).

## License

See [LICENSE](LICENSE).
