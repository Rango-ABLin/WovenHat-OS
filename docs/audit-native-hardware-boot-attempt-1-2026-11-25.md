# Native hardware boot attempt 1 — 2026-11-25

Image and preparation: `docs/audit-native-hardware-boot-preparation-2026-11-25.md`.

## User-observed result

The user booted the prepared `hardware-demo` USB on the HP Pavilion x360 and
reported:

- the session ended before the sign-in/desktop appeared;
- a text terminal was available and accepted typed commands, including
  directory navigation;
- pressing F1 opened a small developer-style desktop with two buttons the user
  described as “desktop” and “mockup”;
- the user regarded that small desktop as test-only and not part of the desired
  product experience.

No photograph or exact transcription was supplied with this report.

## Returned media

After the user reinserted the USB, the `WOVENLOG` FAT32 volume was healthy but
empty. It contained no `LOGS` directory and no `BOOTnnnn.TXT`:

- volume ID: `\\?\Volume{4b5cfdc7-c97b-4356-b8a1-6ab87023657c}\`
- recursive read-only listing: no entries.

This reproduces the earlier native-log problem recorded in
`docs/audit-live-boot-log-startup-2026-10-16.md`; QEMU live-log acceptance and
raw image read-back are not sufficient to prove the USB logging worker works on
this machine.

## Source correspondence

`kernel/src/main.rs:1827-1927` starts WovenShell and waits up to 500 ticks for
`session::active()`. If the shell is alive but has not taken the display, the
kernel records that it is still running; if it exits, the kernel records that
it died before taking the display.

`kernel/src/main.rs:1931-1944` starts `/bin/sh` only when
`desktop_session_started` is false. `kernel/src/main.rs:1947-1959` constructs
the small placeholder desktop with `ACTIVATE` and `SECOND` buttons;
`kernel/src/main.rs:1963-1978` documents it as a developer-only fallback shown
when `!session_ready`, and `kernel/src/main.rs:2022-2030` toggles it with F1.
The observed terminal plus F1 desktop therefore corresponds to WovenShell not
taking the display, not to a successful desktop session.

`kernel/src/session.rs:44-63` shows that with accounts present, session startup
loads durable/account identity before spawning WovenShell. A failure there is
reported as `Error::Identity`; a load failure as `Error::Image`; a spawn failure
as `Error::Spawn`. The exact on-screen `SESSION:` line is needed to distinguish
them.

`kernel/src/livelog.rs:103-120` starts live logging outside the boot-critical
path, and `kernel/src/livelog.rs:173-195` retries USB/WOVENLOG discovery in a
deferred worker. The empty returned volume means no accepted native log was
produced, even though the image contains the expected partition.

## Qualification decision

This attempt establishes only that the native UEFI image boots far enough to
run the kernel text console and accept keyboard input on this laptop. It does
**not** pass WovenShell desktop, sign-in, USB live-log, graphics, Wi-Fi, audio,
webcam, Bluetooth, touchpad, laptop power, or release qualification. No native
failure was recorded in the formal Tier 2 matrix at the time because the exact
failure cause was still unconfirmed; this document preserves the observed
limitation and the next evidence needed. The subsequent exact replies recorded
in `docs/audit-native-hardware-boot-attempt-2-2026-11-25.md` supersede that
provisional Tier 2 classification with fail-closed Tier 3 native failures.
