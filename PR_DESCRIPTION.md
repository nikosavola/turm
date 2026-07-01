# Don't choke on non-UTF-8 log/squeue output

Slurm logs and squeue output aren't guaranteed to be valid UTF-8 (binary writes, progress bars, whatever). Previously that meant a permanently stuck error in the log pane, or a straight-up panic in the squeue parser.

**What changed:**
- Log reader now reads raw bytes and decodes incrementally, carrying over any dangling multi-byte sequence to the next read so we never mangle a character split across a read boundary
- Invalid bytes become `�` instead of erroring out forever
- squeue's stdout is decoded lossily instead of `.unwrap()`ing per line

**Testing:** build/clippy/test all green, +2 new tests for the incremental decode (split multibyte char, invalid bytes).
