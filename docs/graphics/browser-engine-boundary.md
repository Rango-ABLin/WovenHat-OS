# Browser engine boundary

The browser-engine substrate now adds `libwoven::browser::parse_html`, a
deterministic fixed-capacity block layout pass, and
`libwoven::browser::paint` and `libwoven::browser::render`. The borrowed DOM supports bounded nesting, parent
links, text capture, self-closing tags, and a small inline-style surface
(`width`, `height`, `margin`, `display`, `color`, and `background-color`).
Layout produces checked rectangles for a caller-supplied viewport and fails
closed on invalid viewports, overflow, excessive depth, or capacity
exhaustion. The paint pass emits checked renderer-neutral fill/text commands
without coupling browser state to a display backend; the render adapter maps
those commands to caller-owned software-canvas pixels with checked coordinates.
Inline script text can be passed explicitly to the bounded scalar runtime via
`libwoven::browser::execute_script`; this seam exposes no DOM or system
capabilities.

Evidence:

- Full `libwoven` host suite: 171 passed.
- `libwoven` host and `x86_64-unknown-none` warning-denying Clippy: passed.
- Formatting/diff checks: passed for the increment.

Network fetch, full CSS cascade, general JavaScript execution, text shaping,
sandboxing, and GPU composition remain separate browser-engine boundaries.
