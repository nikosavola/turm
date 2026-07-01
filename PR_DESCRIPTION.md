# Stop the file watcher thread from silently dying

The file-watcher thread was full of `unwrap()`/`expect()`/`panic!` on stuff that can genuinely happen at runtime (inotify overflow, unwatching an already-gone path, channels closing during shutdown). Since it's a background thread, a panic there doesn't crash the app — it just silently stops updating logs, with zero indication anything went wrong.

**What changed:**
- Notify callback errors are now ignored instead of unwrapped
- Unwatch failures are ignored (harmless if the path's already gone)
- Channel send failures either get ignored (best-effort) or cleanly exit the loop (peer gone), never panic
- Happy path behavior is unchanged

**Testing:** build/clippy/test all green.
