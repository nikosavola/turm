# Split app.rs into focused modules

`src/app.rs` had grown to ~1250 lines mixing app state, input handling, dialogs, rendering, text layout, and Slurm command execution. Splitting it makes each piece easier to find and change.

**What changed:**
- `src/slurm.rs`: `CommandFailure` + the `scancel`/`scontrol` command execution
- `src/ui_text.rs`: `fit_text` and `chunked_string` text-layout helpers
- `src/dialog.rs`: `Dialog` enum, its helpers, and dialog rendering (turned out to be a pure `(Frame, &Dialog)` function, no `App` state needed)
- `app.rs` keeps `App`/`Job`/`AppMessage`/`Focus`, the event loop, and all input handling

**Testing:** build/clippy/test/fmt all green, same 2 tests passing (moved with their code, none added or removed).
