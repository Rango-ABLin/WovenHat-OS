# Browser engine boundary

The first browser-engine increment adds `libwoven::browser::parse_html`, a
fixed-capacity borrowed DOM parser with bounded nesting, parent links, text
capture, self-closing tags, and fail-closed malformed markup handling.

Evidence:

- Full `libwoven` host suite: 147 passed.
- Settings target warning-denying Clippy: passed.
- Formatting and diff checks: passed.

Network fetch, layout/CSS, JavaScript execution, sandboxing, and pixel
rendering remain separate browser-engine boundaries.
