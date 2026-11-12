#!/usr/bin/env python3
"""Generate original Skell Studio PNG + Windows multi-resolution ICO (stdlib only).

Provisional visual identity; permanent official skull is owned by the SkellCore
design system. Does not download anything or use third-party image assets.
"""
from pathlib import Path
from struct import pack
import zlib

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates/os-app/assets"
HEAD = [(27,10),(38,5),(63,5),(75,10),(85,26),(84,49),(77,63),(69,70),
        (69,84),(62,90),(55,85),(50,91),(45,85),(39,90),(32,84),(32,70),
        (24,63),(17,49),(16,26)]
LEFT = [(24,36),(45,37),(40,53),(31,55),(23,48)]
RIGHT = [(55,37),(76,36),(77,48),(69,55),(60,53)]
NOSE = [(50,54),(56,62),(44,62)]

def polygon(x, y, vertices):
    inside = False
    for i, (x1,y1) in enumerate(vertices):
        x2,y2 = vertices[(i+1)%len(vertices)]
        if (y1>y) != (y2>y):
            cross = (x2-x1)*(y-y1)/(y2-y1) + x1
            if x < cross:
                inside = not inside
    return inside

def solid(x,y):
    if not polygon(x,y,HEAD):
        return False
    if polygon(x,y,LEFT) or polygon(x,y,RIGHT) or polygon(x,y,NOSE):
        return False
    if (38.4<x<40.8 or 48.8<x<51.2 or 59.4<x<61.8) and 71<y<86:
        return False
    return True

def sample(x,y):
    if not solid(x,y):
        return (0,0,0,0)
    # Brighter at the crown, grounded hardware-green toward the jaw.
    fade = min(1.,max(0.,(y-5.)/90.))
    r = round(193-(193-64)*fade)
    g = round(255-(255-171)*fade)
    b = round(126-(126-53)*fade)
    # Green highlight along the original silhouette's perimeter.
    edge = any(not solid(x+dx,y+dy) for dx,dy in ((1.1,0),(-1.1,0),(0,1.1),(0,-1.1)))
    if edge:
        r,g,b = 155,255,67
    return (r,g,b,255)

def png(size):
    # 2x2 coverage anti-aliasing; build transparent, original RGBA pixels.
    raw = bytearray()
    for py in range(size):
        raw.append(0)
        for px in range(size):
            accum = [0,0,0,0]
            for oy in (0.25,0.75):
                for ox in (0.25,0.75):
                    x = 100.0*(px+ox)/size
                    y = 100.0*(py+oy)/size
                    rgba = sample(x,y)
                    for c in range(4):
                        accum[c] += rgba[c]
            raw.extend(v//4 for v in accum)
    def chunk(kind, data):
        return pack(">I",len(data)) + kind + data + pack(">I",zlib.crc32(kind+data)&0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR",pack(">IIBBBBB",size,size,8,6,0,0,0))
            + chunk(b"IDAT",zlib.compress(bytes(raw),9)) + chunk(b"IEND",b""))

def ico(images):
    entries = bytearray()
    payload = bytearray()
    offset = 6 + len(images)*16
    for size, data in images:
        entries.extend(pack("<BBBBHHII",size if size < 256 else 0,
                            size if size < 256 else 0,0,0,1,32,len(data),offset))
        payload.extend(data)
        offset += len(data)
    return pack("<HHH",0,1,len(images)) + entries + payload

def main():
    OUT.mkdir(parents=True,exist_ok=True)
    images = [(n,png(n)) for n in (16,24,32,48,64,128,256)]
    (OUT/"skell-studio.png").write_bytes(images[-1][1])
    (OUT/"skell-studio.ico").write_bytes(ico(images))
    # Identical geometry for nightly builds keeps a recognizable universal brand.
    (OUT/"skell-studio-nightly.ico").write_bytes(ico(images))
    print("Generated original Skell Studio icon PNG and Windows ICO (7 resolutions)")
    print("Sizes:",{p.name:p.stat().st_size for p in OUT.glob("skell-studio*.ico")})

if __name__=="__main__":
    main()
