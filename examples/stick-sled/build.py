#!/usr/bin/env python3
"""A stick sled: the leaning half-ellipse Spectre milled round one solid stick.

The stick is held between a socket on a spindle and the point of a
second spindle, on a flat sled that slides on the base between two
fences; it only turns. Beside it, fixed to the base, lies
the two-axis router positioner (examples/router-positioner), its bit
pointing at the stick: its X sets how deep the bit reaches, its Z how
high. For each pass three motors go to the next row of a table and
stop, and the sled is pushed past the bit and drawn back by hand.

It replaces the hinged sled (examples/stick-sled-hinged), which hung
the stick from two hinges over a cutter fixed in a router table: the
positioner's two straight screws do what the hinges and their links
did, and the stick's own mounting has nothing left to do but turn.

    cargo build --release -p ok-mcp
    python3 examples/router-positioner/build.py     # the positioner's sizes are read from its out/positioner.json
    python3 examples/stick-sled/build.py
    cargo test -p ok-render --test stick_sled   # what CI runs

Writes out/sled.okpart (every part in its own tab, and the sled at one
row), sled.png, end.png, drive.png, tail.png, sled.pdf, passes.csv (the rows the controller
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
AXIS_OVER = 43.5                                # the stick's axis over the sled's deck: the socket turns 4 mm clear of it
BOX_IN = 12.0                                   # the sled's box stands this far in from its bottom's far edge: the flange the datum fence's lip is over
FENCE_L, FENCE_R = 30.0, 14.0                   # the fences, across. The far one is the datum; the near one stands between the sled and the positioner
LIP = (6.0, 10.0, 7.0, 0.3)                     # a fence's lip: thick, how far over the sled at the datum fence and at the near one, and clear of the sled's top
SPRING_GAP = 1.0                                # between the sled and the near fence: spring plungers in the sled's edge press it onto the datum
PLUNGER = (6.0, (0.0, 250.0))                   # those plungers: across, and where along the sled
PLATE_LEFT = -180.0                             # the sled's far edge, on the datum fence. Its width is what keeps it from tipping
MOTOR = (42.3, 40.0)                            # a NEMA 17: square, and long
SPINDLE_D = 12.0                                # a ground rod, flats ground on it for set screws and nothing threaded: see DECISIONS, the pulley's runout
WHEEL_T, PINION_T, BELT = 160, 20, 400.0        # the spindle's wheel, the motor's pulley, and a closed GT2 belt: eight to one
WHEEL_W = 12.0
# Bought parts, at the sizes they are usually sold in: check the ones in hand before drilling for them.
KFL = (16.0, 38.0, 63.0, 48.0)                  # a KFL001 flange bearing unit, 12 bore: thick along the shaft, wide, long, and its two bolts apart
HUB = (36.0, 5.0, 24.0, 14.0, 28.0)             # a 12 mm flanged hub: its flange across and thick, its boss across and long, its bolt circle
SENSOR = (13.0, 24.0, 11.0)                     # an optical endstop: how far it stands off what it is screwed to, long, wide
FLAG_R = 34.0                                   # the home flag, a fin on the wheel's face, this far from the axis
# The blank's socket, printed: a pocket for the blank's end on one side, a boss the spindle's rod is epoxied into on
# the other. The owner's idea. It drives the blank by its flats and says which way round it is.
SOCKET_IN, SOCKET_SLIDE = 12.0, 8.0             # how far the blank stands in the pocket, and how much further it can be pushed to get it onto the tail centre
SOCKET_WALL, SOCKET_FIT, SOCKET_FLOOR = 4.0, 0.3, 6.0
SOCKET_BOSS = (26.0, 22.0)                      # the boss: across, and long, which is how far the rod's roughened end goes into it
NUT_CLEAR = 2.0                                 # a pass ends with the collet nut this short of the socket

# The sled. x along the stick from its tail end, where the bit waits, toward the socket and the drive; y across toward
# the positioner from the stick's axis; z up from the base.
MASTER_R = SPINDLE_D / 2                        # the master: the tail spindle's ground rod, bare between its bearing and the stick. With the sled back the bit is beside it
TAIL_X = ((-95.0, -76.0), (-56.0, -37.0))       # the tailstock's two uprights
TAIL_KFL_X = tuple((x[1], x[1] + KFL[0]) for x in TAIL_X)     # a flange bearing on the stick's side of each
TAIL_ROD_X = (TAIL_X[0][0] - 5.0, 0.0)          # the tail spindle: a rod in those two bearings, its end a three-flat point in the stick's end
MASTER_X = (TAIL_KFL_X[1][1], 0.0)
SOCKET_X = (STICK - SOCKET_IN, STICK + SOCKET_SLIDE)          # the socket's pocket
BOSS_X = (SOCKET_X[1] + SOCKET_FLOOR, SOCKET_X[1] + SOCKET_FLOOR + SOCKET_BOSS[1])
_K = BOSS_X[1] + 4.0
HEAD_X = ((_K + KFL[0], _K + KFL[0] + PLY), (_K + 2 * KFL[0] + PLY + 4.0, _K + 2 * KFL[0] + 2 * PLY + 4.0))   # the headstock's two uprights, past the socket
KFL_X = tuple((x[0] - KFL[0], x[0]) for x in HEAD_X)          # a flange bearing on the stick's side of each
WHEEL_X = (HEAD_X[1][1] + SENSOR[0] + 2.0, HEAD_X[1][1] + SENSOR[0] + 2.0 + WHEEL_W)   # the wheel, outboard, past the home sensor
HUB_X = (WHEEL_X[1], WHEEL_X[1] + HUB[1] + HUB[3])            # its hub, on its outer face
ROD_X = (BOSS_X[0], HUB_X[1] + 1.0)             # the spindle, from the bottom of the socket's boss to past the hub
STOCK_HALF = 25.0                               # the uprights are this wide each side of the axis: a flange bearing's width and a margin, inside the near fence's lip
STOCK_ABOVE = 40.0                              # and this high over it: a flange bearing's upper bolt, and the home sensor
PLATE_X = (TAIL_X[0][0] - 2.0, WHEEL_X[0] - 2.0)   # the sled's plate: the wheel hangs off its far end
BACK_X = -12.0                                  # the bit along the sled with the sled back: beside the master, 9 mm of the straight bit clear of the stick's end
DRAWN_X = 100.0                                 # where the bit is drawn: mid stick, mid pass
FACE_CLEAR = 2.5                                # the uncut blank turns this clear of the positioner's face
BED_CLEAR = 2.5                                 # and of the sled's plate
PLINTH = 0.0                                    # the positioner lies on the base itself
SAFE_CLEAR = 2.0                                # drawn back to the safe depth, the bit's tip is this clear of the master whatever its height
MOVE_CLEAR = 1.5                                # a move may go straight to its row if the bit passes the master by this much all the way
BIT_MARGIN = 2.0                                # a bit may stand this much further out of the collet, or less far, than the table takes it to
STOP = (40.0, 40.0)                             # a stop post on the base at each end of the sled's travel: along, across
SWITCH = (28.0, 16.0, 10.0)                     # the sled-back switch, on the back stop
ROUGH_R = P.ROUGH_BIT / 2
WHEEL_D = WHEEL_T * 2.0 / math.pi               # GT2: 2 mm a tooth
WHEEL_RIM = WHEEL_D / 2 + 1.4                   # its flanges
PINION_D = PINION_T * 2.0 / math.pi


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
    za = bit_home_z + z_travel / 2                               # the stick's axis: level with the middle of the height's travel
    # The sled is a box: a bottom that slides between the fences, two rails on edge, and a deck that the uprights
    # stand on. The positioner's bit is high over the base, and a box brings the deck up to it where a single
    # plate would want uprights twice as tall.
    plate_top = SLIP + PLY
    sled_top = za - AXIS_OVER
    if sled_top - PLY - plate_top < 20.0:
        raise SystemExit(f"the stick's axis, {za:.0f} over the base, is too low for a box under the deck")
    if za - sled_top - blank < BED_CLEAR:
        raise SystemExit(f"the blank turns into the sled's plate: {za - sled_top - blank:.1f}")
    deep = cands + passes
    low_tip = min(q["tip"] for q in deep)
    tip_home = x_travel - BIT_MARGIN + low_tip                   # with the quill at home: so the deepest cut leaves the margin of the travel
    drawn_axes = P.Axes(tip_home, -z_travel / 2)
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
    print(f"the positioner's face {face:.0f} from the stick's axis, the blank's corners {face - blank:.1f} clear of it and {nut_face_home - (x_travel - BIT_MARGIN) - blank:.1f} of the collet nut at the deepest cut; the axis {za:.1f} over the base, {za - sled_top - blank:.1f} clear of the sled's plate; "
          f"the bit {bit_out:.1f} out of the collet nut, its tip {tip_home:.1f} from the axis with the quill at home")

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
    rows = []
    for n, c in enumerate(cuts):
        depth, height, turn = axes.row(c[2], c[3], c[4])
        rows.append({"n": n + 1, "kind": c[0], "tool": c[1], "t": c[2], "across": c[3], "tip": c[4], "label": c[5], "depth": round(depth, 4), "height": round(height, 4), "turn": round(turn, 4)})
    back = max(abs(axes.cutter(r["depth"], r["height"], r["turn"])[0] - r["across"]) + abs(axes.cutter(r["depth"], r["height"], r["turn"])[1] - r["tip"]) for r in rows)
    if back > 1e-3:
        raise SystemExit(f"the rows do not give the cuts back: {back}")
    cutting = [r for r in rows if r["kind"] in ("rough", "finish")]
    depths, heights = (min(r["depth"] for r in rows), max(r["depth"] for r in rows)), (min(r["height"] for r in rows), max(r["height"] for r in rows))
    if depths[0] < FW.PAST_MM + BIT_MARGIN or depths[1] > x_travel - BIT_MARGIN + 1e-6 or heights[0] < FW.PROBE_HEIGHT_MM + 2.0 or heights[1] > z_travel - FW.PROBE_HEIGHT_MM - 2.0:
        raise SystemExit(f"the rows do not fit the positioner's travel: depth {depths}, height {heights}")
    print(f"depth {depths[0]:.1f} to {depths[1]:.1f} of the positioner's {x_travel:.0f}, height {heights[0]:.1f} to {heights[1]:.1f} of its {z_travel:.0f}; drawn back to {safe:.1f} the bit clears the master")

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
    with open(table, "w") as f:
        f.write(f"#safe,{round(safe, 4)}\n")
        f.write("n,kind,tool,depth,height,turn,via,label\n")
        for r in rows:
            f.write(f"{r['n']},{r['kind']},{r['tool']},{r['depth']},{r['height']},{r['turn']},{r['via']},{r['label']}\n")
    steps = os.path.join(OUT, "steps.json")
    subprocess.run([sys.executable, os.path.join(HERE, "firmware", "main.py"), "--sim", table, steps], check=True, cwd=os.path.join(HERE, "firmware"))
    sim = json.load(open(steps))
    # and again as if the stick had been left half way: homed, the bit set again, and taken up from the row after
    half = len(rows) // 2
    again = os.path.join(OUT, "steps_again.json")
    subprocess.run([sys.executable, os.path.join(HERE, "firmware", "main.py"), "--sim", table, again, str(half)], check=True, cwd=os.path.join(HERE, "firmware"))
    sim["resumed"] = json.load(open(again))
    os.remove(again)
    if sim["resumed"]["rows"] != sim["rows"][half:]:
        raise SystemExit("taken up half way, the controller does not end the rows where it does run through")
    with open(steps, "w") as f:
        json.dump(sim, f)
    # -- homing the spindle ------------------------------------------------------
    # The home fin is 6 mm wide where the sensor reads it, so the sensor says home over ten degrees. Wherever the
    # spindle is left, in the fin or out of it, the controller's own homing has to end on the same edge of it.
    import main as fw
    fin_deg = math.degrees(6.0 / FLAG_R)
    at = {"steps": 0, "way": 1}

    def pin_set(n, v):
        if n == FW.TURN_PINS[1]:
            at["way"] = 1 if bool(v) != FW.TURN_FLIP else -1
        elif n == FW.TURN_PINS[0] and v:
            at["steps"] += at["way"]
    keep = fw.PINS.set
    fw.PINS.set, fw.HOME = pin_set, (lambda n: (at["steps"] / FW.TURN_STEPS_PER_DEG) % 360.0 < fin_deg)
    homed = []
    for start in (-200.0, -3.0, 0.0, fin_deg / 2, fin_deg - 0.05, fin_deg + 0.05, 30.0, 180.0, 355.0):
        at["steps"] = round(start * FW.TURN_STEPS_PER_DEG)
        fw.Axis("turn", FW.TURN_PINS, FW.TURN_STEPS_PER_DEG, FW.TURN_FLIP, 0.0).seek(FW.TURN_HOME_PIN, 400.0, FW.TURN_CLEAR_DEG)
        homed.append([start, (at["steps"] / FW.TURN_STEPS_PER_DEG) % 360.0])
    fw.PINS.set, fw.HOME = keep, None
    if max(h[1] for h in homed) - min(h[1] for h in homed) > 1.5 / FW.TURN_STEPS_PER_DEG:
        raise SystemExit(f"homing the spindle ends in different places: {homed}")
    print(f"homing the spindle from {len(homed)} places, in the fin and out of it, ends within a step of {homed[0][1]:.2f} deg: the fin's edge at {fin_deg:.2f}")

    # -- the two tables that set a built machine up ------------------------------
    probing = K.probe_rows(drawn_axes, MASTER_R, (FW.PROBE_DEPTH_MM, FW.PROBE_HEIGHT_MM), (x_travel, z_travel))
    K.write_table(os.path.join(OUT, "probe.csv"), probing, safe)
    bands = K.band_rows(axes, setting)
    if max(r["depth"] for r in bands) > x_travel - BIT_MARGIN or max(r["height"] for r in bands) > z_travel - 2.0:
        raise SystemExit("the bands do not fit the positioner's travel")
    K.write_table(os.path.join(OUT, "bands.csv"), bands, safe)
    banded = os.path.join(OUT, "steps_bands.json")
    subprocess.run([sys.executable, os.path.join(HERE, "firmware", "main.py"), "--sim", os.path.join(OUT, "bands.csv"), banded], check=True, cwd=os.path.join(HERE, "firmware"))
    sim["bands"] = json.load(open(banded))["rows"]
    os.remove(banded)
    with open(steps, "w") as f:
        json.dump(sim, f)
    print(f"probing: {sum(1 for r in probing if r['kind'] in ('in', 'up', 'dn'))} touches in {len(probing)} rows; bands: {len(bands)} rows")
    print(f"{len(rows)} rows; the controller, simulated, ends at {sim['rows'][-1][1:]} steps and never changes height with the bit further out than step {sim['deepest_crossing']} (safe {sim['safe_steps']})")

    # -- where things are ---------------------------------------------------------
    drawn = next(i for i, r in enumerate(rows) if r["kind"] == "finish" and abs(r["across"]) < 2.0 and r["tip"] > 12.0 and r["label"].startswith("finish bite 1 of 1"))
    row = rows[drawn]
    psi = -row["t"]                                  # the section in the machine: d toward the router, e up
    section = tf(((0.0, za), psi), outline)
    centres = belt_centres(BELT, WHEEL_D, PINION_D)
    motor_z, motor_y = za, -centres                  # the spindle's motor: level with the spindle, out to the left, on a bracket on the deck
    plate_y = (PLATE_LEFT, face - FENCE_R - SPRING_GAP)        # from the datum fence to the near one, which stands against the positioner's face
    deck_y = (plate_y[0] + BOX_IN, plate_y[1] - LIP[2] - 2.0)  # the box and its deck: inside both lips
    lip_top = plate_top + LIP[3] + LIP[0]
    if motor_y - MOTOR[0] / 2 - 4.0 < deck_y[0]:
        raise SystemExit("the motor's bracket is off the deck")
    if STOCK_HALF > plate_y[1] - LIP[2] - 2.0:
        raise SystemExit("an upright is under the near fence's lip")
    stock_top = za + STOCK_ABOVE
    # The belt on the motor's pulley: teeth in mesh. Six is the least a toothed belt is usually given.
    mesh = PINION_T * (math.pi - 2 * math.asin((WHEEL_D - PINION_D) / 2 / centres)) / (2 * math.pi)
    if mesh < 6.0:
        raise SystemExit(f"only {mesh:.1f} of the pulley's teeth are in the belt")
    # With the sled back the bit is beside the tail spindle's bare rod, between its inner flange bearing and the stick.
    if BACK_X - ROUGH_R < MASTER_X[0] + 5.0 or BACK_X + ROUGH_R > MASTER_X[1] - 2.0:
        raise SystemExit("the bit is not beside the bare rod with the sled back")
    # The collet nut is beside that bearing there, and nearest the axis at the deepest cut.
    if nut_face_home - (x_travel - BIT_MARGIN) - KFL[1] / 2 < 5.0:
        raise SystemExit("the collet nut meets the tail's inner flange bearing")
    # The socket, turning: its corners clear the sled's plate and the positioner's face. A pass ends with the collet
    # nut short of it, so the last of the stick is not cut.
    blank_m = tf(((0.0, za), psi), blank_of["corners"])
    socket_out, socket_in = grown(blank_m, SOCKET_FIT + SOCKET_WALL), grown(blank_m, SOCKET_FIT)
    socket_swing = max(math.hypot(y, z - za) for y, z in socket_out)
    if za - sled_top - socket_swing < BED_CLEAR or face - socket_swing < 5.0:
        raise SystemExit(f"the socket, swinging {socket_swing:.1f}, meets the plate or the positioner")
    forward_x = STICK - SOCKET_IN - pos["router"]["collet"][0] / 2 - NUT_CLEAR
    cut_to = forward_x + ROUGH_R                     # how much of the stick the bit reaches
    half_box = pos["box"]["y"]
    if WHEEL_X[0] - (forward_x + half_box) < 5.0 or FLAG_R + SENSOR[2] / 2 > STOCK_ABOVE:
        raise SystemExit("the wheel meets the positioner's box with the sled forward, or the home sensor is off its upright")
    # The base, fixed, in the sled's frame where the sled is drawn: the bit at DRAWN_X along it.
    base = lambda u: u + DRAWN_X                     # from the base's own frame, the bit at 0
    travel = forward_x - BACK_X
    stops_x = (PLATE_X[0] - forward_x, PLATE_X[1] - BACK_X)      # the plate's two ends at the ends of the travel, in the base's frame
    base_x = (stops_x[0] - STOP[0] - 15.0, stops_x[1] + STOP[0] + 15.0)
    base_y = (plate_y[0] - FENCE_L - 15.0, face + (box_x[1] - box_x[0]) + 10.0)
    fence_x = (stops_x[0] - STOP[0], stops_x[1] + STOP[0])       # the fences, end to end: the sled's whole travel and both stops
    if za - WHEEL_RIM < lip_top + 10.0:
        raise SystemExit("the wheel meets a fence's lip")
    if STOCK_HALF + FACE_CLEAR > face:
        raise SystemExit("an upright meets the positioner's face")
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
        # On the bottom: a rail on edge along each side, and the deck across them.
        for y in ((deck_y[0], deck_y[0] + PLY), (deck_y[1] - PLY, deck_y[1])):
            box(p, "rail", PLATE_X, y, (plate_top, sled_top - PLY))
        box(p, "deck", PLATE_X, deck_y, (sled_top - PLY, sled_top))

    def plungers(p):
        # Spring plungers in the bottom's near edge, their noses on the near fence: they keep the far edge on the datum.
        for x in PLUNGER[1]:
            ycyl(p, "spring plunger's nose", (plate_y[1], plate_y[1] + SPRING_GAP), (x, plate_top - PLY / 2), PLUNGER[0])

    def stock(p, uprights):
        # Two uprights tied at the foot. They only carry the flange bearings: the rod passes them in a slack hole.
        for x in uprights:
            box(p, "upright", x, (-STOCK_HALF, STOCK_HALF), (sled_top, stock_top))
            xcyl(p, "the rod's way", (x[0] - 1.0, x[1] + 1.0), (0.0, za), SPINDLE_D + 8.0, op="remove")
        box(p, "tie", (uprights[0][1], uprights[1][0]), (-STOCK_HALF, STOCK_HALF), (sled_top, sled_top + 10.0), op="add")

    def flange_bearings(p, where):
        # Bolted to the uprights' faces, long way up, each on the stick's side of its upright. Their set screws hold
        # the rod endways.
        for x in where:
            box(p, "KFL001", x, (-KFL[1] / 2, KFL[1] / 2), (za - KFL[2] / 2, za + KFL[2] / 2))
            xcyl(p, "bore", (x[0] - 0.5, x[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

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

    def wheel(p):
        xcyl(p, f"{WHEEL_T}-tooth wheel", WHEEL_X, (0.0, za), 2 * WHEEL_RIM)
        xcyl(p, "bore, clear of the rod", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (0.0, za), SPINDLE_D + 1.0, op="remove")
        # The home flag: a fin on the face toward the headstock, in the sensor's slot at turn 0 and drawn where this row has it.
        a = math.radians(90.0 + row["turn"])
        cs, sn = math.cos(a), math.sin(a)
        fin = p.sketch("right", WHEEL_X[0] - 7.0, "home flag")
        p.polygon(fin, [(r * cs - t * sn, za + r * sn + t * cs) for r, t in ((FLAG_R - 0.75, -3.0), (FLAG_R + 0.75, -3.0), (FLAG_R + 0.75, 3.0), (FLAG_R - 0.75, 3.0))])
        p.extrude(fin, 7.0, op="add", name="home flag")

    def hub(p):
        # A flanged hub on the rod, its set screws on a flat. The wheel is bolted to its flange through slack holes,
        # so it can be tapped true under an indicator before the bolts are tightened.
        xcyl(p, "hub's flange", (HUB_X[0], HUB_X[0] + HUB[1]), (0.0, za), HUB[0])
        xcyl(p, "hub's boss", (HUB_X[0] + HUB[1], HUB_X[1]), (0.0, za), HUB[2], op="add")
        xcyl(p, "bore", (HUB_X[0] - 0.5, HUB_X[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def home_sensor(p):
        # On the outer upright's outer face, over the axis: the wheel's fin passes through its slot once a turn.
        x, z = (HEAD_X[1][1], HEAD_X[1][1] + SENSOR[0]), za + FLAG_R
        box(p, "optical endstop", x, (-SENSOR[1] / 2, SENSOR[1] / 2), (z - SENSOR[2] / 2, z + SENSOR[2] / 2))
        box(p, "its slot", (x[1] - 8.0, x[1] + 0.5), (-SENSOR[1] / 2 - 0.5, SENSOR[1] / 2 + 0.5), (z - 1.6, z + 1.6), op="remove")

    def drive_motor(p):
        m = MOTOR[0] / 2
        x0 = WHEEL_X[0] - 6.0                           # the motor's face: its shaft out through the bracket to the pulley, in the wheel's plane
        box(p, "bracket", (x0, x0 + 4.0), (motor_y - m - 4.0, motor_y + m + 4.0), (sled_top, motor_z + m + 4.0))
        box(p, "bracket's foot", (x0 - 40.0, x0 + 4.0), (motor_y - m - 4.0, motor_y + m + 4.0), (sled_top, sled_top + 6.0), op="add")
        xcyl(p, "motor's pilot", (x0 - 1.0, x0 + 5.0), (motor_y, motor_z), 22.5, op="remove")
        box(p, "spindle motor", (x0 - MOTOR[1], x0), (motor_y - m, motor_y + m), (motor_z - m, motor_z + m))
        xcyl(p, f"{PINION_T}-tooth pulley", WHEEL_X, (motor_y, motor_z), PINION_D + 3.0)
        xcyl(p, "bore", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (motor_y, motor_z), 5.2, op="remove")
        xcyl(p, "motor shaft", (x0, WHEEL_X[1]), (motor_y, motor_z), 5.0)       # last: a bore cut after it would take it too

    def stick(p):
        # Milled as far as the bit reaches; the rest, into the socket, is still the blank.
        slab(p, "stick, milled", (0.0, cut_to), section)
        slab(p, "the end the bit does not reach", (cut_to, STICK), blank_m, op="add")

    def stops(p):
        """Blocks on the base, in the sled's way at each end of its travel: the plate's ends meet them."""
        y = (plate_y[0], plate_y[0] + STOP[1])
        box(p, "forward stop", (base(stops_x[0] - STOP[0]), base(stops_x[0])), y, (0.0, plate_top))
        box(p, "back stop", (base(stops_x[1]), base(stops_x[1] + STOP[0])), y, (0.0, plate_top), op="new")

    def back_switch(p):
        # on the back stop's side, its end level with the stop's face: the plate's end closes it
        box(p, "sled-back switch", (base(stops_x[1]), base(stops_x[1]) + SWITCH[0]), (plate_y[0] + STOP[1], plate_y[0] + STOP[1] + SWITCH[2]), (plate_top - SWITCH[1], plate_top))

    def controller(p):
        # on the base, where nothing passes: beside the positioner, past the near fence
        box(p, "controller box", (base(base_x[1] - 95.0), base(base_x[1] - 5.0)), (face + 6.0, face + 76.0), (0.0, 40.0))

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
        m = drive["motor"][0] / 2
        box(p, "X motor", (base(-m), base(m)), (ycol[1], ycol[1] + drive["motor"][1]), (bit_z - m, bit_z + m))
        zc = y0 + pos["stage"]["x"] / 2 - box_x[0]
        ztop = PLINTH + lid + wall
        box(p, "Z column", (base(-col), base(col)), (zc - col, zc + col), (ztop, ztop + drive["length"] + drive["cap"]))
        box(p, "Z motor", (base(-m), base(m)), (zc - m, zc + m), (ztop + drive["length"] + drive["cap"], ztop - pos["z"]["end"] - wall))

    def bit(p):
        tool = P.BIT if row["tool"] == "ball" else P.ROUGH_BIT
        ycyl(p, "bit", (row["tip"], row["tip"] + bit_out), (base(0.0), bit_z), tool)
        # the collet nut, and behind it the spindle and the router's nose, as far as the quill's front
        y = row["tip"] + bit_out
        for k, (name, key) in enumerate((("collet nut", "collet"), ("spindle", "shaft"), ("router's nose", "nose"))):
            d, long = pos["router"][key]
            ycyl(p, name, (y, y + long), (base(0.0), bit_z), d, op="add" if k else "new")
            y += long

    parts = [("Base", base_board), ("Fences", fences), ("Slips", slips), ("Sled plate", plate), ("Sled box", sled_box), ("Spring plungers", plungers), ("Tailstock", lambda p: stock(p, TAIL_X)),
             ("Tail bearings KFL001", lambda p: flange_bearings(p, TAIL_KFL_X)), ("Tail spindle", tail_spindle), ("Headstock", lambda p: stock(p, HEAD_X)),
             ("Flange bearings KFL001", lambda p: flange_bearings(p, KFL_X)), ("Spindle", spindle), ("Blank socket", socket), ("Wheel", wheel), ("Wheel hub", hub),
             ("Turn home sensor", home_sensor), ("Spindle drive", drive_motor), ("Stick", stick),
             ("Stops", stops), ("Sled-back switch", back_switch), ("Controller", controller), ("Router positioner", positioner), ("Bit", bit)]

    mcp.call("create_document", {"name": "sled"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Stick sled for the leaning half-ellipse Spectre"}, {"type": "rename_tab", "tab": 1, "name": parts[0][0]}]})
    tabs, bodies = {}, {}
    for i, (title, make) in enumerate(parts):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
    asm = new_tab(mcp, {"type": "add_assembly", "name": "Sled"})
    zero = {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}
    ops = [{"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True, "placement": zero}}
           for t, _ in parts for k in range(len(bodies[t]))]
    text = mcp.call("apply", {"ops": ops})
    if "ERROR:" in text:
        raise SystemExit(text)
    top_z = PLINTH + lid + wall
    mcp.call("screenshot", {"tab": asm, "view": "-0.55,-0.7,0.5", "width": 1700, "height": 1100, "path": os.path.join(OUT, "sled.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{DRAWN_X}", "fit": f"{DRAWN_X},{base_y[0] - 10},-30,{DRAWN_X + 1},{face + 200},{top_z * 0.62}", "width": 1600, "height": 1000, "path": os.path.join(OUT, "end.png")})
    # the stick's own chain, cut along its axis: the drive end from the socket to the hub, and the tail spindle where the bit waits
    for name, x in (("drive.png", (cut_to - 14.0, ROD_X[1] + 8.0)), ("tail.png", (TAIL_ROD_X[0] - 8.0, 14.0))):
        mcp.call("screenshot", {"tab": asm, "view": "0,-1,0", "section": "y:0", "fit": f"{x[0]},-1,{sled_top - PLY - 4.0},{x[1]},1,{stock_top + 8.0}", "width": 1500, "height": 900, "path": os.path.join(OUT, name)})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "sled.pdf"),
                              "note": f"stick sled: the stick turns on a sled between two fences, the router positioner beside it sets depth and height; {len(rows)} rows a stick"}))
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
        "axes": {"tip_home": tip_home, "across_home": axes.across_home, "scale": list(axes.scale), "shear": axes.shear, "travel": [x_travel, z_travel], "safe": safe, "safe_clear": SAFE_CLEAR,
                 "move_clear": MOVE_CLEAR, "bit_margin": BIT_MARGIN, "bit_out": bit_out, "past": FW.PAST_MM, "turn_past": FW.TURN_PAST_DEG, "nearest_master": nearest, "probe": [FW.PROBE_DEPTH_MM, FW.PROBE_HEIGHT_MM]},
        "straight": straight,
        "sled": {"za": za, "face": face, "face_clear": FACE_CLEAR, "bed_clear": BED_CLEAR, "top": sled_top, "plate_top": plate_top, "axis_over": AXIS_OVER, "deck_y": deck_y, "plate_x": PLATE_X, "plate_y": plate_y, "stock_half": STOCK_HALF, "stock_top": stock_top,
                 "head_x": HEAD_X, "tail_x": TAIL_X, "tail_kfl_x": TAIL_KFL_X, "tail_rod_x": TAIL_ROD_X, "master_x": MASTER_X, "master_r": MASTER_R, "wheel_x": WHEEL_X, "wheel_rim": WHEEL_RIM, "wheel": [WHEEL_T, PINION_T, BELT, centres], "mesh": mesh,
                 "kfl": KFL, "kfl_x": KFL_X, "hub": HUB, "hub_x": HUB_X, "rod_x": ROD_X, "flag_r": FLAG_R,
                 "socket": {"in": SOCKET_IN, "slide": SOCKET_SLIDE, "wall": SOCKET_WALL, "fit": SOCKET_FIT, "floor": SOCKET_FLOOR, "boss": SOCKET_BOSS, "x": SOCKET_X, "boss_x": BOSS_X,
                            "swing": socket_swing, "nut_clear": NUT_CLEAR},
                 "forward_x": forward_x, "cut_to": cut_to, "sensor": SENSOR, "stock_above": STOCK_ABOVE,
                 "homing": {"fin_deg": fin_deg, "step_deg": 1.0 / FW.TURN_STEPS_PER_DEG, "clear_deg": FW.TURN_CLEAR_DEG, "ends": homed},
                 "motor_y": motor_y, "motor_z": motor_z, "slip": SLIP, "fence": [FENCE_L, FENCE_R], "fence_x": fence_x, "lip": LIP, "lip_top": lip_top, "spring_gap": SPRING_GAP, "plunger": PLUNGER,
                 "back_x": BACK_X, "travel": travel, "drawn_x": DRAWN_X, "stops_x": stops_x, "stop": STOP, "switch": SWITCH, "base_x": base_x, "base_y": base_y, "ply": PLY, "plinth": PLINTH},
        "positioner": {"half_box": half_box, "box_y": [face, face + box_x[1] - box_x[0]], "top": top_z, "bit_home_z": bit_home_z, "collet": pos["router"]["collet"], "nut_behind": nut_behind,
                       "router_d": pos["router"]["d"], "size": pos["size"],
                       "length_beyond_face": pos["x"]["end"] - box_x[0]},
        "drawn": {"row": row["n"], "x": DRAWN_X, "psi": psi, "bit_z": bit_z},
        "calibration": {"drawn": {"tip_home": tip_home, "across_home": -z_travel / 2, "master_r": MASTER_R}, "rows": probing, "setting": setting, "built": bool(built),
                        "pin_d": K.PIN_D, "over": K.OVER},
        "bands": {"r": K.BAND_R, "span": K.BAND_SPAN, "rows": bands},
        "count": count, "rows": rows,
        "bodies": {k: [list(v) for v in b_] for k, b_ in bodies.items()},
    }
    with open(os.path.join(OUT, "sled.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"base {base_x[1] - base_x[0]:.0f} x {base_y[1] - base_y[0]:.0f}; the sled's plate {PLATE_X[1] - PLATE_X[0]:.0f} x {plate_y[1] - plate_y[0]:.0f}, {travel:.0f} of travel; the wheel {2 * WHEEL_RIM:.0f} across, its motor {centres:.1f} from the spindle; rows: {count}")


if __name__ == "__main__":
    main()
