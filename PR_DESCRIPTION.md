# Table-drive `SqueueArgs::to_vec`

`to_vec` had 20 near-identical `if` blocks, one per flag, that had to be kept in sync by hand with the clap struct above it. Easy to typo or forget when adding a new flag.

**What changed:**
- Collapsed the if-blocks into a single ordered table of `Flag::Value`/`Flag::Bool` entries plus one loop
- Kept the clap `#[derive(Args)]` struct untouched — CLI surface is identical
- Preserved the exact original argument order (13 value flags, 7 bool flags)

**Testing:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo fmt --all --check` all pass, plus a new unit test pinning the exact output for a mix of value and bool flags.
