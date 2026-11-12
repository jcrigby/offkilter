"""A test strip for the line between tiles: three samples of the lid
side by side in one STL, the same patch of tiles in each, with the
line at the current width, narrower, and narrower still, left to
right. Each sample is a 40 mm square cut from the lid, every tile a
3 mm pad on a thin base, so the gaps between pads are the lines;
printed in a dark filament they read as the dark glue will. Notches
in the base's front edge count the samples: one for the current line,
two for the narrower, three for the narrowest.

    cargo build --release -p ok-mcp
    python3 examples/spectre-cauls/gap_sample.py            # the strip of three
    python3 examples/spectre-cauls/gap_sample.py --round    # one sample at the chosen line, its tiles' top edges rounded

Writes out/gap_sample.stl and out/gap_sample.png, or with --round
out/gap_sample_round.stl and .png: the sample at ROUND_GAP with a
fillet of ROUND on every tile's top edges, the way a tile sanded over
at its edges would look, so the line reads deeper than it is.
"""
import json
import math
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
ROUND_GAP, ROUND = 0.7, 0.6                     # the --round sample: the line the eye picked, and the radius on the tiles' top edges
LAYER = 0.2                                     # the printer's layer: the roundover is built as the slicer would build it, a step a layer


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


def sample(k, gap, round_over=0.0):
    path = os.path.join(OUT, f"gap_sample_{k + 1}.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": f"gap sample {gap}"})
    mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": f"Line {gap} mm"}]})
    p = Part(mcp, 1, f"gap {gap}")
    # The pads: the lid's pieces, 3 mm tall, sunk into the base. With a
    # roundover, the pad stops its radius short of the top and the rest
    # is a layer at a time, each eroded to where the circle stands at
    # that height, which is what the slicer makes of a true fillet.
    def lid(z0, z1, g, name):
        p.feature({"type": "add_spectre", "plane": {"type": "standard", "base": "top", "offset": z0}, "width": SIDE, "height": SIDE, "edge": EDGE, "thickness": z1 - z0,
                   "gap": g, "curve": "hex", "bulge": 0.433, "bit": 0.0, "whole": False, "scrap": 12.0, "shift": v2(*SHIFT), "show": "design", "name": name})
    lid(-SINK, PAD - round_over, gap, f"lid at {gap}")
    pads = len(p.bodies())
    if round_over > 0.0:
        steps = max(1, round(round_over / LAYER))
        for i in range(steps):
            h = (i + 0.5) * round_over / steps
            inset = round_over - math.sqrt(max(0.0, round_over * round_over - h * h))
            z0 = PAD - round_over + i * round_over / steps
            lid(z0 - 0.05, z0 + round_over / steps, gap + 2.0 * inset, f"roundover layer {i + 1}, {inset:.2f} in")
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


def picture(stl, name, view):
    """A picture of an STL, by importing it back."""
    path = os.path.join(OUT, "gap_sample_view.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "gap sample"})
    mcp.call("import", {"path": stl, "name": "gap sample", "tab": 1})
    mcp.call("screenshot", {"tab": 1, "view": view, "width": 1800, "height": 700 if "round" not in name else 1200, "path": os.path.join(OUT, name)})
    mcp.close()
    if os.path.exists(path):
        os.remove(path)


def main():
    os.makedirs(OUT, exist_ok=True)
    if "--round" in sys.argv:
        pads, tris = sample(2, ROUND_GAP, ROUND)
        out = os.path.join(OUT, "gap_sample_round.stl")
        write_stl(out, tris, f"offkilter spectre gap sample: {ROUND_GAP} line, {ROUND} roundover")
        print(f"round sample: line {ROUND_GAP} mm, roundover {ROUND}, {pads} pads, {len(tris)} triangles; wrote {out}")
        picture(out, "gap_sample_round.png", "-0.3,-0.8,0.5")
        with open(os.path.join(OUT, "gap_sample_round.json"), "w") as f:
            json.dump({"gap": ROUND_GAP, "round": ROUND, "side": SIDE, "pad": PAD, "base": BASE, "sink": SINK, "height": PAD + BASE + SINK, "pads": pads, "triangles": len(tris)}, f, indent=1)
        return
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
    picture(out, "gap_sample.png", "-0.2,-0.7,0.7")
    with open(os.path.join(OUT, "gap_sample.json"), "w") as f:
        json.dump({"gaps": GAPS, "side": SIDE, "pad": PAD, "base": BASE, "sink": SINK, "height": PAD + BASE + SINK, "pitch": PITCH, "pads": counts, "triangles": len(strip)}, f, indent=1)


if __name__ == "__main__":
    main()
