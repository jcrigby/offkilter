"""A stick sled: the leaning half-ellipse Spectre milled round one solid
stick on a router table (or over a saw), the stick positioned by three
small motors and pushed over the cutter by hand.

The sled slides on the table against the fence. The stick hangs under
it between a spindle and a tail centre, over the cutter, which stays
where it is. A lift plate swings on a hinge along the sled's right
side and is held at its left by a screw: that is the stick's height.
A frame hangs from a second hinge under the plate, above the stick,
and a second screw, level, on the sled, swings it: that is the
stick's place left and right. Each screw's nose is tied to what it
moves by a short link pinned at both ends, so it holds both ways.
A third motor turns the spindle through a belt. For each pass the
sled is pulled back, the foot switch pressed, the motors go to the
next row of a table and stop, and the sled is pushed through and
pulled back. Nothing is touched off and nothing is dialled.

    cargo build --release -p ok-mcp
    python3 examples/stick-sled/build.py
    cargo test -p ok-render --test stick_sled   # what CI runs

Outputs under out/: sled.okpart (every part in place at one pass),
passes.csv (the table the controller reads), steps.json (the
controller's own program, simulated over the whole table),
master.stl, lift_block.stl, swing_block.stl and swing_base.stl to print, sled.png, end.png and
head.png, sled.pdf, and sled.json (the numbers). plan.py is the
planner and the hinges' geometry; firmware/ is the Pico's program.
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
sys.path.insert(0, os.path.join(HERE, "firmware"))
import config as FW  # noqa: E402

OUT = os.path.join(HERE, "out")

# The tile, as the stick mill's.
EDGE, BULGE, GAP = 10.56, 0.4, 0.7              # the lid's edge; 0.8 of a semicircle deep; the line between tiles
STICK = 200.0                                   # the stick's length
TABLE_CLEAR = 2.5                               # the least the blank clears the table

# The owner's hardware, as the duplicator's and the mill's.
PLY = 19.0
SHAFT_D = 20.0
SK_W, SK_T, SK_H, SK_HTOT = 60.0, 20.0, 51.0, 70.0   # SK20: base width, thickness along the shaft, base to centre, height
BLK_W, BLK_L, BLK_H, BLK_C = 50.0, 45.0, 42.0, 25.0  # SC20UU: width across, length along the shaft, height, base to centre
MOTOR = (42.0, 40.0)                            # a NEMA 17: square, and long
SPINDLE_D = 12.0                                # M12 studding
BEARING_D, BEARING_W = 32.0, 10.0               # 6201
PULLEY = (38.2, 10.2, 12.0)                     # GT2: 60 teeth on the spindle, 16 on the motor, and their width
PULLEY_R = 20.5                                 # the 60's flanges
BELT_C = 52.0                                   # the motor over the spindle

# The sled. x along the stick from its spindle end, y to the right (the lift hinge's side), z up from the table.
H1_Y = 150.0                                    # hinge 1, right of the cutter
ARM = (-H1_Y, -(SK_H - BLK_C))                  # hinge 1 to hinge 2 in the plate's frame: over the cutter, a support's height less a block's under the plate
DROP = 130.0                                    # hinge 2 down to the stick's axis
P1_Y = -130.0                                   # pusher 1, left of the cutter: the plate bridges from hinge to pusher over the stick
E1, L1 = (P1_Y - H1_Y, BLK_C - 15.0), 30.0       # link 1: its pin in a clevis under the plate's free edge, over the screw, and its length
E2_UP, L2 = 25.0, 60.0                          # link 2: its pin on the tail hanger's middle line this far over the axis, and its length
LEAN = 30.0                                     # the most a link may lean off its screw's line, degrees
HANGER_R = 18.5                                 # the hangers' lower ends, round about the axis: a bearing and 2.5
MASTER_R = 8.0                                  # the master: a sleeve over the spindle from the hanger to the spur, and the spur its size. With the sled back the cutter is under it
SPINDLE_CLEAR = 3.0                             # between the master and the cutter while the frame swings with the sled back
MOVE_CLEAR = (1.5, 1.0)                         # a move may go straight to its row, not by the safe height, if the master passes the cutter by this much and the blank the table by this much
HOME_OVER = (3.0, 2.0)                          # the home switches beyond the furthest any row goes: lift, swing
BACK_X = -12.0                                  # the cutter along the sled with the sled back: under the master, 9 mm of the straight bit clear of the stick's end, 18 of the hanger
RUN = 6.0                                       # the cutter past the stick's far end
DRAWN_X = 100.0                                 # where the cutter is drawn: mid stick, mid pass
SLED_X = (-205.0, 335.0)
LEFT_Y, RIGHT_Y = (-190.0, -95.0), (110.0, 290.0)   # the runners; the right one's edge on the fence
TIE = 30.0                                      # a tie across each end
PLATE_X = (-135.0, 285.0)
PLATE_Y = (-35.0, 292.0)                        # the plate in its own frame, from past the hinge to just past link 1's clevis, short of the lift's rail
BEAM_X, BEAM_Y = (-100.0, 250.0), 30.0
HANGER_X = ((-95.0, -76.0), (-52.0, -33.0))     # the head's two hangers, a bearing in each
TAIL_X = (232.0, 251.0)
MASTER_X = (HANGER_X[1][1], -4.0)
PUSH_X = 100.0                                  # link 1 and its clevis, half way along
ROUGH_R = P.ROUGH_BIT / 2
# Each screw's nose: a block on a linear rail's carriage, so it cannot move across its screw. Its link leans up to 21 deg,
# so what play it had in a printed bore came through to the stick at four tenths of itself.
RAIL = (12.0, 8.0, 100.0)                       # MGN12 rail: wide, high, long
CARRIAGE = (27.0, 45.4, 13.0)                   # MGN12H carriage: wide, long, and its top over the rail's seat
BLOCK = (27.0, 46.0, 24.0, 8.0)                 # the nose block on it: wide, long, high, and how far it starts short of link 2's pin
UNIT_X = TAIL_X[1] + 6.5 + BLOCK[0] / 2         # the rail's middle line: the block's near face against link 2, which lies on the tail hanger's far face
SCREW2_X = UNIT_X + 6.0                         # screw 2, through the block beside the pin's end, at the pin's height
# Screw 1's nose the same, upright: the rail on a post on the left runner, facing the stick.
RAIL1_L = 150.0                                 # its rail: 79 of travel and a carriage
BLOCK1 = (27.0, 68.0, 24.0, 8.0)                # its block: wide, tall, thick, and how far its top is over link 1's pin; the screw's nut is in its foot
UNIT1_X = PUSH_X + 14.5 + BLOCK1[0] / 2         # the rail's middle line: the block's near face against link 1
SCREW1_X = UNIT1_X + 6.0
POST_Y = P1_Y - BLOCK1[2] / 2 - CARRIAGE[2]     # the post's face: rail, carriage and half the block from the screw's line


def slab(p, name, x, pts, op="new"):
    s = p.sketch("right", x[0], name)
    p.polygon(s, pts)
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def box(p, name, x, y, z, op="new"):
    return slab(p, name, x, [(y[0], z[0]), (y[1], z[0]), (y[1], z[1]), (y[0], z[1])], op)


def xcyl(p, name, x0, x1, c, d, op="new"):
    s = p.sketch("right", x0, name)
    p.circle(s, c, d / 2)
    return p.extrude(s, x1 - x0, op=op, name=name)


def xbore(p, x0, x1, c, d, name):
    s = p.sketch("right", x0 - 1.0, name)
    p.circle(s, c, d / 2)
    p.cut(s, x1 - x0 + 2.0, name=name)


def vcyl(p, name, cx, cy, z0, z1, d, op="new"):
    s = p.sketch("top", z0, name)
    p.circle(s, (cx, cy), d / 2)
    return p.extrude(s, z1 - z0, op=op, name=name)


def tf(fr, pts):
    """Points of a frame (its origin, its angle) in the sled's."""
    (oy, oz), a = fr
    c, s = math.cos(a), math.sin(a)
    return [(oy + y * c - z * s, oz + y * s + z * c) for y, z in pts]


def rect(y0, y1, z0, z1):
    return [(y0, z0), (y1, z0), (y1, z1), (y0, z1)]


def hanger_outline():
    """A hanger in the hanging frame: under the beam, down to a round end about the axis."""
    arc = [(HANGER_R * math.cos(math.radians(-a)), -DROP + HANGER_R * math.sin(math.radians(-a))) for a in range(0, 181, 15)]
    return [(-35.0, -BLK_C - PLY), (35.0, -BLK_C - PLY), (35.0, -DROP + 30.0)] + arc + [(-35.0, -DROP + 30.0)]


def link(a, b, w=10.0):
    """A link between two pins, as a strip."""
    dy, dz = b[0] - a[0], b[1] - a[1]
    ln = math.hypot(dy, dz)
    ny, nz = -dz / ln * w / 2, dy / ln * w / 2
    ey, ez = dy / ln * w / 2, dz / ln * w / 2
    return [(a[0] - ey + ny, a[1] - ez + nz), (a[0] - ey - ny, a[1] - ez - nz), (b[0] + ey - ny, b[1] + ez - nz), (b[0] + ey + ny, b[1] + ez + nz)]


class Recorder:
    """Stands in for a Part: takes the same sketches and extrudes and keeps each body as prisms,
    (x0, x1, outline in y and z), so the sled can be tried at many poses without the kernel.
    Cuts and holes are left out, so a bored part is solid here; an upright cylinder is its box."""

    def __init__(self):
        self.sketches, self.bodies = [], []

    def sketch(self, base, offset, name):
        self.sketches.append([base, offset, None])
        return len(self.sketches) - 1

    def polygon(self, s, pts):
        self.sketches[s][2] = ("poly", list(pts))

    def circle(self, s, c, r):
        self.sketches[s][2] = ("circle", c, r)

    def point(self, s, pt):
        pass

    def hole(self, s, d, name=None):
        pass

    def cut(self, s, depth, name=None):
        pass

    def extrude(self, s, depth, op="new", name=None):
        base, off, shape = self.sketches[s]
        if base == "right":
            pts = shape[1] if shape[0] == "poly" else [(shape[1][0] + shape[2] * math.cos(math.radians(15 * k)), shape[1][1] + shape[2] * math.sin(math.radians(15 * k))) for k in range(24)]
            prism = (off, off + depth, pts)
        elif base == "top" and shape[0] == "circle":       # an upright cylinder: (x, y) centre, from z = off up
            (cx, cy), r = shape[1], shape[2]
            prism = (cx - r, cx + r, rect(cy - r, cy + r, off, off + depth))
        else:
            raise SystemExit(f"the recorder cannot take a {base} sketch")
        if op == "new" or not self.bodies:
            self.bodies.append([prism])
        else:
            self.bodies[-1].append(prism)


def crossing(a, b, c, d, eps=1e-6):
    """Two segments properly cross: each one's ends stand clear on opposite sides of the other."""
    def side(o, p, q):
        ln = math.hypot(p[0] - o[0], p[1] - o[1]) or 1.0
        return ((p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])) / ln
    s1, s2, s3, s4 = side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b)
    return ((s1 > eps and s2 < -eps) or (s1 < -eps and s2 > eps)) and ((s3 > eps and s4 < -eps) or (s3 < -eps and s4 > eps))


def within(q, poly, eps=1e-6):
    return P.inside(q, poly) and min(P.near(q, poly[i - 1], poly[i]) for i in range(len(poly))) > eps


def outlines_meet(a, b):
    if max(q[0] for q in a) <= min(q[0] for q in b) or max(q[0] for q in b) <= min(q[0] for q in a):
        return False
    if max(q[1] for q in a) <= min(q[1] for q in b) or max(q[1] for q in b) <= min(q[1] for q in a):
        return False
    if any(crossing(a[i - 1], a[i], b[j - 1], b[j]) for i in range(len(a)) for j in range(len(b))):
        return True
    return within(a[0], b) or within(b[0], a)


# Parts that are bored for one another, or touch by design: solid in the recorder, so not tried against each other.
FITTED = [("Hinge 1 shaft", "Hinge 1 supports SK20"), ("Hinge 1 shaft", "Hinge 1 blocks SC20UU"), ("Hinge 2 shaft", "Hinge 2 supports SK20"), ("Hinge 2 shaft", "Hinge 2 blocks SC20UU"),
          ("Spindle and centre", "Bearings 6201"), ("Spindle and centre", "Head hangers"), ("Spindle and centre", "Belt drive"), ("Spindle and centre", "Master"),
          ("Spindle and centre", "Tail hanger"), ("Bearings 6201", "Head hangers"), ("Lift pusher", "Lift block"), ("Swing pusher", "Swing block"), ("Cutter", "Stick"), ("Cutter", "Table and fence")]


def meetings(parts, skip=()):
    """Which bodies of a posed sled are in one another: pairs of tab names."""
    rec = []
    for title, make in parts:
        if title in skip:
            continue
        r = Recorder()
        make(r)
        rec.extend((title, body) for body in r.bodies)
    hits = set()
    for i in range(len(rec)):
        for k in range(i + 1, len(rec)):
            (ta, ba), (tb, bb) = rec[i], rec[k]
            if ta == tb or (ta, tb) in FITTED or (tb, ta) in FITTED:
                continue
            if any(pa[0] < pb[1] - 1e-6 and pb[0] < pa[1] - 1e-6 and outlines_meet(pa[2], pb[2]) for pa in ba for pb in bb):
                hits.add((ta, tb))
    return hits, rec


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "sled.okpart")
    if os.path.exists(path):
        os.remove(path)
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
    print(f"tile {tile['across']:.1f} across, swings {swing_r:.1f}; {len(passes)} finishing passes from as many directions, the cutter within {max(abs(q['across']) for q in passes):.1f} of the axis; "
          f"{100 * covered:.1f} % of the outline within 0.08 of a pass, the worst {max(left):.2f}; {orphans} points nothing reaches")
    blank = swing_r + P.BLANK

    # -- the cutter's height: the blank clear of the table, the pulley and the hangers too, at the lowest any cut brings the stick
    deep = cands + passes                            # every cut at its last bite; the bites before it are shallower
    need = max(P.blank_reach(outline, q["t"]) - q["tip"] for q in deep) + TABLE_CLEAR
    low_tip, high_tip = min(min(q["tip"] for q in deep), MASTER_R), blank          # the first roughing bites are the highest the stick is ever cut
    bit_up = math.ceil(2 * max(need, PULLEY_R + 1.5 - low_tip, HANGER_R + 2.0 - low_tip)) / 2

    # -- the hinges ----------------------------------------------------------
    za_mid = bit_up + (low_tip + high_tip) / 2
    h1 = (H1_Y, round(za_mid + DROP - ARM[1]))       # so the plate lies level and the frame hangs square at the middle of the travel
    e2 = (0.0, -(DROP - E2_UP))
    sled = P.Sled(h1, ARM, DROP, P1_Y, E1, L1, 0.0, e2, L2)
    pins = [sled.pin2(*sled.angles(-q["across"], bit_up + q["tip"]))[1] for q in deep] + [sled.pin2(*sled.angles(0.0, bit_up + high_tip))[1]]
    sled.p2_z = p2_z = round((min(pins) + max(pins)) / 2)        # screw 2 level with the middle of where its link's far pin goes

    # -- the order, and then the bites -----------------------------------------
    # Roughing, and then finishing, go round the stick in the order of the spindle's own angle, so it turns a little at a
    # time and mostly forwards. The order has to be settled before the bites are counted: what a cut meets is what the
    # cuts before it left. (Counted in one order and made in another, a roughing cut met 11.7 mm in one bite.)
    turn_of = lambda q: sled.pose(q["t"], q["across"], q["tip"], bit_up)["gamma"]
    cands.sort(key=lambda q: (turn_of(q), q["across"]))
    passes.sort(key=turn_of)
    places, waste = P.bites(outline, cands, passes)
    print(f"waste {waste['waste']:.0f} mm2 of section: {len(places)} roughing places take {waste['rough_area']:.0f} in {sum(q['bites'] for q in places)} runs; "
          f"the ball-nose takes the rest in {sum(q['bites'] for q in passes)} runs, the deepest {max(q['depth'] for q in passes):.1f}; {waste['left_at_end']:.1f} stands at the end")
    cuts = []                                        # every time the cutter is brought to the stick: kind, tool, t, across, tip, label
    cuts.append(("set", "straight", 0.0, 0.0, MASTER_R, "set the straight bit on the master"))
    for q in places:
        for b in range(1, q["bites"] + 1):
            cuts.append(("rough", "straight", q["t"], q["across"], q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"], f"rough {math.degrees(q['t']):.0f} deg {q['across']:+.0f} bite {b} of {q['bites']}"))
    cuts.append(("set", "ball", 0.0, 0.0, MASTER_R, "set the ball-nose on the master"))
    for q in passes:
        for b in range(1, q["bites"] + 1):
            cuts.append(("finish", "ball", q["t"], q["across"], q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"], f"finish bite {b} of {q['bites']}"))
    poses = [sled.pose(c[2], c[3], c[4], bit_up) for c in cuts]
    leans = [sled.lean(q["alpha"], q["beta"]) for q in poses]
    lean = [max(abs(math.degrees(v[k])) for v in leans) for k in (0, 1)]
    if max(lean) > LEAN:
        raise SystemExit(f"a link leans {lean} deg")
    c1_home = math.ceil(max(q["c1"] for q in poses) + HOME_OVER[0])
    c2_home = math.ceil(max(q["c2"] for q in poses) + HOME_OVER[1])
    rows = []
    for n, (c, q) in enumerate(zip(cuts, poses)):
        rows.append({"n": n + 1, "kind": c[0], "tool": c[1], "t": c[2], "across": c[3], "tip": c[4], "label": c[5],
                     "lift": round(c1_home - q["c1"], 4), "swing": round(c2_home - q["c2"], 4), "turn": round(q["gamma"], 4)})
    back = max(abs(sled.cutter(c1_home - r["lift"], c2_home - r["swing"], r["turn"], bit_up)[0] - r["across"]) + abs(sled.cutter(c1_home - r["lift"], c2_home - r["swing"], r["turn"], bit_up)[1] - r["tip"]) for r in rows)
    if back > 1e-3:
        raise SystemExit(f"the joints do not give the cuts back: {back}")
    alphas, betas = [q["alpha"] for q in poses], [q["beta"] for q in poses]
    # The lift at or above which the spindle clears the cutter wherever the frame swings: the sled is back then, only the spindle over the cutter.
    safe = 0.0
    while True:
        a = sled.alpha_of(c1_home - (safe + 0.25))
        c2_lo = min(q["c2"] for q in poses)             # the frame is lowest hanging straight down, in the middle of its swing: try all of it
        if min(sled.axis(a, sled.beta_of(c2_lo + (c2_home - c2_lo) * k / 40, a))[1] for k in range(41)) < bit_up + MASTER_R + SPINDLE_CLEAR:
            break
        safe += 0.25
    print(f"cutter {bit_up:.1f} over the table; hinge 1 at {h1}; the plate swings {math.degrees(min(alphas)):.1f} to {math.degrees(max(alphas)):.1f} deg, the frame hangs {math.degrees(min(x + y for x, y in zip(alphas, betas))):.1f} to {math.degrees(max(x + y for x, y in zip(alphas, betas))):.1f} off upright; screw 2 at {p2_z:.0f}; the links lean at most {lean[0]:.1f} and {lean[1]:.1f} deg; "
          f"lift {min(r['lift'] for r in rows):.1f} to {max(r['lift'] for r in rows):.1f} mm from home, swing {min(r['swing'] for r in rows):.1f} to {max(r['swing'] for r in rows):.1f}; safe to swing at lift {safe:.2f} or less")

    print(f"axis moved {centre} in the tile: a circle of {circle:.2f} fits about it, the master and spur {2 * MASTER_R:.0f} round inside that")

    # -- which moves can go straight -----------------------------------------
    # Going up to the safe height and down again between passes is most of what the lift screw does. A move may skip it
    # if, done as the controller does it (swing, then turn, then lift), the master stays clear of the cutter under it and
    # the blank stays clear of the table as it turns: both are followed along the move's own path.
    def cutter_edge(tool):
        """Points round the cutter's outline over the table, for the bit in the router."""
        if tool == "ball":
            r, zc, pts = P.BIT / 2, bit_up - P.BIT / 2, []
            for k in range(61):
                off = P.SHANK / 2 * k / 60
                f = P.flank(off, r)
                if f is not None:
                    pts += [(off, zc - f), (-off, zc - f)]
            low = min(z for _, z in pts)
            return pts + [(sy * P.SHANK / 2, z) for sy in (-1, 1) for z in range(0, int(low) + 1)]
        r = ROUGH_R
        return [(r * k / 10, bit_up) for k in range(-10, 11)] + [(sy * r, z) for sy in (-1, 1) for z in range(0, int(bit_up) + 1)]
    edges = {tool: cutter_edge(tool) for tool in ("straight", "ball")}

    def master_gap(lift, swing, tool):
        a_ = sled.alpha_of(c1_home - lift)
        b_ = sled.beta_of(c2_home - swing, a_)
        ya_, za_ = sled.axis(a_, b_)
        return min(math.hypot(ya_ - y, za_ - z) for y, z in edges[tool]) - MASTER_R, za_, a_ + b_

    def way_round(g0, g1):
        """The spindle's angles on its way from g0 to g1, as the controller turns it: forwards, or back a little past and up to it."""
        ahead = (g1 - g0) % 360.0
        legs = [(g0, g0 + ahead)] if ahead <= 180.0 else [(g0, g0 + ahead - 360.0 - FW.TURN_PAST_DEG), (g1 - FW.TURN_PAST_DEG, g1)]
        return [a0 + (a1 - a0) * k / max(1, math.ceil(abs(a1 - a0))) for a0, a1 in legs for k in range(max(1, math.ceil(abs(a1 - a0))) + 1)]

    def straight_ok(prev, row):
        steps = lambda a0, a1: [a0 + (a1 - a0) * k / max(1, math.ceil(abs(a1 - a0) / 0.5)) for k in range(max(1, math.ceil(abs(a1 - a0) / 0.5)) + 1)]
        t_of = lambda g, hang: -(math.radians(g) + hang) - math.pi / 2
        for swing in steps(prev["swing"], row["swing"]):                       # the swing, at the old lift and angle
            gap, za_, hang = master_gap(prev["lift"], swing, row["tool"])
            if gap < MOVE_CLEAR[0] or za_ - P.blank_reach(outline, t_of(prev["turn"], hang)) < MOVE_CLEAR[1]:
                return False
        _, za_, hang = master_gap(prev["lift"], row["swing"], row["tool"])
        if any(za_ - P.blank_reach(outline, t_of(g, hang)) < MOVE_CLEAR[1] for g in way_round(prev["turn"], row["turn"])):     # the turn, there
            return False
        for lift in steps(prev["lift"], row["lift"]):                           # the lift, at the new swing and angle
            gap, za_, hang = master_gap(lift, row["swing"], row["tool"])
            if gap < MOVE_CLEAR[0] or za_ - P.blank_reach(outline, t_of(row["turn"], hang)) < MOVE_CLEAR[1]:
                return False
        return True
    for i, r in enumerate(rows):
        r["via"] = "d" if r["kind"] != "set" and i > 0 and rows[i - 1]["kind"] != "set" and straight_ok(rows[i - 1], r) else "s"
    straight = sum(1 for r in rows if r["via"] == "d")
    print(f"{straight} of {len(rows)} moves go straight to their row; the rest by the safe height")

    # -- the table, and the controller's own run over it ---------------------
    table = os.path.join(OUT, "passes.csv")
    with open(table, "w") as f:
        f.write(f"#safe,{safe}\n")
        f.write("n,kind,tool,lift,swing,turn,via,label\n")
        for r in rows:
            f.write(f"{r['n']},{r['kind']},{r['tool']},{r['lift']},{r['swing']},{r['turn']},{r['via']},{r['label']}\n")
    steps = os.path.join(OUT, "steps.json")
    subprocess.run([sys.executable, os.path.join(HERE, "firmware", "main.py"), "--sim", table, steps], check=True, cwd=os.path.join(HERE, "firmware"))
    sim = json.load(open(steps))
    # and again as if the stick had been left half way: homed, and taken up from the row after
    half = len(rows) // 2
    again = os.path.join(OUT, "steps_again.json")
    subprocess.run([sys.executable, os.path.join(HERE, "firmware", "main.py"), "--sim", table, again, str(half)], check=True, cwd=os.path.join(HERE, "firmware"))
    sim["resumed"] = json.load(open(again))
    os.remove(again)
    if sim["resumed"]["rows"] != sim["rows"][half:]:
        raise SystemExit("taken up half way, the controller does not end the rows where it does run through")
    with open(steps, "w") as f:
        json.dump(sim, f)
    print(f"{len(rows)} rows; the controller, simulated, ends at {sim['rows'][-1][1:]} steps and never swings below lift step {sim['deepest_crossing']} (safe {sim['safe_steps']})")

    # -- the parts, at one pass ----------------------------------------------
    drawn = next(i for i, r in enumerate(rows) if r["kind"] == "finish" and abs(r["across"]) < 2.0 and r["tip"] > 12.0 and r["label"].startswith("finish bite 1 of 1"))
    top_z = h1[1] + SK_W / 2
    c1_low = min(p["c1"] for p in poses)
    over1 = lambda c1_: c1_ + BLOCK1[3] - BLOCK1[1] / 2    # the lift block's middle, and its carriage's
    rail1_z = (math.floor(over1(c1_low) - CARRIAGE[1] / 2 - 6.0), math.floor(over1(c1_low) - CARRIAGE[1] / 2 - 6.0) + RAIL1_L)
    if over1(c1_home) + CARRIAGE[1] / 2 > rail1_z[1] - 2.0:
        raise SystemExit("the lift's carriage runs off its rail")
    screw1_top = c1_home + BLOCK1[3] - BLOCK1[1] + 6.0   # the screw's end: just past the nut with the block at home
    motor_c = (0.0, -DROP + BELT_C)                  # the spindle's motor in the hanging frame
    c2_low = min(p["c2"] for p in poses)
    rail_z = p2_z - CARRIAGE[2] - BLOCK[2] / 2       # the rail's seat: the pin half way up the block
    over = lambda c2_: c2_ - BLOCK[3] + BLOCK[1] / 2   # the block's middle, and the carriage's under it
    rail_y = (math.floor(over(c2_low) - CARRIAGE[1] / 2 - 6.0), math.floor(over(c2_low) - CARRIAGE[1] / 2 - 6.0) + RAIL[2])
    if over(c2_home) + CARRIAGE[1] / 2 > rail_y[1] - 2.0:
        raise SystemExit("the carriage runs off its rail")
    screw2_end = c2_low - BLOCK[3] - 4.0             # the swing screw's end: just past the block's far face at its furthest
    motor2_y = max(c2_home - BLOCK[3] + BLOCK[1] + 8.0, rail_y[1] + 2.0)   # the motor's face: clear of the block at home, and past the rail's end

    def parts_at(q):
        """Every part, as a list of (tab, maker), with the sled at the joints q."""
        a, b = q["alpha"], q["beta"]
        h2 = sled.hinge2(a)
        axis = (q["ya"], q["za"])
        PL, PL2, HG = (h1, a), (h2, a), (h2, a + b)
        psi = math.radians(q["gamma"]) + a + b
        section = tf((axis, psi), outline)
        c1, c2 = q["c1"], q["c2"]

        def sled_base(p):
            box(p, "left runner", SLED_X, LEFT_Y, (0.0, PLY))
            box(p, "right runner", SLED_X, RIGHT_Y, (0.0, PLY), op="add")
            for x in ((SLED_X[0], SLED_X[0] + TIE), (SLED_X[1] - TIE, SLED_X[1])):
                box(p, "tie", x, (LEFT_Y[1], RIGHT_Y[0]), (0.0, PLY), op="add")
            box(p, "shelf under the swing rail", (TAIL_X[1] + 1.0, SLED_X[1] - TIE), (rail_y[0] - 4.0, RIGHT_Y[0]), (0.0, PLY), op="add")

        def riser(p):
            y0 = H1_Y + SK_H
            box(p, "riser wall", SLED_X, (y0, y0 + PLY), (PLY, top_z))
            for x in ((SLED_X[0], SLED_X[0] + PLY), (SLED_X[1] - PLY, SLED_X[1])):
                slab(p, "gusset", x, [(y0 + PLY, PLY), (y0 + PLY + 70.0, PLY), (y0 + PLY, top_z - 20.0)], op="add")

        def supports1(p):
            for i, x in enumerate((-160.0, 300.0)):
                box(p, "SK20", (x, x + SK_T), (H1_Y - (SK_HTOT - SK_H), H1_Y + SK_H), (h1[1] - 16.0, h1[1] + 16.0), op="new" if i == 0 else "new")
                xbore(p, x, x + SK_T, h1, SHAFT_D, "shaft")

        def blocks(p, fr, at, flip):
            """SC20UU on a shaft at a frame's origin, their bases up (under the plate) or down (on the beam)."""
            z = (-(BLK_H - BLK_C), BLK_C) if not flip else (-BLK_C, BLK_H - BLK_C)
            for x in at:
                slab(p, "SC20UU", (x - BLK_L / 2, x + BLK_L / 2), tf(fr, rect(-BLK_W / 2, BLK_W / 2, z[0], z[1])))
                xbore(p, x - BLK_L / 2, x + BLK_L / 2, fr[0], SHAFT_D, "shaft")

        def plate(p):
            slab(p, "lift plate", PLATE_X, tf(PL, rect(-PLATE_Y[1], -PLATE_Y[0], BLK_C, BLK_C + PLY)))

        def supports2(p):
            for x in (-128.0, 258.0):
                slab(p, "SK20", (x, x + SK_T), tf(PL2, rect(-16.0, 16.0, -(SK_HTOT - SK_H), SK_H)))
                xbore(p, x, x + SK_T, h2, SHAFT_D, "shaft")

        def beam(p):
            slab(p, "beam", BEAM_X, tf(HG, rect(-BEAM_Y, BEAM_Y, -BLK_C - PLY, -BLK_C)))

        def hangers(p):
            for x in HANGER_X:
                slab(p, "head hanger", x, tf(HG, hanger_outline()))
                xbore(p, x[0], x[1], axis, BEARING_D, "bearing")

        def tail(p):
            slab(p, "tail hanger", TAIL_X, tf(HG, hanger_outline()))
            xbore(p, TAIL_X[0], TAIL_X[1], axis, 10.5, "centre")

        def bearings(p):
            for x in (HANGER_X[0][0], HANGER_X[1][1] - BEARING_W):
                xcyl(p, "6201", x, x + BEARING_W, axis, BEARING_D - 0.1)
                xbore(p, x, x + BEARING_W, axis, SPINDLE_D + 0.2, "bore")

        def spindle(p):
            xcyl(p, "M12 studding", -116.0, -4.0, axis, SPINDLE_D)
            xcyl(p, "drive spur", -4.0, 0.0, axis, 2 * MASTER_R, op="add")
            xcyl(p, "tail centre, M10 pointed", STICK, TAIL_X[1] + 5.0, axis, 10.0, op="new")

        def drive(p):
            mc = tf(HG, [motor_c])[0]
            x1 = -100.0
            xcyl(p, "60-tooth pulley", x1 - PULLEY[2], x1, axis, PULLEY[0])
            xbore(p, x1 - PULLEY[2], x1, axis, SPINDLE_D + 0.2, "bore")
            xcyl(p, "16-tooth pulley", x1 - PULLEY[2], x1, mc, PULLEY[1], op="new")
            m = MOTOR[0] / 2
            slab(p, "spindle motor", (x1 - 25.0 - MOTOR[1], x1 - 25.0), tf(HG, rect(-m, m, motor_c[1] - m, motor_c[1] + m)))
            xcyl(p, "motor shaft", x1 - 25.0, x1 - PULLEY[2], mc, 5.0, op="add")
            for y in ((m, m + 6.0), (-m - 6.0, -m)):
                slab(p, "bracket cheek", (x1 - 25.0, HANGER_X[0][0]), tf(HG, rect(y[0], y[1], motor_c[1] - m, motor_c[1] + m)))

        def master(p):
            xcyl(p, "master", MASTER_X[0], MASTER_X[1], axis, 2 * MASTER_R)
            xbore(p, MASTER_X[0], MASTER_X[1], axis, SPINDLE_D + 0.2, "spindle")

        def stick(p):
            slab(p, "stick, milled", (0.0, STICK), section)

        pin1, pin2 = sled.pin1(a), sled.pin2(a, b)

        block1_z = (c1 + BLOCK1[3] - BLOCK1[1], c1 + BLOCK1[3])

        def lift_pusher(p):
            m = MOTOR[0] / 2
            box(p, "lift motor", (SCREW1_X - m, SCREW1_X + m), (P1_Y - m, P1_Y + m), (PLY, PLY + MOTOR[1]))
            vcyl(p, "T8 screw", SCREW1_X, P1_Y, PLY + MOTOR[1], screw1_top, 8.0, op="new")

        def lift_rail(p):
            box(p, "post", (UNIT1_X - 30.0, UNIT1_X + 30.0), (POST_Y - PLY, POST_Y), (PLY, rail1_z[1] + 4.0))
            box(p, "MGN12 rail", (UNIT1_X - RAIL[0] / 2, UNIT1_X + RAIL[0] / 2), (POST_Y, POST_Y + RAIL[1]), rail1_z, op="new")
            mid = over1(c1)
            box(p, "MGN12H carriage", (UNIT1_X - CARRIAGE[0] / 2, UNIT1_X + CARRIAGE[0] / 2), (POST_Y + RAIL[1], POST_Y + CARRIAGE[2]), (mid - CARRIAGE[1] / 2, mid + CARRIAGE[1] / 2), op="new")

        def lift_block(p):
            """The lift's nose: on the carriage, link 1's pin near its top, the screw's nut in its foot."""
            box(p, "nose block", (UNIT1_X - BLOCK1[0] / 2, UNIT1_X + BLOCK1[0] / 2), (POST_Y + CARRIAGE[2], POST_Y + CARRIAGE[2] + BLOCK1[2]), block1_z)
            s = p.sketch("top", block1_z[1], "the screw's way")
            p.point(s, (SCREW1_X, P1_Y))
            p.hole(s, 9.0, name="the screw's way")

        def links(p):
            """Each nose to what it moves: a strip beside the nose, a pin through each end (the pins are not drawn)."""
            slab(p, "link 1", (PUSH_X + 8.0, PUSH_X + 14.0), link((P1_Y, c1), pin1))      # between the clevis and the block's near face
            slab(p, "clevis under the plate", (PUSH_X - 7.0, PUSH_X + 7.0), tf(PL, rect(E1[0] - 10.0, E1[0] + 10.0, E1[1] - 8.0, BLK_C)))
            slab(p, "link 2", (TAIL_X[1], TAIL_X[1] + 6.0), link((c2, p2_z), pin2))

        block_y = (c2 - BLOCK[3], c2 - BLOCK[3] + BLOCK[1])

        def swing_pusher(p):
            m = MOTOR[0] / 2
            box(p, "swing motor", (SCREW2_X - m, SCREW2_X + m), (motor2_y, motor2_y + MOTOR[1]), (p2_z - m, p2_z + m))
            box(p, "T8 screw", (SCREW2_X - 4.0, SCREW2_X + 4.0), (screw2_end, motor2_y), (p2_z - 4.0, p2_z + 4.0), op="new")

        def swing_rail(p):
            box(p, "MGN12 rail", (UNIT_X - RAIL[0] / 2, UNIT_X + RAIL[0] / 2), rail_y, (rail_z, rail_z + RAIL[1]))
            mid = over(c2)
            box(p, "MGN12H carriage", (UNIT_X - CARRIAGE[0] / 2, UNIT_X + CARRIAGE[0] / 2), (mid - CARRIAGE[1] / 2, mid + CARRIAGE[1] / 2), (rail_z + RAIL[1], rail_z + CARRIAGE[2]), op="new")

        def swing_block(p):
            """The nose: on the carriage, link 2's pin in its near end and the screw's nut beside it. Bores not drawn."""
            box(p, "nose block", (UNIT_X - BLOCK[0] / 2, UNIT_X + BLOCK[0] / 2), block_y, (rail_z + CARRIAGE[2], rail_z + CARRIAGE[2] + BLOCK[2]))
            s = p.sketch("front", -block_y[1] - 1.0, "the screw's way")
            p.circle(s, (SCREW2_X, p2_z), 6.0)
            p.cut(s, BLOCK[1] + 2.0, name="the screw's way")

        def swing_base(p):
            """Under the rail and the motor, on the shelf and the right runner."""
            m = MOTOR[0] / 2
            box(p, "rail's seat", (UNIT_X - 13.0, UNIT_X + 13.0), rail_y, (PLY, rail_z))
            box(p, "motor's seat", (SCREW2_X - m, SCREW2_X + m), (motor2_y, motor2_y + MOTOR[1]), (PLY, p2_z - m), op="add")

        def controller(p):
            box(p, "controller box", (0.0, 90.0), (H1_Y + SK_H + PLY + 6.0, RIGHT_Y[1] - 4.0), (PLY, PLY + 40.0))

        def table_top(p):
            box(p, "table", (DRAWN_X - 420.0, DRAWN_X + 420.0), (LEFT_Y[0] - 60.0, RIGHT_Y[1] + 60.0), (-30.0, 0.0))
            box(p, "fence", (DRAWN_X - 420.0, DRAWN_X + 420.0), (RIGHT_Y[1], RIGHT_Y[1] + 40.0), (0.0, 60.0), op="new")

        def cutter(p):
            vcyl(p, "ball-nose, 3 mm", DRAWN_X, 0.0, 0.0, bit_up, P.BIT)      # from the table up; the router is under it

        parts = [("Table and fence", table_top), ("Cutter", cutter), ("Sled", sled_base), ("Riser", riser),
                 ("Hinge 1 shaft", lambda p: xcyl(p, "20 mm shaft", -165.0, 325.0, h1, SHAFT_D)), ("Hinge 1 supports SK20", supports1),
                 ("Hinge 1 blocks SC20UU", lambda p: blocks(p, PL, (-60.0, 210.0), False)), ("Lift plate", plate),
                 ("Hinge 2 supports SK20", supports2), ("Hinge 2 shaft", lambda p: xcyl(p, "20 mm shaft", -133.0, 283.0, h2, SHAFT_D)),
                 ("Hinge 2 blocks SC20UU", lambda p: blocks(p, HG, (-70.0, 215.0), True)), ("Beam", beam), ("Head hangers", hangers), ("Tail hanger", tail),
                 ("Bearings 6201", bearings), ("Spindle and centre", spindle), ("Belt drive", drive), ("Master", master), ("Stick", stick),
                 ("Lift pusher", lift_pusher), ("Lift rail", lift_rail), ("Lift block", lift_block), ("Links", links), ("Swing pusher", swing_pusher), ("Swing rail", swing_rail), ("Swing block", swing_block), ("Swing base", swing_base), ("Controller", controller)]
        return parts, {"a": a, "b": b, "h2": h2, "axis": axis, "c1": c1, "c2": c2, "psi": psi, "pin1": pin1, "pin2": pin2}

    # -- nothing through anything, wherever the motors can put the sled ------
    # The table of rows is one pose in six hundred; between them the controller lifts to the safe height, swings and turns there,
    # and comes down. So: every twelfth row as it stands, its way down from the safe height, and the whole of the swing at and above it.
    def joints(lift, swing, turn=0.0):
        a_ = sled.alpha_of(c1_home - lift)
        b_ = sled.beta_of(c2_home - swing, a_)
        ya_, za_ = sled.axis(a_, b_)
        return {"alpha": a_, "beta": b_, "gamma": turn, "ya": ya_, "za": za_, "c1": c1_home - lift, "c2": c2_home - swing}
    swing_most = max(r["swing"] for r in rows)
    tried = [(poses[i], ()) for i in range(0, len(poses), 12)]
    for r in rows[::12]:
        tried += [(joints(safe + (r["lift"] - safe) * k / 4, r["swing"], r["turn"]), ("Stick",)) for k in range(4) if r["lift"] > safe]
    tried += [(joints(safe * i / 4, swing_most * k / 5), ("Stick",)) for i in range(5) for k in range(6)]
    # and, of the moves that go straight, every sixth at its corner: the old lift and angle with the new swing
    tried += [(joints(rows[i - 1]["lift"], r["swing"], rows[i - 1]["turn"]), ()) for i, r in enumerate(rows) if r["via"] == "d" and i % 6 == 0]
    lane = rect(-ROUGH_R - 1.0, ROUGH_R + 1.0, -1.0, bit_up + 1.0)       # where the cutter is, wherever the sled is along its travel
    lane_x = (MASTER_X[0], STICK + RUN + 23.0)
    in_lane = ("Cutter", "Table and fence", "Stick", "Master", "Spindle and centre")    # these the plan itself keeps off the cutter
    met, in_the_lane = {}, set()
    for q_, skip in tried:
        hits, rec = meetings(parts_at(q_)[0], skip=("Cutter",) + skip)
        for h in hits:
            met[h] = met.get(h, 0) + 1
        for title, body in rec:
            if title not in in_lane and any(pr[0] < lane_x[1] and lane_x[0] < pr[1] and outlines_meet(pr[2], lane) for pr in body):
                in_the_lane.add(title)
    print(f"{len(tried)} poses tried: " + ("nothing through anything, nothing in the cutter's lane" if not met and not in_the_lane else f"MEETINGS {met}; IN THE CUTTER'S LANE {sorted(in_the_lane)}"))

    if met or in_the_lane:
        raise SystemExit("the sled meets itself")

    parts, at = parts_at(poses[drawn])
    a, b, h2, axis, c1, c2, psi, pin1, pin2 = (at[k] for k in ("a", "b", "h2", "axis", "c1", "c2", "psi", "pin1", "pin2"))

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
    mcp.call("screenshot", {"tab": asm, "view": "-0.6,-0.65,0.45", "width": 1700, "height": 1100, "path": os.path.join(OUT, "sled.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{DRAWN_X}", "fit": f"{DRAWN_X},{LEFT_Y[0] - 20},-35,{DRAWN_X + 1},{RIGHT_Y[1] + 45},{top_z + 40}", "width": 1600, "height": 1000, "path": os.path.join(OUT, "end.png")})
    mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{HANGER_X[0][0] - 2}", "fit": f"{HANGER_X[0][0] - 2},-90,-35,{HANGER_X[0][0]},120,{top_z + 40}", "width": 1300, "height": 1100, "path": os.path.join(OUT, "head.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "sled.pdf"),
                              "note": f"stick sled: a lift plate and a hanging frame on two hinges, three motors, {len(rows)} rows a stick; the cutter {bit_up:.1f} over the table"}))
    for title, name in (("Master", "master"), ("Lift block", "lift_block"), ("Swing block", "swing_block"), ("Swing base", "swing_base")):
        print(mcp.call("export", {"tab": tabs[title], "format": "stl", "path": os.path.join(OUT, f"{name}.stl")}))
    mcp.close()

    count = {k: sum(1 for r in rows if r["kind"] == k) for k in ("set", "rough", "finish")}
    numbers = {
        "edge": EDGE, "bulge": BULGE, "gap": GAP, "bit": P.BIT, "taper": P.TAPER, "shank": P.SHANK, "ridge": P.RIDGE, "stick": STICK,
        "sweep": {"poses": len(tried), "lane": [lane_x, lane], "stepped": sim["stepped"]},
        "tile": {"across": tile["across"], "area": tile["area"], "outline": outline, "swing": swing_r, "axis_in_tile": centre, "circle": circle},
        "blank_swing": blank,
        "plan": {"passes": len(passes), "covered": covered, "worst": max(left), "orphans": orphans, "across_limit": P.Y_LIM, "face_min": P.FACE_MIN, "off_centre": P.OFF_CENTRE,
                 "finish_runs": sum(q["bites"] for q in passes), "deepest": max(q["depth"] for q in passes), "ball_bite": P.BALL_BITE},
        "rough": {"bit": P.ROUGH_BIT, "bite": P.ROUGH_BITE, "step": P.ROUGH_STEP, "allow": P.ALLOW, "angles": P.ROUGH_ANGLES, "blank": P.BLANK, "places": len(places),
                  "runs": sum(q["bites"] for q in places), **waste},
        "bit_up": bit_up, "table_clear": TABLE_CLEAR, "pulley_r": PULLEY_R, "hanger_r": HANGER_R, "spindle_d": SPINDLE_D, "spindle_clear": SPINDLE_CLEAR, "move_clear": MOVE_CLEAR, "turn_past": FW.TURN_PAST_DEG, "straight": straight,
        "sled": {"h1": h1, "arm": ARM, "drop": DROP, "p1_y": P1_Y, "e1": E1, "l1": L1, "p2_z": p2_z, "e2": e2, "l2": L2, "lean": lean, "lean_most": LEAN, "swing": {"rail": RAIL, "rail_y": rail_y, "rail_z": rail_z, "carriage": CARRIAGE, "block": BLOCK, "unit_x": UNIT_X, "screw_x": SCREW2_X, "motor_y": motor2_y}, "tail_x": TAIL_X,
                 "c1_home": c1_home, "c2_home": c2_home, "safe": safe, "lift": {"rail_z": rail1_z, "block": BLOCK1, "unit_x": UNIT1_X, "screw_x": SCREW1_X, "screw_top": screw1_top, "post_y": POST_Y}, "x": SLED_X, "left_y": LEFT_Y, "right_y": RIGHT_Y, "ply": PLY,
                 "back_x": BACK_X, "run": RUN, "master_x": MASTER_X, "master_r": MASTER_R, "belt": [60, 16]},
        "drawn": {"row": rows[drawn]["n"], "x": DRAWN_X, "alpha": a, "beta": b, "axis": axis, "h2": h2, "c1": c1, "c2": c2, "psi": psi, "pin1": pin1, "pin2": pin2},
        "count": count, "rows": rows,
        "bodies": {k: [list(v) for v in b_] for k, b_ in bodies.items()},
    }
    with open(os.path.join(OUT, "sled.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"sled {SLED_X[1] - SLED_X[0]:.0f} x {RIGHT_Y[1] - LEFT_Y[0]:.0f}, {top_z:.0f} high; rows: {count}")


if __name__ == "__main__":
    main()
