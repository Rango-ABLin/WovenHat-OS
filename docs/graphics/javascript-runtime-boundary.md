# Freestanding JavaScript runtime boundary

The current increment adds `libwoven::javascript::Runtime`, a no-std,
allocation-free scalar execution core for small UI bindings and policy
expressions. It supports bounded scripts containing integer literals,
parentheses, unary negation, arithmetic precedence, `let` bindings,
assignment/update, semicolon-separated statements, and line comments.

The runtime has fixed variable capacity, a 4096-byte script limit, checked
integer arithmetic, checked division, and a 32-level expression nesting limit.
Invalid tokens and unknown variables fail closed. It does not claim complete
ECMAScript compatibility, objects/prototypes, strings, arrays, functions,
DOM bindings, promises, modules, JIT execution, or browser sandboxing.

Evidence:

- Full `libwoven` host suite: 165 passed.
- Host warning-denying Clippy: passed.
- `x86_64-unknown-none` warning-denying Clippy: passed.
