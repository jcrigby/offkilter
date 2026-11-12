"""A test strip for the line between tiles: three samples of the lid
side by side in one STL, the same patch of tiles in each, with the
line at the current width, narrower, and narrower still, left to
right. Each sample is a 40 mm square cut from the lid, every tile a
3 mm pad on a thin base, so the gaps between pads are the lines;
printed in a dark filament they read as the dark glue will. Notches
in the base's front edge count the samples: one for the current line,
two for the narrower, three for the narrowest.

    cargo build --release -p ok-mcp
    python3 examples/spectre-cauls/gap_sample.py

Writes out/gap_sample.stl and out/gap_sample.png.
"""
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
EDGE = 10.56
GAPS = [1.5, 1.0, 0.7]                          # left to right: the current line, narrower, narrower still
SIDE = 40.0                                     # each sample, a square cut from the lid
PAD, BASE, SINK = 3.0, 1.5, 0.3                 # the tiles stand this high; the base under them; the pads sunk into it
MARGIN = 2.0                                    # base past the sample
PITCH = SIDE + 2 * MARGIN + 3.0                 # sample to sample along the strip
SHIFT = [2.0, 5.0]                              # the same slide of the tiling in every sample
NOTCH_W, NOTCH_D = 2.0, 1.0


def read_stl(path):
    data = open(path, "rb").read()
    n = struct.unpack_from("<I", data, 80)[0]
    tris = []
    for i in range(n):
        off = 84 + 50 * i
        v = struct.unpack_from("<12f", data, off)
        tris.append(v)
    return tris


def write_stl(path, tris, label):
    out = bytearray(label.encode()[:80].ljust(80, b"\0"))
    out += struct.pack("<I", len(tris))
    for t in tris:
        out += struct.pack("<12f", *t) + b"\0\0"
    open(path, "wb").write(out)


def sample(k, gap):
    path = os.path.join(OUT, f"gap_sample_{k + 1}.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": f"gap sample {gap}"})
    mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": f"Line {gap} mm"}]})
    p = Part(mcp, 1, f"gap {gap}")
    # The pads: the lid's pieces, 3 mm tall, sunk into the base.
    p.feature({"type": "add_spectre", "plane": {"type": "standard", "base": "top", "offset": -SINK}, "width": SIDE, "height": SIDE, "edge": EDGE, "thickness": PAD + SINK,
               "gap": gap, "curve": "hex", "bulge": 0.433, "bit": 0.0, "whole": False, "scrap": 12.0, "shift": v2(*SHIFT), "show": "design", "name": f"lid at {gap}"})
    pads = len(p.bodies())
    # The base, with the sample's notches in its front edge.
    s = p.sketch("top", -BASE - SINK, "base")
    p.rect(s, (-MARGIN, -MARGIN), (SIDE + MARGIN, SIDE + MARGIN))
    p.extrude(s, BASE, op="new", name="base")
    s = p.sketch("top", 1.0, "notches")
    for i in range(k + 1):
        x0 = SIDE / 2 - (k * 4.0) / 2 + i * 4.0 - NOTCH_W / 2
        p.rect(s, (x0, -MARGIN - 1.0), (x0 + NOTCH_W, -MARGIN + NOTCH_D))
    p.cut(s, BASE + SINK + 2.0, direction="reverse", name=f"{k + 1} notch")
    stl = os.path.join(OUT, f"gap_sample_{k + 1}.stl")
    mcp.call("export", {"tab": 1, "format": "stl", "path": stl})
    mcp.close()
    if os.path.exists(path):
        os.remove(path)
    tris = read_stl(stl)
    os.remove(stl)
    return pads, tris


def main():
    os.makedirs(OUT, exist_ok=True)
    strip = []
    counts = []
    for k, gap in enumerate(GAPS):
        pads, tris = sample(k, gap)
        counts.append(pads)
        dx = k * PITCH
        for t in tris:
            strip.append(tuple(t[i] + (dx if i % 3 == 0 and i >= 3 else 0.0) for i in range(12)))
        print(f"sample {k + 1}: line {gap} mm, {pads} pads, {len(tris)} triangles")
    out = os.path.join(OUT, "gap_sample.stl")
    write_stl(out, strip, "offkilter spectre gap sample: 1.5, 1.0, 0.7 left to right")
    print(f"wrote {out}: {len(strip)} triangles, {2 * PITCH + SIDE + 2 * MARGIN:.0f} mm long")
    # A picture, by importing the strip back.
    path = os.path.join(OUT, "gap_sample_view.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "gap sample"})
    mcp.call("import", {"path": out, "name": "gap sample", "tab": 1})
    mcp.call("screenshot", {"tab": 1, "view": "-0.2,-0.7,0.7", "width": 1800, "height": 700, "path": os.path.join(OUT, "gap_sample.png")})
    mcp.close()
    if os.path.exists(path):
        os.remove(path)
    with open(os.path.join(OUT, "gap_sample.json"), "w") as f:
        json.dump({"gaps": GAPS, "side": SIDE, "pad": PAD, "base": BASE, "sink": SINK, "height": PAD + BASE + SINK, "pitch": PITCH, "pads": counts, "triangles": len(strip)}, f, indent=1)


if __name__ == "__main__":
    main()
