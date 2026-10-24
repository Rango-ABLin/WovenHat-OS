# Stage 13.15 WovenAudio playback for userspace audit — 2026-10-14

## What changed

**Capability.** `AudioPlayback` is capability 24, so `Capability::COUNT` is
now 25. It is in the system-service ceiling and in the user ceiling. The
permission broker maps the new `Permission::Audio` (16) to it, so a user
application gets it only when the broker grants it. Recording, not yet
implemented, will use the existing broker-gated `Microphone` capability.

**HDA output (`kernel/src/hda.rs`).** `start_output`, called from bring-up:
1. finds the output route (converter, pin, amplifier) and enables the pin;
2. sets the amplifier to 75%;
3. allocates the playback stream's buffer descriptor list and 16 DMA pages
   (64 KiB) once;
4. binds the controller's MSI-X or MSI vector through the driver manager.

**`play`.** It accepts 48 kHz 16-bit stereo, whole frames, up to 64 KiB.
1. Only one playback runs at a time. The caller registers as the vector's
   waiter (`interrupts::set_pci_vector_waiter`, released with the new
   `clear_pci_vector_waiter`).
2. It fills the pages and the descriptor list, with interrupt-on-completion
   on the last entry only.
3. It starts the stream with IOCE and stream interrupts enabled.
4. It sleeps in `wait_for_event_until` until BCIS shows completion, or the
   deadline passes. The deadline is twice the playing time plus half a
   second.
5. It stops the stream and clears status and the interrupt enable.

`set_volume` sets the amplifier gain; 0 mutes.

**Syscalls (`kernel/src/syscall.rs`).**
- **203 `AudioInfo`:** returns the output format.
- **204 `AudioPlay`:** checks `AudioPlayback`. It copies the PCM into
  kernel memory before taking the controller lock, so a user page fault
  never happens under the IRQ lock. Every attempt is recorded in the
  security ledger as `Action::AudioPlay`, allowed or denied.
- **205 `AudioSetVolume`:** checks `AudioPlayback`.

**libwoven.** `audio_info`, `audio_play` and `audio_set_volume`.

**hello-gui `--audio`.** Plays a quarter second of a 500 Hz square wave.
The wave is filled at run time in `.bss`, so it is neither on the 8 KiB
stack nor in the 64 KiB-limited image.

## Gate

`[S13.15] WovenAudio playback: PASSED`. Harness:
`scripts/test-stage13-15-audio.py`.

The machine is q35 with an Intel HDA controller and a `hda-duplex` codec.
QEMU records the codec's output to a WAV file at 48 kHz 16-bit stereo.
The gate checks:
- hda0 is ready, with output on a PCI device vector;
- a client with `AudioPlayback` plays the wave and exits 0, with exactly one
  completed playback;
- the completion arrives as a device interrupt and wakes the player before
  its deadline (counted by `AUDIO_ON_TIME`);
- the same client without the capability exits 3 (refused), and nothing
  more is played.

The harness then requires the recording to hold the wave: at least
three-quarters of its 12,000 frames loud, and the sign changing every
48 ± 2 frames. QEMU leaves the WAV sizes unset when the guest exits through
`isa-debug-exit`, so the data chunk is read to the end of the file.

## Evidence (`audit-artifacts/`)

Stage 13.15 on 1/2/4 CPUs, all passing: `stage13.15-1cpu-1792005061603763000`, `stage13.15-2cpu-1792004414605407100`, `stage13.15-4cpu-1792004486814112100`

Negative controls, each on 1 CPU with the source restored byte for byte
afterwards. All five built, and each failed the gate as intended:
- **Capability not checked:** the client without `AudioPlayback` is not
  refused.
- **No completion interrupt:** no interrupt, and playback times out.
- **Player not registered as the waiter:** interrupts arrive, but the
  player only notices at its deadline, so the on-time check fails.
- **Silence copied instead of the PCM:** the recording has no loud frames.
- **Output muted:** the recording has no loud frames.

After the restore, the gate passes again on 1 CPU.

Regression on 1/2/4 CPUs, all passing:
- the HDA driver (13.8), the permission broker (15.3), signed applications
  (15.4) and the security ledger (15.8), all affected by the new capability
  and audit action:
- 13.8: `stage13.8-1cpu-1792005158794826400`, `stage13.8-2cpu-1792005254650444900`, `stage13.8-4cpu-1792005324219174200`
- 15.3: `stage15.3-1cpu-1792005390267631200`, `stage15.3-2cpu-1792005476337633700`, `stage15.3-4cpu-1792005542090239400`
- 15.4: `stage15.4-1cpu-1792005607690301900`, `stage15.4-2cpu-1792005694779679700`, `stage15.4-4cpu-1792005762415378500`
- 15.8: `stage15.8-1cpu-1792005829865106300`, `stage15.8-2cpu-1792005917530759900`, `stage15.8-4cpu-1792005984911482100`
- Stage 13.12 `devices` and `production`, 13.13 and 13.14 `nvme` on 2 CPUs;
  Stages 20.11 and 20.12 on 1 CPU.

**Lint.** Warning-denying Clippy passes for:
- production;
- `stage13-12-test` through `stage13-15-test`, and `stage13-8-test`;
- `stage15-3-test`, `stage15-4-test` and `stage15-8-test`;
- `stage20-12-test`;
- host `--all-targets`.

## Limits

- **One stream, one buffer at a time.** Playback is a single blocking
  buffer of up to 64 KiB, which is about a third of a second. There is no
  queue for gapless streaming and no mixing of several clients.
- **Fixed format.** 48 kHz 16-bit stereo only, with no resampling.
- **No recording.** Capture behind `Microphone` is the next audio step.
- **QEMU only.** Physical codecs are not qualified.
