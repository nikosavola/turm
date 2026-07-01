# Add dependency security scanning

No automated way to catch vulnerable dependencies, and no automated dependency updates. Fixing both.

**What changed:**
- New `audit.yml` workflow running `cargo audit` on PRs, weekly, and on-demand
- New `dependabot.yml` for both `cargo` and `github-actions` ecosystems, weekly

**Testing:** CI-config-only; build/clippy/test unaffected. YAML validated.
