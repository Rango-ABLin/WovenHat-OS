# WovenAssets host-side graphics qualification (parallel track)

Independent host tooling for WovenHat graphics work, based on commit `353a2c2190987c8b3da31344c06f122b132840ed`.

## Scope
- `compare_frames.py` compares **actual guest screenshots**, with JSON diagnostics and optional diff PNG.
- `validate_tokens.py` validates the light/dark WovenUI design tokens.
- `wovenui-tokens.json` provides original visual design tokens.
- `test_tools.py` runs deterministic isolated unit tests.

## Running
Install host dependency `python -m pip install Pillow`. Then:
```
python -m unittest discover -s tools/wovenassets -p 'test_*.py' -v
python tools/wovenassets/validate_tokens.py tools/wovenassets/wovenui-tokens.json
python tools/wovenassets/compare_frames.py reference.png guest-capture.png --diff-png diff.png
```

Reference images MUST originate from real guest captures; do not use design mockups as pass criteria. This suite does not assert any QEMU/guest result, nor does it change kernel, libwoven, boot packaging or Codex's active graphics work. Binary image fixtures, original SVG sources and additional host tests are in the separately produced WovenAssets v0.1 kit and can be integrated later after explicit review.

## CI handoff
Run host tests as a separate job; only promote screenshot reference baselines after a human review. Use proper 1/2/4-CPU guest-side graphics tests for stage acceptance.
