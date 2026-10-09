"""A mini chop saw from a 4-1/2 in angle grinder: the old orange Chicago
Electric / Central Machinery grinder (11,000 rpm, 5/8-11 spindle) on a
wooden arm that pivots on a 1/2 in bolt between two laminated birch
cheeks, the whole stand one plywood base clamped to the bench. Its first
job is the duplicator's 20 mm hardened shafts: the rod lies in two
hardwood V-blocks and is turned by hand while the disc, held at a depth
stop just past the rod's centre, works round it; the hard case goes in
the first turn and the core parts when the offcut drops. The brief and
what is measured versus guessed are in README.md here.

    cargo build --release -p ok-mcp
    python3 examples/chop-saw/build.py            # the document, pictures and sheets
    cargo test -p ok-render --test chop_saw       # what CI runs

Frame: x along the rod (the disc's inner face is the plane x = 0, the
grinder's gearhead on the +x side, the long end of the rod fed from -x),
y from the operator (-y, where the motor body points) back to the pivot
(+y), z up from the bench. The drawn pose is the arm down on its depth
stop. Wood is in inches like the shop; the grinder and the rod are in
millimetres as measured.
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
# The grinder, millimetres. M = measured, G = guessed from the photos.
# ---------------------------------------------------------------------
DISC_D = 114.3          # 4-1/2 in cut-off disc, new (M: the label)
DISC_T = 1.0            # 0.040 in thin metal cut-off
DISC_BORE = 22.23       # 7/8 in arbor hole
SPINDLE_D = 15.9        # 5/8-11 (M: the label)
NUT_D, NUT_T = 30.0, 6.0          # outer locking nut (G)
COLLAR_D = 40.0                   # bearing collar round the spindle, disc to gearhead (G)
GH_X = (28.0, 75.0)               # gearhead along the spindle from the disc's inner face (M: 25-28 and 72-75 in the photo)
GH_FWD, GH_BACK = 57.0, 28.0      # gearhead from the spindle centre toward the body, and the nose behind it (G: 85 long, spindle 28 from the nose)
GH_HALF = 32.0                    # gearhead half-height about the spindle axis, boss face to boss face / 2 (G)
BOSS_X = 52.0                     # side-handle boss centre from the disc's inner face (M: 52 +/- 1)
BOSS_FWD = 14.0                   # boss centre ahead of the spindle centre along the body (G: 12-15)
BOSS_THREAD, BOSS_DEPTH = 10.0, 20.0   # M10 x 1.5 (G: bigger than the Bauer's M8; confirm at the store) and how deep it is threaded (G)
BODY_D, BODY_L = 62.0, 183.0      # motor body diameter and length past the gearhead (G)
GUARD_R, GUARD_T = 62.5, 1.5      # the guard's rim radius and sheet thickness (G)
GUARD_X = (-10.0, 4.5)            # rim across the disc, back plate on the gearhead side (G)

# ---------------------------------------------------------------------
# The work.
# ---------------------------------------------------------------------
ROD_D = 20.0                      # SFC20 shaft, induction hardened
ROD_X = (-350.0, 250.0)           # a length of it for the picture; the real rods are 1000 long
OVERCUT = 0.5                     # the depth stop puts the disc's bottom this far below the rod's axis

# ---------------------------------------------------------------------
# The stand, inches.
# ---------------------------------------------------------------------
PLY = 0.75                        # 3/4 birch ply, the ramp's leftovers
BASE_X, BASE_Y = (-8.0, 8.0), (-4.5, 10.0)   # 16 x 14-1/2
V_H, V_DEPTH = 1.5, 2.5           # V-blocks: hardwood 1-1/2 tall, 2-1/2 front to back
V_TOP = 1.25                      # the 90 degree groove's width at the top
V_IN = (-5.75, -0.28)             # infeed V-block along x (its inner face 7 mm from the disc)
V_OUT = (0.24, 4.75)              # outfeed V-block (6 mm from the disc on the gearhead side)
ARM_W = 1.5                       # arm and cheeks: two plies of 3/4 birch laminated, 1-1/2 square
PIVOT_Y = 8.0                     # pivot behind the spindle
ARM_FRONT = -6.0                  # the arm's front end, over the motor body where the hose clamps go
CHEEK_W = 3.0                     # cheeks front to back
PIVOT_D, PIVOT_BORE = 0.5, 0.512  # 1/2 in bolt, 13 mm bores drilled snug and waxed
WASHER_T, WASHER_OD = 1.6, 27.0   # 1/2 SAE washers (mm): one each side of the arm, one under the head and nut
POST = (1.75, 3.25)               # the depth-stop post along y, 1-1/2 square, under the arm
POST_H = 4.0
STOP_D = 0.25                     # 1/4-20 stop bolt, its head bearing on the arm's underside
CLAMP_Y = -130.0                  # mm: the hose clamps round arm and body

# Derived heights, millimetres.
BASE_TOP = PLY * IN
V_TOP_Z = BASE_TOP + V_H * IN
ROD_Z = V_TOP_Z - V_TOP / 2 * IN + ROD_D / 2 * math.sqrt(2.0)   # the rod's axis, sitting in the 90 degree V
DISC_Z = ROD_Z - OVERCUT + DISC_D / 2                           # the spindle axis with the arm on its stop
ARM_Z = (DISC_Z + GH_HALF, DISC_Z + GH_HALF + ARM_W * IN)       # the arm sits on the gearhead's top boss
PIVOT_Z = (ARM_Z[0] + ARM_Z[1]) / 2
ARM_X = (BOSS_X - ARM_W / 2 * IN, BOSS_X + ARM_W / 2 * IN)      # centred over the boss
CHEEK_L = (ARM_X[0] - WASHER_T - ARM_W * IN, ARM_X[0] - WASHER_T)
CHEEK_R = (ARM_X[1] + WASHER_T, ARM_X[1] + WASHER_T + ARM_W * IN)

MATERIALS = {"birch": ("3/4 birch ply", 0.68), "hardwood": ("maple", 0.70), "steel": ("steel", 7.85), "rod": ("hardened steel", 7.85)}


def mm(*v):
    return tuple(c * IN for c in v)


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, d = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": d}})
    p.apply(*ops)


def block(p, name, x, y, z, op="new"):
    """A box, millimetres."""
    s = p.sketch("top", z[0], name)
    p.rect(s, (x[0], y[0]), (x[1], y[1]))
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def cyl_x(p, name, x0, x1, y, z, d, op="new"):
    s = p.sketch("right", x0, name)
    p.circle(s, (y, z), d / 2)
    return p.extrude(s, x1 - x0, op=op, name=name)


def cyl_y(p, name, y0, y1, x, z, d, op="new"):
    """A cylinder along y from y0 down to y1 (y1 < y0)."""
    s = p.sketch("front", -y0, name)       # the front plane's normal is -y: offset -y0 puts it at y = y0
    p.circle(s, (x, z), d / 2)
    return p.extrude(s, y0 - y1, op=op, name=name)


def cyl_z(p, name, z0, z1, x, y, d, op="new"):
    s = p.sketch("top", z0, name)
    p.circle(s, (x, y), d / 2)
    return p.extrude(s, z1 - z0, op=op, name=name)


def bore_x(p, name, x0, x1, y, z, d):
    s = p.sketch("right", x0 - 1.0, name)
    p.point(s, (y, z))
    p.hole(s, d, depth=x1 - x0 + 2.0, direction="normal", name=name)


def bore_down(p, name, z_top, x, y, d, depth):
    s = p.sketch("top", z_top, name)
    p.point(s, (x, y))
    p.hole(s, d, depth=depth, direction="reverse", name=name)


def profile_x(p, name, x0, x1, pts, op="new"):
    """A polygon in the y-z plane (millimetres) extruded from x0 to x1."""
    s = p.sketch("right", x0, name)
    p.polygon(s, pts)
    return p.extrude(s, x1 - x0, op=op, name=name)


def round_end_x(p, name, x0, x1, y, z, r):
    """A half-round on a bar's end: a full circle unioned on."""
    s = p.sketch("right", x0, name)
    p.circle(s, (y, z), r)
    p.extrude(s, x1 - x0, op="add", name=name)


def half_annulus_x(p, name, x0, x1, y, z, r_out, r_in, op="new"):
    """The upper half of a ring about an axis along x, from x0 to x1."""
    s = p.sketch("right", x0, name)
    p.draw(s, {"type": "add_arc", "center": {"x": y, "y": z}, "start": {"x": y + r_out, "y": z}, "end": {"x": y - r_out, "y": z}})
    p.draw(s, {"type": "add_arc", "center": {"x": y, "y": z}, "start": {"x": y + r_in, "y": z}, "end": {"x": y - r_in, "y": z}})
    p.draw(s, {"type": "add_line", "a": {"x": y - r_out, "y": z}, "b": {"x": y - r_in, "y": z}})
    p.draw(s, {"type": "add_line", "a": {"x": y + r_in, "y": z}, "b": {"x": y + r_out, "y": z}})
    return p.extrude(s, x1 - x0, profiles="largest", op=op, name=name)


# -- the stand: everything that stays put. One tab per part, since an
# "add" unions into every body whose box it touches.

def base(p):
    f = block(p, "base", mm(*BASE_X), mm(*BASE_Y), (0.0, BASE_TOP))
    part_is(p, f, f"base, 3/4 ply {BASE_X[1] - BASE_X[0]:g} x {BASE_Y[1] - BASE_Y[0]:g}", "birch")


def v_block(name, span):
    """A 90 degree groove, apex on y = 0, the rod's axis above it."""
    def build(p):
        h, d = V_TOP / 2 * IN, V_DEPTH / 2 * IN
        x0, x1 = span
        f = profile_x(p, name, x0 * IN, x1 * IN, [(-d, BASE_TOP), (d, BASE_TOP), (d, V_TOP_Z), (h, V_TOP_Z), (0.0, V_TOP_Z - h), (-h, V_TOP_Z), (-d, V_TOP_Z)])
        part_is(p, f, f"{name}, {x1 - x0:.2f} x {V_DEPTH:g} x {V_H:g}", "hardwood")
    return build


def cheek(name, span):
    """Two plies laminated, a half-round over the pivot."""
    def build(p):
        y0, y1 = PIVOT_Y * IN - CHEEK_W / 2 * IN, PIVOT_Y * IN + CHEEK_W / 2 * IN
        r = CHEEK_W / 2 * IN
        x0, x1 = span
        f = profile_x(p, name, x0, x1, [(y0, BASE_TOP), (y1, BASE_TOP), (y1, PIVOT_Z), (y0, PIVOT_Z)])
        round_end_x(p, f"{name} round", x0, x1, PIVOT_Y * IN, PIVOT_Z, r)
        bore_x(p, f"{name} bore", x0, x1, PIVOT_Y * IN, PIVOT_Z, PIVOT_BORE * IN)
        part_is(p, f, f"{name}, 2 x 3/4 ply, {CHEEK_W:g} x {(PIVOT_Z + r - BASE_TOP) / IN:.2f}", "birch")
    return build


POST_X = ARM_X
POST_TOP = BASE_TOP + POST_H * IN
POST_C = ((ARM_X[0] + ARM_X[1]) / 2, (POST[0] + POST[1]) / 2 * IN)


def stop_post(p):
    f = block(p, "stop post", POST_X, mm(*POST), (BASE_TOP, POST_TOP))
    bore_down(p, "stop bolt hole", POST_TOP, POST_C[0], POST_C[1], STOP_D * IN, 1.5 * IN)
    part_is(p, f, f"stop post, 2 x 3/4 ply, 1-1/2 square x {POST_H:g}", "birch")


def stop_bolt(p):
    """Its head bears on the arm's underside with the arm down; a jam
    nut on the post's top locks it."""
    cx, cy = POST_C
    head_t = 4.4
    f = cyl_z(p, "stop bolt", POST_TOP - 1.5 * IN + 2.0, ARM_Z[0] - head_t, cx, cy, STOP_D * IN - 0.3)
    cyl_z(p, "stop bolt head", ARM_Z[0] - head_t, ARM_Z[0], cx, cy, 12.7, op="add")
    cyl_z(p, "jam nut", POST_TOP, POST_TOP + 5.6, cx, cy, 12.7, op="add")
    part_is(p, f, "stop bolt, 1/4-20 x 2-1/2 hex, jam nut", "steel")


def pivot_bolt(p):
    """Head and washer on the left, washer and nylock nut on the right;
    the shank through both cheeks and the arm."""
    lx = CHEEK_L[0] - WASHER_T
    rx = CHEEK_R[1] + WASHER_T
    f = cyl_x(p, "pivot bolt", lx, lx + 5.5 * IN, PIVOT_Y * IN, PIVOT_Z, PIVOT_D * IN - 0.2)
    cyl_x(p, "pivot bolt head", lx - 8.0, lx, PIVOT_Y * IN, PIVOT_Z, 21.0, op="add")
    cyl_x(p, "outer washer, head", lx, CHEEK_L[0], PIVOT_Y * IN, PIVOT_Z, WASHER_OD, op="add")
    cyl_x(p, "outer washer, nut", CHEEK_R[1], rx, PIVOT_Y * IN, PIVOT_Z, WASHER_OD, op="add")
    cyl_x(p, "nylock nut", rx, rx + 12.7, PIVOT_Y * IN, PIVOT_Z, 21.0, op="add")
    part_is(p, f, "pivot bolt, 1/2-13 x 5-1/2 hex, nylock, 2 washers", "steel")


def inner_washers(p):
    """The two washers between the cheeks and the arm."""
    for name, x0 in (("inner washer, left", CHEEK_L[1]), ("inner washer, right", ARM_X[1])):
        f = cyl_x(p, name, x0, x0 + WASHER_T, PIVOT_Y * IN, PIVOT_Z, WASHER_OD)
        bore_x(p, f"{name} hole", x0, x0 + WASHER_T, PIVOT_Y * IN, PIVOT_Z, 13.5)
        part_is(p, f, f"{name}, 1/2 SAE", "steel")


def rod(p):
    """The work: a length of the 20 mm shaft in the V-blocks."""
    f = cyl_x(p, "rod", ROD_X[0], ROD_X[1], 0.0, ROD_Z, ROD_D)
    part_is(p, f, "rod, 20 mm shaft (SFC20)", "rod")


# -- the head: everything on the arm ------------------------------------

def arm(p):
    """Two plies laminated, a half-round about the pivot."""
    y0, y1 = ARM_FRONT * IN, PIVOT_Y * IN
    f = profile_x(p, "arm", ARM_X[0], ARM_X[1], [(y0, ARM_Z[0]), (y1, ARM_Z[0]), (y1, ARM_Z[1]), (y0, ARM_Z[1])])
    round_end_x(p, "arm round", ARM_X[0], ARM_X[1], y1, PIVOT_Z, ARM_W / 2 * IN)
    bore_x(p, "arm pivot bore", ARM_X[0], ARM_X[1], y1, PIVOT_Z, PIVOT_BORE * IN)
    bore_down(p, "arm boss bolt hole", ARM_Z[1], BOSS_X, -BOSS_FWD, BOSS_THREAD + 0.5, ARM_W * IN + 2.0)
    part_is(p, f, f"arm, 2 x 3/4 ply, 1-1/2 square x {(y1 - y0) / IN + ARM_W / 2:.2f}", "birch")


def grinder(p):
    """Gearhead, bearing collar, spindle, outer nut and motor body."""
    f = block(p, "gearhead", GH_X, (-GH_FWD, GH_BACK), (DISC_Z - GH_HALF, DISC_Z + GH_HALF))
    cyl_x(p, "bearing collar", 0.0, GH_X[0], 0.0, DISC_Z, COLLAR_D, op="add")
    cyl_x(p, "spindle", -DISC_T - NUT_T, 0.0, 0.0, DISC_Z, SPINDLE_D, op="add")
    cyl_x(p, "outer nut", -DISC_T - NUT_T, -DISC_T, 0.0, DISC_Z, NUT_D, op="add")
    cyl_y(p, "motor body", -GH_FWD, -GH_FWD - BODY_L, BOSS_X, DISC_Z, BODY_D, op="add")
    bore_down(p, "side-handle boss", DISC_Z + GH_HALF, BOSS_X, -BOSS_FWD, BOSS_THREAD, BOSS_DEPTH)
    part_is(p, f, "grinder, 4-1/2 in (Chicago Electric, orange)")


def disc(p):
    """Its inner face on x = 0."""
    f = cyl_x(p, "disc", -DISC_T, 0.0, 0.0, DISC_Z, DISC_D)
    bore_x(p, "disc arbor hole", -DISC_T, 0.0, 0.0, DISC_Z, DISC_BORE)
    part_is(p, f, "disc, 4-1/2 x 0.040 x 7/8 cut-off", "steel")


def guard(p):
    """Turned to cover the top half: a back plate on the gearhead side
    of the disc and a rim across it."""
    f = half_annulus_x(p, "guard back plate", GUARD_X[1] - GUARD_T, GUARD_X[1], 0.0, DISC_Z, GUARD_R, COLLAR_D / 2 + 1.0)
    half_annulus_x(p, "guard rim", GUARD_X[0], GUARD_X[1] - GUARD_T, 0.0, DISC_Z, GUARD_R, GUARD_R - GUARD_T, op="add")
    part_is(p, f, "guard, the grinder's own, top half", "steel")


def boss_bolt(p):
    """Through the arm into the top boss, a washer under the head."""
    f = cyl_z(p, "boss bolt", ARM_Z[1] + 2.0 - 50.0, ARM_Z[1] + 2.0, BOSS_X, -BOSS_FWD, BOSS_THREAD - 0.3)
    cyl_z(p, "boss bolt washer", ARM_Z[1], ARM_Z[1] + 2.0, BOSS_X, -BOSS_FWD, 21.0, op="add")
    cyl_z(p, "boss bolt head", ARM_Z[1] + 2.0, ARM_Z[1] + 8.5, BOSS_X, -BOSS_FWD, 18.0, op="add")
    part_is(p, f, "boss bolt, M10 x 50 hex, washer", "steel")


STAND = [("base", base), ("infeed V-block", v_block("infeed V-block", V_IN)), ("outfeed V-block", v_block("outfeed V-block", V_OUT)),
         ("left cheek", cheek("left cheek", CHEEK_L)), ("right cheek", cheek("right cheek", CHEEK_R)), ("stop post", stop_post),
         ("stop bolt", stop_bolt), ("pivot bolt", pivot_bolt), ("inner washers", inner_washers), ("rod", rod)]
HEAD = [("arm", arm), ("grinder", grinder), ("disc", disc), ("guard", guard), ("boss bolt", boss_bolt)]


# -- mate helpers (as in examples/ramp/build.py) -------------------------

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


def _frame(cyl, face):
    """A cylindrical face's connector frame as the kernel builds it."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    t = _dot(_sub(c, o), z)
    origin = (o[0] + z[0] * t, o[1] + z[1] * t, o[2] + z[2] * t)
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    return origin, _cross(y, z), y, z


def cylinder(body, r):
    """The cylinder of radius r on a body, with its frame (bodies are
    built in place, so the frame is the world one)."""
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    for c in body["cylinders"]:
        if abs(c["radius"] - r) < 1e-6:
            return c, _frame(c, faces[json.dumps(c["reference"], sort_keys=True)])
    raise RuntimeError(f"{body['name']}: no cylinder of radius {r}")


def mate_parameters(target, moving):
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


def new_tab(mcp, op):
    text = mcp.call("apply", {"ops": [op]})
    return int(re.search(r"tab (\d+)", text).group(1))


def full(mcp, tab):
    return json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))


IDENTITY = {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}


HARDWARE = [
    ("pivot bolt", "1/2-13 x 5-1/2 hex bolt, nylock nut", 1, "through both cheeks and the arm; snug the nut until the arm has no side play and still falls under its own weight"),
    ("washer", "1/2 SAE flat washer", 4, "one under the head, one under the nut, one each side of the arm"),
    ("boss bolt", "M10 x 1.5 x 50 hex bolt (confirm the thread in the boss; 3/8-16 if not metric), washer", 1, "down through the arm into the grinder's top side-handle boss, about 10 mm of thread engaged; 60 long if the boss is threaded deeper than 20"),
    ("hose clamp", "stainless, 2-1/2 to 4 in", 2, f"joined into one loop round the arm and the motor body, {-CLAMP_Y:g} mm ahead of the spindle"),
    ("stop bolt", "1/4-20 x 2-1/2 hex bolt, 2 nuts", 1, "threaded into the post's top (a tee nut, or tapped into hardwood), its head under the arm, locked with a jam nut"),
    ("return spring", "screen-door spring or a short bungee", 1, "from the arm's front to a screw in the base's back edge, enough to lift the head off the work"),
    ("disc", "4-1/2 x 0.040 x 7/8 metal cut-off, Type 1, rated 13,300 rpm", 5, "11,000 rpm grinder; reset the stop bolt as the disc wears"),
    ("clamps", "F-clamps or quick-grips", 2, "the base to the bench through its front corners"),
]

CUTLIST = [
    ("base", f"3/4 birch ply, {BASE_X[1] - BASE_X[0]:g} x {BASE_Y[1] - BASE_Y[0]:g}", 1, "the rod runs left to right across it"),
    ("cheek", f"3/4 birch ply, {CHEEK_W:g} x {(PIVOT_Z + CHEEK_W / 2 * IN - BASE_TOP) / IN:.2f}, round top", 4, "glued in pairs to 1-1/2; 1/2 in hole through each pair at the round's centre, then glued and screwed to the base from below"),
    ("arm", f"3/4 birch ply, 1-1/2 x {PIVOT_Y - ARM_FRONT + ARM_W / 2:.2f}, round back end", 2, "glued to 1-1/2 square; 1/2 in hole at the round's centre, 10.5 mm hole for the boss bolt"),
    ("stop post", f"3/4 birch ply, 1-1/2 x {POST_H:g}", 2, "glued to 1-1/2 square, screwed to the base"),
    ("V-block", f"hardwood 1-1/2 thick, {V_DEPTH:g} wide, {V_IN[1] - V_IN[0]:.2f} and {V_OUT[1] - V_OUT[0]:.2f} long", 2, f"a 90 degree V {V_TOP:g} wide along the top, cut as one block; screwed down after the first kerf, square to it, with a 1/2 in gap at the disc"),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "chop_saw.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "chop_saw"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Angle grinder chop saw"}]})
    tabs, bodies = {}, {}
    for i, (title, fn) in enumerate(STAND + HEAD):
        tab = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]})
        fn(Part(mcp, tab, title))
        tabs[title] = tab
        bodies[title] = full(mcp, tab)["bodies"]
        print(f"{title:<16} tab {tab:>2}: {', '.join(b['name'] for b in bodies[title])}")

    def instances(parts, fixed):
        return [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}",
                 "fixed": fixed, "placement": IDENTITY} for t, _ in parts for k, b in enumerate(bodies[t])]

    # The head as a rigid sub-assembly, every body where it was drawn.
    head_asm = new_tab(mcp, {"type": "add_assembly", "name": "head assembly"})
    apply(mcp, instances(HEAD, True), head_asm)
    head_ids = {i["name"]: i["id"] for i in full(mcp, head_asm)["instances"]}
    # The saw: the stand fixed, the head on a revolute about the pivot bolt.
    saw = new_tab(mcp, {"type": "add_assembly", "name": "chop saw"})
    ops = instances(STAND, True)
    ops.append({"type": "add_instance", "studio": head_asm, "body": 0, "name": "head", "fixed": False, "placement": IDENTITY})
    apply(mcp, ops, saw)
    ids = {i["name"]: i["id"] for i in full(mcp, saw)["instances"]}
    cb, tb = cylinder(bodies["pivot bolt"][0], (PIVOT_D * IN - 0.2) / 2)
    ca, ta = cylinder(bodies["arm"][0], PIVOT_BORE * IN / 2)
    offset, angle, flip = mate_parameters(tb, ta)
    apply(mcp, [{"type": "add_mate", "kind": "revolute", "a": {"instance": ids["pivot bolt"], "face": cb["reference"]},
                 "b": {"instance": ids["head"], "sub": head_ids["arm"], "face": ca["reference"]},
                 "offset": offset, "angle": angle, "flip": flip, "name": "pivot"}], saw)
    report = full(mcp, saw)
    for k in ("mates", "instances"):
        for m in report[k]:
            if m.get("error"):
                raise RuntimeError(f"{k} {m['name']}: {m['error']}")
    pivot = next(m for m in report["mates"] if m["name"] == "pivot")

    def disc_low(a):
        apply(mcp, [{"type": "set_mate", "id": pivot["id"], "angle": a}], saw)
        return next(b for b in full(mcp, saw)["bodies"] if b["name"] == "head / disc")["bounds"][0]["z"]

    low = disc_low(pivot["angle"])
    sign = 1.0 if disc_low(pivot["angle"] + 5.0) > low else -1.0
    at = lambda deg: pivot["angle"] + sign * deg
    apply(mcp, [{"type": "set_mate", "id": pivot["id"], "angle": at(0.0)}], saw)
    plunge = math.degrees(math.atan2(PIVOT_Z - DISC_Z, PIVOT_Y * IN))
    print(f"rod axis z {ROD_Z:.2f}, disc bottom at the stop z {low:.2f} ({ROD_Z - low:.2f} past the axis), "
          f"spindle z {DISC_Z:.2f}, pivot z {PIVOT_Z:.2f}, the disc plunges {plunge:.1f} degrees off vertical at the stop")
    print(f"gearhead underside z {DISC_Z - GH_HALF:.2f}: {DISC_Z - GH_HALF - (ROD_Z + ROD_D / 2):.1f} over the rod, "
          f"{DISC_Z - GH_HALF - V_TOP_Z:.1f} over the V-blocks; a disc worn to {2 * (DISC_D / 2 - (DISC_Z - GH_HALF - ROD_Z - ROD_D / 2)):.0f} mm puts the gearhead on the rod")

    # Pictures: the saw down on its stop and raised; the swing from the side.
    shot = lambda name, **kw: mcp.call("screenshot", {"tab": saw, "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot("iso", view="-0.55,-0.75,0.45")
    shot("iso_back", view="0.6,0.7,0.45")
    shot("front", view="front")
    shot("side", view="right")
    shot("top", view="top")
    # Across the pivot: cheeks, washers and arm on the bolt.
    shot("pivot_section", view="0,-1,0.0001", section=f"y:{PIVOT_Y * IN}", fit="-40,150,0,150,260,215")
    # The cut, looking along the rod at the disc's plane from the long side.
    window = f"-60,-80,0,120,90,{DISC_Z + 80:.0f}"
    shot("cut_detail", view="-1,-0.25,0.2", fit=window)
    apply(mcp, [{"type": "set_mate", "id": pivot["id"], "angle": at(20.0)}], saw)
    shot("raised", view="-0.55,-0.75,0.45")
    apply(mcp, [{"type": "set_mate", "id": pivot["id"], "angle": at(0.0)}], saw)
    positions = [{"pivot": at(d), "label": "on the stop" if d == 0 else f"up {d:g} degrees"} for d in (0.0, 5.0, 10.0, 20.0)]
    print(mcp.call("range_of_motion", {"tab": saw, "positions": positions, "view": "right", "format": "pdf", "sheet": "A3",
                                       "note": "the arm on its depth stop and raised; the rod in the V-blocks", "path": os.path.join(OUT, "swing.pdf")}))
    mcp.call("range_of_motion", {"tab": saw, "positions": positions, "view": "right", "width": 900, "height": 700, "path": os.path.join(OUT, "swing.png")})
    print(mcp.call("export", {"tab": saw, "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", "iso", f"section-side@{BOSS_X}"],
                              "note": "preliminary: grinder dimensions partly guessed, see README", "path": os.path.join(OUT, "chop_saw.pdf")}))
    with open(os.path.join(OUT, "parts.csv"), "w") as f:
        f.write("kind,item,size,qty,note\n")
        for kind, rows in (("wood", CUTLIST), ("hardware", HARDWARE)):
            for item, size, qty, note in rows:
                f.write(f'{kind},"{item}","{size}",{qty},"{note}"\n')
    print(f"parts: {len(CUTLIST)} wood lines, {len(HARDWARE)} hardware lines")
    mcp.close()


if __name__ == "__main__":
    main()
