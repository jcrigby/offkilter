"""A folding torsion-box truck ramp: two 21 x 48 in panels of 15/32 in
ply skins on 2x2 ribs, hinged on a 3/4 in pipe below the decks that
doubles as the carrying handle and pulls out to separate the halves.
The brief is README.md in this directory; this script is the document
it asked for, with the changes its section 8 records.

    cargo build --release -p ok-mcp
    python3 examples/ramp/build.py            # the document, pictures, sheets and cut list
    cargo test -p ok-render --test ramp       # what CI runs

The shop works in inches, the document in millimetres like the other
examples: every dimension is a named constant in inches and the helpers
convert. Frame of a panel: x across the width (0 to 21), y along the
length (0 at the hinge end, 48 at the free end), z up from the bottom
face of the bottom skin. The two panels differ only at the hinge: panel
A's rails carry their lugs in one piece, panel B's rails are plain with
a lug stub screwed to the outside of each, and the interior lugs sit
against different ribs, so that on the pipe each end has A's rail lug
beside B's stub and B's interior lugs land outboard of A's. Panel B is
placed turned 180 degrees about z and moved 21 in x. Each panel is
built in place in its own frame; the pipe, caps, end plates and ground
angle are built in the ramp's frame.
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
# The panels, in inches.
# ---------------------------------------------------------------------
W, L = 21.0, 48.0                  # a panel, over the box
SKIN = 0.4375                      # 15/32 CDX
RIB = 1.5                          # 2x2 actual, on edge: the box is SKIN + RIB + SKIN
BOX = 2 * SKIN + RIB               # 2.375
SKIN_END = 45.0                    # the skins stop here; the stubs run bare to 48
CURB = 1.0                         # the rails stand this much above the deck
PIN_DROP = 1.5                     # pipe centre below the bottom skin: lever arm against hang, and the most the lugs' radius can be
PIPE_OD, PIPE_ID = 1.05, 0.824     # 3/4 Sch 40
BORE = 1.125                       # the lug bores
BIRCH = 0.75                       # the rails, stubs and lugs: one ply
RAIL_H = BOX + CURB                # 3.375 along the length
LUG_R = 1.5                        # the half-round about the pin: a 15/16 wall round the bore, the rail bottom at -3; no more than PIN_DROP or it meets the other panel
TAPER = 8.0                        # the lug bottom climbs back to z = 0 over this
CHAMFER = 0.375                    # the bottom skin's hinge-end edge, which sweeps a 1-1/2 in radius about the pin
RIBS = (1.5, 6.0, 10.5, 13.5, 19.5)   # rib centres, both panels
# Interior lug centres (one ply, against a rib's side): panel A beside
# the ribs at 6 and 13.5, panel B beside the ribs at 1.5 and 19.5, so
# that turned, B's land outboard of A's on the pipe.
LUGS_A = (6.0 - RIB / 2 - BIRCH / 2, 13.5 + RIB / 2 + BIRCH / 2)      # 4.875, 14.625
LUGS_B = (1.5 + RIB / 2 + BIRCH / 2, 19.5 - RIB / 2 - BIRCH / 2)      # 2.625, 18.375
LUG_IN = 6.0                       # the foot inside the box, behind the joint block
NOTCH_L = 3.0                      # the bottom skin's notch, where the lug passes through
STUB_LAP = 2.5                     # panel B's lug stub laps the rail's outside face this far up
CROSS_Y = (16.0, 32.0)             # cross block rows
JOINT = 3.5                        # the 2x4 flat across the hinge end, between the skins
STUB_W, STUB_L, STUB_Y0 = 7.25, 12.0, 36.0   # 2x8 flat at the free end, 12 long from y 36, bare past 45
BEVEL = 30.0                       # degrees, on the ground end
PLATE_L = 2.0                      # the end plates' ghost
ANGLE_T, ANGLE_LEG = 0.125, 1.0    # the aluminium angle over the ground edge
CAP_OD, CAP_L = 1.3, 1.0

# Panel B's placement in the ramp: 180 degrees about z, moved W in x:
# its x runs 21 to 0, its y 0 to -48.
PLACE_B = ((W * IN, 0.0, 0.0), (0.0, 0.0, 180.0))


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


# -- parts both panels share -------------------------------------------

def skin_top(p):
    f = box(p, "top skin, 15/32 CDX", (0.0, W), (0.0, SKIN_END), (BOX - SKIN, BOX))
    p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#skin"})


def skin_bottom(lugs):
    """The bottom skin, its hinge-end bottom edge chamfered since it
    sweeps a radius about the pin, and notched where the lugs pass."""
    def build(p):
        profile_x(p, "bottom skin, 15/32 CDX", 0.0, W, [(CHAMFER, 0.0), (SKIN_END, 0.0), (SKIN_END, SKIN), (0.0, SKIN), (0.0, CHAMFER)])
        s = p.sketch("top", SKIN * IN + 1.0, "lug notches")
        for a in lugs:
            p.rect(s, mm(a - BIRCH / 2, JOINT), mm(a + BIRCH / 2, JOINT + NOTCH_L))
        p.cut(s, SKIN * IN + 2.0, direction="reverse", name="lug notches")
    return build


def rib_span(c):
    """Where a rib runs: from the joint block to the free end, or to the
    stubs where it would meet one."""
    y1 = STUB_Y0 if (c - RIB / 2 < STUB_W or c + RIB / 2 > W - STUB_W) else SKIN_END
    return JOINT, y1


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
    where only panel B's end meets the ground."""
    for x0 in (0.0, W - STUB_W):
        box(p, "tailgate stub, 2x8 flat x 12", (x0, x0 + STUB_W), (STUB_Y0, L), (SKIN, SKIN + RIB), op="new")
    back = RIB * math.tan(math.radians(BEVEL))
    s = p.sketch("right", -1.0 * IN, "bevel")
    p.polygon(s, [mm(L - back, SKIN + RIB), mm(L + 0.1, SKIN + RIB), mm(L + 0.1, SKIN)])
    f = p.cut(s, (W + 2.0) * IN, direction="normal", name=f"ground bevel, {BEVEL:g} degrees")
    p.apply({"type": "set_suppressed", "id": f, "suppressed": True})


# -- the hinge: panel A's rails with lugs, panel B's plain rails and stubs

def lug_profile():
    """The hanging part about the pin: from the skin line down the
    half-round and back up over TAPER."""
    return [(0.0, 0.0), (TAPER, 0.0), (0.0, -2 * LUG_R)]


def rail_with_lug(x0):
    """Panel A: a flush rail, 3/4 birch on edge, a curb above the deck,
    its lug built in below the deck as one piece."""
    def build(p):
        profile_x(p, "rail with lug, 3/4 birch", x0, x0 + BIRCH,
                  [(0.0, RAIL_H), (SKIN_END, RAIL_H), (SKIN_END, 0.0), (TAPER, 0.0), (0.0, -2 * LUG_R)], circle=((0.0, -PIN_DROP), LUG_R))
        bore_x(p, "pipe bore", x0, x0 + BIRCH, 0.0, -PIN_DROP, BORE)
    return build


def rails_plain(p):
    """Panel B: two flush rails, no lug, ending flat on the joint plane."""
    for i, x0 in enumerate((-BIRCH, W)):
        box(p, "rail, 3/4 birch", (x0, x0 + BIRCH), (0.0, SKIN_END), (0.0, RAIL_H), op="new")


def lug_stub(x0):
    """Panel B: a lug screwed to the outside of a plain rail at the hinge
    end, lapping the rail's face STUB_LAP up from the deck line."""
    def build(p):
        profile_x(p, "lug stub, 3/4 birch", x0, x0 + BIRCH,
                  [(0.0, STUB_LAP), (TAPER, STUB_LAP), (TAPER, 0.0), (0.0, -2 * LUG_R)], circle=((0.0, -PIN_DROP), LUG_R))
        bore_x(p, "pipe bore", x0, x0 + BIRCH, 0.0, -PIN_DROP, BORE)
    return build


def interior_lug(a):
    """A lug at x = a: one ply of birch, its foot inside the box behind
    the joint block and against the side of a rib (screwed to it, and
    nailed from the bottom skin), down through the skin's notch, and the
    half-round about the pin under the joint block, tapering back up to
    the skin. The pull on it bears on the joint block; the fasteners
    hold it square and are the insurance."""
    def build(p):
        x0, x1 = a - BIRCH / 2, a + BIRCH / 2
        profile_x(p, "interior lug, 3/4 birch", x0, x1,
                  [(0.0, 0.0), (0.0, -2 * LUG_R), (JOINT, -2 * LUG_R), (JOINT + NOTCH_L, 0.0)], circle=((0.0, -PIN_DROP), LUG_R))
        box(p, "through the notch", (x0, x1), (JOINT, JOINT + NOTCH_L), (0.0, SKIN), op="add")
        box(p, "inside the box", (x0, x1), (JOINT, JOINT + LUG_IN), (SKIN, SKIN + RIB), op="add")
        bore_x(p, "pipe bore", x0, x1, 0.0, -PIN_DROP, BORE)
    return build


# -- ramp parts, in the ramp's frame ----------------------------------

PIPE_X = (-2 * BIRCH - 0.25, W + 2 * BIRCH + 0.25)   # -1.75 to 22.75: 24-1/2 in, a quarter past each stub for the caps


def pipe(p):
    s = p.sketch("right", PIPE_X[0] * IN, "pipe")
    p.circle(s, mm(0.0, -PIN_DROP), PIPE_OD / 2 * IN)
    p.extrude(s, (PIPE_X[1] - PIPE_X[0]) * IN, name=f"hinge pipe, 3/4 Sch 40 x {PIPE_X[1] - PIPE_X[0]:g}")
    bore_x(p, "bore", PIPE_X[0], PIPE_X[1], 0.0, -PIN_DROP, PIPE_ID)


def pipe_caps(p):
    for x0, x1 in ((PIPE_X[0] - CAP_L, PIPE_X[0]), (PIPE_X[1], PIPE_X[1] + CAP_L)):
        s = p.sketch("right", x0 * IN, "cap")
        p.circle(s, mm(0.0, -PIN_DROP), CAP_OD / 2 * IN)
        p.extrude(s, (x1 - x0) * IN, op="new", name="pipe cap, 3/4")


def end_plates(p):
    """Ghosts of the ramp end plates on panel A's bare stubs: a block
    with the 1-1/2 x 7-1/4 pocket, 2 in long, no casting."""
    for x0 in (0.0, W - STUB_W):
        box(p, "end plate (ghost)", (x0 - 0.25, x0 + STUB_W + 0.25), (L - PLATE_L, L), (0.0, BOX), op="new")
    for x0 in (0.0, W - STUB_W):
        s = p.sketch("front", -(L + 0.5) * IN, "pocket")
        p.rect(s, mm(x0, SKIN), mm(x0 + STUB_W, SKIN + RIB))
        p.cut(s, (PLATE_L + 1.0) * IN, direction="normal", name="board pocket")


def ground_angle(p):
    """1/8 x 1 aluminium angle over panel B's ground edge, at the ramp's
    y = -48: a leg on the stubs' top and a leg down the end."""
    y = -L
    box(p, "ground angle, 1/8 x 1 aluminium x 21", (0.0, W), (y, y + ANGLE_LEG), (SKIN + RIB, SKIN + RIB + ANGLE_T))
    box(p, "leg", (0.0, W), (y - ANGLE_T, y), (SKIN + RIB + ANGLE_T - ANGLE_LEG, SKIN + RIB + ANGLE_T), op="add")


SHARED = [("skin_top", skin_top), ("rib", ribs), ("cross_block", cross_blocks), ("joint_block", joint_block), ("stub", stubs)]
PANEL_A = [("skin_bottom_a", skin_bottom(LUGS_A)), ("rail_lug_left", rail_with_lug(-BIRCH)), ("rail_lug_right", rail_with_lug(W)),
           ("interior_lug_a1", interior_lug(LUGS_A[0])), ("interior_lug_a2", interior_lug(LUGS_A[1]))]
PANEL_B = [("skin_bottom_b", skin_bottom(LUGS_B)), ("rail_plain", rails_plain), ("lug_stub_left", lug_stub(-2 * BIRCH)), ("lug_stub_right", lug_stub(W + BIRCH)),
           ("interior_lug_b1", interior_lug(LUGS_B[0])), ("interior_lug_b2", interior_lug(LUGS_B[1]))]
RAMP_PARTS = [("pipe", pipe), ("pipe_cap", pipe_caps), ("end_plate", end_plates), ("ground_angle", ground_angle)]

# The cut list: what to cut, in inches, per the shop's stock.
CUTLIST = [
    ("top skin", f"{W:g} x {SKIN_END:g} x 15/32 CDX", 2, "one 4x8 sheet: two 21 in rips, each crosscut at 45 and 45"),
    ("bottom skin", f"{W:g} x {SKIN_END:g} x 15/32 CDX, two 3/4 x 3 notches", 2, "from the same sheet; the notches differ between the panels"),
    ("rib", "2x2 x 32.5", 8, "the four lines that meet a stub, joint block to stubs"),
    ("rib", "2x2 x 41.5", 2, "middle line, joint block to the skins' end"),
    ("cross block", "2x2 x 3", 8, "between the ribs at 1.5 and 6, and 6 and 10.5"),
    ("cross block", "2x2 x 1.5", 4, "between the ribs at 10.5 and 13.5"),
    ("cross block", "2x2 x 4.5", 4, "between the ribs at 13.5 and 19.5"),
    ("joint block", "2x4 flat x 21", 2, ""),
    ("stub", "2x8 x 12", 4, "3 in bare past the skins"),
    ("rail with lug", f"3/4 birch, {SKIN_END:g} x {RAIL_H:g} with the lug profile, {TAPER:g} x {2 * LUG_R:g} below", 2, "panel A, flush both sides"),
    ("rail", f"3/4 birch, {SKIN_END:g} x {RAIL_H:g}", 2, "panel B, flush both sides"),
    ("lug stub", f"3/4 birch, {TAPER:g} x {STUB_LAP + 2 * LUG_R:g}, lug profile", 2, "panel B, screwed to the outside of each rail: four no. 8 x 1-1/4"),
    ("interior lug", f"3/4 birch, {JOINT + LUG_IN:g} x {RIB + SKIN + 2 * LUG_R:g}, lug profile", 4, "two per panel; two no. 8 x 2 screws into the rib beside each, three 1-1/4 nails from the bottom skin into the foot"),
    ("hinge pipe", f"3/4 Sch 40 galvanized, {PIPE_X[1] - PIPE_X[0]:g}, threaded both ends", 1, "with two caps"),
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


def instance_ids(mcp, tab):
    return {i["name"]: i["id"] for i in json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["instances"]}


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "ramp.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "ramp"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Folding torsion-box truck ramp"}]})
    tabs, bodies, reports = {}, {}, {}
    for i, (title, fn) in enumerate(SHARED + PANEL_A + PANEL_B + RAMP_PARTS):
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
                    (("skin", SKIN), ("rib", RIB), ("curb", CURB), ("pin_drop", PIN_DROP), ("lug_r", LUG_R))], tab)
        fn(part)
        bodies[title] = part.bodies()
        reports[title] = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["bodies"]
        print(f"{title:<18} tab {tab:>2}: {len(bodies[title])} bodies")
    # The two panels, every part fixed in place in the panel's frame.
    panels = {}
    for name, parts in (("panel A", SHARED + PANEL_A), ("panel B", SHARED + PANEL_B)):
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": name}]})
        panels[name] = int(re.search(r"tab (\d+)", text).group(1))
        apply(mcp, instance_ops(tabs, bodies, [t for t, _ in parts], True), panels[name])
        print(f"{name:<18} tab {panels[name]:>2}: {len(instance_ids(mcp, panels[name]))} instances")
    b_ids = instance_ids(mcp, panels["panel B"])
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "ramp"}]})
    ramp = int(re.search(r"tab (\d+)", text).group(1))
    ops = [{"type": "add_instance", "studio": panels["panel A"], "body": 0, "name": "panel A", "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}},
           {"type": "add_instance", "studio": panels["panel B"], "body": 0, "name": "panel B", "fixed": False,
            "placement": {"position": dict(zip("xyz", PLACE_B[0])), "rotation": dict(zip("xyz", PLACE_B[1]))}}]
    ops += instance_ops(tabs, bodies, [t for t, _ in RAMP_PARTS], True)
    apply(mcp, ops, ramp)
    ids = instance_ids(mcp, ramp)
    # The fold: a revolute between the pipe and panel B's left lug stub
    # bore, its parameters read off the drawn pose.
    ca, cb, ta, tb = coaxial(reports["pipe"][0], PIPE_OD / 2 * IN, IDENTITY, reports["lug_stub_left"][0], BORE / 2 * IN, PLACE_B)
    offset, angle, flip = mate_parameters(ta, tb)
    apply(mcp, [{"type": "add_mate", "kind": "revolute", "a": {"instance": ids["pipe"], "face": ca["reference"]},
                 "b": {"instance": ids["panel B"], "sub": b_ids["lug_stub_left"], "face": cb["reference"]},
                 "offset": offset, "angle": angle, "flip": flip, "name": "fold"}], ramp)
    report = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
    for m in report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")
    for i in report["instances"]:
        if i.get("error"):
            raise RuntimeError(f"instance {i['name']}: {i['error']}")
    pb = next(i for i in report["instances"] if i["name"] == "panel B")["placed"]
    drift = max(abs(_v(pb["position"])[k] - PLACE_B[0][k]) for k in range(3))
    fold = next(m for m in report["mates"] if m["name"] == "fold")
    print(f"ramp tab {ramp}: {len(ids)} instances, the fold resolved to the drawn pose (worst {drift:.1e} mm)")

    def low_b(angle_):
        apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": angle_}], ramp)
        rep = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
        return min(b["bounds"][0]["z"] for b in rep["bodies"] if b["name"].startswith("panel B /"))

    # Which way folds under: panel B swings down and under panel A,
    # bottoms together, so its free end drops.
    sign = 1.0 if low_b(fold["angle"] + 30.0) < low_b(fold["angle"]) - 1.0 else -1.0
    at = lambda deg: fold["angle"] + sign * deg

    for view, name in (("iso", "ramp_iso"), ("-0.6,-0.7,0.4", "ramp_front"), ("right", "ramp_side")):
        mcp.call("screenshot", {"tab": ramp, "view": view, "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "section": f"x:{LUGS_A[0] * IN}:flip", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_lug_section.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "-0.5,-0.6,-0.6", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_below.png")})
    for name, stem in (("panel A", "panel_a"), ("panel B", "panel_b")):
        mcp.call("screenshot", {"tab": panels[name], "view": "0.5,-0.7,-0.5", "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{stem}_below.png")})
        # The box from below with the bottom skin cut away: the lug feet
        # against their ribs, the joint block, the top skin intact above.
        mcp.call("screenshot", {"tab": panels[name], "view": "0.3,-0.4,-0.85", "section": f"z:{(SKIN + 0.05) * IN}", "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{stem}_cutaway.png")})
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(180.0)}], ramp)
    mcp.call("screenshot", {"tab": ramp, "view": "iso", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded_side.png")})
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(0.0)}], ramp)
    # Sheets: each panel with a section across the width at mid-length
    # and one along the length through a lug; the ramp with balloons;
    # the stub with its bevel on.
    for name, stem, lug in (("panel A", "panel_a", LUGS_A[0]), ("panel B", "panel_b", LUGS_B[0])):
        print(mcp.call("export", {"tab": panels[name], "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", f"section@{SKIN_END / 2 * IN}", f"section-side@{lug * IN}"],
                                  "note": f"{name}: 21 x 48 x 2-3/8 torsion box, rails and lugs 3/4 birch", "path": os.path.join(OUT, f"{stem}.pdf")}))
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
