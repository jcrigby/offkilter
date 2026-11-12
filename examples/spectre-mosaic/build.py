"""The 4 in lid as a bundle of triangular sticks, and the mold that
holds the bundle at the pattern: the hex-edged Spectre (half a hexagon
on every edge, out then in) lies on the triangular lattice of
half-edge triangles, so every tile is sawn from four shapes of stick,
no curve routed, and the slivers between the lattices' runs are left
to the resin. The sticks stand end-on in a silicone comb cast from a
printed positive: the lid's pieces as bars on a plate, poured over in
a printed box whose rim the plate rests on and which backs the comb
after. Two lines show: 0.7 mm between a tile's sticks, the texture,
and 1.5 between tiles, the pieces.

    cargo build --release -p ok-mcp
    python3 examples/spectre-mosaic/build.py
    cargo test -p ok-render --test spectre_mosaic   # what CI runs

Outputs under out/: mosaic.json (the tool's answer: the tile's cover,
every stick on the lid, the counts and sizes), positive.stl and
box.stl (the mold, FDM), lid.svg (the lid at 1:1, every stick, for a
check against a print) and tile.svg (one tile's cover by shape).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4
SIDE = 4.0 * IN                                 # the box's lid, a 4 in square, trimmed from the bundle's slices
EDGE = 10.56                                    # the tile's edge, as the prism lid's: the sticks are half of it on a side
GAP, INNER = 1.5, 0.7                           # between tiles (the pieces) and between a tile's sticks (the texture)
MIN = 1.0                                       # a stick narrower than this after the gaps is resin instead
SHIFT = [2.0, 5.0]                              # the slide of the tiling the prism lid settled on: 21 whole tiles
HEIGHT, BACKING, DRAFT = 10.0, 5.0, 1.0         # the bars: how deep the comb's slots are, the silicone under their tips, their taper
ROUNDS, SEARCH_SEED = 6000, 3                   # the cover's search: repeatable
COLOURS = {"triangle": "#4a7fe0", "half": "#f5a03a", "small": "#12a5a8", "trapezoid": "#9b59d0"}
WOODS = ["#d9b98a", "#7a5230", "#c47d4a", "#9a9a9a"]


def svg_path(pts, scale, origin, flip_y):
    return "M " + " L ".join(f"{origin[0] + x * scale:.3f} {origin[1] - y * scale if flip_y else origin[1] + y * scale:.3f}" for x, y in pts) + " Z"


def main():
    os.makedirs(OUT, exist_ok=True)
    mcp = Mcp(os.path.join(OUT, "scratch.okpart"))
    text = mcp.call("spectre_mosaic", {"width": SIDE, "height": SIDE, "edge": EDGE, "gap": GAP, "inner": INNER, "min": MIN, "shift": SHIFT,
                                       "bar": HEIGHT, "backing": BACKING, "draft": DRAFT, "rounds": ROUNDS, "search_seed": SEARCH_SEED, "out": OUT})
    mcp.close()
    if os.path.exists(os.path.join(OUT, "scratch.okpart")):
        os.remove(os.path.join(OUT, "scratch.okpart"))
    if "ERROR" in text[:40]:
        raise SystemExit(text)
    m = json.loads(text)
    with open(os.path.join(OUT, "mosaic.json"), "w") as f:
        json.dump(m, f, indent=1)
    tile = m["tile"]
    print(f"one tile: {100 * tile['coverage']:.2f} % covered by {len(tile['pieces'])} sticks: " + ", ".join(f"{m['shapes'][s]['per_tile']} {s}" for s in m["shapes"]))
    print(f"the lid: {m['tiles']} whole tiles, {m['sticks']} sticks ({m['dropped']} under {MIN} mm dropped to the resin): " + ", ".join(f"{m['shapes'][s]['count']} {s}" for s in m["shapes"]))
    for s, v in m["shapes"].items():
        print(f"  {s:<10} sides " + " x ".join(f"{x:.2f}" for x in v["sides"]) + f" mm, {v['area']:.1f} mm2")
    (lo, hi), (plo, phi) = m["board"], m["plate"]
    print(f"plate {phi[0] - plo[0]:.0f} x {phi[1] - plo[1]:.0f}, bars {HEIGHT:g} tall; box {m['box'][1][0] - m['box'][0][0]:.0f} x {m['box'][1][1] - m['box'][0][1]:.0f} x {m['box'][2]:.0f}; files {', '.join(os.path.basename(f) for f in m['files'])}")
    # The lid at 1:1 (SVG mm): every stick in its tile's wood, the trim line.
    mm = 3.7795275591                              # px per mm at 96 dpi
    w, h = phi[0] - plo[0] + 10, phi[1] - plo[1] + 10
    with open(os.path.join(OUT, "lid.svg"), "w") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w * mm:.2f} {h * mm:.2f}">\n')
        o = ((5 - plo[0]) * mm, (h - 5 + plo[1]) * mm)
        f.write('<rect width="100%" height="100%" fill="#fff"/>\n')
        for p in m["pieces"]:
            f.write(f'<path d="{svg_path(p["outline"], mm, o, True)}" fill="{WOODS[p["wood"]]}" stroke="none"/>\n')
        f.write(f'<rect x="{o[0] + lo[0] * mm:.2f}" y="{o[1] - hi[1] * mm:.2f}" width="{(hi[0] - lo[0]) * mm:.2f}" height="{(hi[1] - lo[1]) * mm:.2f}" fill="none" stroke="#c00" stroke-width="1" stroke-dasharray="8 4"/>\n')
        f.write(f'<text x="{o[0] + lo[0] * mm + 4:.1f}" y="{o[1] - lo[1] * mm - 4:.1f}" font-family="sans-serif" font-size="11" fill="#c00">trim to {hi[0] - lo[0]:.1f} x {hi[1] - lo[1]:.1f}; {m["sticks"]} sticks in {m["tiles"]} tiles, edge {EDGE} mm, lines {INNER} and {GAP} mm; print at 100 %</text>\n')
        f.write("</svg>\n")
    # One tile's cover by shape, at 60 px per edge.
    s = 60.0
    pts = [q for p in tile["pieces"] for q in p["outline"]]
    xs, ys = [q[0] for q in pts], [q[1] for q in pts]
    o = (20 - min(xs) * s, 20 + max(ys) * s)
    with open(os.path.join(OUT, "tile.svg"), "w") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{(max(xs) - min(xs)) * s + 40:.0f}" height="{(max(ys) - min(ys)) * s + 70:.0f}">\n<rect width="100%" height="100%" fill="#fff"/>\n')
        for p in tile["pieces"]:
            f.write(f'<path d="{svg_path(p["outline"], s, o, True)}" fill="{COLOURS[p["shape"]]}" stroke="#fff" stroke-width="0.8"/>\n')
        f.write(f'<text x="20" y="{(max(ys) - min(ys)) * s + 55:.0f}" font-family="sans-serif" font-size="13">{100 * tile["coverage"]:.2f} % of the tile: ' + ", ".join(f"{tile['counts'][i]} {name}" for i, name in enumerate(["triangles", "halves", "small", "trapezoids"])) + "</text>\n</svg>\n")
    print("wrote mosaic.json, lid.svg, tile.svg")


if __name__ == "__main__":
    main()
