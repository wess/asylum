#!/usr/bin/env python3
"""Export the Agent sprites (crates/app/sprites/*.txt) as 16x16 PNGs in each
class's own colors for the website: site/assets/sprites/<class>.png.

The palette mirrors crates/app/src/avatar/sprite.rs; keep the two in step."""
import glob, os
from PIL import Image

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
out = os.path.join(root, "site/assets/sprites")

def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))

def shade(c, f):
    return tuple(min(255, int(x * f)) for x in c)

def main():
    os.makedirs(out, exist_ok=True)
    for path in sorted(glob.glob(os.path.join(root, "crates/app/sprites/*.txt"))):
        lines = open(path).read().splitlines()
        meta = dict(kv.split("=") for line in lines if line.startswith("#") for kv in line[1:].split() if "=" in kv)
        rows = [l for l in lines if l and not l.startswith("#")]
        p, a, h, s = (rgb(meta[k]) for k in ("primary", "accent", "hair", "skin"))
        ink = (27, 20, 38)
        pal = {"o": ink, "e": ink, "s": s, "S": shade(s, .82), "h": h, "H": shade(h, .72), "p": p, "P": shade(p, .72),
               "a": a, "A": shade(a, .72), "m": (201, 209, 220), "M": (125, 135, 152), "w": (255, 255, 255),
               "b": (122, 74, 42), "B": (78, 46, 26), "g": (255, 216, 102)}
        im = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
        for y, row in enumerate(rows[:16]):
            for x, ch in enumerate(row[:16]):
                if ch in pal:
                    im.putpixel((x, y), pal[ch] + (255,))
        name = os.path.basename(path)[:-4]
        im.save(os.path.join(out, f"{name}.png"), optimize=True)
        print(name)

if __name__ == "__main__":
    main()
