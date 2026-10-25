# Browser engine boundary

The browser-engine substrate now adds `libwoven::browser::parse_html`, a
deterministic fixed-capacity block layout pass, and
`libwoven::browser::paint`. The borrowed DOM supports bounded nesting, parent
links, text capture, self-closing tags, and a small inline-style surface
(`width`, `height`, `margin`, `display`, `color`, and `background-color`).
Layout produces checked rectangles for a caller-supplied viewport and fails
closed on invalid viewports, overflow, excessive depth, or capacity
exhaustion. The paint pass emits checked renderer-neutral fill/text commands
without coupling browser state to a display backend.

Evidence:

- Full `libwoven` host suite: 169 passed.
- `libwoven` host and `x86_64-unknown-none` warning-denying Clippy: passed.
- Formatting/diff checks: passed for the increment.

Network fetch, full CSS cascade, JavaScript execution, text shaping,
sandboxing, and GPU pixel rendering remain separate browser-engine boundaries.
