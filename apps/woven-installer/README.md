# WovenInstaller

`wovenhat-installer` is the bounded Stage 28 Ring-3 application shell. It
accepts a compact snapshot of the already validated installer UI/state model
and renders welcome, target, partition review, settings, progress,
cancellation, failure, and success screens. The default mode is a deterministic
text renderer; `--gui <shell-pid> <snapshot>` opens a compositor window using
the existing WOPN/shared-surface protocol and supports bounded resize,
pointer, and close handling.

The snapshot must contain `validated=1` and a `plan` field. All fields are
bounded and reject control characters, duplicate keys, unknown keys, invalid
progress, and missing screen prerequisites. The application has no install
subcommand and no capability or syscall path for disk, partition, filesystem,
bootloader, network, or subprocess work. Progress and success are display-only
states; `install_action=unavailable (fail-closed)` is always reported.

Example:

```text
screen=progress;target=USB fixture;plan=validated;settings=UTC;progress=42;confirmed=0;validated=1;message=review
```

The GUI is intentionally authority-free: it cannot discover disks, partition,
format, install a bootloader, create accounts, configure networking, or write
physical media. Those operations must be performed by a separately reviewed
installer service using the validated plan and transaction boundary.
