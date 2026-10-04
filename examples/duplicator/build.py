"""The carving duplicator (Woodsmith SN12918) rebuilt on 20 mm round
rail: the Y rails straight on a ply base, one gantry riding both of
them (a deck across the two rails' blocks and a wall standing on its
back edge, an angle that cannot rack, with a gusset in each corner),
the two X rails one above the other on the wall, the Z carriage plate
riding them on blocks bolted to its back (no bed, no joint in bending),
the Z slide on its front, and two work platforms, the blank on one and
the printed puzzle pattern plate on the other.

The router motor and the pilot sit side by side in split clamps through
one 38 mm tool plate, both bores drilled in one setup, the slits to the
front so tightening shifts each tool in y and never in the x spacing
that calibration sets. Depth: an M8 stop screw on a stop block limits
the early passes; the pilot bottoming in the pattern's groove limits
the last. The pilot is 2 mm drill rod in a keyless mini chuck whose
3/8 in shank slides in its clamp to set the height.

    cargo build --release -p ok-mcp
    python3 examples/duplicator/build.py          # the document, pictures and sheets
    cargo test -p ok-render --test duplicator     # what CI runs

Frame: x across, y toward the operator negative, z up from the base
top. All in mm; every part built in place at mid travel, Z raised.
Shafts: X 760 (x2), Y 600 (x2), Z 250 (x2): the four 1000 mm shafts on
hand cut 760 + 240 and 600 + 400, the Z pair from the 400 offcuts; the
third SFC20 kit supplies the last four SK20s and SC20UUs.
"""
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
BASE = dict(x=(-450.0, 450.0), y=(-430.0, 330.0), z=(-PLY, 0.0))
TOOL_Y = -280.0                                         # the bit and pilot at mid travel
BIT_X, PILOT_X = -175.0, 175.0                          # the two tools, over the two platforms
PLATFORM_W, PLATFORM_D = 330.0, 300.0
BOARD_W, BOARD_D, BOARD_T = 304.0, 260.0, 12.0
PLATE_W, PLATE_D, PLATE_FLOOR, GROOVE_D, GROOVE_W = 324.0, 280.0, 3.0, 12.5, 2.0
BOARD_TOP = PLY + BOARD_T                               # 31: the Z slide's datum
# Y axis: shafts along y at x = +/-300, on SK20s on risers.
Y_RAIL_X, Y_SHAFT = 380.0, (-300.0, 300.0)              # rails outside the platforms (x to +/-340) and their risers
Y_SK_Y = (-290.0, 290.0)                                # support centres
Y_AXIS_Z = SK_H                                         # 51: the supports stand on the base
Y_BED_Z = Y_AXIS_Z + BLK_C                              # 76: the blocks' bases up, the deck on them
DECK_X, DECK_Y = 430.0, 140.0                           # the deck, 860 x 280, over both rails' blocks
NOTCH_X, NOTCH_Y = 120.0, -100.0                        # the bite out of its front edge for the Z carriage plate
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
    box(p, "base, 900 x 760 ply", BASE["x"], BASE["y"], BASE["z"])


def deck(p):
    """One ply across both Y rails' blocks, the gantry's floor: racking
    is in-plane shear of this sheet. An H in plan, the bite out of the
    front edge for the Z carriage plate, which hangs to the board top."""
    s = p.sketch("top", Y_BED_Z, "deck")
    p.polygon(s, [(-DECK_X, YC - DECK_Y), (-NOTCH_X, YC - DECK_Y), (-NOTCH_X, YC + NOTCH_Y), (NOTCH_X, YC + NOTCH_Y),
                  (NOTCH_X, YC - DECK_Y), (DECK_X, YC - DECK_Y), (DECK_X, YC + DECK_Y), (-DECK_X, YC + DECK_Y)])
    p.extrude(s, PLY, op="new", name="deck, ply")


def wall(p):
    """One ply standing on the deck's back edge, full width, the X
    supports on its front face: with the deck it is an angle."""
    box(p, "wall, ply", (-DECK_X, DECK_X), (POST_FACE_Y, POST_FACE_Y + WALL_T), (Y_BED_Z + PLY, PLATE_TOP + 20.0))


def gussets(p):
    """A triangular web in each corner of the angle, glued to the deck and the wall."""
    for i, sx in enumerate((-Y_RAIL_X, Y_RAIL_X)):
        s = p.sketch("right", sx - PLY / 2, "gusset")
        p.polygon(s, [(POST_FACE_Y + WALL_T, Y_BED_Z + PLY), (YC + 135.0, Y_BED_Z + PLY), (POST_FACE_Y + WALL_T, PLATE_TOP)])
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
        ycyl(p, "20 mm shaft, Y, 600", Y_SHAFT[0], Y_SHAFT[1], sx, Y_AXIS_Z, SHAFT_D, op="new" if i == 0 else "add")


def y_blocks(p):
    first = True
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC - 115.0, YC + 115.0):
            box(p, "block", (sx - BLK_W / 2, sx + BLK_W / 2), (by - BLK_L / 2, by + BLK_L / 2), (Y_BED_Z - BLK_H, Y_BED_Z), op="new" if first else "add")
            first = False
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC - 115.0, YC + 115.0):
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
    # A few of the gap lattice's grooves, to show the pilot's work.
    s = p.sketch("top", z0 + PLATE_FLOOR + GROOVE_D + 1.0, "grooves")
    for k in range(-3, 4):
        p.rect(s, (PILOT_X - 152.0, TOOL_Y + k * 38.0 - GROOVE_W / 2), (PILOT_X + 152.0, TOOL_Y + k * 38.0 + GROOVE_W / 2))
    for k in range(-4, 5):
        p.rect(s, (PILOT_X + k * 38.0 - GROOVE_W / 2, TOOL_Y - 130.0), (PILOT_X + k * 38.0 + GROOVE_W / 2, TOOL_Y + 130.0))
    p.cut(s, GROOVE_D + 1.0, direction="reverse", name="gap lattice, as grooves")
    # The reference hole: 5 mm inside the front-left corner, outside the
    # board's outline. The pilot in it puts the bit 5 mm outside the blank's
    # corner; the plunge mark in the platform is what the slotted fence is set by.
    s = p.sketch("top", z0 + PLATE_FLOOR + GROOVE_D + 1.0, "reference")
    p.point(s, (PILOT_X - PLATE_W / 2 + REF_IN, TOOL_Y - PLATE_D / 2 + REF_IN))
    p.hole(s, PILOT_D + 0.05, depth=PLATE_FLOOR + GROOVE_D + 2.0, direction="reverse", name="2 mm reference hole")


PARTS = [
    ("Base", base), ("Y supports SK20", y_supports), ("Y shafts", y_shafts), ("Y blocks SC20UU", y_blocks), ("Deck", deck), ("Wall", wall), ("Gussets", gussets),
    ("X supports SK20", x_supports), ("X shafts", x_shafts), ("X blocks SC20UU", x_blocks),
    ("Z carriage plate", carriage_plate), ("Z supports SK20", z_supports), ("Z shafts", z_shafts), ("Z blocks SC20UU", z_blocks),
    ("Tool support", tool_support), ("Tool support webs", tool_support_webs), ("Clamp bolts", clamp_bolts), ("Router", router), ("Pilot", pilot), ("Stop screw", stop_screw),
    ("Platforms", platforms), ("Fences", fences), ("Blank", blank), ("Pattern plate", pattern),
]


GROUPS = [
    ("base_y", "Base and Y axis", ["Base", "Y supports SK20", "Y shafts", "Y blocks SC20UU", "Deck", "Platforms", "Fences"],
     "ply base 900 x 760; Y shafts 600 at x = +/-380, SK20s 580 apart; one deck 860 x 280 on the four blocks, 230 apart"),
    ("x_axis", "X axis", ["Deck", "Wall", "Gussets", "X supports SK20", "X shafts", "X blocks SC20UU"],
     "deck and wall, one ply each, an angle with a gusset in each corner; X shafts 760 at z = 160 and 310, SK20s 740 apart"),
    ("z_axis", "Z axis", ["X blocks SC20UU", "Z carriage plate", "Z supports SK20", "Z shafts", "Z blocks SC20UU"],
     "carriage plate 220 x 314, one ply, X blocks on its back; Z shafts 250 at x = +/-60, SK20s 210 apart"),
    ("tool_holder", "Tool holder", ["Tool support", "Tool support webs", "Clamp bolts", "Router", "Pilot", "Stop screw"],
     "tool plate 480 x 110 x 38; split clamps 66 (router) and 9.7 (chuck shank), slits to the front"),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "duplicator.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "duplicator"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Carving duplicator on round rail"}]})
    tabs, bodies = {}, {}
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
        print(f"{title:<20} tab {tab:>2}: {len(bodies[title])} bodies")
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Duplicator"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops = [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}} for t, _ in PARTS for k in range(len(bodies[t]))]
    text = mcp.call("apply", {"ops": ops, "tab": asm})
    if "ERROR:" in text:
        raise RuntimeError(text)
    print(f"assembly tab {asm}: {len(ops)} instances")
    for view, name in (("-0.55,-0.75,0.45", "iso"), ("0.75,-0.55,0.4", "iso_right"), ("front", "front"), ("right", "right"), ("top", "top")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{name}.png")})
    mcp.call("screenshot", {"tab": asm, "view": "front", "section": f"y:{TOOL_Y + 0.5}:flip", "width": 1600, "height": 1100, "path": os.path.join(OUT, "section_tools.png")})
    mcp.call("screenshot", {"tab": asm, "view": "right", "section": "x:0", "width": 1600, "height": 1100, "path": os.path.join(OUT, "section_carriage.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A2", "note": "round rail duplicator, puzzle size: X 760 stacked, Y 600, Z 250 shafts; one-piece gantry on the Y blocks", "path": os.path.join(OUT, "duplicator.pdf")}))
    # Sub-assembly sheets: each group of parts on its own tab, drawn at the
    # largest scale that fits an A3, with its own balloons and parts list.
    for file, title, group, note in GROUPS:
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": title}]})
        sub = int(re.search(r"tab (\d+)", text).group(1))
        ops = [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True,
                "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}} for t in group for k in range(len(bodies[t]))]
        text = mcp.call("apply", {"ops": ops, "tab": sub})
        if "ERROR:" in text:
            raise RuntimeError(text)
        mcp.call("screenshot", {"tab": sub, "view": "-0.55,-0.75,0.45", "width": 1600, "height": 1100, "path": os.path.join(OUT, f"{file}_iso.png")})
        print(mcp.call("export", {"tab": sub, "format": "pdf", "sheet": "A3", "note": note, "path": os.path.join(OUT, f"{file}.pdf")}))
    mcp.close()


if __name__ == "__main__":
    main()
