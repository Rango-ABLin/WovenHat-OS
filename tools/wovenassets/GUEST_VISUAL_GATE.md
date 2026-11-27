# Guest graphics evidence gate

This host-only entrypoint consumes **actual** WovenHat QEMU screenshots plus human-reviewed reference PNGs; it does not capture guest images or assert a QEMU pass.

1. Start WovenHat in the existing project-specific QEMU boot harness; use an actual guest screenshot mechanism, such as QEMU's monitor `screendump`, and preserve the image and command logs.
2. Have a maintainer review and explicitly approve a reference screenshot. Record its SHA-256, pixel size, reviewer identity, originating guest commit and capture method in a JSON manifest.
3. Place current run screenshot files beside a capture map (paths resolved relative to capture-map directory), with exactly one capture per reviewed baseline ID.
4. Run `python tools/wovenassets/run_guest_visual_gate.py reviewed-baselines.json guest-captures.json --report result.json`.
5. Exit 0 means the supplied files matched the reviewed visual baselines; exit 1 means a pixel difference; exit 2 means invalid or incomplete evidence.

## Reference manifest schema
```json
{
  "schema_version": 1,
  "kind": "wovenhat-guest-baselines",
  "baselines": [{
    "id": "desktop",
    "path": "reviewed/desktop.png",
    "sha256": "<64 lowercase hexadecimal characters from actual approved image>",
    "size": [1280, 800],
    "reviewed_by": "maintainer-review-id",
    "guest_commit": "<verified Git SHA>",
    "capture_method": "QEMU monitor screendump",
    "max_abs": 0,
    "mean_abs": 0
  }]
}
```
## Capture map schema
```json
{"schema_version": 1, "captures": [{"id": "desktop", "path": "captures/desktop.png"}]}
```

No reference PNGs are checked in by this change: approvals and actual guest screenshot evidence must come from the live QEMU environment. Never use host-generated mockups as reference guest screenshots. This is a visual gate only; run the existing 1/2/4-CPU OS acceptance matrix separately.
