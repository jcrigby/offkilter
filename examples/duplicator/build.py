"""The carving duplicator (Woodsmith SN12918) rebuilt on 20 mm round
rail: the Y rails straight on a ply base, one gantry riding both of
them (a deck across the two rails' blocks and a wall standing on it,
an angle that cannot rack, with a gusset in each corner), the two X
rails one above the other on the wall, the Z carriage plate riding
them on blocks bolted to its back (no bed, no joint in bending) and
hanging in front of the deck's front edge, clear of it over the whole
X travel, the Z slide on its front, and two work platforms, the blank
on one and the printed puzzle pattern plate on the other.

The router motor and the pilot sit side by side in split clamps through
one 38 mm tool plate, both bores drilled in one setup, the slits to the
front so tightening shifts each tool in y and never in the x spacing
that calibration sets. Depth: an M8 stop screw on a stop block limits
the early passes; the pilot bottoming in the pattern's groove limits
the last. A spring balancer on the carriage plate carries all but half
a kilogram of the Z slide, so the pilot rides the groove with a few
newtons and its drag cannot bend it. The pilot is 2 mm drill rod in a keyless mini chuck whose
3/8 in shank slides in its clamp to set the height.

    cargo build --release -p ok-mcp
    python3 examples/duplicator/build.py          # the document, pictures and sheets
    cargo test -p ok-render --test duplicator     # what CI runs

Frame: x across, y toward the operator negative, z up from the base
top. All in mm; every part built in place at mid travel, Z raised.
Shafts: X 760 (x2), Y 700 (x2), Z 250 (x2): the four 1000 mm shafts on
hand cut 760 + 240 and 700 + 300, the Z pair from the 300 offcuts; the
third SFC20 kit supplies the last four SK20s and SC20UUs.
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
PLY = 19.0
SHAFT_D = 20.0
SK_W, SK_T, SK_H, SK_HTOT = 60.0, 20.0, 51.0, 70.0      # SK20: base width, thickness along the shaft, base to centre, height
BLK_W, BLK_L, BLK_H, BLK_C = 50.0, 45.0, 42.0, 25.0     # SC20UU: width across, length along the shaft, height, base to centre
# Base and platforms.
BASE = dict(x=(-450.0, 450.0), y=(-430.0, 425.0), z=(-PLY, 0.0))
BATTEN_Y, BATTEN_H = ((-430.0, -390.0), (-20.0, 20.0), (330.0, 370.0)), 40.0   # under the base, clear of the Y supports' T-nuts at y -295 and 385
TOOL_Y = -280.0                                         # the bit and pilot at mid travel
BIT_X, PILOT_X = -175.0, 175.0                          # the two tools, over the two platforms
PLATFORM_W, PLATFORM_D = 330.0, 300.0
BOARD_W, BOARD_D, BOARD_T = 304.0, 260.0, 12.0
PLATE_W, PLATE_D, PLATE_FLOOR, GROOVE_D, GROOVE_W = 324.0, 280.0, 3.0, 12.5, 2.0
BOARD_TOP = PLY + BOARD_T                               # 31: the Z slide's datum
# Y axis: shafts along y at x = +/-300, on SK20s on risers.
Y_RAIL_X, Y_SHAFT = 415.0, (-305.0, 395.0)              # rails outside the platforms (x to +/-340), their supports clear of the tool plate's ends at full X
Y_SK_Y = (-295.0, 385.0)                                # support centres, 45 back of centre with the gantry's blocks
Y_AXIS_Z = SK_H                                         # 51: the supports stand on the base
Y_BED_Z = Y_AXIS_Z + BLK_C                              # 76: the blocks' bases up, the deck on them
DECK_X, DECK_FRONT, DECK_BACK = 445.0, -100.0, 200.0   # the deck, 890 x 300, over both rails' blocks, its front edge behind the Z carriage plate
Y_BLOCK_Y = (-70.0, 160.0)                              # the gantry's blocks, 230 apart, under the deck
YC = 0.0                                                # the carriages' y at this pose
# X axis: two shafts along x, one above the other, between upright posts on the Y beds.
X_SHAFT = (-380.0, 380.0)
X_SK_X = 370.0
X_AXIS_Z = (160.0, 310.0)                               # 150 apart: the Z plate's blocks pull above and push below
XC = 0.0
WALL_T = PLY                                            # the wall on the deck's back edge, one ply, full width
# The Z slide, as drawn before, hung from the X bed's front edge: its
# carriage plate's front face at y = -110, its bottom at the board top.
PLATE_FACE_Y = -130.0
PLATE_BACK_Y = PLATE_FACE_Y + PLY                        # -111: the X blocks' bases
X_SHAFT_Y = PLATE_BACK_Y + BLK_C                         # -86
POST_FACE_Y = X_SHAFT_Y + SK_H                           # -35: the SK20s' bases, on the posts' front faces
PLATE_TOP = X_AXIS_Z[1] + 35.0                            # 345
Z0 = BOARD_TOP
SHAFT_X, SHAFT_Y = 60.0, PLATE_FACE_Y - 51.0            # the Z shafts
SK_Z = (Z0 + 10.0, Z0 + 220.0)
BLOCK_BASE_Y = SHAFT_Y - BLK_C
SUPPORT_Y = (BLOCK_BASE_Y - PLY, BLOCK_BASE_Y)
TOOL_PLATE_Y = (TOOL_Y - 55.0, SUPPORT_Y[0])              # 110 deep: 22 mm of wall in front of the router bore
TOOL_T = 2 * PLY
TOOL_Z_BELOW = 65.0
ROUTER_D, ROUTER_H, COLLET_D, COLLET_L, BIT_D, BIT_OUT = 65.0, 126.0, 22.0, 16.0, 2.0, 30.0
PILOT_D, PILOT_OUT = 2.0, 18.0                             # 2 mm drill rod, 18 out of the chuck: the chuck clears the plate by 2
CHUCK_D, CHUCK_L, SHANK_D, SHANK_L = 28.0, 30.0, 9.5, 42.0  # keyless mini chuck, 3/8 in shank
SLIT_W, PILOT_SLIT_W = 2.0, 1.5                            # the split clamps' slits, to the front edge
ROUTER_BOLT_D, PILOT_BOLT_D = 6.0, 4.0                     # M6 and M4 knob bolts across the slits
FENCE_H, FENCE_W, FENCE_T = 10.0, 30.0, 12.0               # L fences: the blank's 30 wide and slotted, the pattern's 12 and fixed
REF_IN = 5.0                                               # the reference hole, 5 mm inside the pattern plate's corner
TIP_BELOW = 115.0
ZC = Z0 + 155.0                                         # raised: tips 40 above the board
STOP_BLOCK = dict(x=(-15.0, 15.0), y=(PLATE_FACE_Y - 40.0, PLATE_FACE_Y), z=(Z0 + 30.0, Z0 + 60.0))
EAR_Y = (SUPPORT_Y[1], PLATE_FACE_Y - 20.0)
SCREW_D, SCREW_Y = 8.0, PLATE_FACE_Y - 30.0
# The balancer: a retractable spring reel on a bracket at the top of the
# carriage plate, its cable down to an eye on the tool support's ear,
# set to carry all but half a kilogram or so of the 4.2 kg Z slide, so
# the pilot rides the groove floor with a few newtons and its drag
# cannot bend it. The reel hangs in front of the plate above the Z
# supports, between the upper Z shaft supports, and the cable passes
# 8 mm in front of the stop screw's knob.
BAL_Y = PLATE_FACE_Y - 60.0                              # -190: the cable's line, over the ear, the drum clear of the bracket leg
BAL_X = 25.0                                             # the bracket's half width, between the upper Z supports' bases
BAL_LEG_Z, BAL_ARM_Z = (290.0, 419.0), (400.0, 419.0)   # the leg on the plate's face, the arm over the reel
REEL_D, REEL_W, REEL_Z = 70.0, 36.0, 345.0               # a 1 to 3 kg spring balancer's drum, hung from the arm
HOOK_H, CABLE_D, EYE_D, EYE_H = 20.0, 1.5, 6.0, 8.0
STOP_PROTRUDE = 49.0                                    # below the ear: set for the first pass, 4 mm deep; backed off past 32.5 for the pilot's pass


def box(p, name, x, y, z, op="new"):
    s = p.sketch("top", z[0], name)
    p.rect(s, (x[0], y[0]), (x[1], y[1]))
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def vcyl(p, name, cx, cy, z0, z1, d, op="new"):
    s = p.sketch("top", z0, name)
    p.circle(s, (cx, cy), d / 2)
    return p.extrude(s, z1 - z0, op=op, name=name)


def xcyl(p, name, x0, x1, cy, cz, d, op="new"):
    s = p.sketch("right", x0, name)
    p.circle(s, (cy, cz), d / 2)
    return p.extrude(s, x1 - x0, op=op, name=name)


def ycyl(p, name, y0, y1, cx, cz, d, op="new"):
    """A cylinder along y from y0 to y1 (y1 > y0): the front plane's normal points -y."""
    s = p.sketch("front", -y1, name)
    p.circle(s, (cx, cz), d / 2)
    return p.extrude(s, y1 - y0, op=op, name=name)


def base(p):
    box(p, "base, 900 x 855 ply", BASE["x"], BASE["y"], BASE["z"])


def battens(p):
    """Three battens under the base, two plies glued (38 x 40), the full
    width: the base cannot sag over the bench, and the Y supports'
    T-nuts have room underneath."""
    for i, (y0, y1) in enumerate(BATTEN_Y):
        box(p, "batten, two plies", BASE["x"], (y0, y1), (-PLY - BATTEN_H, -PLY), op="new" if i == 0 else "add")


def deck(p):
    """One ply across both Y rails' blocks, the gantry's floor: racking
    is in-plane shear of this sheet. Its front edge is behind the Z
    carriage plate, which hangs from the X shafts to the board top and
    sweeps the whole X travel in front of it."""
    box(p, "deck, ply", (-DECK_X, DECK_X), (YC + DECK_FRONT, YC + DECK_BACK), (Y_BED_Z, Y_BED_Z + PLY))


def wall(p):
    """One ply standing on the deck, full width, the X supports on its
    front face: with the deck it is an angle."""
    box(p, "wall, ply", (-DECK_X, DECK_X), (POST_FACE_Y, POST_FACE_Y + WALL_T), (Y_BED_Z + PLY, PLATE_TOP + 20.0))


def gussets(p):
    """A triangular web in each corner of the angle, glued to the deck and the wall."""
    for i, sx in enumerate((-Y_RAIL_X, Y_RAIL_X)):
        s = p.sketch("right", sx - PLY / 2, "gusset")
        p.polygon(s, [(POST_FACE_Y + WALL_T, Y_BED_Z + PLY), (YC + DECK_BACK - 5.0, Y_BED_Z + PLY), (POST_FACE_Y + WALL_T, PLATE_TOP)])
        p.extrude(s, PLY, op="new" if i == 0 else "add", name="gusset, ply")


def y_supports(p):
    """Four SK20s standing on the base, shaft along y."""
    first = True
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for sy in Y_SK_Y:
            box(p, "base", (sx - SK_W / 2, sx + SK_W / 2), (sy - SK_T / 2, sy + SK_T / 2), (0.0, 12.0), op="new" if first else "add")
            box(p, "body", (sx - 16.0, sx + 16.0), (sy - SK_T / 2, sy + SK_T / 2), (0.0, SK_HTOT), op="add")
            first = False
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for sy in Y_SK_Y:
            s = p.sketch("front", -(sy + SK_T / 2 + 1.0), "bore")
            p.point(s, (sx, Y_AXIS_Z))
            p.hole(s, SHAFT_D, depth=SK_T + 2.0, direction="normal", name="shaft bore")


def y_shafts(p):
    for i, sx in enumerate((-Y_RAIL_X, Y_RAIL_X)):
        ycyl(p, "20 mm shaft, Y, 700", Y_SHAFT[0], Y_SHAFT[1], sx, Y_AXIS_Z, SHAFT_D, op="new" if i == 0 else "add")


def y_blocks(p):
    first = True
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC + Y_BLOCK_Y[0], YC + Y_BLOCK_Y[1]):
            box(p, "block", (sx - BLK_W / 2, sx + BLK_W / 2), (by - BLK_L / 2, by + BLK_L / 2), (Y_BED_Z - BLK_H, Y_BED_Z), op="new" if first else "add")
            first = False
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC + Y_BLOCK_Y[0], YC + Y_BLOCK_Y[1]):
            s = p.sketch("front", -(by + BLK_L / 2 + 1.0), "bore")
            p.point(s, (sx, Y_AXIS_Z))
            p.hole(s, SHAFT_D, depth=BLK_L + 2.0, direction="normal", name="shaft bore")


def x_supports(p):
    """Four SK20s on the wall's front face, shaft along x, one above the other."""
    first = True
    for sx in (-X_SK_X, X_SK_X):
        for sz in X_AXIS_Z:
            box(p, "base", (sx - SK_T / 2, sx + SK_T / 2), (POST_FACE_Y - 12.0, POST_FACE_Y), (sz - SK_W / 2, sz + SK_W / 2), op="new" if first else "add")
            box(p, "body", (sx - SK_T / 2, sx + SK_T / 2), (POST_FACE_Y - SK_HTOT, POST_FACE_Y), (sz - 16.0, sz + 16.0), op="add")
            first = False
    for sx in (-X_SK_X, X_SK_X):
        for sz in X_AXIS_Z:
            s = p.sketch("right", sx - SK_T / 2 - 1.0, "bore")
            p.point(s, (X_SHAFT_Y, sz))
            p.hole(s, SHAFT_D, depth=SK_T + 2.0, direction="normal", name="shaft bore")


def x_shafts(p):
    for i, sz in enumerate(X_AXIS_Z):
        xcyl(p, "20 mm shaft, X, 760", X_SHAFT[0], X_SHAFT[1], X_SHAFT_Y, sz, SHAFT_D, op="new" if i == 0 else "add")


def x_blocks(p):
    """Four SC20UU on the back of the Z carriage plate, bases forward."""
    first = True
    for bx in (XC - 60.0, XC + 60.0):
        for sz in X_AXIS_Z:
            box(p, "block", (bx - BLK_L / 2, bx + BLK_L / 2), (PLATE_BACK_Y, PLATE_BACK_Y + BLK_H), (sz - BLK_W / 2, sz + BLK_W / 2), op="new" if first else "add")
            first = False
    for bx in (XC - 60.0, XC + 60.0):
        for sz in X_AXIS_Z:
            s = p.sketch("right", bx - BLK_L / 2 - 1.0, "bore")
            p.point(s, (X_SHAFT_Y, sz))
            p.hole(s, SHAFT_D, depth=BLK_L + 2.0, direction="normal", name="shaft bore")


def carriage_plate(p):
    box(p, "Z carriage plate, ply", (XC - 110.0, XC + 110.0), (PLATE_FACE_Y, PLATE_BACK_Y), (Z0, PLATE_TOP))
    box(p, "stop block", STOP_BLOCK["x"], STOP_BLOCK["y"], STOP_BLOCK["z"], op="add")


def z_supports(p):
    first = True
    for z0 in SK_Z:
        for sx in (-SHAFT_X, SHAFT_X):
            box(p, "base", (sx - SK_W / 2, sx + SK_W / 2), (PLATE_FACE_Y - 12.0, PLATE_FACE_Y), (z0, z0 + SK_T), op="new" if first else "add")
            box(p, "body", (sx - 16.0, sx + 16.0), (PLATE_FACE_Y - SK_HTOT, PLATE_FACE_Y), (z0, z0 + SK_T), op="add")
            first = False
    for z0 in SK_Z:
        s = p.sketch("top", z0 + SK_T + 1.0, "bores")
        for sx in (-SHAFT_X, SHAFT_X):
            p.point(s, (sx, SHAFT_Y))
        p.hole(s, SHAFT_D, depth=SK_T + 2.0, direction="reverse", name="shaft bores")


def z_shafts(p):
    for i, sx in enumerate((-SHAFT_X, SHAFT_X)):
        vcyl(p, "20 mm shaft, Z, 250", sx, SHAFT_Y, Z0, Z0 + 250.0, SHAFT_D, op="new" if i == 0 else "add")


def z_blocks(p):
    first = True
    for sx in (-SHAFT_X, SHAFT_X):
        for bz in (ZC - 40.0, ZC + 40.0):
            box(p, "block", (sx - BLK_W / 2, sx + BLK_W / 2), (BLOCK_BASE_Y, BLOCK_BASE_Y + BLK_H), (bz - BLK_L / 2, bz + BLK_L / 2), op="new" if first else "add")
            first = False
    s = p.sketch("top", ZC + 70.0, "bores")
    for sx in (-SHAFT_X, SHAFT_X):
        p.point(s, (sx, SHAFT_Y))
    p.hole(s, SHAFT_D, depth=140.0, direction="reverse", name="shaft bores")


def tool_support(p):
    zb = ZC - TOOL_Z_BELOW
    zm = zb + TOOL_T / 2                                   # the clamp bolts' height, mid plate
    front = TOOL_PLATE_Y[0]
    box(p, "support plate, ply", (-110.0, 110.0), SUPPORT_Y, (zb, ZC + 65.0))
    box(p, "tool plate, two plies", (BIT_X - 65.0, PILOT_X + 65.0), TOOL_PLATE_Y, (zb, zb + TOOL_T), op="add")
    box(p, "stop ear", (-15.0, 15.0), EAR_Y, (ZC - 10.0, ZC + 9.0), op="add")
    vcyl(p, "cable eye", 0.0, BAL_Y, ZC + 9.0, ZC + 9.0 + EYE_H, EYE_D, op="add")
    # Both bores in one drill-press setup, then a slit from each to the front edge.
    s = p.sketch("top", zb + TOOL_T + 1.0, "router bore")
    p.point(s, (BIT_X, TOOL_Y))
    p.hole(s, ROUTER_D + 1.0, depth=TOOL_T + 2.0, direction="reverse", name="router clamp bore, 66")
    s = p.sketch("top", zb + TOOL_T + 1.0, "pilot bore")
    p.point(s, (PILOT_X, TOOL_Y))
    p.hole(s, SHANK_D + 0.2, depth=TOOL_T + 2.0, direction="reverse", name="chuck shank bore, 9.7")
    s = p.sketch("top", zb + TOOL_T + 1.0, "slits")
    p.rect(s, (BIT_X - SLIT_W / 2, front - 1.0), (BIT_X + SLIT_W / 2, TOOL_Y))
    p.rect(s, (PILOT_X - PILOT_SLIT_W / 2, front - 1.0), (PILOT_X + PILOT_SLIT_W / 2, TOOL_Y))
    p.cut(s, TOOL_T + 2.0, direction="reverse", name="clamp slits, to the front")
    # The knob bolts run along x through the front wall: the router's from the
    # plate's left end, the pilot's from its right end, each into a nut pocket
    # just past its slit.
    wall_y = front + (TOOL_Y - ROUTER_D / 2 - 1.0 - front) / 2  # mid front wall at the router: -324
    s = p.sketch("right", BIT_X - 66.0, "router bolt")
    p.point(s, (wall_y, zm))
    p.hole(s, ROUTER_BOLT_D + 0.5, depth=66.0 + 32.0, direction="normal", name="M6 clearance")
    s = p.sketch("right", PILOT_X + 66.0, "pilot bolt")
    p.point(s, (wall_y, zm))
    p.hole(s, PILOT_BOLT_D + 0.5, depth=66.0 + 20.0, direction="reverse", name="M4 clearance")
    s = p.sketch("top", zb + TOOL_T + 1.0, "nut pockets")
    p.rect(s, (BIT_X + 26.0, wall_y - 5.0), (BIT_X + 32.0, wall_y + 5.0))
    p.rect(s, (PILOT_X - 20.0, wall_y - 3.5), (PILOT_X - 16.0, wall_y + 3.5))
    p.cut(s, TOOL_T / 2 + 1.0 + 5.0, direction="reverse", name="nut pockets, from the top")
    s = p.sketch("top", ZC + 10.0, "screw hole")
    p.point(s, (0.0, SCREW_Y))
    p.hole(s, SCREW_D, depth=21.0, direction="reverse", name="M8 tapped")


def clamp_bolts(p):
    zm = ZC - TOOL_Z_BELOW + TOOL_T / 2
    wall_y = TOOL_PLATE_Y[0] + (TOOL_Y - ROUTER_D / 2 - 1.0 - TOOL_PLATE_Y[0]) / 2
    xcyl(p, "M6 x 100 knob bolt", BIT_X - 77.0, BIT_X + 30.0, wall_y, zm, ROUTER_BOLT_D)
    xcyl(p, "knob", BIT_X - 77.0, BIT_X - 65.0, wall_y, zm, 25.0, op="add")
    xcyl(p, "M4 x 90 knob bolt", PILOT_X - 18.0, PILOT_X + 75.0, wall_y, zm, PILOT_BOLT_D, op="new")
    xcyl(p, "knob", PILOT_X + 65.0, PILOT_X + 75.0, wall_y, zm, 18.0, op="add")


def balancer_bracket(p):
    """The reel's bracket, a leg on the carriage plate's face above the Z
    supports and an arm over the reel, with the cable down to the eye on
    the ear: drawn at the raised pose, so at a lower pose the cable ends
    short of the eye rather than through it."""
    box(p, "bracket leg, ply", (-BAL_X, BAL_X), (PLATE_FACE_Y - PLY, PLATE_FACE_Y), BAL_LEG_Z)
    box(p, "bracket arm, ply", (-BAL_X, BAL_X), (BAL_Y - 20.0, PLATE_FACE_Y), BAL_ARM_Z, op="add")
    s = p.sketch("top", BAL_ARM_Z[1] + 1.0, "hook hole")
    p.point(s, (0.0, BAL_Y))
    p.hole(s, 8.0, depth=BAL_ARM_Z[1] - BAL_ARM_Z[0] + 2.0, direction="reverse", name="hook hole")
    vcyl(p, "cable", 0.0, BAL_Y, ZC + 9.0 + EYE_H, REEL_Z - REEL_D / 2, CABLE_D, op="new")


def balancer_reel(p):
    """A 1 to 3 kg retractable spring balancer on its hook (its own
    studio: an add joins every body it touches and the first one too)."""
    box(p, "hook", (-3.0, 3.0), (BAL_Y - 3.0, BAL_Y + 3.0), (REEL_Z + REEL_D / 2, BAL_ARM_Z[0] - 1.0))
    xcyl(p, "spring balancer, 1 to 3 kg", -REEL_W / 2, REEL_W / 2, BAL_Y, REEL_Z, REEL_D, op="add")


def tool_support_webs(p):
    """Three webs between the support plate's face and the tool plate's
    top, so the tool plate hangs on triangles, not on a glue line in
    bending: one at each end of the support plate and one in the middle."""
    zb = ZC - TOOL_Z_BELOW
    for i, gx in enumerate((-100.0, 0.0, 100.0)):
        s = p.sketch("right", gx - PLY / 2, "web")
        p.polygon(s, [(SUPPORT_Y[0], zb + TOOL_T), (TOOL_PLATE_Y[0] + 15.0, zb + TOOL_T), (SUPPORT_Y[0], ZC + 55.0)])
        p.extrude(s, PLY, op="new", name="web, ply")



def router(p):
    tip = ZC - TIP_BELOW
    vcyl(p, "bit", BIT_X, TOOL_Y, tip, tip + BIT_OUT, BIT_D)
    vcyl(p, "collet nut", BIT_X, TOOL_Y, tip + BIT_OUT, tip + BIT_OUT + COLLET_L, COLLET_D, op="add")
    vcyl(p, "trim router motor", BIT_X, TOOL_Y, tip + BIT_OUT + COLLET_L, tip + BIT_OUT + COLLET_L + ROUTER_H, ROUTER_D, op="add")


def pilot(p):
    """A 2 mm pilot in a keyless mini chuck; its 3/8 in shank slides in the
    tool plate's split clamp to set the pilot's height."""
    tip = ZC - TIP_BELOW
    vcyl(p, "2 mm pilot", PILOT_X, TOOL_Y, tip, tip + PILOT_OUT + 5.0, PILOT_D)
    vcyl(p, "keyless chuck", PILOT_X, TOOL_Y, tip + PILOT_OUT, tip + PILOT_OUT + CHUCK_L, CHUCK_D, op="add")
    vcyl(p, "3/8 shank", PILOT_X, TOOL_Y, tip + PILOT_OUT + CHUCK_L, tip + PILOT_OUT + CHUCK_L + SHANK_L, SHANK_D, op="add")


def stop_screw(p):
    tip = ZC - 10.0 - STOP_PROTRUDE
    vcyl(p, "M8 stop screw", 0.0, SCREW_Y, tip, ZC + 25.0, SCREW_D)
    vcyl(p, "knob", 0.0, SCREW_Y, ZC + 25.0, ZC + 37.0, 24.0, op="add")


def platforms(p):
    box(p, "blank platform, ply", (BIT_X - PLATFORM_W / 2, BIT_X + PLATFORM_W / 2), (TOOL_Y - PLATFORM_D / 2, TOOL_Y + PLATFORM_D / 2), (0.0, PLY))
    # The pattern platform is 3.5 thinner, so the plate's top sits level with the board's.
    box(p, "pattern platform, 15.5 mm", (PILOT_X - PLATFORM_W / 2, PILOT_X + PLATFORM_W / 2), (TOOL_Y - PLATFORM_D / 2, TOOL_Y + PLATFORM_D / 2), (0.0, PLY - 3.5), op="add")


def blank(p):
    box(p, "blank, 304 x 260 x 12", (BIT_X - BOARD_W / 2, BIT_X + BOARD_W / 2), (TOOL_Y - BOARD_D / 2, TOOL_Y + BOARD_D / 2), (PLY, BOARD_TOP))


def fences(p):
    """An L fence on each platform, against the blank's and the plate's
    back and left edges. The blank's is 30 wide with slots across it, so it
    shifts in x and y to calibrate; the pattern's is set once and screwed."""
    x0, x1 = BIT_X - BOARD_W / 2, BIT_X + BOARD_W / 2
    y0, y1 = TOOL_Y - BOARD_D / 2, TOOL_Y + BOARD_D / 2
    box(p, "blank back fence, slotted", (x0 - FENCE_W, x1), (y1, y1 + FENCE_W), (PLY, PLY + FENCE_H))
    box(p, "blank left fence, slotted", (x0 - FENCE_W, x0), (y0, y1), (PLY, PLY + FENCE_H), op="add")
    s = p.sketch("top", PLY + FENCE_H + 1.0, "slots")
    for cx in (x0 + 30.0, BIT_X, x1 - 30.0):
        p.rect(s, (cx - 3.0, y1 + 7.0), (cx + 3.0, y1 + FENCE_W - 7.0))
    for cy in (y0 + 30.0, TOOL_Y, y1 - 30.0):
        p.rect(s, (x0 - FENCE_W + 7.0, cy - 3.0), (x0 - 7.0, cy + 3.0))
    p.cut(s, FENCE_H + 2.0, direction="reverse", name="slots, 6 x 16, across the fence")
    zp = PLY - 3.5
    px0, px1 = PILOT_X - PLATE_W / 2, PILOT_X + PLATE_W / 2
    py0, py1 = TOOL_Y - PLATE_D / 2, TOOL_Y + PLATE_D / 2
    box(p, "pattern back fence", (px0 - FENCE_T, px1), (py1, py1 + FENCE_T), (zp, zp + FENCE_H), op="new")
    box(p, "pattern left fence", (px0 - FENCE_T, px0), (py0, py1), (zp, zp + FENCE_H), op="add")
    s = p.sketch("top", zp + FENCE_H + 1.0, "screws")
    for cx in (px0 + 30.0, PILOT_X, px1 - 30.0):
        p.point(s, (cx, py1 + FENCE_T / 2))
    for cy in (py0 + 30.0, TOOL_Y, py1 - 30.0):
        p.point(s, (px0 - FENCE_T / 2, cy))
    p.hole(s, 4.0, depth=FENCE_H + 2.0, direction="reverse", name="screw holes")


def pattern(p):
    z0 = PLY - 3.5
    box(p, "printed pattern plate", (PILOT_X - PLATE_W / 2, PILOT_X + PLATE_W / 2), (TOOL_Y - PLATE_D / 2, TOOL_Y + PLATE_D / 2), (z0, z0 + PLATE_FLOOR + GROOVE_D))
    # A stand-in for the gap lattice: a grid of grooves, each cut from
    # its own sketch. One sketch of all of them would also enclose the
    # squares between, and a cut takes every region a sketch encloses;
    # the duplicator check on the plate is what found that.
    grooves = [((PILOT_X - 152.0, TOOL_Y + k * 38.0 - GROOVE_W / 2), (PILOT_X + 152.0, TOOL_Y + k * 38.0 + GROOVE_W / 2)) for k in range(-3, 4)]
    grooves += [((PILOT_X + k * 38.0 - GROOVE_W / 2, TOOL_Y - 130.0), (PILOT_X + k * 38.0 + GROOVE_W / 2, TOOL_Y + 130.0)) for k in range(-4, 5)]
    for n, (a, b) in enumerate(grooves):
        s = p.sketch("top", z0 + PLATE_FLOOR + GROOVE_D + 1.0, f"groove {n + 1}")
        p.rect(s, a, b)
        p.cut(s, GROOVE_D + 1.0, direction="reverse", name=f"groove {n + 1}")
    # The reference hole: 5 mm inside the front-left corner, outside the
    # board's outline. The pilot in it puts the bit 5 mm outside the blank's
    # corner; the plunge mark in the platform is what the slotted fence is set by.
    s = p.sketch("top", z0 + PLATE_FLOOR + GROOVE_D + 1.0, "reference")
    p.point(s, (PILOT_X - PLATE_W / 2 + REF_IN, TOOL_Y - PLATE_D / 2 + REF_IN))
    p.hole(s, PILOT_D + 0.05, depth=PLATE_FLOOR + GROOVE_D + 2.0, direction="reverse", name="2 mm reference hole")


PARTS = [
    ("Base", base), ("Battens", battens), ("Y supports SK20", y_supports), ("Y shafts", y_shafts), ("Y blocks SC20UU", y_blocks), ("Deck", deck), ("Wall", wall), ("Gussets", gussets),
    ("X supports SK20", x_supports), ("X shafts", x_shafts), ("X blocks SC20UU", x_blocks),
    ("Z carriage plate", carriage_plate), ("Z supports SK20", z_supports), ("Z shafts", z_shafts), ("Z blocks SC20UU", z_blocks), ("Balancer bracket", balancer_bracket), ("Balancer reel", balancer_reel),
    ("Tool support", tool_support), ("Tool support webs", tool_support_webs), ("Clamp bolts", clamp_bolts), ("Router", router), ("Pilot", pilot), ("Stop screw", stop_screw),
    ("Platforms", platforms), ("Fences", fences), ("Blank", blank), ("Pattern plate", pattern),
]


# ---------------------------------------------------------------------
# Motion. Three sub-assemblies, each a rigid group of parts built in
# place, and the stop screw on its own; in the top assembly one slider
# each: the gantry on a Y shaft, the X slide on the gantry's lower X
# shaft, the Z slide on the X slide's left Z shaft, and the stop screw
# in the tool support's ear. The mate parameters are read off the drawn
# pose, so the resolved assembly lands exactly where the fixed one did,
# and the range-of-motion sheets then move the mates.
# ---------------------------------------------------------------------
MOTION = [
    ("gantry", "Gantry", ["Y blocks SC20UU", "Deck", "Wall", "Gussets", "X supports SK20", "X shafts"],
     "deck and wall, one ply each, an angle with a gusset in each corner; X shafts 760 at z = 160 and 310, SK20s 740 apart"),
    ("x_slide", "X slide", ["X blocks SC20UU", "Z carriage plate", "Z supports SK20", "Z shafts", "Balancer bracket", "Balancer reel"],
     "carriage plate 220 x 314, one ply, X blocks on its back; Z shafts 250 at x = +/-60, SK20s 210 apart; spring balancer on a bracket"),
    ("z_slide", "Z slide", ["Z blocks SC20UU", "Tool support", "Tool support webs", "Clamp bolts", "Router", "Pilot"],
     "tool plate 480 x 110 x 38; split clamps 66 (router) and 9.7 (chuck shank), slits to the front"),
]
FIXED = ["Base", "Battens", "Y supports SK20", "Y shafts", "Platforms", "Fences", "Blank", "Pattern plate"]
# (name, placed side, moving side, radius on each): "Sub/part" names a member of a sub-assembly.
MATES = [
    ("Y travel", "Y shafts 1", "Gantry/Y blocks SC20UU 1", SHAFT_D / 2, SHAFT_D / 2),
    ("X travel", "Gantry/X shafts 1", "X slide/X blocks SC20UU 1", SHAFT_D / 2, SHAFT_D / 2),
    ("Z travel", "X slide/Z shafts 1", "Z slide/Z blocks SC20UU 1", SHAFT_D / 2, SHAFT_D / 2),
    ("stop screw", "Z slide/Tool support", "Stop screw", SCREW_D / 2, SCREW_D / 2),
]


# ---------------------------------------------------------------------
# Hardware. Bought parts bolt through the ply into T-nuts, never into
# wood screws, since the rail supports are aligned by loosening and
# sliding. Ply joints are glued, the screws clamps and insurance:
# number 8, 32 mm into face grain, 50 mm into an edge. The SK20's slots
# and the SC20UU's tapped holes are what they usually are; measure the
# kit before buying.
# ---------------------------------------------------------------------
HARDWARE = [
    ("12 SK20 supports: 4 Y on the base, 4 X on the wall, 4 Z on the carriage plate", "M6 x 40 hex bolt, washer, pronged T-nut from the far face", 24),
    ("4 Y blocks under the deck, from above", "M5 x 30 (or M6, as the block is tapped) into the block", 16),
    ("4 X blocks on the carriage plate's back, from its front face", "M5 x 30 (or M6) into the block", 16),
    ("4 Z blocks on the support plate's back, from its front face; the lower pair countersunk, before the tool plate goes on", "M5 x 30 (or M6) into the block", 16),
    ("Router clamp", "M6 x 100 knob bolt, hex nut in the pocket", 1),
    ("Pilot clamp", "M4 x 90 knob bolt, hex nut in the pocket", 1),
    ("Stop screw", "M8 x 90 with a knob, through an M8 T-nut set into the top of the ear, an M8 jam nut above the ear", 1),
    ("Blank's slotted fence", "M5 x 25 bolt and washer into a T-nut set into the platform from below, before the platform is screwed down", 6),
    ("Balancer bracket leg to the carriage plate", "M6 x 40 bolt and T-nut", 2),
    ("Cable eye on the ear", "M4 screw eye", 1),
    ("Balancer hook in the arm", "an 8 mm S-hook or shackle", 1),
    ("Wall onto the deck's back edge, from below through the deck", "no. 8 x 50 wood screw, every 100 mm, glued", 9),
    ("Gussets to the deck and the wall", "no. 8 x 50 wood screw, 3 each way, glued", 12),
    ("Tool plate's two plies laminated", "no. 8 x 32 wood screw, countersunk from below, clear of the bores", 8),
    ("Tool plate to the support plate, from behind", "no. 8 x 50 wood screw, glued", 4),
    ("Webs to the support plate from behind and down into the tool plate", "no. 8 x 50 wood screw, 2 each way, glued", 12),
    ("Stop ear to the support plate, from behind", "no. 8 x 50 wood screw, glued", 2),
    ("Balancer bracket arm to its leg", "no. 8 x 50 wood screw, glued", 2),
    ("Battens under the base", "no. 8 x 32 wood screw from above, countersunk, every 150 mm, glued", 18),
    ("Platforms to the base, countersunk flush", "no. 8 x 32 wood screw", 12),
    ("Pattern plate's L fence", "no. 6 x 25 wood screw", 6),
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


def _frame(cyl, face):
    """The connector frame of a cylindrical face as the kernel builds it
    (see docs/OPS.md): origin at the face's middle on the axis, z along
    the axis, x and y canonical for z. Every part is built in place, so
    the frame on the studio's body is the frame in the world."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    t = _dot(_sub(c, o), z)
    origin = (o[0] + z[0] * t, o[1] + z[1] * t, o[2] + z[2] * t)
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    x = _cross(y, z)
    return origin, x, y, z


def coaxial(body_a, body_b, ra, rb):
    """The first cylinder of radius ra on body_a and rb on body_b that
    share an axis, with their frames (the bodies from a studio report)."""
    faces = lambda b: {json.dumps(f["reference"], sort_keys=True): f for f in b["faces"]}
    fa, fb = faces(body_a), faces(body_b)
    for ca in body_a["cylinders"]:
        if abs(ca["radius"] - ra) > 1e-6:
            continue
        ta = _frame(ca, fa[json.dumps(ca["reference"], sort_keys=True)])
        for cb in body_b["cylinders"]:
            if abs(cb["radius"] - rb) > 1e-6:
                continue
            tb = _frame(cb, fb[json.dumps(cb["reference"], sort_keys=True)])
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
    offset = _dot(_sub(om, ot), zt)
    angle = math.degrees(math.atan2(_dot(_cross(xt, xm), z), _dot(xt, xm)))
    return offset, angle, flip


def instance_ids(mcp, tab):
    report = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))
    return {i["name"]: i["id"] for i in report["instances"]}


def apply(mcp, ops, tab):
    text = mcp.call("apply", {"ops": ops, "tab": tab})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    return text


def instance_ops(tabs, bodies, titles, fixed):
    return [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": fixed,
             "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}} for t in titles for k in range(len(bodies[t]))]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "duplicator.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "duplicator"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Carving duplicator on round rail"}]})
    tabs, bodies, reports = {}, {}, {}
    for i, (title, fn) in enumerate(PARTS):
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]}); tab = 1
        else:
            text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": title}]})
            tab = int(re.search(r"tab (\d+)", text).group(1))
        tabs[title] = tab
        part = Part(mcp, tab, title)
        fn(part)
        bodies[title] = part.bodies()
        reports[title] = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["bodies"]
        print(f"{title:<20} tab {tab:>2}: {len(bodies[title])} bodies")
    # The three motion sub-assemblies, every member fixed in place.
    sub_tabs, sub_ids = {}, {}
    for file, title, members, note in MOTION:
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": title}]})
        sub_tabs[title] = int(re.search(r"tab (\d+)", text).group(1))
        apply(mcp, instance_ops(tabs, bodies, members, True), sub_tabs[title])
        sub_ids[title] = instance_ids(mcp, sub_tabs[title])
        print(f"{title:<20} tab {sub_tabs[title]:>2}: {len(sub_ids[title])} instances")
    # The machine: the fixed parts, the three sub-assemblies and the stop
    # screw, each moving one on a slider.
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Duplicator"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops = instance_ops(tabs, bodies, FIXED, True)
    ops += [{"type": "add_instance", "studio": sub_tabs[title], "body": 0, "name": title, "fixed": False,
             "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}} for _, title, _, _ in MOTION]
    ops += instance_ops(tabs, bodies, ["Stop screw"], False)
    apply(mcp, ops, asm)
    ids = instance_ids(mcp, asm)

    def side(spec):
        """A connector: an instance of the machine, or Sub/member."""
        if "/" in spec:
            sub, member = spec.split("/", 1)
            return {"instance": ids[sub], "sub": sub_ids[sub][member]}, member
        return {"instance": ids[spec]}, spec

    def body_of(member):
        """The studio body an instance was placed from."""
        m = re.fullmatch(r"(.*) (\d+)", member)
        title, k = (m.group(1), int(m.group(2)) - 1) if m and m.group(1) in reports else (member, 0)
        return reports[title][k]

    mate_ops = []
    for name, a, b, ra, rb in MATES:
        ca_ref, a_member = side(a)
        cb_ref, b_member = side(b)
        ca, cb, ta, tb = coaxial(body_of(a_member), body_of(b_member), ra, rb)
        offset, angle, flip = mate_parameters(ta, tb)
        mate_ops.append({"type": "add_mate", "kind": "slider", "a": {**ca_ref, "face": ca["reference"]}, "b": {**cb_ref, "face": cb["reference"]},
                         "offset": offset, "angle": angle, "flip": flip, "name": name})
    apply(mcp, mate_ops, asm)
    report = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
    for m in report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")
    worst = 0.0
    for i in report["instances"]:
        if i.get("error"):
            raise RuntimeError(f"instance {i['name']}: {i['error']}")
        if "placed" in i:
            worst = max(worst, max(abs(c) for c in _v(i["placed"]["position"])))
    mates = {m["name"]: m for m in report["mates"]}
    print(f"assembly tab {asm}: {len(ids)} instances, {len(mates)} sliders, resolved to the drawn pose (worst {worst:.1e} mm)")

    def where(name):
        rep = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
        return _v(next(i for i in rep["instances"] if i["name"] == name)["placed"]["position"])

    # Which way each slider's offset moves its part, by trying it.
    signs = {}
    for name, part, axis in (("Y travel", "Gantry", 1), ("X travel", "X slide", 0), ("Z travel", "Z slide", 2), ("stop screw", "Stop screw", 2)):
        m = mates[name]
        apply(mcp, [{"type": "set_mate", "id": m["id"], "offset": m["offset"] + 10.0}], asm)
        moved = where(part)[axis]
        apply(mcp, [{"type": "set_mate", "id": m["id"], "offset": m["offset"]}], asm)
        signs[name] = 1.0 if moved > 0.0 else -1.0
        assert abs(abs(moved) - 10.0) < 1e-6, f"{name}: {part} moved {moved}"
    at = lambda name, d: mates[name]["offset"] + signs[name] * d

    for view, name in (("-0.55,-0.75,0.45", "iso"), ("0.75,-0.55,0.4", "iso_right"), ("front", "front"), ("right", "right"), ("top", "top")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{name}.png")})
    mcp.call("screenshot", {"tab": asm, "view": "front", "section": f"y:{TOOL_Y + 0.5}:flip", "width": 1600, "height": 1100, "path": os.path.join(OUT, "section_tools.png")})
    mcp.call("screenshot", {"tab": asm, "view": "right", "section": "x:0", "width": 1600, "height": 1100, "path": os.path.join(OUT, "section_carriage.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A2", "note": "round rail duplicator, puzzle size: X 760 stacked, Y 700, Z 250 shafts; one-piece gantry on the Y blocks", "path": os.path.join(OUT, "duplicator.pdf")}))
    for file, title, _, note in MOTION:
        mcp.call("screenshot", {"tab": sub_tabs[title], "view": "-0.55,-0.75,0.45", "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{file}_iso.png")})
        print(mcp.call("export", {"tab": sub_tabs[title], "format": "pdf", "sheet": "A3", "note": note, "path": os.path.join(OUT, f"{file}.pdf")}))
    # Range of motion. The depth sequence from the front: raised, the
    # first pass on the stop screw, the last pass with the screw backed
    # off and the pilot in the groove. The reach from above: the bit at
    # the blank's front-left corner (the pilot at the reference hole),
    # at the centre and at the back-right corner.
    raised = ZC - TIP_BELOW - BOARD_TOP                     # 40: the tips above the board
    landing = (ZC - 10.0 - STOP_PROTRUDE) - STOP_BLOCK["z"][1]  # how far the slide drops before the screw lands: 36
    last = raised + GROOVE_D                                # 52.5
    z_positions = [
        {"Z travel": at("Z travel", 0.0), "label": f"raised, tips {raised:g} up"},
        {"Z travel": at("Z travel", -landing), "label": f"first pass, {raised - landing:g} mm, on the screw"},
        {"Z travel": at("Z travel", -last), "stop screw": at("stop screw", last - landing), "label": f"last pass, {GROOVE_D:g} mm, screw backed off"},
    ]
    xy_positions = [
        {"X travel": at("X travel", -BOARD_W / 2), "Y travel": at("Y travel", -PLATE_D / 2 + REF_IN), "label": "front-left corner, reference hole"},
        {"X travel": at("X travel", 0.0), "Y travel": at("Y travel", 0.0), "label": "centre"},
        {"X travel": at("X travel", BOARD_W / 2), "Y travel": at("Y travel", BOARD_D / 2), "label": "back-right corner"},
    ]
    for stem, positions, view in (("motion_z", z_positions, "front"), ("motion_xy", xy_positions, "top")):
        print(mcp.call("range_of_motion", {"tab": asm, "positions": positions, "view": view, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, f"{stem}.pdf")}))
        mcp.call("range_of_motion", {"tab": asm, "positions": positions, "view": view, "width": 1800, "height": 600, "path": os.path.join(OUT, f"{stem}.png")})
    with open(os.path.join(OUT, "hardware.csv"), "w") as f:
        f.write("joint,fastener,count\n")
        for joint, fastener, count in HARDWARE:
            f.write(f'"{joint}","{fastener}",{count}\n')
    print(f"hardware: {sum(c for _, _, c in HARDWARE)} fasteners in {len(HARDWARE)} lines")
    # The pattern plate as a master: its grooves are the gap wide, so a
    # bit the size of the gap follows them and the next size up does not.
    for d, stem in ((BIT_D, "check_2mm"), (3.175, "check_3mm")):
        print(mcp.call("duplicator_check", {"tab": tabs["Pattern plate"], "bit": d, "reach": GROOVE_D + 2.0, "pitch": 0.5, "path": os.path.join(OUT, f"{stem}.png")}))
    mcp.close()


if __name__ == "__main__":
    main()
