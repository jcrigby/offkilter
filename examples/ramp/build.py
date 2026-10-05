"""A folding torsion-box truck ramp: two identical 21 x 48 in panels of
15/32 in ply skins on 2x2 ribs, hinged on a 3/4 in pipe below the
decks that doubles as the carrying handle and pulls out to separate the
halves. The brief is README.md in this directory; this script is the
document it asked for.

    cargo build --release -p ok-mcp
    python3 examples/ramp/build.py            # the document, pictures, sheets and cut list
    cargo test -p ok-render --test ramp       # what CI runs

The shop works in inches, the document in millimetres like the other
examples: every dimension is a named constant in inches and the helpers
convert. Frame of one panel: x across the width (0 to 21), y along the
length (0 at the hinge end, 48 at the free end), z up from the bottom
face of the bottom skin. The second panel is the same sub-assembly
turned 180 degrees about z and moved 21 in x, so its flush rail lands
beside the first's spaced rail and its lugs between the first's. Each
panel is built in place in its own frame; the pipe, caps, end plates
and ground angle are built in the ramp's frame.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4

# ---------------------------------------------------------------------
# The panel, in inches.
# ---------------------------------------------------------------------
W, L = 21.0, 48.0                  # a panel, over the box
SKIN = 0.4375                      # 15/32 CDX
RIB = 1.5                          # 2x2 actual, on edge: the box is SKIN + RIB + SKIN
BOX = 2 * SKIN + RIB               # 2.375
SKIN_END = 45.0                    # the skins stop here; the stubs run bare to 48
CURB = 1.0                         # the rails stand this much above the deck
PIN_DROP = 1.0                     # pipe centre below the bottom skin: lever arm against hang
PIPE_OD, PIPE_ID, PIPE_L = 1.05, 0.824, 26.0   # 3/4 Sch 40
BORE = 1.125                       # the lug bores
BIRCH = 0.75                       # the rails, cheeks, spacer and lug plies
RAIL_H = BOX + CURB                # 3.375 along the length
LUG_R = 1.0                        # the half-round about the pin, so the rail bottom reaches -2
TAPER = 6.0                        # the rail bottom climbs back to z = 0 over this
CHAMFER = 0.375                    # the bottom skin's hinge-end edge, which sweeps a 1 in radius about the pin
RIBS = (1.5, 4.5, 10.5, 15.0, 19.5)   # rib centres; 4.5 and 15 are the lug lines
LUG_A, LUG_B = 4.5, 15.0           # interior lug centres: a + b = W - 1.5 so the turned panel's nest beside them
LUG_T, LUG_IN = 1.5, 6.0           # two plies thick; the part inside the box, behind the joint block
NOTCH_L = 3.0                      # the bottom skin's notch, where the lug passes through
CROSS_Y = (16.0, 32.0)             # cross block rows
JOINT = 3.5                        # the 2x4 flat across the hinge end, between the skins
STUB_W, STUB_L, STUB_Y0 = 7.25, 12.0, 36.0   # 2x8 flat at the free end, 12 long from y 36, bare past 45
BEVEL = 30.0                       # degrees, on the ground end
PLATE_L = 2.0                      # the end plates' ghost
ANGLE_T, ANGLE_LEG = 0.125, 1.0    # the aluminium angle over the ground edge
CAP_OD, CAP_L = 1.3, 1.0

# The turned panel's placement in the ramp: 180 degrees about z, moved
# W in x: its x runs 21 to 0, its y 0 to -48.
PANEL2 = ((W * IN, 0.0, 0.0), (0.0, 0.0, 180.0))


def mm(*v):
    return tuple(c * IN for c in v)


def box(p, name, x, y, z, op="new"):
    """A block, inches."""
    s = p.sketch("top", z[0] * IN, name)
    p.rect(s, mm(x[0], y[0]), mm(x[1], y[1]))
    return p.extrude(s, (z[1] - z[0]) * IN, op=op, name=name)


def profile_x(p, name, x0, x1, pts, circle=None, op="new"):
    """A polygon in the y-z plane (inches) extruded from x0 to x1, with
    a circle unioned onto it for a half-round end."""
    s = p.sketch("right", x0 * IN, name)
    p.polygon(s, [mm(*pt) for pt in pts])
    f = p.extrude(s, (x1 - x0) * IN, op=op, name=name)
    if circle:
        (cy, cz), r = circle
        s = p.sketch("right", x0 * IN, f"{name} round")
        p.circle(s, mm(cy, cz), r * IN)
        p.extrude(s, (x1 - x0) * IN, op="add", name=f"{name} round")
    return f


def bore_x(p, name, x0, x1, cy, cz, d):
    """A hole along x through everything on the line, inches."""
    s = p.sketch("right", (x0 - 0.5) * IN, name)
    p.point(s, mm(cy, cz))
    p.hole(s, d * IN, depth=(x1 - x0 + 1.0) * IN, direction="normal", name=name)


# -- panel parts ------------------------------------------------------

def skin_top(p):
    f = box(p, "top skin, 15/32 CDX", (0.0, W), (0.0, SKIN_END), (BOX - SKIN, BOX))
    p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#skin"})


def skin_bottom(p):
    """The bottom skin, its hinge-end bottom edge chamfered since it
    sweeps a 1 in radius about the pin, and notched for the lugs."""
    profile_x(p, "bottom skin, 15/32 CDX", 0.0, W, [(CHAMFER, 0.0), (SKIN_END, 0.0), (SKIN_END, SKIN), (0.0, SKIN), (0.0, CHAMFER)])
    s = p.sketch("top", SKIN * IN + 1.0, "lug notches")
    for a in (LUG_A, LUG_B):
        p.rect(s, mm(a - LUG_T / 2, JOINT), mm(a + LUG_T / 2, JOINT + NOTCH_L))
    p.cut(s, SKIN * IN + 2.0, direction="reverse", name="lug notches")


def rib_span(c):
    """Where a rib runs: behind the joint block, or behind its lug, to
    the free end, or to the stubs on the outer lines."""
    y0 = JOINT + LUG_IN if c in (LUG_A, LUG_B) else JOINT
    y1 = STUB_Y0 if (c < STUB_W or c > W - STUB_W) else SKIN_END
    return y0, y1


def ribs(p):
    for i, c in enumerate(RIBS):
        y0, y1 = rib_span(c)
        f = box(p, f"rib, 2x2 x {y1 - y0:g}", (c - RIB / 2, c + RIB / 2), (y0, y1), (SKIN, SKIN + RIB), op="new")
        if i == 0:
            p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#rib"})


def cross_blocks(p):
    for y in CROSS_Y:
        for a, b in zip(RIBS, RIBS[1:]):
            box(p, f"cross block, 2x2 x {b - a - RIB:g}", (a + RIB / 2, b - RIB / 2), (y - RIB / 2, y + RIB / 2), (SKIN, SKIN + RIB), op="new")


def joint_block(p):
    box(p, "joint block, 2x4 flat x 21", (0.0, W), (0.0, JOINT), (SKIN, SKIN + RIB))


def stubs(p):
    """2x8 flat at the free end's outer positions, 3 in bare past the
    skins for the end plates or the ground bevel. The bevel is a
    suppressed feature: on in the stub's own sheet, off in the ramp,
    whose panels are identical and only one of them meets the ground."""
    for x0 in (0.0, W - STUB_W):
        box(p, "tailgate stub, 2x8 flat x 12", (x0, x0 + STUB_W), (STUB_Y0, L), (SKIN, SKIN + RIB), op="new")
    back = RIB * math.tan(math.radians(BEVEL))
    s = p.sketch("right", -1.0 * IN, "bevel")
    p.polygon(s, [mm(L - back, SKIN + RIB), mm(L + 0.1, SKIN + RIB), mm(L + 0.1, SKIN)])
    f = p.cut(s, (W + 2.0) * IN, direction="normal", name=f"ground bevel, {BEVEL:g} degrees")
    p.apply({"type": "set_suppressed", "id": f, "suppressed": True})


def rail_profile():
    return [(0.0, RAIL_H), (SKIN_END, RAIL_H), (SKIN_END, 0.0), (TAPER, 0.0), (0.0, -2 * LUG_R)]


def rail_flush(p):
    """The rail glued flush to the box side at x = 0, 3/4 birch on edge,
    a curb above the deck, a half-round about the pin below it."""
    profile_x(p, "flush rail, 3/4 birch", -BIRCH, 0.0, rail_profile(), circle=((0.0, -PIN_DROP), LUG_R))
    bore_x(p, "pipe bore", -BIRCH, 0.0, 0.0, -PIN_DROP, BORE)


def rail_spaced(p):
    """The rail on the other side, on a 3/4 spacer, so the turned panel's
    flush rail lands beside it on the pipe."""
    profile_x(p, "spaced rail, 3/4 birch", W + BIRCH, W + 2 * BIRCH, rail_profile(), circle=((0.0, -PIN_DROP), LUG_R))
    bore_x(p, "pipe bore", W + BIRCH, W + 2 * BIRCH, 0.0, -PIN_DROP, BORE)


def cheek_profile():
    return [(0.0, 0.0), (TAPER, 0.0), (0.0, -2 * LUG_R)]


def cheek_flush(p):
    """A second ply under the box inside the flush rail, so the pipe
    bears on 1-1/2 in of birch."""
    profile_x(p, "lug cheek, 3/4 birch", 0.0, BIRCH, cheek_profile(), circle=((0.0, -PIN_DROP), LUG_R))
    bore_x(p, "pipe bore", 0.0, BIRCH, 0.0, -PIN_DROP, BORE)


def cheek_spaced(p):
    """The spaced rail's cheek goes outside: inside is where the turned
    panel's flush rail sits."""
    profile_x(p, "lug cheek, 3/4 birch", W + 2 * BIRCH, W + 3 * BIRCH, cheek_profile(), circle=((0.0, -PIN_DROP), LUG_R))
    bore_x(p, "pipe bore", W + 2 * BIRCH, W + 3 * BIRCH, 0.0, -PIN_DROP, BORE)


def rail_spacer(p):
    box(p, "rail spacer, 3/4 birch", (W, W + BIRCH), (0.0, SKIN_END), (0.0, BOX))


def interior_lug(a):
    """A lug in the rib line at x = a: two plies of birch, the part
    inside the box behind the joint block and in line with its rib,
    down through the bottom skin's notch, and the half-round about the
    pin under the joint block, tapering back up to the skin."""
    def build(p):
        x0, x1 = a - LUG_T / 2, a + LUG_T / 2
        profile_x(p, "interior lug, two plies birch", x0, x1,
                  [(0.0, 0.0), (0.0, -2 * LUG_R), (JOINT, -2 * LUG_R), (JOINT + NOTCH_L, 0.0)], circle=((0.0, -PIN_DROP), LUG_R))
        box(p, "through the notch", (x0, x1), (JOINT, JOINT + NOTCH_L), (0.0, SKIN), op="add")
        box(p, "inside the box", (x0, x1), (JOINT, JOINT + LUG_IN), (SKIN, SKIN + RIB), op="add")
        bore_x(p, "pipe bore", x0, x1, 0.0, -PIN_DROP, BORE)
    return build


# -- ramp parts, in the ramp's frame ----------------------------------

PIPE_X = (-3 * BIRCH - 0.25, W + 3 * BIRCH + 0.25)   # -2.5 to 23.5: 26 in, a quarter past each outer cheek for the caps


def pipe(p):
    s = p.sketch("right", PIPE_X[0] * IN, "pipe")
    p.circle(s, mm(0.0, -PIN_DROP), PIPE_OD / 2 * IN)
    p.extrude(s, (PIPE_X[1] - PIPE_X[0]) * IN, name="hinge pipe, 3/4 Sch 40 x 26")
    bore_x(p, "bore", PIPE_X[0], PIPE_X[1], 0.0, -PIN_DROP, PIPE_ID)


def pipe_caps(p):
    for i, (x0, x1) in enumerate(((PIPE_X[0] - CAP_L, PIPE_X[0]), (PIPE_X[1], PIPE_X[1] + CAP_L))):
        s = p.sketch("right", x0 * IN, "cap")
        p.circle(s, mm(0.0, -PIN_DROP), CAP_OD / 2 * IN)
        p.extrude(s, (x1 - x0) * IN, op="new", name="pipe cap, 3/4")


def end_plates(p):
    """Ghosts of the ramp end plates on the first panel's bare stubs: a
    block with the 1-1/2 x 7-1/4 pocket, 2 in long, no casting."""
    for x0 in (0.0, W - STUB_W):
        box(p, "end plate (ghost)", (x0 - 0.25, x0 + STUB_W + 0.25), (L - PLATE_L, L), (0.0, BOX), op="new")
    for x0 in (0.0, W - STUB_W):
        s = p.sketch("front", -(L + 0.5) * IN, "pocket")
        p.rect(s, mm(x0, SKIN), mm(x0 + STUB_W, SKIN + RIB))
        p.cut(s, (PLATE_L + 1.0) * IN, direction="normal", name="board pocket")


def ground_angle(p):
    """1/8 x 1 aluminium angle over the turned panel's ground edge, at
    the ramp's y = -48: a leg on the stubs' top and a leg down the end."""
    y = -L
    box(p, "ground angle, 1/8 x 1 aluminium x 21", (0.0, W), (y, y + ANGLE_LEG), (SKIN + RIB, SKIN + RIB + ANGLE_T))
    box(p, "leg", (0.0, W), (y - ANGLE_T, y), (SKIN + RIB + ANGLE_T - ANGLE_LEG, SKIN + RIB + ANGLE_T), op="add")


PANEL_PARTS = [
    ("skin_top", skin_top), ("skin_bottom", skin_bottom), ("rib", ribs), ("cross_block", cross_blocks), ("joint_block", joint_block),
    ("stub", stubs), ("rail_flush", rail_flush), ("rail_spaced", rail_spaced), ("lug_cheek_flush", cheek_flush), ("lug_cheek_spaced", cheek_spaced),
    ("rail_spacer", rail_spacer), ("interior_lug_a", interior_lug(LUG_A)), ("interior_lug_b", interior_lug(LUG_B)),
]
RAMP_PARTS = [("pipe", pipe), ("pipe_cap", pipe_caps), ("end_plate", end_plates), ("ground_angle", ground_angle)]

# The cut list: what to cut, in inches, per the shop's stock.
CUTLIST = [
    ("top skin", f"{W:g} x {SKIN_END:g} x 15/32 CDX", 2, "one 4x8 sheet: two 21 in rips, each crosscut at 45 and 45"),
    ("bottom skin", f"{W:g} x {SKIN_END:g} x 15/32 CDX, two 1-1/2 x 3 notches", 2, "from the same sheet"),
    ("rib", "2x2 x 32.5", 4, "outer lines, joint block to stubs"),
    ("rib", "2x2 x 26.5", 4, "lug lines, lug to stubs"),
    ("rib", "2x2 x 41.5", 2, "middle line, joint block to the skins' end"),
    ("cross block", "2x2 x 1.5", 4, "between the outer and lug lines"),
    ("cross block", "2x2 x 4.5", 4, "between the lug and middle lines"),
    ("cross block", "2x2 x 3", 4, "between the middle and lug lines"),
    ("cross block", "2x2 x 3", 4, "between the lug and outer lines"),
    ("joint block", "2x4 flat x 21", 2, ""),
    ("stub", "2x8 x 12", 4, "3 in bare past the skins"),
    ("rail", f"3/4 birch, {SKIN_END:g} x {RAIL_H:g}, lug profile", 4, "two flush, two on spacers"),
    ("lug cheek", "3/4 birch, 6 x 2, lug profile", 4, "inside the flush rail, outside the spaced one"),
    ("rail spacer", f"3/4 birch, {SKIN_END:g} x {BOX:g}", 2, ""),
    ("interior lug", "3/4 birch, 9.5 x 4, two laminated", 8, "4 lugs of 2 plies"),
    ("hinge pipe", f"3/4 Sch 40 galvanized, {PIPE_L:g}, threaded both ends", 1, "with two caps"),
    ("ground angle", "1/8 x 1 aluminium angle x 21", 1, "over the bevel"),
]


def _v(d):
    return (d["x"], d["y"], d["z"])


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _norm(a):
    k = 1.0 / math.sqrt(_dot(a, a))
    return (a[0] * k, a[1] * k, a[2] * k)


def _rotation(rx, ry, rz):
    """A placement's rotation matrix: x, then y, then z (degrees)."""
    a, b, c = (math.radians(t) for t in (rx, ry, rz))
    ca, sa, cb, sb, cc, sc = math.cos(a), math.sin(a), math.cos(b), math.sin(b), math.cos(c), math.sin(c)
    return ((cc * cb, cc * sb * sa - sc * ca, cc * sb * ca + sc * sa), (sc * cb, sc * sb * sa + cc * ca, sc * sb * ca - cc * sa), (-sb, cb * sa, cb * ca))


def _apply(m, p):
    return tuple(_dot(row, p) for row in m)


def _frame(cyl, face):
    """A cylindrical face's connector frame as the kernel builds it (see
    docs/OPS.md): origin at the face's middle on the axis, z along the
    axis, x and y canonical for z."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    t = _dot(_sub(c, o), z)
    origin = (o[0] + z[0] * t, o[1] + z[1] * t, o[2] + z[2] * t)
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    return origin, _cross(y, z), y, z


def _placed(frame, placement):
    """The frame in the world once its body is placed."""
    (x, y, z), (rx, ry, rz) = placement
    m = _rotation(rx, ry, rz)
    o, fx, fy, fz = frame
    po = _apply(m, o)
    return (po[0] + x, po[1] + y, po[2] + z), _apply(m, fx), _apply(m, fy), _apply(m, fz)


def cylinder_frames(body, r, placement):
    """Every cylinder of radius r on a studio body, each with its placed
    frame."""
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    out = []
    for c in body["cylinders"]:
        if abs(c["radius"] - r) < 1e-6:
            out.append((c, _placed(_frame(c, faces[json.dumps(c["reference"], sort_keys=True)]), placement)))
    return out


def coaxial(a_body, ra, a_placement, b_body, rb, b_placement):
    for ca, ta in cylinder_frames(a_body, ra, a_placement):
        for cb, tb in cylinder_frames(b_body, rb, b_placement):
            d = _sub(tb[0], ta[0])
            off = _sub(d, tuple(ta[3][i] * _dot(d, ta[3]) for i in range(3)))
            if math.sqrt(_dot(_cross(ta[3], tb[3]), _cross(ta[3], tb[3]))) < 1e-9 and math.sqrt(_dot(off, off)) < 1e-6:
                return ca, cb, ta, tb
    raise RuntimeError(f"no coaxial cylinders of radii {ra} and {rb}")


def mate_parameters(target, moving):
    """offset, angle, flip that put `moving` where the mate rule puts it
    from `target`: z opposed unless flip, x turned by angle about z,
    origin offset along the target's z."""
    ot, xt, _, zt = target
    om, xm, _, zm = moving
    flip = _dot(zt, zm) > 0.0
    z = zt if flip else tuple(-c for c in zt)
    return _dot(_sub(om, ot), zt), math.degrees(math.atan2(_dot(_cross(xt, xm), z), _dot(xt, xm))), flip


def apply(mcp, ops, tab):
    text = mcp.call("apply", {"ops": ops, "tab": tab})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    return text


IDENTITY = ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0))


def instance_ops(tabs, bodies, titles, fixed, placement=IDENTITY):
    (x, y, z), (rx, ry, rz) = placement
    return [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": fixed,
             "placement": {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": rx, "y": ry, "z": rz}}} for t in titles for k in range(len(bodies[t]))]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "ramp.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "ramp"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Folding torsion-box truck ramp"}]})
    tabs, bodies, reports = {}, {}, {}
    for i, (title, fn) in enumerate(PANEL_PARTS + RAMP_PARTS):
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]}); tab = 1
        else:
            text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": title}]})
            tab = int(re.search(r"tab (\d+)", text).group(1))
        tabs[title] = tab
        part = Part(mcp, tab, title)
        # The tunable numbers, as document variables: the skin and rib
        # extrudes are bound to them; the sketched profiles carry their
        # values and are redrawn by this script.
        apply(mcp, [{"type": "add_variable", "name": n, "expression": f"{v * IN:g}"} for n, v in
                    (("skin", SKIN), ("rib", RIB), ("curb", CURB), ("pin_drop", PIN_DROP), ("lug_a", LUG_A), ("lug_b", LUG_B))], tab)
        fn(part)
        bodies[title] = part.bodies()
        reports[title] = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["bodies"]
        print(f"{title:<18} tab {tab:>2}: {len(bodies[title])} bodies")
    # The panel: every part fixed in place. The same tab is placed twice
    # in the ramp, the second time turned.
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "panel"}]})
    panel = int(re.search(r"tab (\d+)", text).group(1))
    apply(mcp, instance_ops(tabs, bodies, [t for t, _ in PANEL_PARTS], True), panel)
    panel_ids = {i["name"]: i["id"] for i in json.loads(mcp.call("report", {"tab": panel, "detail": "full"}))["instances"]}
    print(f"panel tab {panel}: {len(panel_ids)} instances")
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "ramp"}]})
    ramp = int(re.search(r"tab (\d+)", text).group(1))
    ops = [{"type": "add_instance", "studio": panel, "body": 0, "name": "panel 1", "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}},
           {"type": "add_instance", "studio": panel, "body": 0, "name": "panel 2", "fixed": False,
            "placement": {"position": dict(zip("xyz", PANEL2[0])), "rotation": dict(zip("xyz", PANEL2[1]))}}]
    ops += instance_ops(tabs, bodies, [t for t, _ in RAMP_PARTS], True)
    apply(mcp, ops, ramp)
    ids = {i["name"]: i["id"] for i in json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))["instances"]}
    # The fold: a revolute between the pipe and the turned panel's flush
    # rail bore, its parameters read off the drawn pose.
    ca, cb, ta, tb = coaxial(reports["pipe"][0], PIPE_OD / 2 * IN, IDENTITY, reports["rail_flush"][0], BORE / 2 * IN, PANEL2)
    offset, angle, flip = mate_parameters(ta, tb)
    apply(mcp, [{"type": "add_mate", "kind": "revolute", "a": {"instance": ids["pipe"], "face": ca["reference"]},
                 "b": {"instance": ids["panel 2"], "sub": panel_ids["rail_flush"], "face": cb["reference"]},
                 "offset": offset, "angle": angle, "flip": flip, "name": "fold"}], ramp)
    report = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
    for m in report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")
    for i in report["instances"]:
        if i.get("error"):
            raise RuntimeError(f"instance {i['name']}: {i['error']}")
    p2 = next(i for i in report["instances"] if i["name"] == "panel 2")["placed"]
    drift = max(abs(_v(p2["position"])[k] - PANEL2[0][k]) for k in range(3))
    fold = next(m for m in report["mates"] if m["name"] == "fold")
    print(f"ramp tab {ramp}: {len(ids)} instances, the fold resolved to the drawn pose (worst {drift:.1e} mm)")

    def free_end_z(angle_):
        apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": angle_}], ramp)
        rep = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
        lo = min(b["bounds"][0]["z"] for b in rep["bodies"] if b["name"].startswith("panel 2 /"))
        return lo

    # Which way folds under: the turned panel swings down and under the
    # first, bottoms together, so its free end drops.
    sign = 1.0 if free_end_z(fold["angle"] + 30.0) < free_end_z(fold["angle"]) - 1.0 else -1.0
    at = lambda deg: fold["angle"] + sign * deg

    for view, name in (("iso", "ramp_iso"), ("-0.6,-0.7,0.4", "ramp_front"), ("right", "ramp_side")):
        mcp.call("screenshot", {"tab": ramp, "view": view, "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "section": f"x:{LUG_A * IN}:flip", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_lug_section.png")})
    mcp.call("screenshot", {"tab": panel, "view": "0.5,-0.7,-0.5", "width": 1600, "height": 1000, "path": os.path.join(OUT, "panel_below.png")})
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(180.0)}], ramp)
    mcp.call("screenshot", {"tab": ramp, "view": "iso", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded_side.png")})
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(0.0)}], ramp)
    # Sheets: the panel with a section across the width at mid-length
    # and one along the length through a lug; the ramp with balloons;
    # the stub with its bevel on.
    print(mcp.call("export", {"tab": panel, "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", f"section@{SKIN_END / 2 * IN}", f"section-side@{LUG_A * IN}"],
                              "note": "one panel: 21 x 48 x 2-3/8 torsion box, rails 3/4 birch, lugs two plies", "path": os.path.join(OUT, "panel.pdf")}))
    print(mcp.call("export", {"tab": ramp, "format": "pdf", "sheet": "A3", "note": "two panels on the pipe, open; the pipe pulls to separate them", "path": os.path.join(OUT, "ramp.pdf")}))
    stub_report = json.loads(mcp.call("report", {"tab": tabs["stub"], "detail": "full"}))
    bevel = next(f["id"] for f in stub_report["features"] if f["name"].startswith("ground bevel"))
    apply(mcp, [{"type": "set_suppressed", "id": bevel, "suppressed": False}], tabs["stub"])
    print(mcp.call("export", {"tab": tabs["stub"], "format": "pdf", "sheet": "A4", "note": "the ground panel's stubs, beveled 30 degrees", "path": os.path.join(OUT, "stub.pdf")}))
    apply(mcp, [{"type": "set_suppressed", "id": bevel, "suppressed": True}], tabs["stub"])
    # The fold from the side, 0 to 180 degrees.
    positions = [{"fold": at(d), "label": f"{d:g} degrees" if d else "open"} for d in (0.0, 45.0, 90.0, 135.0, 180.0)]
    print(mcp.call("range_of_motion", {"tab": ramp, "positions": positions, "view": "right", "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "fold.pdf")}))
    mcp.call("range_of_motion", {"tab": ramp, "positions": positions, "view": "right", "width": 2000, "height": 500, "path": os.path.join(OUT, "fold.png")})
    with open(os.path.join(OUT, "cutlist.csv"), "w") as f:
        f.write("item,size,qty,note\n")
        for item, size, qty, note in CUTLIST:
            f.write(f'"{item}","{size}",{qty},"{note}"\n')
    print(f"cut list: {sum(q for _, _, q, _ in CUTLIST)} pieces in {len(CUTLIST)} lines")
    mcp.close()


if __name__ == "__main__":
    main()
