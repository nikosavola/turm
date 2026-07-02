# Destructure squeue fields with a slice pattern

The squeue line parser was pulling 19 fields out by hand-counted index (`parts[0]` … `parts[18]`), guarded only by a length check against a `fields` array defined ~40 lines earlier. Reordering or inserting a field in one place without updating the other would silently scramble every downstream field.

**What changed:**
- Replaced the length check + indexed bindings with one slice pattern that binds names and enforces arity at the same time (`let [id, name, ...] = parts.as_slice() else { return None; }`)
- Added a comment tying the `fields` array order to the destructuring pattern, since the compiler can't express that constraint
- No behavior change: a line with the wrong number of parts still returns `None`

**Testing:** `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo fmt --all --check` all pass.
