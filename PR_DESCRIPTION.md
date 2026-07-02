# Small DRY cleanups in app.rs

Noticed a few repeated patterns in `app.rs` while poking around — nothing behavior-changing, just pulling out the duplication into named helpers so it's easier to read and tweak later.

**What changed:**
- Extracted `detail_line()` for the repeated job-detail label/value spans (State/Name/Command/Nodes/TRES/stdout)
- Extracted `page_scroll_delta()` (+ `FAST_SCROLL_LINES` const) instead of duplicating the modifier check in the PageUp/PageDown arms
- Extracted `help_line()` for the hand-rolled separator fold in the help bar
- Added `impl From<CommandFailure> for Dialog` instead of manually destructuring/reconstructing in `handle()`

**Testing:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo fmt --all --check` all pass; rendering is byte-identical.
