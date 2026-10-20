"""A sliding cutoff grinder: a 4-1/2 in angle grinder on a carriage that
slides along a hardwood rail and pivots up and down on a plywood disc,
over a steel channel let into a three-layer plywood base, with a steel
angle fence in two positions. It is an adaptation of Woodsmith plan
WS22134 to the shop's own grinder, the one the chop saw carries; the
plan's dimensions are kept where they fit it, and README.md says what
was changed and why.

    cargo build --release -p ok-mcp
    python3 examples/cutoff-grinder/build.py       # the document, pictures, sheets and parts list
    cargo test -p ok-render --test cutoff_grinder  # what CI runs

Frame: x across the base from its left edge (the support arm and the
handle on the left, the grinder body in the middle, the wheel at the
right end of the grinder), y from the front edge (the operator) to the
back, z up from the bench. Wood is in inches like the plan; the grinder
is in millimetres as measured. The drawn pose is the carriage at the
middle of its travel with the handle at rest, the pivot plate level.
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
# The grinder, millimetres, as in examples/chop-saw (M measured, G guessed).
# ---------------------------------------------------------------------
DISC_D = 114.3          # 4-1/2 in cut-off wheel, new (M)
DISC_T = 1.0            # 0.040 in thin metal cut-off
DISC_BORE = 22.23       # 7/8 in arbor hole
SPINDLE_D = 15.9        # 5/8-11 (M)
NUT_D, NUT_T = 30.0, 6.0          # outer locking nut (G)
COLLAR_D = 40.0                   # bearing collar round the spindle (G)
GH_X = (28.0, 75.0)               # gearhead along the spindle from the wheel's inner face (M)
GH_FWD, GH_BACK = 57.0, 28.0      # gearhead from the spindle centre toward the body, and the nose past it (G)
GH_HALF = 37.0                    # gearhead half-height about the spindle axis (M)
BODY_X = 52.0                     # the body's axis from the wheel's inner face along the spindle (M: the side-handle boss)
BODY_D, BODY_L = 62.0, 183.0      # motor body diameter and length past the gearhead (G)
GUARD_R, GUARD_T = 62.5, 1.5      # the guard's rim radius and sheet thickness (G)
GUARD_X = (-10.0, 4.5)            # rim across the wheel, back plate on the gearhead side (G)

# ---------------------------------------------------------------------
# The woodwork, inches. Plan values unless the comment says otherwise.
# ---------------------------------------------------------------------
PLY = 0.75
BASE_W, BASE_D, BASE_T = 23.0, 21.0, 3 * PLY         # three layers
BASE_R = 1.0                                          # front corner radius
TAB_X, TAB_D = (0.5, 6.5), 1.0                        # the tab on the back edge the support arm's foot sits on
ARM_L, ARM_H, ARM_T = 14.0, 6.0, 2 * PLY              # main support arm B: two layers, 6 tall, 14 long
ARM_Y = (BASE_D + TAB_D - ARM_L, BASE_D + TAB_D)      # along y, its back end on the tab's edge
FOOT_L = 4.0                                          # the full-height foot at the back
BAR_H = 1.75                                          # the arm's bar past the foot
BAR_R, BAR_NOSE_R = 1.25, 0.25                        # the inside corner under the bar, the bar's front corner
SMALL_L, SMALL_H, SMALL_FRONT = 10.0, 6.0, 2.5        # small support arm C: 10 long, 6 at the back, tapering (the front height is a guess)
BLOCK_W, BLOCK_H = 2.25, 3.625                        # support blocks D, two end to end
RAIL_L, RAIL_H, RAIL_T = 21.0, 2.5, 1.0               # rail E, 1 in hardwood
RAIL_DROP = 0.75                                      # the rail's top below the arm's top
SLIDE_T, SLIDE_W, SLIDE_OUT = 0.25, 0.75, 5.0 / 16    # resin slides in the rail's edges, standing 5/16 proud
GUIDE = 1.0                                           # guides H: 1 x 1 hardwood
GUIDE_L = 5.0
GUIDE_GROOVE = (0.375, 0.375)                         # the guide's groove over the slide: width, depth
PLATE_L, PLATE_H = 8.75, 4.75                         # carriage plate F
PLATE_CHAMFER = 2.375                                 # its lower corners, 45 degrees
SLOT_W, SLOT_DEEP = 0.5, 2.375                        # the stop slots in its ends, open at the end
PIN_D, PIN_L, PIN_IN = 0.25, 1.0, 0.625               # steel pins in the rail, this much in the rail
PIN_Y = (8.25, 17.125)                                # the pins along the rail: 4-5/8 in of travel, the pivot plate never over the fence's leg
DISC_PLY_D = 3.0                                      # disc I, 1/4 ply
RING_OD, RING_ID = 4.4375, 3.0                        # ring N, 1/4 ply
STOP_L, STOP_H = 1.875, 2.0                           # stop G (adapted: shorter, behind the pivot plate)
PIVOT_PLATE = (4.75, 5.625)                           # pivot plate J: along y, tall
PIVOT_UP = 3.125                                      # the pivot above the plate's bottom edge
SPACER = (3.125, 1.75)                                # spacer block L: along y, tall
MOUNT = (1.375, 4.5)                                  # mounting blocks M: along y, tall
MOUNT_BITE = 0.5                                      # how deep the body sits in their cutouts
MOUNT_Y = (3.75, 5.75)                                # the blocks behind the spindle, along the body, on the handle's tab
PIVOT_BOLT_D, PIVOT_BOLT_L = 0.375, 2.0               # 3/8-16 x 2 carriage bolt
CARRIAGE_BOLT_D, CARRIAGE_BOLT_L = 0.5, 8.0           # 1/2-13 x 8 carriage bolts, the arm to the base
CHANNEL = (1.0, 0.5, 0.125, 17.0)                     # 1/2 x 1 x 1/8 steel U-channel, 17 long, from the front edge
CHANNEL_DADO = 0.625                                  # its dado's depth; the channel's top 1/8 below the surface
INSERT_W, INSERT_T = 4.0, 0.125                       # steel inserts either side, in 1/8 rabbets, flush
FENCE = (1.0, 0.125)                                  # 1 x 1 x 1/8 steel angle
FENCE_GAP = 2.5                                       # between the two fence halves, at the wheel
FENCE_NOTCH = 2.5                                     # the left fence's vertical leg cut away for this length at the gap, where the gearhead comes down
FENCE_ROWS = (5.875, 7.875)                           # the fence's two positions, its front face along y; both ahead of the pivot plate's reach
INSERT_X = (1.5, 4.0, 17.0, 21.0)                     # threaded inserts along a row, two each side, clear of the steel inserts and the pivot plate
PEG_HOLE = (0.375, 1.0)                               # 3/8 holes, 1 deep, for dowel pegs
PEG_AT = ((19.0, 3.0), (21.5, 3.0))
WASHER_T = 1.6 / IN

# The handle K, from the plan's full-size pattern (page 10 of WS22134),
# traced from the vector drawing and turned so the tab's bottom edge,
# which sits on the spacer block, is level: u along y (negative is
# forward, toward the operator), v up from that edge. The grip leans
# forward over the wheel. Inches.
HANDLE = [
    (0.000, 0.000), (2.999, -0.000), (3.001, 1.242), (3.750, 1.242), (3.753, 2.570), (0.231, 6.709),
    (0.100, 6.818), (-0.049, 6.895), (-0.210, 6.940), (-0.377, 6.955), (-0.544, 6.942), (-0.706, 6.902),
    (-4.372, 5.568), (-4.503, 5.507), (-4.619, 5.436), (-4.722, 5.353), (-4.810, 5.260), (-4.884, 5.154),
    (-4.944, 5.038), (-4.987, 4.907), (-5.015, 4.781), (-5.029, 4.660), (-5.027, 4.545), (-5.011, 4.434),
    (-4.979, 4.329), (-4.390, 2.711), (-4.340, 2.590), (-4.280, 2.479), (-4.214, 2.379), (-4.145, 2.294),
    (-4.077, 2.226), (-4.013, 2.175), (-3.937, 2.133), (-3.846, 2.095), (-3.746, 2.063), (-3.642, 2.040),
    (-3.540, 2.026), (-3.446, 2.023), (-1.500, 2.111), (-1.284, 2.086), (-1.102, 2.003), (-0.955, 1.870),
    (-0.847, 1.694), (-0.779, 1.480), (-0.756, 1.237), (-0.000, 1.240),
]
HAND_HOLD = ((-2.973, 3.754), (-0.507, 4.650), 1.375)   # the two Forstner holes joined into the hand-hold
HANDLE_TAB = 3.0                                        # the tab's bottom edge, on the spacer block

# ---------------------------------------------------------------------
# Where everything sits: the stack across the base, left to right.
# ---------------------------------------------------------------------
BASE_TOP = BASE_T
X_BLOCK = TAB_X[0]                                   # support blocks D on the far side
X_SMALL = X_BLOCK + PLY                              # small arm C
X_ARM = X_SMALL + PLY                                # main arm B
X_RAIL = X_ARM + ARM_T                               # the rail on the arm's right face; the guides straddle it
X_PLATE = X_RAIL + RAIL_T                            # carriage plate F on the guides
X_DISC = X_PLATE + PLY                               # the disc on the plate, the ring round it, same layer
X_PIVOT_PLATE = X_DISC + 0.25                        # pivot plate J
X_SPACER = X_PIVOT_PLATE + PLY                       # spacer block L
X_HANDLE = X_SPACER + PLY                            # handle K
X_MOUNT = X_HANDLE + PLY                             # mounting blocks M on the handle's right face
BODY_R = BODY_D / 2 / IN
X_BODY = X_MOUNT + PLY - MOUNT_BITE + BODY_R         # the body nestled in the blocks' cutouts
X_WHEEL = X_BODY + BODY_X / IN                       # the wheel's inner face: the cut, and the channel under it
ARM_TOP = BASE_TOP + ARM_H
RAIL_Z = (ARM_TOP - RAIL_DROP - RAIL_H, ARM_TOP - RAIL_DROP)
Z_PIVOT = (RAIL_Z[0] + RAIL_Z[1]) / 2                # the carriage plate centred on the rail
GUIDE_Z = ((RAIL_Z[0] - GUIDE, RAIL_Z[0]), (RAIL_Z[1], RAIL_Z[1] + GUIDE))   # the guides touch the rail's edges
PLATE_Z = (Z_PIVOT - PLATE_H / 2, Z_PIVOT + PLATE_H / 2)
RAIL_Y = (ARM_Y[1] - RAIL_L, ARM_Y[1])
Y_PIVOT = 12.75                                      # the drawn pose: mid-travel
TRAVEL = (PIN_Y[0] + PIN_D / 2 + PLATE_L / 2 - SLOT_DEEP, PIN_Y[1] - PIN_D / 2 - PLATE_L / 2 + SLOT_DEEP)  # y of the pivot at the stops: a slot's closed end on its pin
LEVER = 5.0                                          # the spindle ahead of the pivot: far enough that a 3-1/2 plate fits between the fence and the wheel's reach
PLATE_BEVEL = 1.25                                   # the pivot plate's front bottom corner, 45 degrees, clear of the fence when plunged
Z_BODY = Z_PIVOT - 0.25                              # the body's axis, at rest
Y_SPINDLE = Y_PIVOT - LEVER
PIVOT_PLATE_Z = (Z_PIVOT - PIVOT_UP, Z_PIVOT - PIVOT_UP + PIVOT_PLATE[1])
SPACER_Z = (PIVOT_PLATE_Z[1] - SPACER[1], PIVOT_PLATE_Z[1])
Z_HANDLE = SPACER_Z[0]                               # the handle's tab bottom on the spacer's bottom edge
MOUNT_Z = (Z_BODY - 2.0, Z_BODY - 2.0 + MOUNT[1])
STOP_Y = (Y_PIVOT + PIVOT_PLATE[0] / 2 + 0.125, Y_PIVOT + PLATE_L / 2)
STOP_Z = (PLATE_Z[1] - 1.125, PLATE_Z[1] - 1.125 + STOP_H)
ARM_BOLT_X = (X_SMALL + X_ARM + ARM_T) / 2           # the carriage bolts centred in the glued-up small and main arms
ARM_BOLT_Y = (ARM_Y[1] - 3.0, ARM_Y[1] - 1.0)
COUNTERBORE = (1.25, 1.0)

MATERIALS = {"birch": ("3/4 birch ply", 0.68), "hardwood": ("maple", 0.70), "resin": ("resin", 1.4),
             "steel": ("steel", 7.85), "aluminium": ("aluminium", 2.7)}


def mm(*v):
    return tuple(c * IN for c in v)


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, d = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": d}})
    p.apply(*ops)


# -- shapes, inches in, millimetres to the kernel ---------------------------

def block(p, name, x, y, z, op="new"):
    s = p.sketch("top", z[0] * IN, name)
    p.rect(s, mm(x[0], y[0]), mm(x[1], y[1]))
    return p.extrude(s, (z[1] - z[0]) * IN, op=op, name=name)


def cut_block(p, name, x, y, z):
    s = p.sketch("top", z[1] * IN, name)
    p.rect(s, mm(x[0], y[0]), mm(x[1], y[1]))
    p.cut(s, (z[1] - z[0]) * IN, direction="reverse", name=name)


def cyl_x(p, name, x0, x1, y, z, d, op="new"):
    s = p.sketch("right", x0 * IN, name)
    p.circle(s, mm(y, z), d / 2 * IN)
    return p.extrude(s, (x1 - x0) * IN, op=op, name=name)


def cyl_y(p, name, y0, y1, x, z, d, op="new"):
    """A cylinder along y from y0 back to y1 (y1 > y0)."""
    s = p.sketch("front", -y1 * IN, name)
    p.circle(s, mm(x, z), d / 2 * IN)
    return p.extrude(s, (y1 - y0) * IN, op=op, name=name)


def cyl_z(p, name, z0, z1, x, y, d, op="new"):
    s = p.sketch("top", z0 * IN, name)
    p.circle(s, mm(x, y), d / 2 * IN)
    return p.extrude(s, (z1 - z0) * IN, op=op, name=name)


def bore_x(p, name, x0, x1, y, z, d):
    s = p.sketch("right", (x0 - 0.05) * IN, name)
    p.point(s, mm(y, z))
    p.hole(s, d * IN, depth=(x1 - x0 + 0.1) * IN, direction="normal", name=name)


def bore_down(p, name, z_top, x, y, d, depth, counterbore=None):
    s = p.sketch("top", z_top * IN, name)
    p.point(s, mm(x, y))
    cb = {"diameter": counterbore[0] * IN, "depth": counterbore[1] * IN} if counterbore else None
    p.hole(s, d * IN, depth=depth * IN, direction="reverse", counterbore=cb, name=name)


def bore_up(p, name, z_bottom, x, y, d, depth, counterbore=None):
    """A hole drilled from the underside, upward."""
    s = p.sketch("top", z_bottom * IN, name)
    p.point(s, mm(x, y))
    cb = {"diameter": counterbore[0] * IN, "depth": counterbore[1] * IN} if counterbore else None
    p.hole(s, d * IN, depth=depth * IN, direction="normal", counterbore=cb, name=name)


def profile(p, s, segs):
    """Lines and arcs round a closed region: ("line", a, b) or
    ("arc", centre, start, end), inches in the sketch's plane."""
    ops = []
    for seg in segs:
        if seg[0] == "line":
            ops.append({"type": "sketch", "id": s, "op": {"type": "add_line", "a": {"x": seg[1][0] * IN, "y": seg[1][1] * IN}, "b": {"x": seg[2][0] * IN, "y": seg[2][1] * IN}}})
        else:
            ops.append({"type": "sketch", "id": s, "op": {"type": "add_arc", "center": {"x": seg[1][0] * IN, "y": seg[1][1] * IN},
                                                         "start": {"x": seg[2][0] * IN, "y": seg[2][1] * IN}, "end": {"x": seg[3][0] * IN, "y": seg[3][1] * IN}}})
    p.apply(*ops)


def profile_x(p, name, x0, x1, segs, op="new"):
    """A profile in the y-z plane extruded from x0 to x1."""
    s = p.sketch("right", x0 * IN, name)
    profile(p, s, segs)
    return p.extrude(s, (x1 - x0) * IN, profiles="largest", op=op, name=name)


def profile_y(p, name, y0, y1, segs, op="new"):
    """A profile in the x-z plane extruded from y0 back to y1."""
    s = p.sketch("front", -y1 * IN, name)
    profile(p, s, segs)
    return p.extrude(s, (y1 - y0) * IN, profiles="largest", op=op, name=name)


def polygon_x(p, name, x0, x1, pts, op="new"):
    s = p.sketch("right", x0 * IN, name)
    p.polygon(s, [mm(*q) for q in pts])
    return p.extrude(s, (x1 - x0) * IN, profiles="largest", op=op, name=name)


def rounded_rect(x, y, r, corners=("fl", "fr", "bl", "br")):
    """A rectangle (x0, x1) x (y0, y1) with radius r on the named corners
    (front/back, left/right), as profile segments in the x-y plane,
    counter-clockwise."""
    x0, x1 = x
    y0, y1 = y
    segs = []
    # front edge, left to right
    a = (x0 + r, y0) if "fl" in corners else (x0, y0)
    b = (x1 - r, y0) if "fr" in corners else (x1, y0)
    segs.append(("line", a, b))
    if "fr" in corners:
        segs.append(("arc", (x1 - r, y0 + r), (x1 - r, y0), (x1, y0 + r)))
    a = (x1, y0 + r) if "fr" in corners else (x1, y0)
    b = (x1, y1 - r) if "br" in corners else (x1, y1)
    segs.append(("line", a, b))
    if "br" in corners:
        segs.append(("arc", (x1 - r, y1 - r), (x1, y1 - r), (x1 - r, y1)))
    a = (x1 - r, y1) if "br" in corners else (x1, y1)
    b = (x0 + r, y1) if "bl" in corners else (x0, y1)
    segs.append(("line", a, b))
    if "bl" in corners:
        segs.append(("arc", (x0 + r, y1 - r), (x0 + r, y1), (x0, y1 - r)))
    a = (x0, y1 - r) if "bl" in corners else (x0, y1)
    b = (x0, y0 + r) if "fl" in corners else (x0, y0)
    segs.append(("line", a, b))
    if "fl" in corners:
        segs.append(("arc", (x0 + r, y0 + r), (x0, y0 + r), (x0 + r, y0)))
    return segs


# -- the stand: everything that stays put -------------------------------------

def base(p):
    """Three layers of 3/4 ply glued up: 23 x 21 with 1 in radii on the
    front corners and a 6 x 1 tab on the back edge for the support arm."""
    s = p.sketch("top", 0.0, "base")
    profile(p, s, rounded_rect((0.0, BASE_W), (0.0, BASE_D), BASE_R, ("fl", "fr")))
    f = p.extrude(s, BASE_T * IN, profiles="largest", name="base")
    s = p.sketch("top", 0.0, "tab")
    profile(p, s, rounded_rect(TAB_X, (BASE_D - 0.5, BASE_D + TAB_D), BASE_R, ("bl", "br")))
    p.extrude(s, BASE_T * IN, profiles="largest", op="add", name="tab")
    # The channel's dado and the rabbets for the steel inserts, from the front edge.
    cw, _, _, cl = CHANNEL
    cut_block(p, "channel dado", (X_WHEEL - cw / 2, X_WHEEL + cw / 2), (-0.1, cl), (BASE_TOP - CHANNEL_DADO, BASE_TOP + 0.1))
    cut_block(p, "insert rabbets", (X_WHEEL - cw / 2 - INSERT_W, X_WHEEL + cw / 2 + INSERT_W), (-0.1, cl), (BASE_TOP - INSERT_T, BASE_TOP + 0.1))
    # Carriage bolt holes for the arm, counterbored from below for the heads.
    for y in ARM_BOLT_Y:
        bore_up(p, f"arm bolt hole y {y:g}", 0.0, ARM_BOLT_X, y, CARRIAGE_BOLT_D + 1 / 16, BASE_T + 0.1, counterbore=COUNTERBORE)
    # Threaded inserts for the fence, two rows, and the dowel peg holes.
    for row in FENCE_ROWS:
        for x in INSERT_X:
            bore_down(p, f"insert x {x:g} y {row:g}", BASE_TOP, x, row + FENCE[0] / 2, 0.375, 0.5)
    for x, y in PEG_AT:
        bore_down(p, f"peg hole x {x:g}", BASE_TOP, x, y, PEG_HOLE[0], PEG_HOLE[1])
    part_is(p, f, f"base, 3 x 3/4 ply, {BASE_W:g} x {BASE_D:g} with a {TAB_X[1] - TAB_X[0]:g} x {TAB_D:g} tab", "birch")


def channel(p):
    cw, ch, ct, cl = CHANNEL
    z0 = BASE_TOP - CHANNEL_DADO
    f = block(p, "channel", (X_WHEEL - cw / 2, X_WHEEL + cw / 2), (0.0, cl), (z0, z0 + ch))
    cut_block(p, "channel inside", (X_WHEEL - cw / 2 + ct, X_WHEEL + cw / 2 - ct), (-0.1, cl + 0.1), (z0 + ct, z0 + ch + 0.1))
    part_is(p, f, f"U-channel, 1/2 x 1 x 1/8 steel, {cl:g} long", "steel")


def insert(name, side):
    def build(p):
        cw, _, _, cl = CHANNEL
        x = (X_WHEEL + side * cw / 2, X_WHEEL + side * (cw / 2 + INSERT_W))
        f = block(p, name, (min(x), max(x)), (0.0, cl), (BASE_TOP - INSERT_T, BASE_TOP))
        part_is(p, f, f"{name}, 1/8 x {INSERT_W:g} x {cl:g} steel", "steel")
    return build


def fence(name, x, notch):
    """A length of 1 x 1 x 1/8 angle, vertical leg at the front, lying
    on the base with its horizontal leg back over the threaded inserts;
    the gap end's leg beveled 45 degrees, the left fence's leg cut down
    where the gearhead comes over it."""
    def build(p):
        row = FENCE_ROWS[1]
        leg, t = FENCE
        x0, x1 = x
        y0 = row
        f = block(p, f"{name} horizontal leg", (x0, x1), (y0, y0 + leg), (BASE_TOP, BASE_TOP + t))
        block(p, f"{name} vertical leg", (x0, x1), (y0, y0 + t), (BASE_TOP, BASE_TOP + leg), op="add")
        # 45 degree bevel on the leg at the gap end
        gap_end = x1 if name.startswith("left") else x0
        d = 1 if name.startswith("left") else -1
        s = p.sketch("front", -(y0 + t + 0.1) * IN, f"{name} bevel")
        p.polygon(s, [mm(gap_end, BASE_TOP + leg + 0.1), mm(gap_end - d * (leg - t + 0.1), BASE_TOP + leg + 0.1), mm(gap_end, BASE_TOP + t)])
        p.cut(s, (t + 0.2) * IN, name=f"{name} bevel")
        if notch:
            cut_block(p, f"{name} notch", (x1 - FENCE_NOTCH, x1 + 0.1), (y0 - 0.1, y0 + t + 0.1), (BASE_TOP + t, BASE_TOP + leg + 0.1))
        # bolt holes over the inserts
        s = p.sketch("top", (BASE_TOP + t) * IN, f"{name} bolt holes")
        for ix in INSERT_X:
            if x0 < ix < x1:
                p.point(s, mm(ix, y0 + leg / 2))
        p.hole(s, 0.28 * IN, depth=(t + 0.1) * IN, direction="reverse", name=f"{name} bolt holes")
        part_is(p, f, f"{name}, 1 x 1 x 1/8 steel angle, {x1 - x0:.2f} long", "steel")
    return build


def fence_bolts(p):
    """1/4-20 x 1/2 hex bolts with washers through the fence into the
    inserts, one body each."""
    row = FENCE_ROWS[1]
    leg, t = FENCE
    for k, ix in enumerate(INSERT_X):
        f = cyl_z(p, f"fence bolt {k + 1}", BASE_TOP - 0.3, BASE_TOP + t + WASHER_T, ix, row + leg / 2, 0.24)
        cyl_z(p, f"fence bolt {k + 1} washer", BASE_TOP + t, BASE_TOP + t + WASHER_T, ix, row + leg / 2, 0.62, op="add")
        cyl_z(p, f"fence bolt {k + 1} head", BASE_TOP + t + WASHER_T, BASE_TOP + t + WASHER_T + 0.17, ix, row + leg / 2, 0.44, op="add")
        part_is(p, f, f"fence bolt {k + 1}, 1/4-20 x 1/2 hex, washer", "steel")


def pegs(p):
    for k, (x, y) in enumerate(PEG_AT):
        f = cyl_z(p, f"peg {k + 1}", BASE_TOP - PEG_HOLE[1] + 0.02, BASE_TOP + 1.5, x, y, PEG_HOLE[0] - 0.01)
        part_is(p, f, f"dowel peg {k + 1}, 3/8 x 2-1/2", "hardwood")


def main_arm(p):
    """Two plies laminated: a 6 in foot at the back, 4 in long, and a
    1-3/4 bar forward of it with a 1-1/4 radius under its root."""
    y0, y1 = ARM_Y
    foot = y1 - FOOT_L
    bar_z = ARM_TOP - BAR_H
    r, nr = BAR_R, BAR_NOSE_R
    segs = [
        ("line", (foot, BASE_TOP), (y1, BASE_TOP)),
        ("line", (y1, BASE_TOP), (y1, ARM_TOP)),
        ("line", (y1, ARM_TOP), (y0, ARM_TOP)),
        ("line", (y0, ARM_TOP), (y0, bar_z + nr)),
        ("arc", (y0 + nr, bar_z + nr), (y0, bar_z + nr), (y0 + nr, bar_z)),
        ("line", (y0 + nr, bar_z), (foot - r, bar_z)),
        ("arc", (foot - r, bar_z - r), (foot - r, bar_z), (foot, bar_z - r)),
        ("line", (foot, bar_z - r), (foot, BASE_TOP)),
    ]
    f = profile_x(p, "main support arm", X_ARM, X_ARM + ARM_T, segs)
    for y in ARM_BOLT_Y:
        bore_down(p, f"arm bolt hole y {y:g}", ARM_TOP, ARM_BOLT_X, y, CARRIAGE_BOLT_D + 1 / 32, ARM_H + 0.1)
    part_is(p, f, f"main support arm, 2 x 3/4 ply, {ARM_H:g} x {ARM_L:g}", "birch")


def small_arm(p):
    """3/4 ply glued to the main arm's far side, 6 tall at the back,
    its top tapering to the front end."""
    y1 = ARM_Y[1]
    y0 = y1 - SMALL_L
    y_full = y1 - FOOT_L
    pts = [(y0, BASE_TOP), (y1, BASE_TOP), (y1, ARM_TOP), (y_full, ARM_TOP), (y0, BASE_TOP + SMALL_FRONT)]
    f = polygon_x(p, "small support arm", X_SMALL, X_SMALL + PLY, pts)
    # the bolt holes, centred in the glued-up pair, fall wholly in the main arm
    part_is(p, f, f"small support arm, 3/4 ply, {SMALL_H:g} x {SMALL_L:g}, tapered", "birch")


def support_block(name, y):
    def build(p):
        f = block(p, name, (X_BLOCK, X_BLOCK + PLY), y, (BASE_TOP, BASE_TOP + BLOCK_H))
        part_is(p, f, f"{name}, 3/4 ply, {BLOCK_W:g} x {BLOCK_H:g}", "birch")
    return build


def rail(p):
    """1 in hardwood, 2-1/2 x 21, a centred groove in each edge for the
    slides, holes near the ends for the stop pins, screwed to the arm."""
    y0, y1 = RAIL_Y
    f = block(p, "rail", (X_RAIL, X_RAIL + RAIL_T), (y0, y1), RAIL_Z)
    xc = X_RAIL + RAIL_T / 2
    gd = SLIDE_W - SLIDE_OUT
    cut_block(p, "top groove", (xc - SLIDE_T / 2, xc + SLIDE_T / 2), (y0 - 0.1, y1 + 0.1), (RAIL_Z[1] - gd, RAIL_Z[1] + 0.1))
    cut_block(p, "bottom groove", (xc - SLIDE_T / 2, xc + SLIDE_T / 2), (y0 - 0.1, y1 + 0.1), (RAIL_Z[0] - 0.1, RAIL_Z[0] + gd))
    for y in PIN_Y:
        bore_x(p, f"pin hole y {y:g}", X_RAIL + RAIL_T - PIN_IN, X_RAIL + RAIL_T, y, Z_PIVOT, PIN_D)
    part_is(p, f, f"rail, 1 in maple, {RAIL_H:g} x {RAIL_L:g}", "hardwood")


def slide(name, top):
    def build(p):
        xc = X_RAIL + RAIL_T / 2
        gd = SLIDE_W - SLIDE_OUT
        z = (RAIL_Z[1] - gd, RAIL_Z[1] + SLIDE_OUT) if top else (RAIL_Z[0] - SLIDE_OUT, RAIL_Z[0] + gd)
        f = block(p, name, (xc - SLIDE_T / 2, xc + SLIDE_T / 2), RAIL_Y, z)
        part_is(p, f, f"{name}, 1/4 x 3/4 x {RAIL_L:g} resin", "resin")
    return build


def pin(name, y):
    def build(p):
        f = cyl_x(p, name, X_RAIL + RAIL_T - PIN_IN, X_RAIL + RAIL_T - PIN_IN + PIN_L, y, Z_PIVOT, PIN_D - 0.005)
        part_is(p, f, f"{name}, 1/4 x 1 steel", "steel")
    return build


def arm_bolt(name, y):
    """1/2-13 x 8 carriage bolt up through the base and the arm, its
    head in the counterbore under the base, fender washer and lock nut on top."""
    def build(p):
        head_t = 0.33
        z0 = COUNTERBORE[1]                      # the head up against the counterbore's ceiling
        f = cyl_z(p, name, z0, z0 + CARRIAGE_BOLT_L - head_t, ARM_BOLT_X, y, CARRIAGE_BOLT_D - 0.01)
        cyl_z(p, f"{name} head", z0 - head_t, z0, ARM_BOLT_X, y, 1.1, op="add")
        cyl_z(p, f"{name} washer", ARM_TOP, ARM_TOP + 0.06, ARM_BOLT_X, y, 1.5, op="add")
        cyl_z(p, f"{name} nut", ARM_TOP + 0.06, ARM_TOP + 0.06 + 0.44, ARM_BOLT_X, y, 0.87, op="add")
        part_is(p, f, f"{name}, 1/2-13 x 8 carriage, fender washer, lock nut", "steel")
    return build


# -- the carriage: slides on the rail ------------------------------------------

def plate(p):
    """3/4 ply, 8-3/4 x 4-3/4, the lower corners cut at 45 degrees, a slot
    in each end for the stop pins, the pivot bolt through its middle
    with the head counterbored into the rail side."""
    yc, zc = Y_PIVOT, Z_PIVOT
    hl, hh, c = PLATE_L / 2, PLATE_H / 2, PLATE_CHAMFER
    pts = [(yc - hl + c, zc - hh), (yc + hl - c, zc - hh), (yc + hl, zc - hh + c), (yc + hl, zc + hh), (yc - hl, zc + hh), (yc - hl, zc - hh + c)]
    f = polygon_x(p, "carriage plate", X_PLATE, X_PLATE + PLY, pts)
    for side in (-1, 1):
        y_open = yc + side * (hl + 0.1)
        y_end = yc + side * (hl - SLOT_DEEP)
        cut_block(p, f"stop slot {'back' if side > 0 else 'front'}", (X_PLATE - 0.1, X_PLATE + PLY + 0.1), (min(y_open, y_end), max(y_open, y_end)), (zc - SLOT_W / 2, zc + SLOT_W / 2))
    bore_x(p, "pivot bolt hole", X_PLATE, X_PLATE + PLY, yc, zc, PIVOT_BOLT_D + 1 / 32)
    s = p.sketch("right", X_PLATE * IN, "head counterbore")
    p.circle(s, mm(yc, zc), 0.875 / 2 * IN)
    p.cut(s, 0.25 * IN, name="head counterbore")
    part_is(p, f, f"carriage plate, 3/4 ply, {PLATE_H:g} x {PLATE_L:g}", "birch")


def guide(name, top):
    """1 x 1 x 5 hardwood on the plate's rail side, over or under the rail,
    grooved for the slide, touching the rail's edge."""
    def build(p):
        z = GUIDE_Z[1] if top else GUIDE_Z[0]
        y = (Y_PIVOT - GUIDE_L / 2, Y_PIVOT + GUIDE_L / 2)
        f = block(p, name, (X_RAIL, X_RAIL + GUIDE), y, z)
        xc = X_RAIL + RAIL_T / 2
        gw, gd = GUIDE_GROOVE
        gz = (z[0], z[0] + gd) if top else (z[1] - gd, z[1])
        cut_block(p, f"{name} groove", (xc - gw / 2, xc + gw / 2), (y[0] - 0.1, y[1] + 0.1), (gz[0] - (0.1 if top else 0), gz[1] + (0 if top else 0.1)))
        part_is(p, f, f"{name}, 1 x 1 x {GUIDE_L:g} maple, grooved", "hardwood")
    return build


def disc(p):
    f = cyl_x(p, "disc", X_DISC, X_DISC + 0.25, Y_PIVOT, Z_PIVOT, DISC_PLY_D)
    bore_x(p, "disc hole", X_DISC, X_DISC + 0.25, Y_PIVOT, Z_PIVOT, PIVOT_BOLT_D + 1 / 32)
    part_is(p, f, f"disc, 1/4 ply, {DISC_PLY_D:g} dia", "birch")


def stop(p):
    """3/4 ply on the plate's outer face behind the pivot plate: the
    plate's top back corner meets it when the spring lifts the handle."""
    f = block(p, "stop", (X_PLATE + PLY, X_PLATE + 2 * PLY), STOP_Y, STOP_Z)
    part_is(p, f, f"stop, 3/4 ply, {STOP_L:g} x {STOP_H:g}", "birch")


def pivot_bolt(p):
    """3/8-16 x 2 carriage bolt through the plate, disc, ring and pivot
    plate; fender washer and lock nut on the pivot plate's outer face."""
    head_t, x0 = 0.23, X_PLATE + 0.25 - 0.23
    f = cyl_x(p, "pivot bolt", x0, x0 + PIVOT_BOLT_L, Y_PIVOT, Z_PIVOT, PIVOT_BOLT_D - 0.2 / IN)
    cyl_x(p, "pivot bolt head", x0, x0 + head_t, Y_PIVOT, Z_PIVOT, 0.8, op="add")
    x_nut = X_PIVOT_PLATE + PLY
    cyl_x(p, "pivot washer", x_nut, x_nut + 0.06, Y_PIVOT, Z_PIVOT, 1.0, op="add")
    cyl_x(p, "pivot lock nut", x_nut + 0.06, x_nut + 0.06 + 0.33, Y_PIVOT, Z_PIVOT, 0.65, op="add")
    part_is(p, f, "pivot bolt, 3/8-16 x 2 carriage, fender washer, lock nut", "steel")


# -- the handle assembly: pivots on the disc -------------------------------------

def pivot_plate(p):
    """3/4 ply, the pivot 3-1/8 up from its bottom edge, its front bottom
    corner cut at 45 degrees like the carriage plate's."""
    yc = Y_PIVOT
    y0, y1 = yc - PIVOT_PLATE[0] / 2, yc + PIVOT_PLATE[0] / 2
    z0, z1 = PIVOT_PLATE_Z
    b = PLATE_BEVEL
    f = polygon_x(p, "pivot plate", X_PIVOT_PLATE, X_PIVOT_PLATE + PLY, [(y0 + b, z0), (y1, z0), (y1, z1), (y0, z1), (y0, z0 + b)])
    bore_x(p, "pivot hole", X_PIVOT_PLATE, X_PIVOT_PLATE + PLY, yc, Z_PIVOT, PIVOT_BOLT_D + 1 / 32)
    part_is(p, f, f"pivot plate, 3/4 ply, {PIVOT_PLATE[0]:g} x {PIVOT_PLATE[1]:g}", "birch")


def ring(p):
    f = cyl_x(p, "ring", X_DISC, X_DISC + 0.25, Y_PIVOT, Z_PIVOT, RING_OD)
    bore_x(p, "ring inside", X_DISC, X_DISC + 0.25, Y_PIVOT, Z_PIVOT, RING_ID + 1 / 32)
    part_is(p, f, f"ring, 1/4 ply, {RING_OD:g} dia, {RING_ID:g} inside", "birch")


def spacer(p):
    yc = Y_PIVOT
    f = block(p, "spacer block", (X_SPACER, X_SPACER + PLY), (yc - SPACER[0] / 2, yc + SPACER[0] / 2), SPACER_Z)
    part_is(p, f, f"spacer block, 3/4 ply, {SPACER[1]:g} x {SPACER[0]:g}", "birch")


def handle(p):
    """The plan's pattern, its tab on the spacer block, the hand-hold cut
    through; the grip leans forward over the wheel."""
    y0 = Y_PIVOT - HANDLE_TAB / 2
    pts = [(y0 + u, Z_HANDLE + v) for u, v in HANDLE]
    f = polygon_x(p, "handle", X_HANDLE, X_HANDLE + PLY, pts)
    (ua, va), (ub, vb), d = HAND_HOLD
    s = p.sketch("right", (X_HANDLE - 0.1) * IN, "hand-hold")
    p.slot(s, mm(y0 + ua, Z_HANDLE + va), mm(y0 + ub, Z_HANDLE + vb), d * IN)
    p.cut(s, (PLY + 0.2) * IN, name="hand-hold")
    part_is(p, f, "handle, 3/4 ply, from the pattern", "birch")


def mount_block(name, dy):
    """3/4 ply on the handle's outer face, its outer edge cut to the
    grinder body's round so the body nestles in it."""
    def build(p):
        y0 = Y_SPINDLE + dy
        f = block(p, name, (X_MOUNT, X_MOUNT + PLY), (y0, y0 + MOUNT[0]), MOUNT_Z)
        s = p.sketch("front", -(y0 + MOUNT[0] + 0.1) * IN, f"{name} cutout")
        p.circle(s, mm(X_BODY, Z_BODY), BODY_R * IN + 0.3)
        p.cut(s, (MOUNT[0] + 0.2) * IN, name=f"{name} cutout")
        part_is(p, f, f"{name}, 3/4 ply, {MOUNT[0]:g} x {MOUNT[1]:g}, cut to the body", "birch")
    return build


def grinder(p):
    """Gearhead, bearing collar, spindle, outer nut and motor body: the
    wheel's inner face on x = X_WHEEL, the spindle along x, the body
    along y toward the back, lying in the mounting blocks."""
    xw, ys, zs = X_WHEEL * IN, Y_SPINDLE * IN, Z_BODY * IN
    # chop-saw frame (x' from the wheel along the spindle, y' toward the body) to this one: x = xw - x', y = ys - y'
    f = block(p, "gearhead", ((xw - GH_X[1]) / IN, (xw - GH_X[0]) / IN), ((ys - GH_BACK) / IN, (ys + GH_FWD) / IN), ((zs - GH_HALF) / IN, (zs + GH_HALF) / IN))
    cyl_x(p, "bearing collar", (xw - GH_X[0]) / IN, xw / IN, ys / IN, zs / IN, COLLAR_D / IN, op="add")
    cyl_x(p, "spindle", xw / IN, (xw + DISC_T + NUT_T) / IN, ys / IN, zs / IN, SPINDLE_D / IN, op="add")
    cyl_x(p, "outer nut", (xw + DISC_T) / IN, (xw + DISC_T + NUT_T) / IN, ys / IN, zs / IN, NUT_D / IN, op="add")
    cyl_y(p, "motor body", (ys + GH_FWD) / IN, (ys + GH_FWD + BODY_L) / IN, X_BODY, Z_BODY, BODY_D / IN, op="add")
    part_is(p, f, "grinder, 4-1/2 in (Chicago Electric, orange)")


def wheel(p):
    xw = X_WHEEL * IN
    f = cyl_x(p, "wheel", xw / IN, (xw + DISC_T) / IN, Y_SPINDLE, Z_BODY, DISC_D / IN)
    bore_x(p, "arbor hole", xw / IN, (xw + DISC_T) / IN, Y_SPINDLE, Z_BODY, DISC_BORE / IN)
    part_is(p, f, "wheel, 4-1/2 x 0.040 x 7/8 cut-off", "steel")


def guard(p):
    """The grinder's own, turned to cover the top of the wheel: a back
    plate on the gearhead side and a rim across the wheel."""
    xw = X_WHEEL * IN
    y, z = Y_SPINDLE, Z_BODY
    ro, ri = GUARD_R / IN, (COLLAR_D / 2 + 1.0) / IN

    def half_annulus(name, x0, x1, r_out, r_in, op):
        s = p.sketch("right", x0 * IN, name)
        p.draw(s, {"type": "add_arc", "center": {"x": y * IN, "y": z * IN}, "start": {"x": (y + r_out) * IN, "y": z * IN}, "end": {"x": (y - r_out) * IN, "y": z * IN}})
        p.draw(s, {"type": "add_arc", "center": {"x": y * IN, "y": z * IN}, "start": {"x": (y + r_in) * IN, "y": z * IN}, "end": {"x": (y - r_in) * IN, "y": z * IN}})
        p.draw(s, {"type": "add_line", "a": {"x": (y - r_out) * IN, "y": z * IN}, "b": {"x": (y - r_in) * IN, "y": z * IN}})
        p.draw(s, {"type": "add_line", "a": {"x": (y + r_in) * IN, "y": z * IN}, "b": {"x": (y + r_out) * IN, "y": z * IN}})
        return p.extrude(s, (x1 - x0) * IN, profiles="largest", op=op, name=name)
    # chop-saw x' -> x = xw - x': the back plate at x' = GUARD_X[1] - t .. GUARD_X[1], the rim x' = GUARD_X[0] .. GUARD_X[1] - t
    f = half_annulus("guard back plate", (xw - GUARD_X[1]) / IN, (xw - GUARD_X[1] + GUARD_T) / IN, ro, ri, "new")
    half_annulus("guard rim", (xw - GUARD_X[1] + GUARD_T) / IN, (xw - GUARD_X[0]) / IN, ro, ro - GUARD_T / IN, "add")
    part_is(p, f, "guard, the grinder's own, top half", "steel")


def strap(p):
    """1/2 x 1/16 aluminium bent round the body between the mounting
    blocks, from the handle's face over the top, down the outside and
    under, its ends screwed to the handle and a block."""
    y0 = Y_SPINDLE + (MOUNT_Y[0] + MOUNT[0] + MOUNT_Y[1]) / 2 - 0.25
    r = BODY_R + 0.02
    t = 1 / 16
    s = p.sketch("front", -(y0 + 0.5) * IN, "strap")
    xc, zc = X_BODY, Z_BODY
    # a three-quarter ring from the top (90 degrees) round the right to the bottom-left (225 degrees)
    a0, a1 = math.radians(90.0), math.radians(-135.0)
    for rr, (p0, p1) in ((r + t, (a0, a1)), (r, (a0, a1))):
        p.draw(s, {"type": "add_arc", "center": {"x": xc * IN, "y": zc * IN},
                   "start": {"x": (xc + rr * math.cos(p1)) * IN, "y": (zc + rr * math.sin(p1)) * IN},
                   "end": {"x": (xc + rr * math.cos(p0)) * IN, "y": (zc + rr * math.sin(p0)) * IN}})
    for a in (a0, a1):
        p.draw(s, {"type": "add_line", "a": {"x": (xc + r * math.cos(a)) * IN, "y": (zc + r * math.sin(a)) * IN},
                   "b": {"x": (xc + (r + t) * math.cos(a)) * IN, "y": (zc + (r + t) * math.sin(a)) * IN}})
    f = p.extrude(s, 0.5 * IN, profiles="largest", name="strap")
    part_is(p, f, "strap, 1/16 x 1/2 x 9 aluminium, bent round the body", "aluminium")


STAND = [("base", base), ("channel", channel), ("left insert", insert("left insert", -1)), ("right insert", insert("right insert", 1)),
         ("left fence", fence("left fence", (0.25, X_WHEEL - FENCE_GAP / 2), True)), ("right fence", fence("right fence", (X_WHEEL + FENCE_GAP / 2, BASE_W - 0.25), False)),
         ("fence bolts", fence_bolts), ("pegs", pegs), ("main support arm", main_arm), ("small support arm", small_arm),
         ("front support block", support_block("front support block", (ARM_Y[1] - 2 * BLOCK_W, ARM_Y[1] - BLOCK_W))),
         ("back support block", support_block("back support block", (ARM_Y[1] - BLOCK_W, ARM_Y[1]))),
         ("rail", rail), ("top slide", slide("top slide", True)), ("bottom slide", slide("bottom slide", False)),
         ("front pin", pin("front pin", PIN_Y[0])), ("back pin", pin("back pin", PIN_Y[1])),
         ("front arm bolt", arm_bolt("front arm bolt", ARM_BOLT_Y[0])), ("back arm bolt", arm_bolt("back arm bolt", ARM_BOLT_Y[1]))]
CARRIAGE = [("carriage plate", plate), ("top guide", guide("top guide", True)), ("bottom guide", guide("bottom guide", False)),
            ("disc", disc), ("stop", stop), ("pivot bolt", pivot_bolt)]
HANDLE_PARTS = [("pivot plate", pivot_plate), ("ring", ring), ("spacer block", spacer), ("handle", handle),
                ("front mounting block", mount_block("front mounting block", MOUNT_Y[0])), ("back mounting block", mount_block("back mounting block", MOUNT_Y[1])),
                ("grinder", grinder), ("wheel", wheel), ("guard", guard), ("strap", strap)]


# -- mate helpers (as in examples/chop-saw/build.py) -----------------------------

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


def _edge_frame(origin, z, x_hint):
    """An edge-anchored connector's frame: z along the edge, x the face's
    normal projected square to it."""
    z = _norm(z)
    x = _norm(tuple(x_hint[i] - z[i] * _dot(x_hint, z) for i in range(3)))
    return origin, x, _cross(z, x), z


def cylinder(body, r):
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    for c in body["cylinders"]:
        if abs(c["radius"] - r) < 1e-6:
            return c, _frame(c, faces[json.dumps(c["reference"], sort_keys=True)])
    raise RuntimeError(f"{body['name']}: no cylinder of radius {r}")


def plane_face(body, normal, near):
    """The planar face of a body with this normal whose plane passes
    through `near` (millimetres), the one with the nearest centroid if
    several do."""
    best, bd = None, float("inf")
    for f in body["faces"]:
        if f["surface"] != "plane" or _dot(_v(f["normal"]), normal) < 0.999:
            continue
        if abs(_dot(_sub(_v(f["centroid"]), near), normal)) > 0.05:
            continue
        d = math.dist(_v(f["centroid"]), near)
        if d < bd:
            best, bd = f, d
    if best is None:
        raise RuntimeError(f"{body['name']}: no face with normal {normal}")
    return best


def edge_connector(body, face_normal, other_normal, near):
    """A connector on the face with `face_normal`, anchored on its edge
    with the face of `other_normal` nearest `near` (inches), and the
    frame the kernel builds for it: origin at the edge's middle, z along
    the edge the way the face's loop runs (face normal x other normal,
    at a convex edge), x along the face's normal."""
    n = tuple(c * IN for c in near)
    mine, other = plane_face(body, face_normal, n), plane_face(body, other_normal, n)
    conn = {"face": mine["reference"], "anchor": {"type": "edge", "other": other["reference"]}}
    z = _cross(face_normal, other_normal)
    return conn, z, mine, other


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
    ("carriage bolt", "1/2-13 x 8 carriage bolt, 1/2 fender washer, 1/2-13 lock nut", 2, "up through the base's counterbores and the support arm; do not glue the arm to the base"),
    ("pivot bolt", "3/8-16 x 2 carriage bolt, 3/8 fender washer, 3/8-16 lock nut", 1, "through the carriage plate (head in its counterbore on the rail side), disc, ring and pivot plate; snug so the handle swings freely without side play"),
    ("resin slide", "1/4 x 3/4 x 21 white resin bar (McMaster 8739K86)", 2, "epoxied into the rail's edge grooves, edges dressed with a block plane"),
    ("steel pin", "1/4 x 1 steel rod", 2, f"in the rail at {PIN_Y[0]:g} and {PIN_Y[1]:g} from the front, after the carriage is on; they stop the carriage's slots"),
    ("spring", ".438 OD x 3-3/4 extension spring, two 1/4 screw eyes", 1, "between the carriage plate's top back corner and the pivot plate: returns the grinder to the stop"),
    ("rail screws", "#8 x 1-3/4 Fh woodscrew", 6, "the rail to the arm, countersunk in the rail's face"),
    ("stop and guide screws", "#8 x 1-1/4 Fh woodscrew", 9, "the stop and guides to the carriage plate, the mounting blocks and spacer to the handle and pivot plate"),
    ("ring screws", "#8 x 1-1/2 Fh woodscrew", 2, "the ring to the pivot plate"),
    ("strap", "1/16 x 1/2 x 9 aluminium bar, two #10 x 1-1/4 Rh woodscrews, 1/4 washers", 1, "bent round the body on the grinder itself, ends at 90 degrees, screwed to the handle and a mounting block"),
    ("channel", "1/2 x 1 x 1/8 steel U-channel, 17 long", 1, "in the dado under the wheel, #8 x 1-1/2 Fh screws through its web"),
    ("steel insert", "1/8 x 4 x 17 hot-rolled flat", 2, "in the rabbets either side of the channel, flush with the base, #8 x 1-1/2 Fh screws"),
    ("fence", "1 x 1 x 1/8 steel angle, two lengths", 2, "beveled at the gap, the left one's vertical leg cut away for 2-1/2 at the gap where the gearhead comes down; 1/4-20 x 1/2 hex bolts into the inserts"),
    ("threaded insert", "1/4-20 threaded insert (McMaster 90016A029)", 8, "two rows for the fence's wide and narrow positions"),
    ("dowel peg", "3/8 dowel, 2-1/2 long", 2, "in the base's front right, to lean stock against"),
    ("wheel", "4-1/2 x 0.040 x 7/8 metal cut-off, Type 1", 5, "change it when the gearhead gets within 1/4 of the fence at full plunge"),
]

CUTLIST = [
    ("base", f"3/4 birch ply, {BASE_W:g} x {BASE_D:g}, three layers", 3, "glue up oversize, trim, then the tab and the front radii; the channel dado and insert rabbets after the arm is fitted and the wheel has scored the line"),
    ("main support arm", f"3/4 birch ply, {ARM_H:g} x {ARM_L:g}", 2, "glued to 1-1/2; the bar and its radii on the band saw, sanded; drill the 1/2 bolt holes through it and the small arm together"),
    ("small support arm", f"3/4 birch ply, {SMALL_H:g} x {SMALL_L:g}", 1, "tapered to the front; glued to the main arm's far side, bottoms and backs flush"),
    ("support block", f"3/4 birch ply, {BLOCK_W:g} x {BLOCK_H:g}", 2, "end to end against the small arm's foot"),
    ("rail", f"maple, 1 x {RAIL_H:g} x {RAIL_L:g}", 1, "centred 1/4 groove 7/16 deep in each edge, cut with the rail flipped between passes; 1/8 roundovers; 1/4 holes 5/8 deep for the pins"),
    ("guide", f"maple, 1 x 1 x {GUIDE_L:g}", 2, "a 3/8 x 3/8 groove along one face for the slide"),
    ("carriage plate", f"3/4 birch ply, {PLATE_H:g} x {PLATE_L:g}", 1, "lower corners at 45 degrees; 1/2 slots 2-3/8 deep in from each end at the pivot's height; 3/8 hole with a 7/8 counterbore on the rail side"),
    ("stop", f"3/4 birch ply, {STOP_H:g} x {STOP_L:g}", 1, "on the plate's outer face behind the pivot plate"),
    ("disc", f"1/4 birch ply, {DISC_PLY_D:g} dia", 1, "circle cutter in the drill press, slow; 3/8 centre hole"),
    ("ring", f"1/4 birch ply, {RING_OD:g} dia, {RING_ID:g} inside", 1, "inner circle first, then the outer"),
    ("pivot plate", f"3/4 birch ply, {PIVOT_PLATE[1]:g} x {PIVOT_PLATE[0]:g}", 1, f"3/8 hole {PIVOT_UP:g} up from the bottom edge, centred"),
    ("spacer block", f"3/4 birch ply, {SPACER[1]:g} x {SPACER[0]:g}", 1, "at the pivot plate's top, between it and the handle"),
    ("handle", "3/4 birch ply, 8 x 10 rough", 1, "from the pattern: two 1-3/8 Forstner holes joined with a jig saw for the hand-hold, band saw to the outline, 1/8 roundovers but for the tab"),
    ("mounting block", f"3/4 birch ply, {MOUNT[0]:g} x {MOUNT[1]:g}", 2, "outer edge cut to the body's round until the wheel sits parallel to the handle"),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "cutoff_grinder.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "cutoff_grinder"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Sliding cutoff grinder"}]})
    tabs, bodies = {}, {}
    for i, (title, fn) in enumerate(STAND + CARRIAGE + HANDLE_PARTS):
        tab = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]})
        fn(Part(mcp, tab, title))
        tabs[title] = tab
        bodies[title] = full(mcp, tab)["bodies"]
        print(f"{title:<22} tab {tab:>2}: {', '.join(b['name'] for b in bodies[title])}")

    def instances(parts, fixed):
        return [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}",
                 "fixed": fixed, "placement": IDENTITY} for t, _ in parts for k, b in enumerate(bodies[t])]

    carriage_asm = new_tab(mcp, {"type": "add_assembly", "name": "carriage assembly"})
    apply(mcp, instances(CARRIAGE, True), carriage_asm)
    carriage_ids = {i["name"]: i["id"] for i in full(mcp, carriage_asm)["instances"]}
    handle_asm = new_tab(mcp, {"type": "add_assembly", "name": "handle assembly"})
    apply(mcp, instances(HANDLE_PARTS, True), handle_asm)
    handle_ids = {i["name"]: i["id"] for i in full(mcp, handle_asm)["instances"]}

    saw = new_tab(mcp, {"type": "add_assembly", "name": "sliding cutoff grinder"})
    ops = instances(STAND, True)
    ops.append({"type": "add_instance", "studio": carriage_asm, "body": 0, "name": "carriage", "fixed": False, "placement": IDENTITY})
    ops.append({"type": "add_instance", "studio": handle_asm, "body": 0, "name": "handle assembly", "fixed": False, "placement": IDENTITY})
    apply(mcp, ops, saw)
    ids = {i["name"]: i["id"] for i in full(mcp, saw)["instances"]}

    # The slide: the rail's top right edge and the top guide's bottom
    # right edge, which lie on one line along the rail (the guide rides
    # on the rail's edge), each as an edge-anchored connector on the
    # right face.
    x_edge = X_RAIL + RAIL_T
    rail_conn, rail_z, _, _ = edge_connector(bodies["rail"][0], (1.0, 0.0, 0.0), (0.0, 0.0, 1.0), (x_edge, (RAIL_Y[0] + RAIL_Y[1]) / 2, RAIL_Z[1]))
    guide_conn, guide_z, _, _ = edge_connector(bodies["top guide"][0], (1.0, 0.0, 0.0), (0.0, 0.0, -1.0), (x_edge, Y_PIVOT, RAIL_Z[1]))
    rail_frame = _edge_frame(mm(x_edge, (RAIL_Y[0] + RAIL_Y[1]) / 2, RAIL_Z[1]), rail_z, (1.0, 0.0, 0.0))
    guide_frame = _edge_frame(mm(x_edge, Y_PIVOT, RAIL_Z[1]), guide_z, (1.0, 0.0, 0.0))
    offset, angle, flip = mate_parameters(rail_frame, guide_frame)
    apply(mcp, [{"type": "add_mate", "kind": "slider", "a": {"instance": ids["rail"], **rail_conn},
                 "b": {"instance": ids["carriage"], "sub": carriage_ids["top guide"], **guide_conn},
                 "offset": offset, "angle": angle, "flip": flip, "name": "slide"}], saw)
    # The pivot: the bolt's shank in the pivot plate's hole.
    cb, tb = cylinder(bodies["pivot bolt"][0], (PIVOT_BOLT_D - 0.2 / IN) / 2 * IN)
    cp, tp = cylinder(bodies["pivot plate"][0], (PIVOT_BOLT_D + 1 / 32) / 2 * IN)
    offset2, angle2, flip2 = mate_parameters(tb, tp)
    apply(mcp, [{"type": "add_mate", "kind": "revolute", "a": {"instance": ids["carriage"], "sub": carriage_ids["pivot bolt"], "face": cb["reference"]},
                 "b": {"instance": ids["handle assembly"], "sub": handle_ids["pivot plate"], "face": cp["reference"]},
                 "offset": offset2, "angle": angle2, "flip": flip2, "name": "pivot"}], saw)
    report = full(mcp, saw)
    for k in ("mates", "instances"):
        for m in report[k]:
            if m.get("error"):
                raise RuntimeError(f"{k} {m['name']}: {m['error']}")
    mates = {m["name"]: m for m in report["mates"]}
    slide_m, pivot_m = mates["slide"], mates["pivot"]
    for inst in report["instances"]:
        pl = inst.get("placed") or IDENTITY
        moved = max(abs(v) for v in (*pl["position"].values(), *pl["rotation"].values()))
        if moved > 1e-3:
            raise RuntimeError(f"{inst['name']} resolved {moved:.3f} away from where it was drawn: {pl}")

    def wheel_bounds():
        return next(b for b in full(mcp, saw)["bodies"] if b["name"] == "handle assembly / wheel")["bounds"]

    def lowest(name):
        return next(b for b in full(mcp, saw)["bodies"] if b["name"] == name)["bounds"][0]["z"]

    def set_pose(y_pivot, deg):
        """Mates set for the carriage's pivot at y_pivot (inches) and the
        handle turned deg degrees down from rest."""
        apply(mcp, [{"type": "set_mate", "id": slide_m["id"], "offset": slide_m["offset"] + slide_sign * (y_pivot - Y_PIVOT) * IN},
                    {"type": "set_mate", "id": pivot_m["id"], "angle": pivot_m["angle"] + pivot_sign * deg}], saw)

    # Which way each mate's value moves things.
    slide_sign, pivot_sign = 1.0, 1.0
    y0 = wheel_bounds()[0]["y"]
    apply(mcp, [{"type": "set_mate", "id": slide_m["id"], "offset": slide_m["offset"] + 5.0}], saw)
    slide_sign = 1.0 if wheel_bounds()[0]["y"] > y0 else -1.0
    apply(mcp, [{"type": "set_mate", "id": slide_m["id"], "offset": slide_m["offset"]}], saw)
    z0 = wheel_bounds()[0]["z"]
    apply(mcp, [{"type": "set_mate", "id": pivot_m["id"], "angle": pivot_m["angle"] + 5.0}], saw)
    pivot_sign = 1.0 if wheel_bounds()[0]["z"] < z0 else -1.0
    set_pose(Y_PIVOT, 0.0)
    # How far the handle goes down: until the wheel's bottom is 1 mm off
    # the channel's floor, or the grinder's gearhead 1.5 mm off the
    # fence's horizontal leg, which it passes over at full plunge.
    floor = (BASE_TOP - CHANNEL_DADO + CHANNEL[2]) * IN
    leg_top = (BASE_TOP + FENCE[1]) * IN
    lo, hi = 0.0, 40.0
    for _ in range(30):
        mid = (lo + hi) / 2
        set_pose(Y_PIVOT, mid)
        if wheel_bounds()[0]["z"] > floor + 1.0 and lowest("handle assembly / grinder") > leg_top + 1.5:
            lo = mid
        else:
            hi = mid
    plunge = lo
    set_pose(Y_PIVOT, plunge)
    wheel_low, grinder_low = wheel_bounds()[0]["z"], lowest("handle assembly / grinder")
    set_pose(Y_PIVOT, 0.0)
    wb = wheel_bounds()
    depth = (BASE_TOP * IN - wheel_low) / IN
    print(f"wheel at rest: bottom z {wb[0]['z'] / IN:.2f} in, {(wb[0]['z'] - BASE_TOP * IN) / IN:.2f} over the base; "
          f"full plunge {plunge:.1f} degrees: the wheel {depth:.2f} in below the base's top ({(wheel_low - floor) / IN:.2f} off the channel floor), "
          f"the gearhead {(grinder_low - leg_top) / IN:.2f} over the fence's leg; travel {TRAVEL[0]:.3f} to {TRAVEL[1]:.3f} in of pivot y, "
          f"{TRAVEL[1] - TRAVEL[0]:.2f} in; wheel plane x {X_WHEEL:.2f} in; handle top z {Z_HANDLE + max(v for _, v in HANDLE):.2f} in")
    with open(os.path.join(OUT, "pose.json"), "w") as f:
        json.dump({"slide_sign": slide_sign, "pivot_sign": pivot_sign, "plunge": plunge, "depth": depth, "travel": TRAVEL, "y_pivot": Y_PIVOT,
                   "x_wheel": X_WHEEL, "lever": LEVER, "fence_rows": FENCE_ROWS}, f, indent=1)

    shot = lambda name, **kw: mcp.call("screenshot", {"tab": saw, "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot("iso", view="-0.6,-0.7,0.4")
    shot("iso_back", view="0.7,0.6,0.45")
    shot("front", view="front")
    shot("side", view="right")
    shot("top", view="top")
    shot("carriage_detail", view="-0.6,-0.7,0.4", fit=f"{(X_RAIL - 1) * IN:.0f},{(Y_PIVOT - 6) * IN:.0f},{(BASE_TOP) * IN:.0f},{(X_MOUNT + 2) * IN:.0f},{(Y_PIVOT + 5) * IN:.0f},{15 * IN:.0f}")
    shot("pivot_section", view="0,-1,0.0001", section=f"y:{Y_PIVOT * IN:.1f}", fit=f"{(X_ARM - 1) * IN:.0f},{(Y_PIVOT - 4) * IN:.0f},{BASE_TOP * IN:.0f},{(X_WHEEL + 1) * IN:.0f},{(Y_PIVOT + 4) * IN:.0f},{12 * IN:.0f}")
    set_pose(Y_PIVOT, plunge)
    shot("plunged", view="-0.6,-0.7,0.4")
    shot("cut_detail", view="1,-0.5,0.3", fit=f"{(X_WHEEL - 5) * IN:.0f},{(Y_SPINDLE - 4) * IN:.0f},0,{(X_WHEEL + 4) * IN:.0f},{(Y_SPINDLE + 5) * IN:.0f},{9 * IN:.0f}")
    set_pose(Y_PIVOT, 0.0)
    swing = [{"pivot": pivot_m["angle"] + pivot_sign * d, "label": "at rest" if d == 0 else f"down {d:g} degrees"} for d in (0.0, 10.0, round(plunge, 1))]
    print(mcp.call("range_of_motion", {"tab": saw, "positions": swing, "view": "right", "format": "pdf", "sheet": "A3",
                                       "note": "the handle at rest on its stop, part way, and plunged to the channel floor", "path": os.path.join(OUT, "swing.pdf")}))
    mcp.call("range_of_motion", {"tab": saw, "positions": swing, "view": "right", "width": 1200, "height": 700, "path": os.path.join(OUT, "swing.png")})
    travel = [{"slide": slide_m["offset"] + slide_sign * (y - Y_PIVOT) * IN, "label": label} for y, label in ((TRAVEL[0], "forward stop"), (Y_PIVOT, "middle"), (TRAVEL[1], "back stop"))]
    mcp.call("range_of_motion", {"tab": saw, "positions": travel, "view": "right", "width": 1200, "height": 700, "path": os.path.join(OUT, "travel.png")})
    print(mcp.call("range_of_motion", {"tab": saw, "positions": travel, "view": "right", "format": "pdf", "sheet": "A3",
                                       "note": "the carriage at its front stop, the middle and its back stop", "path": os.path.join(OUT, "travel.pdf")}))
    print(mcp.call("export", {"tab": saw, "format": "pdf", "sheet": "A2", "views": ["front", "top", "right", "iso", f"section-side@{Y_PIVOT * IN:.1f}"], "explode": 0.6,
                              "note": "adapted from Woodsmith WS22134 to the shop's grinder; see README", "path": os.path.join(OUT, "cutoff_grinder.pdf")}))
    for name, tab in (("carriage_assembly", carriage_asm), ("handle_assembly", handle_asm)):
        print(mcp.call("export", {"tab": tab, "format": "pdf", "sheet": "A3", "explode": 0.8, "path": os.path.join(OUT, f"{name}.pdf")}))
    mcp.call("export", {"tab": tabs["handle"], "format": "dxf", "view": "right", "path": os.path.join(OUT, "handle.dxf")})
    with open(os.path.join(OUT, "parts.csv"), "w") as f:
        f.write("kind,item,size,qty,note\n")
        for kind, rows in (("wood", CUTLIST), ("hardware", HARDWARE)):
            for item, size, qty, note in rows:
                f.write(f'{kind},"{item}","{size}",{qty},"{note}"\n')
    print(f"parts: {len(CUTLIST)} wood lines, {len(HARDWARE)} hardware lines")
    mcp.close()


if __name__ == "__main__":
    main()
