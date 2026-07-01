# Remove the dead Focus enum

`Focus` only ever had one variant (`Jobs`), which made `h`/`l`/Left/Right panel navigation a no-op and left a bunch of single-arm `match self.focus` scattered around for no reason.

**What changed:**
- Deleted `Focus`, the `focus` field, and the no-op `focus_next_panel`/`focus_previous_panel` functions
- Removed the dead `h`/`l`/Left/Right key bindings
- Flattened all the pointless `match self.focus { Focus::Jobs => x }` down to just `x`
- No behavior change other than those no-op keys being gone (dialog-dim border logic is untouched)

**Testing:** build/clippy/test all green.
