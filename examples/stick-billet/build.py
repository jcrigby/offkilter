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
numbers the test checks and the shapes the animation draws.
"""
import csv
import json
import math
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)
from okmcp import Mcp, Part, new_tab  # noqa: E402
import billet as B  # noqa: E402

P = B.P
OUT = os.path.join(HERE, "out")
SLED = os.path.join(os.path.dirname(HERE), "stick-sled", "out", "sled.json")

# The pieces.
PIECE = 3.0                     # a piece, and so a tile: 3 mm of solid wood, the owner's
HOLE = 3.66                     # a #27 drill, 0.144 in: the rods' clearance through the pieces
NEAR_CAP = 12.0                 # the cap at the tail end: a block of hardwood the size of a piece, which the tail centre bites into and the four rods thread into
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
TAIL_HOLE = 14.0                # a hole in the middle for the tail spindle's 12 mm rod and its point
BAND = 1.0                      # a finger reaches this far over the tile's edge at the plate, where it joins it: twice PLATE_IN, so they overlap
# The drilling jig, printed: a box the pieces lie in, a few at a time, and a lid that drops on them with a guide hole
# over each rod and an awl's hole over the stick's axis for the cap.
JIG_FIT = 0.3                   # the box's pocket over a piece, all round
JIG_WALL, JIG_FLOOR, JIG_DEEP = 5.0, 4.0, 18.0   # its walls and floor, and the pocket's depth: six pieces
JIG_LID = 10.0                  # the lid: a plug that drops into the pocket onto the pieces
GUIDE = HOLE + 0.1              # the lid's guide holes
AWL = 1.6                       # the lid's hole over the stick's axis, for an awl: the tail centre's mark on the cap
JIG_UNDER = 6.0                 # the box's floor has a hole this wide under each guide, for the drill to come out into

INK = "#111111"


def main():
    t_start = time.time()
    os.makedirs(OUT, exist_ok=True)
    n = json.load(open(SLED))
    outline = [tuple(q) for q in n["tile"]["outline"]]
    stock = [tuple(q) for q in n["blank"]["corners"]]
    blank = B.grown(stock, B.GROW)
    sled, ax = n["sled"], n["axes"]
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
    plan = B.stages(outline, stock, places, passes)
    (p1, f1), (p2, f2) = plan["first"], plan["second"]
    rows1 = [c for q in p1 for c in B.bitten(q, "straight")] + [c for q in f1 for c in B.bitten(q, "ball")]
    rows2 = [c for q in p2 for c in B.bitten(q, "straight")] + [c for q in f2 for c in B.bitten(q, "ball")]
    sled_rows = sum(1 for r in n["rows"] if r["kind"] in ("rough", "finish"))
    print(f"first stage: {len(p1)} roughing places in {sum(q['bites'] for q in p1)} runs, {len(f1)} finishing passes in {sum(q['bites'] for q in f1)}; "
          f"second: {len(p2)} places in {sum(q['bites'] for q in p2)} runs, {len(f2)} passes in {sum(q['bites'] for q in f2)}. {len(rows1) + len(rows2)} runs where the sled's solid stick takes {sled_rows}")
    print(f"waste {plan['waste'].area:.0f} mm2: {plan['left_first']:.0f} left after the first stage, {plan['left_end']:.1f} at the end")

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
    axes = P.Axes(ax["tip_home"], ax["across_home"], tuple(ax["scale"]), ax["shear"])
    along = next(r["along"] for r in n["rows"] if r["kind"] == "rough")

    def table(rows, path):
        out = []
        for k, r in enumerate(rows):
            depth, height, turn = axes.row(r["t"], r["across"], r["tip"])
            label = (f"rough {turn:.0f} deg {r['across']:+.0f}" if r["tool"] == "straight" else f"finish {turn:.1f} deg") + f" bite {r['bite']} of {r['of']}"
            out.append({"n": k + 1, "tool": r["tool"], "depth": round(depth, 4), "height": round(height, 4), "turn": round(turn, 4), "along": along, "label": label})
        if max(r["depth"] for r in out) > ax["travel"][0] - ax["bit_margin"] or max(abs(r["height"]) for r in out) > ax["travel"][1]:
            raise SystemExit(f"{path}: a row is past the positioner's travel")
        with open(path, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(out[0].keys()))
            w.writeheader()
            w.writerows(out)
        return out
    t1 = table(rows1, os.path.join(OUT, "stage1.csv"))
    t2 = table(rows2, os.path.join(OUT, "stage2.csv"))

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

    rods1, rods2 = plan["rods"], [f["rod"] for f in fingers]

    def piece(p, x, name="piece"):
        slab(p, name, x, blank)
        rodlike(p, "rod holes", (x[0] - 0.5, x[1] + 0.5), rods1, HOLE)

    def near_cap(p, x=(0.0, NEAR_CAP)):
        slab(p, "near cap", x, blank)
        rodlike(p, "tapped for the rods", (x[1] - TAP_DEEP, x[1] + 0.5), rods1, TAP)
        rodlike(p, "the tail centre's mark", (x[0] - 0.5, x[0] + 1.5), [(0.0, 0.0)], AWL)

    def far_plate(p):
        slab(p, "far plate", plate_x, blank)
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
        rodlike(p, "inserts for the second stage's rods", (plate_x[0] - 0.5, plate_x[0] + INSERT[1]), rods2, INSERT[0])

    def comb(p, x0=-PLATE):
        slab(p, "plate", (x0, 0.0), B.offset(outline, -PLATE_IN))
        for k, f in enumerate(fingers):
            slab(p, f"finger {k + 1}", (x0, spacer[0]), f["loop"], op="add")
        slab(p, "the tile, and the fit off its finished edge", (0.0, spacer[0] + 0.5), B.offset(outline, B.FIT), op="remove")
        rodlike(p, "the tail spindle's way", (x0 - 0.5, 0.5), [(0.0, 0.0)], TAIL_HOLE)
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
        p.circle(s, (0.0, 0.0), AWL / 2)
        p.extrude(s, JIG_LID + 1.0, op="remove", name="guides")

    def tile(p):
        slab(p, "a tile", (0.0, PIECE), outline)

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
    parts = [("Piece", lambda p: piece(p, (0.0, PIECE))), ("Near cap", near_cap), ("Pieces", pieces_body), ("Far plate", far_plate), ("Rods, first stage", rods_first),
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
        "comb": {"plate": PLATE, "plate_in": PLATE_IN, "tail_hole": TAIL_HOLE, "band": BAND, "fit": B.FIT, "plate_loop": r4(B.offset(outline, -PLATE_IN))},
        "jig": {"fit": JIG_FIT, "wall": JIG_WALL, "floor": JIG_FLOOR, "deep": JIG_DEEP, "lid": JIG_LID, "guide": GUIDE, "awl": AWL},
        "cuts": {"first": [[c["tool"][0], round(c["t"], 6), round(c["across"], 4), round(c["tip"], 4)] for c in rows1],
                 "second": [[c["tool"][0], round(c["t"], 6), round(c["across"], 4), round(c["tip"], 4)] for c in rows2]},
        "tables": {"first": len(t1), "second": len(t2), "along": along},
        "outline": r4(outline),
        "bodies": {k: [list(v) for v in b_] for k, b_ in bodies.items()},
    }
    with open(os.path.join(OUT, "billet.json"), "w") as fh:
        json.dump(numbers, fh)
    print(f"done in {time.time() - t_start:.0f} s")


if __name__ == "__main__":
    main()
