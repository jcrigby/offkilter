"""An indexed stick mill: a reduced duplicator that mills the leaning
half-ellipse Spectre's whole outline round one solid stick. The stick
turns between a headstock and a tail centre, its spindle in two ball
bearings. A printed twelve-pointed star on the spindle (a hexagon and
the same hexagon turned 30°) is held at any of twelve stops by one
cradle, the negative of its lower three points, which is the only
part that moves: it slides along the spindle's axis in a guide on the
base, onto the star to lock it and off to let the stick turn. Star
and pocket taper, so the cradle seats without play. With the cradle
back, a crank turns the stick under the bit: that is how the two
round journals are cut that the stick then rests on. A trim router in
its plunge base stands on a shelf out from a ply square that rides
two 20 mm shafts, one above the other behind the stick, each held at
both ends. Beside the stick, on the same spindle, is a printed
pattern: the tile's section at full size. For each pass the carriage
is brought over the pattern, the cross slide stepped by its screw,
the router (off) plunged until the bit itself touches the pattern,
the plunge locked, and the carriage run along the stick.

    cargo build --release -p ok-mcp
    python3 examples/stick-mill/build.py
    cargo test -p ok-render --test stick_mill   # what CI runs

Outputs under out/: mill.okpart (every part in place and the machine
assembled), pattern.stl, star.stl, cradle.stl, cradle_guide.stl and
post.stl (two) to print, mill.png, end.png, journal.png and
head.png, mill.pdf, and mill.json (the numbers and the pass plan).
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")

# The tile: the leaning half ellipse, drawn for the bit that cuts it.
EDGE, BULGE, GAP = 10.56, 0.4, 0.7              # the lid's edge; 0.8 of a semicircle deep; the line between tiles
BIT = 3.0                                       # a ball-nose, 1.5 radius: the kernel fillets the inside corners to it
RIDGE = 0.05                                    # what a pass may leave standing between it and the next, to sand
MARK = 0.125                                    # the cross screw's handwheel: ten marks a turn of M8
ROUGH_BIT, ROUGH_BITE = 6.0, 3.0                # roughing: the journals' straight bit, and the most it takes at a bite
ROUGH_STEP = 3.0                                # its places across, half its width apart
ALLOW = 0.4                                     # what roughing leaves on the tile for the ball-nose: a feeler under the depth stop
BALL_BITE = 1.5                                 # the most the ball-nose takes at a bite
BLANK = 2.0                                     # the blank: sawn to the tile's hull and this much over
CELL = 0.15                                     # the grid the waste is followed on
STOPS = 12                                      # index positions, 30° apart
STICK = 200.0                                   # the stick's length, as the cauls'

# The owner's hardware, as the duplicator's.
PLY = 19.0
SHAFT_D = 20.0
SK_W, SK_T, SK_H, SK_HTOT = 60.0, 20.0, 51.0, 70.0   # SK20: base width, thickness along the shaft, base to centre, height
BLK_W, BLK_L, BLK_H, BLK_C = 50.0, 45.0, 42.0, 25.0  # SC20UU: width across, length along the shaft, height, base to centre
ROUTER_D, ROUTER_H = 65.0, 126.0                # the Colt's motor
ROUTER_BASE = 100.0                             # its plunge base, square
PLUNGE = 50.0                                   # the base's travel
REACH = 50.0                                    # the bit's tip under the base's sole at full plunge: a tapered ball-nose on a 6 mm shank, this far out of the collet

# The machine. x along the stick from its headstock end, y across (the rails behind, at +y), z up from the base's top.
SLIDE_T = 6.0                                   # the cross slide: an aluminium plate, the router's base on it
SLIDE_Z = 98.0                                  # the shelf's top, where the cross slide lies
AXIS_Z = SLIDE_Z - 32.0                         # 66: the stick's axis, as low as the star lets it be: its point 57.7 under, 5.3 of cradle and the guide's floor. A blank sawn within 2 mm of the tile swings 29
PATTERN_T = 20.0                                # the pattern, against the stick's end on the same spindle
RUN = 6.0                                       # the bit past the stick's far end
BIT_X = (-PATTERN_T / 2, STICK + RUN)           # where the bit goes: mid pattern to past the stick's far end
CAR_AT = BIT_X[0]                               # the carriage as drawn: over the pattern, setting a pass
# The journals: two lengths of the stick turned first, while the blank is whole, to a round about the axis
# that lies inside the tile, so no later pass touches them; a printed post under each holds the stick while it is milled.
JOURNAL_AT = (STICK / 3.0, 2.0 * STICK / 3.0)   # their middles
JOURNAL_L = 14.0                                # along the stick: a post and 2 mm each side
JOURNAL_IN = 0.1                                # the round this far inside the tile's largest circle
JOURNAL_BIT = 6.0                               # a straight bit for this step, lowered a bite a turn of the crank
MASTER_T = 10.0                                 # the journal's master, the same round, printed on the pattern's outer face: the straight bit is set on it
POST_T, POST_WALL = 10.0, 3.0                   # a post along the stick, and its plastic outside the seat
POST_FOOT = (30.0, 5.0, 18.0)                   # its foot: along the stick, thick, and past the upright each side for a screw
POST_CLEAR = 1.0                                # the least a bit may pass over a post
CAR_MIN = -PATTERN_T - MASTER_T / 2             # the carriage's furthest: the bit over the journal's master
# The carriage: a ply square upright behind the stick, a bearing block at each corner, on two shafts one above the other.
SQUARE = 200.0
FACE_Y = 100.0                                  # the square's front face, behind anything the stick swings
RAIL_Y = FACE_Y + PLY + BLK_C                   # 144: the shafts
RAIL_Z = (AXIS_Z + 1.0, AXIS_Z + 151.0)         # stacked, 150 apart, the lower at the stick's height
BLOCK_X = 72.5                                  # the blocks either side of the square's middle
WALL_Y = (RAIL_Y + SK_H, RAIL_Y + SK_H + 2 * PLY)   # the back wall, two plies: the shaft supports bolt to its face at both ends
SUPPORT_X = (CAR_MIN - SQUARE / 2 - 30.0, BIT_X[1] + SQUARE / 2 + 30.0)   # the supports, clear of the square at both ends of its travel
SHAFT_X = (SUPPORT_X[0] - SK_T / 2 - 5.0, SUPPORT_X[1] + SK_T / 2 + 5.0)
GUSSET_D, GUSSET_H = 120.0, 200.0               # the wall's braces, in front of it at each end, outside the shafts' ends
WALL_X = (SHAFT_X[0] - 6.0 - PLY, SHAFT_X[1] + 6.0 + PLY)
WALL_TOP = RAIL_Z[1] + 45.0
BASE_X = (WALL_X[0] - 10.0, WALL_X[1] + 10.0)
BASE_Y = (-150.0, WALL_Y[1] + 10.0)
# The router's shelf: out from the square's face over the stick, the cross slide on it.
SHELF_W = 150.0                                 # along the stick
SHELF_Y = (-95.0, FACE_Y)
SLIDE_L, SLIDE_W = 190.0, 130.0                 # the cross slide, across and along
SLOT = 40.0                                     # its travel across, either side: the tile is 51 across
SCREW_D = 8.0                                   # the cross slide's screw: M8, 1.25 mm a turn
# The spindle and its twelve stops: a twelve-pointed star, and one cradle that slides onto it.
SPINDLE_D = 12.0                                # M12 studding
BEARING_D, BEARING_W = 32.0, 10.0               # 6201: 12 bore, one in each face of the headstock
HEX_AF, HEX_T = 100.0, 20.0                     # the star: two hexagons this far across their flats, 30° apart, printed
HEX_R = HEX_AF / math.sqrt(3.0)                 # its points' radius, 57.7
STAR_IN = HEX_AF / 2.0 / math.cos(math.radians(15.0))   # its inside corners' radius, 51.8
TAPER = 10.0                                    # star and pocket narrow this much a side toward the cradle's way off: it seats as a cone does, and lets go
STAR_K = 1.0 - HEX_T * math.tan(math.radians(TAPER)) / HEX_R   # the small end's scale, 0.939
TIP_FLAT, EASE = 1.5, 1.0                       # the star's points blunted and the pocket's crests cut back, so only flanks meet
HEX_X = (CAR_MIN - SHELF_W / 2 - 15.0 - HEX_T, CAR_MIN - SHELF_W / 2 - 15.0)   # outside the shelf's reach; the large end toward the headstock
CRADLE_W = 150.0                                # the cradle across
TRAVEL = 26.0                                   # the cradle's way off the star, toward the pattern: the star's thickness and 6
GUIDE_FLOOR, GUIDE_LIP, GUIDE_WALL, GUIDE_PLAY, GUIDE_EAR = 3.0, 6.0, 5.0, 0.15, 18.0   # the guide: screwed to the base, the cradle between its lips
GUIDE_END = (12.0, 24.0)                        # its end wall, thick and high over the floor: the push screw's nut
PUSH_D = 6.0                                    # the push screw, M6 with a knob
HEAD_X = (HEX_X[0] - 25.0 - 2 * PLY, HEX_X[0] - 25.0)   # the headstock upright: two plies, a bearing in each face
TAIL_X = (STICK + 30.0, STICK + 30.0 + PLY)     # the tail centre's upright
UPRIGHT_Y = 70.0
UPRIGHT_TOP = AXIS_Z + 25.0
CRANK_D, CRANK_T = 60.0, 10.0                   # the crank, outboard of the headstock


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
    s = p.sketch("front", -y1, name)
    p.circle(s, (cx, cz), d / 2)
    return p.extrude(s, y1 - y0, op=op, name=name)


def xbore(p, x0, x1, cy, cz, d, name):
    s = p.sketch("right", x0 - 1.0, name)
    p.circle(s, (cy, cz), d / 2)
    p.cut(s, x1 - x0 + 2.0, name=name)


# -- the parts, each drawn where it stands ----------------------------------

def base(p):
    box(p, "base", BASE_X, BASE_Y, (-PLY, 0.0))


def wall(p):
    box(p, "back wall, two plies", WALL_X, WALL_Y, (0.0, WALL_TOP))
    for i, x in enumerate((WALL_X[0], WALL_X[1] - PLY)):
        s = p.sketch("right", x, "gusset")
        p.polygon(s, [(WALL_Y[0], 0.0), (WALL_Y[0], GUSSET_H), (WALL_Y[0] - GUSSET_D, 0.0)])
        p.extrude(s, PLY, op="add", name="brace, in front")


def supports(p):
    """Four SK20s on the wall's face: both ends of both shafts."""
    spots = [(sx, sz) for sx in SUPPORT_X for sz in RAIL_Z]
    for i, (sx, sz) in enumerate(spots):
        box(p, "base", (sx - SK_T / 2, sx + SK_T / 2), (WALL_Y[0] - 12.0, WALL_Y[0]), (sz - SK_W / 2, sz + SK_W / 2), op="new" if i == 0 else "add")
        box(p, "body", (sx - SK_T / 2, sx + SK_T / 2), (WALL_Y[0] - SK_HTOT, WALL_Y[0]), (sz - 16.0, sz + 16.0), op="add")
    for sx, sz in spots:
        xbore(p, sx - SK_T / 2, sx + SK_T / 2, RAIL_Y, sz, SHAFT_D, "shaft bore")


def shafts(p):
    for i, sz in enumerate(RAIL_Z):
        xcyl(p, f"20 mm shaft, {SHAFT_X[1] - SHAFT_X[0]:.0f}", SHAFT_X[0], SHAFT_X[1], RAIL_Y, sz, SHAFT_D, op="new" if i == 0 else "add")


def blocks(p):
    """Four SC20UU, one behind each corner of the square, bases forward."""
    spots = [(CAR_AT + bx, sz) for bx in (-BLOCK_X, BLOCK_X) for sz in RAIL_Z]
    for i, (bx, sz) in enumerate(spots):
        box(p, "block", (bx - BLK_L / 2, bx + BLK_L / 2), (RAIL_Y - BLK_C, RAIL_Y - BLK_C + BLK_H), (sz - BLK_W / 2, sz + BLK_W / 2), op="new" if i == 0 else "add")
    for bx, sz in spots:
        xbore(p, bx - BLK_L / 2, bx + BLK_L / 2, RAIL_Y, sz, SHAFT_D, "shaft bore")


def square(p):
    mid = (RAIL_Z[0] + RAIL_Z[1]) / 2
    box(p, "carriage square", (CAR_AT - SQUARE / 2, CAR_AT + SQUARE / 2), (FACE_Y, FACE_Y + PLY), (mid - SQUARE / 2, mid + SQUARE / 2))


def shelf(p):
    """Out from the square over the stick, a gusset under each edge, an opening for the bit."""
    box(p, "shelf", (CAR_AT - SHELF_W / 2, CAR_AT + SHELF_W / 2), SHELF_Y, (SLIDE_Z - PLY, SLIDE_Z))
    s = p.sketch("top", SLIDE_Z, "bit's way")
    p.rect(s, (CAR_AT - 20.0, -SLOT - 20.0), (CAR_AT + 20.0, SLOT + 20.0))
    p.cut(s, PLY + 1.0, direction="reverse", name="opening across the stick")
    for x in (CAR_AT - SHELF_W / 2, CAR_AT + SHELF_W / 2 - PLY):
        s = p.sketch("right", x, "gusset")
        p.polygon(s, [(FACE_Y, SLIDE_Z), (FACE_Y, SLIDE_Z + 110.0), (FACE_Y - 150.0, SLIDE_Z)])
        p.extrude(s, PLY, op="add", name="gusset over the shelf's edge")


def slide(p):
    """The cross slide at mid travel: the router's base on it, a hole for the bit."""
    box(p, "cross slide, 6 mm aluminium", (CAR_AT - SLIDE_W / 2 + PLY + 1.0, CAR_AT + SLIDE_W / 2 - PLY - 1.0), (-SLIDE_L / 2, SLIDE_L / 2 - 25.0), (SLIDE_Z, SLIDE_Z + SLIDE_T))
    s = p.sketch("top", SLIDE_Z + SLIDE_T, "bit")
    p.point(s, (CAR_AT, 0.0))
    p.hole(s, 30.0, name="for the bit and the collet")


def router(p):
    w = SLIDE_W / 2 - PLY - 1.0
    box(p, "plunge base", (CAR_AT - w, CAR_AT + w), (-ROUTER_BASE / 2, ROUTER_BASE / 2 - 25.0), (SLIDE_Z + SLIDE_T, SLIDE_Z + SLIDE_T + 12.0))
    vcyl(p, "motor", CAR_AT, 0.0, SLIDE_Z + SLIDE_T + 40.0, SLIDE_Z + SLIDE_T + 40.0 + ROUTER_H, ROUTER_D, op="add")
    for sx in (-34.0, 34.0):
        vcyl(p, "plunge post", CAR_AT + sx, -30.0, SLIDE_Z + SLIDE_T + 12.0, SLIDE_Z + SLIDE_T + 40.0, 12.0, op="add")


def bit(p, tip):
    """The bit down on the pattern: a 6 mm shank and the 3 mm nose."""
    vcyl(p, "shank", CAR_AT, 0.0, tip + 14.0, SLIDE_Z + SLIDE_T + 12.0, 6.0)
    vcyl(p, "ball-nose, 3 mm", CAR_AT, 0.0, tip, tip + 14.0, BIT, op="add")


def screw(p):
    """The cross slide's screw, from a bracket at the shelf's front edge."""
    box(p, "bracket", (CAR_AT - 20.0, CAR_AT + 20.0), (SHELF_Y[0] - PLY, SHELF_Y[0]), (SLIDE_Z - PLY, SLIDE_Z + 34.0))
    ycyl(p, "M8 screw", SHELF_Y[0] - PLY - 45.0, -SLIDE_L / 2 + 30.0, CAR_AT, SLIDE_Z + SLIDE_T + 14.0, SCREW_D, op="add")
    ycyl(p, "handwheel, 10 marks", SHELF_Y[0] - PLY - 37.0, SHELF_Y[0] - PLY - 25.0, CAR_AT, SLIDE_Z + SLIDE_T + 14.0, 60.0, op="add")
    box(p, "nut block on the slide", (CAR_AT - 15.0, CAR_AT + 15.0), (-SLIDE_L / 2, -SLIDE_L / 2 + 30.0), (SLIDE_Z + SLIDE_T, SLIDE_Z + SLIDE_T + 28.0), op="add")


def headstock(p):
    box(p, "headstock, two plies", HEAD_X, (-UPRIGHT_Y, UPRIGHT_Y), (0.0, UPRIGHT_TOP))
    xbore(p, HEAD_X[0], HEAD_X[1], 0.0, AXIS_Z, BEARING_D, "bored for the bearings")


def bearings(p):
    for i, x0 in enumerate((HEAD_X[0], HEAD_X[1] - BEARING_W)):
        xcyl(p, "6201", x0, x0 + BEARING_W, 0.0, AXIS_Z, BEARING_D - 0.1, op="new" if i == 0 else "add")
    xbore(p, HEAD_X[0], HEAD_X[1], 0.0, AXIS_Z, SPINDLE_D + 0.2, "bore")


def crank(p):
    x1 = HEAD_X[0] - 18.0
    xcyl(p, "crank", x1 - CRANK_T, x1, 0.0, AXIS_Z, CRANK_D)
    xcyl(p, "handle", x1 - CRANK_T - 30.0, x1 - CRANK_T, 0.0, AXIS_Z + CRANK_D / 2 - 8.0, 10.0, op="add")
    xbore(p, x1 - CRANK_T, x1, 0.0, AXIS_Z, SPINDLE_D + 0.2, "spindle")


def tailstock(p):
    box(p, "tail upright", TAIL_X, (-UPRIGHT_Y, UPRIGHT_Y), (0.0, UPRIGHT_TOP))
    xbore(p, TAIL_X[0], TAIL_X[1], 0.0, AXIS_Z, 10.5, "centre")


def spindle(p):
    xcyl(p, "M12 studding", HEAD_X[0] - 30.0, -PATTERN_T + 8.0, 0.0, AXIS_Z, SPINDLE_D)
    xcyl(p, "tail centre, M10 pointed", STICK, TAIL_X[1] + 25.0, 0.0, AXIS_Z, 10.0, op="new")


def star_section(k=1.0, cutter=False):
    """The star's section about the axis at scale k, a point straight
    down: its points blunted, or, as the pocket's cutter, its points
    sharp and its inside corners cut back instead."""
    at = lambda r, deg: (k * r * math.cos(math.radians(deg)), k * r * math.sin(math.radians(deg)))
    flank = math.dist(at(HEX_R, 0.0), at(STAR_IN, 15.0)) / k
    out = []
    for i in range(24):
        here, a = (HEX_R, STAR_IN)[i % 2], -90.0 + 15.0 * i
        q = at(here, a)
        if (i % 2 == 1) != cutter:
            out.append(q)
            continue
        other = (STAR_IN, HEX_R)[i % 2]
        back = (EASE if cutter else TIP_FLAT) / flank
        for side in (-15.0, 15.0):
            n = at(other, a + side)
            out.append((q[0] + (n[0] - q[0]) * back, q[1] + (n[1] - q[1]) * back))
    return out


def star_loft(p, cutter, op, name):
    ends = []
    for x, k in ((HEX_X[0], 1.0), (HEX_X[1], STAR_K)):
        s = p.sketch("right", x, name)
        p.polygon(s, [(y, AXIS_Z + z) for y, z in star_section(k, cutter)])
        ends.append(s)
    return p.feature({"type": "add_loft", "sketch": ends[0], "sketch_b": ends[1], "op": op, "name": name})


def star(p):
    star_loft(p, False, "new", "index star, tapered")
    xbore(p, HEX_X[0], HEX_X[1], 0.0, AXIS_Z, SPINDLE_D + 0.2, "spindle")


def cradle_top():
    return AXIS_Z - STAR_IN * math.sin(math.radians(45.0))     # up to the inside corners beyond the lower three points, at the large end


def cradle(p):
    """Drawn home on the star. A block with the star's lower three points cut in it, tapered as the star is."""
    box(p, "cradle", HEX_X, (-CRADLE_W / 2, CRADLE_W / 2), (GUIDE_FLOOR, cradle_top()))
    star_loft(p, True, "remove", "pocket: the star, its crests cut back")


def guide(p):
    """Screwed to the base: a floor, a lip each side of the cradle, and an end wall for the push screw."""
    half = CRADLE_W / 2 + GUIDE_PLAY
    x = (HEX_X[0] - 2.0, HEX_X[1] + TRAVEL + GUIDE_END[0])
    box(p, "floor", x, (-half - GUIDE_WALL - GUIDE_EAR, half + GUIDE_WALL + GUIDE_EAR), (0.0, GUIDE_FLOOR))
    for y in ((-half - GUIDE_WALL, -half), (half, half + GUIDE_WALL)):
        box(p, "lip", x, y, (GUIDE_FLOOR, GUIDE_FLOOR + GUIDE_LIP), op="add")
    box(p, "end wall", (x[1] - GUIDE_END[0], x[1]), (-half - GUIDE_WALL, half + GUIDE_WALL), (GUIDE_FLOOR, GUIDE_FLOOR + GUIDE_END[1]), op="add")
    xbore(p, x[1] - GUIDE_END[0], x[1], 0.0, GUIDE_FLOOR + GUIDE_END[1] / 2 + 2.0, PUSH_D + 0.5, "push screw")
    s = p.sketch("top", GUIDE_FLOOR, "screws")
    for sx in (x[0] + 8.0, x[1] - 8.0):
        for sy in (-half - GUIDE_WALL - GUIDE_EAR / 2, half + GUIDE_WALL + GUIDE_EAR / 2):
            p.point(s, (sx, sy))
    p.hole(s, 4.5, name="four screws to the base")


def push(p):
    """The push screw, home: its end on the cradle's face."""
    z = GUIDE_FLOOR + GUIDE_END[1] / 2 + 2.0
    x1 = HEX_X[1] + TRAVEL + GUIDE_END[0]
    xcyl(p, "M6 push screw", HEX_X[1], x1 + 22.0, 0.0, z, PUSH_D)
    xcyl(p, "knob", x1 + 22.0, x1 + 34.0, 0.0, z, 25.0, op="add")


def inside(q, poly):
    hit = False
    for i in range(len(poly)):
        a, b = poly[i - 1], poly[i]
        if (a[1] > q[1]) != (b[1] > q[1]) and q[0] < a[0] + (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]):
            hit = not hit
    return hit


def near(q, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0.0, min(1.0, ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(q[0] - a[0] - t * dx, q[1] - a[1] - t * dy)


def room(q, outline):
    """The largest circle about q inside the outline; nothing if q is outside."""
    return min(near(q, outline[i - 1], outline[i]) for i in range(len(outline))) if inside(q, outline) else -1.0


def widest(outline):
    """Where the tile's largest inscribed circle is, and its radius: the place for the axis that gives the fattest journal."""
    xs, ys = [q[0] for q in outline], [q[1] for q in outline]
    best = max(((room((x, y), outline), x, y) for x in (min(xs) + 0.5 * i for i in range(int((max(xs) - min(xs)) * 2) + 1))
                for y in (min(ys) + 0.5 * j for j in range(int((max(ys) - min(ys)) * 2) + 1))))
    step = 0.25
    while step > 1e-4:
        cand = max((room((best[1] + dx * step, best[2] + dy * step), outline), best[1] + dx * step, best[2] + dy * step) for dx in (-1, 0, 1) for dy in (-1, 0, 1))
        if cand[0] > best[0] + 1e-12:
            best = cand
        else:
            step /= 2
    return (round(best[1], 2), round(best[2], 2))


def over_post(passes, half, top):
    """The least any pass's ball comes over a post half wide with its top at top (off the axis)."""
    r, least = BIT / 2, 99.0
    for q in passes:
        lo, hi = max(q["across"] - r, -half), min(q["across"] + r, half)
        if lo < hi:
            for j in range(9):
                y = lo + (hi - lo) * j / 8
                least = min(least, q["tip"] + r - math.sqrt(max(0.0, r * r - (y - q["across"]) ** 2)) - top)
    return least


def post_seat(passes, radius):
    """A V under the round journal, a right angle, its lips as high as the passes let them be."""
    for top in (-2.5, -4.0, -5.5):
        mouth = top + radius * math.sqrt(2.0)
        pk = [(mouth, top), (0.0, -radius * math.sqrt(2.0)), (-mouth, top)]
        half = mouth + POST_WALL
        if over_post(passes, half, top) >= POST_CLEAR:
            return pk, half, top
    raise SystemExit("no post fits under the journal")


def post(p, at, pk, half, top):
    s = p.sketch("right", at - POST_T / 2, "post")
    p.polygon(s, [(-half, 0.0), (half, 0.0), (half, AXIS_Z + top)] + [(y, AXIS_Z + z) for y, z in pk] + [(-half, AXIS_Z + top)])
    p.extrude(s, POST_T, name="post")
    fx, ft, fy = POST_FOOT
    box(p, "foot", (at - fx / 2, at + fx / 2), (-half - fy, half + fy), (0.0, ft), op="add")
    s = p.sketch("top", ft, "screws")
    for sy in (-half - fy / 2, half + fy / 2):
        p.point(s, (at, sy))
    p.hole(s, 4.5, name="two screws to the base")


def pattern(p, outline, radius):
    s = p.sketch("right", -PATTERN_T, "tile section")
    p.polygon(s, [(y, AXIS_Z + z) for y, z in outline])
    p.extrude(s, PATTERN_T, name="pattern")
    xcyl(p, "journal master", -PATTERN_T - MASTER_T, -PATTERN_T, 0.0, AXIS_Z, 2 * radius, op="add")
    xbore(p, -PATTERN_T - MASTER_T, 0.0, 0.0, AXIS_Z, SPINDLE_D + 0.2, "spindle")


def stick_spans():
    """The stick along its length: tile, journal, tile, journal, tile."""
    cuts = [0.0] + [x for at in JOURNAL_AT for x in (at - JOURNAL_L / 2, at + JOURNAL_L / 2)] + [STICK]
    return [(cuts[i], cuts[i + 1], i % 2 == 1) for i in range(len(cuts) - 1)]


def stick(p, outline, radius):
    for i, (x0, x1, is_journal) in enumerate(stick_spans()):
        if is_journal:
            xcyl(p, "journal", x0, x1, 0.0, AXIS_Z, 2 * radius, op="add")
            continue
        s = p.sketch("right", x0, "tile section")
        p.polygon(s, [(y, AXIS_Z + z) for y, z in outline])
        p.extrude(s, x1 - x0, op="new" if i == 0 else "add", name="stick, milled")


# -- the pass plan -----------------------------------------------------------

def plan(outline):
    """Every pass: a stop, a place across, and the height the bit finds
    when it comes down on the pattern there. Each point of the outline
    is given to the stop that faces it most squarely among those whose
    bit, dropped where it would touch that point, does touch it and is
    not held up by another part of the tile; the passes of a stop are
    the STEP-wide places across that its points fall in."""
    r = BIT / 2
    n = len(outline)
    a2 = sum(outline[i][0] * outline[(i + 1) % n][1] - outline[(i + 1) % n][0] * outline[i][1] for i in range(n))
    sgn = 1.0 if a2 > 0 else -1.0
    normals = []
    for i in range(n):
        p0, p2 = outline[i - 1], outline[(i + 1) % n]
        tx, ty = p2[0] - p0[0], p2[1] - p0[1]
        ln = math.hypot(tx, ty)
        normals.append((sgn * ty / ln, -sgn * tx / ln))
    reach = max(math.hypot(x, y) for x, y in outline) + r
    frames = [((math.cos(t), math.sin(t)), (-math.sin(t), math.cos(t))) for t in (2 * math.pi * k / STOPS for k in range(STOPS))]   # the bit comes along -d; e is across
    across = [[(q[0] * e[0] + q[1] * e[1], q[0] * d[0] + q[1] * d[1]) for q in outline] for d, e in frames]

    def drop(k, y):
        """The ball's centre when the bit comes down at y from stop k."""
        return max((h + math.sqrt(r * r - (u - y) ** 2) for u, h in across[k] if abs(u - y) < r), default=None)

    given = [[] for _ in range(STOPS)]
    orphans = 0
    for i, q in enumerate(outline):
        nx, ny = normals[i]
        c = (q[0] + r * nx, q[1] + r * ny)
        for k in sorted(range(STOPS), key=lambda k: -(nx * frames[k][0][0] + ny * frames[k][0][1])):
            d, e = frames[k]
            if nx * d[0] + ny * d[1] < 0.05:
                orphans += 1
                break
            y, hc = c[0] * e[0] + c[1] * e[1], c[0] * d[0] + c[1] * d[1]
            if drop(k, y) <= hc + 0.05:        # the outline is facets: a neighbour stands a hair proud
                given[k].append((y, i))
                break
    # A stop's passes: across its points in order, a new pass wherever
    # the passes so far would leave a point standing more than the ridge.
    passes, left = [], [9.0] * n                     # how far each point of the outline is from the nearest pass's ball
    for k in range(STOPS):
        mine = []
        for y, i in sorted(given[k]):
            u, h = across[k][i]
            if all(math.hypot(u - py, h - pc) - r > RIDGE for py, pc in mine[-6:]):
                y = round(y / MARK) * MARK
                best = drop(k, y)
                if best is not None:
                    mine.append((y, best))
        for y, best in mine:
            passes.append({"stop": k, "across": round(y, 3), "tip": round(best - r, 3)})
            for i, (u, h) in enumerate(across[k]):
                if abs(u - y) <= r + 0.5:
                    left[i] = min(left[i], math.hypot(u - y, h - best) - r)
    # What is still standing gets the pass, from any stop, that comes
    # nearest it: the walls a bit only reaches with its side.
    extra = 0
    for i in range(n):
        if left[i] <= RIDGE + 0.03:
            continue
        found = None
        for k in range(STOPS):
            u, h = across[k][i]
            for j in range(int(-r / MARK), int(r / MARK) + 1):
                y = round((u + j * MARK) / MARK) * MARK
                best = drop(k, y)
                if best is not None:
                    gapleft = math.hypot(u - y, h - best) - r
                    if found is None or gapleft < found[0]:
                        found = (gapleft, k, y, best)
        if found and found[0] < left[i] - 0.02:
            _, k, y, best = found
            passes.append({"stop": k, "across": round(y, 3), "tip": round(best - r, 3)})
            extra += 1
            for m, (u, h) in enumerate(across[k]):
                if abs(u - y) <= r + 0.5:
                    left[m] = min(left[m], math.hypot(u - y, h - best) - r)
    passes.sort(key=lambda q: (q["stop"], q["across"]))
    # cut: within a ridge's height of some pass
    return passes, sum(1 for v in left if v < 0.08) / n, reach, max(left), orphans


def hull(points):
    pts = sorted(set(points))
    def half(seq):
        out = []
        for q in seq:
            while len(out) >= 2 and (out[-1][0] - out[-2][0]) * (q[1] - out[-2][1]) - (out[-1][1] - out[-2][1]) * (q[0] - out[-2][0]) <= 0:
                out.pop()
            out.append(q)
        return out
    lower, upper = half(pts), half(pts[::-1])
    return lower[:-1] + upper[:-1]


def rough(outline, passes):
    """The bites before the finish. The waste (the blank less the tile)
    is followed on a grid. First the straight bit, stop by stop, at
    places ROUGH_STEP apart: at each it may go down until it would
    touch the pattern, which is how its depth is found (the bit on the
    pattern, the depth stop set there over a feeler ALLOW thick), and
    it gets there in bites of ROUGH_BITE. Then every finishing pass,
    in order: what still stands over the ball's tip is its depth, and
    it takes that in bites of BALL_BITE to the same touch."""
    h_ = hull(outline)
    lo_x, hi_x = min(q[0] for q in h_) - BLANK, max(q[0] for q in h_) + BLANK
    lo_y, hi_y = min(q[1] for q in h_) - BLANK, max(q[1] for q in h_) + BLANK
    cells = []
    for i in range(int((hi_x - lo_x) / CELL) + 1):
        for j in range(int((hi_y - lo_y) / CELL) + 1):
            q = (lo_x + (i + 0.5) * CELL, lo_y + (j + 0.5) * CELL)
            if not inside(q, outline) and (inside(q, h_) or min(near(q, h_[m - 1], h_[m]) for m in range(len(h_))) <= BLANK):
                cells.append(q)
    waste = len(cells) * CELL * CELL
    alive = bytearray([1]) * len(cells)
    frames = [((math.cos(t), math.sin(t)), (-math.sin(t), math.cos(t))) for t in (2 * math.pi * k / STOPS for k in range(STOPS))]
    # per stop: the cells in bins a millimetre wide across, and the outline finely stepped
    fine = []
    for i in range(len(outline)):
        a, b = outline[i - 1], outline[i]
        m = max(1, int(math.hypot(b[0] - a[0], b[1] - a[1]) / 0.05))
        fine.extend((a[0] + (b[0] - a[0]) * t / m, a[1] + (b[1] - a[1]) * t / m) for t in range(m))
    bins, lines = [], []
    for d, e in frames:
        by = {}
        for i, q in enumerate(cells):
            u = q[0] * e[0] + q[1] * e[1]
            by.setdefault(math.floor(u), []).append((u, q[0] * d[0] + q[1] * d[1], i))
        bins.append(by)
        lines.append([(q[0] * e[0] + q[1] * e[1], q[0] * d[0] + q[1] * d[1]) for q in fine])

    def cut(k, y, tip, r, ball, dry=False):
        """What a bit at y, its tip at tip, takes at stop k: the area, and how far the waste stood over its tip."""
        took, top = [], tip
        for b in range(math.floor(y - r), math.floor(y + r) + 1):
            for u, h, i in bins[k].get(b, ()):
                if alive[i] and abs(u - y) <= r and h >= (tip + r - math.sqrt(r * r - (u - y) ** 2) if ball else tip):
                    took.append(i)
                    top = max(top, h)
        if not dry:
            for i in took:
                alive[i] = 0
        return len(took) * CELL * CELL, top - tip

    r6 = ROUGH_BIT / 2
    roughs, rough_area = [], 0.0
    for k in range(STOPS):
        span = max(abs(u) for u, _ in lines[k])
        n = int(span / ROUGH_STEP) + 1
        for j in range(-n, n + 1):
            y = j * ROUGH_STEP
            under = [h for u, h in lines[k] if abs(u - y) <= r6]
            if not under:
                continue                              # beside the tile at this stop: another stop faces it
            touch = max(under)                        # where the bit meets the pattern
            area, depth = cut(k, y, touch + ALLOW, r6, False, dry=True)
            if area < 1.0:
                continue
            cut(k, y, touch + ALLOW, r6, False)
            rough_area += area
            roughs.append({"stop": k, "across": y, "touch": round(touch, 3), "tip": round(touch + ALLOW, 3), "depth": round(depth, 3), "bites": max(1, math.ceil(depth / ROUGH_BITE - 1e-9)), "area": round(area, 2)})
    left_after_rough = sum(alive) * CELL * CELL
    rb = BIT / 2
    for q in passes:
        area, depth = cut(q["stop"], q["across"], q["tip"], rb, True)
        q["depth"] = round(depth, 3)
        q["bites"] = max(1, math.ceil(depth / BALL_BITE - 1e-9))
    return roughs, {"waste": waste, "rough_area": rough_area, "left_after_rough": left_after_rough, "left_at_end": sum(alive) * CELL * CELL, "hull": h_}


def placed(x, y, z):
    return {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "mill.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    tile = json.loads(mcp.call("spectre_tile", {"edge": EDGE, "curve": "ellipse", "bulge": BULGE, "bit": BIT, "gap": GAP}))
    raw = [tuple(q) for q in tile["outline"]]
    centre = widest(raw)                             # the axis goes where the tile is fattest, for the journals
    outline = [(x - centre[0], y - centre[1]) for x, y in raw]
    circle = room((0.0, 0.0), outline)
    radius = math.floor((circle - JOURNAL_IN) * 20) / 20     # the journal's, to 0.05
    passes, covered, reach, worst, orphans = plan(outline)
    per_stop = [sum(1 for q in passes if q["stop"] == k) for k in range(STOPS)]
    tips = [q["tip"] for q in passes]
    top = max(q[1] for q in outline)                 # the pattern's top at the stop drawn
    print(f"tile {tile['across']:.1f} across, swings {reach - BIT / 2:.1f}; {len(passes)} passes, {min(per_stop)} to {max(per_stop)} a stop; the bit's tip from {min(tips):.1f} to {max(tips):.1f} off the axis; {100 * covered:.1f} % of the outline within 0.08 of a pass, the worst {worst:.2f}; {orphans} points no stop reaches")
    roughs, waste = rough(outline, passes)
    rough_runs, finish_runs = sum(q["bites"] for q in roughs), sum(q["bites"] for q in passes)
    r6 = ROUGH_BIT / 2
    print(f"waste {waste['waste']:.0f} mm2 of section: {len(roughs)} roughing places take {waste['rough_area']:.0f} in {rough_runs} runs, the deepest {max(q['depth'] for q in roughs):.1f}; "
          f"the ball-nose takes the {waste['left_after_rough']:.0f} left in {finish_runs} runs, {sum(1 for q in passes if q['bites'] > 1)} of its {len(passes)} passes in more than one bite, the deepest {max(q['depth'] for q in passes):.1f}; {waste['left_at_end']:.1f} stands at the end")
    pk, post_half, post_top = post_seat(passes, radius)
    rough_over_post = min((q["tip"] - post_top for q in roughs if abs(q["across"]) < post_half + r6), default=99.0)
    if rough_over_post < POST_CLEAR:
        raise SystemExit(f"a roughing bite comes {rough_over_post:.2f} over a post")
    blank = max(math.hypot(x, y) for x, y in outline) + BLANK
    journal_bites = math.ceil((blank - radius) / ROUGH_BITE)
    journal_places = math.ceil(JOURNAL_L / (JOURNAL_BIT - 1.0))
    print(f"axis moved {centre} in the tile: a circle of {circle:.2f} fits, the journal {2 * radius:.1f} round; posts {2 * post_half:.1f} wide with their lips {-post_top:.1f} under the axis, the bit never nearer than {over_post(passes, post_half, post_top):.2f}; "
          f"a journal is {journal_bites} bites at {journal_places} places, {journal_bites * journal_places} turns of the crank")
    mcp.call("create_document", {"name": "mill"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Indexed stick mill for the leaning half-ellipse Spectre"}, {"type": "rename_tab", "tab": 1, "name": "Base"}]})
    tip_drawn = AXIS_Z + max(z + math.sqrt(max(0.0, (BIT / 2) ** 2 - y * y)) for y, z in outline if abs(y) < BIT / 2) - BIT / 2
    parts = [("Base", base), ("Back wall", wall), ("Shaft supports SK20", supports), ("Shafts", shafts), ("Blocks SC20UU", blocks), ("Carriage square", square), ("Shelf", shelf),
             ("Cross slide", slide), ("Router", router), ("Bit", lambda p: bit(p, tip_drawn)), ("Cross screw", screw), ("Headstock", headstock), ("Bearings 6201", bearings), ("Crank", crank), ("Tailstock", tailstock),
             ("Spindle and centre", spindle), ("Star", star), ("Cradle", cradle), ("Cradle guide", guide), ("Push screw", push),
             ("Pattern", lambda p: pattern(p, outline, radius)), ("Stick", lambda p: stick(p, outline, radius)),
             ("Post 1", lambda p: post(p, JOURNAL_AT[0], pk, post_half, post_top)), ("Post 2", lambda p: post(p, JOURNAL_AT[1], pk, post_half, post_top))]
    tabs, bodies = {}, {}
    for i, (title, make) in enumerate(parts):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
    asm = new_tab(mcp, {"type": "add_assembly", "name": "Mill"})
    ops = [{"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True, "placement": placed(0.0, 0.0, 0.0)}}
           for t, _ in parts for k in range(len(bodies[t]))]
    text = mcp.call("apply", {"ops": ops})
    if "ERROR:" in text:
        raise SystemExit(text)
    mcp.call("screenshot", {"tab": asm, "view": "-0.55,-0.7,0.5", "width": 1700, "height": 1100, "path": os.path.join(OUT, "mill.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{-PATTERN_T / 2}", "fit": f"{-PATTERN_T / 2},{BASE_Y[0]},-20,{-PATTERN_T / 2 + 1},{BASE_Y[1]},310", "width": 1500, "height": 1100, "path": os.path.join(OUT, "end.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{JOURNAL_AT[0]}", "fit": f"{JOURNAL_AT[0]},-60,-20,{JOURNAL_AT[0] + 1},60,110", "width": 1300, "height": 1000, "path": os.path.join(OUT, "journal.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-0.2,-1,0.35", "fit": f"{HEAD_X[0] - 70},-90,-20,{-PATTERN_T + 5},90,130", "width": 1500, "height": 1000, "path": os.path.join(OUT, "head.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "mill.pdf"),
                              "note": f"indexed stick mill: two stacked 20 mm shafts held at both ends, a twelve-pointed star and one sliding cradle for {STOPS} stops, round journals turned by a crank, the bit set on a pattern of the tile at full size; {len(passes)} passes a stick"}))
    for title, name in (("Pattern", "pattern"), ("Star", "star"), ("Cradle", "cradle"), ("Cradle guide", "cradle_guide"), ("Post 1", "post")):
        print(mcp.call("export", {"tab": tabs[title], "format": "stl", "path": os.path.join(OUT, f"{name}.stl")}))
    mcp.close()
    numbers = {
        "edge": EDGE, "bulge": BULGE, "gap": GAP, "bit": BIT, "ridge": RIDGE, "mark": MARK, "stops": STOPS, "stick": STICK,
        "tile": {"across": tile["across"], "area": tile["area"], "outline": outline, "swing": reach - BIT / 2, "axis_in_tile": centre, "circle": circle},
        "journal": {"radius": radius, "at": JOURNAL_AT, "length": JOURNAL_L, "bit": JOURNAL_BIT, "bites": journal_bites, "places": journal_places, "turns": journal_bites * journal_places,
                    "blank_swing": blank, "master_t": MASTER_T, "spans": stick_spans(),
                    "post": {"seat": pk, "half": post_half, "top": post_top, "t": POST_T, "foot": POST_FOOT, "clear": over_post(passes, post_half, post_top), "least": POST_CLEAR}},
        "car_min": CAR_MIN,
        "axis_z": AXIS_Z, "shaft": {"z": RAIL_Z, "y": RAIL_Y, "x": SHAFT_X, "d": SHAFT_D}, "supports_x": SUPPORT_X, "sk_t": SK_T,
        "slide": {"z": SLIDE_Z, "t": SLIDE_T, "travel": SLOT}, "carriage": {"square": SQUARE, "at": CAR_AT, "block_x": BLOCK_X, "blk_l": BLK_L, "face_y": FACE_Y},
        "bit_x": BIT_X, "pattern_t": PATTERN_T, "reach": REACH, "plunge": PLUNGE, "tip_drawn": tip_drawn, "upright_top": UPRIGHT_TOP,
        "star": {"af": HEX_AF, "r": HEX_R, "inside": STAR_IN, "t": HEX_T, "x": HEX_X, "spindle": SPINDLE_D, "taper": TAPER, "k": STAR_K, "tip_flat": TIP_FLAT, "ease": EASE,
                 "section": star_section(), "cutter": star_section(cutter=True), "cradle_w": CRADLE_W, "cradle_top": cradle_top(), "travel": TRAVEL,
                 "guide": {"floor": GUIDE_FLOOR, "lip": GUIDE_LIP, "wall": GUIDE_WALL, "play": GUIDE_PLAY, "end": GUIDE_END}, "bearing": {"d": BEARING_D, "w": BEARING_W}},
        "rough": {"bit": ROUGH_BIT, "bite": ROUGH_BITE, "step": ROUGH_STEP, "allow": ALLOW, "ball_bite": BALL_BITE, "blank": BLANK, "cell": CELL, "places": len(roughs), "runs": rough_runs, "finish_runs": finish_runs,
                  "over_post": rough_over_post, **waste, "list": roughs},
        "plan": {"passes": len(passes), "per_stop": per_stop, "tip_low": min(tips), "tip_high": max(tips), "covered": covered, "worst": worst, "orphans": orphans,
                 "list": passes},
        "bodies": {k: [list(b) for b in v] for k, v in bodies.items()},
    }
    with open(os.path.join(OUT, "mill.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"base {BASE_X[1] - BASE_X[0]:.0f} x {BASE_Y[1] - BASE_Y[0]:.0f}, shafts {SHAFT_X[1] - SHAFT_X[0]:.0f} long at z {RAIL_Z[0]:.0f} and {RAIL_Z[1]:.0f}, axis at {AXIS_Z:.0f}, the shelf's top at {SLIDE_Z:.0f}, star {2 * HEX_R:.1f} over its points tapering {TAPER:.0f} deg, the cradle {TRAVEL:.0f} off it to turn")


if __name__ == "__main__":
    main()
