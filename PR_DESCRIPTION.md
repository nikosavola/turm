# Drop lazy_static for std::sync::LazyLock

We're on edition 2024 / MSRV 1.87, and `LazyLock` has been stable since 1.80. No reason to pull in `lazy_static` for one regex.

**What changed:**
- Swapped the `lazy_static!` macro for a plain `static RE: LazyLock<Regex>`
- Removed the now-unused `lazy_static` dependency

**Testing:** build/clippy/test all green.
