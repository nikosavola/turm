# DRY the dialog rendering in App::ui

The four dialog arms (cancel confirm, signal select, edit time limit, command error) all rebuilt the same bordered/centered/cleared Paragraph scaffold by hand. Pulled that into one helper.

**What changed:**
- Hoisted `centered_dialog_area` to a module-level fn
- Added `render_dialog(f, title, color, height, content, wrap)` that builds the block, computes the centered area, clears it, renders, and returns the inner `Rect`
- Rewrote all four dialog arms to just build their `Text` content and call the helper (EditTimeLimit uses the returned inner rect for cursor placement)

**Testing:** build/clippy/test/fmt all green, rendering is unchanged (same titles, colors, wrapping, sizes).
