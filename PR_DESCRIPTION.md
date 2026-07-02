# Clarify the file-watcher reader lifecycle

`FileWatcher::run` re-created and shadowed a bunch of underscore-prefixed channel
variables every time the watched file changed. The underscores said "unused", but
dropping the old pair is actually what stops the previous `FileReader` thread — a
neat trick, but invisible to anyone reading the code.

**What changed:**
- Added a small `ReaderConnection` type that owns the channels to the current (or
  not-yet-existent) `FileReader` thread, with a doc comment spelling out that
  replacing it is what severs the old thread's channels and makes it exit.
- Renamed channels for direction/purpose: `notify_tx`/`notify_rx` (watch events),
  `kick_tx`/`kick_rx` (watcher tells reader to re-read), `content_tx`/`content_rx`
  (reader reports content back).
- No underscore-prefixed-but-used variables left; no behavior changes (same
  unwraps, same watch/unwatch/retry logic).

**Testing:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo
test`, and `cargo fmt --all --check` all pass; manually smoke-tested log tailing
and switching between jobs with the mock Slurm scripts in a tmux pty, no panics.
