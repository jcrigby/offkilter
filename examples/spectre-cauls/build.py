"""Clamping cauls for one tile stick of the hex-edged Spectre: the
sticks of a tile (lattice triangles and their halves, from the
spectre-mosaic cover) are laid lengthwise into a printed cradle whose
pocket is the tile's outline, glued, and a printed plunger lid is
pressed down on them, so the tile comes out of the cauls as one solid
stick with no resin in it; the stick is sliced into tiles, and the
tiles are laid out on a printed template at their spacing, taped, and
glued down to a substrate, the dark glue or a coat of dark resin in
the lines between them. The tiles are to stand apart by the line, so
the tile stick is the tile eroded by half of it, and the sticks along
its boundary are the lattice shapes cut back by that much: the cut
list of them is part of the answer. The
outline is turned and split so the lid descends onto the bundle
without catching: above the split the outline must only narrow going
up, inside the chord, since the lid is a negative of that part sliding
down between the cradle's vertical walls; below the split the pocket
may have any bay, because the sticks go in one at a time from above.
The search is over every degree of turn and every split height at a
corner, for the split that gives the lid the most of the tile.

    cargo build --release -p ok-mcp
    python3 examples/spectre-cauls/build.py
    cargo test -p ok-render --test spectre_cauls   # what CI runs

Outputs under out/: cauls.okpart (base, lid and a tile stick in
place), base.stl and lid.stl (short cauls; three along a stick), the
pictures, cauls.pdf, cauls.json (the turn, the split, the numbers and
the cut list), and template.okpart and template.stl, the template the
sliced tiles are laid out on, with its picture.
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
EDGE = 10.56                                    # the tile's edge, as the mosaic lid's
CLEAR = 0.1                                     # the pocket past the tile's outline, each side: the saw's error over a few sticks
PLAY = 0.15                                     # the plunger short of the cradle's walls, each side
WALL, FLOOR = 6.0, 6.0                          # the cradle round the pocket
GUIDE = 8.0                                     # the cradle's walls above the bundle's top, which guide the plunger
FLANGE = 12.0                                   # the plunger above the walls when it is down, for the clamps
LENGTH = 50.0                                   # a short caul; three along a 200 mm stick
LIFT = 12.0                                     # the lid drawn this far up, off the bundle
SIDE, GAP, INNER = 4.0 * IN, 1.5, 0.1           # the 4 in lid; the line between tiles; the glue line between a tile's sticks
POCKET = 3.0                                    # the template's pockets, deep enough to hold a 10 mm tile for the tape
SHIFT = [2.0, 5.0]


def rot(p, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return (p[0] * c - p[1] * s, p[0] * s + p[1] * c)


def area(poly):
    return 0.5 * sum(poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1] for i in range(len(poly)))


def offset(poly, d):
    """The polygon moved out by d (counter-clockwise; mitred corners)."""
    n = len(poly)
    out = []
    for i in range(n):
        p0, p1, p2 = poly[i - 1], poly[i], poly[(i + 1) % n]
        def nrm(a, b):
            dx, dy = b[0] - a[0], b[1] - a[1]
            k = math.hypot(dx, dy)
            return (dy / k, -dx / k)                 # outward for a ccw polygon
        n1, n2 = nrm(p0, p1), nrm(p1, p2)
        bx, by = n1[0] + n2[0], n1[1] + n2[1]
        k = bx * n1[0] + by * n1[1]
        out.append((p1[0] + bx * d / k, p1[1] + by * d / k))
    return out


def clip(poly, s, keep_below):
    """Sutherland-Hodgman against y = s; one piece when the chord is one."""
    out = []
    n = len(poly)
    for i in range(n):
        p, q = poly[i], poly[(i + 1) % n]
        pin = (p[1] <= s) if keep_below else (p[1] >= s)
        qin = (q[1] <= s) if keep_below else (q[1] >= s)
        if pin:
            out.append(p)
        if pin != qin:
            t = (s - p[1]) / (q[1] - p[1])
            out.append((p[0] + (q[0] - p[0]) * t, s))
    return out


def clip_half(poly, inside):
    """Sutherland-Hodgman against a half-plane given by `inside(p)` and
    the crossing found by bisection on the edge."""
    out = []
    n = len(poly)
    for i in range(n):
        p, q = poly[i], poly[(i + 1) % n]
        pin, qin = inside(p), inside(q)
        if pin:
            out.append(p)
        if pin != qin:
            a, b = (p, q) if pin else (q, p)
            for _ in range(60):
                m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
                if inside(m):
                    a = m
                else:
                    b = m
            out.append(a)
    return out


def crossings(poly, y):
    """The x's where the outline crosses height y, sorted."""
    xs = []
    n = len(poly)
    for i in range(n):
        p, q = poly[i], poly[(i + 1) % n]
        if (p[1] > y) != (q[1] > y):
            xs.append(p[0] + (y - p[1]) * (q[0] - p[0]) / (q[1] - p[1]))
    return sorted(xs)


def lid_able(poly, s):
    """Whether the part above y = s only narrows going up, within the chord."""
    top = max(p[1] for p in poly)
    xs = crossings(poly, s + 1e-6)
    if len(xs) != 2:
        return False
    left, right = xs
    steps = 200
    for k in range(1, steps):
        y = s + (top - s) * k / steps
        c = crossings(poly, y)
        if len(c) != 2:
            return False
        if c[0] < left - 1e-6 or c[1] > right + 1e-6:
            return False
        left, right = c
    return True


def chord_at(poly, s):
    return crossings(poly, s + 1e-6)


def choose(outline):
    """The turn and the split that give the lid the most of the tile."""
    best = None
    for deg in range(360):
        poly = [rot(p, deg) for p in outline]
        ys = sorted(set(round(p[1], 6) for p in poly))
        for s in ys[:-1]:
            if len(crossings(poly, s + 1e-6)) != 2 or len(crossings(poly, s - 1e-6)) != 2:
                continue
            if not lid_able(poly, s):
                continue
            upper = clip(poly, s, keep_below=False)
            share = area(upper) / area(poly)
            if best is None or share > best[0] + 1e-9:
                best = (share, deg, s)
            break                                   # the lowest feasible split at this turn
    return best


def ring(poly, s, lower):
    """The clipped polygon's vertices from one chord end round to the
    other: for the lower part from the left end round the bottom to
    the right, for the upper from the right round the top to the left."""
    part = clip(poly, s, keep_below=lower)
    part = [p for i, p in enumerate(part) if math.hypot(p[0] - part[i - 1][0], p[1] - part[i - 1][1]) > 1e-6]
    ends = [i for i, p in enumerate(part) if abs(p[1] - s) < 1e-6]
    assert len(ends) == 2, ends
    i, j = ends
    n = len(part)
    # The chord is one edge of the clipped polygon: start just past it
    # and go the long way round, so the sequence runs from one chord
    # end to the other without the chord itself.
    if j == (i + 1) % n:
        seq = part[j:] + part[:j]
    elif i == (j + 1) % n:
        seq = part[i:] + part[:i]
    else:
        raise AssertionError(f"the chord is not one edge: ends at {i} and {j} of {n}")
    if lower and seq[0][0] > seq[-1][0]:
        seq = list(reversed(seq))
    if not lower and seq[0][0] < seq[-1][0]:
        seq = list(reversed(seq))
    return seq


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "cauls.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    # The tile stick is the tile eroded by half the line between tiles.
    tile = json.loads(mcp.call("spectre_tile", {"edge": EDGE, "curve": "hex", "gap": GAP}))
    outline = [tuple(p) for p in tile["outline"]]
    if area(outline) < 0:
        outline.reverse()
    share, deg, s = choose(outline)
    poly = [rot(p, deg) for p in outline]
    loose = offset(poly, CLEAR)
    bottom, top = min(p[1] for p in poly), max(p[1] for p in poly)
    left, right = chord_at(loose, s)
    xlo, xhi = min(p[0] for p in loose), max(p[0] for p in loose)
    print(f"turned {deg} degrees, split at y = {s:.2f} ({(s - bottom):.1f} up of {top - bottom:.1f}): the lid takes {100 * share:.0f} % of the tile; chord {right - left:.2f} wide")
    # The sticks of one tile with the glue line inside and the half line
    # outside: the boundary sticks come out cut back, in a few shapes.
    lid_sticks = json.loads(mcp.call("spectre_mosaic", {"width": SIDE, "height": SIDE, "edge": EDGE, "gap": GAP, "inner": INNER, "min": 1.0, "shift": SHIFT}))
    first = min(p["tile"] for p in lid_sticks["pieces"])
    shapes = {}
    for piece in (p for p in lid_sticks["pieces"] if p["tile"] == first):
        o = piece["outline"]
        sides = sorted(round(math.hypot(o[(i + 1) % len(o)][0] - o[i][0], o[(i + 1) % len(o)][1] - o[i][1]), 2) for i in range(len(o)))
        key = (piece["shape"], tuple(sides))
        shapes[key] = shapes.get(key, 0) + 1
    cut_list = sorted(({"shape": k[0], "sides": list(k[1]), "count": n} for k, n in shapes.items()), key=lambda r: (-r["count"], r["shape"]))
    print(f"one tile: {sum(r['count'] for r in cut_list)} sticks in {len(cut_list)} shapes:")
    for r in cut_list:
        print(f"  {r['count']:>3} {r['shape']:<10} " + " x ".join(f"{x:.2f}" for x in r["sides"]))
    walls_top = top + GUIDE
    mcp.call("create_document", {"name": "cauls"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Spectre tile stick cauls"}, {"type": "rename_tab", "tab": 1, "name": "Base"}]})
    # The base: a block round the pocket. The pocket is the part of the
    # outline below the split, exactly, and above the split a slot the
    # outline's full width with vertical walls, which the lid slides in.
    base = Part(mcp, 1, "Base")
    sk = base.sketch("top", 0.0, "block")
    base.rect(sk, (xlo - WALL, bottom - FLOOR), (xhi + WALL, walls_top))
    base.extrude(sk, LENGTH, name="cradle")
    sk = base.sketch("top", LENGTH + 1.0, "pocket")
    seq = ring(loose, s, lower=True)
    base.polygon(sk, seq + [(right, s), (xhi, s), (xhi, walls_top + 1.0), (xlo, walls_top + 1.0), (xlo, s), (left, s)])
    base.cut(sk, LENGTH + 2.0, direction="reverse", name="pocket, through")
    base_bodies = base.bodies()
    # The lid: a plunger the slot wide less the play, its underside the
    # negative of the part above the split, which only narrows going
    # up, so it comes down onto the bundle; beside that it stops a play
    # above the split, over the base's own material.
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "Lid"}]})
    lid_tab = int(re.search(r"tab (\d+)", text).group(1))
    lid = Part(mcp, lid_tab, "Lid")
    sk = lid.sketch("top", 0.0, "plunger")
    lid.rect(sk, (xlo + PLAY, s + PLAY + LIFT), (xhi - PLAY, walls_top + FLANGE + LIFT))
    lid.extrude(sk, LENGTH, name="plunger")
    sk = lid.sketch("top", LENGTH + 1.0, "negative")
    seq = ring(loose, s, lower=False)
    lid.polygon(sk, [(x, y + LIFT) for x, y in seq] + [(seq[-1][0], s + LIFT - 1.0), (seq[0][0], s + LIFT - 1.0)])
    lid.cut(sk, LENGTH + 2.0, direction="reverse", name="the tile's top, through")
    lid_bodies = lid.bodies()
    # The tile stick, for the picture: the tile in its pocket.
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "Tile stick"}]})
    stick_tab = int(re.search(r"tab (\d+)", text).group(1))
    stick = Part(mcp, stick_tab, "Tile stick")
    sk = stick.sketch("top", 0.0, "tile")
    stick.polygon(sk, poly)
    stick.extrude(sk, LENGTH, name=f"tile stick, {LENGTH:.0f} of 200")
    stick_bodies = stick.bodies()
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Cauls"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops = [{"type": "add_instance", "studio": t, "body": 0, "name": n, "fixed": True, "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}}
           for t, n in ((1, "Base"), (lid_tab, "Lid"), (stick_tab, "Tile stick"))]
    mcp.call("apply", {"ops": ops, "tab": asm})
    # The profile is in the top view: the cauls are extruded along z.
    for view, name in (("top", "section"), ("-0.55,-0.75,0.45", "iso")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A4", "note": f"cauls for the hex Spectre tile stick at {EDGE} mm: turned {deg} degrees, split {s - bottom:.1f} up, clearance {CLEAR}, plunger play {PLAY}; three of each along a 200 mm stick", "path": os.path.join(OUT, "cauls.pdf")}))
    for tab, name in ((1, "base"), (lid_tab, "lid")):
        print(mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, f"{name}.stl")}))
    mcp.close()
    # The template the sliced tiles are laid out on: a pocket per tile,
    # the lines between them standing as ridges.
    tpath = os.path.join(OUT, "template.okpart")
    if os.path.exists(tpath):
        os.remove(tpath)
    mcp = Mcp(tpath)
    mcp.call("create_document", {"name": "template"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Spectre hex layout template"}, {"type": "rename_tab", "tab": 1, "name": "Template"}]})
    top_part = Part(mcp, 1, "Template")
    top_part.feature({"type": "add_spectre", "plane": {"type": "standard", "base": "top", "offset": 0}, "width": SIDE, "height": SIDE, "edge": EDGE, "thickness": 10.0,
                      "gap": GAP, "curve": "hex", "bulge": 0.433, "bit": 0.0, "whole": True, "show": "template", "groove": POCKET, "shift": v2(*SHIFT), "name": "Template"})
    plate = top_part.bodies()
    mcp.call("screenshot", {"tab": 1, "view": "-0.3,-0.6,0.75", "width": 1400, "height": 1200, "path": os.path.join(OUT, "template.png")})
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "template.stl")}))
    mcp.close()
    numbers = {
        "edge": EDGE, "turn": deg, "split": s, "bottom": bottom, "top": top, "chord": [left, right], "lid_share": share,
        "clear": CLEAR, "play": PLAY, "wall": WALL, "floor": FLOOR, "guide": GUIDE, "flange": FLANGE, "length": LENGTH, "lift": LIFT,
        "tile_area": tile["area"], "outline": poly, "loose": loose,
        "pocket_area": area(clip(loose, s, True)) + (xhi - xlo) * (walls_top - s),
        # the part of the negative that is inside the plunger: the strip
        # under its bottom and anything past its sides are not cut from it
        "negative_area": area(clip_half(clip_half(clip_half(clip(loose, s, False), lambda p: p[0] >= xlo + PLAY), lambda p: p[0] <= xhi - PLAY), lambda p: p[1] >= s + PLAY)),
        "block": [[xlo - WALL, bottom - FLOOR], [xhi + WALL, walls_top]],
        "plunger": [[xlo + PLAY, s + PLAY], [xhi - PLAY, walls_top + FLANGE]],
        "gap": GAP, "inner": INNER, "pocket": POCKET, "cut_list": cut_list, "tiles": lid_sticks["tiles"],
        "bodies": {"base": base_bodies, "lid": lid_bodies, "stick": stick_bodies, "template": plate},
    }
    with open(os.path.join(OUT, "cauls.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"base {xhi - xlo + 2 * WALL:.0f} x {walls_top - bottom + FLOOR:.0f} x {LENGTH:.0f}, lid {xhi - xlo - 2 * PLAY:.0f} x {walls_top + FLANGE - s - PLAY:.0f} x {LENGTH:.0f}; template {len(plate)} body, {lid_sticks['tiles']} pockets")


if __name__ == "__main__":
    main()
