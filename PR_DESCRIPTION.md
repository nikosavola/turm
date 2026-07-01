# Actually make CI do its job

The test workflow only ran `cargo build` + `cargo test` on one target. No fmt check, no clippy gate, no `--locked`, no MSRV check, no caching, default (too broad) token perms.

**What changed:**
- Split into `lint` (fmt + clippy `-D warnings`), `test` (`--locked`), and `msrv` (pinned to 1.87) jobs
- Added `Swatinem/rust-cache` for dependency caching
- `permissions: contents: read` + a concurrency group to cancel stale runs

**Testing:** fmt/build/clippy/test all pass locally; YAML validated.
