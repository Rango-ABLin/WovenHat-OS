# Actual QEMU screenshot capture integration

The existing source harness `scripts/screenshot-hello-gui-qemu.py` launches the real Stage 19.2 hello-gui guest and uses QMP screendump after its serial PASS marker. Rather than duplicate device arguments, `capture_hello_gui.py` invokes that harness, propagates failures, and validates that a nonempty PNG exists.

## Capture
On a configured WovenHat developer machine with Rust, QEMU, and OVMF:
```bash
python tools/wovenassets/capture_hello_gui.py --output target/guest-evidence/hello-gui.png --qemu /usr/bin/qemu-system-x86_64 --firmware /usr/share/OVMF/OVMF_CODE_4M.fd
```
The underlying harness's firmware/boot configuration must match the supported local environment; use the repository's documented paths. This command does not create an accepted visual baseline.

## Qualification
After an independent reviewer approves an actual capture as a reference, place that PNG and its accurate hash into the reviewed baseline manifest specified in `GUEST_VISUAL_GATE.md`. Prepare a capture map pointing at a newly captured guest screenshot and run:
```bash
python tools/wovenassets/run_guest_visual_gate.py reviewed-baselines.json guest-captures.json --report visual-gate.json
```

## Isolation and limitations
- Host tests exercise the capture bridge with an injected harness stub; they do not execute WovenHat in QEMU.
- The present hello-gui harness always boots one CPU, so do not claim 1/2/4-CPU qualification based on it.
- The bridge must not auto-approve reference screenshots or manufacture evidence.
- Keep reviewed PNG references under explicit review and retain a separate kernel/desktop CI gate.
