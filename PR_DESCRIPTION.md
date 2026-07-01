# Ditch the archived actions-rs actions

`actions-rs/toolchain` and `actions-rs/cargo` have been unmaintained/archived since 2022 and run on deprecated Node runtimes. Time to move on.

**What changed:**
- `actions-rs/toolchain` → `dtolnay/rust-toolchain`
- `actions-rs/cargo` + `use-cross` → `cross` installed via `taiki-e/install-action`, run directly
- Matrix, artifact names, and release upload behavior unchanged

**Testing:** CI-config-only change; build/clippy/test still pass locally unchanged.
