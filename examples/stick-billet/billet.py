"""The billet's planning: which of the stick sled's cuts can be made with
the stack held on four rods at its corners, which wait until three
printed fingers hold it instead, and where those fingers go. Pure
Python, no kernel: build.py draws what this returns.

The sled's own planner (examples/stick-sled/plan.py) gives the cuts:
roughing places for the straight bit and finishing passes for the
tapered ball tip, each (t, across, tip) as it has them, in the order
its table makes them. Nothing here changes a cut. A cut is only moved
from the first stage to the second, or, for a roughing place, made
shallower in the first and to its own depth in the second.

The section is the sled's: the tile's outline about the stick's axis,
in millimetres, the cuts coming from the direction t.
"""
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SLED = os.path.join(os.path.dirname(HERE), "stick-sled")
sys.path.insert(0, SLED)
import plan as P  # noqa: E402

# The billet's section.
GROW = 3.0                      # the blank, grown this much on every side from the sled's 49 x 42: 55 x 48, the owner's choice, so a rod fits in every corner
ROD = 3.505                     # a #6-32 brass rod, 0.138 in over the threads: the owner's "fine rod across the board"
INSET = (4.0, 4.0, 4.0, 7.0)    # each corner rod's centre in from both of the blank's faces at its corner. The fourth is 3 mm further in on both: there it lets the ball-nose finish the flute where a finger goes, which at 4 it could not
CLEAR = 1.0                     # no cut comes nearer a rod than this, nor nearer a finger in the second stage
TAB = 4.0                       # the wood left from each rod to the tile in the first stage, wide
WALL = 1.0                      # a finger's printed wall round its rod, at least
FIT = 0.15                      # a finger stands this far off the finished edge it bears on
NUT = 6.35 / math.sqrt(3.0)     # a small-pattern #6-32 hex nut, 1/4 in across its flats: its corners' radius, 3.67

STEP = 0.25                     # the grid the fingers and the wood left are followed on, mm


def grown(c, d):
    """A rectangle given by its four corners in order, grown by d on every side (the sled's build has the same)."""
    def unit(p, q):
        n = math.hypot(q[0] - p[0], q[1] - p[1])
        return ((q[0] - p[0]) / n, (q[1] - p[1]) / n)
    a, b = unit(c[0], c[1]), unit(c[0], c[3])
    return [(c[k][0] + d * (sa * a[0] + sb * b[0]), c[k][1] + d * (sa * a[1] + sb * b[1])) for k, (sa, sb) in enumerate(((-1, -1), (1, -1), (1, 1), (-1, 1)))]


def rods(stock):
    """The four rods' centres: each corner of the grown blank, in by its INSET along both faces."""
    out = []
    for k, q in enumerate(grown(stock, GROW)):
        inner = grown(stock, GROW - INSET[k])[k]
        out.append(inner)
    return out


def near_edge(q, outline):
    """How far q is from the tile's edge, less than nothing inside it."""
    m = min(P.near(q, outline[i - 1], outline[i]) for i in range(len(outline)))
    return -m if P.inside(q, outline) else m


def body(tool, t, y, tip, q, grow=0.0):
    """Whether the point q is inside the bit of this cut, the bit grown by `grow` all round (near enough: the
    straight bit's corner and the ball's are taken square)."""
    d, e = P.frame(t)
    off = abs(q[0] * e[0] + q[1] * e[1] - y)
    h = q[0] * d[0] + q[1] * d[1]
    if tool == "straight":
        return off <= P.ROUGH_BIT / 2 + grow and h >= tip - grow
    if off >= P.SHANK / 2 + grow:
        return False
    f = P.flank(min(max(0.0, off - grow), P.SHANK / 2 - 1e-9), P.BIT / 2)
    return h >= tip + P.BIT / 2 + f - grow


def disc(c, r, rings=3, n=36):
    """Points filling a disc, for asking whether a bit enters it."""
    pts = [c]
    for k in range(1, rings + 1):
        rr = r * k / rings
        pts += [(c[0] + rr * math.cos(2 * math.pi * j / n), c[1] + rr * math.sin(2 * math.pi * j / n)) for j in range(n)]
    return pts


def strip(a, b, w, step=0.25):
    """Points filling a strip w wide from a to b."""
    ln = math.dist(a, b)
    u = ((b[0] - a[0]) / ln, (b[1] - a[1]) / ln)
    pts = []
    for i in range(int(ln / step) + 1):
        s = min(ln, i * step)
        for j in range(9):
            o = -w / 2 + w * j / 8
            pts.append((a[0] + u[0] * s - u[1] * o, a[1] + u[1] * s + u[0] * o))
    return pts


def hits(cut, pts, grow=0.0):
    return any(body(cut["tool"], cut["t"], cut["across"], cut["tip"], q, grow) for q in pts)


def shallow(place, pts):
    """A roughing place made no deeper than leaves every point of `pts` uncut: its tip at the furthest of them that
    lies across the bit's width. The place itself if none do."""
    d, e = P.frame(place["t"])
    top = place["tip"]
    for q in pts:
        if abs(q[0] * e[0] + q[1] * e[1] - place["across"]) <= P.ROUGH_BIT / 2 + 1e-9:
            top = max(top, q[0] * d[0] + q[1] * d[1] + 0.05)
    return top


class Waste:
    """The waste, the grown blank less the tile, as the middles of P.CELL squares, followed cut by cut. The sled's
    P.bites does this for its own blank; this one takes any blank and is made in stages."""

    def __init__(self, outline, corners):
        lo_x, hi_x = min(q[0] for q in corners), max(q[0] for q in corners)
        lo_y, hi_y = min(q[1] for q in corners), max(q[1] for q in corners)
        self.cells = []
        for i in range(int((hi_x - lo_x) / P.CELL) + 1):
            for j in range(int((hi_y - lo_y) / P.CELL) + 1):
                q = (lo_x + (i + 0.5) * P.CELL, lo_y + (j + 0.5) * P.CELL)
                if P.inside(q, corners) and not P.inside(q, outline):
                    self.cells.append(q)
        self.alive = bytearray([1]) * len(self.cells)
        self.area = len(self.cells) * P.CELL * P.CELL

    def cut(self, tool, t, y, tip, dry=False):
        """What a cut takes: its area, how far the waste stood over it, and how far from the stick's axis the
        furthest of what it takes is."""
        d, e = P.frame(t)
        ball, r = tool == "ball", (P.BIT if tool == "ball" else P.ROUGH_BIT) / 2
        took, top, far = [], 0.0, 0.0
        for i, q in enumerate(self.cells):
            if not self.alive[i]:
                continue
            off = abs(q[0] * e[0] + q[1] * e[1] - y)
            h = q[0] * d[0] + q[1] * d[1]
            if ball:
                f = P.flank(off, r)
                if f is None or h < tip + r + f:
                    continue
                top = max(top, h - (tip + r + f))
            else:
                if off > r or h < tip:
                    continue
                top = max(top, h - tip)
            took.append(i)
            far = max(far, math.hypot(q[0], q[1]))
        if not dry:
            for i in took:
                self.alive[i] = 0
        return len(took) * P.CELL * P.CELL, top, far

    def left(self):
        return sum(self.alive) * P.CELL * P.CELL

    def bites(self, tool, q, depth, fits):
        """A place or pass taken down to its depth in equal bites: as few as the sled's own most-at-a-bite allows,
        and more if `fits(tool, area, over, far, last)` says a bite is too heavy for what holds the stick. The sled's
        table is planned so that none is; here a cut can meet more than the sled's did, where the first stage's
        roughing stopped short of a rod. Returns how many, and what each took."""
        most = P.BALL_BITE if tool == "ball" else P.ROUGH_BITE
        n = max(1, math.ceil(depth / most - 1e-9))
        while True:
            was, took, ok = bytearray(self.alive), [], True
            for b in range(1, n + 1):
                area, over, far = self.cut(tool, q["t"], q["across"], q["tip"] + depth * (n - b) / n)
                took.append((area, over, far))
                if fits is not None and not fits(tool, area, over, far, tool == "ball" and b == n):
                    ok = False
                    break
            if ok or n >= 40:
                return n, took, ok
            self.alive = was
            n += 1

    def run(self, places, passes, fits=None):
        """The sled's way: a roughing place that would take under a square millimetre is dropped, the others go down
        in bites; every finishing pass then takes what stands over it in bites. Returns the places kept and the
        passes, each with its depth, its bites and what each bite took."""
        kept = []
        for q in places:
            area, depth, _ = self.cut("straight", q["t"], q["across"], q["tip"], dry=True)
            if area < 1.0:
                continue
            n, took, ok = self.bites("straight", q, depth, fits)
            kept.append(dict(q, depth=depth, bites=n, area=area, took=took, ok=ok))
        done = []
        for q in passes:
            area, depth, _ = self.cut("ball", q["t"], q["across"], q["tip"], dry=True)
            n, took, ok = self.bites("ball", q, depth, fits)
            done.append(dict(q, depth=depth, bites=n, area=area, took=took, ok=ok))
        return kept, done


def bitten(q, tool):
    """A place or pass as the rows the machine makes, a bite each, the last at its own depth."""
    out = []
    for b in range(1, q["bites"] + 1):
        area, over, far = q["took"][b - 1]
        out.append({"tool": tool, "t": q["t"], "across": q["across"], "tip": q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"], "bite": b, "of": q["bites"],
                    "area": area, "over": over, "far": far})
    return out


def tabs(outline, centres, places, passes):
    """A tab from each rod to the tile, chosen one rod at a time: of the straight runs from the rod's centre to a
    point of the outline within 18 mm that meet the tile only at their end, the one that keeps back the fewest
    finishing passes, counted with the tabs already chosen. Returns each tab's far end, the points that fill rods and
    tabs, and which passes they keep back."""
    rod_pts = [disc(c, ROD / 2 + CLEAR) for c in centres]
    held = set(i for i, q in enumerate(passes) if any(hits(dict(q, tool="ball"), pts) for pts in rod_pts))
    ends, tab_pts = [], []
    for c in centres:
        best = None
        for k in range(0, len(outline), 3):
            o = outline[k]
            ln = math.dist(o, c)
            if ln > 18.0:
                continue
            end = (o[0] + (o[0] - c[0]) / ln * 0.5, o[1] + (o[1] - c[1]) / ln * 0.5)   # half a millimetre into the tile
            pts = strip(c, end, TAB)
            if any(P.inside(p, outline) for p in pts[:-9 * 4]):
                continue
            more = set(i for i, q in enumerate(passes) if i not in held and hits(dict(q, tool="ball"), pts))
            if best is None or len(more) < best[0]:
                best = (len(more), end, pts, more)
        ends.append(best[1])
        tab_pts.append(best[2])
        held |= best[3]
    return ends, rod_pts, tab_pts, held


def stages(outline, stock, places, passes, fits=None):
    """The two stages. The first: every roughing place made as deep as leaves the rods and the tabs, the finishing
    passes that do not touch them. The second: the roughing places to their own depth where the first stopped them
    short, the finishing passes the first left. Both in the sled's order."""
    centres = rods(stock)
    ends, rod_pts, tab_pts, held = tabs(outline, centres, places, passes)
    keep = [q for pts in rod_pts + tab_pts for q in pts]
    first_places, second_places = [], []
    for q in places:
        top = shallow(q, keep)
        first_places.append(dict(q, tip=top))
        if top > q["tip"] + 1e-6:
            second_places.append(dict(q))
    first_passes = [q for i, q in enumerate(passes) if i not in held]
    second_passes = [q for i, q in enumerate(passes) if i in held]
    waste = Waste(outline, grown(stock, GROW))
    p1, f1 = waste.run(first_places, first_passes, fits)
    after_first = bytearray(waste.alive)
    left_first = waste.left()
    p2, f2 = waste.run(second_places, second_passes, fits)
    return {"rods": centres, "tab_ends": ends, "rod_pts": rod_pts, "tab_pts": tab_pts, "waste": waste, "after_first": after_first,
            "first": (p1, f1), "second": (p2, f2), "left_first": left_first, "left_end": waste.left()}


# -- the grid: what is left after the first stage, and where the fingers go --------------------------------------

class Grid:
    """Squares of STEP over the grown blank, by their indices."""

    def __init__(self, corners):
        self.lo = (min(q[0] for q in corners) - 1.0, min(q[1] for q in corners) - 1.0)
        self.n = (int((max(q[0] for q in corners) + 1.0 - self.lo[0]) / STEP) + 1, int((max(q[1] for q in corners) + 1.0 - self.lo[1]) / STEP) + 1)
        self.corners = corners

    def at(self, i, j):
        return (self.lo[0] + (i + 0.5) * STEP, self.lo[1] + (j + 0.5) * STEP)

    def all(self):
        return ((i, j) for i in range(self.n[0]) for j in range(self.n[1]))


def wood_after_first(outline, plan, grid):
    """The squares of wood the first stage leaves: inside the tile, or inside the blank and reached by no cut of it."""
    cuts = [dict(q, tool="straight") for q in plan["first"][0]] + [dict(q, tool="ball") for q in plan["first"][1]]
    wood = set()
    for i, j in grid.all():
        q = grid.at(i, j)
        if not P.inside(q, grid.corners):
            continue
        if P.inside(q, outline) or not any(body(c["tool"], c["t"], c["across"], c["tip"], q) for c in cuts):
            wood.add((i, j))
    return wood


def joined(wood, start, grid):
    """The squares of wood joined to `start` side to side."""
    seen, todo = {start}, [start]
    while todo:
        c = todo.pop()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            m = (c[0] + dx, c[1] + dy)
            if m in wood and m not in seen:
                seen.add(m)
                todo.append(m)
    return seen


def cell_of(q, grid):
    return (int((q[0] - grid.lo[0]) / STEP), int((q[1] - grid.lo[1]) / STEP))


def finger_room(outline, plan, grid, wood):
    """The squares a finger may fill in the second stage: inside the blank, cut away in the first stage, outside the
    tile, and CLEAR from every cut of the second. Returns them as components, each a set."""
    cuts = [dict(q, tool="straight") for q in plan["second"][0]] + [dict(q, tool="ball") for q in plan["second"][1]]
    room = set()
    for i, j in grid.all():
        if (i, j) in wood:
            continue
        q = grid.at(i, j)
        if not P.inside(q, grid.corners) or near_edge(q, outline) < 0.05:
            continue
        if any(body(c["tool"], c["t"], c["across"], c["tip"], q, CLEAR) for c in cuts):
            continue
        room.add((i, j))
    comps, seen = [], set()
    for c in room:
        if c in seen:
            continue
        part = joined(room, c, grid)
        seen |= part
        comps.append(part)
    return comps, cuts


def spots(comp, grid, outline, cuts):
    """Where in a finger's room its rod can go: the rod and WALL round it inside the room, and the rod's nut, on the
    comb's plate at the near end, clear of every cut of the second stage (over the tile is clear: no cut goes there).
    The one nearest the tile."""
    best = None
    r = ROD / 2 + WALL
    reach = int(math.ceil(r / STEP))
    for c in comp:
        if any((c[0] + a, c[1] + b) not in comp for a in range(-reach, reach + 1) for b in range(-reach, reach + 1) if math.hypot(a, b) * STEP <= r):
            continue
        q = grid.at(*c)
        if any(hits(cut, disc(q, NUT + 0.2, rings=2, n=24)) for cut in cuts):
            continue
        d = near_edge(q, outline)
        if best is None or d < best[0]:
            best = (d, q)
    return best[1] if best else None


def bears(comp, grid, outline):
    """How much of the tile's edge a finger in this room bears on, roughly: the squares of it next to the edge."""
    return sum(1 for c in comp if near_edge(grid.at(*c), outline) < 0.4) * STEP


def fingers(outline, plan, grid, wood):
    """The three fingers: of the rooms that hold a rod and its nut, three with the stick's axis inside the triangle
    of their rods, the one whose finger that bears least bears most."""
    comps, cuts = finger_room(outline, plan, grid, wood)
    cand = []
    for comp in comps:
        if len(comp) * STEP * STEP < 10.0:
            continue
        s = spots(comp, grid, outline, cuts)
        if s is not None:
            cand.append({"cells": comp, "rod": s, "bears": bears(comp, grid, outline), "area": len(comp) * STEP * STEP})
    best = None
    for a in range(len(cand)):
        for b in range(a + 1, len(cand)):
            for c in range(b + 1, len(cand)):
                tri = [cand[a], cand[b], cand[c]]
                p = [f["rod"] for f in tri]
                side = [(p[(k + 1) % 3][0] - p[k][0]) * (0 - p[k][1]) - (p[(k + 1) % 3][1] - p[k][1]) * (0 - p[k][0]) for k in range(3)]
                if not (all(s > 0 for s in side) or all(s < 0 for s in side)):
                    continue
                worst = min(f["bears"] for f in tri)
                if best is None or worst > best[0]:
                    best = (worst, tri)
    return (best[1] if best else []), cand, cuts


# -- outlines of squares, for drawing --------------------------------------------------------------------------

def outlines(cells, grid):
    """The boundaries of a set of squares as closed loops of points, runs along one line merged: the outer ones
    anticlockwise, holes clockwise."""
    edges = {}
    for i, j in cells:
        for (a, b), (c, d), m in (((i, j), (i + 1, j), (i, j - 1)), ((i + 1, j), (i + 1, j + 1), (i + 1, j)),
                                  ((i + 1, j + 1), (i, j + 1), (i, j + 1)), ((i, j + 1), (i, j), (i - 1, j))):
            if m not in cells:
                edges.setdefault((a, b), []).append((c, d))
    loops = []
    while edges:
        start = next(iter(edges))
        loop, at, came = [start], start, None
        while True:
            outs = edges[at]
            if len(outs) > 1 and came is not None:
                # Two squares meet here only at a corner: turn left, which keeps each loop to its own squares.
                outs.sort(key=lambda m: came[0] * (m[1] - at[1]) - came[1] * (m[0] - at[0]))
            nxt = outs.pop()
            if not outs:
                del edges[at]
            came = (nxt[0] - at[0], nxt[1] - at[1])
            if nxt == start and start not in edges:
                break
            loop.append(nxt)
            at = nxt
        # merge straight runs
        merged = []
        n = len(loop)
        for k in range(n):
            a, b, c = loop[k - 1], loop[k], loop[(k + 1) % n]
            if (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) != 0:
                merged.append(b)
        loops.append([(grid.lo[0] + i * STEP, grid.lo[1] + j * STEP) for i, j in merged])
    return loops


def nut_need(poly, t, across, tip, nut_r, air):
    """How far a bit has to stand out of the collet nut for the nut to pass `poly` by `air` at this cut: the sled's
    P.nut_need for any outline, not only its blank. None if nothing of it is within the nut's width."""
    d, e = P.frame(t)
    wide, top = nut_r + air, None
    for i in range(len(poly)):
        a, b = poly[i - 1], poly[i]
        va, vb = a[0] * e[0] + a[1] * e[1] - across, b[0] * e[0] + b[1] * e[1] - across
        lo, hi = 0.0, 1.0
        if abs(vb - va) < 1e-12:
            if abs(va) > wide:
                continue
        else:
            s0, s1 = sorted(((-wide - va) / (vb - va), (wide - va) / (vb - va)))
            lo, hi = max(lo, s0), min(hi, s1)
            if lo > hi:
                continue
        for s in (lo, hi):
            u = (a[0] + s * (b[0] - a[0])) * d[0] + (a[1] + s * (b[1] - a[1])) * d[1]
            top = u if top is None else max(top, u)
    return None if top is None else top - tip + air


def area(poly):
    return sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(poly, poly[1:] + poly[:1])) / 2


def offset(outline, by):
    """The tile's outline moved `by` outward (inward if less than nothing). Each point goes along its normal; where
    that brings it nearer another stretch of the edge than `by`, at a sharp corner, it is dropped, so the line does
    not cross itself there."""
    ns = P.normals(outline)
    moved = [(q[0] + by * n[0], q[1] + by * n[1]) for q, n in zip(outline, ns)]
    return [q for q in moved if abs(near_edge(q, outline) - by) < 0.02]
