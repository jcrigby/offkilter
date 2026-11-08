#!/usr/bin/env python3
"""A Spectre lid for a 4 in puzzle box from glued-up prisms, built through
the MCP server: the aperiodic monotile with circular-arc edges of five
sixteenths of a circle at a 10.56 mm edge, so a 1/4 in bullnose and a
1/2 in core box cut every bump and flute; the tile split along three
glue lines into five convex prisms, each a stick a table saw rips and a
router table curves, the sticks glued into a tile stick and sliced into
pieces; every tile the board reaches kept whole, with the plan to trim
the glue-up to the 4 in square afterwards. The gap between pieces is one
constant, GAP, and a sample piece is exported to print and try.

    cargo build --release -p ok-mcp
    python3 examples/spectre-lid/build.py
    cargo test -p ok-render --test spectre_lid    # what CI runs

Writes out/spectre_lid.okpart, screenshots, the lid as a PDF at 1:1 with
the trim rectangle (lid.pdf), the piece outlines as a DXF (templates.dxf),
the five prism profiles as a DXF (prisms.dxf), the sample piece as STL
and the numbers the test reads (lid.json).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4
SIDE, THICKNESS = 4.0 * IN, 12.0                # the box's lid, a 4 in square, trimmed from the glue-up
EDGE = 10.56                                    # the tile's edge: five sixteenths of a circle at this edge is a 1/4 in radius
CURVE, BULGE = "arc", 0.2673                    # circular arcs of sagitta 0.2673 edge: 112.6 degrees of a 6.35 mm circle
GAP = 0.7                                       # between pieces, for the glue line or a resin fill; one number, change it here
BIT = 0.0                                       # no flush trim bit: the prisms' faces are the router table's and the saw's
LIGHT = {"name": "Maple", "density": 0.7}       # g/cm3: the parts list names the species
DARK = {"name": "Walnut", "density": 0.64}      # the mystic pairs
SHEETS = ["A4", "A3"]                           # the lid at 1:1: whole tiles around a 4 in square overflow A4, so A3 (or tile the A4 print)
# Whole tiles around the board: the slide of the tiling under it
# decides how many tiles reach it; these are tried and the one with the
# fewest tiles (the least wood past the trim line) is kept.
SHIFTS = [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (-3.0, 2.0), (4.0, -4.0), (2.0, 5.0), (-5.0, -3.0), (4.0, 4.0), (7.0, 1.0), (1.0, -7.0)]


def pieces_of(report):
    return [b for b in report["bodies"] if b["name"].startswith("Piece ")]


def dxf(path, polylines):
    """A minimal DXF of closed polylines, millimetres, 1:1."""
    lines = ["0", "SECTION", "2", "HEADER", "9", "$INSUNITS", "70", "4", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"]
    for layer, pts in polylines:
        lines += ["0", "LWPOLYLINE", "8", layer, "90", str(len(pts)), "70", "1"]
        for x, y in pts:
            lines += ["10", f"{x:.4f}", "20", f"{y:.4f}"]
    lines += ["0", "ENDSEC", "0", "EOF"]
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_lid.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    # The tile's prisms first: the fewest glue lines that leave every
    # prism convex and every bullnose shoulder clear.
    prisms = json.loads(mcp.call("spectre_prisms", {"edge": EDGE, "curve": CURVE, "bulge": BULGE, "gap": GAP}))
    cuts = prisms["cuts"]
    print(f"{len(prisms['prisms'])} prisms in {len(prisms['shapes'])} shapes from {len(cuts)} glue lines; "
          f"arc {prisms['theta']:.1f} degrees, corners under {prisms['limit']:.1f}; {prisms['bits']}")
    assert prisms["clean"], prisms
    for p in prisms["prisms"]:
        faces = ", ".join(f"{f['kind']} {f['length']:.1f}" for f in p["faces"])
        print(f"  prism {p['index']} shape {p['shape']}: {p['area']:.0f} mm2, faces {faces}")
    mcp.call("create_document", {"name": "spectre_lid"})
    top = Part(mcp, 1, "Lid")
    top.apply({"type": "rename_document", "name": "Spectre lid"}, {"type": "rename_tab", "tab": 1, "name": "Lid"})
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
            "whole": True,
            "prisms": cuts,
            "light": LIGHT,
            "dark": DARK,
            "name": "Spectre lid",
        }
    )
    per_tile = len(prisms["prisms"])
    best = None
    for shift in SHIFTS:
        top.apply({"type": "set_spectre", "id": feature, "shift": v2(*shift)})
        pieces = pieces_of(top.report())
        tiles = len(pieces) // per_tile
        wood = sum(b["volume"] for b in pieces) / THICKNESS
        print(f"shift {shift[0]:+.0f},{shift[1]:+.0f}: {tiles} whole tiles, {len(pieces)} pieces, {wood / 100:.0f} cm2 of wood for a {SIDE * SIDE / 100:.0f} cm2 lid")
        if best is None or tiles < best[1]:
            best = (shift, tiles)
    top.apply({"type": "set_spectre", "id": feature, "shift": v2(*best[0])})
    report = top.report()
    if report["features"][0].get("error"):
        raise RuntimeError(report["features"][0]["error"])
    pieces = pieces_of(report)
    light = [b for b in pieces if b["name"].endswith("light")]
    dark = [b for b in pieces if b["name"].endswith("dark")]
    tiles = len(pieces) // per_tile
    assert len(pieces) == tiles * per_tile, len(pieces)
    print(f"shift {best[0]}: {tiles} tiles, {len(pieces)} pieces, {len(light)} maple and {len(dark)} walnut")
    print(f"maple {sum(b['volume'] for b in light) / 1000:.1f} cm3, walnut {sum(b['volume'] for b in dark) / 1000:.1f} cm3")
    for view, name in (("iso", "lid_iso"), ("top", "lid_top")):
        mcp.call("screenshot", {"tab": 1, "view": view, "width": 1000, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    # The lid at 1:1: the whole tiles around the trim rectangle, to lay
    # the glue-up out on, and every piece outline as a DXF.
    for sheet in SHEETS:
        try:
            print(mcp.call("export", {"tab": 1, "format": "pdf", "views": ["top"], "scale": 1.0, "sheet": sheet, "parts": False,
                                      "note": f"Spectre lid: the {SIDE:.1f} mm square is trimmed from {tiles} whole tiles after the glue-up; edge {EDGE} mm, gap {GAP} mm",
                                      "path": os.path.join(OUT, "lid.pdf")}))
            break
        except RuntimeError as e:
            print(f"{sheet}: {e}")
    else:
        raise RuntimeError("the lid fits no sheet at 1:1")
    print(mcp.call("export", {"tab": 1, "format": "dxf", "view": "top", "path": os.path.join(OUT, "templates.dxf")}))
    # The five prism profiles at 1:1: the sticks' sections with half the
    # gap off their outside faces, the glue faces full.
    dxf(os.path.join(OUT, "prisms.dxf"), [(f"prism-{p['index']}-shape-{p['shape']}", poly) for p in prisms["prisms"] for poly in p["profile"]])
    # The sample piece: one whole tile as the pieces come off the stick,
    # eroded by half the gap, to print and try against its neighbours.
    top.apply({"type": "set_spectre", "id": feature, "show": "sample"})
    sample = top.report()["bodies"]
    assert [b["name"] for b in sample] == ["Sample piece"], sample
    print(f"sample piece: {sample[0]['volume'] / THICKNESS:.0f} mm2 of the tile's {prisms['tile_area']:.0f}")
    mcp.call("screenshot", {"tab": 1, "view": "iso", "width": 800, "height": 600, "path": os.path.join(OUT, "sample_piece.png")})
    print(mcp.call("export", {"tab": 1, "format": "stl", "body": "Sample piece", "path": os.path.join(OUT, "sample_piece.stl")}))
    top.apply({"type": "set_spectre", "id": feature, "show": "design"})
    assert len(pieces_of(top.report())) == len(pieces)
    with open(os.path.join(OUT, "lid.json"), "w") as f:
        json.dump(
            {
                "side": SIDE,
                "edge": EDGE,
                "bulge": BULGE,
                "gap": GAP,
                "shift": list(best[0]),
                "cuts": cuts,
                "prisms": per_tile,
                "shapes": len(prisms["shapes"]),
                "theta": prisms["theta"],
                "radius": prisms["radius"],
                "tiles": tiles,
                "pieces": len(pieces),
                "light": len(light),
                "dark": len(dark),
                "sample_area": sample[0]["volume"] / THICKNESS,
                "tile_area": prisms["tile_area"],
            },
            f,
            indent=1,
        )
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
