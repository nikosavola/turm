# Skip redundant log clones/sends

Every tick, even on a completely idle file, the reader cloned the entire accumulated log content and sent it over the channel. Wasteful, especially combined with large logs.

**What changed:**
- If a read produces 0 new bytes and the file hasn't been truncated, skip the clone+send entirely
- First read after selecting a file still always sends, so an empty file still renders correctly

**Testing:** build/clippy/test all green.
