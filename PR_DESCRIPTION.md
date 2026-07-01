# Stop panicking when squeue misbehaves

Right now if `squeue` isn't on PATH, fails to spawn, or exits non-zero, the whole app panics and takes the terminal down with it. Not great for a TUI.

**What changed:**
- squeue spawn/exit failures now produce a `JobsError` message instead of `.expect()`-ing into oblivion
- The error shows up in red in the Jobs panel title (exit code + stderr included)
- Clears itself automatically once squeue succeeds again
- Channel sends no longer `.unwrap()` — if the app's gone, the watcher thread just exits quietly

**Testing:** build/clippy/test all green.
