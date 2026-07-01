# Tune the release profile

Release binaries were using Cargo defaults — easy wins left on the table for a binary that gets shipped via GitHub Releases/PyPI/crates.io.

**What changed:**
- Added `[profile.release]`: `lto = true`, `codegen-units = 1`, `strip = true`
- Deliberately skipped `panic = "abort"` — it'd interact with the terminal-restore panic hook, needs separate validation

**Testing:** debug build/clippy/test green, plus a full `cargo build --release` to confirm it compiles with the new settings.
