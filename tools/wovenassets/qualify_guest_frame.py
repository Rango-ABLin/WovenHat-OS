#!/usr/bin/env python3
"""Fail-closed comparison of guest captures with reviewed, SHA-256 pinned references."""
import argparse, hashlib, json
from pathlib import Path
from PIL import Image
from compare_frames import compare

def qualify(manifest, baseline_id, capture):
    manifest=Path(manifest)
    obj=json.loads(manifest.read_text(encoding="utf-8"))
    if obj.get("schema_version")!=1 or obj.get("kind")!="wovenhat-guest-baselines":
        raise ValueError("invalid baseline manifest")
    items=obj.get("baselines")
    if not isinstance(items,list):raise ValueError("missing baseline list")
    selected=[b for b in items if isinstance(b,dict) and b.get("id")==baseline_id]
    if len(selected)!=1:raise ValueError("baseline identifier must uniquely exist")
    b=selected[0]
    for key in ("path","sha256","reviewed_by","guest_commit","capture_method"):
        if not isinstance(b.get(key),str) or not b[key].strip():raise ValueError("baseline missing "+key)
    dims=b.get("size")
    if not isinstance(dims,list) or len(dims)!=2 or any(type(v)!=int or v<=0 for v in dims):
        raise ValueError("invalid baseline dimensions")
    for key in ("max_abs","mean_abs"):
        val=b.get(key)
        if type(val) not in (int,float) or not 0<=val<=255:raise ValueError("invalid threshold "+key)
    if len(b["sha256"])!=64 or any(c not in "0123456789abcdef" for c in b["sha256"]):
        raise ValueError("invalid SHA-256")
    rel=Path(b["path"])
    if rel.is_absolute() or ".." in rel.parts or rel.suffix.lower()!=".png":
        raise ValueError("unsafe baseline path")
    path=(manifest.parent/rel).resolve()
    if not path.is_relative_to(manifest.parent.resolve()):raise ValueError("baseline escapes directory")
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest()!=b["sha256"]:
        raise ValueError("reviewed baseline missing or hash mismatch")
    with Image.open(path) as image:
        if list(image.size)!=dims:raise ValueError("reviewed baseline dimensions mismatch")
    result=compare(path,capture,max_abs=b["max_abs"],mean_abs=b["mean_abs"])
    return {"baseline":baseline_id,"reviewed_by":b["reviewed_by"],"guest_commit":b["guest_commit"],**result}

if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("manifest");parser.add_argument("baseline_id");parser.add_argument("guest_capture")
    args=parser.parse_args()
    try:
        result=qualify(args.manifest,args.baseline_id,args.guest_capture)
        print(json.dumps(result,indent=2))
        raise SystemExit(0 if result["pass"] else 1)
    except (OSError,ValueError,KeyError) as error:
        print(json.dumps({"pass":False,"error":str(error)}))
        raise SystemExit(2)
