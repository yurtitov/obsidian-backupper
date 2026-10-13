---
name: cli-smoke-test
description: Exercise the built obsidian-backupper binary end to end in a throwaway environment (init, check, backup --dry-run) without touching the user's real config, vault or GPG keyring. Use to confirm a CLI-visible change works in the real binary, beyond unit tests.
---

# CLI smoke test

Never use the user's real config or vault. Isolate everything under a temp dir and point `XDG_CONFIG_HOME` at it:

```sh
. ~/.cargo/env && cargo build -q
B=$PWD/target/debug/obsidian-backupper
T=$(mktemp -d)
export XDG_CONFIG_HOME=$T/cfg

$B init                      # creates $T/cfg/obsidian-backupper/config.toml
$B init; echo "exit=$?"      # must fail: already exists (needs --force)
mkdir -p $T/v && echo hi > $T/v/a.md
sed -i "s|~/Documents/MyObsidianVault|$T/v|;s|~/Backups/ObsidianVaultEncrypted|$T/b|" "$XDG_CONFIG_HOME/obsidian-backupper/config.toml"
$B backup --dry-run          # table: CREATE a.md; must not create $T/b
ls $T/b 2>&1                 # expect "No such file"
$B check; echo "exit=$?"     # [FAIL] lines are expected: example recipient/remote do not exist
rm -rf $T
```

Expected behaviours to verify:
- `--dry-run` creates nothing on disk and never runs git or checks the GPG key.
- `check` exits non-zero when any line is `[FAIL]`.
- Errors print as `error: ...` on stderr with exit code 1.

A real encrypt/push cycle is covered by `tests/backup_test.rs` (see the `write-integration-test` skill); do not run a real `backup` against the user's actual vault or remote while testing.
