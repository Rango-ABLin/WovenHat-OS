#!/usr/bin/env python3
"""Validate host-side theme token schema before handoff to target UI."""
import json,re,sys
from pathlib import Path
HEX=re.compile(r"^#[0-9a-fA-F]{6}$")
def validate(path):
    data=json.loads(Path(path).read_text(encoding="utf-8"))
    if data.get("schema_version")!=1:raise ValueError("unsupported schema")
    if set(data.get("themes",{}))!={"dark","light"}:raise ValueError("dark/light themes required")
    required={"background","surface","raised","text","muted","accent","border","danger"}
    for name,theme in data["themes"].items():
        if set(theme)!=required:raise ValueError(f"{name}: theme keys invalid")
        for key,color in theme.items():
            if not isinstance(color,str) or not HEX.fullmatch(color):raise ValueError(f"{name}.{key}: invalid color")
    for key in ("spacing_px","radii_px","type_scale_px","motion_ms"):
        value=data.get(key)
        if not isinstance(value,(list,dict)):raise ValueError(f"invalid {key}")
        entries=value if isinstance(value,list) else value.values()
        if any(type(n) is not int or n<0 for n in entries):raise ValueError(f"invalid {key} values")
    return data
if __name__=="__main__":
    try:
        validate(sys.argv[1] if len(sys.argv)>1 else Path(__file__).with_name("wovenui-tokens.json"))
        print("WovenUI tokens: PASS")
    except (ValueError,OSError,IndexError) as e:
        print(f"WovenUI tokens: FAIL: {e}",file=sys.stderr);sys.exit(1)
