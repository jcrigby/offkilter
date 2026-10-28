#!/usr/bin/env python3
"""A cage for routing a dry stack of Spectre pieces on the duplicator,
built through the MCP server. Every tile of a top is the same shape, so
the top's pieces are cut as one prism: thirty hex blanks of veneer on a
ply backer, stacked dry in a hexagonal tube and clamped end to end,
the tube's six sides held by rods along its corners and removable one
at a time so a vertical bit can rout the profile through each window
with the master indexed in the same hex, and each side put back with
fins that fill what was cut, so the stack stays supported. Nothing is glued across the stack: when the
rods come out, the pieces fall apart finished.

    cargo build --release -p ok-mcp
    python3 examples/spectre-cage/build.py
    cargo test -p ok-render --test spectre_cage    # what CI runs

Writes out/spectre_cage.okpart, screenshots, the printable parts as
STL (each master with its hex ends and its six finned panels, an end
plug, a plain panel, the flat and the V cradle), the stack order, and
out/cage.json with the numbers the test checks.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4
# The top the pieces are for: the spectre-box's 4 in square at a 9 mm
# edge, arcs of sagitta 0.3, but with a resin gap between the pieces
# and four woods, no two neighbours alike.
SIDE, THICKNESS = 4.0 * IN, 3.0                 # the top: veneer on a backer, 3 mm all up
EDGE, CURVE, BULGE = 9.0, "arc", 0.3
BIT = 3.0 / 16.0 * IN                           # the duplicator's bit and pilot, 3/16 in either way
GAP = 0.7                                       # the gap between the pieces, left open and dark
# Two masters for the same cage and stack. The curved tile needs a
# ball-nose bit, which follows the arcs and leaves its own radius in the
# inside corners, so the master carries those fillets and, for the knob
# template-routed pieces have, every convex corner rounded to the bit
# too (`round` 0 keeps the Spectre's corners). The straight tile is cut
# with a square-end bit: every edge of it lies on the 30 degree grid,
# so at one of the twelve indices it stands parallel to the bit or flat
# to its floor, and the bit makes it exactly, sharp corners and all.
MASTERS = {
    "curved": dict(curve=CURVE, bulge=BULGE, bit=BIT, round=BIT / 2, shape="ball"),
    "straight": dict(curve=CURVE, bulge=0.0, bit=0.0, round=0.0, shape="flat"),
}
SHORT = 0.5                                     # an edge shorter than this (the gap's rounding of a corner) a square-end bit leaves as a corner
SCRAP = 40.0
WOODS = [("Maple", 0.7), ("Walnut", 0.64), ("Cherry", 0.6), ("Ebony", 1.1)]
# The stack: one laminate per piece, veneer glued to its own backer.
VENEER, BACKER = 1.5, 1.5
LAM = VENEER + BACKER
SPARES = 2
# The cage. Blanks are hexes across `F` (set from the master's circle
# and a margin). Six panels make the tube, their edges mitred so they
# meet at the corners, and six M4 rods along the corners outside hold
# them in; an end plug at each end, a hex boss into the tube and a
# flange the rods pull on. There are no rails: anything inside the hex
# at a corner would be where the bit has to reach the blank's corners.
MARGIN = 3.0                                    # blank hex past the master's circle, each side
PANEL_T = 2.0                                   # the panels: their inner faces are the blank hex
MITRE_CLEAR = 0.15                              # off each mitred edge, so the panels never bind
ROD, ROD_HOLE, NUT_AF, NUT_T = 4.0, 4.3, 7.0, 3.2   # M4 rods in the V between two panels' outer faces at each corner
BOSS, FLANGE = 15.0, 10.0                       # the end plugs: a hex boss into the tube, a flange outside it the rods pull on
MASTER_END = 25.0                               # the master's hex ends, the same hex as the flanges, for the same cradles
CRADLE_L, CRADLE_FLOOR = 24.0, 10.0             # a cradle under each flange
# Registration cones: a 90 degree cone let into each rim of every cradle
# at its mid-length, where the duplicator's bit and stylus are dropped,
# not turning, to set the stylus to the bit. The master's cradles and
# the cage's are the same print, so the cones sit the same off each
# trough's axis; a ball-nose tip or a flat-ended rod centres in a cone.
CONE_D, CONE_ANGLE, CONE_HOLE = 8.0, 90.0, 1.0  # 8 across at the rim, a 1 mm hole at the bottom so the cone is cut, not a point
CLEAR = 0.3                                     # fins stop this short of the profile, and of each other at the sector lines
FIN_STEP = 0.25                                 # fins are sampled this far apart across the window
FIN_MIN = 1.5                                   # a fin lobe thinner than this at its thickest is left off


def rot(q, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return (q[0] * c - q[1] * s, q[0] * s + q[1] * c)


def hexagon(across, phase=0.0):
    """Corners of a hexagon `across` flats, a flat on top, counter-clockwise from the right."""
    r = across / math.sqrt(3.0)
    return [rot((r, 0.0), 60.0 * k + phase) for k in range(6)]


def ray_hit(poly, origin, direction, skip=-1):
    """Distance along the ray to the polygon's nearest edge, or None."""
    best = None
    n = len(poly)
    for i in range(n):
        if i == skip:
            continue
        a, b = poly[i], poly[(i + 1) % n]
        ex, ey = b[0] - a[0], b[1] - a[1]
        denom = direction[0] * ey - direction[1] * ex
        if abs(denom) < 1e-12:
            continue
        wx, wy = a[0] - origin[0], a[1] - origin[1]
        t = (wx * ey - wy * ex) / denom
        u = (wx * direction[1] - wy * direction[0]) / denom
        if t > 1e-9 and -1e-9 <= u <= 1 + 1e-9 and (best is None or t < best):
            best = t
    return best


class Hex:
    """The blank hex `F` across flats with its windows: face k's outward
    normal is at 60k + 30 degrees (k = 1 on top), its tangent 90 degrees
    behind that, and the window a panel leaves is the whole face, `w`
    wide, since the neighbours' mitred edges lean away from it."""

    def __init__(self, F):
        self.F = F
        self.side = F / math.sqrt(3.0)
        self.apothem = F / 2.0
        self.w = self.side - 2.0 * MITRE_CLEAR

    def frame(self, k):
        n = rot((1.0, 0.0), 60.0 * k + 30.0)
        t = (n[1], -n[0])
        return t, n


def indices(hx):
    """The twelve ways the cage meets a vertical bit: on the flat cradle
    a panel's window faces straight up (`d` is its normal); on the V
    cradle the hex rests on a corner and the window either side of the
    corner on top is 30 degrees off vertical, so the bit comes down a
    corner's direction through one of two panels. (direction, open
    panel, label)."""
    out = []
    for k in range(6):
        out.append((hx.frame(k)[1], k, f"flat {k + 1}"))
    for j in range(6):
        d = rot((1.0, 0.0), 60.0 * j)
        out.append((d, j, f"V {j + 1} by panel {j + 1}"))
        out.append((d, (j - 1) % 6, f"V {j + 1} by panel {j}" if j else "V 1 by panel 6"))
    return out


def column_clear(outline, panels, open_k, c, d, r, skip):
    """Whether a bit of radius `r` standing at `c` can be drawn up
    direction `d` out of the open window: the column its body sweeps,
    its centre line and its two sides, started a hair off the surface so
    a side ray does not graze the facet it is tangent to, meets neither
    the profile nor a panel still in place."""
    t = (d[1], -d[0])
    for off in (0.1 - r, 0.0, r - 0.1):
        o = (c[0] + off * t[0] + 0.05 * d[0], c[1] + off * t[1] + 0.05 * d[1])
        if ray_hit(outline, o, d, skip=skip) is not None:
            return False
        for k, poly in enumerate(panels):
            if k != open_k and ray_hit(poly, o, d) is not None:
                return False
    return True


def reach(outline, hx, bit, shape):
    """Which of the outline's edges a vertical bit can cut at some index.
    A ball-nose bit cuts an edge that does not face away from it when,
    with the ball on the edge, the column its shank sweeps up to the
    window is clear of the profile and of the five panels still in
    place. A square-end bit cuts only an edge parallel to it, with its
    side, the same column beside the edge clear; or an edge square to
    it facing up, with its floor, the column above it clear with the
    bit anywhere over the edge; an edge shorter than `SHORT` (the gap's
    rounding of a corner) it leaves as the corner. Returns (fraction of
    the perimeter reachable, length reachable per index, unreachable
    edge midpoints)."""
    n = len(outline)
    r = bit / 2.0
    idx = indices(hx)
    panels = [panel_polygon(hx, k) for k in range(6)]
    per = [0.0] * len(idx)
    total = covered = 0.0
    missed = []
    for i in range(n):
        a, b = outline[i], outline[(i + 1) % n]
        ex, ey = b[0] - a[0], b[1] - a[1]
        length = math.hypot(ex, ey)
        if length < 1e-9:
            continue
        m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
        e = (ex / length, ey / length)
        ne = (ey / length, -ex / length)        # outward: the interior is left of a ccw edge
        total += length
        ok = shape == "flat" and length < SHORT
        for w, (d, open_k, _) in enumerate(idx):
            facing = ne[0] * d[0] + ne[1] * d[1]
            along = abs(e[0] * d[0] + e[1] * d[1])
            if shape == "ball":
                if facing < -0.02:
                    continue                    # an overhang from this direction
                c = (m[0] + r * ne[0], m[1] + r * ne[1])   # the bit's centre, the ball on the edge
                hit = column_clear(outline, panels, open_k, c, d, r, i)
            elif along > 0.9999:
                c = (m[0] + r * ne[0], m[1] + r * ne[1])   # the bit beside the wall
                hit = column_clear(outline, panels, open_k, c, d, r, i)
            elif facing > 0.9999:
                t = (d[1], -d[0])
                hit = any(
                    column_clear(outline, panels, open_k, (m[0] + off * t[0], m[1] + off * t[1]), d, r, i)
                    for off in (-r + 0.1, -r / 2, 0.0, r / 2, r - 0.1)
                )
            else:
                continue
            if hit:
                ok = True
                per[w] += length
        if ok:
            covered += length
        else:
            missed.append(m)
    return covered / total, per, missed


def fin_lobes(outline, hx, k):
    """The fin for window k: the window's column above the profile as a
    bit from that window leaves it, kept to the hex's sector for that
    face so the six fins never meet, `CLEAR` short of the profile every
    way and of the sector lines, as polygons in the profile plane. The
    top edge sits on the panel's inner face, the blank hex itself. The
    column is sampled across the window and at every vertex of the
    profile, a hair either side, so a wall between samples is seen, and
    the clearance is taken as the least depth within `CLEAR` across,
    so the fin stands off a wall sideways too."""
    t, n = hx.frame(k)
    a = hx.apothem
    half = hx.w / 2 - 0.2
    xs = {-half + i * FIN_STEP for i in range(int(2 * half / FIN_STEP) + 1)} | {half}
    for q in outline:
        x = q[0] * t[0] + q[1] * t[1]
        for dx in (-0.01, 0.01):
            if -half < x + dx < half:
                xs.add(x + dx)
    xs = sorted(xs)
    raw = []
    for x in xs:
        o = (x * t[0] + a * n[0], x * t[1] + a * n[1])
        hit = ray_hit(outline, o, (-n[0], -n[1]))
        sector = a * (1.0 - abs(x) / (hx.side / 2))
        raw.append(min(hit if hit is not None else math.inf, sector))
    depth = []
    lo = 0
    for i, x in enumerate(xs):
        while xs[lo] < x - CLEAR:
            lo += 1
        hi = i
        while hi + 1 < len(xs) and xs[hi + 1] <= x + CLEAR:
            hi += 1
        depth.append(max(min(raw[lo:hi + 1]) - CLEAR, 0.0))
    lobes, run = [], []
    for x, d in zip(xs, depth):
        if d > 0.0:
            run.append((x, d))
        elif run:
            lobes.append(run)
            run = []
    if run:
        lobes.append(run)
    polys = []
    for run in lobes:
        if max(d for _, d in run) < FIN_MIN or run[-1][0] - run[0][0] < 1.0:
            continue
        pts = [(run[0][0], 0.0), (run[-1][0], 0.0)] + [(x, d) for x, d in reversed(run)]
        polys.append([(x * t[0] + (a - d) * n[0], x * t[1] + (a - d) * n[1]) for x, d in pts])
    return polys


def prism(p, name, pts, x0, x1, material=None):
    """A polygon in the profile plane (y, z) extruded along x, the body
    named after the feature (a boolean keeps its first target's name)."""
    s = p.sketch("right", x0, name)
    p.polygon(s, pts)
    f = p.extrude(s, x1 - x0, name=name)
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        ops.append({"type": "set_part_material", "source": f, "material": material})
    p.apply(*ops)
    return f


def xcyl(p, name, cy, cz, x0, x1, d):
    s = p.sketch("right", x0, name)
    p.circle(s, (cy, cz), d / 2)
    f = p.extrude(s, x1 - x0, name=name)
    p.apply({"type": "rename_part", "source": f, "name": name})
    return f


def union(p, target, tools, name=None):
    return p.feature({"type": "add_boolean", "op": "union", "targets": [target], "tools": tools, "name": name})


def subtract(p, target, tools, name=None):
    return p.feature({"type": "add_boolean", "op": "subtract", "targets": [target], "tools": tools, "name": name})


def panel_polygon(hx, k):
    """Panel k's section: a plate on the window, its edges mitred at 60
    degrees to meet its neighbours at the corners, a hair short of them."""
    t, n = hx.frame(k)
    half = hx.side / 2 - MITRE_CLEAR
    lean = PANEL_T * math.tan(math.radians(30.0))
    pts = [(-half, 0.0), (half, 0.0), (half + lean, PANEL_T), (-half - lean, PANEL_T)]
    return [(x * t[0] + (hx.apothem + d) * n[0], x * t[1] + (hx.apothem + d) * n[1]) for x, d in pts]


def rod_centre(hx, k):
    """Rod k in the V between the outer faces of panels k - 1 and k, touching both."""
    r_out = (hx.F + 2 * PANEL_T) / math.sqrt(3.0) + (ROD / 2) / math.sin(math.radians(60.0))
    return rot((r_out, 0.0), 60.0 * k)


def cradle(p, name, F_out, x0, kind="flat"):
    """A block the hex ends rest in: `flat` has the lower half of the hex
    let into its top, a flat down, and indexes the cage at every 60
    degrees; `v` is a 120 degree V the hex sits in on a corner, which
    indexes it 30 degrees from those."""
    r = F_out / math.sqrt(3.0)
    if kind == "flat":
        outer = hexagon(F_out)
        trough = [outer[3], outer[4], outer[5], outer[0], (outer[0][0], 1.0), (outer[3][0], 1.0)]
        width, depth = 2 * r + 24.0, F_out / 2
    else:
        c, hw = r * math.cos(math.radians(30.0)), r / 2
        trough = [(0.0, -r), (c, -hw), (c, -hw + 1.0), (-c, -hw + 1.0), (-c, -hw)]
        width, depth = 2 * c + 24.0, r
    s = p.sketch("right", x0, name)
    p.rect(s, (-width / 2, -depth - CRADLE_FLOOR), (width / 2, -depth + (F_out / 2 if kind == "flat" else r / 2)))
    body = p.extrude(s, CRADLE_L, name=name)
    p.apply({"type": "rename_part", "source": body, "name": name})
    tool = prism(p, f"{name} trough", trough, x0 - 1.0, x0 + CRADLE_L + 1.0)
    body = subtract(p, body, [tool], name=name)
    # The registration cones, one in each rim at mid-length.
    rim = -depth + (F_out / 2 if kind == "flat" else r / 2)
    inner = r if kind == "flat" else c
    s = p.sketch("top", rim, f"{name} cones")   # on the rim itself: the countersink's diameter is at the sketch plane
    for sy in (-(inner + 6.0), inner + 6.0):
        p.point(s, (x0 + CRADLE_L / 2, sy))
    p.hole(s, CONE_HOLE, depth=CONE_D / 2 / math.tan(math.radians(CONE_ANGLE / 2)) + 1.5, direction="reverse",
           countersink={"diameter": CONE_D, "angle": CONE_ANGLE}, name=f"{name} registration cones")
    return body


def clock(outline0, hx, bit, shape):
    """The turn of the profile in the hex at which the twelve indices
    between them reach every edge of the outline with this bit."""
    best = None
    for phase in range(0, 30):
        outline = [rot(q, phase) for q in outline0]
        frac, per, missed = reach(outline, hx, bit, shape)
        score = (frac, min(per))
        if best is None or score > best[0]:
            best = (score, phase, outline, per, missed)
    (frac, _), phase, outline, per, missed = best
    return phase, outline, frac, per, missed


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_cage.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "spectre_cage"})
    # -- the masters: one tile each, as the pieces are routed -----------
    tiles = {}
    for name, cfg in MASTERS.items():
        tile = json.loads(mcp.call("spectre_tile", {"edge": EDGE, "curve": cfg["curve"], "bulge": cfg["bulge"], "bit": cfg["bit"], "gap": GAP, "round": cfg["round"]}))
        tiles[name] = tile
        print(f"{name} master: {len(tile['outline'])} points, {tile['across']:.2f} across, {tile['area']:.0f} of the tile's {tile['tile_area']:.0f} mm2")
    # One hex holds both, and the flanges and the masters' ends are a
    # hex whose corners hold the rods and their nuts.
    F = math.ceil((max(t["across"] for t in tiles.values()) + 2 * MARGIN) / 2.0) * 2.0
    hx = Hex(F)
    F_out = math.ceil((rod_centre(hx, 0)[0] + NUT_AF / math.sqrt(3.0) + 1.0) * math.sqrt(3.0) / 2.0) * 2.0
    print(f"blank hex {F:.0f} across flats, side {hx.side:.2f}, window {hx.w:.2f}; rods at {rod_centre(hx, 0)[0]:.1f}; flange hex {F_out:.0f}")
    # Clock each profile in the hex.
    clocked = {}
    for name, cfg in MASTERS.items():
        outline0 = [(q[0], q[1]) for q in tiles[name]["outline"]]
        phase, outline, frac, per, missed = clock(outline0, hx, BIT, cfg["shape"])
        clocked[name] = (phase, outline, frac, per)
        print(f"{name} clocked at {phase} degrees, {cfg['shape']} bit: {100 * frac:.1f}% of the outline reachable; per index " + ", ".join(f"{label} {x:.0f}" for (_, _, label), x in zip(indices(hx), per)) + " mm")
        if missed:
            print("  unreachable at " + ", ".join(f"({y:.1f}, {z:.1f})" for y, z in missed))
    # -- the top the pieces are for: how many, and of what -------------
    top = Part(mcp, 1, "Top")
    top.apply({"type": "rename_document", "name": "Spectre stack cage"}, {"type": "rename_tab", "tab": 1, "name": "Top"})
    top.feature(
        {
            "type": "add_spectre",
            "plane": {"type": "standard", "base": "top", "offset": 0},
            "width": SIDE,
            "height": SIDE,
            "edge": EDGE,
            "thickness": THICKNESS,
            "gap": GAP,
            "curve": CURVE,
            "bulge": BULGE,
            "bit": BIT,
            "scrap": SCRAP,
            "scheme": "four_map",
            "light": {"name": WOODS[0][0], "density": WOODS[0][1]},
            "dark": {"name": WOODS[1][0], "density": WOODS[1][1]},
            "third": {"name": WOODS[2][0], "density": WOODS[2][1]},
            "fourth": {"name": WOODS[3][0], "density": WOODS[3][1]},
            "name": "Spectre top",
        }
    )
    report = top.report()
    if report["features"][0].get("error"):
        raise RuntimeError(report["features"][0]["error"])
    pieces = [b for b in report["bodies"] if b["name"].startswith("Piece ")]
    stack = [b["material"]["name"] for b in pieces]
    counts = {w: stack.count(w) for w, _ in WOODS}
    stack += [max(counts, key=counts.get)] * SPARES
    N = len(stack)
    L = N * LAM
    print(f"top: {len(pieces)} pieces, " + ", ".join(f"{c} {w.lower()}" for w, c in counts.items()) + f"; stack of {N} with {SPARES} spares, {L:.0f} mm")
    # -- tabs ------------------------------------------------------------
    tabs = {}
    names = ["Cage", "Cradles"] + [f"Master {n}" for n in MASTERS] + [f"Fins {n}" for n in MASTERS]
    for name in names:
        text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": name}]})
        tabs[name] = int(re.search(r"tab (\d+)", text).group(1))
    x_panel = (-BOSS, L + BOSS)
    parts = []
    # Each master: the profile over the stack's length, a hex end at each
    # end; and its six finned panels round it, at the same place.
    fin_area = {}
    for name in MASTERS:
        outline = clocked[name][1]
        m = Part(mcp, tabs[f"Master {name}"], f"Master {name}")
        body = prism(m, "Master", outline, 0.0, L)
        ends = [prism(m, "Master end", hexagon(F_out), -MASTER_END, 0.0), prism(m, "Master end", hexagon(F_out), L, L + MASTER_END)]
        union(m, body, ends, name="Master")
        f = Part(mcp, tabs[f"Fins {name}"], f"Fins {name}")
        areas = []
        for k in range(6):
            panel = prism(f, f"Finned panel {k + 1}", panel_polygon(hx, k), *x_panel)
            area, lobes = 0.0, []
            for lobe in fin_lobes(outline, hx, k):
                lobes.append(prism(f, f"Fin {k + 1}", lobe, 0.5, L - 0.5))
                area += abs(sum(lobe[i][0] * lobe[(i + 1) % len(lobe)][1] - lobe[(i + 1) % len(lobe)][0] * lobe[i][1] for i in range(len(lobe)))) / 2
            if lobes:
                union(f, panel, lobes, name=f"Finned panel {k + 1}")
            areas.append(area)
        prism(f, "Master", outline, 0.0, L)
        fin_area[name] = areas
        print(f"{name} fins: " + ", ".join(f"{a:.0f}" for a in areas) + " mm2 of the window's sections")
        parts += [m, f]
    # The cage, loaded: plugs, rods, five plain panels (the top window
    # open), the stack inside, the cradles under the flanges.
    c = Part(mcp, tabs["Cage"], "Cage")
    rods = [rod_centre(hx, k) for k in range(6)]
    for k in range(6):
        if k == 1:
            continue
        prism(c, f"Panel {k + 1}", panel_polygon(hx, k), *x_panel)
    for end, (x0, x1) in (("A", (-BOSS - FLANGE, 0.0)), ("B", (L, L + BOSS + FLANGE))):
        bx = (x0 + FLANGE, x1) if end == "A" else (x0, x1 - FLANGE)
        fx = (x0, x0 + FLANGE) if end == "A" else (x1 - FLANGE, x1)
        plug = prism(c, f"Plug {end}", hexagon(F - CLEAR), *bx)
        flange = prism(c, f"Plug {end} flange", hexagon(F_out), *fx)
        union(c, plug, [flange], name=f"Plug {end}")   # the body keeps the boss's feature as its source
        holes = [xcyl(c, f"Plug {end} rod hole", rod[0], rod[1], fx[0] - 1.0, fx[1] + 1.0, ROD_HOLE) for rod in rods]
        subtract(c, plug, holes, name=f"Plug {end}")
    x_rod = (-BOSS - FLANGE - NUT_T - 4.0, L + BOSS + FLANGE + NUT_T + 4.0)
    for k, rod in enumerate(rods):
        xcyl(c, f"Rod {k + 1} M4", rod[0], rod[1], *x_rod, ROD)
        for end, x0 in (("A", -BOSS - FLANGE - NUT_T), ("B", L + BOSS + FLANGE)):
            prism(c, f"Nut {k + 1}{end}", [(rod[0] + q[0], rod[1] + q[1]) for q in hexagon(NUT_AF)], x0, x0 + NUT_T)
    densities = dict(WOODS)
    for i, wood in enumerate(stack):
        prism(c, f"Laminate {i + 1} {wood.lower()}", hexagon(F - CLEAR), i * LAM, (i + 1) * LAM, material={"name": wood, "density": densities[wood]})
    cradle(c, "Cradle A", F_out, -BOSS - FLANGE - (CRADLE_L - FLANGE) / 2)
    cradle(c, "Cradle B", F_out, L + BOSS - (CRADLE_L - FLANGE) / 2)
    # The two cradles on their own, to print: the masters' ends and the
    # cage's flanges sit in either.
    cr = Part(mcp, tabs["Cradles"], "Cradles")
    cradle(cr, "Flat cradle", F_out, 0.0, "flat")
    cradle(cr, "V cradle", F_out, CRADLE_L + 20.0, "v")
    for part in parts + [c, cr]:
        r = part.report()
        bad = [(ft["name"], ft["error"]) for ft in r["features"] if ft.get("error")]
        if bad:
            raise RuntimeError(f"{part.name}: {bad}")
        print(f"{part.name}: {len(r['bodies'])} bodies")
    # -- pictures and files ----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    export = lambda tab, body, name: mcp.call("export", {"tab": tab, "format": "stl", "body": body, "path": os.path.join(OUT, f"{name}.stl")})
    shot(tabs["Cage"], "cage_iso", view="iso")
    shot(tabs["Cage"], "cage_end", view="right")
    shot(tabs["Cradles"], "cradles", view="iso")
    shot(1, "top", view="top")
    export(tabs["Cage"], "Plug A", "plug")
    export(tabs["Cage"], "Panel 3", "panel")
    export(tabs["Cradles"], "Flat cradle", "cradle_flat")
    export(tabs["Cradles"], "V cradle", "cradle_v")
    for name, cfg in MASTERS.items():
        shot(tabs[f"Fins {name}"], f"fins_{name}_end", view="right")
        shot(tabs[f"Master {name}"], f"master_{name}_iso", view="iso")
        export(tabs[f"Master {name}"], "Master", f"master_{name}")
        for k in range(6):
            export(tabs[f"Fins {name}"], f"Finned panel {k + 1}", f"finned_panel_{name}_{k + 1}")
        print(mcp.call("export", {"tab": tabs[f"Fins {name}"], "format": "pdf", "sheet": "A3", "views": ["right", "front", "iso"], "parts": False,
                                  "note": f"Spectre {EDGE:g} mm edge, {name}, {cfg['shape']} bit {BIT:.2f}, hex {F:.0f}; fins {CLEAR} off the profile",
                                  "path": os.path.join(OUT, f"fins_{name}.pdf")}))
    with open(os.path.join(OUT, "stack.txt"), "w") as fh:
        fh.write(f"# {N} laminates of {LAM:g} mm ({VENEER:g} veneer on {BACKER:g} ply), hex {F:.0f} across flats, end A first\n")
        for i, wood in enumerate(stack):
            fh.write(f"{i + 1:3d}  {wood}\n")
    numbers = {
        "edge": EDGE, "bit": BIT, "gap": GAP,
        "F": F, "F_out": F_out, "panel_t": PANEL_T, "mitre_clear": MITRE_CLEAR, "clear": CLEAR, "rod": ROD, "rod_radius": rod_centre(hx, 0)[0],
        "laminate": LAM, "count": N, "length": L, "pieces": len(pieces), "counts": counts,
        "indices": [label for _, _, label in indices(hx)], "boss": BOSS, "flange": FLANGE, "master_end": MASTER_END, "short": SHORT,
        "masters": {
            name: {**cfg, "phase": clocked[name][0], "reach": clocked[name][2], "per_index": clocked[name][3], "fin_area": fin_area[name], "across": tiles[name]["across"]}
            for name, cfg in MASTERS.items()
        },
    }
    with open(os.path.join(OUT, "cage.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
