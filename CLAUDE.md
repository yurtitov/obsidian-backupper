# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Language rules

- Chat with the user in Russian.
- Everything saved in the project must be in English only: documentation, code comments, commit messages, identifiers, log/error messages, and any other artifacts.

## Commands

Rust is installed via rustup (`. ~/.cargo/env` if `cargo` is not on PATH).

- Build: `cargo build`
- Lint (must pass with no warnings): `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt -- --check`
- All tests: `cargo test`
- Single test: `cargo test <name>` (e.g. `cargo test full_backup_and_restore_cycle`, `cargo test --test backup_test`)

The spec is `docs/tech-task.md` (in Russian); it is the source of truth for behaviour and acceptance criteria.

## Architecture

A library crate (`src/lib.rs`) plus a thin binary (`src/main.rs`: clap dispatch, logging, `anyhow` context). Domain errors are one `thiserror` enum in `error.rs`.

`backup::run` is the core pipeline: `scan` (walkdir + `filter` globs, SHA-256 of every file) -> `Manifest::load` -> `diff::compute` (pure function: CREATE/UPDATE/DELETE/SKIP, decided only by plaintext SHA-256 because GPG output is non-deterministic) -> apply via `gpg` -> atomic manifest save -> `git` add/commit/push. `restore::run` reverses it using the manifest and verifies the plaintext hash after decrypting.

Non-obvious behaviours:
- `.sync_manifest.json` in `target_dir` is the only state; encrypted files live under `target_dir/vault/<rel>.gpg` (always `.gpg`, even with `armor = true`). Restore trusts the manifest's `target_rel_path`, after validating it with `fsutil::ensure_safe_rel`.
- All writes go through a sibling `.tmp` file + `rename` (`fsutil::tmp_path` / `write_atomic`).
- A failed file operation does not abort the run: remaining files are processed, the manifest is saved with the successes, then `Error::Partial` is returned and Git sync is skipped.
- `--dry-run` must not touch disk or Git (no `target_dir` creation, no `git init`, no key check).
- gpg and git are invoked as external binaries (`exec.rs`) to keep `gpg-agent` / SSH agent access; do not replace with library crates.
- `include_obsidian_config = false` adds `.obsidian/**` to the exclude globs. Symlinks are skipped; `target_dir` is skipped if nested inside `source_dir`.
- `restore` refuses to overwrite differing files unless `--force`; default destination is `paths.source_dir`.

## Testing notes

`tests/backup_test.rs` runs the real `gpg` and `git` against a throwaway keyring (a wrapper script passed as `gpg_binary` sets `GNUPGHOME`, so no env mutation is needed) and a local bare repo as the remote. It skips itself if `gpg`/`git` are missing. The test pre-creates the target repo with a local `user.name`/`user.email`, since the tool does not configure a Git identity.
