#!/usr/bin/env python3
"""A billet for the stick sled: the tile's pieces milled as a stack.

The stick sled (examples/stick-sled) mills the leaning half-ellipse
Spectre round one solid stick, which is then sawn into tiles: end grain,
one wood, a kerf lost at every tile. This is the owner's way round all
three. Every piece is sawn to one size, 55 x 48 and 3 mm thick, and
drilled in a jig at four places outside the tile. The pieces are
stacked on four brass rods, which thread into a wooden cap at the tail
end and are nutted against a printed plate at the socket end, and the
sled mills that stack in two stages:

1. Everything that does not come near a rod, nor near a tab: a strip
   of wood left from each rod to the tile, so that every piece stays
   joined to the rods that hold it. The roughing goes as deep as it can
   short of them; the finishing passes that would touch them wait.
2. Off the sled, a printed comb goes on: a plate over the cap and
   three fingers that slide along the grooves the first stage cut,
   each bearing on edges already finished. Three more rods go through
   the fingers into inserts in the far plate, the four old rods come
   out, and the rest is milled: the tabs, the corners, the edges they
   shaded.

Then the nuts come off, and the pieces are tiles: face grain, a wood
each if the stack was made so, and nothing to saw.

    cargo build --release -p ok-mcp
    python3 examples/stick-sled/build.py         # the sled's tile and table, in its out/sled.json
    python3 examples/stick-billet/build.py
    python3 examples/stick-billet/animate.py     # the whole job as one captioned animation
    cargo test -p ok-render --test stick_billet  # what CI runs

Writes out/billet.okpart, the printable parts as STL (the far plate, the
comb, the drilling jig's box and lid), pictures, the two stages' pass
tables (out/stage1.csv, out/stage2.csv) and out/billet.json with the
numbers the test checks and the shapes the animation draws. Each stage's
table is one the sled's controller takes as it is, and the build runs
the controller's own program over it; out/dry1.csv and out/dry2.csv are
a short dry run of each, no bit in the collet.
"""
import json
import math
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)
from okmcp import Mcp, Part, new_tab  # noqa: E402
import billet as B  # noqa: E402

P = B.P
OUT = os.path.join(HERE, "out")
SLED_DIR = os.path.join(os.path.dirname(HERE), "stick-sled")
SLED = os.path.join(SLED_DIR, "out", "sled.json")
sys.path.insert(0, os.path.join(SLED_DIR, "firmware"))
import calibrate as K  # noqa: E402  the sled's: it writes a table as the controller reads it
import config as FW  # noqa: E402  the controller's own

# The pieces.
PIECE = 3.0                     # a piece, and so a tile: 3 mm of solid wood, the owner's
HOLE = 3.66                     # a #27 drill, 0.144 in: the rods' clearance through the pieces
NEAR_CAP = 12.0                 # the cap at the tail end: 12 mm Baltic birch plywood the size of a piece. The tail spindle's plain end sits in a hole in it and the four rods thread into it. Plywood, not solid wood: once it is cut down to the tile and four tabs, its tabs carry the whole clamp in bending, and one of them would lie across a solid block's grain
TAP = 2.71                      # a #36 drill, 0.1065 in: the #6-32 tap's
TAP_DEEP = 10.0                 # how far the rods thread into the cap, from its inner face. Thin CA in the threads hardens them
# The far plate, printed: the stack's end against the socket. A boss the size of the sled's own blank goes into the
# sled's socket, so the sled is not changed; the four rods are nutted against the plate in notches in the boss's
# corners; the second stage's three rods thread into inserts in its inner face.
INSERT = (4.7, 6.0)             # a #6-32 heat-set insert's hole: across, and deep
NUT_T = 2.38                    # a small-pattern #6-32 hex nut, 3/32 in thick (1/4 in across its flats: billet.NUT)
NOTCH = B.NUT + 0.6             # the boss is notched this far round each corner rod, for its nut and a nut driver
# The comb, printed in one: a plate over the cap and three fingers along the grooves.
PLATE = 4.0                     # the plate, thick
PLATE_IN = 0.5                  # it lies over the tile this far in from the edge, and no further out: no cut goes inside the tile
PLATE_HOLE = 14.0               # a hole in the middle for the tail spindle's 12 mm rod, on its way to the cap's socket
BAND = 1.0                      # a finger reaches this far over the tile's edge at the plate, where it joins it: twice PLATE_IN, so they overlap
# The drilling jig, printed: a box the pieces lie in, a few at a time, and a lid that drops on them with a guide hole
# over each rod and one over the stick's axis for the 12 mm drill that makes the cap's socket.
JIG_FIT = 0.3                   # the box's pocket over a piece, all round
JIG_WALL, JIG_FLOOR, JIG_DEEP = 5.0, 4.0, 18.0   # its walls and floor, and the pocket's depth: six pieces
JIG_LID = 10.0                  # the lid: a plug that drops into the pocket onto the pieces
GUIDE = HOLE + 0.1              # the lid's guide holes
AXIS_GUIDE = 12.3               # the lid's hole over the stick's axis, a guide for a 12 mm brad-point drill: the cap's socket for the tail spindle
JIG_UNDER = 6.0                 # the box's floor has a hole this wide under each guide, for the drill to come out into

DRY = 10                        # a dry run's passes, spread through its stage's table, besides the four at the ends of the positioner's reach
# The clamp. The rods are at the corners and the tile is in the middle, so the far plate is crowned and the cap has a
# shim: each touches the stack over a small disc about the axis and stands off it everywhere else, and tightening the
# nuts bends the far plate's corners down. That bend is a spring, and its push goes down the middle of the stack, through the tile of every
# piece. The owner's idea: springy cauls.
CROWN = 0.3                     # how far the far plate's face stands off the stack outside the disc: two printed layers of 0.15. At 0.2 the clamp held the heaviest turning 1.9 times over
CROWN_R = 9.5                   # the disc: inside the tile everywhere, whose largest circle about the axis is 9.8
SHIM = 0.1                      # a disc of paper glued on the cap's inner face, as wide: the cap's crown, for the first stage
PRINT_E = 2500.0                # MPa: a printed plate's stiffness in bending, PLA or PETG, at the low end
PRINT_OK = 20.0                 # MPa: what a printed plate may carry for hours without creeping much. A guess
PLY_OK = 30.0                   # MPa: Baltic birch plywood in bending, either way of the sheet, well under what breaks it
FRICTION = 0.3                  # wood on wood, at the low end
HOLD_OVER = 2.0                 # the clamp's friction alone holds a piece against the heaviest cut this many times over, or the build stops
ACROSS_GRAIN = 4.0              # MPa: what a hardwood takes pulled across its grain, at the low end (the Wood Handbook has sugar maple and cherry near 4, walnut 4.8, red oak 5.5)


def main():
    t_start = time.time()
    os.makedirs(OUT, exist_ok=True)
    n = json.load(open(SLED))
    outline = [tuple(q) for q in n["tile"]["outline"]]
    stock = [tuple(q) for q in n["blank"]["corners"]]
    blank = B.grown(stock, B.GROW)
    sled, ax = n["sled"], n["axes"]
    tail_in, tail_hole = sled["tail_in"], sled["tail_hole"]         # the sled's tail spindle: its plain end, this far into a hole this wide
    if tail_in > NEAR_CAP - 3.0 or tail_hole / 2 > CROWN_R - 2.0:
        raise SystemExit("the tail spindle's hole leaves too little of the cap under it, or of the shim round it")
    stick, socket_in = n["stick"], sled["socket"]["in"]
    forward = sled["forward_x"]
    reach = forward + P.BIT / 2                      # the ball-nose finishes the edge this far along the stick and no further
    pieces = int((reach - NEAR_CAP) / PIECE)
    spacer = (NEAR_CAP + pieces * PIECE, NEAR_CAP + (pieces + 1) * PIECE)
    plate_x = (spacer[1], stick - socket_in)
    boss_x = (plate_x[1], stick)
    if plate_x[1] - plate_x[0] < 6.0 or spacer[1] < sled["cut_to"]:
        raise SystemExit(f"the far plate {plate_x} or the spacer {spacer} is not where the cut ends, {sled['cut_to']:.1f}")
    print(f"the stack: a cap of {NEAR_CAP:.0f}, {pieces} pieces of {PIECE:.0f} to {spacer[0]:.0f}, a spacer to {spacer[1]:.0f} which the bit reaches {sled['cut_to'] - spacer[0]:.1f} into, "
          f"the far plate to {plate_x[1]:.0f}, its boss {boss_x[1] - boss_x[0]:.0f} into the socket")

    # -- the plan -------------------------------------------------------------
    def runs(kind):
        """The sled's places or passes, each at its last bite, in its table's order."""
        out = []
        for r in n["rows"]:
            if r["kind"] != kind:
                continue
            if out and (out[-1]["t"], out[-1]["across"]) == (r["t"], r["across"]):
                out[-1]["tip"] = min(out[-1]["tip"], r["tip"])
            else:
                out.append({"t": r["t"], "across": r["across"], "tip": r["tip"]})
        return out
    places, passes = runs("rough"), runs("finish")
    # Nothing but the belt holds the stick while it is cut, and the sled's bites are the deepest its estimate of that
    # passes. The same estimate here, bite by bite, with the sled's own numbers: a bite that would move the tile's
    # edge further than the sled allows is made in more, shallower ones.
    hold, swing_r = n["hold"], n["tile"]["swing"]

    def give(tool, area, over, far):
        edge_speed = math.pi * P.cutting_dia("ball" if tool == "ball" else "straight", over) * hold["rpm"] / 60.0
        push = hold["peak"] * hold["cut_energy"] * 1000.0 * area * hold["feed"] / edge_speed
        return push * far / 1000.0 / hold["stiff"] * swing_r, push

    def fits(tool, area, over, far, last):
        return give(tool, area, over, far)[0] <= hold["limit"][1 if last else 0]
    plan = B.stages(outline, stock, places, passes, fits)
    if not all(q["ok"] for stage in ("first", "second") for part in plan[stage] for q in part):
        raise SystemExit("a cut cannot be made in bites light enough for the belt")
    (p1, f1), (p2, f2) = plan["first"], plan["second"]
    rows1 = [c for q in p1 for c in B.bitten(q, "straight")] + [c for q in f1 for c in B.bitten(q, "ball")]
    rows2 = [c for q in p2 for c in B.bitten(q, "straight")] + [c for q in f2 for c in B.bitten(q, "ball")]
    sled_rows = sum(1 for r in n["rows"] if r["kind"] in ("rough", "finish"))
    print(f"first stage: {len(p1)} roughing places in {sum(q['bites'] for q in p1)} runs, {len(f1)} finishing passes in {sum(q['bites'] for q in f1)}; "
          f"second: {len(p2)} places in {sum(q['bites'] for q in p2)} runs, {len(f2)} passes in {sum(q['bites'] for q in f2)}. {len(rows1) + len(rows2)} runs where the sled's solid stick takes {sled_rows}")
    print(f"waste {plan['waste'].area:.0f} mm2: {plan['left_first']:.0f} left after the first stage, {plan['left_end']:.1f} at the end")
    for r in rows1 + rows2:
        r["give"], r["push"] = give(r["tool"], r["area"], r["over"], r["far"])
        r["face"] = r["tool"] == "ball" and r["bite"] == r["of"]
    worst = {stage: {"bite": max(r["give"] for r in rows if not r["face"]), "face": max(r["give"] for r in rows if r["face"])} for stage, rows in (("first", rows1), ("second", rows2))}
    plain = lambda q, tool: max(1, math.ceil(q["depth"] / (P.BALL_BITE if tool == "ball" else P.ROUGH_BITE) - 1e-9))
    extra = sum(q["bites"] - plain(q, tool) for stage in ("first", "second") for part, tool in zip(plan[stage], ("straight", "ball")) for q in part)
    print(f"the belt holds every bite: the tile's edge moves at most {1000 * worst['first']['bite']:.1f} um in the first stage and {1000 * worst['second']['bite']:.1f} in the second, "
          f"{1000 * max(worst['first']['face'], worst['second']['face']):.1f} in a bite that leaves a face (the sled's limits: {1000 * hold['limit'][0]:.0f} and {1000 * hold['limit'][1]:.0f}); "
          f"{extra} runs are there only to keep a bite that light")

    grid = B.Grid(blank)
    wood = B.wood_after_first(outline, plan, grid)
    held = B.joined(wood, B.cell_of((0.0, 0.0), grid), grid)
    joined = []
    for c in plan["rods"]:
        ring = [B.cell_of((c[0] + (B.ROD / 2 + 0.4) * math.cos(a), c[1] + (B.ROD / 2 + 0.4) * math.sin(a)), grid) for a in [j * math.pi / 8 for j in range(16)]]
        joined.append(any(x in held for x in ring))
    if not all(joined):
        raise SystemExit(f"after the first stage a rod is no longer joined to the tile: {joined}")
    fingers, rooms, cuts2 = B.fingers(outline, plan, grid, wood)
    if len(fingers) != 3:
        raise SystemExit(f"no three fingers round the axis: {len(rooms)} rooms")
    print("the four rods are each joined to the tile by wood after the first stage; the fingers: " +
          ", ".join(f"rod at ({f['rod'][0]:.1f}, {f['rod'][1]:.1f}) bearing on {f['bears']:.0f} mm of edge, {f['area']:.0f} mm2" for f in fingers))

    rods1, rods2 = plan["rods"], [f["rod"] for f in fingers]
    # How near anything comes. A rod to the tile and to every first-stage cut; a finger's rod to its own wall; a nut.
    rod_tile = [B.near_edge(c, outline) - B.ROD / 2 for c in plan["rods"]]
    cuts1 = [dict(q, tool="straight") for q in p1] + [dict(q, tool="ball") for q in f1]

    def gap(pts, cuts):
        """The least distance from a set of points to the bits of `cuts`, found by growing the bits a hundredth at a time."""
        g = 0.0
        while g < 3.0 and not any(B.hits(c, pts, g) for c in cuts):
            g += 0.05
        return g
    rod_cut = min(gap(B.disc(c, B.ROD / 2, rings=1, n=48), cuts1) for c in plan["rods"])
    finger_rod_cut = min(gap(B.disc(f["rod"], B.ROD / 2, rings=1, n=48), cuts2) for f in fingers)
    nut_cut = min(gap(B.disc(f["rod"], B.NUT, rings=1, n=48), cuts2) for f in fingers)
    print(f"the rods stand {min(rod_tile):.1f} or more off the tile and {rod_cut:.2f} or more off any first-stage cut; the fingers' rods {finger_rod_cut:.2f} off any second-stage cut, their nuts {nut_cut:.2f}")

    # The finger's shapes: its room, and a band over the tile's edge where it joins the plate.
    for f in fingers:
        band = set()
        for c in f["cells"]:
            for a in range(-4, 5):
                for b in range(-4, 5):
                    m = (c[0] + a, c[1] + b)
                    q = grid.at(*m)
                    if P.inside(q, outline) and B.near_edge(q, outline) > -BAND:
                        band.add(m)
        loops = B.outlines(f["cells"] | band, grid)
        f["loop"] = max(loops, key=B.area)
        f["room"] = max(B.outlines(f["cells"], grid), key=B.area)
    wood_out = [c for c in held if not P.inside(grid.at(*c), outline)]     # what is joined to the tile: a crumb that is not falls away
    wood_loops = [loop for loop in B.outlines(set(wood_out), grid) if abs(B.area(loop)) > 0.5]

    # The collet nut. Beside the far end of a pass it passes the stack as sawn, now 55 x 48, and the far plate, the same.
    # At the near end, with the bit drawn back beside the master between passes, it is over the comb's plate.
    nut_r = n["positioner"]["collet"][0] / 2
    need = {}
    for stage, rows in (("first", rows1), ("second", rows2)):
        for tool in ("straight", "ball"):
            got = [B.nut_need(blank, r["t"], r["across"], r["tip"], nut_r, n["axes"]["bits"][tool]["nut_air"]) for r in rows if r["tool"] == tool]
            need[f"{stage} {tool}"] = max(g for g in got if g is not None)
    comb_foot = [B.offset(outline, -PLATE_IN)] + [f["loop"] for f in fingers]
    comb_need = 0.0
    for r in rows2:
        for poly in comb_foot:
            g = B.nut_need(poly, r["t"], r["across"], r["tip"], nut_r, n["axes"]["bits"][r["tool"]]["nut_air"])
            if g is not None:
                comb_need = max(comb_need, g)
    bits = {}
    for tool in ("straight", "ball"):
        b = n["axes"]["bits"][tool]
        out = max(need[f"first {tool}"], need[f"second {tool}"], comb_need if tool == "ball" else 0.0)
        bits[tool] = {"name": b["name"], "long": b["long"], "sled_out": b["out"], "range": b["range"], "need": out, "set": round(max(out, b["out"]) + 0.05, 1)}
        if bits[tool]["set"] > b["range"][1] or bits[tool]["set"] - b["out"] > ax["bit_margin"]:
            raise SystemExit(f"the {tool} bit would have to stand {out:.2f} out of the collet nut: {b['name']} sets {b['range']}, the table takes {b['out']:.2f} within {ax['bit_margin']}")
    print("set the bits further out of the collet nut than the sled's table has them, within the 2 mm it allows: " +
          "; ".join(f"{v['name']} {v['set']:.1f} where the sled sets {v['sled_out']:.1f}" for v in bits.values()) + f" (the comb's plate wants {comb_need:.1f})")

    # -- the tables -----------------------------------------------------------
    # Each stage as the controller takes it, like the sled's own passes.csv: the billet put on, the straight bit set
    # on the master, the roughing, a look for the wheel's mark, the ball-nose set, the finishing, a last look.
    axes = P.Axes(ax["tip_home"], ax["across_home"], tuple(ax["scale"]), ax["shear"])
    along = next(r["along"] for r in n["rows"] if r["kind"] == "rough")
    load = next(r for r in n["rows"] if r["kind"] == "load")
    sets = {r["tool"]: r for r in n["rows"] if r["kind"] == "set"}
    swing = max(math.hypot(*q) for q in blank)
    label = lambda *text: "".join(t.ljust(16) for t in text).rstrip()
    master_r, move_clear, past = sled["master_r"], ax["move_clear"], ax["past"]
    # With the sled back the bit waits beside the master, past everything that stands out of the billet's tail end:
    # in the second stage the comb's plate, its nuts and the ends of its rods.
    stands_out = PLATE + NUT_T + 1.0
    bit_waits = -sled["back_x"] - P.ROUGH_BIT / 2
    if bit_waits - stands_out < 1.0:
        raise SystemExit(f"drawn back, the bit is {bit_waits:.1f} from the billet's end and the comb's rods stand {stands_out:.1f} out of it")

    def bit_edge(tool):
        """Points round a bit's outline from its tip: (toward the router, across). The sled's build has the same."""
        if tool == "ball":
            r, pts = P.BIT / 2, []
            for k in range(61):
                off = P.SHANK / 2 * k / 60
                f = P.flank(off, r)
                if f is not None:
                    pts += [(r + f, off), (r + f, -off)]
            far = max(u for u, _ in pts)
            return pts + [(far + k, s * P.SHANK / 2) for s in (-1, 1) for k in range(1, 40)]
        return [(0.0, P.ROUGH_BIT / 2 * k / 10) for k in range(-10, 11)] + [(float(k), s * P.ROUGH_BIT / 2) for s in (-1, 1) for k in range(1, 60)]
    edges = {tool: bit_edge(tool) for tool in ("straight", "ball")}

    def master_gap(depth, height, tool):
        tip, across = axes.tip(depth, height)
        return min(math.hypot(tip + u, across + v) for u, v in edges[tool]) - master_r

    def straight_ok(prev, row):
        """Whether the move to a row may go straight: height first, then depth, the whole bit clear of the master."""
        steps = lambda a0, a1: [a0 + (a1 - a0) * k / max(1, math.ceil(abs(a1 - a0) / 0.25)) for k in range(max(1, math.ceil(abs(a1 - a0) / 0.25)) + 1)]
        low = row["height"] - past if row["height"] < prev["height"] else row["height"]
        if any(master_gap(prev["depth"], h, row["tool"]) < move_clear for h in steps(prev["height"], low)):
            return False
        return all(master_gap(d, row["height"], row["tool"]) >= move_clear for d in steps(prev["depth"], row["depth"]))

    def table(cuts_, path, on):
        seq = [("load", "none", load["t"], 0.0, swing, on)]
        seq.append(("set", "straight", sets["straight"]["t"], sets["straight"]["across"], sets["straight"]["tip"], "set the straight bit on the master"))
        for r in cuts_:
            if r["tool"] == "straight":
                seq.append(("rough", "straight", r["t"], r["across"], r["tip"], f"rough {math.degrees(r['t']):.0f} deg {r['across']:+.0f} bite {r['bite']} of {r['of']}"))
        seq.append(("check", "none", 0.0, None, None, "after roughing"))
        seq.append(("set", "ball", sets["ball"]["t"], sets["ball"]["across"], sets["ball"]["tip"], "set the ball-nose on the master"))
        for r in cuts_:
            if r["tool"] == "ball":
                seq.append(("finish", "ball", r["t"], r["across"], r["tip"], f"finish bite {r['bite']} of {r['of']}"))
        seq.append(("check", "none", 0.0, None, None, "at the end"))
        out = []
        for k, c in enumerate(seq):
            if c[0] == "check":                              # the positioner stays where the row before left it
                c = c[:3] + (out[-1]["across"], out[-1]["tip"]) + c[5:]
            depth, height, turn = axes.row(c[2], c[3], c[4])
            out.append({"n": k + 1, "kind": c[0], "tool": c[1], "t": c[2], "across": c[3], "tip": c[4], "label": c[5], "depth": round(depth, 4), "height": round(height, 4), "turn": round(turn, 4),
                        "along": along if c[0] in ("rough", "finish") else 0.0})
        for i, r in enumerate(out):
            r["via"] = "d" if r["kind"] in ("rough", "finish") and out[i - 1]["kind"] in ("rough", "finish") and straight_ok(out[i - 1], r) else "s"
        depths, heights = (min(r["depth"] for r in out), max(r["depth"] for r in out)), (min(r["height"] for r in out), max(r["height"] for r in out))
        if depths[0] < past + ax["bit_margin"] or depths[1] > ax["travel"][0] - ax["bit_margin"] + 1e-6 or heights[0] < FW.PROBE_HEIGHT_MM + 2.0 or heights[1] > ax["travel"][1] - FW.PROBE_HEIGHT_MM - 2.0:
            raise SystemExit(f"{path}: the rows do not fit the positioner's travel: depth {depths}, height {heights}")
        K.write_table(path, out, ax["safe"])
        # and the controller's own program run over it on a computer
        scratch = os.path.join(OUT, "steps_scratch.json")
        subprocess.run([sys.executable, os.path.join(SLED_DIR, "firmware", "main.py"), "--sim", path, scratch], check=True, cwd=os.path.join(SLED_DIR, "firmware"), stdout=subprocess.DEVNULL)
        sim = json.load(open(scratch))
        os.remove(scratch)
        cutting = [r for r in out if r["kind"] in ("rough", "finish")]
        if sim["passes"] != len(cutting) or any(r[4] for r in sim["rows"]) or len(sim["checks"]) != 2:
            raise SystemExit(f"{path}: the controller made {sim['passes']} passes of the table's {len(cutting)}, or did not look for the wheel's mark twice")
        return out, sim
    (t1, sim1) = table(rows1, os.path.join(OUT, "stage1.csv"), label("billet in socket", "cap's mark on ctr", "marked corner", "to the tail mark"))
    (t2, sim2) = table(rows2, os.path.join(OUT, "stage2.csv"), label("billet + comb on", "back in socket", "the same way up"))
    # A dry run of each stage before it is cut: a sample of its own rows with no bit in the collet, the billet on. What
    # it tries is what is new here: the collet nut passing a blank 3 mm bigger all round, and in the second stage
    # the comb. The sled's own precheck, which tries what holds the stick, is the machine's and is not made again.
    def dry_run(rows, path, on):
        cutting = [r for r in rows if r["kind"] in ("rough", "finish")]
        pick = {round(k * (len(cutting) - 1) / (DRY - 1)) for k in range(DRY)}
        for key in ("depth", "height"):                 # and the passes at the ends of the positioner's reach
            pick |= {min(range(len(cutting)), key=lambda i: cutting[i][key]), max(range(len(cutting)), key=lambda i: cutting[i][key])}
        out = []

        def add(kind, text, like=None):
            like = like or {}
            out.append({"n": len(out) + 1, "kind": kind, "tool": "none", "depth": like.get("depth", 0.0), "height": like.get("height", 0.0), "turn": like.get("turn", 0.0), "via": "s",
                        "along": like.get("along", 0.0), "label": text})
        add("say", K.lines("DRY RUN. router", "OFF and NO BIT", "in the collet.", on))
        for i in sorted(pick):
            add("dry", f"as row {cutting[i]['n']}", cutting[i])
        add("say", K.lines("done. if nothing", "touched: fit the", "bit and run this", "stage"))
        K.write_table(path, out, ax["safe"])
        scratch = os.path.join(OUT, "steps_scratch.json")
        subprocess.run([sys.executable, os.path.join(SLED_DIR, "firmware", "main.py"), "--sim", path, scratch], check=True, cwd=os.path.join(SLED_DIR, "firmware"), stdout=subprocess.DEVNULL)
        sim = json.load(open(scratch))
        os.remove(scratch)
        if sim["passes"] != len(pick) or sim["bits"]:
            raise SystemExit(f"{path}: the controller does not run the dry run as it is written")
        return len(pick), sim["seconds"]
    dry1 = dry_run(t1, os.path.join(OUT, "dry1.csv"), "the billet on")
    dry2 = dry_run(t2, os.path.join(OUT, "dry2.csv"), "billet + comb on")
    print(f"a dry run before each stage, no bit in the collet: {dry1[0]} passes in {dry1[1] / 60:.0f} min, and {dry2[0]} in {dry2[1] / 60:.0f}")
    # -- what holds a piece ------------------------------------------------------------------------------------
    # The clamp first. Tightened until its corners meet the stack, the far plate has bent by its crown and the
    # cap's shim together, and pushes on the middle with what that bend takes.
    span1 = (math.dist(rods1[0], rods1[2]) + math.dist(rods1[1], rods1[3])) / 2
    plate_t, plate_w = plate_x[1] - plate_x[0] - CROWN, 0.6 * min(math.dist(blank[0], blank[1]), math.dist(blank[0], blank[3]))
    soft1 = B.plate_spring(span1, plate_w, plate_t, PRINT_E)
    clamp1 = (CROWN + SHIM) / soft1
    plate_stress1 = clamp1 * span1 / 4 / (plate_w * plate_t ** 2 / 6)
    # The cap pushes back through its own wood: cut down to the tile and four tabs, each tab bends over its length.
    cap_stress = max(clamp1 / 4 * math.dist(c, e) / (B.TAB * NEAR_CAP ** 2 / 6) for c, e in zip(rods1, plan["tab_ends"]))
    # What that holds, by friction on a piece's two faces: a push, and a turning about the axis. Next to an end the
    # push is spread over the crown's disc only, which is the least it can turn against.
    arm = 2.0 / 3.0 * CROWN_R
    push1 = max(r["push"] for r in rows1)
    turn1 = max(r["push"] * r["far"] for r in rows1) / 1000.0
    hold_push, hold_turn = 2 * FRICTION * clamp1, 2 * FRICTION * clamp1 * arm / 1000.0
    print(f"the clamp: the far plate, {plate_t:.1f} thick over {span1:.0f} between rods, bends {1000 * soft1:.2f} um a newton; its corners down by its crown and the shim, {CROWN + SHIM:.1f}, "
          f"it pushes {clamp1:.0f} N down the middle ({plate_stress1:.0f} MPa in the plate, {cap_stress:.1f} in the cap's tabs). At {FRICTION} that holds {hold_push:.0f} N and {hold_turn:.2f} N m; "
          f"the first stage's heaviest cut is {push1:.1f} N and {turn1:.2f} N m: {hold_push / push1:.0f} and {hold_turn / turn1:.1f} times over")
    if hold_turn / turn1 < HOLD_OVER or hold_push / push1 < HOLD_OVER or plate_stress1 > PRINT_OK or cap_stress > PLY_OK:
        raise SystemExit("the clamp does not hold a piece with room to spare, or asks too much of the far plate or the cap")
    # The tabs second, as if there were no clamp at all: the four of one piece together, the tile rigid, each tab
    # pinned on its rod, and the whole of each cut's push on that one piece.
    loads = []
    for r in rows1:
        if r["area"] <= 0.0:
            continue
        d, e = P.frame(r["t"])
        q = (r["tip"] * d[0] + r["across"] * e[0], r["tip"] * d[1] + r["across"] * e[1])
        ln = math.hypot(*q) or 1.0
        loads.append(((q[0] / ln * r["far"], q[1] / ln * r["far"]), r["push"]))
    tab_stress, tab_row = B.tab_frame(rods1, plan["tab_ends"], B.TAB, PIECE, loads)
    slop = (HOLE - B.ROD) / 2
    print(f"the tabs alone, no clamp: four of {B.TAB:.0f} x {PIECE:.0f} mm on their rods carry the heaviest cut at {tab_stress:.2f} MPa, {ACROSS_GRAIN / tab_stress:.1f} times under the {ACROSS_GRAIN:.0f} MPa "
          f"a hardwood takes across its grain. But a piece held only so can shift {slop:.2f} mm in its holes: the clamp is what holds it, the tabs are what is behind the clamp")
    if ACROSS_GRAIN / tab_stress < 1.5:
        raise SystemExit("the tabs are too weak to stand behind the clamp")
    # The second stage's clamp: the far plate again, now held at three inserts, bent by its crown, and the comb's
    # plate, which is flat and bends with it. A crown on the comb's plate too was tried: at 4 mm, which is all the room
    # there is beside the waiting bit, it put 26 MPa in the print. The fingers hold a piece sideways whatever the
    # clamp does; this only keeps the stack shut.
    span2 = sum(math.dist(rods2[i], rods2[(i + 1) % 3]) for i in range(3)) / 3
    comb_w = 2 * CROWN_R + 6.0
    soft2 = B.plate_spring(span2, plate_w, plate_t, PRINT_E) + B.plate_spring(span2, comb_w, PLATE, PRINT_E)
    clamp2 = CROWN / soft2
    comb_stress = clamp2 * span2 / 4 / (comb_w * PLATE ** 2 / 6)
    print(f"the second stage's clamp: the far plate's crown, {CROWN:.1f}, against the comb's flat plate pushes {clamp2:.0f} N down the middle, {comb_stress:.0f} MPa in the comb's plate")
    if comb_stress > PRINT_OK or clamp2 < 20.0:
        raise SystemExit("the comb's plate is asked too much, or does not shut the stack")
    # The jack screws: where the sled's socket has them, they land on the boss's flat end, clear of the nuts' notches.
    ju = ((stock[1][0] - stock[0][0]) / math.dist(stock[0], stock[1]), (stock[1][1] - stock[0][1]) / math.dist(stock[0], stock[1]))
    if math.dist(stock[0], stock[1]) < math.dist(stock[0], stock[3]):
        ju = (-ju[1], ju[0])
    jack = sled["socket"]["jack"]
    jacks = [(s_ * jack[0] * ju[0], s_ * jack[0] * ju[1]) for s_ in (-1, 1)]
    jack_edge = min(P.near(c, stock[i - 1], stock[i]) for i in range(4) for c in jacks)
    jack_notch = min(math.dist(c, r) for c in jacks for r in rods1) - NOTCH
    if jack_edge < jack[1] / 2 + 1.0 or jack_notch < jack[1] / 2 + 1.0:
        raise SystemExit(f"a jack screw lands {jack_edge:.1f} from the boss's edge and {jack_notch:.1f} from a notch")
    print(f"the sled's two jack screws land on the boss's end, {jack_edge:.1f} in from its edge and {jack_notch:.1f} clear of a nut's notch")
    hm = lambda sec: f"{int(sec // 3600)} h {round(sec % 3600 / 60):02d} min"
    print(f"the controller, simulated: the first stage {len(t1)} rows, {hm(sim1['seconds'])} of moving, {sum(1 for r in t1 if r['via'] == 'd')} moves straight; "
          f"the second {len(t2)} rows, {hm(sim2['seconds'])}, {sum(1 for r in t2 if r['via'] == 'd')} straight. The sled's solid stick: {hm(hold['seconds'])}")

    # -- drawing ---------------------------------------------------------------
    # x along the stick from its tail end; y and z the section as the sled has it, the stick's axis at the origin.
    def slab(p, name, x, pts, op="new"):
        s = p.sketch("right", x[0], name)
        p.polygon(s, [(round(a, 4), round(b, 4)) for a, b in pts])
        return p.extrude(s, x[1] - x[0], op=op, name=name)

    def rodlike(p, name, x, centres, d, op="remove"):
        s = p.sketch("right", x[0], name)
        for c in centres:
            p.circle(s, c, d / 2)
        return p.extrude(s, x[1] - x[0], op=op, name=name)

    def nuts(p, name, x, centres):
        s = p.sketch("right", x[0], name)
        for c in centres:
            p.hexagon(s, c, B.NUT)
        return p.extrude(s, x[1] - x[0], op="new", name=name)


    def piece(p, x, name="piece"):
        slab(p, name, x, blank)
        rodlike(p, "rod holes", (x[0] - 0.5, x[1] + 0.5), rods1, HOLE)

    def near_cap(p, x=(0.0, NEAR_CAP)):
        slab(p, "near cap", x, blank)
        rodlike(p, "tapped for the rods", (x[1] - TAP_DEEP, x[1] + 0.5), rods1, TAP)
        rodlike(p, "the tail spindle's socket", (x[0] - 0.5, x[0] + tail_in), [(0.0, 0.0)], tail_hole)

    def far_plate(p):
        slab(p, "far plate", (plate_x[0] + CROWN, plate_x[1]), blank)
        rodlike(p, "its crown", (plate_x[0], plate_x[0] + CROWN), [(0.0, 0.0)], 2 * CROWN_R, op="add")
        slab(p, "boss, the sled's blank", boss_x, stock, op="add")
        rodlike(p, "notches for the nuts", (boss_x[0], boss_x[1] + 0.5), rods1, 2 * NOTCH)
        # and from each notch out to the boss's corner, so no sliver of the corner stands
        s = p.sketch("right", boss_x[0], "the notches out to the corners")
        u = ((stock[1][0] - stock[0][0]) / math.dist(stock[0], stock[1]), (stock[1][1] - stock[0][1]) / math.dist(stock[0], stock[1]))
        v = (-u[1], u[0])
        for c, f in zip(rods1, B.grown(stock, 1.0)):
            du = (f[0] - c[0]) * u[0] + (f[1] - c[1]) * u[1]
            dv = (f[0] - c[0]) * v[0] + (f[1] - c[1]) * v[1]
            sq = [c, (c[0] + du * u[0], c[1] + du * u[1]), f, (c[0] + dv * v[0], c[1] + dv * v[1])]
            if B.area(sq) < 0:
                sq = sq[::-1]
            p.polygon(s, [(round(a, 4), round(b, 4)) for a, b in sq])
        p.extrude(s, boss_x[1] - boss_x[0] + 0.5, op="remove", name="the notches out to the corners")
        rodlike(p, "the rods' holes", (plate_x[0] - 0.5, boss_x[0] + 0.5), rods1, HOLE)
        rodlike(p, "inserts for the second stage's rods", (plate_x[0] - 0.5, plate_x[0] + CROWN + INSERT[1]), rods2, INSERT[0])

    def comb(p, x0=-PLATE):
        slab(p, "plate", (x0, 0.0), B.offset(outline, -PLATE_IN))       # flat: see the second stage's clamp
        for k, f in enumerate(fingers):
            slab(p, f"finger {k + 1}", (x0, spacer[0]), f["loop"], op="add")
        slab(p, "the tile, and the fit off its finished edge", (0.0, spacer[0] + 0.5), B.offset(outline, B.FIT), op="remove")
        rodlike(p, "the tail spindle's way", (x0 - 0.5, 0.5), [(0.0, 0.0)], PLATE_HOLE)
        rodlike(p, "the rods' holes", (x0 - 0.5, spacer[0] + 0.5), rods2, HOLE)

    def jig(p):
        pocket = B.grown(blank, JIG_FIT)
        s = p.sketch("top", 0.0, "jig box")
        p.polygon(s, B.grown(blank, JIG_FIT + JIG_WALL))
        p.extrude(s, JIG_FLOOR + JIG_DEEP, name="jig box")
        s = p.sketch("top", JIG_FLOOR, "pocket")
        p.polygon(s, pocket)
        p.extrude(s, JIG_DEEP + 0.5, op="remove", name="pocket")
        s = p.sketch("top", -0.5, "the drill's way out")
        for c in rods1:
            p.circle(s, c, JIG_UNDER / 2)
        p.extrude(s, JIG_FLOOR + 1.0, op="remove", name="the drill's way out")
        # The lid, drawn standing on six pieces in the box.
        lid_z = JIG_FLOOR + 6 * PIECE
        s = p.sketch("top", lid_z, "lid")
        p.polygon(s, B.grown(blank, JIG_FIT - 0.2))
        p.extrude(s, JIG_LID, name="lid")
        s = p.sketch("top", lid_z - 0.5, "guides")
        for c in rods1:
            p.circle(s, c, GUIDE / 2)
        p.circle(s, (0.0, 0.0), AXIS_GUIDE / 2)
        p.extrude(s, JIG_LID + 1.0, op="remove", name="guides")

    def tile(p):
        slab(p, "a tile", (0.0, PIECE), outline)

    def shim(p):
        # Paper, glued on the cap's inner face about the axis. Drawn where it is glued; in the stack it is squeezed
        # between the cap and the first piece, which the model draws touching.
        rodlike(p, "shim", (NEAR_CAP, NEAR_CAP + SHIM), [(0.0, 0.0)], 2 * CROWN_R, op="new")

    def pieces_body(p):
        piece(p, (NEAR_CAP, spacer[0]), name=f"{pieces} pieces")

    def rods_first(p):
        rodlike(p, "four rods", (NEAR_CAP - TAP_DEEP, boss_x[0] + NUT_T + 1.0), rods1, B.ROD, op="new")
        nuts(p, "their nuts", (boss_x[0], boss_x[0] + NUT_T), rods1)

    def after_first(p):
        slab(p, "the stack after the first stage", (0.0, spacer[0]), outline)
        for loop in wood_loops:
            slab(p, "what the first stage left", (0.0, spacer[0]), loop, op="add")
        rodlike(p, "the first stage's rod holes", (-0.5, spacer[0] + 0.5), rods1, HOLE)

    def rods_second(p):
        rodlike(p, "three rods", (-PLATE - NUT_T - 1.0, plate_x[0] + INSERT[1] - 0.5), rods2, B.ROD, op="new")
        nuts(p, "their nuts", (-PLATE - NUT_T, -PLATE), rods2)

    path = os.path.join(OUT, "billet.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "billet"})
    # Each part in its own tab, where it is on the stick (the piece and the tile at the stick's tail end, the jig on
    # the printer's bed); the two stages are assemblies of them, so nothing drawn next to something else is joined to it.
    parts = [("Piece", lambda p: piece(p, (0.0, PIECE))), ("Near cap", near_cap), ("Shim", shim), ("Pieces", pieces_body), ("Far plate", far_plate), ("Rods, first stage", rods_first),
             ("Stack after the first stage", after_first), ("Comb", comb), ("Rods, second stage", rods_second), ("Drilling jig", jig), ("A tile", tile)]
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "A billet of tile pieces for the stick sled"}, {"type": "rename_tab", "tab": 1, "name": parts[0][0]}]})
    tabs, bodies = {}, {}
    for i, (title, make) in enumerate(parts):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
        if title in ("Far plate", "Comb", "Drilling jig"):
            part.export(os.path.join(OUT, title.lower().replace(" ", "_") + ".stl"))
    for title in ("Comb", "Far plate", "Piece", "Near cap", "Stack after the first stage"):
        if len(bodies[title]) != 1:
            raise SystemExit(f"{title} is {len(bodies[title])} bodies")
    rot0 = {"x": 0.0, "y": 0.0, "z": 0.0}

    def assembled(name, members):
        """An assembly of the tabs named, each moved along the stick by the amount given."""
        tabs[name] = new_tab(mcp, {"type": "add_assembly", "name": name})
        ops = [{"type": "assembly", "tab": tabs[name], "op": {"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": True,
                                                              "placement": {"position": {"x": dx, "y": 0.0, "z": 0.0}, "rotation": rot0}}}
               for t, dx in members for k in range(len(bodies[t]))]
        text = mcp.call("apply", {"ops": ops})
        if "ERROR:" in text:
            raise SystemExit(text)
    assembled("Stage 1", [("Near cap", 0.0), ("Pieces", 0.0), ("Piece", spacer[0]), ("Far plate", 0.0), ("Rods, first stage", 0.0)])
    assembled("Stage 2", [("Stack after the first stage", 0.0), ("Piece", spacer[0]), ("Far plate", 0.0), ("Comb", 0.0), ("Rods, second stage", 0.0)])

    def shot(tab, name, **kw):
        mcp.call("screenshot", dict({"tab": tabs[tab], "path": os.path.join(OUT, name), "width": 1500, "height": 900}, **kw))
    shot("Stage 1", "stage1.png", view="-0.6,-0.7,0.45")
    shot("Stage 2", "stage2.png", view="-0.6,-0.7,0.45")
    mid = round(NEAR_CAP + PIECE * (pieces // 2) + PIECE / 2, 2)
    shot("Stage 1", "stage1_end.png", view="-1,0,0", section=f"x:{mid}", width=1000, height=900)
    shot("Stage 2", "stage2_end.png", view="-1,0,0", section=f"x:{mid}", width=1000, height=900)
    shot("Comb", "comb.png", view="-0.7,-0.6,0.5", width=1200, height=900)
    shot("Far plate", "far_plate.png", view="0.7,-0.6,0.5", width=1000, height=800)
    shot("Drilling jig", "drilling_jig.png", view="0.6,-0.7,0.6", width=1000, height=800)
    mcp.close()

    r4 = lambda pts: [[round(x, 4), round(y, 4)] for x, y in pts]
    numbers = {
        "piece": {"size": [round(math.dist(blank[0], blank[1]), 3), round(math.dist(blank[0], blank[3]), 3)], "thick": PIECE, "hole": HOLE, "count": pieces, "grow": B.GROW,
                  "corners": r4(blank), "sled_blank": r4(stock)},
        "layout": {"near_cap": [0.0, NEAR_CAP], "pieces": [NEAR_CAP, spacer[0]], "spacer": list(spacer), "far_plate": list(plate_x), "boss": list(boss_x), "stick": stick,
                   "cut_to": sled["cut_to"], "reach": reach, "comb": [-PLATE, spacer[0]]},
        "rod": {"d": B.ROD, "inset": list(B.INSET), "clear": B.CLEAR, "tab": B.TAB, "tap": TAP, "tap_deep": TAP_DEEP, "nut": B.NUT, "nut_t": NUT_T, "notch": NOTCH, "insert": list(INSERT)},
        "first": {"rods": r4(rods1), "tab_ends": r4(plan["tab_ends"]), "joined": joined, "rod_off_tile": rod_tile, "rod_off_cut": rod_cut,
                  "places": len(p1), "passes": len(f1), "rows": len(rows1), "rough_rows": sum(q["bites"] for q in p1), "finish_rows": sum(q["bites"] for q in f1),
                  "left": plan["left_first"], "wood": [r4(loop) for loop in wood_loops]},
        "second": {"rods": r4(rods2), "fingers": [{"rod": r4([f["rod"]])[0], "bears": f["bears"], "area": f["area"], "loop": r4(f["loop"]), "room": r4(f["room"])} for f in fingers],
                   "rooms": len(rooms), "rod_off_cut": finger_rod_cut, "nut_off_cut": nut_cut,
                   "places": len(p2), "passes": len(f2), "rows": len(rows2), "rough_rows": sum(q["bites"] for q in p2), "finish_rows": sum(q["bites"] for q in f2), "left": plan["left_end"]},
        "sled_rows": sled_rows, "waste": plan["waste"].area,
        "bits": bits, "comb_nut_need": comb_need, "nut_need": need,
        "comb": {"plate": PLATE, "plate_in": PLATE_IN, "plate_hole": PLATE_HOLE, "crown": CROWN, "crown_r": CROWN_R, "band": BAND, "fit": B.FIT, "plate_loop": r4(B.offset(outline, -PLATE_IN))},
        "jig": {"fit": JIG_FIT, "wall": JIG_WALL, "floor": JIG_FLOOR, "deep": JIG_DEEP, "lid": JIG_LID, "guide": GUIDE, "axis_guide": AXIS_GUIDE},
        "cuts": {"first": [[c["tool"][0], round(c["t"], 6), round(c["across"], 4), round(c["tip"], 4), round(c["area"], 3), round(c["over"], 4), round(c["far"], 3), c["face"]] for c in rows1],
                 "second": [[c["tool"][0], round(c["t"], 6), round(c["across"], 4), round(c["tip"], 4), round(c["area"], 3), round(c["over"], 4), round(c["far"], 3), c["face"]] for c in rows2]},
        "clamp": {"crown": CROWN, "crown_r": CROWN_R, "shim": SHIM, "print_e": PRINT_E, "print_ok": PRINT_OK, "ply_ok": PLY_OK, "friction": FRICTION, "over": HOLD_OVER,
                  "span": span1, "plate_w": plate_w, "plate_t": plate_t, "soft": soft1, "force": clamp1, "plate_stress": plate_stress1, "cap_stress": cap_stress, "arm": arm,
                  "push": push1, "turn": turn1, "holds_push": hold_push, "holds_turn": hold_turn,
                  "second": {"span": span2, "comb_w": comb_w, "soft": soft2, "force": clamp2, "comb_stress": comb_stress}},
        "tab": {"wide": B.TAB, "thick": PIECE, "stress": tab_stress, "across_grain": ACROSS_GRAIN, "slop": slop},
        "tail": {"in": tail_in, "hole": tail_hole}, "jack": {"at": r4(jacks), "edge": jack_edge, "notch": jack_notch, "d": jack[1]},
        "hold": {"worst": worst, "limit": hold["limit"], "extra_runs": extra, "stiff": hold["stiff"], "swing": swing_r, "peak": hold["peak"], "cut_energy": hold["cut_energy"], "rpm": hold["rpm"], "feed": hold["feed"]},
        "tables": {"first": {"rows": len(t1), "seconds": sim1["seconds"], "straight": sum(1 for r in t1 if r["via"] == "d"), "passes": sim1["passes"]},
                   "second": {"rows": len(t2), "seconds": sim2["seconds"], "straight": sum(1 for r in t2 if r["via"] == "d"), "passes": sim2["passes"]},
                   "dry": {"first": {"passes": dry1[0], "seconds": dry1[1]}, "second": {"passes": dry2[0], "seconds": dry2[1]}},
                   "along": along, "sled_seconds": hold["seconds"], "bit_waits": bit_waits, "stands_out": stands_out, "load_tip": swing},
        "outline": r4(outline),
        "bodies": {k: [list(v) for v in b_] for k, b_ in bodies.items()},
    }
    with open(os.path.join(OUT, "billet.json"), "w") as fh:
        json.dump(numbers, fh)
    print(f"done in {time.time() - t_start:.0f} s")


if __name__ == "__main__":
    main()
