"""A track saw edge gauge: a fixed marking gauge that sets the track's
edge 2 in plus the blade's kerf from the edge of a sheet, so the cut
leaves a 2 in strip. Hook the fence over the sheet edge, slide the
track up to the gauge's face, lift the gauge off, cut. Two of them, one
at each end of the track, square the track to the edge as well.

    cargo build --release -p ok-mcp
    python3 examples/track-gauge/build.py       # the document, pictures and sheets
    cargo test -p ok-render --test track_gauge  # what CI runs

Inches in the shop, millimetres in the document. Frame: x across from
the sheet edge (the fence's inner face is x = 0), y along the edge, z
up from the sheet's top face (the base's underside is z = 0).
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4

WIDTH = 2.0                 # the strip the cut leaves between the sheet edge and the kerf
KERF = 2.2 / IN             # the blade: 2.2 mm on a Festool or Makita track saw; 1.5 or 1.6 on their thin-kerf blades
T = 0.75                    # the stock: 3/4 hardwood, base and fence alike
LEN = 4.0                   # along the edge
DROP = 0.75                 # the fence hangs this far below the base, over the sheet's edge
GROOVE = 0.125              # dust groove along the base's inside bottom edge, so a crumb in the corner cannot hold it off
SCREW_D, PILOT_D, CSK_D = 0.17, 0.109, 0.35   # #8 x 1-1/2 wood screws: clearance, 7/64 pilot, countersink
SCREW_Y, SCREW_Z = (1.0, 3.0), T / 2          # two, into the middle of the base's edge
# The picture of it in use: a corner of a 3/4 sheet, a track and the kerf.
SHEET, SHEET_T = 24.0, 0.75
TRACK_W, TRACK_T, TRACK_Y = 7.25, 0.2, (-3.0, 27.0)   # a Festool-pattern track: 184 wide, 5 thick at its edge
GAUGE_Y = (1.0, 19.0)       # one gauge at each end of the cut
MATERIALS = {"maple": ("maple", 0.7), "ply": ("3/4 plywood", 0.6), "aluminium": ("aluminium", 2.7)}


def mm(*v):
    return tuple(c * IN for c in v)


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, density = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": density}})
    p.apply(*ops)


def gauge(p):
    """The fence first, with its countersunk screw holes cut while it
    is the only body; then the base, its width bound to the variables,
    the dust groove and the pilot holes."""
    p.apply({"type": "add_variable", "name": "width", "expression": f"{WIDTH * IN:g}"},
            {"type": "add_variable", "name": "kerf", "expression": f"{KERF * IN:g}"})
    s = p.sketch("right", -T * IN, "fence")
    p.rect(s, mm(0.0, -DROP), mm(LEN, T))
    f = p.extrude(s, T * IN, op="new", name="fence, 3/4 x 1-1/2")
    part_is(p, f, f"fence 3/4 x {DROP + T:g} x {LEN:g}", "maple")
    s = p.sketch("right", -T * IN, "screw holes")
    for y in SCREW_Y:
        p.point(s, mm(y, SCREW_Z))
    p.hole(s, SCREW_D * IN, depth=(T + 0.25) * IN, direction="normal", countersink={"diameter": CSK_D * IN, "angle": 82.0}, name="screw holes, #8 countersunk")
    s = p.sketch("right", 0.0, "base")
    p.rect(s, mm(0.0, 0.0), mm(LEN, T))
    f = p.extrude(s, (WIDTH + KERF) * IN, op="new", name="base, 2 in plus the kerf wide")
    p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#width + #kerf"})
    part_is(p, f, f"base 3/4 x {WIDTH + KERF:.3f} x {LEN:g}", "maple")
    s = p.sketch("top", 0.0, "dust groove")
    p.rect(s, mm(0.0, -1.0), mm(GROOVE, LEN + 1.0))
    p.cut(s, GROOVE * IN, direction="normal", name="dust groove, 1/8 square")
    s = p.sketch("right", 0.0, "pilot holes")
    for y in SCREW_Y:
        p.point(s, mm(y, SCREW_Z))
    p.hole(s, PILOT_D * IN, depth=T * IN, direction="normal", name="pilot holes, 7/64")


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


def instance(studio, body, name, y=0.0):
    return {"type": "add_instance", "studio": studio, "body": body, "name": name, "fixed": True,
            "placement": {"position": {"x": 0.0, "y": y * IN, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}}


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
                               {"type": "add_assembly", "name": "in use"}]})
    for tab, build in ((1, gauge), (2, sheet), (3, track)):
        p = Part(mcp, tab, str(tab))
        build(p)
        for b in p.report()["bodies"]:
            lo, hi = b["bounds"]
            print(f"tab {tab}: {b['name']}: x {lo['x'] / IN:.3f}..{hi['x'] / IN:.3f}, y {lo['y'] / IN:.1f}..{hi['y'] / IN:.1f}, z {lo['z'] / IN:.3f}..{hi['z'] / IN:.3f} in, {b['volume'] / IN ** 3:.3f} in3")
    ops = [instance(2, 0, "sheet"), instance(3, 0, "track"), instance(3, 1, "kerf")]
    for n, y in enumerate(GAUGE_Y):
        ops += [instance(1, 0, f"gauge {n + 1} / fence", y), instance(1, 1, f"gauge {n + 1} / base", y)]
    mcp.call("apply", {"ops": ops, "tab": 4})
    print(f"in use: {len(ops)} instances")
    mcp.call("screenshot", {"tab": 1, "view": "-0.7,-0.6,0.45", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge.png")})
    mcp.call("screenshot", {"tab": 1, "view": "0.5,-0.7,-0.5", "width": 1400, "height": 900, "path": os.path.join(OUT, "gauge_below.png")})
    mcp.call("screenshot", {"tab": 4, "view": "-0.55,-0.7,0.45", "width": 1600, "height": 1000, "path": os.path.join(OUT, "in_use.png")})
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "views": ["front", "top", "right", "iso"],
                              "note": f"3/4 maple; the base {WIDTH:g} in + {KERF * IN:g} mm kerf = {WIDTH + KERF:.3f} wide, the fence 1-1/2 tall; 1/8 dust groove; two #8 x 1-1/2 countersunk",
                              "path": os.path.join(OUT, "gauge.pdf")}))
    print(mcp.call("export", {"tab": 4, "format": "pdf", "sheet": "A4", "views": ["iso", f"section@{(GAUGE_Y[0] + 2.0) * IN:g}"],
                              "window": ",".join(f"{v * IN:g}" for v in (-1.5, -0.5, -1.0, WIDTH + KERF + 3.0, 6.0, 1.5)),
                              "note": f"hooked on the sheet edge, the track against its face, the kerf {WIDTH:g} in from the edge; section through a gauge",
                              "path": os.path.join(OUT, "in_use.pdf")}))


if __name__ == "__main__":
    main()
