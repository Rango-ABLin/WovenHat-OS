#!/usr/bin/env python3
"""Host-only deterministic screenshot comparison. Exit: 0 pass, 1 mismatch, 2 invalid input."""
import argparse,json,sys
from PIL import Image,ImageChops,ImageStat

def compare(reference,actual,max_abs=0,mean_abs=0.0,out=None):
    with Image.open(reference) as ri, Image.open(actual) as ai:
        r=ri.convert('RGBA');a=ai.convert('RGBA')
        if r.size!=a.size:return {'pass':False,'reason':'dimension_mismatch','reference':list(r.size),'actual':list(a.size)}
        diff=ImageChops.difference(r,a)
        extrema=diff.getextrema();maximum=max(v[1] for v in extrema);mean=sum(ImageStat.Stat(diff).mean)/4
        if out:diff.save(out)
        return {'pass':maximum<=max_abs and mean<=mean_abs,'size':list(r.size),'max_absolute_channel_difference':maximum,'mean_absolute_channel_difference':round(mean,6)}

def main():
    p=argparse.ArgumentParser();p.add_argument('reference');p.add_argument('actual');p.add_argument('--max-abs',type=int,default=0);p.add_argument('--mean-abs',type=float,default=0.0);p.add_argument('--diff-png');a=p.parse_args()
    try:
        if not 0<=a.max_abs<=255 or a.mean_abs<0:raise ValueError('invalid thresholds')
        r=compare(a.reference,a.actual,a.max_abs,a.mean_abs,a.diff_png)
        print(json.dumps(r,indent=2));return 0 if r['pass'] else 1
    except (OSError,ValueError) as e:print(json.dumps({'error':str(e)}),file=sys.stderr);return 2
if __name__=='__main__':sys.exit(main())
