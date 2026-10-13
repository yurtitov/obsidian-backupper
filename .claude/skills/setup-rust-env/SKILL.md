---
name: setup-rust-env
description: Bootstrap the Rust toolchain on a fresh Linux machine for this repo (rustup, clippy, rustfmt, C linker). Use when `cargo` is not found, or a build fails with "linker `cc` not found".
---

# Set up the Rust environment

Check first, install only what is missing:

```sh
. ~/.cargo/env 2>/dev/null; cargo --version; cc --version | head -1; gpg --version | head -1; git --version
```

1. **Rust** (missing `cargo`): ask the user before installing, then use the official installer, not apt (apt versions may not support edition 2024):
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$SCRATCHPAD/rustup-init.sh"
   sh "$SCRATCHPAD/rustup-init.sh" -y --profile minimal -c clippy -c rustfmt
   ```
   Download to the scratchpad, never pipe curl into sh. The installer only touches `~/.cargo` and `~/.rustup`.
2. **C linker** (rustup warns "no default linker (`cc`) was found"): `apt-get install -y build-essential` (needs root; on macOS `xcode-select --install`).
3. **gpg and git** are required at runtime and by the integration tests; install via the OS package manager if missing.
4. Every new shell needs `. ~/.cargo/env` before `cargo` is on PATH (the Bash tool does not persist env between calls, so prefix commands: `. ~/.cargo/env && cargo ...`).

Edition is 2024, so the toolchain must be recent stable (1.85+).
