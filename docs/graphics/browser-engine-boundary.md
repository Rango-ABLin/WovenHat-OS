# Browser engine boundary

The browser-engine substrate now adds `libwoven::browser::parse_html` plus a
deterministic fixed-capacity block layout pass. The borrowed DOM supports
bounded nesting, parent links, text capture, self-closing tags, and a small
inline-style surface (`width`, `height`, `margin`, and `display`). Layout
produces checked rectangles for a caller-supplied viewport and fails closed on
invalid viewports, overflow, excessive depth, or capacity exhaustion.

Evidence:

- Full `libwoven` host suite: 162 passed.
- `libwoven` host and `x86_64-unknown-none` warning-denying Clippy: passed.
- Formatting/diff checks: passed for the increment.

Network fetch, full CSS cascade, JavaScript execution, sandboxing, and pixel
rendering remain separate browser-engine boundaries.
