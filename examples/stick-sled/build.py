#!/usr/bin/env python3
"""A stick sled: the leaning half-ellipse Spectre milled round one solid stick.

The stick is held between a spindle and a tail centre on a sled that
slides on two shafts; it only turns. Beside it, fixed to the base, lies
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
row), sled.png, end.png, sled.pdf, passes.csv (the rows the controller
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

# The owner's hardware, as the duplicator's: two 20 mm shafts, their supports and their bearing blocks. In the hinged
# sled they were hinges; here they are what they were made for, the sled's rails.
PLY = 19.0
SHAFT_D = 20.0
SHAFTS = (490.0, 416.0)                         # the two shafts: the longer under the stick, the shorter out to the left
SK_W, SK_T, SK_H, SK_HTOT = 60.0, 20.0, 51.0, 70.0   # SK20: base width, thickness along the shaft, base to centre, height
BLK_W, BLK_L, BLK_H, BLK_C = 50.0, 45.0, 42.0, 25.0  # SC20UU: width across, length along the shaft, height, base to centre
MOTOR = (42.3, 40.0)                            # a NEMA 17: square, and long
SPINDLE_D = 12.0                                # a ground rod, M12 cut on its ends: see DECISIONS, the pulley's runout
BEARING_D, BEARING_W = 32.0, 10.0               # 6201
WHEEL_T, PINION_T, BELT = 160, 16, 400.0        # the spindle's wheel, the motor's pulley, and a closed GT2 belt: ten to one
WHEEL_W = 12.0

# The sled. x along the stick from its spindle end, y across toward the positioner from the stick's axis, z up from the base.
MASTER_R = 8.0                                  # the master: a ground sleeve on the spindle between the headstock and the spur. With the sled back the bit is beside it
SPUR_R = 7.0
HEAD_X = ((-95.0, -76.0), (-52.0, -33.0))       # the headstock's two uprights, a bearing in each
MASTER_X = (HEAD_X[1][1], -4.0)
WHEEL_X = (-122.0, -110.0)                      # the wheel, outboard: clear of the positioner's box with the sled back
TAIL_X = (232.0, 251.0)                         # the tailstock's upright
STOCK_HALF = 30.0                               # the uprights are this wide each side of the axis: they pass the positioner's face
PLATE_X = (-97.0, 262.0)                        # the sled's plate: the wheel hangs off its head end
RAIL_Y = (0.0, -150.0)                          # the two shafts: under the stick, and out to the left
BLOCKS_X = ((-8.0, 172.0), (30.0, 134.0))       # the bearing blocks on each, as far apart as its shaft lets the sled travel
BACK_X = -12.0                                  # the bit along the sled with the sled back: beside the master, 9 mm of the straight bit clear of the stick's end
RUN = 6.0                                       # the bit past the stick's far end
DRAWN_X = 100.0                                 # where the bit is drawn: mid stick, mid pass
FACE_CLEAR = 2.5                                # the uncut blank turns this clear of the positioner's face
BED_CLEAR = 2.5                                 # and of the sled's plate
PLINTH = PLY                                    # what the positioner lies on
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
        raise SystemExit(f"the master, {MASTER_R} radius, does not fit in the tile's {circle:.1f} about the axis")
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
    sled_top = SK_H + (BLK_H - BLK_C) + PLY
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
    cuts.append(("load", "none", load_t, 0.0, blank, label("blank on centres", f"{under:.1f} up from its", f"wide face {toward:.1f}", "in from router")))
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
    motor_y = -centres                               # the spindle's motor: level with the spindle, out to the left
    plate_y = (RAIL_Y[1] - BLK_W / 2 - 5.0, near - FACE_CLEAR + 0.5)      # as far toward the positioner as the uprights need
    stock_top = za + STOCK_HALF
    # The base, fixed, in the sled's frame where the sled is drawn: the bit at DRAWN_X along it.
    base = lambda u: u + DRAWN_X                     # from the base's own frame, the bit at 0
    travel = STICK + RUN - BACK_X
    spans = []
    for shaft, xs in zip(SHAFTS, BLOCKS_X):
        lo, hi = xs[0] - BLK_L / 2 - (STICK + RUN), xs[1] + BLK_L / 2 - BACK_X      # the blocks' ends, in the base's frame, over the sled's travel
        free = shaft - 2 * SK_T
        if hi - lo > free - 4.0:
            raise SystemExit(f"a {shaft:.0f} shaft is too short for blocks {xs[1] - xs[0]:.0f} apart")
        mid = (lo + hi) / 2
        spans.append((mid - shaft / 2, mid + shaft / 2))
    stops_x = (PLATE_X[0] - (STICK + RUN), PLATE_X[1] - BACK_X)  # the plate's two ends at the ends of the travel, in the base's frame
    base_x = (min(spans[0][0], stops_x[0] - STOP[0]) - 15.0, max(spans[0][1], stops_x[1] + STOP[0]) + 15.0)
    base_y = (plate_y[0] - 25.0, face + (box_x[1] - box_x[0]) + 10.0)
    half_box = pos["box"]["y"]
    if BACK_X - half_box - WHEEL_X[1] < 5.0:
        raise SystemExit("the wheel meets the positioner's box with the sled back")
    if za - WHEEL_RIM < SK_HTOT + 3.0:
        raise SystemExit("the wheel meets a shaft support")
    if STOCK_HALF + FACE_CLEAR > face:
        raise SystemExit("an upright meets the positioner's face")
    bit_z = bit_home_z + row["height"]
    col = pos["sleeve"]["outside"] / 2
    drive = pos["drive"]

    def base_board(p):
        box(p, "base", (base(base_x[0]), base(base_x[1])), base_y, (-PLY, 0.0))
        box(p, "plinth under the positioner", (base(-half_box), base(half_box)), (face, face + box_x[1] - box_x[0]), (0.0, PLINTH), op="add")

    def rails(p):
        for (x0, x1), y in zip(spans, RAIL_Y):
            xcyl(p, "20 mm shaft", (base(x0), base(x1)), (y, SK_H), SHAFT_D)

    def supports(p):
        for (x0, x1), y in zip(spans, RAIL_Y):
            for x in (x0, x1 - SK_T):
                box(p, "SK20", (base(x), base(x) + SK_T), (y - SK_W / 2, y + SK_W / 2), (0.0, SK_HTOT))
                xcyl(p, "shaft", (base(x) - 1.0, base(x) + SK_T + 1.0), (y, SK_H), SHAFT_D, op="remove")

    def blocks(p):
        for xs, y in zip(BLOCKS_X, RAIL_Y):
            for x in xs:
                box(p, "SC20UU", (x - BLK_L / 2, x + BLK_L / 2), (y - BLK_W / 2, y + BLK_W / 2), (SK_H - BLK_C, SK_H - BLK_C + BLK_H))
                xcyl(p, "shaft", (x - BLK_L / 2 - 1.0, x + BLK_L / 2 + 1.0), (y, SK_H), SHAFT_D, op="remove")

    def plate(p):
        box(p, "sled plate", PLATE_X, plate_y, (sled_top - PLY, sled_top))

    def headstock(p):
        for x in HEAD_X:
            box(p, "upright", x, (-STOCK_HALF, STOCK_HALF), (sled_top, stock_top))
            xcyl(p, "bearing's seat", (x[0] - 1.0, x[1] + 1.0), (0.0, za), BEARING_D, op="remove")
        box(p, "tie", (HEAD_X[0][1], HEAD_X[1][0]), (-STOCK_HALF, STOCK_HALF), (sled_top, sled_top + 12.0), op="add")

    def bearings(p):
        for x in HEAD_X:
            mid = (x[0] + x[1]) / 2
            xcyl(p, "6201", (mid - BEARING_W / 2, mid + BEARING_W / 2), (0.0, za), BEARING_D)
            xcyl(p, "bore", (mid - BEARING_W / 2 - 0.5, mid + BEARING_W / 2 + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def spindle(p):
        xcyl(p, "12 mm ground rod", (WHEEL_X[0] - 6.0, -4.0), (0.0, za), SPINDLE_D)
        xcyl(p, "drive spur", (-4.0, 0.0), (0.0, za), 2 * SPUR_R, op="add")

    def master(p):
        xcyl(p, "master", MASTER_X, (0.0, za), 2 * MASTER_R)
        xcyl(p, "bore", (MASTER_X[0] - 0.5, MASTER_X[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def wheel(p):
        xcyl(p, f"{WHEEL_T}-tooth wheel", WHEEL_X, (0.0, za), 2 * WHEEL_RIM)
        xcyl(p, "bore", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (0.0, za), SPINDLE_D + 0.2, op="remove")

    def drive_motor(p):
        m = MOTOR[0] / 2
        x0 = WHEEL_X[1] + 6.0                           # the motor's face: its shaft back through the bracket to the pulley, in the wheel's plane
        box(p, "bracket", (x0 - 4.0, x0), (motor_y - m - 4.0, motor_y + m + 4.0), (sled_top, za + m + 4.0))
        box(p, "bracket's foot", (x0 - 4.0, x0 + 40.0), (motor_y - m - 4.0, motor_y + m + 4.0), (sled_top, sled_top + 6.0), op="add")
        xcyl(p, "motor's pilot", (x0 - 5.0, x0 + 1.0), (motor_y, za), 22.5, op="remove")
        box(p, "spindle motor", (x0, x0 + MOTOR[1]), (motor_y - m, motor_y + m), (za - m, za + m))
        xcyl(p, f"{PINION_T}-tooth pulley", WHEEL_X, (motor_y, za), PINION_D + 3.0)
        xcyl(p, "bore", (WHEEL_X[0] - 0.5, WHEEL_X[1] + 0.5), (motor_y, za), 5.2, op="remove")
        xcyl(p, "motor shaft", (WHEEL_X[0], x0), (motor_y, za), 5.0)       # last: a bore cut after it would take it too

    def tailstock(p):
        box(p, "upright", TAIL_X, (-STOCK_HALF, STOCK_HALF), (sled_top, stock_top))
        xcyl(p, "the centre's hole", (TAIL_X[0] - 1.0, TAIL_X[1] + 1.0), (0.0, za), 11.0, op="remove")
        xcyl(p, "tail centre, M10 pointed", (STICK, TAIL_X[1] + 5.0), (0.0, za), 10.0)

    def stick(p):
        slab(p, "stick, milled", (0.0, STICK), section)

    def stops(p):
        """Posts on the base that the plate's ends meet, out at its left where the wheel does not pass."""
        y = (plate_y[0], plate_y[0] + STOP[1])
        box(p, "forward stop", (base(stops_x[0] - STOP[0]), base(stops_x[0])), y, (0.0, sled_top))
        box(p, "back stop", (base(stops_x[1]), base(stops_x[1] + STOP[0])), y, (0.0, sled_top), op="new")

    def back_switch(p):
        # on the back stop's side, its end level with the stop's face: the plate's end closes it
        box(p, "sled-back switch", (base(stops_x[1]), base(stops_x[1]) + SWITCH[0]), (plate_y[0] + STOP[1], plate_y[0] + STOP[1] + SWITCH[2]), (sled_top - SWITCH[1], sled_top))

    def controller(p):
        # on the base, where nothing passes: beyond the right shaft's end, before the positioner
        box(p, "controller box", (base(spans[0][1] + 10.0), base(base_x[1] - 5.0)), (face + 6.0, face + 76.0), (0.0, 40.0))

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

    parts = [("Base", base_board), ("Shafts", rails), ("Shaft supports SK20", supports), ("Bearing blocks SC20UU", blocks), ("Sled plate", plate), ("Headstock", headstock),
             ("Bearings 6201", bearings), ("Spindle", spindle), ("Master", master), ("Wheel", wheel), ("Spindle drive", drive_motor), ("Tailstock", tailstock), ("Stick", stick),
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
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "sled.pdf"),
                              "note": f"stick sled: the stick turns on a sled on two shafts, the router positioner beside it sets depth and height; {len(rows)} rows a stick"}))
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
        "sled": {"za": za, "face": face, "face_clear": FACE_CLEAR, "bed_clear": BED_CLEAR, "top": sled_top, "plate_x": PLATE_X, "plate_y": plate_y, "stock_half": STOCK_HALF, "stock_top": stock_top,
                 "head_x": HEAD_X, "tail_x": TAIL_X, "master_x": MASTER_X, "master_r": MASTER_R, "spur_r": SPUR_R, "wheel_x": WHEEL_X, "wheel_rim": WHEEL_RIM, "wheel": [WHEEL_T, PINION_T, BELT, centres],
                 "motor_y": motor_y, "rail_y": RAIL_Y, "blocks_x": BLOCKS_X, "shafts": SHAFTS, "spans": spans, "sk": [SK_W, SK_T, SK_H, SK_HTOT], "blk": [BLK_W, BLK_L, BLK_H, BLK_C],
                 "back_x": BACK_X, "run": RUN, "travel": travel, "drawn_x": DRAWN_X, "stops_x": stops_x, "stop": STOP, "switch": SWITCH, "base_x": base_x, "base_y": base_y, "ply": PLY, "plinth": PLINTH},
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
