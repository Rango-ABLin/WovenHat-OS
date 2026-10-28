# WovenCalculator

WovenCalculator is a separate Ring-3 application. Its default mode evaluates
one bounded ASCII arithmetic expression and writes the result through the
userspace ABI; `--gui <expression>` draws the same result through the signed
graphics path. The parser accepts checked `+`, `-`, `*`, `/`, unary signs, and
parentheses with bounded input length and nesting. Malformed input, division
by zero, and arithmetic overflow fail closed.

Build and package from this directory with the pinned target configuration:

```text
cargo build --release
```

Use `scripts/test-calculator-qemu.py` for signed packaging and the 1/2/4-CPU
Ring-3 acceptance matrix. The application does not contain kernel privileges;
the caller must grant display authority before using `--gui`.
