#!/usr/bin/env python3
"""A Spectre puzzle top for the puzzle box, built through the MCP server:
the curved aperiodic monotile (Smith, Myers, Kaplan and Goodman-Strauss)
laid under the checkerboard top's board, every tile the board reaches a
piece with a 1.5 mm gap for a contrasting resin fill, maple for the
field and walnut for the mystic pairs, a 5 mm alignment web in the gaps
and the printable pattern plate that cuts every piece from one board
with a pilot and a bit the size of the gap. The rounded Spectre's edges
alternate in and out like a jigsaw's, which is what makes it a puzzle
top; they do not undercut, so the resin and the substrate hold it, as
they hold the checkerboard.

    cargo build --release -p ok-mcp
    python3 examples/spectre-top/build.py
    cargo test -p ok-render --test spectre_top    # what CI runs

Writes out/spectre_top.okpart, screenshots, the piece outlines as a DXF
at 1:1, the pieces and the pattern plate as STL.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
WIDTH, HEIGHT, THICKNESS = 304.0, 228.0, 12.0   # the checkerboard top's board: 8 x 6 of 38
EDGE = 12.7                                     # the tile's edge, half an inch; a tile is about 60 x 41
GAP, WEB = 1.5, 5.0
SCRAP = 150.0                                   # a piece under this (mm2, about 12 x 12) is resin, not a piece to cut and glue
GROOVE = THICKNESS + 0.5                        # the pilot bottoms there on the last pass, the bit just through
LIGHT = {"name": "Maple", "density": 0.7}       # g/cm3: the parts list names the species
DARK = {"name": "Walnut", "density": 0.64}      # the mystic pairs
# The board's edge cuts the outer tiles wherever it falls, and what it
# cuts under SCRAP goes to the resin; these slides of the tiling are
# tried and the one that keeps the most wood is kept.
SHIFTS = [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (-4.0, 3.0), (6.0, -5.0), (3.0, 7.0), (-7.0, -4.0), (5.0, 5.0)]


def pieces_of(report):
    return [b for b in report["bodies"] if b["name"].startswith("Piece ")]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_top.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "spectre_top"})
    top = Part(mcp, 1, "Top")
    top.apply({"type": "rename_document", "name": "Spectre top"}, {"type": "rename_tab", "tab": 1, "name": "Top"})
    feature = top.feature(
        {
            "type": "add_spectre",
            "plane": {"type": "standard", "base": "top", "offset": 0},
            "width": WIDTH,
            "height": HEIGHT,
            "edge": EDGE,
            "thickness": THICKNESS,
            "gap": GAP,
            "scrap": SCRAP,
            "web": WEB,
            "groove": GROOVE,
            "light": LIGHT,
            "dark": DARK,
            "name": "Spectre puzzle",
        }
    )
    best = None
    for shift in SHIFTS:
        top.apply({"type": "set_spectre", "id": feature, "shift": v2(*shift)})
        pieces = pieces_of(top.report())
        wood = sum(b["volume"] for b in pieces) / THICKNESS
        smallest = min(b["volume"] for b in pieces) / THICKNESS
        print(f"shift {shift[0]:+.0f},{shift[1]:+.0f}: {len(pieces)} pieces of {wood / 100:.0f} cm2, the smallest {smallest:.0f} mm2")
        if best is None or wood > best[1]:
            best = (shift, wood)
    top.apply({"type": "set_spectre", "id": feature, "shift": v2(*best[0])})
    report = top.report()
    if report["features"][0].get("error"):
        raise RuntimeError(report["features"][0]["error"])
    pieces = pieces_of(report)
    light = [b for b in pieces if b["name"].endswith("light")]
    dark = [b for b in pieces if b["name"].endswith("dark")]
    web = next(b for b in report["bodies"] if b["name"] == "Alignment web")
    areas = sorted(b["volume"] / THICKNESS for b in pieces)
    print(f"shift {best[0]}: {len(pieces)} pieces, {len(light)} maple and {len(dark)} walnut, {len(report['bodies'])} bodies")
    print(f"pieces {areas[0]:.0f} to {areas[-1]:.0f} mm2; maple {sum(b['volume'] for b in light) / 1000:.1f} cm3, walnut {sum(b['volume'] for b in dark) / 1000:.1f} cm3")
    print(f"web {web['volume'] / WEB:.0f} mm2 of the board's {WIDTH * HEIGHT:.0f}: {web['volume'] / WEB / (WIDTH * HEIGHT) * 100:.1f} % resin")
    for view, name in (("iso", "spectre_iso"), ("top", "spectre_top"), ("0.3,-1,0.5", "spectre_front")):
        mcp.call("screenshot", {"tab": 1, "view": view, "width": 1200, "height": 900, "path": os.path.join(OUT, f"{name}.png")})
    # The templates: every outline at 1:1, and the pieces as STL.
    print(mcp.call("export", {"tab": 1, "format": "dxf", "view": "top", "path": os.path.join(OUT, "templates.dxf")}))
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "pieces.stl")}))
    # The straight-sided tile, for comparison, then Kaplan's curve again.
    top.apply({"type": "set_spectre", "id": feature, "bulge": 0.0})
    straight = pieces_of(top.report())
    print(f"straight-sided: {len(straight)} pieces")
    mcp.call("screenshot", {"tab": 1, "view": "top", "width": 1200, "height": 900, "path": os.path.join(OUT, "straight_top.png")})
    top.apply({"type": "set_spectre", "id": feature, "bulge": 0.6})
    # The pattern plate: its own layout, exported to print, then back.
    top.apply({"type": "set_spectre", "id": feature, "show": "pattern"})
    plate = top.report()["bodies"]
    print(f"pattern plate: {len(plate)} body, {plate[0]['volume'] / 1000:.0f} cm3")
    mcp.call("screenshot", {"tab": 1, "view": "iso", "width": 1200, "height": 900, "path": os.path.join(OUT, "pattern_plate.png")})
    print(mcp.call("export", {"tab": 1, "format": "stl", "body": "Pattern plate", "path": os.path.join(OUT, "pattern_plate.stl")}))
    top.apply({"type": "set_spectre", "id": feature, "show": "design"})
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
