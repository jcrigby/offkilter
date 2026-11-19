#!/usr/bin/env python3
"""A stick sled: the leaning half-ellipse Spectre milled round one solid stick.

The stick is held between a socket on a spindle and the point of a
second spindle, on a flat sled that slides on the base between two
fences; it only turns. Beside it, fixed to the base, lies
the two-axis router positioner (examples/router-positioner), its bit
pointing at the stick: its X sets how deep the bit reaches, its Z how
high. For each pass those two motors and the spindle's go to the next
row of a table and stop, and a fourth motor feeds the sled past the bit
on a screw and brings it back. Nobody pushes anything.

The wheel that turns the stick is on the sled's own spindle and goes
where the sled goes, with its motor in the sled's box under it. The
belt holds the stick while it is cut: the bites are sized for that
(the estimate is further down), and a lock on the spindle is drawn in
its own tabs and left off the machine. Its place is kept clear.

It replaces the hinged sled (examples/stick-sled-hinged), which hung
the stick from two hinges over a cutter fixed in a router table: the
positioner's two straight screws do what the hinges and their links
did, and the stick's own mounting has nothing left to do but turn.

    cargo build --release -p ok-mcp
    python3 examples/router-positioner/build.py     # the positioner's sizes are read from its out/positioner.json
    python3 examples/stick-sled/build.py
    python3 examples/stick-sled/animate.py          # out/passes_machine.gif and out/passes.gif: every pass, a frame each
    cargo test -p ok-render --test stick_sled   # what CI runs

Writes out/sled.okpart (every part in its own tab, the sled at one
row, and the sled drawn back), sled.png, end.png, drive.png, tail.png, index.png, home.png, travel.png, sled.pdf, passes.csv (the rows the controller
reads), steps.json (the controller's own run of it, simulated) and
sled.json.
"""
import json
import math
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)
from okmcp import Mcp, Part, new_tab  # noqa: E402
import plan as P  # noqa: E402
import calibrate as K  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "firmware"))
import config as FW  # noqa: E402

OUT = os.path.join(HERE, "out")
POSITIONER = os.path.join(os.path.dirname(HERE), "router-positioner", "out", "positioner.json")

# The tile, as the stick mill's.
EDGE, BULGE, GAP = 10.56, 0.4, 0.7              # the lid's edge; 0.8 of a semicircle deep; the line between tiles
STICK = 200.0                                   # the stick's length

# The slide, as the owner asked for it: a flat sled on the base between two fences, each fence with a lip over the
# sled's edge so that it cannot lift. No shafts and no bearing blocks.
PLY = 19.05                                     # 3/4 in plywood, nominal, as the positioner's
SLIP = 3.0                                      # two low-friction strips on the base that the sled rides on
T12 = 12.7                                      # 1/2 in plywood, nominal, as the positioner's
BOX_IN = 12.0                                   # the sled's box stands this far in from its bottom's far edge: the flange the datum fence's lip is over
FENCE_L, FENCE_R = 30.0, 14.0                   # the fences, across. The far one is the datum; the near one stands between the sled and the positioner
LIP = (6.0, 10.0, 7.0, 0.3)                     # a fence's lip: thick, how far over the sled at the datum fence and at the near one, and clear of the sled's top
SPRING_GAP = 1.0                                # between the sled and the near fence: spring plungers in the sled's edge press it onto the datum
PLUNGER = (6.0, (0.0, 250.0))                   # those plungers: across, and where along the sled
PLATE_LEFT = -180.0                             # the sled's far edge, on the datum fence. Its width is what keeps it from tipping
MOTOR = (42.3, 40.0, 24.0, 5.0)                 # a NEMA 17: square, and long; its shaft, long and across
BRACKET = (4.0, 6.0, 4.0)                       # the angle bracket such a motor is sold with: its face thick, its foot thick, and how far it stands out round the motor
SPINDLE_D = 12.0                                # a ground rod, flats ground on it for set screws and nothing threaded: see DECISIONS, the pulley's runout
# The wheel, its motor's pulley, and a closed GT2 belt. The owner's idea: the wheel on the sled's own spindle, so
# that the motor turns the stick wherever the sled is and no hand is wanted at it. The pulley was 20 teeth while the
# motor stood 208 mm off on the base; in the sled's box it is 148 off, and a pulley of 20 would have under five teeth
# in the belt there. Thirty has seven, and the ratio is twelve to one.
WHEEL_T, PINION_T, BELT = 360, 30, 760.0
WHEEL_W = 12.0
# Bought parts, at the sizes they are usually sold in: check the ones in hand before drilling for them.
KP = (16.0, 71.0, 38.0, 19.0, 56.0, 6.0)        # a KP001 pillow block, 12 bore: along the shaft, long across it, high, the shaft's centre over its foot, its two bolts apart, its foot thick
# On the sled both spindles run in pillow blocks bolted down on a riser: a solid block under each pair, glued face
# down on the deck, in place of two thin uprights stood on their ends with a flange bearing on the face of each.
RISER = 2 * T12                                 # the riser: two layers of 1/2 in glued face to face
RISER_HALF = 38.0                               # and this wide each side of the axis: the block's foot and a margin
AXIS_OVER = RISER + KP[3]                       # the stick's axis over the sled's deck: the socket turns 5 mm clear of it
AXIS_UP = 8.0                                   # the stick's axis this far above the middle of the positioner's height. The wheel goes with the sled now, over the near fence's lip and the controller's box: level with the middle, its rim passed that lip by 1.8 mm. The height has the room: the rows use under half of it
DECK_CLEAR = 6.0                                # the deck's near edge from the positioner's face: it reaches out over the near fence, which is far below it. It was 10 until the nut's face was measured and the positioner came 5 mm nearer: the risers are 38 each side of the axis and have to be on the deck
HUB = (36.0, 5.0, 24.0, 14.0, 28.0)             # a 12 mm flanged hub: its flange across and thick, its boss across and long, its bolt circle
SENSOR = (13.0, 24.0, 11.0)                     # an optical endstop: how far it stands off what it is screwed to, long, wide
FIN = (1.5, 6.0, 6.0)                           # the home flag, a fin on the disc's inner face: thick, wide, and how far it goes into the sensor's slot
FLAG_R = AXIS_OVER + PLY / 2                    # it is this far from the axis: level with the middle of the deck's end, which the sensor is screwed to
# The blank's socket, printed: a pocket for the blank's end on one side, a boss the spindle's rod is epoxied into on
# the other. The owner's idea. It drives the blank by its flats and says which way round it is.
SOCKET_IN, SOCKET_SLIDE = 12.0, 8.0             # how far the blank stands in the pocket, and how much further it can be pushed to get it onto the tail centre
SOCKET_WALL, SOCKET_FIT, SOCKET_FLOOR = 4.0, 0.3, 6.0
SOCKET_BOSS = (26.0, 22.0)                      # the boss: across, and long, which is how far the rod's roughened end goes into it
NUT_CLEAR = 2.0                                 # a pass ends with the collet nut this short of the socket
# On the spindle's far end, past the headstock: a hub, a disc, and the wheel on the disc's neck. The disc carries the
# home flag and is what a hand turns the stick by. It is also what a lock would pinch, if one is ever wanted: a post
# beside the headstock, a toggle clamp through it, and an anvil that comes in behind the disc, between it and the
# wheel, which is what the neck is for. The lock is drawn, each part in its own tab, and left off the machine unless
# the controller's config says it is fitted.
DISC = (176.0, 8.0)                             # the disc: across, thick
NECK = (60.0, 9.0, 44.0)                        # its neck: across, long, and the circle the wheel's four screws are on
LOCK_R = 80.0                                   # where a clamp would pinch the disc
LOCK_Y = -(DISC[0] / 2 + 25.0)                  # the lock post's far edge: 25 mm of it outside the disc's rim, for the anvil's spacer
ANVIL = (6.0, 20.0, 0.3)                        # the anvil behind the disc: 6 mm steel, this far above and below the axis, and clear of the disc until the clamp closes
CLAMP = (36.0, 24.0, 90.0, 10.0, 16.0)          # a push-pull toggle clamp: its body across, high and long; its plunger, and the pad on it
# The sled's travel: a screw fixed along the base, its motor at the tail end, and a nut in the sled's box.
SCREW = (8.0, 8.0, 250.0)                       # a T8 lead screw of four starts: across, how far a turn takes its nut, long
NUT = (22.0, 3.5, 10.2, 15.0)                   # its flanged nut: the flange across and thick, the body across and long
COUPLER = (19.0, 25.0, 2.0)                     # a rigid coupler: across, long, and between the two shafts' ends inside it
TRAVEL_Y = -74.0                                # where the screw runs, across: under the middle of the sled's bottom, so that it pushes where the slips' drag is
NUT_BLOCK = (44.0, 14.0)                        # the block the nut is bolted to, on the sled's bottom: across, and how far it stands over the screw
GUARD = 1.0                                     # a stop block at each end of the travel, this far beyond where the sled goes: only a sled that has lost its place meets one

# What a cut asks of the belt, estimated. Nobody has measured any of it: the wood, the bit and the belt will each be
# what they are. The numbers are taken on the heavy side, and the bites are the deepest that keep the answer small.
CUT_ENERGY = 0.15                               # joules to cut a cubic millimetre of hardwood to chips this thin: published figures run from 0.03 to 0.15
ROUTER_RPM = 16000.0                            # the Colt on its slowest setting: the slower the bit, the harder each tooth pushes
PEAK = 2.0                                      # the most a two-flute bit pushes, over what it pushes on average
BELT_EA = 8000.0                                # newtons to stretch a 6 mm GT2 belt by its own length: within a factor of two
MOTOR_HOLD = 0.40                               # newton metres, a 40 mm NEMA 17 standing still
GIVE = (0.010, 0.005)                           # the most the tile's edge may be let move, mm: at any cut, and at one that leaves a finished face

# The sled. x along the stick from its tail end, where the bit waits, toward the socket and the drive; y across toward
# the positioner from the stick's axis; z up from the base.
MASTER_R = SPINDLE_D / 2                        # the master: the tail spindle's ground rod, bare between its bearing and the stick. With the sled back the bit is beside it
TAIL_PB_X = ((-86.0, -70.0), (-42.0, -26.0))    # the tail's two pillow blocks. The inner one ends short of where the collet nut is with the sled back: its foot is as far out as the nut comes in
TAIL_RISER_X = (TAIL_PB_X[0][0] - 9.0, TAIL_PB_X[1][1])       # the block they stand on
TAIL_ROD_X = (TAIL_PB_X[0][0] - 5.0, 0.0)       # the tail spindle: a rod in those two bearings, its end a three-flat point in the stick's end
MASTER_X = (TAIL_PB_X[1][1], 0.0)
SOCKET_X = (STICK - SOCKET_IN, STICK + SOCKET_SLIDE)          # the socket's pocket
BOSS_X = (SOCKET_X[1] + SOCKET_FLOOR, SOCKET_X[1] + SOCKET_FLOOR + SOCKET_BOSS[1])
_K = BOSS_X[1] + 4.0
HEAD_PB_X = ((_K, _K + KP[0]), (_K + 58.0, _K + 58.0 + KP[0]))            # the headstock's two pillow blocks, past the socket
HEAD_RISER_X = (_K - 2.0, HEAD_PB_X[1][1])      # the block they stand on
POST_X = (HEAD_RISER_X[1] - PLY, HEAD_RISER_X[1])             # the lock's post, if there is one: beside the riser's far end
BOX_X = (TAIL_RISER_X[0] - 2.0, HEAD_RISER_X[1] + 2.0)        # the sled's box: its rails and its deck
HUB_X = (BOX_X[1], BOX_X[1] + HUB[1] + HUB[3])  # the hub: its boss toward the headstock, where its set screws can be got at
DISC_X = (HUB_X[1], HUB_X[1] + DISC[1])         # the disc, on the hub's flange
NECK_X = (DISC_X[1], DISC_X[1] + NECK[1])       # its neck
WHEEL_X = (NECK_X[1], NECK_X[1] + WHEEL_W)      # and the wheel on the neck's end
ROD_X = (BOSS_X[0], WHEEL_X[1] - 2.0)           # the spindle, from the bottom of the socket's boss to just short of the wheel's outer face
PLATE_X = (BOX_X[0], WHEEL_X[1] + 4.0)          # the sled's bottom: longer than its box, it goes on under the wheel and carries the motor's bracket
FIN_L = HUB_X[1] - HUB_X[0] - SENSOR[0] + FIN[2]              # the fin's length: from the disc back into the sensor's slot
DRIVE_X = (WHEEL_X[0] - 10.0, WHEEL_X[0] - 10.0 + BRACKET[0])   # the face of the wheel's motor's bracket: the motor behind it in the box, its shaft through it to the pulley
NUT_X = (BOX_X[0] + 6.0, BOX_X[0] + 6.0 + PLY)  # the travel nut's block, just inside the box's tail end
BACK_X = -12.0                                  # the bit along the sled with the sled back: beside the master, 9 mm of the straight bit clear of the stick's end
DRAWN_X = 100.0                                 # where the bit is drawn: mid stick, mid pass
FACE_CLEAR = 2.5                                # the uncut blank turns this clear of the positioner's face
BED_CLEAR = 2.5                                 # and of the sled's plate
PLINTH = 0.0                                    # the positioner lies on the base itself
SAFE_CLEAR = 2.0                                # drawn back to the safe depth, the bit's tip is this clear of the master whatever its height
MOVE_CLEAR = 1.5                                # a move may go straight to its row if the bit passes the master by this much all the way
BIT_MARGIN = 2.0                                # a bit may stand this much further out of the collet, or less far, than the table takes it to
SHANK_GRIP = 19.0                               # a 1/4 in shank is in the collet at least this far: 3/4 in
SHANK_OFF = 1.5                                 # and stops this short of the bottom, so the nut can draw it down
NUT_AIR = 1.0                                   # the collet nut passes the uncut blank by this much at every cut, the bit set as short as the margin lets it be
BITS = (("straight", "Freud 04-110", P.ROUGH_LONG), ("ball", "Freud 72-405", P.BIT_LONG))   # the two bits: what the table calls each, what it is, how long end to end
STOP = (40.0, 40.0)                             # a stop block on the base beyond each end of the sled's travel: along, across
SWITCH = (28.0, 16.0, 10.0)                     # the travel's home switch, beside the back stop
ROUGH_R = P.ROUGH_BIT / 2
WHEEL_D = WHEEL_T * 2.0 / math.pi               # GT2: 2 mm a tooth
WHEEL_RIM = WHEEL_D / 2 + 1.4                   # its flanges
PINION_D = PINION_T * 2.0 / math.pi
RATIO = WHEEL_T / PINION_T


def belt_centres(belt, big, small):
    """How far apart two pulleys stand in a closed belt of this length."""
    lo, hi = (big - small) / 2 + 1.0, belt
    for _ in range(60):
        c = (lo + hi) / 2
        if 2 * c + math.pi * (big + small) / 2 + (big - small) ** 2 / (4 * c) < belt:
            lo = c
        else:
            hi = c
    return (lo + hi) / 2


def slab(p, name, x, pts, op="new"):
    s = p.sketch("right", x[0], name)
    p.polygon(s, pts)
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def box(p, name, x, y, z, op="new"):
    return slab(p, name, x, [(y[0], z[0]), (y[1], z[0]), (y[1], z[1]), (y[0], z[1])], op)


def xcyl(p, name, x, c, d, op="new"):
    s = p.sketch("right", x[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def zcyl(p, name, z, c, d, op="new"):
    """A cylinder standing from z[0] to z[1], about (x, y)."""
    s = p.sketch("top", z[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def ycyl(p, name, y, c, d, op="new"):
    """A cylinder along y, from y[0] to y[1], about (x, z)."""
    s = p.sketch("front", -y[1], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, y[1] - y[0], op=op, name=name)


def tf(fr, pts):
    """Points of a frame (its origin, its angle) in the sled's."""
    (oy, oz), a = fr
    c, s = math.cos(a), math.sin(a)
    return [(oy + y * c - z * s, oz + y * s + z * c) for y, z in pts]


def grown(c, d):
    """A rectangle given by its four corners in order, grown by d on every side."""
    def unit(p, q):
        n = math.hypot(q[0] - p[0], q[1] - p[1])
        return ((q[0] - p[0]) / n, (q[1] - p[1]) / n)
    a, b = unit(c[0], c[1]), unit(c[0], c[3])
    return [(c[k][0] + d * (sa * a[0] + sb * b[0]), c[k][1] + d * (sa * a[1] + sb * b[1])) for k, (sa, sb) in enumerate(((-1, -1), (1, -1), (1, 1), (-1, 1)))]



def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "sled.okpart")
    if os.path.exists(path):
        os.remove(path)
    pos = json.load(open(POSITIONER))
    mcp = Mcp(path)
    tile = json.loads(mcp.call("spectre_tile", {"edge": EDGE, "curve": "ellipse", "bulge": BULGE, "bit": P.BIT, "gap": GAP}))
    raw = [tuple(q) for q in tile["outline"]]
    centre = P.widest(raw)                           # the axis where the tile is fattest: room there for a spur and a master that every pass stays clear of
    outline = [(x - centre[0], y - centre[1]) for x, y in raw]
    circle = P.room((0.0, 0.0), outline)
    if circle < MASTER_R + 1.0:
        raise SystemExit(f"the tail spindle, {MASTER_R} radius, does not fit in the tile's {circle:.1f} about the axis")
    swing_r = max(math.hypot(x, y) for x, y in outline)

    # -- the cuts ------------------------------------------------------------
    passes, left, orphans = P.finish(outline)
    cands = P.rough_places(outline)
    covered = sum(1 for v in left if v < 0.08) / len(left)
    print(f"tile {tile['across']:.1f} across, swings {swing_r:.1f}; {len(passes)} finishing passes from as many directions, the bit within {max(abs(q['across']) for q in passes):.1f} of the axis's height; "
          f"{100 * covered:.1f} % of the outline within 0.08 of a pass, the worst {max(left):.2f}; {orphans} points nothing reaches")
    blank_of = P.stock(outline)
    blank = max(math.hypot(*q) for q in blank_of["corners"])     # the furthest corner of the blank from the axis
    print(f"blank {blank_of['size'][0]:.0f} x {blank_of['size'][1]:.0f}, its corners up to {blank:.1f} from the axis; the axis {blank_of['axis'][0]:.1f} and {blank_of['axis'][1]:.1f} in from two of its faces")

    # -- the positioner, lying beside the stick ---------------------------------
    # It lies on its lid: its Z column stands up out of the way, and its bit's axis comes low, a stick's height over the
    # base. Its bit-side face is a wall beside the stick, so the blank must turn clear of it, and the bit has to reach
    # from behind that wall to past the stick's axis.
    x_travel, z_travel = pos["travel"]["x"], pos["travel"]["z"]
    box_x, wall = pos["box"]["x"], pos["outer_wall"]
    lid = pos["box"]["inside_height"] + wall                     # the lid's outer face, up from the positioner's own floor
    # The collet nut is in front of the quill by the router's nose and spindle. With the quill at home it is behind the
    # positioner's face by `nut_behind`; run out for the deepest cut it is in front of it, and the blank has to turn
    # clear of it there. That, more than the face itself, is what sets how far the positioner stands from the stick.
    nut_behind = pos["quill"]["front"] - box_x[0] - pos["router"]["nut_face"]
    near = math.ceil(blank + FACE_CLEAR)                         # as near as the face could come: the blank turning clear of it
    face = max(near, math.ceil(blank + x_travel - BIT_MARGIN - nut_behind))   # the positioner's face, from the stick's axis
    bit_home_z = PLINTH + lid - pos["axis_z"]                    # the bit's axis over the base, the stage at home
    za = bit_home_z + z_travel / 2 + AXIS_UP                     # the stick's axis: a little above the middle of the height's travel
    # The sled is a box: a bottom that slides between the fences, two rails on edge, and a deck that the risers
    # stand on. The positioner's bit is high over the base, and a box brings the deck up to it where a single
    # plate would want the spindles on tall posts.
    plate_top = SLIP + PLY
    sled_top = za - AXIS_OVER
    if sled_top - PLY - plate_top < 20.0:
        raise SystemExit(f"the stick's axis, {za:.0f} over the base, is too low for a box under the deck")
    if za - sled_top - blank < BED_CLEAR:
        raise SystemExit(f"the blank turns into the sled's plate: {za - sled_top - blank:.1f}")
    deep = cands + passes
    low_tip = min(q["tip"] for q in deep)
    tip_home = x_travel - BIT_MARGIN + low_tip                   # with the quill at home: so the deepest cut leaves the margin of the travel
    drawn_axes = P.Axes(tip_home, -(z_travel / 2 + AXIS_UP))
    # What calibrate.py found a built machine to be, if it has been run: where the bit's height is level with the stick's
    # axis, the height screw's true travel, and the master's runout. The table is then planned for that.
    built = json.load(open(K.BUILT)) if os.path.exists(K.BUILT) else None
    axes = P.Axes(tip_home, built["across_home"], (1.0, built["scale_z"])) if built else drawn_axes
    master_r, ecc = (built["master_r"], built["ecc"]) if built else (MASTER_R, [0.0, 0.0])
    if built:
        print(f"PLANNED FOR THE MACHINE AS BUILT ({K.BUILT}): the bit level with the stick's axis at height {-axes.across_home / axes.scale[1]:.3f}, the height's screw {axes.scale[1]:.4f} to one, the master {math.hypot(*ecc):.3f} off the spindle's centre")
    nut_face_home = face + nut_behind                            # the collet nut's face from the axis, the quill at home
    bit_out = nut_face_home - tip_home                           # how far the bit stands out of the collet nut
    safe = tip_home - (MASTER_R + SAFE_CLEAR)                    # the depth at or short of which the bit clears the master at any height
    # How long a bit can be, end to end, and be set within the margin of that: its shank in the collet by enough, and
    # not on the bottom. The owner bottomed a bit, tightened the nut and marked it at the nut's face: 32 mm.
    shank_in = pos["router"]["shank_in"]
    bit_long = (bit_out - BIT_MARGIN + SHANK_GRIP, bit_out + BIT_MARGIN + shank_in - SHANK_OFF)
    print(f"the positioner's face {face:.0f} from the stick's axis, the blank's corners {face - blank:.1f} clear of it and {nut_face_home - (x_travel - BIT_MARGIN) - blank:.1f} of the collet nut at the deepest cut; the axis {za:.1f} over the base, {za - sled_top - blank:.1f} clear of the sled's plate; "
          f"the bit {bit_out:.1f} out of the collet nut, its tip {tip_home:.1f} from the axis with the quill at home; a bit {bit_long[0]:.1f} to {bit_long[1]:.1f} long can be set to that, {SHANK_GRIP:.0f} or more in a collet {shank_in:.0f} deep")

    # -- the order, and then the bites -----------------------------------------
    # Roughing, and then finishing, go round the stick in the order of the spindle's own angle, so it turns a little at a
    # time and mostly forwards. The order has to be settled before the bites are counted: what a cut meets is what the
    # cuts before it left.
    turn_of = lambda q: axes.row(q["t"], q["across"], q["tip"])[2]
    cands.sort(key=lambda q: (turn_of(q), q["across"]))
    passes.sort(key=turn_of)
    places, waste = P.bites(outline, cands, passes)
    print(f"waste {waste['waste']:.0f} mm2 of section: {len(places)} roughing places take {waste['rough_area']:.0f} in {sum(q['bites'] for q in places)} runs; "
          f"the ball-nose takes the rest in {sum(q['bites'] for q in passes)} runs, the deepest {max(q['depth'] for q in passes):.1f}; {waste['left_at_end']:.1f} stands at the end")
    cuts = []                                        # every time the bit is brought to the stick: kind, tool, t, across, tip, label
    # First the blank is put on its centres, and it has to go on the way round the table takes it to be: the spindle is
    # turned to where one wide face of the blank is level and underneath.
    wide = 0 if blank_of["size"][0] >= blank_of["size"][1] else 1
    load_t = blank_of["turn"] + wide * math.pi / 2
    under, toward = (blank_of["axis"][1], blank_of["size"][0] - blank_of["axis"][0]) if wide == 0 else (blank_of["size"][0] - blank_of["axis"][0], blank_of["size"][1] - blank_of["axis"][1])
    label = lambda *text: "".join(t.ljust(16) for t in text).rstrip()
    cuts.append(("load", "none", load_t, 0.0, blank, label("blank in socket", f"tail ctr {under:.1f} up", f"{toward:.1f} from router", "tail mark up")))
    # A bit is set by touch: run out at the master, level with the stick's axis, until the clip on it reads.
    setting = (0.0, ecc[1], master_r + ecc[0])        # level with the master's middle, at its near side
    cuts.append(("set", "straight") + setting + ("set the straight bit on the master",))
    for q in places:
        for b in range(1, q["bites"] + 1):
            cuts.append(("rough", "straight", q["t"], q["across"], q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"], f"rough {math.degrees(q['t']):.0f} deg {q['across']:+.0f} bite {b} of {q['bites']}"))
    cuts.append(("set", "ball") + setting + ("set the ball-nose on the master",))
    for q in passes:
        for b in range(1, q["bites"] + 1):
            cuts.append(("finish", "ball", q["t"], q["across"], q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"], f"finish bite {b} of {q['bites']}"))
    # How far the sled is fed for a pass: from where the bit waits beside the master to where the collet nut is just
    # short of the socket.
    forward_x = STICK - SOCKET_IN - pos["router"]["collet"][0] / 2 - NUT_CLEAR
    travel = forward_x - BACK_X
    rows = []
    for n, c in enumerate(cuts):
        depth, height, turn = axes.row(c[2], c[3], c[4])
        rows.append({"n": n + 1, "kind": c[0], "tool": c[1], "t": c[2], "across": c[3], "tip": c[4], "label": c[5], "depth": round(depth, 4), "height": round(height, 4), "turn": round(turn, 4),
                     "along": travel if c[0] in ("rough", "finish") else 0.0})
    back = max(abs(axes.cutter(r["depth"], r["height"], r["turn"])[0] - r["across"]) + abs(axes.cutter(r["depth"], r["height"], r["turn"])[1] - r["tip"]) for r in rows)
    if back > 1e-3:
        raise SystemExit(f"the rows do not give the cuts back: {back}")
    cutting = [r for r in rows if r["kind"] in ("rough", "finish")]
    # The two bits, each set as near what the table takes it to as its length and the collet's depth allow. And the
    # collet nut: at every cut it has to pass the uncut blank, with the bit set as short as it may be.
    nut_r, bits = pos["router"]["collet"][0] / 2, {}
    for tool, name, long in BITS:
        lo, hi = max(long - (shank_in - SHANK_OFF), bit_out - BIT_MARGIN), min(long - SHANK_GRIP, bit_out + BIT_MARGIN)
        if lo > hi + 1e-9:
            raise SystemExit(f"{name}, {long} long, cannot stand {bit_out:.1f} out of the nut give or take {BIT_MARGIN}: its shank allows {long - shank_in + SHANK_OFF:.1f} to {long - SHANK_GRIP:.1f}")
        out = min(max(bit_out, lo), hi)
        need = max(P.nut_need(outline, r["t"], r["across"], r["tip"], nut_r, NUT_AIR) for r in cutting if r["tool"] == tool)
        if lo < need - 1e-9:
            raise SystemExit(f"{name} set {lo:.1f} out of the nut: the nut meets the blank, which wants {need:.1f}")
        bits[tool] = {"name": name, "long": long, "out": out, "in": long - out, "range": [lo, hi], "nut_need": need, "nut_air": NUT_AIR}
        print(f"{name}, {long:.1f} long: set {out:.1f} out of the nut ({lo:.1f} to {hi:.1f} will do), {long - out:.1f} of its shank in the collet; the nut passes the uncut blank at every cut with {need:.1f} out or more")
    depths, heights = (min(r["depth"] for r in rows), max(r["depth"] for r in rows)), (min(r["height"] for r in rows), max(r["height"] for r in rows))
    if depths[0] < FW.PAST_MM + BIT_MARGIN or depths[1] > x_travel - BIT_MARGIN + 1e-6 or heights[0] < FW.PROBE_HEIGHT_MM + 2.0 or heights[1] > z_travel - FW.PROBE_HEIGHT_MM - 2.0:
        raise SystemExit(f"the rows do not fit the positioner's travel: depth {depths}, height {heights}")
    print(f"depth {depths[0]:.1f} to {depths[1]:.1f} of the positioner's {x_travel:.0f}, height {heights[0]:.1f} to {heights[1]:.1f} of its {z_travel:.0f}; drawn back to {safe:.1f} the bit clears the master")

    # -- what the cuts ask of the belt ------------------------------------------
    # Nothing but the belt holds the stick while it is cut. Each row's bite is followed on the grid of the waste for
    # its area; that, the feed and the bit's speed give how hard the bit pushes, taken at the furthest point it cuts
    # for the turning it does; and the belt's two runs and the motor's hold, one after the other, give how far the
    # tile's edge moves under that. An estimate, on the heavy side: the bites in plan.py are the deepest that pass it.
    centres = belt_centres(BELT, WHEEL_D, PINION_D)
    span = math.sqrt(centres ** 2 - ((WHEEL_D - PINION_D) / 2) ** 2)            # a run of the belt, pulley to wheel
    k_belt = 2 * BELT_EA * (WHEEL_D / 2) ** 2 / span / 1000.0                   # N m a radian at the stick: both runs, one tightening as the other slackens
    k_motor = 50 * MOTOR_HOLD * RATIO ** 2                                      # a 200-step motor pushed off its place by a: its hold times sin(50 a)
    stiff = 1.0 / (1.0 / k_belt + 1.0 / k_motor)
    worst = {"rough": [0.0, 0], "bite": [0.0, 0], "face": [0.0, 0]}
    for r, (area, over, far) in zip(cutting, P.takes(outline, [(r["tool"], r["t"], r["across"], r["tip"]) for r in cutting])):
        edge_speed = math.pi * P.cutting_dia(r["tool"], over) * ROUTER_RPM / 60.0
        push = PEAK * CUT_ENERGY * 1000.0 * area * FW.FEED_MM_S / edge_speed    # newtons: the power the cut takes, over how fast the edge passes
        torque = push * far / 1000.0
        last = r["kind"] == "finish" and r["label"].split()[2] == r["label"].split()[4]      # the bite that leaves the face
        r.update(area=round(area, 3), over=round(over, 3), push=round(push, 3), torque=round(torque, 4), give=round(torque / stiff * swing_r, 5), face=last)
        key = "rough" if r["kind"] == "rough" else "face" if last else "bite"
        if r["give"] > worst[key][0]:
            worst[key] = [r["give"], r["n"]]
        if r["give"] > GIVE[1 if last else 0]:
            raise SystemExit(f"row {r['n']} ({r['label']}) takes {area:.1f} mm2 and would move the tile's edge {r['give']:.4f}: more than {GIVE[1 if last else 0]}")
    heaviest = max(cutting, key=lambda r: r["torque"])
    print(f"the belt alone holds the stick: {k_belt:.0f} N m a radian from its two runs of {span:.0f}, {k_motor:.0f} from the motor's hold through {RATIO:.0f} to 1, {stiff:.0f} together. "
          f"The heaviest cut, row {heaviest['n']}, takes {heaviest['area']:.1f} mm2 at {FW.FEED_MM_S:.0f} mm/s: {heaviest['push']:.1f} N, {heaviest['torque']:.2f} N m, "
          f"{MOTOR_HOLD * RATIO / heaviest['torque']:.0f} times under what stalls the motor. The tile's edge moves at most {1000 * worst['rough'][0]:.1f} um roughing, "
          f"{1000 * worst['bite'][0]:.1f} in a finishing pass's earlier bites and {1000 * worst['face'][0]:.1f} in one that leaves a face")

    # -- which moves can go straight -----------------------------------------
    # With the sled back the bit is beside the master. The controller moves the height first, then turns the spindle,
    # then runs the bit out; unless told a move may go straight it first draws the bit back to the safe depth. A move
    # may go straight if, followed along that path, the whole of the bit stays clear of the master.
    def bit_edge(tool):
        """Points round the bit's outline from its tip: (toward the router, across), the tip at (0, 0)."""
        if tool == "ball":
            r, pts = P.BIT / 2, []
            for k in range(61):
                off = P.SHANK / 2 * k / 60
                f = P.flank(off, r)
                if f is not None:
                    pts += [(r + f, off), (r + f, -off)]
            far = max(u for u, _ in pts)
            return pts + [(far + k, s * P.SHANK / 2) for s in (-1, 1) for k in range(1, 40)]
        return [(0.0, ROUGH_R * k / 10) for k in range(-10, 11)] + [(float(k), s * ROUGH_R) for s in (-1, 1) for k in range(1, 60)]
    edges = {tool: bit_edge(tool) for tool in ("straight", "ball")}

    def master_gap(depth, height, tool):
        tip, across = axes.tip(depth, height)
        return min(math.hypot(tip + u, across + v) for u, v in edges[tool]) - MASTER_R

    def straight_ok(prev, row):
        steps = lambda a0, a1: [a0 + (a1 - a0) * k / max(1, math.ceil(abs(a1 - a0) / 0.25)) for k in range(max(1, math.ceil(abs(a1 - a0) / 0.25)) + 1)]
        low = row["height"] - FW.PAST_MM if row["height"] < prev["height"] else row["height"]      # a move down goes a little past and comes up
        if any(master_gap(prev["depth"], h, row["tool"]) < MOVE_CLEAR for h in steps(prev["height"], low)):
            return False
        return all(master_gap(d, row["height"], row["tool"]) >= MOVE_CLEAR for d in steps(prev["depth"], row["depth"]))
    for i, r in enumerate(rows):
        r["via"] = "d" if r["kind"] in ("rough", "finish") and rows[i - 1]["kind"] in ("rough", "finish") and straight_ok(rows[i - 1], r) else "s"
    straight = sum(1 for r in rows if r["via"] == "d")
    nearest = min(master_gap(r["depth"], r["height"], r["tool"]) for r in cutting)
    print(f"{straight} of {len(rows)} moves go straight to their row; the rest by the safe depth. At its row the bit is never nearer the master than {nearest:.2f}")

    # -- the table, and the controller's own run over it ---------------------
    table = os.path.join(OUT, "passes.csv")
    K.write_table(table, rows, safe)
    steps = os.path.join(OUT, "steps.json")
    program = os.path.join(HERE, "firmware", "main.py")

    def simulated(*arg):
        """The controller's own program run over a table on a computer: what it writes."""
        scratch = os.path.join(OUT, "steps_scratch.json")
        subprocess.run([sys.executable, program] + [a for a in arg if a == "--lock"] + ["--sim", arg[0], scratch] + [str(a) for a in arg[1:] if a != "--lock"], check=True, cwd=os.path.join(HERE, "firmware"))
        got = json.load(open(scratch))
        os.remove(scratch)
        return got
    sim = simulated(table)
    # and again as if the stick had been left half way: homed, the bit set again, and taken up from the row after
    half = len(rows) // 2
    sim["resumed"] = simulated(table, half)
    if sim["resumed"]["rows"] != sim["rows"][half:]:
        raise SystemExit("taken up half way, the controller does not end the rows where it does run through")
    if sim["passes"] != len(cutting) or sim["fed"] != len(cutting) * round(travel * FW.TRAVEL_STEPS_PER_MM) or any(r[4] for r in sim["rows"]):
        raise SystemExit(f"the controller made {sim['passes']} passes of the table's {len(cutting)}, or did not bring the sled back from one")
    hours = sim["seconds"] / 3600.0
    print(f"{len(cutting)} passes of {travel:.0f} mm at {FW.FEED_MM_S:.0f} mm/s and back at {FW.RETURN_MM_S:.0f}: by the pauses the controller asks for, {int(hours)} h {round(60 * (hours % 1)):02d} min of moving a stick, "
          f"the sled's screw turning {sim['stepped'][3] / FW.TRAVEL_STEPS_PER_MM / SCREW[1] :.0f} times")
    # -- the lock, if one is fitted ---------------------------------------------------
    # None is: the belt holds the stick. The controller is run once more as if one were, to see that the option still
    # does what it says: the lock let go for every row at a new angle and for no other, and the same rows at the end.
    per_turn = round(360.0 * FW.TURN_STEPS_PER_DEG)
    if abs(per_turn - 360.0 * FW.TURN_STEPS_PER_DEG) > 1e-6:
        raise SystemExit("a turn of the stick is not a whole number of steps")
    if abs(per_turn / RATIO / 200.0 - round(per_turn / RATIO / 200.0)) > 1e-9 or abs(FW.TRAVEL_STEPS_PER_MM * SCREW[1] / 200.0 - round(FW.TRAVEL_STEPS_PER_MM * SCREW[1] / 200.0)) > 1e-9:
        raise SystemExit("the controller's steps are not for the wheel and the pulley drawn here, or not for this screw")
    at_steps, locked, unlocks, angles = 0, False, 1, 0
    for r in rows:
        to = round(r["turn"] * FW.TURN_STEPS_PER_DEG) % per_turn
        if to != at_steps:
            unlocks, angles, at_steps = unlocks + locked, angles + 1, to
        locked = True
    with_lock = simulated(table, "--lock")
    if sim["lock"] != FW.LOCK or sim["unlocks"] != (unlocks if FW.LOCK else 0) or with_lock["unlocks"] != unlocks or with_lock["rows"] != sim["rows"] or not with_lock["lock"]:
        raise SystemExit(f"the controller let a lock go {with_lock['unlocks']} times, the table has {unlocks}; or it ends its rows elsewhere with a lock than without")
    sim["with_lock"] = {"unlocks": with_lock["unlocks"], "seconds": with_lock["seconds"]}
    print(f"the stick is turned to a new angle {angles} times in {len(rows)} rows; a lock, were one fitted, would be let go {unlocks} times, homing included")
    # -- homing the wheel --------------------------------------------------------
    # The home fin is 6 mm wide where the sensor reads it, so the sensor says home over six degrees. Wherever the
    # wheel is left, in the fin or out of it, the controller's own homing has to end on the same edge of it.
    import main as fw
    fin_deg = math.degrees(FIN[1] / FLAG_R)
    at = {"steps": 0, "way": 1}

    def turn_step(n, v):
        if n == FW.TURN_PINS[1]:
            at["way"] = 1 if bool(v) != FW.TURN_FLIP else -1
        elif n == FW.TURN_PINS[0] and v:
            at["steps"] += at["way"]
    fw.STEPS, fw.HOME = turn_step, (lambda n: (at["steps"] / FW.TURN_STEPS_PER_DEG) % 360.0 < fin_deg)
    homed = []
    for start in (-200.0, -3.0, 0.0, fin_deg / 2, fin_deg - 0.05, fin_deg + 0.05, 30.0, 180.0, 355.0):
        at["steps"] = round(start * FW.TURN_STEPS_PER_DEG)
        fw.Axis("turn", FW.TURN_PINS, FW.TURN_STEPS_PER_DEG, FW.TURN_FLIP, 0.0).seek(FW.TURN_HOME_PIN, 400.0, FW.TURN_CLEAR_DEG)
        homed.append([start, (at["steps"] / FW.TURN_STEPS_PER_DEG) % 360.0])
    if max(h[1] for h in homed) - min(h[1] for h in homed) > 1.5 / FW.TURN_STEPS_PER_DEG:
        raise SystemExit(f"homing the wheel ends in different places: {homed}")
    print(f"homing the wheel from {len(homed)} places, in the fin and out of it, ends within a step of {homed[0][1]:.2f} deg: the fin's edge at {fin_deg:.2f}")
    # -- the sled coming home ------------------------------------------------------
    # Every pass ends with the sled finding its switch. A sled that stalled on the way out comes to it sooner than
    # its steps say; one that stalled coming back does not come to it at all. The controller's own pass, on a sled
    # that loses so many millimetres of steps: it has to carry on after a little, and stop after more.
    sled_at = {"steps": 0, "way": 1, "out": 0, "back": 0}

    def travel_step(n, v):
        if n == FW.TRAVEL_PINS[1]:
            sled_at["way"] = 1 if bool(v) != FW.TRAVEL_FLIP else -1
        elif n == FW.TRAVEL_PINS[0] and v:
            lose = "out" if sled_at["way"] > 0 else "back"
            if sled_at[lose] > 0:
                sled_at[lose] -= 1                    # a stall: the motor is stepped and the sled stays where it is
            else:
                sled_at["steps"] = max(sled_at["steps"] + sled_at["way"], -round(GUARD * FW.TRAVEL_STEPS_PER_MM))      # no further back than its stop
    fw.STEPS, fw.HOME = travel_step, (lambda n: sled_at["steps"] <= 0)
    fw.SCREEN.quiet = True
    came = []
    for lost_out, lost_back in ((0.0, 0.0), (0.3, 0.0), (2.0, 0.0), (10.0, 0.0), (0.0, 0.3), (0.0, 12.0)):
        sled_at.update(steps=0, way=1, out=round(lost_out * FW.TRAVEL_STEPS_PER_MM), back=round(lost_back * FW.TRAVEL_STEPS_PER_MM))
        machine = fw.Sled(safe)
        try:
            machine.cut(travel)
            said = ""
        except RuntimeError as why:
            said = str(why)
        came.append({"lost_out": lost_out, "lost_back": lost_back, "slip": machine.slip, "stopped": said, "ends": sled_at["steps"] / FW.TRAVEL_STEPS_PER_MM, "counted": machine.travel.at})
    fw.STEPS = fw.HOME = None
    for c in came:
        lost = max(c["lost_out"], c["lost_back"])
        if bool(c["stopped"]) != (lost > FW.HOME_SLIP_MM) or (not c["stopped"] and (abs(c["ends"]) > 0.011 or c["counted"] != 0 or abs(c["slip"] - lost) > 0.011)):
            raise SystemExit(f"the sled's coming home is not what it should be: {c}")
    print("the sled coming home, on a sled that loses steps: " + "; ".join(f"{max(c['lost_out'], c['lost_back']):.1f} mm {'out' if c['lost_out'] else 'back' if c['lost_back'] else 'lost'}: " + (f"stops ({c['stopped']})" if c["stopped"] else f"home, {c['slip']:.2f} out") for c in came))

    # -- the two tables that set a built machine up ------------------------------
    probing = K.probe_rows(drawn_axes, MASTER_R, (FW.PROBE_DEPTH_MM, FW.PROBE_HEIGHT_MM), (x_travel, z_travel))
    K.write_table(os.path.join(OUT, "probe.csv"), probing, safe)
    bands = K.band_rows(axes, setting, BACK_X)
    if max(r["depth"] for r in bands) > x_travel - BIT_MARGIN or max(r["height"] for r in bands) > z_travel - 2.0 or max(r["along"] for r in bands) > travel:
        raise SystemExit("the bands do not fit the positioner's travel, or the sled's")
    K.write_table(os.path.join(OUT, "bands.csv"), bands, safe)
    banded_run = simulated(os.path.join(OUT, "bands.csv"))
    sim["bands"], sim["band_rounds"] = banded_run["rows"], banded_run["rounds"]
    with open(steps, "w") as f:
        json.dump(sim, f)
    print(f"probing: {sum(1 for r in probing if r['kind'] in ('in', 'up', 'dn'))} touches in {len(probing)} rows; bands: {len(bands)} rows, {banded_run['rounds']} bands turned by the wheel's motor")
    print(f"{len(rows)} rows; the controller, simulated, ends at {sim['rows'][-1][1:]} steps and never changes height with the bit further out than step {sim['deepest_crossing']} (safe {sim['safe_steps']})")

    # -- where things are ---------------------------------------------------------
    drawn = next(i for i, r in enumerate(rows) if r["kind"] == "finish" and abs(r["across"]) < 2.0 and r["tip"] > 12.0 and r["label"].startswith("finish bite 1 of 1"))
    row = rows[drawn]
    psi = -row["t"]                                  # the section in the machine: d toward the router, e up
    section = tf(((0.0, za), psi), outline)
    plate_y = (PLATE_LEFT, face - FENCE_R - SPRING_GAP)        # from the datum fence to the near one, which stands against the positioner's face
    rail_near = plate_y[1] - LIP[2] - 2.0            # the near rail's outer face: inside the near fence's lip
    deck_y = (plate_y[0] + BOX_IN, face - DECK_CLEAR)          # the deck: from inside the datum fence's lip to out over the near fence, short of the positioner's face
    if RISER_HALF > deck_y[1] - 1.0 or KP[1] / 2 > RISER_HALF or deck_y[1] - rail_near > 2 * PLY:
        raise SystemExit("a riser is off the deck, a pillow block off its riser, or the deck too far over its rail")
    lip_top = plate_top + LIP[3] + LIP[0]
    hollow_y, hollow_z = (deck_y[0] + PLY, rail_near - PLY), (plate_top, sled_top - PLY)       # the inside of the sled's box, between its rails and under its deck
    half_box = pos["box"]["y"]
    m = MOTOR[0] / 2
    # The wheel goes where the sled goes. Its rim is beyond the end of the sled's box and wider than the box is high:
    # it has to pass over the sled's own bottom, the back stop, the near fence's lip and the controller's box, and the
    # disc and it have to stay beyond the positioner's box with the sled forward. The owner's point, when the wheel
    # stood on the base: a wheel only needs a groove where it does not fit. On the sled it does not need that.
    lip_under = za - math.sqrt(WHEEL_RIM ** 2 - (plate_y[1] - LIP[2]) ** 2) - lip_top        # the rim over the near lip's inner edge, where it is lowest over it
    controller_y, controller_z = (face + 6.0, face + 76.0), 40.0
    box_under = za - math.sqrt(WHEEL_RIM ** 2 - controller_y[0] ** 2) - controller_z
    if za - WHEEL_RIM - plate_top < 5.0 or lip_under < 5.0 or box_under < 5.0:
        raise SystemExit(f"the wheel's rim is {za - WHEEL_RIM - plate_top:.1f} over the sled's bottom, {lip_under:.1f} over the near fence's lip, {box_under:.1f} over the controller's box")
    if DISC_X[0] - (forward_x + half_box) < 5.0:
        raise SystemExit("the disc meets the positioner's box with the sled forward")
    # Its motor is in the sled's box, on the bottom, as low as its bracket's foot lets it be and the belt's length from
    # the spindle: under the deck, between the rails, its shaft out past the box's end to the pulley in the wheel's plane.
    motor_z = plate_top + BRACKET[1] + 2.0 + m
    motor_y = -math.sqrt(centres ** 2 - (za - motor_z) ** 2)
    if motor_y - m - BRACKET[2] < hollow_y[0] + 2.0 or motor_z + m > hollow_z[1] - 2.0 or DRIVE_X[0] - MOTOR[1] > BOX_X[1] - 10.0 or WHEEL_X[1] - DRIVE_X[0] > MOTOR[2] or DRIVE_X[1] > WHEEL_X[0] - 3.0:
        raise SystemExit("the wheel's motor does not fit in the sled's box, or its shaft does not reach the wheel's plane")
    bracket_r = math.hypot(motor_y + m + BRACKET[2], za - (motor_z + m + BRACKET[2]))          # the corner of the bracket's face nearest the spindle: it stands beside the disc
    belt_low = min(za - WHEEL_RIM, motor_z - PINION_D / 2 - 1.5)
    if bracket_r < DISC[0] / 2 + 5.0 or belt_low - plate_top < 5.0 or centres - PINION_D / 2 - 1.5 - WHEEL_RIM < 5.0:
        raise SystemExit("the motor's bracket meets the disc, the belt the sled's bottom, or the pulley the wheel")
    # The belt on the motor's pulley: teeth in mesh. Six is the least a toothed belt is usually given.
    mesh = PINION_T * (math.pi - 2 * math.asin((WHEEL_D - PINION_D) / 2 / centres)) / (2 * math.pi)
    if mesh < 6.0:
        raise SystemExit(f"only {mesh:.1f} of the pulley's teeth are in the belt")
    # The home sensor is on the end of the deck, under the spindle; the fin on the disc's inner face passes through it.
    sensor_z = sled_top - PLY / 2
    if abs(za - FLAG_R - sensor_z) > 1e-9 or SENSOR[2] > PLY or FLAG_R - FIN[0] < HUB[0] / 2 + 5.0 or FLAG_R + FIN[0] > LOCK_R - CLAMP[4] / 2 - 5.0:
        raise SystemExit("the home sensor is off the deck's end, or its fin meets the hub or where a lock's pad would be")
    # With the sled back the bit is beside the tail spindle's bare rod, between its inner flange bearing and the stick.
    if BACK_X - ROUGH_R < MASTER_X[0] + 5.0 or BACK_X + ROUGH_R > MASTER_X[1] - 2.0:
        raise SystemExit("the bit is not beside the bare rod with the sled back")
    # The collet nut is beside the tail's inner pillow block there. Run out for the deepest cut its face is nearer the
    # axis than the block's foot is long, and low enough to meet it, so along the sled it has to be past the block.
    nut_past = BACK_X - pos["router"]["collet"][0] / 2 - TAIL_PB_X[1][1]
    if nut_past < 3.0:
        raise SystemExit(f"the collet nut meets the tail's inner pillow block: {nut_past:.1f} along the sled")
    # The socket, turning: its corners clear the sled's plate and the positioner's face. A pass ends with the collet
    # nut short of it, so the last of the stick is not cut.
    blank_m = tf(((0.0, za), psi), blank_of["corners"])
    socket_out, socket_in = grown(blank_m, SOCKET_FIT + SOCKET_WALL), grown(blank_m, SOCKET_FIT)
    socket_swing = max(math.hypot(y, z - za) for y, z in socket_out)
    if za - sled_top - socket_swing < BED_CLEAR or face - socket_swing < 5.0:
        raise SystemExit(f"the socket, swinging {socket_swing:.1f}, meets the plate or the positioner")
    cut_to = forward_x + ROUGH_R                     # how much of the stick the bit reaches
    # A lock, if one is fitted: the clamp's pad has to be on the disc, the anvil outside the neck and short of the wheel.
    if LOCK_R + CLAMP[4] / 2 > DISC[0] / 2 + 1e-9 or LOCK_R - 9.0 < NECK[0] / 2 + 5.0 or DISC_X[1] + ANVIL[2] + ANVIL[0] > WHEEL_X[0] - 2.0 or LOCK_Y < deck_y[0] or NECK[2] + 8.0 > NECK[0] or NECK[2] < HUB[0] + 6.0:
        raise SystemExit("the disc, its neck and a lock do not fit one another")
    # The base, fixed, in the sled's frame where the sled is drawn: the bit at DRAWN_X along it.
    base = lambda u: u + DRAWN_X                     # from the base's own frame, the bit at 0
    stops_x = (PLATE_X[0] - forward_x, PLATE_X[1] - BACK_X)      # the bottom's two ends at the ends of the travel, in the base's frame
    # The travel: a screw along the base, in through the tail end of the sled's box to a nut on the sled's bottom.
    # Its motor stands on a block at the tail end of the base, beyond where the sled's end comes; the screw's other
    # end is free, in the nut or past it. Everything here is along the base, the bit at 0.
    travel_z = T12 + BRACKET[1] + 2.0 + m            # the screw's height: the motor on its bracket on a block of 1/2 in
    nut_fwd = NUT_X[0] - NUT[1] - forward_x          # the nut's flange with the sled forward: as near the motor as it comes
    coupler_x = (nut_fwd - 8.0 - COUPLER[1], nut_fwd - 8.0)
    screw_x = (coupler_x[0] + (COUPLER[1] + COUPLER[2]) / 2, coupler_x[0] + (COUPLER[1] + COUPLER[2]) / 2 + SCREW[2])
    travel_motor_x = (coupler_x[0] + (COUPLER[1] - COUPLER[2]) / 2 - MOTOR[2] - MOTOR[1], coupler_x[0] + (COUPLER[1] - COUPLER[2]) / 2 - MOTOR[2])
    nut_back = NUT_X[1] - BACK_X                     # the far side of the nut's block with the sled back: as far along the screw as it goes
    if (screw_x[1] - nut_back < 10.0 or travel_z - NUT[0] / 2 - plate_top < 3.0 or travel_z + NUT_BLOCK[1] > hollow_z[1] - 3.0 or not hollow_y[0] + 5.0 < TRAVEL_Y - NUT_BLOCK[0] / 2 < TRAVEL_Y + NUT_BLOCK[0] / 2 < hollow_y[1] - 5.0
            or coupler_x[1] > stops_x[0] - GUARD - 3.0 or screw_x[1] + forward_x > DRIVE_X[0] - MOTOR[1] - 10.0):
        raise SystemExit("the travel's screw, its nut or its coupler do not fit the sled's box")
    base_x = (travel_motor_x[0] - 15.0, stops_x[1] + GUARD + STOP[0] + 15.0)
    base_y = (plate_y[0] - FENCE_L - 15.0, face + (box_x[1] - box_x[0]) + 10.0)
    fence_x = (stops_x[0] - GUARD - STOP[0], stops_x[1] + 6.0)   # the fences, end to end: the sled's whole travel
    clamp_x = (POST_X[0] - CLAMP[2], POST_X[0])      # a lock's clamp, on its riser beside the headstock: riser and body hard up against the post, which the anvil pulls the other way
    if deck_y[1] + 5.0 > face:
        raise SystemExit("the deck meets the positioner's face")
    bit_z = bit_home_z + row["height"]
    col = pos["sleeve"]["outside"] / 2
    drive = pos["drive"]

    def base_board(p):
        box(p, "base", (base(base_x[0]), base(base_x[1])), base_y, (-PLY, 0.0))
        if PLINTH > 0.0:
            box(p, "plinth under the positioner", (base(-half_box), base(half_box)), (face, face + box_x[1] - box_x[0]), (0.0, PLINTH), op="add")

    def fences(p):
        # Two strips screwed to the base, each with a lip over the sled's edge. The far one is the datum: the sled's
        # edge runs along it, and any bow in it is cut into the stick. The near one only carries its lip and takes
        # the plungers' push; it stands against the positioner's face.
        x, z = (base(fence_x[0]), base(fence_x[1])), (0.0, plate_top + LIP[3])
        box(p, "datum fence", x, (plate_y[0] - FENCE_L, plate_y[0]), z)
        box(p, "its lip", x, (plate_y[0] - FENCE_L, plate_y[0] + LIP[1]), (z[1], lip_top), op="add")
        box(p, "near fence", x, (plate_y[1] + SPRING_GAP, face), z)
        box(p, "its lip", x, (plate_y[1] - LIP[2], face), (z[1], lip_top), op="add")

    def slips(p):
        # What the sled rides on: two strips of something slippery on the base, one under each edge.
        for y in ((plate_y[0] + 2.0, plate_y[0] + 32.0), (plate_y[1] - 32.0, plate_y[1] - 2.0)):
            box(p, "slip", (base(stops_x[0]), base(stops_x[1])), y, (0.0, SLIP))

    def plate(p):
        box(p, "sled's bottom", PLATE_X, plate_y, (SLIP, plate_top))

    def sled_box(p):
        # On the bottom: a rail on edge along each side, and the deck across them. The bottom goes on past them, under the wheel.
        for y in ((deck_y[0], deck_y[0] + PLY), (rail_near - PLY, rail_near)):
            box(p, "rail", BOX_X, y, (plate_top, sled_top - PLY))
        box(p, "deck", BOX_X, deck_y, (sled_top - PLY, sled_top))

    def plungers(p):
        # Spring plungers in the bottom's near edge, their noses on the near fence: they keep the far edge on the datum.
        for x in PLUNGER[1]:
            ycyl(p, "spring plunger's nose", (plate_y[1], plate_y[1] + SPRING_GAP), (x, plate_top - PLY / 2), PLUNGER[0])

    def riser(p, name, x):
        # Two layers of 1/2 in glued face to face, and that glued and nailed face down on the deck. Both are sawn from
        # one glued-up strip, so the two spindles start at one height.
        box(p, name, x, (-RISER_HALF, RISER_HALF), (sled_top, sled_top + RISER))

    def pillow_blocks(p, where):
        # Bolted down through the riser and the deck, M6 in holes drilled a size over. Their set screws hold the rod
        # endways, and what pushes along the rod goes into the riser's glue in shear.
        for x in where:
            box(p, "KP001's foot", x, (-KP[1] / 2, KP[1] / 2), (za - KP[3], za - KP[3] + KP[5]))
            xcyl(p, "its housing", x, (0.0, za), KP[2], op="add")
        xcyl(p, "bore", (where[0][0] - 0.5, where[1][1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def tail_spindle(p):
        # Where the bit waits. The same rod as the drive's, turning with the stick: three flats ground on its end make
        # a point that bites. Bare beside the stick, it is the master a bit is set against.
        xcyl(p, "tail spindle, 12 mm ground rod", TAIL_ROD_X, (0.0, za), SPINDLE_D)

    def spindle(p):
        xcyl(p, "12 mm ground rod", ROD_X, (0.0, za), SPINDLE_D)

    def socket(p):
        # Printed. The blank's end stands SOCKET_IN in the pocket and can be pushed SOCKET_SLIDE further; a jack screw
        # through the floor presses it back onto the tail spindle's point, and a screw through a wall locks it against
        # the wall opposite. The rod's end, roughened, is epoxied into the boss.
        slab(p, "blank socket", (SOCKET_X[0], BOSS_X[0]), socket_out)
        slab(p, "pocket", (SOCKET_X[0] - 0.5, SOCKET_X[1]), socket_in, op="remove")
        xcyl(p, "boss", BOSS_X, (0.0, za), SOCKET_BOSS[0], op="add")
        xcyl(p, "the rod's socket", (BOSS_X[0], BOSS_X[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    # -- on the spindle's far end: the hub, the disc and the wheel ---------------------------
    def wheel_hub(p):
        # A flanged hub on the rod, its set screws on a flat, its boss toward the headstock where a key reaches them.
        xcyl(p, "hub's boss", (HUB_X[0], HUB_X[1] - HUB[1]), (0.0, za), HUB[2])
        xcyl(p, "hub's flange", (HUB_X[1] - HUB[1], HUB_X[1]), (0.0, za), HUB[0], op="add")
        xcyl(p, "bore", (HUB_X[0] - 0.5, HUB_X[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def disc(p):
        # Printed in one: the disc, its neck toward the wheel, and the home flag on its inner face. Bolted to the
        # hub's flange through slack holes and trued by its rim under an indicator. A hand turns the stick by it.
        xcyl(p, "disc", DISC_X, (0.0, za), DISC[0])
        xcyl(p, "neck", NECK_X, (0.0, za), NECK[0], op="add")
        xcyl(p, "bore, clear of the rod", (DISC_X[0] - 0.5, NECK_X[1] + 0.5), (0.0, za), SPINDLE_D + 1.0, op="remove")
        # The flag: in the sensor's slot, under the spindle, at turn 0, and drawn where this row has it.
        a = math.radians(-90.0 + row["turn"])
        cs, sn = math.cos(a), math.sin(a)
        fin = p.sketch("right", DISC_X[0] - FIN_L, "home flag")
        p.polygon(fin, [(r * cs - t * sn, za + r * sn + t * cs) for r, t in ((FLAG_R - FIN[0] / 2, -FIN[1] / 2), (FLAG_R + FIN[0] / 2, -FIN[1] / 2), (FLAG_R + FIN[0] / 2, FIN[1] / 2), (FLAG_R - FIN[0] / 2, FIN[1] / 2))])
        p.extrude(fin, FIN_L, op="add", name="home flag")

    def wheel(p):
        # Printed: the toothed rim and its web. Four screws through slack holes into the disc's neck, so that it can
        # be tapped true under an indicator, by its own rim, before they are tightened.
        xcyl(p, f"{WHEEL_T}-tooth wheel", WHEEL_X, (0.0, za), 2 * WHEEL_RIM)
        xcyl(p, "bore, clear of the rod", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (0.0, za), SPINDLE_D + 1.0, op="remove")

    def home_sensor(p):
        # On the end of the deck, under the spindle: the disc's fin passes through its slot once a turn.
        x = (BOX_X[1], BOX_X[1] + SENSOR[0])
        box(p, "optical endstop", x, (-SENSOR[1] / 2, SENSOR[1] / 2), (sensor_z - SENSOR[2] / 2, sensor_z + SENSOR[2] / 2))
        box(p, "its slot", (x[1] - 8.0, x[1] + 0.5), (-SENSOR[1] / 2 - 0.5, SENSOR[1] / 2 + 0.5), (sensor_z - 1.6, sensor_z + 1.6), op="remove")

    def drive_motor(p):
        # In the sled's box. The bracket's foot is screwed to the sled's bottom through slots across the sled: they
        # tension the belt, and the belt's pull stays inside the sled.
        y = (motor_y - m - BRACKET[2], motor_y + m + BRACKET[2])
        box(p, "bracket", DRIVE_X, y, (plate_top, motor_z + m + BRACKET[2]))
        box(p, "bracket's foot", (DRIVE_X[0] - MOTOR[1], DRIVE_X[1]), y, (plate_top, plate_top + BRACKET[1]), op="add")
        xcyl(p, "motor's pilot", (DRIVE_X[0] - 1.0, DRIVE_X[1] + 1.0), (motor_y, motor_z), 22.5, op="remove")
        box(p, "wheel's motor", (DRIVE_X[0] - MOTOR[1], DRIVE_X[0]), (motor_y - m, motor_y + m), (motor_z - m, motor_z + m))
        xcyl(p, f"{PINION_T}-tooth pulley", WHEEL_X, (motor_y, motor_z), PINION_D + 3.0)
        xcyl(p, "bore", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (motor_y, motor_z), MOTOR[3] + 0.2, op="remove")
        xcyl(p, "motor shaft", (DRIVE_X[0], WHEEL_X[1]), (motor_y, motor_z), MOTOR[3])       # last: a bore cut after it would take it too

    # -- a lock, if one is fitted: each part in its own tab, and on the machine only then ------------
    def lock_post(p):
        # Beside the headstock's riser, glued and nailed to it, to the deck and to the clamp's riser: the clamp's
        # plunger goes through it and the anvil is bolted to its face. The anvil pulls the post one way and the clamp
        # pushes its riser the other, so the two are hard together and the pinch stays between them.
        box(p, "lock post", POST_X, (LOCK_Y, -RISER_HALF), (sled_top, za + 28.0))
        xcyl(p, "the plunger's way", (POST_X[0] - 1.0, POST_X[1] + 1.0), (-LOCK_R, za), CLAMP[3] + 4.0, op="remove")
        box(p, "clamp's riser", clamp_x, (-LOCK_R - CLAMP[0] / 2, -LOCK_R + CLAMP[0] / 2), (sled_top, za - CLAMP[1] / 2))

    def lock_anvil(p):
        # A spacer on the post's face, outside the disc's rim, and on it a steel plate that reaches in behind the
        # disc, between it and the wheel: what the clamp pinches the disc against. The spindle's bearings feel none of it.
        z = (za - ANVIL[1], za + ANVIL[1])
        box(p, "spacer", (POST_X[1], DISC_X[1] + ANVIL[2]), (LOCK_Y, -(DISC[0] / 2 + 3.0)), z)
        box(p, "anvil", (DISC_X[1] + ANVIL[2], DISC_X[1] + ANVIL[2] + ANVIL[0]), (LOCK_Y, -(LOCK_R - 9.0)), z, op="add")

    def clamp(p):
        # A push-pull toggle clamp. Its plunger moves straight along the spindle, so closing it pushes the disc onto
        # the anvil and does not drag it round. Drawn closed.
        y = (-LOCK_R - CLAMP[0] / 2, -LOCK_R + CLAMP[0] / 2)
        box(p, "toggle clamp", clamp_x, y, (za - CLAMP[1] / 2, za + CLAMP[1] / 2))
        xcyl(p, "plunger", (clamp_x[1], DISC_X[0] - 5.0), (-LOCK_R, za), CLAMP[3], op="add")
        xcyl(p, "pad", (DISC_X[0] - 5.0, DISC_X[0]), (-LOCK_R, za), CLAMP[4], op="add")

    def lock_switch(p):
        # On the post's top, where the clamp's handle comes down on it.
        box(p, "lock switch", (POST_X[0], POST_X[0] + SWITCH[1]), (-LOCK_R - SWITCH[0] / 2, -LOCK_R + SWITCH[0] / 2), (za + 28.0, za + 28.0 + SWITCH[2]))

    # -- the travel -----------------------------------------------------------------------------
    def travel_nut(p):
        # On the sled: a block screwed to its bottom just inside the box's tail end, and the screw's flanged nut
        # bolted to its outer face. The block is screwed down last, with the sled at the motor's end, so that it
        # takes its place from the screw.
        box(p, "nut block", NUT_X, (TRAVEL_Y - NUT_BLOCK[0] / 2, TRAVEL_Y + NUT_BLOCK[0] / 2), (plate_top, travel_z + NUT_BLOCK[1]))
        xcyl(p, "the nut's way", (NUT_X[0] - 0.5, NUT_X[1] + 0.5), (TRAVEL_Y, travel_z), NUT[2] + 0.4, op="remove")
        xcyl(p, "nut's flange", (NUT_X[0] - NUT[1], NUT_X[0]), (TRAVEL_Y, travel_z), NUT[0])
        xcyl(p, "nut's body", (NUT_X[0], NUT_X[0] + NUT[3]), (TRAVEL_Y, travel_z), NUT[2])        # its own body: added to the flange it would be added to the block as well
        xcyl(p, "thread", (NUT_X[0] - NUT[1] - 0.5, NUT_X[0] + NUT[3] + 0.5), (TRAVEL_Y, travel_z), SCREW[0] + 0.2, op="remove")

    def travel_drive(p):
        # On the base: a block, the motor on its bracket on that, a rigid coupler and the screw. The screw's far end
        # is carried by the nut. What the cut pushes back along the screw goes into the motor's own bearings.
        y = (TRAVEL_Y - m - BRACKET[2], TRAVEL_Y + m + BRACKET[2])
        face_x = (base(travel_motor_x[1]), base(travel_motor_x[1]) + BRACKET[0])
        box(p, "bracket", face_x, y, (T12, travel_z + m + BRACKET[2]))
        box(p, "bracket's foot", (base(travel_motor_x[0]), face_x[1]), y, (T12, T12 + BRACKET[1]), op="add")
        xcyl(p, "motor's pilot", (face_x[0] - 1.0, face_x[1] + 1.0), (TRAVEL_Y, travel_z), 22.5, op="remove")
        box(p, "motor block", (base(travel_motor_x[0]) - 5.0, face_x[1] + 5.0), y, (0.0, T12))      # after the bracket: the foot is added to whatever it touches
        box(p, "travel's motor", (base(travel_motor_x[0]), base(travel_motor_x[1])), (TRAVEL_Y - m, TRAVEL_Y + m), (travel_z - m, travel_z + m))
        xcyl(p, "coupler", (base(coupler_x[0]), base(coupler_x[1])), (TRAVEL_Y, travel_z), COUPLER[0])
        xcyl(p, "its bore", (base(coupler_x[0]) - 0.5, base(coupler_x[1]) + 0.5), (TRAVEL_Y, travel_z), SCREW[0] + 0.2, op="remove")
        xcyl(p, "motor shaft", (base(travel_motor_x[1]), base(travel_motor_x[1]) + MOTOR[2]), (TRAVEL_Y, travel_z), MOTOR[3])
        xcyl(p, "T8 screw", (base(screw_x[0]), base(screw_x[1])), (TRAVEL_Y, travel_z), SCREW[0])

    def stick(p):
        # Milled as far as the bit reaches; the rest, into the socket, is still the blank.
        slab(p, "stick, milled", (0.0, cut_to), section)
        slab(p, "the end the bit does not reach", (cut_to, STICK), blank_m, op="add")

    def stops(p):
        """Blocks on the base beyond each end of the sled's travel. The screw puts the sled where it goes; only a
        sled that has lost its place meets one of these, before the collet nut meets the socket."""
        y = (plate_y[0], plate_y[0] + STOP[1])
        box(p, "forward stop", (base(stops_x[0] - GUARD - STOP[0]), base(stops_x[0] - GUARD)), y, (0.0, plate_top))
        box(p, "back stop", (base(stops_x[1] + GUARD), base(stops_x[1] + GUARD + STOP[0])), y, (0.0, plate_top), op="new")

    def home_switch(p):
        # beside the back stop, its end where the sled's bottom ends with the sled drawn back: the bottom's end opens it
        box(p, "travel's home switch", (base(stops_x[1]), base(stops_x[1]) + SWITCH[0]), (plate_y[0] + STOP[1], plate_y[0] + STOP[1] + SWITCH[2]), (plate_top - SWITCH[1], plate_top))

    def controller(p):
        # on the base, beside the positioner: the disc and the wheel go by over it
        box(p, "controller box", (base(half_box + 12.0), base(half_box + 102.0)), controller_y, (0.0, controller_z))

    # The positioner as its own example draws it, by its outside: the box on its lid, the X column and its drive out
    # of the far side, level with the bit; the Z column on top; the collet nut and the bit at this row.
    def positioner(p):
        y0 = face
        box(p, "outer box", (base(-half_box), base(half_box)), (y0, y0 + box_x[1] - box_x[0]), (PLINTH, PLINTH + lid + wall))
        slot = pos["box"]["slot_w"] / 2
        # the slot, and behind it the room the quill's nose, the collet nut and the bit move in
        box(p, "bit slot", (base(-slot), base(slot)), (y0 - 1.0, y0 + 100.0), (PLINTH + lid - pos["box"]["slot_z"][1], PLINTH + lid - pos["box"]["slot_z"][0]), op="remove")
        ycol = (y0 + box_x[1] - box_x[0], y0 + pos["x"]["end"] - drive["motor"][1] - box_x[0])
        box(p, "X column and drive", (base(-col), base(col)), ycol, (bit_z - col, bit_z + col))
        mm = drive["motor"][0] / 2
        box(p, "X motor", (base(-mm), base(mm)), (ycol[1], ycol[1] + drive["motor"][1]), (bit_z - mm, bit_z + mm))
        zc = y0 + pos["stage"]["x"] / 2 - box_x[0]
        ztop = PLINTH + lid + wall
        box(p, "Z column", (base(-col), base(col)), (zc - col, zc + col), (ztop, ztop + drive["length"] + drive["cap"]))
        box(p, "Z motor", (base(-mm), base(mm)), (zc - mm, zc + mm), (ztop + drive["length"] + drive["cap"], ztop - pos["z"]["end"] - wall))

    def bit(p):
        tool = P.BIT if row["tool"] == "ball" else P.ROUGH_BIT
        ycyl(p, "bit", (row["tip"], row["tip"] + bit_out), (base(0.0), bit_z), tool)
        # the collet nut, and behind it the spindle and the router's nose, as far as the quill's front
        y = row["tip"] + bit_out
        for k, (name, key) in enumerate((("collet nut", "collet"), ("spindle", "shaft"), ("router's nose", "nose"))):
            d, long = pos["router"][key]
            ycyl(p, name, (y, y + long), (base(0.0), bit_z), d, op="add" if k else "new")
            y += long

    lock = ("Lock post", "Lock anvil", "Toggle clamp", "Lock switch")       # a lock's parts: drawn, and on the machine only if the controller's config says one is fitted
    parts = [("Base", base_board), ("Fences", fences), ("Slips", slips), ("Sled plate", plate), ("Sled box", sled_box), ("Spring plungers", plungers), ("Tailstock", lambda p: riser(p, "tail riser", TAIL_RISER_X)),
             ("Tail bearings KP001", lambda p: pillow_blocks(p, TAIL_PB_X)), ("Tail spindle", tail_spindle), ("Headstock", lambda p: riser(p, "head riser", HEAD_RISER_X)),
             ("Head bearings KP001", lambda p: pillow_blocks(p, HEAD_PB_X)), ("Spindle", spindle), ("Blank socket", socket),
             ("Wheel hub", wheel_hub), ("Disc", disc), ("Index wheel", wheel), ("Turn home sensor", home_sensor), ("Wheel drive", drive_motor), ("Travel nut", travel_nut), ("Stick", stick),
             ("Lock post", lock_post), ("Lock anvil", lock_anvil), ("Toggle clamp", clamp), ("Lock switch", lock_switch),
             ("Travel drive", travel_drive), ("Stops", stops), ("Travel home switch", home_switch), ("Controller", controller), ("Router positioner", positioner), ("Bit", bit)]
    # What goes with the sled.
    rides = ("Sled plate", "Sled box", "Spring plungers", "Tailstock", "Tail bearings KP001", "Tail spindle", "Headstock", "Head bearings KP001", "Spindle", "Blank socket",
             "Wheel hub", "Disc", "Index wheel", "Turn home sensor", "Wheel drive", "Travel nut", "Stick") + lock
    fitted = [t for t, _ in parts if FW.LOCK or t not in lock]

    mcp.call("create_document", {"name": "sled"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Stick sled for the leaning half-ellipse Spectre"}, {"type": "rename_tab", "tab": 1, "name": parts[0][0]}]})
    tabs, bodies = {}, {}
    for i, (title, make) in enumerate(parts):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
    zero = {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}

    def assembled(name, moved):
        tab = new_tab(mcp, {"type": "add_assembly", "name": name})
        ops = [{"type": "assembly", "tab": tab, "op": {"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True,
                                                       "placement": {"position": {"x": moved(t), "y": 0.0, "z": 0.0}, "rotation": zero["rotation"]}}}
               for t in fitted for k in range(len(bodies[t]))]
        text = mcp.call("apply", {"ops": ops})
        if "ERROR:" in text:
            raise SystemExit(text)
        return tab
    asm = assembled("Sled", lambda t: 0.0)
    # And again with the sled drawn back, where it waits while the positioner and the spindle go to a row.
    seated = DRAWN_X - BACK_X                        # from along the sled to where it is drawn there
    home = assembled("Sled home", lambda t: seated if t in rides else 0.0)
    top_z = PLINTH + lid + wall
    mcp.call("screenshot", {"tab": home, "view": "0,-1,0", "section": "y:0", "fit": f"{BOSS_X[0] + seated - 6.0},-1,{-PLY - 4.0},{base(stops_x[1] + GUARD + STOP[0]) + 6.0},1,{za + WHEEL_RIM + 8.0}", "width": 1300, "height": 1000,
                            "path": os.path.join(OUT, "index.png")})
    mcp.call("screenshot", {"tab": home, "view": "0.6,-0.65,0.45", "width": 1700, "height": 1100, "path": os.path.join(OUT, "home.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-0.55,-0.7,0.5", "width": 1700, "height": 1100, "path": os.path.join(OUT, "sled.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{DRAWN_X}", "fit": f"{DRAWN_X},{base_y[0] - 10},-30,{DRAWN_X + 1},{face + 200},{top_z * 0.62}", "width": 1600, "height": 1000, "path": os.path.join(OUT, "end.png")})
    # the travel, cut along its screw with the sled forward... as drawn, part way: the motor on its block, the coupler, the screw in through the box's end, the nut
    mcp.call("screenshot", {"tab": asm, "view": "0,-1,0", "section": f"y:{TRAVEL_Y}", "fit": f"{base(base_x[0]) - 6.0},-1,{-PLY - 4.0},{base(screw_x[1]) + 30.0},1,{sled_top + 30.0}", "width": 1700, "height": 700,
                            "path": os.path.join(OUT, "travel.png")})
    # the stick's own chain, cut along its axis: the head end from the socket to the wheel, and the tail spindle where the bit waits
    for name, x in (("drive.png", (cut_to - 14.0, WHEEL_X[1] + 10.0)), ("tail.png", (TAIL_ROD_X[0] - 8.0, 14.0))):
        mcp.call("screenshot", {"tab": asm, "view": "0,-1,0", "section": "y:0", "fit": f"{x[0]},-1,{sled_top - PLY - 4.0},{x[1]},1,{za + (DISC[0] / 2 + 6.0 if name == 'drive.png' else 48.0)}", "width": 1500, "height": 900, "path": os.path.join(OUT, name)})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "sled.pdf"),
                              "note": f"stick sled: the stick turns on a sled between two fences, a wheel on its own spindle sets the angle, the router positioner beside it sets depth and height, and a screw feeds the sled past the bit; {len(rows)} rows a stick"}))
    mcp.close()

    count = {k: sum(1 for r in rows if r["kind"] == k) for k in ("load", "set", "rough", "finish")}
    numbers = {
        "edge": EDGE, "bulge": BULGE, "gap": GAP, "spindle_d": SPINDLE_D, "bit": P.BIT, "taper": P.TAPER, "shank": P.SHANK, "ridge": P.RIDGE, "stick": STICK,
        "tile": {"across": tile["across"], "area": tile["area"], "outline": outline, "swing": swing_r, "axis_in_tile": centre, "circle": circle},
        "blank_swing": blank,
        "blank": {"corners": blank_of["corners"], "size": blank_of["size"], "turn": blank_of["turn"], "axis": blank_of["axis"], "over": P.BLANK,
                  "load": {"t": load_t, "under": under, "toward": toward}},
        "plan": {"passes": len(passes), "covered": covered, "worst": max(left), "orphans": orphans, "across_limit": P.Y_LIM, "face_min": P.FACE_MIN, "off_centre": P.OFF_CENTRE,
                 "finish_runs": sum(q["bites"] for q in passes), "deepest": max(q["depth"] for q in passes), "ball_bite": P.BALL_BITE},
        "rough": {"bit": P.ROUGH_BIT, "bite": P.ROUGH_BITE, "step": P.ROUGH_STEP, "allow": P.ALLOW, "angles": P.ROUGH_ANGLES, "places": len(places),
                  "runs": sum(q["bites"] for q in places), **waste},
        "hold": {"cut_energy": CUT_ENERGY, "rpm": ROUTER_RPM, "peak": PEAK, "feed": FW.FEED_MM_S, "return": FW.RETURN_MM_S, "belt_ea": BELT_EA, "motor_hold": MOTOR_HOLD, "span": span,
                 "k_belt": k_belt, "k_motor": k_motor, "stiff": stiff, "limit": GIVE, "worst": worst, "heaviest": heaviest["n"], "stall_over": MOTOR_HOLD * RATIO / heaviest["torque"],
                 "seconds": sim["seconds"], "cell": P.CELL},
        "axes": {"tip_home": tip_home, "across_home": axes.across_home, "scale": list(axes.scale), "shear": axes.shear, "travel": [x_travel, z_travel], "safe": safe, "safe_clear": SAFE_CLEAR,
                 "move_clear": MOVE_CLEAR, "bit_margin": BIT_MARGIN, "bit_out": bit_out, "bit_long": bit_long, "shank": [shank_in, SHANK_GRIP, SHANK_OFF], "bits": bits, "tip_lim": P.TIP_LIM, "past": FW.PAST_MM, "turn_past": FW.TURN_PAST_DEG, "nearest_master": nearest, "probe": [FW.PROBE_DEPTH_MM, FW.PROBE_HEIGHT_MM]},
        "straight": straight,
        "sled": {"za": za, "axis_up": AXIS_UP, "face": face, "face_clear": FACE_CLEAR, "bed_clear": BED_CLEAR, "top": sled_top, "plate_top": plate_top, "axis_over": AXIS_OVER, "deck_y": deck_y, "rail_near": rail_near, "deck_clear": DECK_CLEAR,
                 "plate_x": PLATE_X, "box_x": BOX_X, "plate_y": plate_y, "hollow_y": hollow_y, "hollow_z": hollow_z,
                 "kp": KP, "riser": RISER, "riser_half": RISER_HALF, "t12": T12, "head_pb_x": HEAD_PB_X, "head_riser_x": HEAD_RISER_X, "tail_pb_x": TAIL_PB_X, "tail_riser_x": TAIL_RISER_X,
                 "nut_past": nut_past, "tail_rod_x": TAIL_ROD_X, "master_x": MASTER_X, "master_r": MASTER_R, "rod_x": ROD_X,
                 "socket": {"in": SOCKET_IN, "slide": SOCKET_SLIDE, "wall": SOCKET_WALL, "fit": SOCKET_FIT, "floor": SOCKET_FLOOR, "boss": SOCKET_BOSS, "x": SOCKET_X, "boss_x": BOSS_X,
                            "swing": socket_swing, "nut_clear": NUT_CLEAR},
                 "forward_x": forward_x, "cut_to": cut_to,
                 "slip": SLIP, "fence": [FENCE_L, FENCE_R], "fence_x": fence_x, "lip": LIP, "lip_top": lip_top, "spring_gap": SPRING_GAP, "plunger": PLUNGER,
                 "back_x": BACK_X, "travel": travel, "drawn_x": DRAWN_X, "seated": seated, "stops_x": stops_x, "stop": STOP, "guard": GUARD, "switch": SWITCH, "base_x": base_x, "base_y": base_y, "ply": PLY, "plinth": PLINTH,
                 "controller": {"x": [half_box + 12.0, half_box + 102.0], "y": controller_y, "z": controller_z}, "rides": rides},
        "turn": {"wheel": [WHEEL_T, PINION_T, BELT, centres], "ratio": RATIO, "wheel_w": WHEEL_W, "wheel_x": WHEEL_X, "wheel_rim": WHEEL_RIM, "mesh": mesh, "hub": HUB, "hub_x": HUB_X,
                 "disc": DISC, "disc_x": DISC_X, "neck": NECK, "neck_x": NECK_X, "flag_r": FLAG_R, "fin": FIN, "fin_l": FIN_L, "sensor": SENSOR, "sensor_z": sensor_z,
                 "motor": MOTOR, "bracket": BRACKET, "motor_y": motor_y, "motor_z": motor_z, "drive_x": DRIVE_X, "bracket_r": bracket_r, "belt_low": belt_low,
                 "over": {"bottom": za - WHEEL_RIM - plate_top, "base": za - WHEEL_RIM, "lip": lip_under, "controller": box_under, "positioner": DISC_X[0] - (forward_x + half_box)},
                 "steps_per_turn": 360.0 * FW.TURN_STEPS_PER_DEG, "angles": angles,
                 "homing": {"fin_deg": fin_deg, "step_deg": 1.0 / FW.TURN_STEPS_PER_DEG, "clear_deg": FW.TURN_CLEAR_DEG, "ends": homed}},
        "travel": {"screw": SCREW, "nut": NUT, "coupler": COUPLER, "y": TRAVEL_Y, "z": travel_z, "nut_block": NUT_BLOCK, "nut_x": NUT_X, "coupler_x": coupler_x, "screw_x": screw_x, "motor_x": travel_motor_x,
                   "nut_fwd": nut_fwd, "nut_back": nut_back, "steps_per_mm": FW.TRAVEL_STEPS_PER_MM, "home_short": FW.HOME_SHORT_MM, "home_slip": FW.HOME_SLIP_MM, "came": came},
        "lock": {"fitted": FW.LOCK, "parts": lock, "post_x": POST_X, "lock_r": LOCK_R, "lock_y": LOCK_Y, "anvil": ANVIL, "clamp": CLAMP, "clamp_x": clamp_x, "unlocks": unlocks},
        "positioner": {"half_box": half_box, "box_y": [face, face + box_x[1] - box_x[0]], "top": top_z, "bit_home_z": bit_home_z, "collet": pos["router"]["collet"], "nut_behind": nut_behind,
                       "router_d": pos["router"]["d"], "size": pos["size"],
                       "length_beyond_face": pos["x"]["end"] - box_x[0]},
        "drawn": {"row": row["n"], "x": DRAWN_X, "psi": psi, "bit_z": bit_z},
        "calibration": {"drawn": {"tip_home": tip_home, "across_home": drawn_axes.across_home, "master_r": MASTER_R}, "rows": probing, "setting": setting, "built": bool(built),
                        "pin_d": K.PIN_D, "over": K.OVER},
        "bands": {"r": K.BAND_R, "span": K.BAND_SPAN, "at": K.BAND_AT, "rows": bands},
        "count": count, "rows": rows,
        "bodies": {k: [list(v) for v in b_] for k, b_ in bodies.items()},
    }
    with open(os.path.join(OUT, "sled.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"base {base_x[1] - base_x[0]:.0f} x {base_y[1] - base_y[0]:.0f}; the sled's bottom {PLATE_X[1] - PLATE_X[0]:.0f} x {plate_y[1] - plate_y[0]:.0f}, its box {BOX_X[1] - BOX_X[0]:.0f} long, {travel:.0f} of travel; "
          f"the wheel {2 * WHEEL_RIM:.0f} across, {za - WHEEL_RIM:.1f} over the base and {lip_under:.1f} over the near fence's lip, its motor {centres:.1f} from the spindle, {mesh:.1f} teeth in the belt; "
          f"the disc {DISC[0]:.0f}; a lock {'fitted' if FW.LOCK else 'drawn and left off'}; rows: {count}")


if __name__ == "__main__":
    main()
