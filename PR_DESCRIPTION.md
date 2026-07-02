# perf: cache job display id and column widths across frames

`App::ui` runs on every message (log ticks every 2s, every key/mouse event), and it was reallocating each job's display id via `format!` and rescanning the whole job list five separate times just to work out column widths. None of that changes between job list refreshes, so let's stop redoing it every frame.

**What changed:**
- `Job` now stores its display id as a plain `String` field, computed once in `job_watcher.rs` when the job is built, instead of via a `format!`-allocating `Job::id()` call every time.
- Dropped the now-unused `job_id`/`array_id`/`array_step` fields on `Job` (they only ever existed to compute the id).
- Added a `ColumnWidths` struct computed once (single pass) in `handle(AppMessage::Jobs(...))` and cached on `App`, replacing the five per-frame `O(N)` sweeps in `ui`.

**Testing:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo fmt --all --check` all pass.
