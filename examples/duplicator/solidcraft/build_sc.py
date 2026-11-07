"""The carving duplicator of `examples/duplicator/build.py`, rebuilt
through SolidCraft's MCP server instead of offkilter's: the same parts
at the same numbers, in place at mid travel with Z raised, as
components of one design; the three moving groups as sub-assemblies
(nested components) on slider joints; the poses of the depth sequence
and the reach driven and photographed; an interference sweep over the
work; the bill of materials; STEP and the native `.sc` file.

    cargo build --release -p solidcraft-cli      # in a SolidCraft checkout
    SOLIDCRAFT_CLI=.../target/release/solidcraft-cli python3 examples/duplicator/solidcraft/build_sc.py

What it cannot do, which `build.py` does, is said in the README beside
it: shop sheets, range-of-motion sheets, the duplicator check.
"""
import csv
import json
import math
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(os.path.dirname(HERE)))
from scmcp import Mcp, Part  # noqa: E402
import importlib.util  # noqa: E402

# Every number is build.py's: import its module for the constants.
spec = importlib.util.spec_from_file_location("okdup", os.path.join(os.path.dirname(HERE), "build.py"))
okdup = importlib.util.module_from_spec(spec)
sys.modules["okdup"] = okdup
spec.loader.exec_module(okdup)
globals().update({k: v for k, v in vars(okdup).items() if k.isupper() and k not in ("HERE", "OUT", "PARTS", "MOTION", "FIXED", "MATES")})

OUT = os.path.join(HERE, "out")
EXE = os.environ.get("SOLIDCRAFT_CLI", "solidcraft-cli")


# ---------------------------------------------------------------------
# The parts, each a function of a Part, as in build.py. Sketch planes
# became explicit cylinders and boxes: offkilter's "front" plane at
# offset o is y = -o with its normal -y, "right" is x = o with +x, "top"
# z = o with +z; a hole of depth d from a plane is a cylinder d long
# from it in the drilling direction.
# ---------------------------------------------------------------------
def base(p):
    p.box("base, 900 x 855 ply", BASE["x"], BASE["y"], BASE["z"])


def battens(p):
    for i, (y0, y1) in enumerate(BATTEN_Y):
        p.box("batten, two plies", BASE["x"], (y0, y1), (-PLY - BATTEN_H, -PLY), op="new" if i == 0 else "add")


def deck(p):
    p.box("deck, ply", (-DECK_X, DECK_X), (YC + DECK_FRONT, YC + DECK_BACK), (Y_BED_Z, Y_BED_Z + PLY))


def wall(p):
    p.box("wall, ply", (-DECK_X, DECK_X), (POST_FACE_Y, POST_FACE_Y + WALL_T), (Y_BED_Z + PLY, PLATE_TOP + 20.0))


def gussets(p):
    for i, sx in enumerate((-Y_RAIL_X, Y_RAIL_X)):
        p.prism_x("gusset, ply", sx - PLY / 2, [(POST_FACE_Y + WALL_T, Y_BED_Z + PLY), (YC + DECK_BACK - 5.0, Y_BED_Z + PLY), (POST_FACE_Y + WALL_T, PLATE_TOP)], PLY, op="new" if i == 0 else "add")


def y_supports(p):
    first = True
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for sy in Y_SK_Y:
            p.box("base", (sx - SK_W / 2, sx + SK_W / 2), (sy - SK_T / 2, sy + SK_T / 2), (0.0, 12.0), op="new" if first else "add")
            p.box("body", (sx - 16.0, sx + 16.0), (sy - SK_T / 2, sy + SK_T / 2), (0.0, SK_HTOT), op="add")
            first = False
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for sy in Y_SK_Y:
            p.bore("shaft bore", (sx, sy, Y_AXIS_Z), (0, 1, 0), SHAFT_D, SK_T + 2.0)


def y_shafts(p):
    for i, sx in enumerate((-Y_RAIL_X, Y_RAIL_X)):
        p.ycyl("20 mm shaft, Y, 700", Y_SHAFT[0], Y_SHAFT[1], sx, Y_AXIS_Z, SHAFT_D, op="new" if i == 0 else "add")


def y_blocks(p):
    first = True
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC + Y_BLOCK_Y[0], YC + Y_BLOCK_Y[1]):
            p.box("block", (sx - BLK_W / 2, sx + BLK_W / 2), (by - BLK_L / 2, by + BLK_L / 2), (Y_BED_Z - BLK_H, Y_BED_Z), op="new" if first else "add")
            first = False
    for sx in (-Y_RAIL_X, Y_RAIL_X):
        for by in (YC + Y_BLOCK_Y[0], YC + Y_BLOCK_Y[1]):
            p.bore("shaft bore", (sx, by, Y_AXIS_Z), (0, 1, 0), SHAFT_D, BLK_L + 2.0)


def x_supports(p):
    first = True
    for sx in (-X_SK_X, X_SK_X):
        for sz in X_AXIS_Z:
            p.box("base", (sx - SK_T / 2, sx + SK_T / 2), (POST_FACE_Y - 12.0, POST_FACE_Y), (sz - SK_W / 2, sz + SK_W / 2), op="new" if first else "add")
            p.box("body", (sx - SK_T / 2, sx + SK_T / 2), (POST_FACE_Y - SK_HTOT, POST_FACE_Y), (sz - 16.0, sz + 16.0), op="add")
            first = False
    for sx in (-X_SK_X, X_SK_X):
        for sz in X_AXIS_Z:
            p.bore("shaft bore", (sx, X_SHAFT_Y, sz), (1, 0, 0), SHAFT_D, SK_T + 2.0)


def x_shafts(p):
    for i, sz in enumerate(X_AXIS_Z):
        p.xcyl("20 mm shaft, X, 760", X_SHAFT[0], X_SHAFT[1], X_SHAFT_Y, sz, SHAFT_D, op="new" if i == 0 else "add")


def x_blocks(p):
    first = True
    for bx in (XC - 60.0, XC + 60.0):
        for sz in X_AXIS_Z:
            p.box("block", (bx - BLK_L / 2, bx + BLK_L / 2), (PLATE_BACK_Y, PLATE_BACK_Y + BLK_H), (sz - BLK_W / 2, sz + BLK_W / 2), op="new" if first else "add")
            first = False
    for bx in (XC - 60.0, XC + 60.0):
        for sz in X_AXIS_Z:
            p.bore("shaft bore", (bx, X_SHAFT_Y, sz), (1, 0, 0), SHAFT_D, BLK_L + 2.0)


def carriage_plate(p):
    p.box("Z carriage plate, ply", (XC - 110.0, XC + 110.0), (PLATE_FACE_Y, PLATE_BACK_Y), (Z0, PLATE_TOP))
    p.box("stop block", STOP_BLOCK["x"], STOP_BLOCK["y"], STOP_BLOCK["z"], op="add")


def z_supports(p):
    first = True
    for z0 in SK_Z:
        for sx in (-SHAFT_X, SHAFT_X):
            p.box("base", (sx - SK_W / 2, sx + SK_W / 2), (PLATE_FACE_Y - 12.0, PLATE_FACE_Y), (z0, z0 + SK_T), op="new" if first else "add")
            p.box("body", (sx - 16.0, sx + 16.0), (PLATE_FACE_Y - SK_HTOT, PLATE_FACE_Y), (z0, z0 + SK_T), op="add")
            first = False
    for z0 in SK_Z:
        for sx in (-SHAFT_X, SHAFT_X):
            p.bore("shaft bore", (sx, SHAFT_Y, z0 + SK_T / 2), (0, 0, 1), SHAFT_D, SK_T + 2.0)


def z_shafts(p):
    for i, sx in enumerate((-SHAFT_X, SHAFT_X)):
        p.vcyl("20 mm shaft, Z, 250", sx, SHAFT_Y, Z0, Z0 + 250.0, SHAFT_D, op="new" if i == 0 else "add")


def z_blocks(p):
    first = True
    for sx in (-SHAFT_X, SHAFT_X):
        for bz in (ZC - 40.0, ZC + 40.0):
            p.box("block", (sx - BLK_W / 2, sx + BLK_W / 2), (BLOCK_BASE_Y, BLOCK_BASE_Y + BLK_H), (bz - BLK_L / 2, bz + BLK_L / 2), op="new" if first else "add")
            first = False
    for sx in (-SHAFT_X, SHAFT_X):
        p.bore("shaft bore", (sx, SHAFT_Y, ZC), (0, 0, 1), SHAFT_D, 140.0)


def tool_support(p):
    zb = ZC - TOOL_Z_BELOW
    zm = zb + TOOL_T / 2
    front = TOOL_PLATE_Y[0]
    p.box("support plate, ply", (-110.0, 110.0), SUPPORT_Y, (zb, ZC + 65.0))
    p.box("tool plate, two plies", (BIT_X - 65.0, PILOT_X + 65.0), TOOL_PLATE_Y, (zb, zb + TOOL_T), op="add")
    p.box("stop ear", (-15.0, 15.0), EAR_Y, (ZC - 10.0, ZC + 9.0), op="add")
    p.vcyl("cable eye", 0.0, BAL_Y, ZC + 9.0, ZC + 9.0 + EYE_H, EYE_D, op="add")
    p.bore("router clamp bore, 66", (BIT_X, TOOL_Y, zb + TOOL_T / 2), (0, 0, 1), ROUTER_D + 1.0, TOOL_T + 2.0)
    half = BLOCK_W / 2 + BLOCK_PLAY
    p.cut_box("stylus block pocket, 62 x 62", (PILOT_X - half, PILOT_X + half), (TOOL_Y - half, TOOL_Y + half), (zb - 1.0, zb + TOOL_T + 1.0))
    for sx in (-HEAD_BOLT_X, HEAD_BOLT_X):
        p.bore("head bolt hole, 8", (PILOT_X + sx, TOOL_Y, zb + TOOL_T / 2), (0, 0, 1), HEAD_BOLT_D + 2 * BLOCK_PLAY, TOOL_T + 2.0)
    # The x screw from the right end, the spring hole in the pocket's far wall (both along x).
    d = 66.0 - half + 1.0
    p.cyl("x adjustment screw, M4 insert", (PILOT_X + 66.0 - d, TOOL_Y + ADJ_X_Y, zm), (1, 0, 0), ADJ_D, d, op="cut")
    p.cyl("x spring hole, 7", (PILOT_X - half + 1.0 - (SPRING_HOLE + 1.0), TOOL_Y + ADJ_X_Y, zm), (1, 0, 0), SPRING_D + 1.0, SPRING_HOLE + 1.0, op="cut")
    # The y screw from the front edge (y = front) back to the pocket, the spring hole in the far wall.
    d = TOOL_Y - half - front + 1.0
    p.cyl("y adjustment screw, M4 insert", (PILOT_X + ADJ_Y_X, front - 1.0, zm), (0, 1, 0), ADJ_D, d, op="cut")
    p.cyl("y spring hole, 7", (PILOT_X + ADJ_Y_X, TOOL_Y + half - 1.0, zm), (0, 1, 0), SPRING_D + 1.0, SPRING_HOLE + 1.0, op="cut")
    p.cut_box("router clamp slit, to the front", (BIT_X - SLIT_W / 2, BIT_X + SLIT_W / 2), (front - 1.0, TOOL_Y), (zb - 1.0, zb + TOOL_T + 1.0))
    wall_y = front + (TOOL_Y - ROUTER_D / 2 - 1.0 - front) / 2
    p.cyl("M6 clearance", (BIT_X - 66.0, wall_y, zm), (1, 0, 0), ROUTER_BOLT_D + 0.5, 66.0 + 32.0, op="cut")
    p.cut_box("nut pocket, from the top", (BIT_X + 26.0, BIT_X + 32.0), (wall_y - 5.0, wall_y + 5.0), (zb + TOOL_T - (TOOL_T / 2 + 5.0), zb + TOOL_T + 1.0))
    p.cyl("M8 tapped", (0.0, SCREW_Y, ZC + 10.0 - 21.0), (0, 0, 1), SCREW_D, 21.0, op="cut")


def clamp_bolts(p):
    zm = ZC - TOOL_Z_BELOW + TOOL_T / 2
    wall_y = TOOL_PLATE_Y[0] + (TOOL_Y - ROUTER_D / 2 - 1.0 - TOOL_PLATE_Y[0]) / 2
    p.xcyl("M6 x 100 knob bolt", BIT_X - 77.0, BIT_X + 30.0, wall_y, zm, ROUTER_BOLT_D)
    p.xcyl("knob", BIT_X - 77.0, BIT_X - 65.0, wall_y, zm, 25.0, op="add")


def stylus_block(p):
    zb = ZC - TOOL_Z_BELOW
    zm = zb + TOOL_T / 2
    hw = BLOCK_W / 2
    p.box("stylus block, two plies", (PILOT_X - hw, PILOT_X + hw), (TOOL_Y - hw, TOOL_Y + hw), (zb, zb + TOOL_T))
    p.box("head, one ply", (PILOT_X - HEAD_X / 2, PILOT_X + HEAD_X / 2), (TOOL_Y - HEAD_Y / 2, TOOL_Y + HEAD_Y / 2), (zb + TOOL_T, zb + TOOL_T + HEAD_T), op="add")
    top = zb + TOOL_T + HEAD_T
    for sx in (-BRIDGE_L / 2, BRIDGE_L / 2 - PLY):
        p.box("bridge foot", (PILOT_X + sx, PILOT_X + sx + PLY), (TOOL_Y - BRIDGE_W / 2, TOOL_Y + BRIDGE_W / 2), (top, top + FOOT_H), op="add")
    p.box("bridge, one ply", (PILOT_X - BRIDGE_L / 2, PILOT_X + BRIDGE_L / 2), (TOOL_Y - BRIDGE_W / 2, TOOL_Y + BRIDGE_W / 2), (top + FOOT_H, top + FOOT_H + PLY), op="add")
    p.cyl("chuck shank bore, 12.1", (PILOT_X, TOOL_Y, top + 1.0 - (TOOL_T + HEAD_T + 2.0)), (0, 0, 1), SHANK_D + 0.1, TOOL_T + HEAD_T + 2.0, op="cut")
    p.cut_box("clamp slit, to the front", (PILOT_X - PILOT_SLIT_W / 2, PILOT_X + PILOT_SLIT_W / 2), (TOOL_Y - HEAD_Y / 2 - 1.0, TOOL_Y), (top + 1.0 - (TOOL_T + HEAD_T + 2.0), top + 1.0))
    for sx in (-HEAD_BOLT_X, HEAD_BOLT_X):
        p.cyl("head bolt hole, 8", (PILOT_X + sx, TOOL_Y, top + 1.0 - (HEAD_T + 2.0)), (0, 0, 1), HEAD_BOLT_D + 2 * BLOCK_PLAY, HEAD_T + 2.0, op="cut")
    bolt_y, bolt_z = TOOL_Y - HEAD_Y / 2 + 10.0, top - HEAD_T / 2
    p.cyl("M4 clearance", (PILOT_X + HEAD_X / 2 + 1.0 - (HEAD_X + 2.0), bolt_y, bolt_z), (1, 0, 0), PILOT_BOLT_D + 0.5, HEAD_X + 2.0, op="cut")
    p.cut_box("nut pocket, from the top", (PILOT_X - 12.0, PILOT_X - 8.0), (bolt_y - 3.5, bolt_y + 3.5), (top + 1.0 - (HEAD_T / 2 + 1.0 + 4.0), top + 1.0))
    p.cyl("z screw, M4 T-nut from above", (PILOT_X, TOOL_Y, top + FOOT_H + PLY + 1.0 - (PLY + 2.0)), (0, 0, 1), ADJ_D, PLY + 2.0, op="cut")
    shank_top = ZC - TIP_BELOW + PILOT_OUT + CHUCK_L
    p.vcyl("M4 x 35 z screw", PILOT_X, TOOL_Y, shank_top, top + FOOT_H + PLY + 8.0, ADJ_D, op="new")
    p.vcyl("knob", PILOT_X, TOOL_Y, top + FOOT_H + PLY + 8.0, top + FOOT_H + PLY + 16.0, 16.0, op="add")
    p.xcyl("M4 x 50 x screw", PILOT_X + hw, PILOT_X + 65.0 + 8.0, TOOL_Y + ADJ_X_Y, zm, ADJ_D, op="new")
    p.xcyl("knob", PILOT_X + 65.0 + 8.0, PILOT_X + 65.0 + 16.0, TOOL_Y + ADJ_X_Y, zm, 16.0, op="add")
    p.xcyl("x spring, 6 x 20", PILOT_X - hw - SPRING_L, PILOT_X - hw, TOOL_Y + ADJ_X_Y, zm, SPRING_D, op="new")
    p.ycyl("M4 x 50 y screw", TOOL_PLATE_Y[0] - 16.0, TOOL_Y - hw, PILOT_X + ADJ_Y_X, zm, ADJ_D, op="new")
    p.ycyl("knob", TOOL_PLATE_Y[0] - 16.0, TOOL_PLATE_Y[0] - 8.0, PILOT_X + ADJ_Y_X, zm, 16.0, op="add")
    p.ycyl("y spring, 6 x 20", TOOL_Y + hw, TOOL_Y + hw + SPRING_L, PILOT_X + ADJ_Y_X, zm, SPRING_D, op="new")
    p.xcyl("M4 x 50 clamp knob bolt", PILOT_X - 10.0, PILOT_X + HEAD_X / 2 + 8.0, bolt_y, bolt_z, PILOT_BOLT_D, op="new")
    p.xcyl("knob", PILOT_X + HEAD_X / 2 + 8.0, PILOT_X + HEAD_X / 2 + 16.0, bolt_y, bolt_z, 16.0, op="add")


def balancer_bracket(p):
    p.box("bracket leg, ply", (-BAL_X, BAL_X), (PLATE_FACE_Y - PLY, PLATE_FACE_Y), BAL_LEG_Z)
    p.box("bracket arm, ply", (-BAL_X, BAL_X), (BAL_Y - 20.0, PLATE_FACE_Y), BAL_ARM_Z, op="add")
    p.bore("hook hole", (0.0, BAL_Y, (BAL_ARM_Z[0] + BAL_ARM_Z[1]) / 2), (0, 0, 1), 8.0, BAL_ARM_Z[1] - BAL_ARM_Z[0] + 2.0)
    p.vcyl("cable", 0.0, BAL_Y, ZC + 9.0 + EYE_H, REEL_Z - REEL_D / 2, CABLE_D, op="new")


def balancer_reel(p):
    p.box("hook", (-3.0, 3.0), (BAL_Y - 3.0, BAL_Y + 3.0), (REEL_Z + REEL_D / 2, BAL_ARM_Z[0] - 1.0))
    p.xcyl("spring balancer, 1 to 3 kg", -REEL_W / 2, REEL_W / 2, BAL_Y, REEL_Z, REEL_D, op="add")


def tool_support_webs(p):
    zb = ZC - TOOL_Z_BELOW
    for gx in (-100.0, 0.0, 100.0):
        p.prism_x("web, ply", gx - PLY / 2, [(SUPPORT_Y[0], zb + TOOL_T), (TOOL_PLATE_Y[0] + 15.0, zb + TOOL_T), (SUPPORT_Y[0], ZC + 55.0)], PLY, op="new")


def router(p):
    tip = ZC - TIP_BELOW
    p.vcyl("bit", BIT_X, TOOL_Y, tip, tip + BIT_OUT, BIT_D)
    p.vcyl("collet nut", BIT_X, TOOL_Y, tip + BIT_OUT, tip + BIT_OUT + COLLET_L, COLLET_D, op="add")
    p.vcyl("trim router motor", BIT_X, TOOL_Y, tip + BIT_OUT + COLLET_L, tip + BIT_OUT + COLLET_L + ROUTER_H, ROUTER_D, op="add")


def pilot(p):
    tip = ZC - TIP_BELOW
    p.vcyl("2 mm stylus", PILOT_X, TOOL_Y, tip, tip + PILOT_OUT + 5.0, PILOT_D)
    p.vcyl("ER11 nut", PILOT_X, TOOL_Y, tip + PILOT_OUT, tip + PILOT_OUT + NUT_L, NUT_D, op="add")
    p.vcyl("collet chuck C12-ER11A-100", PILOT_X, TOOL_Y, tip + PILOT_OUT + NUT_L, tip + PILOT_OUT + CHUCK_L, SHANK_D, op="add")


def stop_screw(p):
    tip = ZC - 10.0 - STOP_PROTRUDE
    p.vcyl("M8 stop screw", 0.0, SCREW_Y, tip, ZC + 25.0, SCREW_D)
    p.vcyl("knob", 0.0, SCREW_Y, ZC + 25.0, ZC + 37.0, 24.0, op="add")


def platforms(p):
    p.box("blank platform, ply", (BIT_X - PLATFORM_W / 2, BIT_X + PLATFORM_W / 2), (TOOL_Y - PLATFORM_D / 2, TOOL_Y + PLATFORM_D / 2), (0.0, PLY))
    p.box("pattern platform, 15.5 mm", (PILOT_X - PLATFORM_W / 2, PILOT_X + PLATFORM_W / 2), (TOOL_Y - PLATFORM_D / 2, TOOL_Y + PLATFORM_D / 2), (0.0, PLY - 3.5), op="add")


def blank(p):
    p.box("blank, 304 x 260 x 12", (BIT_X - BOARD_W / 2, BIT_X + BOARD_W / 2), (TOOL_Y - BOARD_D / 2, TOOL_Y + BOARD_D / 2), (PLY, BOARD_TOP))


def fences(p):
    x0, x1 = BIT_X - BOARD_W / 2, BIT_X + BOARD_W / 2
    y0, y1 = TOOL_Y - BOARD_D / 2, TOOL_Y + BOARD_D / 2
    p.box("blank back fence, slotted", (x0 - FENCE_W, x1), (y1, y1 + FENCE_W), (PLY, PLY + FENCE_H))
    p.box("blank left fence, slotted", (x0 - FENCE_W, x0), (y0, y1), (PLY, PLY + FENCE_H), op="add")
    for cx in (x0 + 30.0, BIT_X, x1 - 30.0):
        p.cut_box("slot, 6 x 16", (cx - 3.0, cx + 3.0), (y1 + 7.0, y1 + FENCE_W - 7.0), (PLY - 1.0, PLY + FENCE_H + 1.0))
    for cy in (y0 + 30.0, TOOL_Y, y1 - 30.0):
        p.cut_box("slot, 6 x 16", (x0 - FENCE_W + 7.0, x0 - 7.0), (cy - 3.0, cy + 3.0), (PLY - 1.0, PLY + FENCE_H + 1.0))
    zp = PLY - 3.5
    px0, px1 = PILOT_X - PLATE_W / 2, PILOT_X + PLATE_W / 2
    py0, py1 = TOOL_Y - PLATE_D / 2, TOOL_Y + PLATE_D / 2
    p.box("pattern back fence", (px0 - FENCE_T, px1), (py1, py1 + FENCE_T), (zp, zp + FENCE_H), op="new")
    p.box("pattern left fence", (px0 - FENCE_T, px0), (py0, py1), (zp, zp + FENCE_H), op="add")
    for cx in (px0 + 30.0, PILOT_X, px1 - 30.0):
        p.bore("screw hole", (cx, py1 + FENCE_T / 2, zp + FENCE_H / 2), (0, 0, 1), 4.0, FENCE_H + 2.0)
    for cy in (py0 + 30.0, TOOL_Y, py1 - 30.0):
        p.bore("screw hole", (px0 - FENCE_T / 2, cy, zp + FENCE_H / 2), (0, 0, 1), 4.0, FENCE_H + 2.0)


def pattern(p):
    z0 = PLY - 3.5
    top = z0 + PLATE_FLOOR + GROOVE_D
    p.box("printed pattern plate", (PILOT_X - PLATE_W / 2, PILOT_X + PLATE_W / 2), (TOOL_Y - PLATE_D / 2, TOOL_Y + PLATE_D / 2), (z0, top))
    grooves = [((PILOT_X - 152.0, TOOL_Y + k * 38.0 - GROOVE_W / 2), (PILOT_X + 152.0, TOOL_Y + k * 38.0 + GROOVE_W / 2)) for k in range(-3, 4)]
    grooves += [((PILOT_X + k * 38.0 - GROOVE_W / 2, TOOL_Y - 130.0), (PILOT_X + k * 38.0 + GROOVE_W / 2, TOOL_Y + 130.0)) for k in range(-4, 5)]
    for n, (a, b) in enumerate(grooves):
        p.cut_box(f"groove {n + 1}", (a[0], b[0]), (a[1], b[1]), (top - GROOVE_D, top + 1.0))
    p.bore("2 mm reference hole", (PILOT_X - PLATE_W / 2 + REF_IN, TOOL_Y - PLATE_D / 2 + REF_IN, (z0 + top) / 2), (0, 0, 1), PILOT_D + 0.05, PLATE_FLOOR + GROOVE_D + 2.0)


# (title, builder, parent component): the three moving groups are
# sub-assemblies, nested components with the parts inside them.
PARTS = [
    ("Base", base, "root"), ("Battens", battens, "root"), ("Y supports SK20", y_supports, "root"), ("Y shafts", y_shafts, "root"),
    ("Y blocks SC20UU", y_blocks, "Gantry"), ("Deck", deck, "Gantry"), ("Wall", wall, "Gantry"), ("Gussets", gussets, "Gantry"),
    ("X supports SK20", x_supports, "Gantry"), ("X shafts", x_shafts, "Gantry"),
    ("X blocks SC20UU", x_blocks, "X slide"), ("Z carriage plate", carriage_plate, "X slide"), ("Z supports SK20", z_supports, "X slide"),
    ("Z shafts", z_shafts, "X slide"), ("Balancer bracket", balancer_bracket, "X slide"), ("Balancer reel", balancer_reel, "X slide"),
    ("Z blocks SC20UU", z_blocks, "Z slide"), ("Tool support", tool_support, "Z slide"), ("Tool support webs", tool_support_webs, "Z slide"),
    ("Clamp bolts", clamp_bolts, "Z slide"), ("Router", router, "Z slide"), ("Stylus block", stylus_block, "Z slide"), ("Pilot", pilot, "Z slide"),
    ("Stop screw", stop_screw, "root"),
    ("Platforms", platforms, "root"), ("Fences", fences, "root"), ("Blank", blank, "root"), ("Pattern plate", pattern, "root"),
]
GROUPS = ["Gantry", "X slide", "Z slide"]
FIXED = ["Base", "Battens", "Y supports SK20", "Y shafts", "Platforms", "Fences", "Blank", "Pattern plate"]

# Travel per axis from the supports and blocks, as the regression test
# measures it off the placed bodies: block edge to support edge.
Y_TRAVEL = (Y_SK_Y[0] + SK_T / 2) - (Y_BLOCK_Y[0] - BLK_L / 2), (Y_SK_Y[1] - SK_T / 2) - (Y_BLOCK_Y[1] + BLK_L / 2)
X_TRAVEL = (-X_SK_X + SK_T / 2) - (XC - 60.0 - BLK_L / 2), (X_SK_X - SK_T / 2) - (XC + 60.0 + BLK_L / 2)
Z_TRAVEL = (SK_Z[0] + SK_T) - (ZC - 40.0 - BLK_L / 2), SK_Z[1] - (ZC + 40.0 + BLK_L / 2)
# (name, a: placed side, b: moving side, point on the axis, axis, limits)
JOINTS = [
    ("Y travel", "Y shafts", "Gantry", [-Y_RAIL_X, YC, Y_AXIS_Z], [0, 1, 0], [Y_TRAVEL[0], Y_TRAVEL[1]]),
    ("X travel", "Gantry", "X slide", [XC, X_SHAFT_Y, X_AXIS_Z[0]], [1, 0, 0], [X_TRAVEL[0], X_TRAVEL[1]]),
    ("Z travel", "X slide", "Z slide", [-SHAFT_X, SHAFT_Y, ZC], [0, 0, 1], [Z_TRAVEL[0], Z_TRAVEL[1]]),
    ("stop screw", "Z slide", "Stop screw", [0.0, SCREW_Y, ZC], [0, 0, 1], [-30.0, 40.0]),
]


def bbox(mcp, occurrence):
    r = mcp.execute("occurrence.properties", {"occurrence": occurrence})
    b = r["total"]["bbox"]
    return b["min"], b["max"]


def main():
    os.makedirs(OUT, exist_ok=True)
    for f in os.listdir(OUT):
        os.remove(os.path.join(OUT, f))
    t0 = time.time()
    mcp = Mcp(EXE, OUT)
    log = {"parts": {}, "timings": {}}
    try:
        mcp.call("new_design", {"name": "Carving duplicator on round rail"})
        mcp.execute("document.metadata.set", {"values": {"project": "Carving duplicator on round rail", "description": "Woodsmith SN12918 on 20 mm round rail, built through SolidCraft's MCP server"}})
        # The three sub-assemblies first, empty, so the parts can be made inside them.
        mcp.batch([{"command": "component.create", "params": {"name": g, "parent": "root", "activate": False, "grounded": False}} for g in GROUPS])
        bodies = 0
        parts_bodies = {}
        for title, fn, parent in PARTS:
            t = time.time()
            part = Part(mcp, title, parent)
            fn(part)
            r = part.build()
            dt = time.time() - t
            bodies += len(part.bodies)
            parts_bodies[title] = list(part.bodies)
            log["parts"][title] = {"bodies": len(part.bodies), "steps": len(part.cmds), "seconds": round(dt, 2)}
            print(f"{title:<20} {len(part.bodies):>2} bodies, {len(part.cmds):>3} steps, {dt:5.1f} s")
        log["timings"]["parts"] = round(time.time() - t0, 1)
        # A colour per part, as offkilter's client gives every body.
        palette = ["#8fa3bf", "#b08968", "#9ec29a", "#c48fa8", "#c9b36a", "#7fb8b8", "#b6a1d6", "#d9a06b"]
        paint = []
        for i, (title, _, _) in enumerate(PARTS):
            paint.append({"command": "appearance.assign", "params": {"bodies": parts_bodies[title], "color": palette[i % len(palette)]}})
        try:
            mcp.batch(paint)
        except RuntimeError as e:
            log["paint"] = str(e)[:300]
        m = mcp.call("measure", {"summary": True})
        print(f"{m['body_count']} bodies, {m['total']['volume_mm3'] / 1000:.0f} cm3, {log['timings']['parts']} s")
        assert m["body_count"] == bodies, (m["body_count"], bodies)
        not_closed = [b["name"] for b in m["bodies"] if not b.get("closed", True)]
        assert not not_closed, not_closed

        # Ground the fixed parts, then the four sliders between the groups.
        t = time.time()
        mcp.batch([{"command": "occurrence.ground", "params": {"occurrence": f"{name}:1", "grounded": True}} for name in FIXED])
        before = {b: bbox(mcp, f"{b}:1") for _, _, b, _, _, _ in JOINTS}
        for name, a, b, point, z, limits in JOINTS:
            r = mcp.execute("joint.create", {"type": "slider", "name": name, "flip": True, "limits": [limits],
                                             "a": {"occurrence": a, "point": point, "z": z}, "b": {"occurrence": b, "point": point, "z": z}, "quiet": True})
            after = bbox(mcp, f"{b}:1")
            moved = max(abs(x - y) for x, y in zip(before[b][0] + before[b][1], after[0] + after[1]))
            assert moved < 1e-6, f"{name}: joining moved {b} by {moved}"
            print(f"joint {name:<10} {a} -> {b}: limits {limits[0]:g}..{limits[1]:g} mm, nothing moved")
        jl = mcp.execute("joint.list")
        assert not jl.get("conflicts"), jl.get("conflicts")
        assert not jl.get("floating"), jl.get("floating")
        dof = {o["occurrence"]: o["dof"] for o in jl.get("dof", [])}
        log["joints"] = {"dof": dof, "fixed": jl.get("fixed"), "seconds": round(time.time() - t, 2)}
        print(f"joints: {len(jl['joints'])} sliders, {len(jl.get('fixed', []))} fixed occurrences, dof per occurrence {sorted(set(dof.values()))}")

        at = {name: 0.0 for name, *_ in JOINTS}

        def drive(**values):
            for name, v in values.items():
                if abs(v - at[name]) < 1e-9:
                    continue
                r = mcp.execute("joint.drive", {"joint": name, "value": v, "quiet": True})
                assert r.get("moved"), f"{name} -> {v}: {r.get('reason')}"
                at[name] = r["values"][0]["value"]
                assert abs(at[name] - v) < 1e-6, f"{name} -> {v}: held at {at[name]}"

        def tip(part):
            lo, hi = bbox(mcp, f"{part}:1")
            return [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2]]

        # Drive each slider 10 mm and read where the part went.
        for name, _, b, _, z, limits in JOINTS:
            step = 10.0 if limits[1] >= 10.0 else -10.0       # within the joint's limits, which clamp a drive
            p0 = bbox(mcp, f"{b}:1")[0]
            drive(**{name: step})
            p1 = bbox(mcp, f"{b}:1")[0]
            drive(**{name: 0.0})
            d = [x - y for x, y in zip(p1, p0)]
            assert all(abs(d[i] - step * z[i]) < 1e-6 for i in range(3)), f"{name}: moved {d}"
        print("each slider moves its group along its axis")

        # Pictures of the drawn pose.
        views = {"iso": "iso", "iso_right": "iso", "front": "front", "right": "right", "top": "top"}
        for stem, view in (("iso", "iso"), ("front", "front"), ("right", "right"), ("top", "top")):
            r = mcp.call("screenshot", {"view": view, "path": os.path.join(OUT, f"{stem}.png"), "width": 1600, "height": 1100, "grid": False, "source": "model"})
            log.setdefault("renderer", r.get("renderer") if isinstance(r, dict) else None)
        # A section through the tools (y = TOOL_Y) from the front, and through the carriage (x = 0) from the right.
        mcp.execute("inspect.section", {"plane": "XZ", "at": TOOL_Y + 0.5})
        mcp.call("screenshot", {"view": "front", "path": os.path.join(OUT, "section_tools.png"), "width": 1600, "height": 1100, "grid": False, "source": "model"})
        mcp.execute("inspect.section", {"plane": "YZ", "at": 0.0})
        mcp.call("screenshot", {"view": "right", "path": os.path.join(OUT, "section_carriage.png"), "width": 1600, "height": 1100, "grid": False, "source": "model"})
        mcp.execute("inspect.section", {"clear": True})
        for g in GROUPS:
            pass  # a sub-assembly alone cannot be photographed without hiding the rest; see the README

        # The depth sequence and the reach, as motion_z.pdf and motion_xy.pdf draw them.
        bit, pil = tip("Router"), tip("Pilot")
        blank_lo, blank_hi = bbox(mcp, "Blank:1")
        plate_lo, plate_hi = bbox(mcp, "Pattern plate:1")
        raised = bit[2] - blank_hi[2]
        landing = (ZC - 10.0 - STOP_PROTRUDE) - STOP_BLOCK["z"][1]
        last = raised + GROOVE_D
        log["depth"] = {"raised": raised, "first_pass": raised - landing, "last_pass": GROOVE_D, "drop": last, "z_travel_down": -Z_TRAVEL[0]}
        print(f"tips {raised:g} above the board; first pass {raised - landing:g} mm on the screw; last pass {GROOVE_D:g} mm, {last:g} of the {-Z_TRAVEL[0]:g} mm of travel")
        assert last <= -Z_TRAVEL[0]
        z_poses = [("raised", 0.0, 0.0), ("first pass, on the screw", -landing, 0.0), ("last pass, screw backed off", -last, last - landing)]
        for i, (label, dz, screw) in enumerate(z_poses):
            drive(**{"Z travel": dz, "stop screw": screw})
            mcp.call("screenshot", {"view": "front", "path": os.path.join(OUT, f"motion_z_{i + 1}.png"), "width": 1200, "height": 800, "grid": False, "source": "model"})
            print(f"  pose {i + 1}: {label}: bit tip {tip('Router')[2] - blank_hi[2]:g} above the board")
        drive(**{"Z travel": 0.0, "stop screw": 0.0})
        xy_poses = [("front-left corner, reference hole", -BOARD_W / 2, -PLATE_D / 2 + REF_IN), ("centre", 0.0, 0.0), ("back-right corner", BOARD_W / 2, BOARD_D / 2)]
        for i, (label, dx, dy) in enumerate(xy_poses):
            drive(**{"X travel": dx, "Y travel": dy})
            mcp.call("screenshot", {"view": "top", "path": os.path.join(OUT, f"motion_xy_{i + 1}.png"), "width": 1200, "height": 800, "grid": False, "source": "model"})
            b = tip("Router")
            print(f"  pose {i + 1}: {label}: bit at ({b[0]:g}, {b[1]:g})")
        drive(**{"X travel": 0.0, "Y travel": 0.0})
        # Reach: the bit over the whole blank, the pilot over the lattice.
        assert bit[0] + X_TRAVEL[0] <= blank_lo[0] and bit[0] + X_TRAVEL[1] >= blank_hi[0], "bit over the blank in x"
        assert bit[1] + Y_TRAVEL[0] <= blank_lo[1] and bit[1] + Y_TRAVEL[1] >= blank_hi[1], "bit over the blank in y"
        assert pil[0] + X_TRAVEL[0] <= plate_lo[0] + 10.0 and pil[0] + X_TRAVEL[1] >= plate_hi[0] - 10.0, "pilot over the lattice in x"
        print(f"reach: X {X_TRAVEL[0]:g}..{X_TRAVEL[1]:g}, Y {Y_TRAVEL[0]:g}..{Y_TRAVEL[1]:g}, Z {Z_TRAVEL[0]:g}..{Z_TRAVEL[1]:g}")
        log["timings"]["poses"] = round(time.time() - t, 1)

        # The sweep: the bit at the blank's edges and middle, every 25 mm
        # in Y from the reference hole to the back edge, first and last
        # pass, and the interference check at every pose. Only the tools
        # in the work may meet.
        t = time.time()
        xs = [blank_lo[0] - bit[0], 0.0, blank_hi[0] - bit[0]]
        y0 = min(plate_lo[1] + 5.0 - bit[1], blank_lo[1] - bit[1])
        y1 = blank_hi[1] - bit[1]
        steps = math.ceil((y1 - y0) / 25.0)
        ys = [y0 + (y1 - y0) * i / steps for i in range(steps + 1)]
        first_pass = -(raised) + 4.0
        last_pass = -(bit[2] - plate_hi[2]) - 12.5
        sweep_every = int(os.environ.get("SWEEP_EVERY", "1"))
        poses, hits, checked, budget, in_work_seen = 0, [], 0, 0.0, 0
        in_work = lambda n: n.startswith("Router") or n.startswith("Pilot")
        work = lambda n: any(n.startswith(w) for w in ("Blank", "Pattern plate", "Platforms", "Fences"))
        for dx in xs:
            for dy in ys[::sweep_every]:
                for dz in (first_pass, last_pass):
                    poses += 1
                    drive(**{"X travel": dx, "Y travel": dy, "Z travel": dz, "stop screw": max(-dz - landing, 0.0)})
                    r = mcp.execute("inspect.interference", {"budget_s": 120})
                    checked += r.get("pairs_intersected", 0)
                    budget += r.get("ms", 0) / 1000
                    for i in r.get("interferences", []):
                        a, b = i["a"], i["b"]
                        v = i.get("volume_mm3", i.get("volume", 0.0))
                        tool = (in_work(a) and work(b)) or (in_work(b) and work(a))
                        if tool and v <= 100.0:
                            in_work_seen += 1
                            continue
                        if v > 1e-3:
                            hits.append(f"{a} x {b} at ({dx:g}, {dy:g}, {dz:g}): {v:.1f} mm3" + (" of tool in the work" if tool else ""))
                    for u in r.get("unchecked", []):
                        hits.append(f"unchecked {u}")
        drive(**{"X travel": 0.0, "Y travel": 0.0, "Z travel": 0.0, "stop screw": 0.0})
        log["sweep"] = {"poses": poses, "pairs_intersected": checked, "hits": hits, "tool_in_work_overlaps": in_work_seen, "seconds": round(time.time() - t, 1), "interference_seconds": round(budget, 1)}
        print(f"sweep: {poses} poses, {checked} pairs intersected, {in_work_seen} tool-in-work overlaps, {len(hits)} hits, {time.time() - t:.0f} s")
        for h in hits:
            print("  " + h)

        # The bill of materials, the mass, and the files.
        t = time.time()
        bom = mcp.execute("assembly.bom", {"levels": "top"})
        with open(os.path.join(OUT, "bom.json"), "w") as f:
            json.dump(bom, f, indent=1)
        rows = bom.get("rows") or bom.get("items") or bom.get("lines") or []
        with open(os.path.join(OUT, "bom.csv"), "w", newline="") as f:
            w = csv.writer(f)
            keys = sorted({k for r in rows for k in r.keys() if not isinstance(r[k], (dict, list))})
            w.writerow(keys)
            for r in rows:
                w.writerow([r.get(k, "") for k in keys])
        print(f"bom: {len(rows)} rows")
        with open(os.path.join(OUT, "hardware.csv"), "w") as f:
            f.write("joint,fastener,count\n")
            for joint, fastener, count in HARDWARE:
                f.write(f'"{joint}","{fastener}",{count}\n')
        try:
            r = mcp.execute("file.export_view_dxf", {"path": os.path.join(OUT, "front.dxf"), "camera": {"target": [0.0, 0.0, 200.0], "yaw": 0.0, "pitch": 0.0, "distance": 2500.0, "fov": 0.0}})
            log["view_dxf"] = "ok"
        except RuntimeError as e:
            log["view_dxf"] = str(e)[:300]
        mcp.call("export", {"path": os.path.join(OUT, "duplicator.step")})
        mcp.call("save", {"path": os.path.join(OUT, "duplicator.sc")})
        log["timings"]["files"] = round(time.time() - t, 1)
        log["timings"]["total"] = round(time.time() - t0, 1)
        log["calls"] = mcp.calls
        log["seconds_in_calls"] = round(mcp.seconds, 1)
        log["bodies"] = bodies
        log["sizes"] = {f: os.path.getsize(os.path.join(OUT, f)) for f in sorted(os.listdir(OUT))}
        print(f"done: {mcp.calls} calls, {log['timings']['total']} s")
    finally:
        with open(os.path.join(OUT, "build.json"), "w") as f:
            json.dump(log, f, indent=1)
        mcp.close()


if __name__ == "__main__":
    main()
