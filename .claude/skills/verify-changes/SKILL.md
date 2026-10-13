---
name: verify-changes
description: Run the project's Definition of Done checks (fmt, clippy with warnings denied, all tests) before committing or reporting work as finished. Use after any change to Rust code in this repo.
---

# Verify changes

Run in this order and fix problems before moving on. Always from the repo root:

```sh
. ~/.cargo/env
cargo fmt                                      # then confirm with: cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Acceptance criteria come from `docs/tech-task.md` section 8: no warnings, formatted, atomic file operations, unit tests for config/tilde, `diff.rs` and glob filtering, and the integration test `tests/backup_test.rs`.

Known pitfalls in this codebase:
- Edition 2024 + current clippy flags nested `if`/`if let` (`collapsible_if`). Use let-chains: `if let Some(x) = a && let Some(y) = f(x)? { ... }`.
- `for ... { return ... }` over an iterator triggers `never_loop`; use `.next()` / `.or()` instead.
- Run `cargo fmt` after editing, because clippy and fmt both run in CI-style checks; fmt rewrites long lines in tests too.
- Unused imports after refactors fail the build under `-D warnings`.

Report results honestly: quote failing test output, and say explicitly if the integration tests were skipped (they skip silently when `gpg`/`git` are missing, so a green run does not prove they executed; check the `cargo test --test backup_test` output shows 2 passed).
