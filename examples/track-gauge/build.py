"""A track saw edge gauge: a fixed marking gauge that sets the track's
edge 2 in plus the blade's kerf from the edge of a sheet, so the cut
leaves a 2 in strip. Hook the fence over the sheet edge, slide the
track up to the gauge's face, lift the gauge off, cut. Two of them, one
at each end of the track, square the track to the edge as well. One
printed piece, an L in section extruded along the edge: the short leg
is the fence, hooking down over the sheet edge, the long leg is the
depth, lying on the sheet out to the track.

    cargo build --release -p ok-mcp
    python3 examples/track-gauge/build.py       # the document, the STL, pictures and sheets
    cargo test -p ok-render --test track_gauge  # what CI runs

The strip is in inches, the print in millimetres, the document in
millimetres. Frame: x across from the sheet edge (the fence's inner
face is x = 0), y along the edge (the L's middle is y = 0), z up from
the sheet's top face (the depth leg's underside is z = 0).
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4

WIDTH = 2.0                 # the strip the cut leaves between the sheet edge and the kerf, inches
KERF = 2.2 / IN             # the blade: 2.2 mm on a Festool or Makita track saw; 1.5 or 1.6 on their thin-kerf blades
# The print, in millimetres: 6 mm walls print stiff in three or four
# perimeters, and the L is long enough along the edge to sit square.
T = 6.0 / IN                # both legs' thickness; the track's edge is 5 tall against the depth leg's end
LEN = 60.0 / IN             # along the sheet edge
DROP = 12.0 / IN            # the fence hangs this far below the depth leg, over the sheet's edge
GROOVE = 2.0 / IN           # dust groove along the inside corner, so a crumb there cannot hold the fence off the edge
ROUND = 4.0 / IN            # the four corners in plan
ENGRAVE = 0.6 / IN          # the labels, cut into the top face: three layers at 0.2
# Seven-segment lettering for the labels, in millimetres on a cell 6
# wide and 10 tall with a 1.2 stroke; each segment is a bar (u0, v0,
# u1, v1), u along the line and v up, with 0.3 between bars so they
# never overlap and the sketch's regions stay simple.
SEG = {"a": (0.0, 8.8, 6.0, 10.0), "b": (4.8, 5.9, 6.0, 8.5), "c": (4.8, 1.5, 6.0, 4.1), "d": (0.0, 0.0, 6.0, 1.2),
       "e": (0.0, 1.5, 1.2, 4.1), "f": (0.0, 5.9, 1.2, 8.5), "g": (0.0, 4.4, 6.0, 5.6)}
GLYPHS = {"2": (6.0, [SEG[k] for k in "abged"]),
          ".": (1.2, [(0.0, 0.0, 1.2, 1.2)]),
          '"': (3.5, [(0.0, 7.0, 1.2, 10.0), (2.3, 7.0, 3.5, 10.0)]),
          "m": (8.0, [(0.0, 0.0, 1.2, 5.3), (3.4, 0.0, 4.6, 5.3), (6.8, 0.0, 8.0, 5.3), (0.0, 5.6, 8.0, 6.8)])}
LABELS = (('2"', 11.0), ("2.2mm", 28.0))   # each line's bottom, mm out from the fence's inner face
# The picture of it in use: a corner of a 3/4 sheet, a track and the kerf.
SHEET, SHEET_T = 24.0, 0.75
TRACK_W, TRACK_T, TRACK_Y = 7.25, 0.2, (-3.0, 27.0)   # a Festool-pattern track: 184 wide, 5 thick at its edge
GAUGE_Y = (3.0, 21.0)       # the L's middles: one gauge at each end of the cut
MATERIALS = {"petg": ("PETG", 1.27), "ply": ("3/4 plywood", 0.6), "aluminium": ("aluminium", 2.7)}


def mm(*v):
    return tuple(c * IN for c in v)


def face(report, normal, x=None):
    """The reference of the body's planar face with this normal (and
    centroid x, when two share the normal), from a full report."""
    for fc in report["bodies"][0]["faces"]:
        n = fc["normal"]
        if all(abs(n[k] - v) < 1e-6 for k, v in zip("xyz", normal)) and (x is None or abs(fc["centroid"]["x"] - x * IN) < 1e-6):
            return fc["reference"]
    raise KeyError(f"no face with normal {normal}")


def label_bars(text, x0):
    """The bars of a line of lettering as (x, y) corner pairs, inches:
    the line centred on y and reading along -y (left to right for
    someone standing at the sheet edge), the glyphs' up along +x."""
    width = sum(GLYPHS[c][0] for c in text) + 2.0 * (len(text) - 1)
    u, bars = -width / 2, []
    for c in text:
        w, segs = GLYPHS[c]
        for u0, v0, u1, v1 in segs:
            bars.append(((x0 + v0) / IN, -(u + u1) / IN, (x0 + v1) / IN, -(u + u0) / IN))
        u += w + 2.0
    return bars


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, density = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": density}})
    p.apply(*ops)


def gauge(p):
    """One body: the fence as the L's short leg, the depth leg added to
    it out to WIDTH + KERF with its length bound to the variables, and
    the dust groove cut along the inside corner."""
    p.apply({"type": "add_variable", "name": "width", "expression": f"{WIDTH * IN:g}"},
            {"type": "add_variable", "name": "kerf", "expression": f"{KERF * IN:g}"})
    s = p.sketch("right", -T * IN, "fence")
    p.rect(s, mm(-LEN / 2, -DROP), mm(LEN / 2, T))
    f = p.extrude(s, T * IN, op="new", name="fence, the short leg")
    s = p.sketch("right", 0.0, "depth")
    p.rect(s, mm(-LEN / 2, 0.0), mm(LEN / 2, T))
    g = p.extrude(s, (WIDTH + KERF) * IN, op="add", name="depth, 2 in plus the kerf")
    p.apply({"type": "set_binding", "id": g, "field": "depth", "expression": "#width + #kerf"})
    s = p.sketch("top", 0.0, "dust groove")
    p.rect(s, mm(0.0, -LEN), mm(GROOVE, LEN))
    p.cut(s, GROOVE * IN, direction="normal", name="dust groove, 2 mm square")
    # The four corners in plan, from the faces the report lists.
    r = p.report()
    edges = [{"a": face(r, (0.0, sy, 0.0)), "b": face(r, (nx, 0.0, 0.0), x)} for sy in (1.0, -1.0) for nx, x in ((-1.0, -T), (1.0, WIDTH + KERF))]
    p.apply({"type": "add_blend", "kind": "fillet", "edges": edges, "size": ROUND * IN, "name": f"corners, R{ROUND * IN:g}"})
    # The labels, engraved in the top face: the strip's width and the
    # kerf it allows for.
    s = p.sketch("top", T * IN, "labels")
    for text, x0 in LABELS:
        for x0_, y0, x1, y1 in label_bars(text, x0):
            p.rect(s, mm(x0_, y0), mm(x1, y1))
    p.cut(s, ENGRAVE * IN, direction="reverse", name="labels, engraved 0.6")
    part_is(p, f, f"edge gauge, {WIDTH:g} in strip, {KERF * IN:g} mm kerf", "petg")


def sheet(p):
    s = p.sketch("top", -SHEET_T * IN, "sheet")
    p.rect(s, mm(0.0, 0.0), mm(SHEET, SHEET))
    f = p.extrude(s, SHEET_T * IN, op="new", name="a corner of the sheet")
    part_is(p, f, "sheet, the corner being cut", "ply")


def track(p):
    """The track with its edge against the gauge's face, and the kerf
    the blade leaves just outside that edge: from WIDTH to WIDTH +
    KERF, so the strip between the sheet edge and the kerf is WIDTH."""
    x0 = WIDTH + KERF
    s = p.sketch("top", 0.0, "track")
    p.rect(s, mm(x0, TRACK_Y[0]), mm(x0 + TRACK_W, TRACK_Y[1]))
    f = p.extrude(s, TRACK_T * IN, op="new", name="track")
    part_is(p, f, "track, its splinter guard edge on the gauge", "aluminium")
    s = p.sketch("top", -SHEET_T * IN, "kerf")
    p.rect(s, mm(WIDTH, TRACK_Y[0]), mm(x0, TRACK_Y[1]))
    f = p.extrude(s, SHEET_T * IN, op="new", name="kerf")
    part_is(p, f, f"kerf, {KERF * IN:g} mm, {WIDTH:g} in from the edge")


def instance(studio, body, name, y=0.0, z=0.0, rx=0.0):
    return {"type": "add_instance", "studio": studio, "body": body, "name": name, "fixed": True,
            "placement": {"position": {"x": 0.0, "y": y * IN, "z": z * IN}, "rotation": {"x": rx, "y": 0.0, "z": 0.0}}}


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "track_gauge.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "track_gauge"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Track saw edge gauge, 2 in"},
                               {"type": "rename_tab", "tab": 1, "name": "gauge"},
                               {"type": "add_part_studio", "name": "sheet"},
                               {"type": "add_part_studio", "name": "track"},
                               {"type": "add_assembly", "name": "in use"},
                               {"type": "add_assembly", "name": "print"}]})
    for tab, build in ((1, gauge), (2, sheet), (3, track)):
        p = Part(mcp, tab, str(tab))
        build(p)
        for b in p.report()["bodies"]:
            lo, hi = b["bounds"]
            print(f"tab {tab}: {b['name']}: x {lo['x'] / IN:.3f}..{hi['x'] / IN:.3f}, y {lo['y'] / IN:.1f}..{hi['y'] / IN:.1f}, z {lo['z'] / IN:.3f}..{hi['z'] / IN:.3f} in, {b['volume'] / IN ** 3:.3f} in3")
    ops = [instance(2, 0, "sheet"), instance(3, 0, "track"), instance(3, 1, "kerf")]
    ops += [instance(1, 0, f"gauge {n + 1}", y) for n, y in enumerate(GAUGE_Y)]
    mcp.call("apply", {"ops": ops, "tab": 4})
    print(f"in use: {len(ops)} instances")
    # The print: the gauge turned over onto its flat top face, that
    # face on the bed at z = 0 and the fence standing up, so the STL
    # drops into a slicer the right way up.
    mcp.call("apply", {"ops": [instance(1, 0, "gauge, top face down", z=T, rx=180.0)], "tab": 5})
    print(mcp.call("export", {"tab": 5, "format": "stl", "path": os.path.join(OUT, "gauge.stl")}))
    mcp.call("screenshot", {"tab": 5, "view": "-0.7,-0.6,0.45", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge_print.png")})
    mcp.call("screenshot", {"tab": 1, "view": "-0.7,-0.6,0.45", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge.png")})
    mcp.call("screenshot", {"tab": 1, "view": "0.5,-0.7,-0.5", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge_below.png")})
    mcp.call("screenshot", {"tab": 1, "view": "top", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge_top.png")})
    mcp.call("screenshot", {"tab": 4, "view": "-0.55,-0.7,0.45", "width": 1600, "height": 1000, "path": os.path.join(OUT, "in_use.png")})
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "views": ["front", "top", "right", "iso"],
                              "note": f"printed, one piece, an L extruded {LEN * IN:g} along the edge; the depth leg {WIDTH:g} in + {KERF * IN:g} mm kerf = {(WIDTH + KERF) * IN:.1f} mm from the fence's inner face; 6 mm legs, the fence {DROP * IN:g} below; 2 mm dust groove; corners R{ROUND * IN:g}; labels engraved {ENGRAVE * IN:g}",
                              "path": os.path.join(OUT, "gauge.pdf")}))
    print(mcp.call("export", {"tab": 4, "format": "pdf", "sheet": "A4", "views": ["iso", f"section@{(GAUGE_Y[0] + 2.0) * IN:g}"],
                              "window": ",".join(f"{v * IN:g}" for v in (-1.5, -0.5, -1.0, WIDTH + KERF + 3.0, 6.0, 1.5)),
                              "note": f"hooked on the sheet edge, the track against its face, the kerf {WIDTH:g} in from the edge; section through a gauge",
                              "path": os.path.join(OUT, "in_use.pdf")}))


if __name__ == "__main__":
    main()
