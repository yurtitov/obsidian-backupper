---
name: add-config-option
description: Checklist for adding or changing a config.toml option in obsidian-backupper so code, defaults, example config, tests and docs stay in sync. Use whenever a configuration key is added, renamed or removed.
---

# Add a config option

A config key touches several places; missing one causes silent drift. Do all of these:

1. **`src/config.rs`**: add the field to the right struct (`Paths`, `Encryption`, `Git`, `Filters`). Use `#[serde(default)]` or `#[serde(default = "fn")]` so existing config files keep working. Update the manual `Default` impls (`Git`, `Filters`) to match. Add validation in `Config::validate` if the value can be invalid.
2. **`EXAMPLE_CONFIG`** (same file): keep it a valid config; `init` writes it and the test `parses_example_config_and_expands_tilde` parses it.
3. **Behaviour**: wire the option into the module that uses it (`backup.rs`, `restore.rs`, `check.rs`, `filter.rs`). If it changes what ends up in the repo, remember `--dry-run` must still not touch disk or Git.
4. **Tests**: extend `config.rs` tests (parsing, default, invalid value) and the relevant module or `tests/backup_test.rs` test. The integration test builds its config from a TOML string in `setup()`; add the key there if the test needs it.
5. **Docs**: update the options table and any prose in `README.md`, and `docs/tech-task.md` only if the user asks (it is the user's spec). Mention behavioural changes in `CLAUDE.md` under "Non-obvious behaviours".
6. Run the `verify-changes` skill.

Rules to preserve:
- Unknown optional sections must default sensibly; required keys are only `paths.*` and `encryption.recipient`.
- `~` is expanded only for `paths.*` (via `expand_tilde`, with `home` injected so tests do not depend on the real home).
- All project text (code, comments, docs, commit messages) is English.
