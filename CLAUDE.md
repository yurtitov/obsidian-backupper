# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Language rules

- Chat with the user in Russian.
- Everything saved in the project must be in English only: documentation, code comments, commit messages, identifiers, log/error messages, and any other artifacts.

## Project status

`obsidian-backupper` is at the initial-commit stage: the repo contains only `README.md` (title only), `LICENSE`, and a `.gitignore`. There is no source code, manifest, or tests yet.

The `.gitignore` is the standard Cargo/Rust template (`target`, `*.rs.bk`, `mutants.out*`, etc.), so the project is intended to be a Rust crate. Once `Cargo.toml` exists, the usual commands apply:

- Build: `cargo build`
- Run: `cargo run`
- Test all: `cargo test`
- Run a single test: `cargo test <test_name>`
- Lint/format: `cargo clippy` / `cargo fmt`

## Maintenance

Replace the "Project status" section with real architecture notes (module layout, how backups are made, config format) once code is added, and update the commands if the toolchain differs from the assumptions above.
