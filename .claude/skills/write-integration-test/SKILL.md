---
name: write-integration-test
description: How to write end-to-end tests for obsidian-backupper that use real gpg and git safely, with a throwaway keyring and a local bare remote. Use when adding or changing tests in tests/ that exercise encryption, restore or git sync.
---

# Writing integration tests with real gpg and git

Reuse the harness in `tests/backup_test.rs` (`setup()`); it already solves the awkward parts:

- **Isolated keyring without mutating process env** (`std::env::set_var` is `unsafe` in edition 2024 and races between tests): create a `GNUPGHOME` dir with mode `0700` and write a wrapper script that exports it and `exec gpg "$@"`. Pass the script as `encryption.gpg_binary`. Generate the key through the same wrapper:
  `--batch --passphrase "" --quick-generate-key test@example.com default default never`.
- **Local remote instead of GitHub**: `git init --bare -b main remote.git`, and use its path as `git.remote_url`.
- **Git identity**: the tool never sets one. Pre-create the target repo (`git init -b main`) and set a repo-local `user.name` / `user.email`; the tool skips `git init` when `.git` exists and adds `origin` itself.
- **Config**: build it with `Config::parse(&toml, Path::new("test.toml"), None)` from a TOML string with absolute temp paths.
- **Skipping**: return early if `gpg --version` or `git --version` fails, so the suite stays green on machines without them.
- Keep the temp dir path short (`tempfile::tempdir()`); long paths can break the gpg-agent socket.

Useful assertions:
- Non-determinism: after a second `backup` with no changes, the ciphertext bytes are identical and the remote commit count (`git rev-list --count main` on the bare repo) did not grow. This is the core guarantee of the tool.
- Filesystem hygiene: no `*.tmp` leftovers, empty dirs under `vault/` pruned.
- Restore: decrypt into a fresh dir and compare contents; a differing existing file is refused without `force`.

Tests that call library functions (`backup::run`, `restore::run`) need no binary build; keep CLI-level behaviour in the `cli-smoke-test` skill instead.
