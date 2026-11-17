#!/usr/bin/env python3
"""A two-axis router positioner: boxes sliding in boxes, a screw down the middle of each.

BRIEF.md is the design brief this is built from. A trim router lies on
its side in a square tube, the quill, which slides along the bit's axis
(X, plunge) through a sleeve in a second box, the Z stage, which slides
up and down (Z, bit height) in a fixed outer box. Each axis is one lead
screw on the centre of the box it moves, its nut in that box and its
motor fixed to the end cap of a column: the X column an extension of
the sleeve out of the Z stage's right side, the Z column under the
outer box.

This is the first pass the brief asks for: proportions derived from a
few numbers at the top, the brief's rules enforced, the four corners of
the travel checked, and the envelope reported, so the trade-offs can be
looked at before anything is detailed. What is a guess says so.

    cargo build --release -p ok-mcp
    python3 examples/router-positioner/build.py
    cargo test -p ok-render --test router_positioner

Writes out/positioner.okpart (every part in its own tab, and the module
assembled at the four corners of its travel), positioner.png,
section-home.png and section-plunged-down.png, positioner.pdf and
positioner.json.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")

# -- the numbers to set -------------------------------------------------------
# The router: a Bosch Colt PR20EVS, if this module uses the one on hand (the brief's first open question).
# Bosch publishes no motor dimensions, so every one of these wants callipers.
ROUTER_D = 65.0                 # the housing a clamp grips: what examples/router-lift takes the Colt's to be. A guess
ROUTER_ENV = 80.0               # the widest thing on it, the top cap and its switch: the quill has to pass it. A guess
ROUTER_L = 190.0                # from the collet nut's seat to the back cap: retailers give 7.5 in for the whole tool
ROUTER_HOUSING = 150.0          # how much of that is the round housing, the rest being the cap. A guess
COLLET_D, COLLET_L = 22.0, 16.0  # the collet nut, as examples/router-lift has it. Measure
BIT_D, BIT_L = 8.0, 40.0        # a mortising bit past the nut, for the pictures and the slot

X_TRAVEL = 60.0                 # plunge. The brief leaves it open: a 50 mm mortise and room to clear the work
Z_TRAVEL = 80.0                 # bit height. Open too

WALL = 8.0                      # the sliding boxes' walls: rigidity before print time, the brief says
OUTER_WALL = 10.0               # the outer box's
GAP = 1.5                       # between a box and its guide all round: a bearing strip thick, and room for dust
STRIP_L = 25.0                  # a bearing strip along the travel, one near each end of a box
GIB = (4.0, 20.0)               # a gib strip: thick, wide. It stands GAP out of its pocket
SET_SCREW = 4.2                 # an M5 set screw's tapping hole, three to a gib
ENGAGE = 1.5                    # a box is in its guide at least this many of its own widths, everywhere in its travel
RAIL = 16.0                     # the Z stage is this much wider than the X column each side: see `rules`

SCREW_D, SCREW_PITCH = 8.0, 1.25  # M8 rod. Open: a T8 lead screw of 2 mm lead goes in the same bores
NUT_AF, NUT_L = 13.0, 24.0      # an M8 coupling nut, DIN 6334: across flats, long. Brass, or it galls
NUT_FLOAT = 0.5                 # the nut's play across its pocket, so a bent rod does not bind
NUT_LEAD = 3.0                  # the screw stands this far through the nut at the far end of the travel
TIP_CLEAR = 4.0                 # and its tip stops this far short of what it is run toward
BEARING = (16.0, 5.0)           # a thrust bearing, F8-16M: across, thick
COLLAR_L = 8.0                  # a collar on the screw each side of the bulkhead
COUPLING = (19.0, 25.0)         # a rigid coupling, 5 to 8 mm
MOTOR = (42.3, 40.0, 24.0)      # NEMA 17: square, long, and its shaft. Assumed, as the brief does
BULKHEAD, CAP = 12.0, 6.0       # the wall the thrust bearing sits against, and the end cap the motor is on
SWITCH = (20.0, 10.0, 6.0)      # a limit switch

SLOT_W = 44.0                   # the bit slot: the collet nut and a finger's room each side
BED = 256.0                     # the printer, a Bambu P1S: nothing longer prints in one piece

# -- what follows from them ---------------------------------------------------
QI = ROUTER_ENV + 2.0                           # the quill's bore: the router goes in from the back
QO = QI + 2 * WALL                              # the quill
SB = QO + 2 * GAP                               # the sleeve's bore, and the column's
CO = SB + 2 * WALL                              # the column, outside
SPIDER_T = 12.0                                 # the spider's plate
BOSS = NUT_L + 4.0                              # a nut boss: the nut and 2 mm each end
BOSS_D = 30.0
REC = COLLAR_L + BEARING[1] + 3.0               # a drive housing's recess: the collar and bearing on the nut's side of the bulkhead
CHAMBER = COLLAR_L + 2.0 + COUPLING[1] + 5.0    # and its chamber: the other collar, the coupling, the motor's shaft
DRIVE = REC + BULKHEAD + CHAMBER                # a drive housing, end to end

MX = CO                                         # the Z stage along the bit: as long as the column is wide
MD = CO + 2 * RAIL                              # and across: the column and a rail each side
Z_CLEAR = Z_TRAVEL + NUT_LEAD + TIP_CLEAR + 1.0  # under the sleeve, in the stage: where the Z screw goes
MH = WALL + SB + WALL + Z_CLEAR + BOSS          # the stage: lid, sleeve, sleeve floor, the screw's room, the nut boss
IH = MH + Z_TRAVEL + 2.0                        # the outer box inside: the stage, its travel, a millimetre each end

# Where things are with both axes at home: X drawn back, Z up. x along the bit from the sleeve's mouth, the bit pointing to -x;
# y front (-) to back (+) from the bit's axis; z up from the outer box's floor.
ZB = Z_TRAVEL + 1.0                             # the stage's foot
AZ = ZB + MH - WALL - SB / 2                    # the bit's axis
XQ = X_TRAVEL                                   # the quill's front: at full plunge it is level with the sleeve's mouth
X_ROUTER_BACK = XQ + ROUTER_L
X_TIP = X_ROUTER_BACK + TIP_CLEAR               # the X screw's tip: it does not move
X_NUT = X_TIP + NUT_LEAD + X_TRAVEL             # the X nut's near end, drawn back
X_SPIDER = X_NUT - 2.0                          # the spider's plate starts 2 mm before its nut
QL = X_SPIDER + SPIDER_T - XQ                   # the quill, so
X_BOSS_END = X_SPIDER + BOSS
X_COL_END = X_BOSS_END + 3.0                    # the column ends where the X drive housing begins
X_END = X_COL_END + DRIVE + CAP + MOTOR[1]      # the X motor's far end
Z_NUT = ZB + 2.0                                # the Z nut's lower end, up
Z_TIP = Z_NUT + NUT_L + NUT_LEAD                # the Z screw's tip
Z_END = -OUTER_WALL - DRIVE - CAP - MOTOR[1]    # the Z motor's far end
BOX_X = (-GAP - OUTER_WALL, MX + GAP + OUTER_WALL)
BOX_Y = MD / 2 + GAP + OUTER_WALL
SLOT_Z = (AZ - Z_TRAVEL - SLOT_W / 2, AZ + SLOT_W / 2)
COL_SLOT_Z = (1.0 + MH - CO - 1.5, IH)          # the column's way through the outer box's right wall


def rules():
    """The brief's rules of thumb, as numbers; stops the build if one is broken."""
    found = {}
    # A box in its guide: 1.5 widths at least. The quill is guided by sleeve and column together, the column being the
    # sleeve carried on; the stage by the outer box.
    found["x_engaged"] = QL / QO
    found["z_engaged"] = MH / max(MX, MD)
    # The quill's two bearing zones stay in their guides: the front one in the sleeve, the back one in the column.
    front = (XQ - X_TRAVEL + 2.0, XQ + 2.0 + STRIP_L)
    back = (XQ + QL - 2.0 - STRIP_L - X_TRAVEL, XQ + QL - 2.0)
    found["front_strip_in_sleeve"] = min(front[0], MX - front[1])
    found["back_strip_in_column"] = min(back[0] - MX, X_COL_END - back[1])
    # The screws: through the nut at one end of the travel, short of what they run toward at the other.
    found["x_tip_to_router"] = X_TIP - X_ROUTER_BACK
    found["x_through_nut"] = (X_NUT - X_TRAVEL) - X_TIP
    found["z_tip_to_sleeve"] = (1.0 + BOSS + Z_CLEAR) - Z_TIP          # the stage down
    found["z_through_nut"] = Z_TIP - (Z_NUT + NUT_L)                   # the stage up
    # The column in the outer box's right wall, the stage up and down.
    found["column_in_slot"] = min((1.0 + MH - CO) - COL_SLOT_Z[0], COL_SLOT_Z[1] - (ZB + MH))
    # At full plunge the bit is at the sleeve's mouth, and the collet nut this far out of the outer box's face.
    found["collet_proud"] = COLLET_L - (GAP + OUTER_WALL)
    bad = []
    if min(found["x_engaged"], found["z_engaged"]) < ENGAGE:
        bad.append(f"a box is under {ENGAGE} widths in its guide: {found['x_engaged']:.2f} along X, {found['z_engaged']:.2f} along Z")
    for key, least in (("front_strip_in_sleeve", 0.0), ("back_strip_in_column", 0.0), ("x_tip_to_router", 3.0), ("x_through_nut", 2.0), ("z_tip_to_sleeve", 3.0),
                       ("z_through_nut", 2.0), ("column_in_slot", 1.0)):
        if found[key] < least - 1e-9:
            bad.append(f"{key} is {found[key]:.1f}, under {least}")
    if bad:
        raise SystemExit("; ".join(bad))
    return found


# -- drawing ------------------------------------------------------------------

def box(p, name, x, y, z, op="new"):
    s = p.sketch("right", x[0], name)
    p.polygon(s, [(y[0], z[0]), (y[1], z[0]), (y[1], z[1]), (y[0], z[1])])
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def xcyl(p, name, x, c, d, op="new"):
    s = p.sketch("right", x[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def xhex(p, name, x, c, af, op="new"):
    s = p.sketch("right", x[0], name)
    r = af / 2 / math.cos(math.radians(30))
    p.polygon(s, [(c[0] + r * math.cos(math.radians(60 * k)), c[1] + r * math.sin(math.radians(60 * k))) for k in range(6)])
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def zbox(p, name, x, y, z, op="new"):
    s = p.sketch("top", z[0], name)
    p.polygon(s, [(x[0], y[0]), (x[1], y[0]), (x[1], y[1]), (x[0], y[1])])
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def zcyl(p, name, z, c, d, op="new"):
    s = p.sketch("top", z[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def zhex(p, name, z, c, af, op="new"):
    s = p.sketch("top", z[0], name)
    r = af / 2 / math.cos(math.radians(30))
    p.polygon(s, [(c[0] + r * math.cos(math.radians(60 * k)), c[1] + r * math.sin(math.radians(60 * k))) for k in range(6)])
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def ybore(p, name, c, d, y_far, depth):
    """A hole along y from y_far toward the front, `depth` deep, at (x, z)."""
    s = p.sketch("front", -y_far, name)
    p.circle(s, c, d / 2)
    p.cut(s, depth, name=name)


def spread(lo, hi, n=3, inset=15.0):
    return [lo + inset + (hi - lo - 2 * inset) * k / (n - 1) for k in range(n)]


# The gibs: which wall of which guide each is let into. The opposite walls are the reference faces.
# The Z stage is pushed back (the feed pushes the bit that way) and to the right, against the two rails (the router's
# weight and the plunge both tip it that way). The quill is pushed back and down.
# The two in the left wall lie outside the sleeve's mouth, where the stage has a face all the way up, each straight across from a rail.
Z_GIBS = [("left", -(SB / 2 + 1.5 + GIB[1] / 2)), ("left", SB / 2 + 1.5 + GIB[1] / 2), ("front", MX / 2)]
Z_SWITCH_Y = (MD / 2 + GAP - 3.0 - SWITCH[1], MD / 2 + GAP - 3.0)


def outer_box(p):
    box(p, "outer box", BOX_X, (-BOX_Y, BOX_Y), (-OUTER_WALL, IH + OUTER_WALL))
    box(p, "inside: the Z guide", (-GAP, MX + GAP), (-MD / 2 - GAP, MD / 2 + GAP), (0.0, IH), op="remove")
    box(p, "bit slot", (BOX_X[0] - 1.0, -GAP + 1.0), (-SLOT_W / 2, SLOT_W / 2), SLOT_Z, op="remove")
    box(p, "the X column's way", (MX + GAP - 1.0, BOX_X[1] + 1.0), (-CO / 2 - 1.5, CO / 2 + 1.5), COL_SLOT_Z, op="remove")
    box(p, "the Z switch's pocket", (7.5, 8.5 + SWITCH[0]), (Z_SWITCH_Y[0] - 0.5, Z_SWITCH_Y[1] + 0.5), (IH - 1.0, IH + SWITCH[2]), op="remove")
    zcyl(p, "the Z screw's way", (-OUTER_WALL - 1.0, 1.0), (MX / 2, 0.0), SCREW_D + 1.0, op="remove")
    sink = GIB[0] - GAP + 0.5
    for wall, at in Z_GIBS:
        if wall == "left":
            box(p, "gib pocket", (-GAP - sink, -GAP + 0.5), (at - GIB[1] / 2 - 0.25, at + GIB[1] / 2 + 0.25), (8.0, IH - 8.0), op="remove")
            for z in spread(8.0, IH - 8.0):
                xcyl(p, "set screw", (BOX_X[0] - 1.0, -GAP), (at, z), SET_SCREW, op="remove")
        else:
            box(p, "gib pocket", (at - GIB[1] / 2 - 0.25, at + GIB[1] / 2 + 0.25), (-MD / 2 - GAP - sink, -MD / 2 - GAP + 0.5), (8.0, IH - 8.0), op="remove")
            for z in spread(8.0, IH - 8.0):
                ybore(p, "set screw", (at, z), SET_SCREW, -MD / 2 - GAP, OUTER_WALL + 1.0)


def z_gibs(p):
    for wall, at in Z_GIBS:
        if wall == "left":
            box(p, "Z gib", (-GIB[0], 0.0), (at - GIB[1] / 2, at + GIB[1] / 2), (9.0, IH - 9.0))
        else:
            box(p, "Z gib", (at - GIB[1] / 2, at + GIB[1] / 2), (-MD / 2 - GIB[0], -MD / 2), (9.0, IH - 9.0))


def z_stage(p):
    top = ZB + MH
    box(p, "Z stage", (0.0, MX), (-MD / 2, MD / 2), (ZB, top))
    box(p, "sleeve: the X guide", (-1.0, MX + 1.0), (-SB / 2, SB / 2), (top - WALL - SB, top - WALL), op="remove")
    box(p, "the Z screw's room", (WALL, MX + 1.0), (-SB / 2, SB / 2), (ZB + BOSS, ZB + BOSS + Z_CLEAR), op="remove")
    zhex(p, "nut pocket", (ZB - 1.0, ZB + 2.0 + NUT_L + 0.5), (MX / 2, 0.0), NUT_AF + 2 * NUT_FLOAT, op="remove")
    zcyl(p, "the screw's way", (ZB + NUT_L, ZB + BOSS + 1.0), (MX / 2, 0.0), SCREW_D + 1.0, op="remove")
    # bearing strips on the two reference sides, one near each end: against the rails, and against the back wall
    for z in ((ZB + 2.0, ZB + 2.0 + STRIP_L), (top - 2.0 - STRIP_L, top - 2.0)):
        for y in ((-MD / 2, -CO / 2 - 3.0), (CO / 2 + 3.0, MD / 2)):
            box(p, "strip on a rail", (MX, MX + GAP), y, z, op="add")
        box(p, "strip on the back wall", (2.0, MX - 2.0), (MD / 2, MD / 2 + GAP), z, op="add")
    # the quill's gibs, in the sleeve's lid and its front wall
    sink = GIB[0] - GAP + 0.5
    box(p, "gib pocket", (8.0, MX - 8.0), (-GIB[1] / 2 - 0.25, GIB[1] / 2 + 0.25), (top - WALL - 0.5, top - WALL + sink), op="remove")
    box(p, "gib pocket", (8.0, MX - 8.0), (-SB / 2 - sink, -SB / 2 + 0.5), (AZ - GIB[1] / 2 - 0.25, AZ + GIB[1] / 2 + 0.25), op="remove")
    for x in spread(8.0, MX - 8.0):
        zcyl(p, "set screw", (top - WALL, top + 1.0), (x, 0.0), SET_SCREW, op="remove")
        ybore(p, "set screw", (x, AZ), SET_SCREW, -SB / 2, RAIL + WALL + 1.0)


def x_column(p):
    box(p, "X column", (MX, X_COL_END), (-CO / 2, CO / 2), (AZ - CO / 2, AZ + CO / 2))
    box(p, "bore: the sleeve carried on", (MX - 1.0, X_COL_END + 1.0), (-SB / 2, SB / 2), (AZ - SB / 2, AZ + SB / 2), op="remove")
    box(p, "the cord's slot", (X_ROUTER_BACK - X_TRAVEL - 10.0, X_COL_END - 30.0), (-CO / 2 - 1.0, -SB / 2 + 1.0), (AZ + GIB[1] / 2 + 6.0, AZ + GIB[1] / 2 + 20.0), op="remove")
    for y in ((-SB / 2 + 6.0, -GIB[1] / 2 - 6.0), (GIB[1] / 2 + 6.0, SB / 2 - 6.0)):   # air in through the lid at the back, each side of the gib; the spider lets it through to the router
        zbox(p, "vent", (X_COL_END - 70.0, X_COL_END - 20.0), y, (AZ + SB / 2 - 1.0, AZ + CO / 2 + 1.0), op="remove")
    lo, hi = XQ + QL - X_TRAVEL - STRIP_L - 10.0, X_COL_END - 8.0       # the gibs lie where the quill's back end runs
    sink = GIB[0] - GAP + 0.5
    box(p, "gib pocket", (lo, hi), (-GIB[1] / 2 - 0.25, GIB[1] / 2 + 0.25), (AZ + SB / 2 - 0.5, AZ + SB / 2 + sink), op="remove")
    box(p, "gib pocket", (lo, hi), (-SB / 2 - sink, -SB / 2 + 0.5), (AZ - GIB[1] / 2 - 0.25, AZ + GIB[1] / 2 + 0.25), op="remove")
    for x in spread(lo, hi):
        zcyl(p, "set screw", (AZ + SB / 2, AZ + CO / 2 + 1.0), (x, 0.0), SET_SCREW, op="remove")
        ybore(p, "set screw", (x, AZ), SET_SCREW, -SB / 2, WALL + 1.0)


def x_gibs(p):
    lo, hi = XQ + QL - X_TRAVEL - STRIP_L - 9.0, X_COL_END - 9.0
    for x in ((9.0, MX - 9.0), (lo, hi)):
        box(p, "X gib, over the quill", x, (-GIB[1] / 2, GIB[1] / 2), (AZ + QO / 2, AZ + QO / 2 + GIB[0]))
        box(p, "X gib, before it", x, (-QO / 2 - GIB[0], -QO / 2), (AZ - GIB[1] / 2, AZ + GIB[1] / 2))


def quill(p):
    box(p, "X quill", (XQ, XQ + QL), (-QO / 2, QO / 2), (AZ - QO / 2, AZ + QO / 2))
    box(p, "bore: the router's", (XQ - 1.0, XQ + QL + 1.0), (-QI / 2, QI / 2), (AZ - QI / 2, AZ + QI / 2), op="remove")
    box(p, "the cord's slot", (X_ROUTER_BACK - 10.0, X_SPIDER - 4.0), (-QO / 2 - 1.0, -QI / 2 + 1.0), (AZ + GIB[1] / 2 + 6.0, AZ + GIB[1] / 2 + 20.0), op="remove")
    for x in ((XQ + 2.0, XQ + 2.0 + STRIP_L), (XQ + QL - 2.0 - STRIP_L, XQ + QL - 2.0)):
        box(p, "strip underneath", x, (-QO / 2 + 4.0, QO / 2 - 4.0), (AZ - QO / 2 - GAP, AZ - QO / 2), op="add")
        box(p, "strip behind", x, (QO / 2, QO / 2 + GAP), (AZ - QO / 2 + 4.0, AZ + QO / 2 - 4.0), op="add")


def spider(p):
    """A plate across the back of the quill: a rim, four ribs, and the nut boss on the centre. Open between the ribs."""
    fit, rim, rib = QI / 2 - 0.2, 6.0, 14.0
    box(p, "spider", (X_SPIDER, X_SPIDER + SPIDER_T), (-fit, fit), (AZ - fit, AZ + fit))
    for sy in (-1, 1):
        for sz in (-1, 1):
            y = sorted((sy * rib / 2, sy * (fit - rim)))
            z = sorted((AZ + sz * rib / 2, AZ + sz * (fit - rim)))
            box(p, "open, for air and the cord", (X_SPIDER - 1.0, X_SPIDER + SPIDER_T + 1.0), y, z, op="remove")
    xcyl(p, "nut boss", (X_SPIDER, X_SPIDER + BOSS), (0.0, AZ), BOSS_D, op="add")
    xhex(p, "nut pocket", (X_NUT, X_SPIDER + BOSS + 1.0), (0.0, AZ), NUT_AF + 2 * NUT_FLOAT, op="remove")
    xcyl(p, "the screw's way", (X_SPIDER - 1.0, X_NUT + 1.0), (0.0, AZ), SCREW_D + 1.0, op="remove")


def router(p):
    xcyl(p, "router housing", (XQ, XQ + ROUTER_HOUSING), (0.0, AZ), ROUTER_D)
    xcyl(p, "router cap", (XQ + ROUTER_HOUSING, X_ROUTER_BACK), (0.0, AZ), ROUTER_ENV, op="add")
    xcyl(p, "collet nut", (XQ - COLLET_L, XQ), (0.0, AZ), COLLET_D, op="add")
    xcyl(p, "bit", (XQ - COLLET_L - BIT_L, XQ - COLLET_L), (0.0, AZ), BIT_D, op="add")


def clamps(p):
    """Two collars round the housing, a close fit in the quill. How they are tightened is the brief's sixth open question."""
    fit = QI / 2 - 0.2
    for x0 in (XQ + 15.0, XQ + ROUTER_HOUSING - 35.0):
        box(p, "collar", (x0, x0 + 20.0), (-fit, fit), (AZ - fit, AZ + fit))
        xcyl(p, "the housing's bore", (x0 - 0.5, x0 + 20.5), (0.0, AZ), ROUTER_D + 0.6, op="remove")


def drive_housing(p, axis):
    """The end of a column: a recess for the collar and bearing, the bulkhead the bearing sits against, a chamber for
    the coupling, and the cap the motor is on. The same part on both axes."""
    half, inner = CO / 2, CO / 2 - WALL
    if axis == "x":
        a = X_COL_END
        box(p, "drive housing", (a, a + DRIVE), (-half, half), (AZ - half, AZ + half))
        xcyl(p, "recess", (a - 1.0, a + REC), (0.0, AZ), BEARING[0] + 6.0, op="remove")
        xcyl(p, "the screw's way", (a + REC - 1.0, a + REC + BULKHEAD + 1.0), (0.0, AZ), SCREW_D + 1.0, op="remove")
        box(p, "chamber", (a + REC + BULKHEAD, a + DRIVE + 1.0), (-inner, inner), (AZ - inner, AZ + inner), op="remove")
        box(p, "end cap", (a + DRIVE, a + DRIVE + CAP), (-half, half), (AZ - half, AZ + half))
        xcyl(p, "motor's pilot", (a + DRIVE - 0.5, a + DRIVE + CAP + 0.5), (0.0, AZ), 22.5, op="remove")
    else:
        a, c = -OUTER_WALL, MX / 2
        zbox(p, "drive housing", (c - half, c + half), (-half, half), (a - DRIVE, a))
        zcyl(p, "recess", (a - REC, a + 1.0), (c, 0.0), BEARING[0] + 6.0, op="remove")
        zcyl(p, "the screw's way", (a - REC - BULKHEAD - 1.0, a - REC + 1.0), (c, 0.0), SCREW_D + 1.0, op="remove")
        zbox(p, "chamber", (c - inner, c + inner), (-inner, inner), (a - DRIVE - 1.0, a - REC - BULKHEAD), op="remove")
        zbox(p, "end cap", (c - half, c + half), (-half, half), (a - DRIVE - CAP, a - DRIVE))
        zcyl(p, "motor's pilot", (a - DRIVE - CAP - 0.5, a - DRIVE + 0.5), (c, 0.0), 22.5, op="remove")


def drive_parts(p, axis):
    """What turns: collar, thrust bearing, the other collar, the coupling, the motor, and last the screw through them."""
    bore = SCREW_D + 0.4
    if axis == "x":
        a, cyl, c = X_COL_END, xcyl, (0.0, AZ)
        rng = lambda u0, u1: (a + u0, a + u1)
    else:
        a, cyl, c = -OUTER_WALL, zcyl, (MX / 2, 0.0)
        rng = lambda u0, u1: (a - u1, a - u0)
    rings = (("collar", REC - BEARING[1] - COLLAR_L, REC - BEARING[1], BEARING[0]), ("thrust bearing", REC - BEARING[1], REC, BEARING[0]),
             ("collar", REC + BULKHEAD, REC + BULKHEAD + COLLAR_L, BEARING[0]), ("coupling", REC + BULKHEAD + COLLAR_L + 2.0, REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1], COUPLING[0]))
    for name, u0, u1, d in rings:
        cyl(p, name, rng(u0, u1), c, d)
        cyl(p, "bore", rng(u0 - 0.2, u1 + 0.2), c, bore, op="remove")
    m = MOTOR[0] / 2
    if axis == "x":
        box(p, "motor", rng(DRIVE + CAP, DRIVE + CAP + MOTOR[1]), (-m, m), (AZ - m, AZ + m))
        xcyl(p, "motor shaft", rng(DRIVE + CAP - MOTOR[2], DRIVE + CAP), c, 5.0)      # a body of its own: joined to the motor it would take the coupling round it too
        xcyl(p, "X screw", (X_TIP, a + REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1] / 2 - 1.0), c, SCREW_D)
    else:
        zbox(p, "motor", (MX / 2 - m, MX / 2 + m), (-m, m), rng(DRIVE + CAP, DRIVE + CAP + MOTOR[1]))
        zcyl(p, "motor shaft", rng(DRIVE + CAP - MOTOR[2], DRIVE + CAP), c, 5.0)
        zcyl(p, "Z screw", (a - (REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1] / 2 - 1.0), Z_TIP), c, SCREW_D)


def nut(p, axis):
    if axis == "x":
        xhex(p, "X nut", (X_NUT, X_NUT + NUT_L), (0.0, AZ), NUT_AF)
        xcyl(p, "thread", (X_NUT - 0.5, X_NUT + NUT_L + 0.5), (0.0, AZ), SCREW_D + 0.4, op="remove")
    else:
        zhex(p, "Z nut", (Z_NUT, Z_NUT + NUT_L), (MX / 2, 0.0), NUT_AF)
        zcyl(p, "thread", (Z_NUT - 0.5, Z_NUT + NUT_L + 0.5), (MX / 2, 0.0), SCREW_D + 0.4, op="remove")


def x_switch(p):
    """Home is the quill drawn back: its end closes the switch, on the column's floor."""
    x0 = XQ + QL + 1.0
    box(p, "X home switch", (x0, x0 + SWITCH[2]), (SB / 2 - 4.0 - SWITCH[1], SB / 2 - 4.0), (AZ - SB / 2, AZ - SB / 2 + SWITCH[0]))


def z_switch(p):
    """Home is the stage up, away from the end where its screw would meet the sleeve: its lid closes the switch."""
    box(p, "Z home switch", (8.0, 8.0 + SWITCH[0]), Z_SWITCH_Y, (IH - 0.5, IH - 0.5 + SWITCH[2]))


# Every part: its tab, what draws it, and what it moves with.
PARTS = [("Outer box", outer_box, "fixed"), ("Z gibs", z_gibs, "fixed"), ("Z drive housing", lambda p: drive_housing(p, "z"), "fixed"),
         ("Z drive", lambda p: drive_parts(p, "z"), "fixed"), ("Z home switch", z_switch, "fixed"),
         ("Z stage", z_stage, "z"), ("Z nut", lambda p: nut(p, "z"), "z"), ("X column", x_column, "z"), ("X gibs", x_gibs, "z"),
         ("X drive housing", lambda p: drive_housing(p, "x"), "z"), ("X drive", lambda p: drive_parts(p, "x"), "z"), ("X home switch", x_switch, "z"),
         ("X quill", quill, "xz"), ("Spider", spider, "xz"), ("X nut", lambda p: nut(p, "x"), "xz"), ("Router", router, "xz"), ("Router clamps", clamps, "xz")]
# The four corners of the travel: how far the quill is run out, and how far the stage is run down.
CORNERS = [("Home: drawn back, up", 0.0, 0.0), ("Plunged, up", X_TRAVEL, 0.0), ("Drawn back, down", 0.0, Z_TRAVEL), ("Plunged, down", X_TRAVEL, Z_TRAVEL)]


def main():
    found = rules()
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "positioner.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "positioner"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Two-axis nested-box router positioner"}, {"type": "rename_tab", "tab": 1, "name": PARTS[0][0]}]})
    tabs, bodies = {}, {}
    for i, (title, make, _) in enumerate(PARTS):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
    corners = {}
    for label, dx, dz in CORNERS:
        asm = new_tab(mcp, {"type": "add_assembly", "name": label})
        ops = []
        for title, _, moves in PARTS:
            at = {"x": -dx if moves == "xz" else 0.0, "y": 0.0, "z": -dz if moves in ("z", "xz") else 0.0}
            for k in range(len(bodies[title])):
                ops.append({"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[title], "body": k, "name": title if len(bodies[title]) == 1 else f"{title} {k + 1}",
                                                                      "fixed": True, "placement": {"position": at, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}}})
        text = mcp.call("apply", {"ops": ops})
        if "ERROR:" in text:
            raise SystemExit(text)
        corners[label] = asm
    home, far = corners[CORNERS[0][0]], corners[CORNERS[3][0]]
    mcp.call("screenshot", {"tab": home, "view": "-0.55,-0.7,0.45", "width": 1500, "height": 1100, "path": os.path.join(OUT, "positioner.png")})
    fit = f"{BOX_X[0] - X_TRAVEL - 20},-1,{Z_END - 10},{X_END + 10},1,{IH + OUTER_WALL + 10}"
    mcp.call("screenshot", {"tab": home, "view": "0,-1,0", "section": "y:0", "fit": fit, "width": 1500, "height": 1100, "path": os.path.join(OUT, "section-home.png")})
    mcp.call("screenshot", {"tab": far, "view": "0,-1,0", "section": "y:0", "fit": fit, "width": 1500, "height": 1100, "path": os.path.join(OUT, "section-plunged-down.png")})
    print(mcp.call("export", {"tab": home, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "positioner.pdf"),
                              "note": f"two-axis router positioner, first pass: X {X_TRAVEL:.0f}, Z {Z_TRAVEL:.0f} of travel; every router dimension a guess"}))
    mcp.close()

    # -- the report the brief asks for -------------------------------------------
    sizes = {"Outer box": (BOX_X[1] - BOX_X[0], 2 * BOX_Y, IH + 2 * OUTER_WALL), "Z stage": (MX + GAP, MD + GAP, MH), "X column": (X_COL_END - MX, CO, CO),
             "X quill": (QL, QO + GAP, QO + GAP), "Spider": (BOSS, QI - 0.4, QI - 0.4), "Drive housing": (DRIVE, CO, CO), "End cap": (CAP, CO, CO)}
    too_long = sorted(k for k, v in sizes.items() if max(v) > BED)
    envelope = {"x": (BOX_X[0], X_END), "y": (-BOX_Y, BOX_Y), "z": (Z_END, IH + OUTER_WALL)}
    numbers = {
        "router": {"d": ROUTER_D, "envelope": ROUTER_ENV, "length": ROUTER_L, "housing": ROUTER_HOUSING, "collet": [COLLET_D, COLLET_L], "bit": [BIT_D, BIT_L]},
        "travel": {"x": X_TRAVEL, "z": Z_TRAVEL}, "wall": WALL, "outer_wall": OUTER_WALL, "gap": GAP, "strip_l": STRIP_L, "gib": GIB, "engage": ENGAGE, "rail": RAIL,
        "screw": {"d": SCREW_D, "pitch": SCREW_PITCH, "per_step": SCREW_PITCH / 200, "nut": [NUT_AF, NUT_L], "float": NUT_FLOAT, "lead": NUT_LEAD, "tip_clear": TIP_CLEAR},
        "quill": {"bore": QI, "outside": QO, "length": QL, "front": XQ}, "sleeve": {"bore": SB, "outside": CO},
        "stage": {"x": MX, "y": MD, "height": MH, "foot": ZB, "screw_room": Z_CLEAR, "boss": BOSS}, "box": {"x": BOX_X, "y": BOX_Y, "inside_height": IH, "slot_z": SLOT_Z, "slot_w": SLOT_W, "column_slot_z": COL_SLOT_Z},
        "axis_z": AZ, "x": {"router_back": X_ROUTER_BACK, "tip": X_TIP, "nut": X_NUT, "spider": X_SPIDER, "column_end": X_COL_END, "end": X_END},
        "z": {"nut": Z_NUT, "tip": Z_TIP, "end": Z_END}, "drive": {"recess": REC, "bulkhead": BULKHEAD, "chamber": CHAMBER, "length": DRIVE, "cap": CAP, "motor": MOTOR},
        "rules": found, "envelope": envelope, "size": [envelope[k][1] - envelope[k][0] for k in "xyz"],
        "behind_router": {"drawn_back": X_END - X_ROUTER_BACK, "plunged": X_END - X_ROUTER_BACK + X_TRAVEL},
        "out_of_the_box": X_END - BOX_X[1], "bed": BED, "sizes": sizes, "too_long_to_print": too_long,
        "parts": [{"tab": t, "moves": m, "bodies": [list(b) for b in bodies[t]]} for t, _, m in PARTS], "corners": [[c[0], c[1], c[2]] for c in CORNERS],
    }
    with open(os.path.join(OUT, "positioner.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"envelope {numbers['size'][0]:.0f} long x {numbers['size'][1]:.0f} deep x {numbers['size'][2]:.0f} tall; {numbers['out_of_the_box']:.0f} of that is column and motor out of the box's right side; "
          f"{numbers['behind_router']['drawn_back']:.0f} behind the router drawn back")
    print(f"quill {QO:.0f} square x {QL:.0f}, in its guide {found['x_engaged']:.2f} widths; Z stage {MX:.0f} x {MD:.0f} x {MH:.0f}, {found['z_engaged']:.2f} widths; outer box {sizes['Outer box'][0]:.0f} x {sizes['Outer box'][1]:.0f} x {sizes['Outer box'][2]:.0f}")
    print(f"X screw tip {found['x_tip_to_router']:.0f} from the router's back, drawn back; Z screw tip {found['z_tip_to_sleeve']:.0f} under the sleeve, down; the collet nut {found['collet_proud']:.1f} out of the box's face at full plunge")
    print(f"over the printer's {BED:.0f}: {', '.join(too_long) or 'nothing'}")


if __name__ == "__main__":
    main()
