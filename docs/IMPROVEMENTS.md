# turm — Project Analysis & Proposed Improvements

This document is the result of a code-safety, stability, DevOps, and performance
review of `turm` at `v0.14.0`. Each section below is written as a ready-to-file
GitHub issue: copy the section into a new issue, or use it as a tracking
checklist. Issues are ordered by category and priority.

**Reviewed surface:** `src/main.rs`, `src/app.rs`, `src/job_watcher.rs`,
`src/file_watcher.rs`, `src/squeue_args.rs`, `.github/workflows/*`, `Cargo.toml`.

**Baseline at time of review:** `cargo build`, `cargo clippy --all-targets`, and
`cargo test` all pass cleanly. There are 2 unit tests. The findings below are
about latent runtime failures, resource usage, supply-chain/CI hygiene, and
maintainability — not current compiler/clippy warnings.

### Priority summary

| # | Title | Category | Priority |
|---|-------|----------|----------|
| 1 | App panics when `squeue` is missing or fails | Safety / Stability | High |
| 2 | Non-UTF-8 log output permanently breaks the log pane | Safety / Stability | High |
| 3 | Unbounded memory growth tailing large log files | Performance / Stability | High |
| 4 | Log truncation / rotation is never detected | Stability | Medium |
| 5 | Panics in the file-watcher kill watching silently | Safety / Stability | Medium |
| 6 | Whole log re-cloned and re-sent every tick | Performance | Medium |
| 7 | Migrate off archived `actions-rs/*` GitHub Actions | DevOps | High |
| 8 | Harden & expand the CI test workflow | DevOps | High |
| 9 | Add supply-chain checks (audit, deny, dependabot) | DevOps / Security | Medium |
| 10 | Add a release build profile for smaller/faster binaries | Performance / DevOps | Medium |
| 11 | Remove `lazy_static` in favour of `std::sync::LazyLock` | Maintainability | Low |
| 12 | Dead single-variant `Focus` abstraction & no-op panel nav | Maintainability / UX | Low |
| 13 | Add tests around `squeue` parsing and path resolution | Stability / Testing | Medium |

---

## Issue 1 — App panics (and corrupts the terminal) when `squeue` is missing or errors

**Labels:** `bug`, `stability`, `priority:high`

### Summary
The job-watcher thread unwraps the result of spawning `squeue`. If `squeue` is
not on `PATH`, is not executable, or the process otherwise fails to spawn, the
whole application panics.

### Details
`src/job_watcher.rs:55-65`:

```rust
let jobs: Vec<Job> = Command::new("squeue")
    .args(&self.squeue_args)
    .arg("--array")
    .arg("--noheader")
    .arg("--Format")
    .arg(&output_format)
    .output()
    .expect("failed to execute process")   // <-- panics if squeue can't be spawned
    .stdout
    .lines()
    .map(|l| l.unwrap().trim().to_string()) // <-- panics on non-UTF-8 line (see Issue 2)
    .filter_map(|l| { ... })
    .collect();
self.app.send(AppMessage::Jobs(jobs)).unwrap(); // <-- panics if the app is gone
```

Because this runs in a spawned thread, the panic message is printed but the
terminal may also be left in raw/alternate-screen mode depending on timing,
even with the panic hook installed in `main.rs`. More importantly, a transient
`squeue` failure (e.g. Slurm controller momentarily unreachable, `squeue`
returning a non-zero exit with a stderr message) is fatal rather than
recoverable.

### Impact
- Running `turm` on a machine without Slurm in `PATH` crashes immediately
  instead of showing a friendly error.
- A momentary controller hiccup terminates a long-lived session.

### Proposed fix
- Replace `.expect(...)` with proper error handling: on spawn failure or
  non-zero exit, send an `AppMessage` carrying the error so the UI can render
  it (similar to how `FileWatcherError` is surfaced in the log pane), then keep
  looping and retry on the next interval.
- Surface `squeue` stderr in the UI when the exit status is non-zero (the same
  `CommandError` dialog used for `scancel`/`scontrol` is a good model).
- Treat `self.app.send(...)` returning `Err` (receiver dropped = app exiting)
  as a clean shutdown signal for the thread, not a panic.

---

## Issue 2 — Non-UTF-8 (or mid-character) log output permanently breaks the log pane

**Labels:** `bug`, `stability`, `priority:high`

### Summary
Both the `squeue` parser and the log file reader assume strictly valid UTF-8.
Slurm job logs routinely contain non-UTF-8 bytes (binary writes, progress-bar
escape sequences, partial multi-byte characters at a read boundary). When that
happens the log pane shows a permanent read error instead of the output.

### Details
**Log reader** — `src/file_watcher.rs:161-170`:

```rust
fn update(&mut self) -> Result<(), SendError<io::Result<String>>> {
    let s = File::open(&self.file_path).and_then(|mut f| {
        self.pos = f.seek(io::SeekFrom::Start(self.pos))?;
        self.pos += f.read_to_string(&mut self.content)? as u64; // fails on invalid UTF-8
        Ok(self.content.clone())
    });
    self.content_sender.send(s)
}
```

Two distinct failure modes:
1. **Any invalid UTF-8 byte** in the file makes `read_to_string` return
   `ErrorKind::InvalidData`. `self.pos` is not advanced, so every subsequent
   tick retries from the same offset and fails again — the log pane is stuck on
   the error forever.
2. **Valid UTF-8 split across an incremental read.** Because reads resume from a
   byte offset (`self.pos`), a multi-byte character that straddles the boundary
   between two reads is seen as invalid by `read_to_string`, producing the same
   permanent error even for perfectly valid files.

**`squeue` parser** — `src/job_watcher.rs:65`: `.map(|l| l.unwrap()...)` panics
on the first non-UTF-8 line (a job name with odd bytes is enough). This overlaps
with Issue 1.

### Proposed fix
- Read raw bytes (`Vec<u8>`) and convert with `String::from_utf8_lossy`, or
  decode incrementally while tracking a partial trailing multi-byte sequence so
  the boundary case never errors.
- For the `squeue` parser, read bytes and use lossy conversion instead of
  `unwrap()` per line.
- Never advance/leave `pos` in a state that makes the error permanent; if a read
  genuinely fails, retry from a safe boundary.

---

## Issue 3 — Unbounded memory growth when tailing large/long-running log files

**Labels:** `performance`, `stability`, `priority:high`

### Summary
The file reader accumulates the entire log file in memory for the lifetime of
the selection and never caps it. For long-running jobs that emit gigabytes of
output, `turm`'s memory grows without bound.

### Details
`src/file_watcher.rs:18-25, 161-170`: `FileReader.content` is a `String` that is
only ever appended to (`read_to_string(&mut self.content)`), starting at `pos`
and growing for as long as the file is selected. The README advertises
`turm ≈ ... + tail -f slurm-log.out`, but `tail` shows a bounded window whereas
`turm` retains everything ever read.

The UI (`fit_text` in `src/app.rs:939`) only ever renders the last `lines`
screen rows anchored to top/bottom, so the vast majority of retained content is
never visible — it is pure overhead.

### Impact
- Memory usage scales with total log size, not screen size.
- Compounded by Issue 6 (the full string is cloned and sent over the channel on
  every tick), large logs cause growing per-tick allocation and copy cost.

### Proposed fix
- Keep a bounded buffer: retain only the last *N* lines or last *N* bytes
  (configurable, e.g. a few MB / a few thousand lines), discarding from the
  front as new data arrives. The scroll-from-bottom UI only needs a tail window.
- Alternatively, store line offsets and read on demand, but a capped ring buffer
  is the simplest robust fix and matches `tail -f` semantics.

---

## Issue 4 — Log truncation / rotation is never detected

**Labels:** `bug`, `stability`, `priority:medium`

### Summary
The reader tracks a byte offset (`pos`) and always seeks to it. If the log file
shrinks (truncated, rotated, or the job re-runs and rewrites the file), the
reader keeps seeking past the new end and shows stale content indefinitely.

### Details
`src/file_watcher.rs:162-166`: `seek(SeekFrom::Start(self.pos))` succeeds even
when `pos` is beyond EOF, and the following read returns 0 new bytes. There is
no check comparing `pos` against the current file length, so a truncation is
invisible and the displayed content is frozen at the old data.

### Proposed fix
- On each update, query the file length (`metadata().len()` or the seek result).
  If the current length is **less than** `pos`, reset `pos = 0` and clear the
  retained buffer, then re-read from the start. This is the standard `tail -F`
  truncation-detection behaviour.

---

## Issue 5 — Panics inside the file-watcher thread silently stop log watching

**Labels:** `bug`, `stability`, `priority:medium`

### Summary
The notify callback and the `FileWatcher::run` loop are full of `unwrap()`,
`expect()`, and explicit `panic!`. A panic in a spawned thread does not crash
the process — it just kills that thread, so the symptom is "logs silently stop
updating" with no error shown to the user.

### Details
`src/file_watcher.rs`:
- `72-78`: `let event = res.unwrap();` — the notify watcher can deliver an
  `Err` (e.g. inotify queue overflow `ENOSPC`); this panics the callback.
- `75`: `watch_sender.send(()).unwrap();`
- `91-92`: `self.file_path.as_ref().expect("Inconsistent state")` and
  `watcher.unwatch(p).unwrap_or_else(|_| panic!("Failed to unwatch {:?}", p))`.
- `108`, `113`, `122-124`: `.unwrap()` on channel sends.

### Impact
Any of these failures (which are reachable under real conditions — inotify
overflow on a busy file, unwatch of an already-removed path, the app shutting
down) silently disables log following for the rest of the session.

### Proposed fix
- Treat notify `Err` events as recoverable: log/ignore or surface a one-line
  status, don't panic.
- Treat channel-send failures as "peer gone → exit this loop cleanly".
- Replace `expect("Inconsistent state")` and the `panic!` on unwatch with
  defensive handling (ignore unwatch errors; they are benign).

---

## Issue 6 — The entire log string is cloned and re-sent on every tick

**Labels:** `performance`, `priority:medium`

### Summary
On every poll/notify, the reader clones the full accumulated content and sends
it across the channel, even when nothing changed and even when the buffer is
large.

### Details
`src/file_watcher.rs:165-169`: `Ok(self.content.clone())` clones the whole
buffer each update; combined with Issue 3 the clone grows unbounded. The polling
fallback (`default(self.interval)`) fires every `file_refresh` seconds
regardless of whether new bytes arrived, so the clone+send happens even on a
completely idle file.

### Proposed fix
- Send only newly appended data (a delta), and let `App` maintain the bounded
  view buffer; or share an `Arc<str>`/`Arc<Vec<u8>>` snapshot instead of cloning
  a `String`.
- Skip sending entirely when `read` returned 0 new bytes and the file length is
  unchanged.
- This pairs naturally with the bounded-buffer fix in Issue 3.

---

## Issue 7 — Migrate off archived `actions-rs/*` GitHub Actions

**Labels:** `ci`, `devops`, `priority:high`

### Summary
`.github/workflows/release.yml` uses `actions-rs/toolchain@v1` and
`actions-rs/cargo@v1`. The `actions-rs` organisation has been **archived and
unmaintained since 2022**, and these actions run on deprecated Node runtimes,
producing deprecation warnings and posing a supply-chain/maintenance risk.

### Details
`.github/workflows/release.yml:66-79`:

```yaml
- name: Setup | Rust
  uses: actions-rs/toolchain@v1   # archived
  ...
- name: Build | Build
  uses: actions-rs/cargo@v1       # archived
  with:
    use-cross: ...
```

### Proposed fix
- Replace `actions-rs/toolchain` with `dtolnay/rust-toolchain` (or
  `actions-rust-lang/setup-rust-toolchain`).
- Replace `actions-rs/cargo` + `use-cross` with a maintained cross-compilation
  path: `taiki-e/install-action` to install `cross`, or
  `houseabsolute/actions-rust-cross`, or `cargo-zigbuild`.
- While here, pin third-party actions to commit SHAs (or at minimum keep the
  major tags consistent) for supply-chain hardening.

---

## Issue 8 — Harden and expand the CI test workflow

**Labels:** `ci`, `devops`, `priority:high`

### Summary
`.github/workflows/test.yml` only runs `cargo build` and `cargo test` on a
single Ubuntu/stable combination. It does not enforce formatting, lints, the
declared MSRV, or use a lockfile, and it grants default (broad) token
permissions.

### Details
`.github/workflows/test.yml`:
- No `cargo fmt --check`.
- No `cargo clippy -- -D warnings` (the code is currently clippy-clean — a gate
  would keep it that way).
- No `--locked` (CI can silently build with a different dependency set than
  `Cargo.lock`).
- `Cargo.toml` declares `rust-version = "1.87"` but nothing verifies the MSRV
  still builds.
- No dependency caching (`Swatinem/rust-cache`) → slow, repeated full rebuilds.
- No explicit `permissions:` block → the job gets the repo default token scope;
  it should be `contents: read`.
- No `concurrency:` group → stacked pushes run redundant jobs.

### Proposed fix
Extend the workflow with separate jobs/steps:
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked --verbose`
- An MSRV job pinned to `1.87` that runs `cargo build --locked`.
- Add `Swatinem/rust-cache@v2`.
- Add top-level `permissions: { contents: read }` and a `concurrency` group
  keyed on the ref with `cancel-in-progress: true`.

---

## Issue 9 — Add supply-chain checks: `cargo audit`, `cargo deny`, Dependabot

**Labels:** `security`, `devops`, `priority:medium`

### Summary
There is no automated detection of vulnerable or unmaintained dependencies and
no automated dependency-update mechanism.

### Details
- No `cargo audit` / `cargo deny` step in CI.
- No `.github/dependabot.yml`.
- `Cargo.lock` is committed (good — it's a binary), so a scheduled audit and
  Dependabot updates are the right tools to keep it current and safe.

### Proposed fix
- Add a scheduled (e.g. weekly) + PR CI job running `cargo audit` (RUSTSEC
  advisories) and optionally `cargo deny check` (licences, bans, advisories,
  sources).
- Add `.github/dependabot.yml` for the `cargo` and `github-actions` ecosystems.

---

## Issue 10 — Add a `[profile.release]` for smaller, faster release binaries

**Labels:** `performance`, `devops`, `priority:medium`

### Summary
`Cargo.toml` has no release profile tuning. The binaries shipped to
GitHub Releases / PyPI / crates.io use default release settings, leaving easy
size and startup wins on the table.

### Details
`Cargo.toml` ends at the `[dependencies]` table — there is no
`[profile.release]`. For a distributed CLI/TUI binary, link-time optimisation,
a single codegen unit, and symbol stripping meaningfully reduce binary size and
can improve runtime.

### Proposed fix
Add, and benchmark:

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
# Optionally, for a TUI where unwinding is not needed:
# panic = "abort"
```

Note: `panic = "abort"` interacts with the panic hook in `main.rs` (terminal
restore) — validate that the `Drop` guard on `TerminalGuard` still restores the
terminal under `abort` before adopting it. Measure size/perf before/after.

---

## Issue 11 — Replace `lazy_static` with `std::sync::LazyLock`

**Labels:** `maintainability`, `cleanup`, `priority:low`

### Summary
The project is on edition 2024 (MSRV 1.87). `std::sync::LazyLock` has been stable
since Rust 1.80, so the `lazy_static` dependency is no longer needed.

### Details
`src/job_watcher.rs:157-159`:

```rust
lazy_static::lazy_static! {
    static ref RE: Regex = Regex::new(r"%(%|A|a|J|j|N|n|s|t|u|x)").unwrap();
}
```

This is the only use of `lazy_static`. It can become:

```rust
use std::sync::LazyLock;
static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%(%|A|a|J|j|N|n|s|t|u|x)").unwrap());
```

### Proposed fix
- Replace the macro with `LazyLock` and drop `lazy_static` from `Cargo.toml`
  (one fewer dependency).

---

## Issue 12 — Dead single-variant `Focus` abstraction and no-op panel navigation

**Labels:** `maintainability`, `ux`, `cleanup`, `priority:low`

### Summary
`Focus` has only one variant (`Jobs`), which makes the `h`/`l`/←/→ panel
navigation and several `match self.focus` arms dead no-ops, while the log pane
cannot be focused for keyboard navigation.

### Details
`src/app.rs:23-25`, `1020-1030`:

```rust
pub enum Focus { Jobs }

fn focus_next_panel(&mut self) {
    match self.focus { Focus::Jobs => self.focus = Focus::Jobs } // no-op
}
fn focus_previous_panel(&mut self) {
    match self.focus { Focus::Jobs => self.focus = Focus::Jobs } // no-op
}
```

Every `match self.focus { Focus::Jobs => ... }` (e.g. `app.rs:391-422`) is a
single-arm match kept only to satisfy the abstraction. The help line advertises
`⏶/⏷ navigate` but `h`/`l`/←/→ are bound to no-ops.

### Proposed fix
Pick one direction:
- **Implement it:** add a `Focus::Log` variant so `h`/`l` switch focus and the
  job-list movement keys (`j`/`k`/`g`/`G`) can drive log scrolling when the log
  pane is focused — turning the existing dead branches into real behaviour.
- **Or remove it:** delete the `Focus` enum and the no-op nav functions/keys to
  cut dead code.

---

## Issue 13 — Add tests for `squeue` output parsing and Slurm path resolution

**Labels:** `testing`, `stability`, `priority:medium`

### Summary
The two most failure-prone, externally-driven code paths — parsing
separator-delimited `squeue` output and resolving Slurm filename patterns — have
no tests. The repo already ships mock Slurm binaries that are unused by CI.

### Details
- `src/job_watcher.rs:54-143` parses `###turm###`-separated fields with strict
  index assumptions (`parts.len() != fields.len() + 1`), and
  `resolve_path` (`145-207`) implements the Slurm `%A/%a/%j/%N/...` filename
  substitution including array-job edge cases and the `4294967294` "no value"
  sentinel. None of this is covered by tests.
- `scripts/mock-slurm/bin/{squeue,scancel,scontrol}` exist for manual local
  testing (`PATH=scripts/mock-slurm/bin:$PATH cargo run -- --me`) but are never
  exercised in CI.

### Proposed fix
- Factor the line-parsing and `resolve_path` logic so they are unit-testable
  with fixed input strings (no process spawning), and add table-driven tests for:
  array vs non-array jobs, each `%`-pattern, multi-node `%N`, the empty-path
  fallback, and malformed/short lines.
- Optionally add a CI job that runs against `scripts/mock-slurm/bin` to smoke-test
  the end-to-end parse path.

---

## Out-of-scope / nice-to-have (not filed as issues)

- **Split `app.rs` (1252 lines)** into `ui`, `input`, `dialog`, and `commands`
  modules for readability — purely organisational.
- **`SqueueArgs::to_vec`** (`src/squeue_args.rs:86-150`) is repetitive
  boilerplate; a small macro or struct-driven approach would shrink it, but the
  explicit form is clear and low-risk.
