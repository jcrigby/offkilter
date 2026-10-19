#!/usr/bin/env python3
"""A Spectre top for a 4 in puzzle box, built through the MCP server:
the aperiodic monotile with circular-arc edges on a 101.6 mm square,
9 mm edges, every piece routed to its own template with a 3/16 in
flush trim bit, so the tile's four inside corners are filleted to the
bit and the convex corners that meet them rounded to match, and the
pieces still nest with no gap. Maple for the field and walnut for the
mystic pairs.

    cargo build --release -p ok-mcp
    python3 examples/spectre-box/build.py
    cargo test -p ok-render --test spectre_box    # what CI runs

Writes out/spectre_box.okpart, screenshots, the piece outlines as a DXF
at 1:1 (one closed outline per piece, the routing templates) and the
pieces as STL.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4
SIDE, THICKNESS = 4.0 * IN, 12.0                # the box's top, a 4 in square
EDGE = 9.0                                      # the tile's edge: a tile is about 43 x 29, 25 or so pieces on the board
CURVE, BULGE = "arc", 0.3                       # circular-arc edges of sagitta 0.3 edge: one radius, 5.1 mm, all along
BIT = 3.0 / 16.0 * IN                           # the flush trim bit every piece is routed with: 4.76 mm, so 2.4 mm fillets
GAP = 0.0                                       # each piece is cut to its own template, so the pieces nest tight
SCRAP = 40.0                                    # a piece under this (mm2) is not worth routing: resin, or a bigger neighbour
LIGHT = {"name": "Maple", "density": 0.7}       # g/cm3: the parts list names the species
DARK = {"name": "Walnut", "density": 0.64}      # the mystic pairs
# The board's edge cuts the outer tiles wherever it falls; these slides
# of the tiling are tried and the one whose smallest piece is biggest
# is kept, so nothing fiddly has to be routed.
SHIFTS = [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 2.0), (4.0, -4.0), (2.0, 5.0), (-5.0, -3.0), (4.0, 4.0)]


def pieces_of(report):
    return [b for b in report["bodies"] if b["name"].startswith("Piece ")]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_box.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "spectre_box"})
    top = Part(mcp, 1, "Top")
    top.apply({"type": "rename_document", "name": "Spectre box top"}, {"type": "rename_tab", "tab": 1, "name": "Top"})
    feature = top.feature(
        {
            "type": "add_spectre",
            "plane": {"type": "standard", "base": "top", "offset": 0},
            "width": SIDE,
            "height": SIDE,
            "edge": EDGE,
            "thickness": THICKNESS,
            "gap": GAP,
            "curve": CURVE,
            "bulge": BULGE,
            "bit": BIT,
            "scrap": SCRAP,
            "light": LIGHT,
            "dark": DARK,
            "name": "Spectre top",
        }
    )
    best = None
    for shift in SHIFTS:
        top.apply({"type": "set_spectre", "id": feature, "shift": v2(*shift)})
        pieces = pieces_of(top.report())
        smallest = min(b["volume"] for b in pieces) / THICKNESS
        print(f"shift {shift[0]:+.0f},{shift[1]:+.0f}: {len(pieces)} pieces, the smallest {smallest:.0f} mm2")
        if best is None or smallest > best[1]:
            best = (shift, smallest)
    top.apply({"type": "set_spectre", "id": feature, "shift": v2(*best[0])})
    report = top.report()
    if report["features"][0].get("error"):
        raise RuntimeError(report["features"][0]["error"])
    pieces = pieces_of(report)
    light = [b for b in pieces if b["name"].endswith("light")]
    dark = [b for b in pieces if b["name"].endswith("dark")]
    areas = sorted(b["volume"] / THICKNESS for b in pieces)
    print(f"shift {best[0]}: {len(pieces)} pieces, {len(light)} maple and {len(dark)} walnut")
    print(f"pieces {areas[0]:.0f} to {areas[-1]:.0f} mm2, together {sum(areas):.0f} of the board's {SIDE * SIDE:.0f}")
    print(f"maple {sum(b['volume'] for b in light) / 1000:.1f} cm3, walnut {sum(b['volume'] for b in dark) / 1000:.1f} cm3")
    for view, name in (("iso", "box_iso"), ("top", "box_top")):
        mcp.call("screenshot", {"tab": 1, "view": view, "width": 1000, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    # The templates: one closed outline per piece at 1:1, and the pieces as STL.
    print(mcp.call("export", {"tab": 1, "format": "dxf", "view": "top", "path": os.path.join(OUT, "templates.dxf")}))
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "pieces.stl")}))
    # The rule the bit imposes: Kaplan's cubic at an 8 mm edge is tighter
    # than the bit, and the feature says so rather than drawing it.
    top.mcp.call("apply", {"ops": [{"type": "set_spectre", "id": feature, "curve": "cubic", "bulge": 0.6, "edge": 8.0}], "tab": 1})
    error = top.report()["features"][0].get("error", "")
    print(f"cubic at 8 mm: {error!r}")
    assert "tightest radius" in error
    top.apply({"type": "set_spectre", "id": feature, "curve": CURVE, "bulge": BULGE, "edge": EDGE})
    # And the same top without the bit, for the picture of what it fixes.
    top.apply({"type": "set_spectre", "id": feature, "bit": 0.0})
    mcp.call("screenshot", {"tab": 1, "view": "top", "width": 1000, "height": 1000, "path": os.path.join(OUT, "sharp_top.png")})
    top.apply({"type": "set_spectre", "id": feature, "bit": BIT})
    report = top.report()
    assert len(pieces_of(report)) == len(pieces)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
