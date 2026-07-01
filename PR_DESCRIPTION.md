# Add tests for squeue parsing + Slurm path resolution

The two most externally-driven, failure-prone bits of code — parsing squeue's separator-delimited output and resolving Slurm's `%A`/`%a`/`%j`/... filename patterns — had zero test coverage.

**What changed:**
- Extracted the inline line-parsing closure into `JobWatcher::parse_line`
- Added 17 new tests: 4 for line parsing (well-formed, array task id handling, malformed/short lines), 13 for `resolve_path` (every `%` pattern, multi-node lists, the `N/A` sentinel, absolute vs relative paths)
- No runtime behavior changes, just made the existing logic testable

**Testing:** build/clippy/test all green, 19 tests total (up from 2).
