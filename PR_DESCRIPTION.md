# Simplify `resolve_path`

The code literally had a comment saying "this is stupid, there has to be a better way to reverse the captures..." — so here's the better way, plus the 8-arg signature got tired of being `#[allow]`'d.

**What changed:**
- New `FilenameContext` struct bundles the 7 shared params (array id, host, user, etc.), built once per job line and reused for both `stdout` and `stderr`.
- Swapped the collect-reverse-`replace_range` loop for a single forward `Regex::replace_all` pass.
- Removed the `#[allow(clippy::too_many_arguments)]` and the TODO comment.

**Testing:** Added unit tests covering `%j`/`%u`/`%N` substitution, the `N/A` array-id sentinel, absolute path preservation, and the empty-path fallback; `cargo build`, `clippy -D warnings`, `test`, and `fmt --check` all pass.
