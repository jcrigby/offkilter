"""A minimal router table with a scissor-jack lift: a plywood box (top,
two sides, a back and a front across half of it) with the Makita
3612BR plunge router screwed under the top and a 1-1/2 ton Haul-Master
scissor jack bolted upside down beside it. Cranking the jack pushes its
saddle down on one end of a hardwood lever; the lever turns on a bolt
through a hanger under the top, and its other end pushes up on a drop
arm screwed to the router's left handle boss in place of the handle,
which plunges the motor and raises the bit. The jack is cranked by a
socket extension through a slot in the front, with a ratchet or a drill.

The lever is 2 to 1: the jack moves 130 mm for the router's 65 mm of
plunge, so a turn of the crank moves the bit half as far as it moves
the saddle. Hanging the jack from the top with the lever beside the
router keeps the table no taller than the router: a jack under the
router would add its own height. Both ends of the lever are pins (3/8
bolts across it) under flat feet, so nothing is linked: the router's
weight and its plunge springs hold it down on the lever and the lever
up against the saddle.

    cargo build --release -p ok-mcp
    python3 examples/jack-table/build.py         # the document, pictures, sheets, parts list and poses
    cargo test -p ok-render --test jack_table    # what CI runs

Frame: x left to right from the operator (the bit at x = 0, the jack's
bay to the left), y from the front (-y) to the back, z up from the
bench. The drawn pose is the router fully retracted (bit below the
table). Wood is in inches like the shop; the router and the jack are in
millimetres, from the makers' specifications (S), measured (M) or
guessed from the photos (G). README.md here has the table of which.
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
# The router, millimetres, upside down under the top. Depths are below
# the base's mounting face (the top's underside), with the motor fully
# retracted (posts extended, bit lowest).
# ---------------------------------------------------------------------
PLUNGE = 65.0                     # plunge capacity (S: the manual, 0-65 mm)
ROUTER_H = 287.0                  # base face to the motor's end (S: overall height)
BASE_D, BASE_T = 160.0, 18.0      # base casting, clear sub-base off (S: 160 wide; G: thickness)
BASE_HOLE = 50.0                  # the base's centre opening (G)
POST_X, POST_D = 64.0, 20.0       # plunge posts either side of the axis (G)
POST_END = 150.0                  # posts' free ends (G)
YOKE = (85.0, 125.0)              # the motor carriage round the posts, retracted (G)
YOKE_X, YOKE_Y = 85.0, 25.0       # its half-width and half-depth (G)
MOTOR_D = 100.0                   # motor housing below the carriage (G)
BOSS_X = 98.0                     # handle boss faces from the axis (G: 276 across the handles, S, less a 40 mm handle)
BOSS_Z = 105.0                    # handle screw's depth (G)
HANDLE = (40.0, 38.0, 40.0)       # the right handle, kept: out from the boss, front to back, tall (G)
NUT_D, NUT = 24.0, (55.0, 85.0)   # collet nut, up from the carriage (G)
BIT_D, BIT_L = 12.7, 50.0         # 1/2 in straight bit, out of the nut
ROUTER_KG = 5.5                   # (S)

# ---------------------------------------------------------------------
# The jack, millimetres (Haul-Master 1-1/2 ton, Harbor Freight 64521):
# 3-3/8 to 15-1/8 in over the saddle (S), 17 in long, 3-3/4 wide (S).
# ---------------------------------------------------------------------
ARM_L = 170.0                     # scissor arm, pin to pin (G: from 17 in long and 15-1/8 high)
FOOT_PIN = 30.0                   # foot face to its pins (G)
SADDLE_PIN = 30.0                 # saddle face to its pins (G)
PIN_Y = 22.0                      # foot and saddle pins either side of the centre (G)
JACK_W = 95.0                     # across the arms (S: 3-3/4)
JACK_MIN, JACK_MAX = 3.375 * IN, 15.125 * IN
SCREW_D, SCREW_PITCH = 14.0, 2.0  # (G) the lift screw; per turn the knuckles close by one pitch
SCREW_L = 400.0                   # from the front knuckle to its tail (G)
EYE = 50.0                        # front knuckle to the eye's front (G)
SADDLE = (50.0, 60.0, 40.0)       # x, y, z (G)
GROOVE = (22.0, 25.0)             # the saddle's groove (along x, across the screw): wide, deep (G)
HOLE_R = 6.0                      # every pivot hole in the jack's parts

# What this table uses of it: the jack from 140 to 270 mm (the router
# retracted to fully plunged). Above about 90 the jack crawls; this
# band keeps the arms between 14 and 38 degrees, where a turn moves the
# saddle 2.5 to 8 mm and the bit half that.
RATIO = 2.0
JACK_USE = (140.0, 140.0 + RATIO * PLUNGE)

# The drive: a scissor-jack drill adapter on the eye, a 3/8 drive
# extension out through the front.
ADAPTER = (30.0, 40.0)            # diameter, length (G)
EXT_D, EXT_L = 16.0, 6.0 * IN     # 6 in extension

# ---------------------------------------------------------------------
# The box, inches: 3/4 ply.
# ---------------------------------------------------------------------
PLY = 0.75
SIDE_H = 13.0                     # sides' height: the top's underside above the bench
TOP_X, TOP_Y = (-21.0, 7.0), (-12.0, 12.0)   # 28 x 24
LEFT_X, RIGHT_X = (TOP_X[0], TOP_X[0] + PLY), (TOP_X[1] - PLY, TOP_X[1])
BACK_Y, FRONT_Y = (TOP_Y[1] - PLY, TOP_Y[1]), (TOP_Y[0], TOP_Y[0] + PLY)
FRONT_X = (LEFT_X[1], -10.0)      # the partial front: across the jack's bay only
BIT_HOLE = 2.0                    # the top's opening

# The lever: 3/4 x 1-1/2 hardwood, pins on its centre line.
LEVER_T, LEVER_H = 0.75 * IN, 1.5 * IN
PIN_D = 0.375 * IN                # 3/8 bolts
PIN_R = PIN_D / 2
PIVOT_BORE = 10.0
ROUTER_ARM = 110.0                # pivot to the router's pin
PIN_LEN = 50.0                    # contact pins, across the lever

# The drop arm: 1-1/2 x 1-1/2 hardwood in place of the left handle,
# its foot forked over the lever with the tines on the router's pin.
DROP = (38.0, 38.0)               # x (out from the boss), y
FORK = (22.0, 40.0)               # slot width (y), height

# Hanger: two 3/4 ply cheeks 2-1/2 wide either side of the lever, a
# spacer between them at the top.
CHEEK_W = 2.5 * IN
WASHER_T = 1.6

# ---------------------------------------------------------------------
# Derived, millimetres.
# ---------------------------------------------------------------------
TOP_U = SIDE_H * IN                               # top's underside
TOP_S = TOP_U + PLY * IN                          # the table
H_MID = (JACK_USE[0] + JACK_USE[1]) / 2
CONTACT_MID = TOP_U - H_MID                       # both contacts with the lever level
PIVOT_Z = CONTACT_MID - PIN_R
DROP_FOOT = H_MID + PLUNGE / 2                    # drop arm's foot depth, retracted
ROUTER_X = -(BOSS_X + DROP[0] / 2)                # drop arm centre
PIVOT_X = ROUTER_X - ROUTER_ARM
JACK_ARM = RATIO * ROUTER_ARM
JACK_X = PIVOT_X - JACK_ARM
FOOT_PIN_Z = TOP_U - FOOT_PIN

MATERIALS = {"birch": ("3/4 birch ply", 0.68), "hardwood": ("oak", 0.75), "steel": ("steel", 7.85),
             "aluminium": ("aluminium", 2.70)}


def lever_sin(plunge):
    """The lever's rise toward the router (sin of its angle) at a plunge."""
    return (plunge - PLUNGE / 2) / ROUTER_ARM


def jack_height(plunge):
    return H_MID + JACK_ARM * lever_sin(plunge)


def jack_theta(h):
    """The arms' angle from the screw at a jack height (foot face to saddle face)."""
    return math.asin((h - FOOT_PIN - SADDLE_PIN) / (2.0 * ARM_L))


def turns_per_mm(h):
    """Crank turns per millimetre of saddle travel at a jack height."""
    return math.tan(jack_theta(h)) / SCREW_PITCH


def mm(*v):
    return tuple(c * IN for c in v)


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, d = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": d}})
    p.apply(*ops)


def block(p, name, x, y, z, op="new"):
    s = p.sketch("top", z[0], name)
    p.rect(s, (x[0], y[0]), (x[1], y[1]))
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def cyl_x(p, name, x0, x1, y, z, d, op="new"):
    s = p.sketch("right", x0, name)
    p.circle(s, (y, z), d / 2)
    return p.extrude(s, x1 - x0, op=op, name=name)


def cyl_y(p, name, y0, y1, x, z, d, op="new"):
    """A cylinder along y from y0 to y1 (y0 < y1)."""
    s = p.sketch("front", -y1, name)       # the front plane's normal is -y
    p.circle(s, (x, z), d / 2)
    return p.extrude(s, y1 - y0, op=op, name=name)


def cyl_z(p, name, z0, z1, x, y, d, op="new"):
    s = p.sketch("top", z0, name)
    p.circle(s, (x, y), d / 2)
    return p.extrude(s, z1 - z0, op=op, name=name)


def bore_x(p, name, x0, x1, y, z, d):
    s = p.sketch("right", x0 - 1.0, name)
    p.point(s, (y, z))
    p.hole(s, d, depth=x1 - x0 + 2.0, direction="normal", name=name)


def bore_y(p, name, y0, y1, x, z, d):
    s = p.sketch("front", -(y1 + 1.0), name)
    p.point(s, (x, z))
    p.hole(s, d, depth=y1 - y0 + 2.0, direction="normal", name=name)


def bore_z(p, name, z0, z1, x, y, d):
    s = p.sketch("top", z0 - 1.0, name)
    p.point(s, (x, y))
    p.hole(s, d, depth=z1 - z0 + 2.0, direction="normal", name=name)


def profile_x(p, name, x0, x1, pts, op="new"):
    """A polygon in the y-z plane extruded from x0 to x1."""
    s = p.sketch("right", x0, name)
    p.polygon(s, pts)
    return p.extrude(s, x1 - x0, op=op, name=name)


def profile_y(p, name, y0, y1, pts, op="new"):
    """A polygon in the x-z plane extruded from y0 to y1 (y0 < y1)."""
    s = p.sketch("front", -y1, name)
    p.polygon(s, pts)
    return p.extrude(s, y1 - y0, op=op, name=name)


def bar(a, b, half):
    """The rectangle of half-width `half` along a to b (2D)."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dx, dy)
    nx, ny = -dy / n * half, dx / n * half
    return [(a[0] + nx, a[1] + ny), (b[0] + nx, b[1] + ny), (b[0] - nx, b[1] - ny), (a[0] - nx, a[1] - ny)]


# -- the box --------------------------------------------------------------

def top(p):
    f = block(p, "top", mm(*TOP_X), mm(*TOP_Y), (TOP_U, TOP_S))
    bore_z(p, "bit hole", TOP_U, TOP_S, 0.0, 0.0, BIT_HOLE * IN)
    part_is(p, f, f"top, 3/4 ply {TOP_X[1] - TOP_X[0]:g} x {TOP_Y[1] - TOP_Y[0]:g}", "birch")


def side(name, xs):
    def build(p):
        f = block(p, name, mm(*xs), mm(*TOP_Y), (0.0, TOP_U))
        part_is(p, f, f"{name}, 3/4 ply {TOP_Y[1] - TOP_Y[0]:g} x {SIDE_H:g}", "birch")
    return build


def back(p):
    f = block(p, "back", (LEFT_X[1] * IN, RIGHT_X[0] * IN), mm(*BACK_Y), (0.0, TOP_U))
    part_is(p, f, f"back, 3/4 ply {RIGHT_X[0] - LEFT_X[1]:g} x {SIDE_H:g}", "birch")


SLOT = None   # the crank slot in the front: (z low, z high), set from the screw's travel in main()


def front(p):
    f = block(p, "front", mm(*FRONT_X), mm(*FRONT_Y), (0.0, TOP_U))
    lo, hi = SLOT
    s = p.sketch("front", -FRONT_Y[1] * IN - 1.0, "crank slot")
    p.slot(s, (JACK_X, lo), (JACK_X, hi), EXT_D + 8.0)
    p.cut(s, PLY * IN + 2.0, name="crank slot")
    part_is(p, f, f"front, 3/4 ply {FRONT_X[1] - FRONT_X[0]:g} x {SIDE_H:g}, crank slot", "birch")


# -- the router --------------------------------------------------------------

def router_base(p):
    """Base casting and the two posts, screwed under the top."""
    f = cyl_z(p, "router base", TOP_U - BASE_T, TOP_U, 0.0, 0.0, BASE_D)
    bore_z(p, "base opening", TOP_U - BASE_T, TOP_U, 0.0, 0.0, BASE_HOLE)
    for k, x in enumerate((-POST_X, POST_X)):
        cyl_z(p, f"post {k + 1}", TOP_U - POST_END, TOP_U - BASE_T, x, 0.0, POST_D, op="add")
    part_is(p, f, "router base and posts (Makita 3612BR)", "aluminium")


def router_motor(p):
    """The carriage on the posts with the motor below it; the right
    handle stays on."""
    z = lambda d: TOP_U - d
    f = block(p, "carriage", (-YOKE_X, YOKE_X), (-YOKE_Y, YOKE_Y), (z(YOKE[1]), z(YOKE[0])))
    for k, x in enumerate((-POST_X, POST_X)):
        bore_z(p, f"post bore {k + 1}", z(YOKE[1]), z(YOKE[0]), x, 0.0, POST_D + 1.0)
    cyl_z(p, "motor", z(ROUTER_H), z(YOKE[1]), 0.0, 0.0, MOTOR_D, op="add")
    for name, sx in (("left boss", -1.0), ("right boss", 1.0)):
        x = (YOKE_X, BOSS_X) if sx > 0 else (-BOSS_X, -YOKE_X)
        block(p, name, x, (-15.0, 15.0), (z(BOSS_Z + 15.0), z(BOSS_Z - 15.0)), op="add")
    block(p, "right handle", (BOSS_X, BOSS_X + HANDLE[0]), (-HANDLE[1] / 2, HANDLE[1] / 2),
          (z(BOSS_Z + HANDLE[2] / 2), z(BOSS_Z - HANDLE[2] / 2)), op="add")
    cyl_z(p, "collet nut", z(YOKE[0]), z(NUT[0]), 0.0, 0.0, NUT_D, op="add")
    part_is(p, f, "router motor (Makita 3612BR), right handle on", "aluminium")


def bit(p):
    f = cyl_z(p, "bit", TOP_U - NUT[0], TOP_U - NUT[0] + BIT_L, 0.0, 0.0, BIT_D)
    part_is(p, f, "bit, 1/2 in straight", "steel")


def drop_arm(p):
    """In place of the left handle, on its screw: hardwood down beside
    the motor to a fork over the lever."""
    x = (-BOSS_X - DROP[0], -BOSS_X)
    y = (-DROP[1] / 2, DROP[1] / 2)
    top_z, foot_z = TOP_U - (BOSS_Z - HANDLE[2] / 2), TOP_U - DROP_FOOT
    w, h = FORK
    pts = [(y[0], foot_z), (-w / 2, foot_z), (-w / 2, foot_z + h), (w / 2, foot_z + h), (w / 2, foot_z),
           (y[1], foot_z), (y[1], top_z), (y[0], top_z)]
    f = profile_x(p, "drop arm", x[0], x[1], pts)
    bore_x(p, "handle screw", x[0], x[1], 0.0, TOP_U - BOSS_Z, 6.5)
    part_is(p, f, f"drop arm, hardwood {DROP[0] / IN:.1f} x {DROP[1] / IN:.1f} x {(top_z - foot_z) / IN:.2f}, forked foot", "hardwood")


# -- the lever and its hanger -------------------------------------------------

def lever_points(s):
    """Pivot, router pin and jack pin (x, z) with the lever at sin s."""
    c = math.sqrt(1.0 - s * s)
    return ((PIVOT_X, PIVOT_Z), (PIVOT_X + ROUTER_ARM * c, PIVOT_Z + ROUTER_ARM * s),
            (PIVOT_X - JACK_ARM * c, PIVOT_Z - JACK_ARM * s))


LEVER_PAST = 22.0     # lever past each pin


def lever(p):
    s0 = lever_sin(0.0)
    piv, rp, jp = lever_points(s0)
    ux, uz = (rp[0] - jp[0]) / (ROUTER_ARM + JACK_ARM), (rp[1] - jp[1]) / (ROUTER_ARM + JACK_ARM)
    a = (jp[0] - ux * LEVER_PAST, jp[1] - uz * LEVER_PAST)
    b = (rp[0] + ux * LEVER_PAST, rp[1] + uz * LEVER_PAST)
    f = profile_y(p, "lever", -LEVER_T / 2, LEVER_T / 2, bar(a, b, LEVER_H / 2))
    bore_y(p, "pivot bore", -LEVER_T / 2, LEVER_T / 2, piv[0], piv[1], PIVOT_BORE)
    for name, c in (("router pin", rp), ("jack pin", jp)):
        cyl_y(p, name, -PIN_LEN / 2, PIN_LEN / 2, c[0], c[1], PIN_D, op="add")
    length = ROUTER_ARM + JACK_ARM + 2 * LEVER_PAST
    part_is(p, f, f"lever, hardwood 3/4 x 1-1/2 x {length / IN:.2f}, two 3/8 x 2 bolts", "hardwood")


CHEEK_Y = LEVER_T / 2 + WASHER_T      # cheeks' inner faces


def cheek(name, sy):
    def build(p):
        y = (CHEEK_Y, CHEEK_Y + PLY * IN) if sy > 0 else (-CHEEK_Y - PLY * IN, -CHEEK_Y)
        x = (PIVOT_X - CHEEK_W / 2, PIVOT_X + CHEEK_W / 2)
        f = block(p, name, x, y, (PIVOT_Z, TOP_U))
        cyl_y(p, f"{name} round", y[0], y[1], PIVOT_X, PIVOT_Z, CHEEK_W, op="add")
        bore_y(p, f"{name} bore", y[0], y[1], PIVOT_X, PIVOT_Z, PIVOT_BORE)
        part_is(p, f, f"{name}, 3/4 ply {CHEEK_W / IN:g} x {(TOP_U - PIVOT_Z + CHEEK_W / 2) / IN:.2f}, round end", "birch")
    return build


def spacer(p):
    f = block(p, "hanger spacer", (PIVOT_X - CHEEK_W / 2, PIVOT_X + CHEEK_W / 2), (-CHEEK_Y, CHEEK_Y), (TOP_U - 1.5 * IN, TOP_U))
    part_is(p, f, "hanger spacer, hardwood 2-1/2 x 1-1/2", "hardwood")


def pivot_bolt(p):
    y = CHEEK_Y + PLY * IN
    f = cyl_y(p, "pivot bolt", -y - 2.0, y + 8.0, PIVOT_X, PIVOT_Z, PIN_D)
    cyl_y(p, "pivot bolt head", -y - 8.0, -y - 2.0, PIVOT_X, PIVOT_Z, 14.3, op="add")
    cyl_y(p, "pivot nut", y + 2.0, y + 8.0, PIVOT_X, PIVOT_Z, 14.3, op="add")
    part_is(p, f, "pivot bolt, 3/8-16 x 2-1/2 hex, nylock", "steel")


# -- the jack, upside down: foot on the top, saddle hanging -----------------

def knuckle(theta, sy):
    return (sy * (PIN_Y + ARM_L * math.cos(theta)), FOOT_PIN_Z - ARM_L * math.sin(theta))


def foot_pin(sy):
    return (sy * PIN_Y, FOOT_PIN_Z)


def saddle_pin(theta, sy):
    return (sy * PIN_Y, FOOT_PIN_Z - 2.0 * ARM_L * math.sin(theta))


THETA0 = None   # set in main(): the arms' angle in the drawn pose


def jack_foot(p):
    x = (JACK_X - 30.0, JACK_X + 30.0)
    f = block(p, "jack foot", x, (-60.0, 60.0), (TOP_U - 8.0, TOP_U))
    profile_x(p, "foot ears", x[0], x[1], [(-PIN_Y - 12.0, TOP_U - 8.0), (PIN_Y + 12.0, TOP_U - 8.0),
                                            (PIN_Y + 12.0, FOOT_PIN_Z - 12.0), (-PIN_Y - 12.0, FOOT_PIN_Z - 12.0)], op="add")
    for k, sy in enumerate((-1.0, 1.0)):
        bore_x(p, f"foot pin {k + 1}", x[0], x[1], sy * PIN_Y, FOOT_PIN_Z, 2 * HOLE_R)
    for k, sy in enumerate((-1.0, 1.0)):
        bore_z(p, f"foot bolt {k + 1}", TOP_U - 8.0, TOP_U, JACK_X, sy * 48.0, 8.5)
    part_is(p, f, "jack foot (Haul-Master 1-1/2 ton)", "steel")


def jack_arm(name, a_end, b_end, width):
    """A scissor arm between two pins (y, z) in the drawn pose."""
    def build(p):
        a, b = a_end(THETA0), b_end(THETA0)
        x = (JACK_X - width / 2, JACK_X + width / 2)
        f = profile_x(p, name, x[0], x[1], bar(a, b, 10.0))
        for k, c in enumerate((a, b)):
            cyl_x(p, f"{name} end {k + 1}", x[0], x[1], c[0], c[1], 20.0, op="add")
            bore_x(p, f"{name} hole {k + 1}", x[0], x[1], c[0], c[1], 2 * HOLE_R)
        part_is(p, f, f"jack {name}", "steel")
    return build


def saddle(p):
    sp = saddle_pin(THETA0, 1.0)
    face = sp[1] - SADDLE_PIN
    x = (JACK_X - SADDLE[0] / 2, JACK_X + SADDLE[0] / 2)
    w, d = GROOVE
    y = SADDLE[1] / 2
    pts = [(-y, face), (-w / 2, face), (-w / 2, face + d), (w / 2, face + d), (w / 2, face), (y, face),
           (y, face + SADDLE[2]), (-y, face + SADDLE[2])]
    f = profile_x(p, "saddle", x[0], x[1], pts)
    for k, sy in enumerate((-1.0, 1.0)):
        bore_x(p, f"saddle pin {k + 1}", x[0], x[1], sy * PIN_Y, sp[1], 2 * HOLE_R)
    part_is(p, f, "jack saddle", "steel")


def screw(p):
    """The lift screw with its front trunnion, the eye, the drill
    adapter and the extension out through the front."""
    ky, kz = knuckle(THETA0, -1.0)
    x = (JACK_X - 20.0, JACK_X + 20.0)
    f = cyl_x(p, "front trunnion", x[0], x[1], ky, kz, 26.0)
    bore_x(p, "front trunnion hole", x[0], x[1], ky, kz, 2 * HOLE_R)
    cyl_y(p, "lift screw", ky + 10.0, ky + SCREW_L, JACK_X, kz, SCREW_D, op="add")
    cyl_y(p, "eye", ky - EYE, ky - 10.0, JACK_X, kz, 22.0, op="add")
    cyl_y(p, "drill adapter", ky - EYE - ADAPTER[1], ky - EYE, JACK_X, kz, ADAPTER[0], op="add")
    cyl_y(p, "extension", ky - EYE - ADAPTER[1] - EXT_L, ky - EYE - ADAPTER[1], JACK_X, kz, EXT_D, op="add")
    part_is(p, f, "jack screw, eye, drill adapter and 3/8 x 6 extension", "steel")


def nut_trunnion(p):
    ky, kz = knuckle(THETA0, 1.0)
    x = (JACK_X - 20.0, JACK_X + 20.0)
    f = cyl_x(p, "rear trunnion", x[0], x[1], ky, kz, 26.0)
    bore_x(p, "rear trunnion hole", x[0], x[1], ky, kz, 2 * HOLE_R)
    part_is(p, f, "jack nut trunnion", "steel")


# -- mates ---------------------------------------------------------------------

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


def cylinder(body, r, near):
    """The cylinder of radius r on a body whose axis passes nearest the
    world point `near`, with its frame (bodies are built in place)."""
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    best = None
    for c in body["cylinders"]:
        if abs(c["radius"] - r) > 1e-6:
            continue
        o, z = _v(c["origin"]), _norm(_v(c["axis"]))
        d = _sub(near, o)
        t = _dot(d, z)
        off = _sub(d, (z[0] * t, z[1] * t, z[2] * t))
        dist = math.sqrt(_dot(off, off))
        if best is None or dist < best[0]:
            best = (dist, c)
    if best is None or best[0] > 1.0:
        raise RuntimeError(f"{body['name']}: no cylinder of radius {r} near {near}")
    c = best[1]
    return c, _frame(c, faces[json.dumps(c["reference"], sort_keys=True)])


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


def direction(a, b):
    return math.atan2(b[1] - a[1], b[0] - a[0])


def jack_turns(theta):
    """World rotations about +x (radians, from the drawn pose) of the
    jack's moving parts at arm angle theta, in the y-z plane."""
    def arms(t):
        return {
            "upper front arm": direction(foot_pin(-1.0), knuckle(t, -1.0)),
            "upper rear arm": direction(foot_pin(1.0), knuckle(t, 1.0)),
            "lower front arm": direction(knuckle(t, -1.0), saddle_pin(t, -1.0)),
            "lower rear arm": direction(saddle_pin(t, 1.0), knuckle(t, 1.0)),
        }
    now, then = arms(theta), arms(THETA0)
    w = {k: now[k] - then[k] for k in now}
    w["saddle"] = w["screw"] = w["nut trunnion"] = 0.0
    return w


# mate name: (parent instance, moving instance, the pin both share as a
# function of theta, or None for mates set otherwise)
JACK_MATES = [
    ("upper front", "jack foot", "upper front arm", lambda t: foot_pin(-1.0)),
    ("upper rear", "jack foot", "upper rear arm", lambda t: foot_pin(1.0)),
    ("lower front", "upper front arm", "lower front arm", lambda t: knuckle(t, -1.0)),
    ("saddle", "lower front arm", "saddle", lambda t: saddle_pin(t, -1.0)),
    ("lower rear", "saddle", "lower rear arm", lambda t: saddle_pin(t, 1.0)),
    ("screw", "upper front arm", "screw", lambda t: knuckle(t, -1.0)),
    ("nut", "upper rear arm", "nut trunnion", lambda t: knuckle(t, 1.0)),
]

BOX = [("top", top), ("left side", side("left side", LEFT_X)), ("right side", side("right side", RIGHT_X)),
       ("back", back), ("front", front), ("left cheek", cheek("left cheek", -1.0)), ("right cheek", cheek("right cheek", 1.0)),
       ("hanger spacer", spacer), ("pivot bolt", pivot_bolt), ("router base", router_base), ("jack foot", jack_foot)]
MOTOR = [("router motor", router_motor), ("bit", bit), ("drop arm", drop_arm)]
MOVING = [("lever", lever), ("upper front arm", None), ("upper rear arm", None), ("lower front arm", None),
          ("lower rear arm", None), ("saddle", saddle), ("screw", screw), ("nut trunnion", nut_trunnion)]


def jack_arm_builders():
    return {
        "upper front arm": jack_arm("upper front arm", lambda t: foot_pin(-1.0), lambda t: knuckle(t, -1.0), JACK_W - 10.0),
        "upper rear arm": jack_arm("upper rear arm", lambda t: foot_pin(1.0), lambda t: knuckle(t, 1.0), JACK_W - 10.0),
        "lower front arm": jack_arm("lower front arm", lambda t: knuckle(t, -1.0), lambda t: saddle_pin(t, -1.0), JACK_W - 30.0),
        "lower rear arm": jack_arm("lower rear arm", lambda t: saddle_pin(t, 1.0), lambda t: knuckle(t, 1.0), JACK_W - 30.0),
    }


HARDWARE = [
    ("scissor jack", "Haul-Master 1-1/2 ton (Harbor Freight 64521)", 1, "upside down, its foot bolted under the top through two 5/16 holes drilled in the foot's flat; its own crank stays in the drawer"),
    ("jack bolts", "5/16-18 x 1-1/2 flat head, washers, nylocks", 2, "countersunk flush in the table, nuts under the jack's foot"),
    ("drill adapter", "scissor-jack drill adapter for the eye, 3/8 drive or 1/2 hex", 1, "on the jack's eye"),
    ("extension", "3/8 drive x 6 in", 1, "from the adapter out through the crank slot; a ratchet or a drill on its end"),
    ("pivot bolt", "3/8-16 x 2-1/2 hex bolt, nylock, 2 washers", 1, "through the cheeks and the lever, a washer each side of the lever; snug, not tight"),
    ("contact pins", "3/8-16 x 2 hex bolts, nuts", 2, "across the lever 110 mm and 220 mm either side of the pivot, on its centre line; the drop arm's fork and the saddle's lips ride on their ends"),
    ("drop arm screw", "the handle's own screw, or one 3/4 in longer of the same thread", 1, "through the drop arm into the left handle boss"),
    ("router screws", "the sub-base screws, longer, flat head", 3, "through the top, countersunk, into the base where the clear sub-base was"),
]

CUTLIST = [
    ("top", f"3/4 ply, {TOP_X[1] - TOP_X[0]:g} x {TOP_Y[1] - TOP_Y[0]:g}", 1, f"{BIT_HOLE:g} in hole over the bit, 7 in from the right edge; three countersunk holes for the router's base; two for the jack's foot"),
    ("side", f"3/4 ply, {TOP_Y[1] - TOP_Y[0]:g} x {SIDE_H:g}", 2, "under the top's ends, glued and screwed"),
    ("back", f"3/4 ply, {RIGHT_X[0] - LEFT_X[1]:g} x {SIDE_H:g}", 1, "between the sides"),
    ("front", f"3/4 ply, {FRONT_X[1] - FRONT_X[0]:g} x {SIDE_H:g}", 1, "across the jack's bay from the left side; the crank slot in it; the router's bay stays open for the switch, the plunge lock and the cord"),
    ("cheek", f"3/4 ply, {CHEEK_W / IN:g} x {(TOP_U - PIVOT_Z + CHEEK_W / 2) / IN:.2f}, round end", 2, "the hanger, screwed down through the top with the spacer between them; 3/8 hole at the round's centre"),
    ("hanger spacer", "hardwood, 2-1/2 x 1-1/2 x 7/8 (the lever's thickness and two washers)", 1, "between the cheeks at the top"),
    ("lever", f"hardwood, 3/4 x 1-1/2 x {(ROUTER_ARM + JACK_ARM + 2 * LEVER_PAST) / IN:.2f}", 1, "3/8 holes on the centre line: the pivot, the router pin 110 mm one way, the jack pin 220 mm the other"),
    ("drop arm", f"hardwood, 1-1/2 x 1-1/2 x {(DROP_FOOT - BOSS_Z + HANDLE[2] / 2) / IN:.2f}", 1, f"a hole for the handle screw {HANDLE[2] / 2:.0f} mm from the top; a 7/8 wide, 1-1/2 deep slot up the foot for the lever"),
]


def main():
    global THETA0, SLOT
    THETA0 = jack_theta(jack_height(0.0))
    # The screw's axis drops by half the saddle's travel: the slot spans it.
    k_lo, k_hi = knuckle(jack_theta(jack_height(PLUNGE)), -1.0)[1], knuckle(THETA0, -1.0)[1]
    SLOT = (k_lo, k_hi)
    for h in JACK_USE:
        if not JACK_MIN < h < JACK_MAX:
            raise RuntimeError(f"jack height {h} outside {JACK_MIN}-{JACK_MAX}")

    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "jack_table.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "jack_table"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Router table, scissor-jack lift"}]})
    arms = jack_arm_builders()
    tabs, bodies = {}, {}
    for i, (title, fn) in enumerate(BOX + MOTOR + MOVING):
        fn = fn or arms[title]
        tab = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]})
        fn(Part(mcp, tab, title))
        tabs[title] = tab
        bodies[title] = full(mcp, tab)["bodies"]
        print(f"{title:<16} tab {tab:>2}: {', '.join(b['name'] for b in bodies[title])}")

    def instances(parts, fixed):
        return [{"type": "add_instance", "studio": tabs[t], "body": 0, "name": t, "fixed": fixed, "placement": IDENTITY}
                for t, _ in parts]

    motor_asm = new_tab(mcp, {"type": "add_assembly", "name": "router motor assembly"})
    apply(mcp, instances(MOTOR, True), motor_asm)
    motor_ids = {i["name"]: i["id"] for i in full(mcp, motor_asm)["instances"]}

    table = new_tab(mcp, {"type": "add_assembly", "name": "router table"})
    ops = instances(BOX, True) + instances(MOVING, False)
    ops.append({"type": "add_instance", "studio": motor_asm, "body": 0, "name": "router motor", "fixed": False, "placement": IDENTITY})
    apply(mcp, ops, table)
    ids = {i["name"]: i["id"] for i in full(mcp, table)["instances"]}

    def mate(kind, name, a, b, r_a, r_b, near, sub=None):
        ca, fa = cylinder(bodies[a][0], r_a, near)
        bb = bodies["router motor"][0] if sub else bodies[b][0]
        cb, fb = cylinder(bb, r_b, near)
        offset, angle, flip = mate_parameters(fa, fb)
        conn_b = {"instance": ids[b], "face": cb["reference"]}
        if sub:
            conn_b["sub"] = motor_ids[sub]
        apply(mcp, [{"type": "add_mate", "kind": kind, "a": {"instance": ids[a], "face": ca["reference"]}, "b": conn_b,
                     "offset": offset, "angle": angle, "flip": flip, "name": name}], table)

    mate("slider", "plunge", "router base", "router motor", POST_D / 2, (POST_D + 1.0) / 2, (-POST_X, 0.0, TOP_U - YOKE[0]),
         sub="router motor")
    mate("revolute", "lever", "pivot bolt", "lever", PIN_R, PIVOT_BORE / 2, (PIVOT_X, 0.0, PIVOT_Z))
    for name, a, b, pin in JACK_MATES:
        y, z = pin(THETA0)
        mate("revolute", name, a, b, HOLE_R, HOLE_R, (JACK_X, y, z))
    report = full(mcp, table)
    for k in ("mates", "instances"):
        for m in report[k]:
            if m.get("error"):
                raise RuntimeError(f"{k} {m['name']}: {m['error']}")
    mates = {m["name"]: m for m in report["mates"]}

    def placed(rep, name):
        return next(i for i in rep["instances"] if i["name"] == name)["placed"]

    def set_mates(values):
        apply(mcp, [{"type": "set_mate", "id": mates[n]["id"], "angle": a, "offset": o} for n, (a, o) in values.items()], table)

    # Which way each mate's value turns or moves its part: probe each one
    # by a small step and read the moving instance's pose. The jack's
    # parts turn about x; the lever about y, where raising its router
    # end (+x toward +z) is a negative turn.
    rest = {n: (m["angle"], m["offset"]) for n, m in mates.items()}
    sign = {}
    for name in mates:
        a, o = rest[name]
        if name == "plunge":
            set_mates({name: (a, o + 1.0)})
            sign[name] = 1.0 if placed(full(mcp, table), "router motor")["position"]["z"] > 0.0 else -1.0
        elif name == "lever":
            set_mates({name: (a + 1.0, o)})
            sign[name] = 1.0 if placed(full(mcp, table), "lever")["rotation"]["y"] < 0.0 else -1.0
        else:
            child = next(b for n, _, b, _ in JACK_MATES if n == name)
            set_mates({name: (a + 1.0, o)})
            sign[name] = 1.0 if placed(full(mcp, table), child)["rotation"]["x"] > 0.0 else -1.0
        set_mates({name: rest[name]})

    def pose(plunge):
        """Every mate's (angle, offset) with the router plunged this far."""
        rise = math.degrees(math.asin(lever_sin(plunge)) - math.asin(lever_sin(0.0)))
        w = jack_turns(jack_theta(jack_height(plunge)))
        out = {"plunge": (rest["plunge"][0], rest["plunge"][1] + sign["plunge"] * plunge),
               "lever": (rest["lever"][0] + sign["lever"] * rise, rest["lever"][1])}
        for name, parent, child, _ in JACK_MATES:
            rel = w[child] - w.get(parent, 0.0)
            out[name] = (rest[name][0] + sign[name] * math.degrees(rel), rest[name][1])
        return out

    positions = [0.0, PLUNGE / 4, PLUNGE / 2, 3 * PLUNGE / 4, PLUNGE]
    poses = []
    for p in positions:
        values = pose(p)
        h = jack_height(p)
        poses.append({"plunge": p, "jack": h, "theta": math.degrees(jack_theta(h)),
                      "turns_per_mm_bit": RATIO * turns_per_mm(h), "mates": {n: {"angle": a, "offset": o} for n, (a, o) in values.items()}})
    set_mates(pose(0.0))
    report = full(mcp, table)
    for m in report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")

    total_turns = 0.0
    steps = 200
    for k in range(steps):
        h = JACK_USE[0] + (JACK_USE[1] - JACK_USE[0]) * (k + 0.5) / steps
        total_turns += turns_per_mm(h) * (JACK_USE[1] - JACK_USE[0]) / steps
    with open(os.path.join(OUT, "poses.json"), "w") as f:
        json.dump({"positions": poses, "ratio": RATIO, "pin_r": PIN_R, "total_turns": total_turns,
                   "screw_pitch": SCREW_PITCH}, f, indent=1)

    print(f"table {TOP_S / IN:.2f} in high; top underside z {TOP_U:.1f}; pivot ({PIVOT_X:.1f}, {PIVOT_Z:.1f}); "
          f"router pin x {ROUTER_X:.1f}, jack x {JACK_X:.1f}")
    for p in poses:
        print(f"plunge {p['plunge']:5.1f}: jack {p['jack']:5.1f} mm ({p['jack'] / IN:.2f} in), arms {p['theta']:4.1f} deg, "
              f"{p['turns_per_mm_bit']:.2f} turns per mm of bit ({1.0 / p['turns_per_mm_bit']:.2f} mm a turn)")
    print(f"{total_turns:.0f} turns of the crank from retracted to fully plunged; crank slot z {SLOT[0]:.1f} to {SLOT[1]:.1f}")

    # Pictures.
    shot = lambda name, **kw: mcp.call("screenshot", {"tab": table, "width": 1600, "height": 1100,
                                                      "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot("iso", view="-0.55,-0.75,0.5")
    shot("iso_under", view="-0.5,-0.8,-0.25")
    shot("front", view="front")
    shot("top", view="top")
    # Through the lever (y = 0) and through the jack (x at its centre),
    # retracted and fully plunged.
    for tag, p in (("retracted", 0.0), ("plunged", PLUNGE)):
        set_mates(pose(p))
        shot(f"section_{tag}", view="0,-1,0.0001", section="y:0")
        shot(f"jack_{tag}", view="-1,0,0.0001", section=f"x:{JACK_X:.1f}", fit=f"{JACK_X - 60:.0f},-320,0,{JACK_X + 60:.0f},320,{TOP_S + 10:.0f}")
    shot("iso_plunged", view="-0.55,-0.75,0.5")
    set_mates(pose(0.0))
    rom = [{**{n: {"angle": v["angle"], "offset": v["offset"]} for n, v in p["mates"].items()},
            "label": f"plunged {p['plunge']:g} mm, jack {p['jack'] / IN:.2f} in"} for p in poses[::2]]
    print(mcp.call("range_of_motion", {"tab": table, "positions": rom, "view": "front", "format": "pdf", "sheet": "A3",
                                       "note": "section at the lever: the jack's saddle down, the lever's far end up, the router plunged",
                                       "path": os.path.join(OUT, "lift.pdf")}))
    mcp.call("range_of_motion", {"tab": table, "positions": rom, "view": "front", "width": 900, "height": 700,
                                 "path": os.path.join(OUT, "lift.png")})
    print(mcp.call("export", {"tab": table, "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", "iso", "section@0"],
                              "note": "preliminary: router and jack dimensions partly guessed, see README",
                              "path": os.path.join(OUT, "jack_table.pdf")}))
    with open(os.path.join(OUT, "parts.csv"), "w") as f:
        f.write("kind,item,size,qty,note\n")
        for kind, rows in (("wood", CUTLIST), ("hardware", HARDWARE)):
            for item, size, qty, note in rows:
                f.write(f'{kind},"{item}","{size}",{qty},"{note}"\n')
    mcp.close()


if __name__ == "__main__":
    main()
