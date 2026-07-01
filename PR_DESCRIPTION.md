# Cap log memory usage

Tailing a long-running job's log meant turm's memory grew forever — the reader just kept appending to a `String` with no limit, even though we only ever render the last screenful.

**What changed:**
- Added a `MAX_RETAINED_LINES` cap (5000) on the in-memory buffer
- Trims from the front on line boundaries only (never splits a UTF-8 char)
- File read position tracking is untouched — only the in-memory copy gets trimmed

**Testing:** build/clippy/test all green, +5 new tests.
