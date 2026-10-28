# WovenHat application template

Copy this directory into a standalone application crate, replace the package
name, and implement `_start` in `src/main.rs`. Build with the pinned Rust
toolchain and the target settings in `.cargo/config.toml`.

The template is intentionally small: use `libwoven` for system calls and
`libwoven::exit` for process termination. See
`docs/writing-wovenhat-applications.md` for loader limits and
`docs/syscall-api.md` for the stable ABI.
