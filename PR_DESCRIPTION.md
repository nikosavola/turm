# Detect log truncation/rotation

If a log file got truncated or rotated (job reruns, log rotation, whatever), the reader just kept seeking to its old byte offset and showed stale content forever — it never checked if the file had shrunk.

**What changed:**
- Before reading, check the file's current length via `seek(End)`
- If it's smaller than our tracked position, the file got truncated — reset position and clear the buffer, then read from scratch

**Testing:** build/clippy/test all green, +1 new test covering read → append → truncate → reset.
