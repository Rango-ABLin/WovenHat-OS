#!/usr/bin/env python3
"""Offline SVG -> raw RGBA icon packager. Do not parse SVG in the OS kernel."""
import argparse,hashlib,io,json
from pathlib import Path
from xml.etree import ElementTree as ET
import cairosvg
from PIL import Image

SIZES=(16,24,32,48,64)
DENIED={"script","foreignObject","image","use","animate","animateTransform","set","filter"}
def validate_svg(raw):
    if len(raw)>65536 or b"<!DOCTYPE" in raw.upper() or b"<!ENTITY" in raw.upper():raise ValueError("SVG size or entity policy")
    root=ET.fromstring(raw)
    if root.tag!="{http://www.w3.org/2000/svg}svg":raise ValueError("SVG root required")
    if len(list(root.iter()))>256:raise ValueError("too many elements")
    for node in root.iter():
        name=node.tag.rsplit("}",1)[-1]
        if name in DENIED:raise ValueError("forbidden element")
        for key,value in node.attrib.items():
            attr=key.rsplit("}",1)[-1].lower()
            if attr.startswith("on") or attr in ("href","src") or "url(" in value.lower() or "@import" in value.lower():raise ValueError("external resource")
def build(source,dest,sizes=SIZES):
    source=Path(source);dest=Path(dest);dest.mkdir(parents=True,exist_ok=True);entries=[]
    for path in sorted(source.glob("*.svg")):
        raw=path.read_bytes();validate_svg(raw)
        for size in sizes:
            if not 8<=size<=256:raise ValueError("unsupported dimensions")
            png=cairosvg.svg2png(bytestring=raw,output_width=size,output_height=size)
            with Image.open(io.BytesIO(png)) as image:
                if image.size!=(size,size):raise ValueError("raster size mismatch")
                pixels=image.convert("RGBA").tobytes()
            name=f"{path.stem}-{size}.rgba";(dest/name).write_bytes(pixels)
            entries.append({"name":path.stem,"size":size,"file":name,"bytes":len(pixels),"stride":size*4,"sha256":hashlib.sha256(pixels).hexdigest()})
    if not entries:raise ValueError("no SVG icons")
    manifest={"schema_version":1,"format":"RGBA8","alpha":"straight","icons":entries}
    (dest/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf8")
    return manifest
if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("source");p.add_argument("dest");p.add_argument("--sizes",default="16,24,32,48,64")
    a=p.parse_args();print("Icons:",len(build(a.source,a.dest,tuple(map(int,a.sizes.split(","))))["icons"]))
