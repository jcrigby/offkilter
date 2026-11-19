#!/usr/bin/env python3
"""Finishing the stick the sled mills: two sanding forms and two scraper templates.

The stick sled (examples/stick-sled) leaves a stick whose section is
the tile, with ridges along it between one pass and the next. They run
with the grain and they are all the same way: along the stick. So a
form that is the negative of an edge, pushed along the stick with an
abrasive film stuck to it, takes the ridges off and cannot change the
shape. The owner's idea; the film is the owner's too, 3M's 268L.

The tile has fourteen edges, seven that bulge out (bumps) and seven
that go in (flutes), and the kernel draws every one as the same curve,
none of them mirrored. Cut, they are alike only away from their ends:
the tile is drawn 0.35 mm inside its edge for the gap between tiles,
and its corners are at different angles, so where a bump runs into a
flute differs from corner to corner. This lays the first bump on each
of the other six, and the first flute likewise, finds the stretch all
seven share, and draws one form for the bumps and one for the flutes
over that stretch.

It also draws the two blades a scraper would want, as plates to scribe
round on card scraper stock.

    cargo build --release -p ok-mcp
    python3 examples/stick-sled/build.py           # the tile comes from its out/sled.json
    python3 examples/stick-finish/build.py
    cargo test -p ok-render --test stick_finish

Writes out/finish.okpart (each form as it prints, the two templates,
and both forms on the stick), form_bump.stl and form_flute.stl,
template_bump.stl and template_flute.stl, templates.pdf at 1:1,
finish.png and section.png, and finish.json.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")
SLED = os.path.join(os.path.dirname(HERE), "stick-sled", "out", "sled.json")

# -- the numbers to set -------------------------------------------------------
# The abrasive: 3M 268L microfinishing film, aluminium oxide on a 3 mil polyester film with adhesive on its back. The
# owner has it in 100, 80 and 60 micron. 3M gives the backing, 0.076 mm, and not the whole: the mineral and its resin
# are about the grade's size again and the adhesive a few hundredths, so 0.17 to 0.21. One allowance for all three.
FILM = 0.20                     # the film, stuck on: an estimate until a sheet is under callipers
LONG = (80.0, 60.0)             # the bump form and the flute form, along the stick. The flute form prints standing on its end, so it is the shorter
SAME = 0.03                     # where the seven are this near each other they are the same curve: the outline is straight pieces, and this is what that costs
SHY = 0.25                      # a form stops this far short of each end of the stretch all seven share, round the curve
LIP = 1.5                       # the bump form's wall either side of its channel
BACK = 7.0                      # what the bump form has behind its channel's bottom, for fingers and stiffness
TAPER = 25.0                    # its walls lean in by this, degrees, going away from the stick: the next lobe round a corner is in a square back's way
TONGUE = 6.0                    # and stop leaning where the form is this wide
AIR = 0.7                       # the flute form's neck passes the lobes either side of a flute's mouth by this. A flute is wider inside than at its mouth, and at some corners the next lobe hooks over it: there is one way out that all seven share, and it is narrow
SHOULDER = 0.9                  # the neck is its own width from this far out of the mouth
GRIP = (14.0, 5.0)              # past the lobes it widens to this, across and thick: what is held
COVER = 0.6                     # a form has to cover this much of an edge, round the curve, or one form for seven is not worth making
CLEAR = 0.02                    # a form on any of the seven touches nothing but the film on its own edge, by this
PLATE = (12.0, 1.2)             # a blade template: how far out from the stick it goes, and thick. It is the end of a blade and no more: the stock's own size is not known yet, and nothing wider passes the lobes either side
LEVEL = 0.25                    # the way out of a flute is looked for at heights this far apart

# How the outline is read.
STEP = 0.05                     # the outline is walked in steps of this
TURN = 1e-4                     # a corner that turns less than this, radians, is straight: neither bump nor flute


# -- plane geometry, in pure Python -------------------------------------------------

def sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def area2(poly):
    return sum(poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1] for i in range(len(poly)))


def along(line):
    """How far each point of an open line is along it."""
    s = [0.0]
    for a, b in zip(line[:-1], line[1:]):
        s.append(s[-1] + dist(a, b))
    return s


def walked(line, step=STEP):
    """An open line again, in steps of about `step`."""
    out = [line[0]]
    for a, b in zip(line[:-1], line[1:]):
        m = max(1, int(math.ceil(dist(a, b) / step)))
        out += [(a[0] + (b[0] - a[0]) * j / m, a[1] + (b[1] - a[1]) * j / m) for j in range(1, m + 1)]
    return out


def between(line, lo, hi):
    """The part of an open line from `lo` to `hi` along it."""
    s, out = along(line), []
    for k in range(len(line) - 1):
        a, b, sa, sb = line[k], line[k + 1], s[k], s[k + 1]
        if sb <= lo or sa >= hi or sb - sa < 1e-12:
            continue
        t0, t1 = max(0.0, (lo - sa) / (sb - sa)), min(1.0, (hi - sa) / (sb - sa))
        p0 = (a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0)
        p1 = (a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1)
        if not out or dist(out[-1], p0) > 1e-9:
            out.append(p0)
        out.append(p1)
    return out


def nearest(q, line):
    """The point of an open line nearest q, and how far."""
    best = None
    for a, b in zip(line[:-1], line[1:]):
        d = sub(b, a)
        l2 = d[0] * d[0] + d[1] * d[1]
        t = 0.0 if l2 < 1e-18 else max(0.0, min(1.0, ((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / l2))
        p = (a[0] + d[0] * t, a[1] + d[1] * t)
        r = dist(q, p)
        if best is None or r < best[1]:
            best = (p, r)
    return best


def inside(q, poly):
    hit = False
    for i in range(len(poly)):
        a, b = poly[i], poly[(i + 1) % len(poly)]
        if (a[1] > q[1]) != (b[1] > q[1]) and q[0] < a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]):
            hit = not hit
    return hit


def pushed(line, by):
    """An open line moved `by` to its left as it is walked: for a profile walked left to right, away from the wood."""
    out = []
    for k, q in enumerate(line):
        a, b = line[max(k - 1, 0)], line[min(k + 1, len(line) - 1)]
        t = sub(b, a)
        n = math.hypot(*t)
        out.append((q[0] - by * t[1] / n, q[1] + by * t[0] / n))
    return out


def moved(m, q):
    """A point of an edge's own frame put on the tile: m is (mirrored, cos, sin, x, y)."""
    x = -q[0] if m[0] else q[0]
    return (m[1] * x - m[2] * q[1] + m[3], m[2] * x + m[1] * q[1] + m[4])


def laid(pts, target, turn, shift):
    """The move that lays `pts` (an edge's own frame) on the line `target` (the tile's) with the turn given: only
    the shift is found, by closest points, the mean of what they are off taken up each time. Every edge of the tile
    is a whole number of twelfths of a turn from every other, so the turn is known and nothing can slide round."""
    c, s = math.cos(turn), math.sin(turn)
    spun = [(c * q[0] - s * q[1], s * q[0] + c * q[1]) for q in pts]
    x, y = shift
    for _ in range(60):
        off = [sub(nearest((q[0] + x, q[1] + y), target)[0], (q[0] + x, q[1] + y)) for q in spun]
        dx, dy = sum(o[0] for o in off) / len(off), sum(o[1] for o in off) / len(off)
        x, y = x + dx, y + dy
        if math.hypot(dx, dy) < 1e-7:
            break
    return (False, c, s, x, y)


# -- the tile's edges ---------------------------------------------------------------

def edges_of(outline):
    """The outline, anticlockwise, cut where its curvature changes sign: bumps (+1) and flutes (-1) by turns, each
    with the indices of its points."""
    n = len(outline)

    def turn(i):
        a, b, c = outline[i - 1], outline[i], outline[(i + 1) % n]
        u, v = sub(b, a), sub(c, b)
        return math.atan2(u[0] * v[1] - u[1] * v[0], u[0] * v[0] + u[1] * v[1])
    sign = [0 if abs(turn(i)) < TURN else (1 if turn(i) > 0 else -1) for i in range(n)]
    first = next(i for i in range(n) if sign[i] and sign[i - 1] and sign[i] != sign[i - 1])
    runs, cur = [], [first]
    for j in range(1, n):
        i = (first + j) % n
        if sign[i] == 0 or sign[i] == sign[cur[0]]:
            cur.append(i)
        else:
            runs.append(cur)
            cur = [i]
    runs.append(cur)
    return [{"kind": sign[r[0]], "at": r} for r in runs]


def own_frame(outline, run):
    """An edge's own frame: its middle the origin, u along its chord, v out of the wood. As a move onto the tile."""
    a, b = outline[run["at"][0]], outline[run["at"][-1]]
    c = dist(a, b)
    ux, uy = (a[0] - b[0]) / c, (a[1] - b[1]) / c           # walked anticlockwise the wood is on the left: with u back along the chord, v is out
    return (False, ux, uy, (a[0] + b[0]) / 2, (a[1] + b[1]) / 2)


def into(m, q):
    """A point of the tile in an edge's own frame: `moved`, undone."""
    x, y = q[0] - m[3], q[1] - m[4]
    u, v = m[1] * x + m[2] * y, -m[2] * x + m[1] * y
    return (-u if m[0] else u, v)


def shapes(outline):
    """For bumps and for flutes: the first one's profile in its own frame, walked from left to right; the move that
    lays it on each of the seven; how far off each it is all along; and the stretch of it, about its middle, that is
    on all seven, which is what a form can be."""
    runs = edges_of(outline)
    if len(runs) != 14 or any(runs[k]["kind"] == runs[k - 1]["kind"] for k in range(14)):
        raise SystemExit(f"the outline is {len(runs)} runs of one curvature, not seven bumps and seven flutes by turns")
    n, out, twelfth = len(outline), {}, math.pi / 6
    for kind, name in ((1, "bump"), (-1, "flute")):
        mine = [r for r in runs if r["kind"] == kind]
        home = own_frame(outline, mine[0])
        profile = walked([into(home, outline[i]) for i in mine[0]["at"]][::-1])
        whole = along(profile)[-1]
        feel = between(profile, whole * 0.3, whole * 0.7)[::8]          # what is laid on each: its middle, where every edge is the curve the kernel drew
        centre = (sum(q[0] for q in feel) / len(feel), sum(q[1] for q in feel) / len(feel))
        places, off = [], [0.0] * len(profile)
        for r in mine:
            target = [outline[(r["at"][0] - 12 + j) % n] for j in range(len(r["at"]) + 24)]
            f = own_frame(outline, r)
            middle = moved(f, (0.0, sum(into(f, outline[i])[1] for i in r["at"]) / len(r["at"])))
            tries = []
            for k in range(12):                              # which twelfth: an edge's chord is not square enough to its curve to say
                turn = math.atan2(home[2], home[1]) + twelfth * k
                c, s_ = math.cos(turn), math.sin(turn)
                m = laid(feel, target, turn, (middle[0] - (c * centre[0] - s_ * centre[1]), middle[1] - (s_ * centre[0] + c * centre[1])))
                tries.append((max(nearest(moved(m, q), target)[1] for q in feel), m))
            m = min(tries, key=lambda t: t[0])[1]
            each = [nearest(moved(m, q), target)[1] for q in profile]
            off = [max(a, b) for a, b in zip(off, each)]
            places.append({"move": list(m), "off_whole": max(each), "chord": dist(outline[r["at"][0]], outline[r["at"][-1]])})
        # the stretch all seven share: out from the middle for as long as they are the same curve
        s, mid = along(profile), len(profile) // 2
        lo = hi = mid
        while lo > 0 and off[lo - 1] <= SAME:
            lo -= 1
        while hi < len(profile) - 1 and off[hi + 1] <= SAME:
            hi += 1
        if off[mid] > SAME or s[hi] - s[lo] - 2 * SHY < COVER * whole:
            raise SystemExit(f"the seven {name}s are one curve for only {s[hi] - s[lo]:.1f} of the {whole:.1f} round one: one form will not do")
        work = between(profile, s[lo] + SHY, s[hi] - SHY)
        for q in places:
            q["off"] = max(nearest(moved(q["move"], w), outline + [outline[0]])[1] for w in work)
        out[name] = {"profile": profile, "work": work, "round": whole, "shared": [s[lo], s[hi]], "covered": s[hi] - s[lo] - 2 * SHY, "places": places,
                     "wide": max(q[0] for q in profile) - min(q[0] for q in profile), "high": max(abs(q[1]) for q in profile)}
    return out


# -- the forms' sections, in an edge's own frame: u across, v out of the wood ----------------

def bump_section(work, film, back):
    """What goes over a bump: the bump's curve the film's thickness further out, a wall either side leaning in as it
    goes away from the stick, a back."""
    face = pushed(work, film)
    top = max(q[1] for q in face) + back
    mid, lean = (face[0][0] + face[-1][0]) / 2, math.tan(math.radians(TAPER))

    def wall(end, side):
        u, v = end[0] + side * LIP, end[1]
        up = (abs(u - mid) - TONGUE / 2) / lean                 # how far out the wall has leaned in to the tongue's width
        if v + up >= top:
            return [(u, v), (u - side * (top - v) * lean, top)]
        return [(u, v), (mid + side * TONGUE / 2, v + up), (mid + side * TONGUE / 2, top)]
    return face + wall(face[-1], 1) + wall(face[0], -1)[::-1], face


def way_out(outline, flute, reach):
    """The way out of a flute that all seven share, in the first one's own frame: the tile's outline laid back from
    each of the seven, and at each height over the mouth how far left and right of the middle the nearest wood of
    any of them is. Returns the heights and, for each, the open stretch."""
    cloud = [into(q["move"], w) for q in flute["places"] for w in walked(outline + [outline[0]], 0.1)]
    levels = []
    v = SHOULDER
    while v <= reach + 1e-9:
        near = [w[0] for w in cloud if abs(w[1] - v) <= LEVEL]
        left, right = max([u for u in near if u < 0.0], default=-99.0), min([u for u in near if u > 0.0], default=99.0)
        levels.append((v, left, right))
        v += LEVEL
    return levels


def flute_neck(levels, grip):
    """The neck: as wide as the way out is at its narrowest, less the air each side, from the mouth to where the way
    has opened to the grip's width and stays open for the grip's thickness."""
    for k, (v, _, _) in enumerate(levels):
        below = levels[:k + 1]
        left, right = max(q[1] for q in below) + AIR, min(q[2] for q in below) - AIR
        mid = (left + right) / 2
        above = [q for q in levels if v <= q[0] <= v + grip[1] + 2 * LEVEL]
        if above[-1][0] >= v + grip[1] and all(q[1] <= mid - grip[0] / 2 - AIR and q[2] >= mid + grip[0] / 2 + AIR for q in above):
            return left, right, v + LEVEL
    raise SystemExit("the way out of a flute never opens to the grip's width")


def flute_section(work, film, neck, grip=None):
    """What goes into a flute: the flute's curve the film's thickness further out, which is a bulb; a neck up the way
    out, far narrower than the bulb; and, on a form, a grip across the top. `neck` is its left, its right, and how
    far out of the mouth it goes."""
    face = pushed(work, film)
    left, right, top = neck
    if grip is None:
        return face + [(right, SHOULDER), (right, top), (left, top), (left, SHOULDER)], face
    mid = (left + right) / 2
    return face + [(right, SHOULDER), (right, top), (mid + grip[0] / 2, top), (mid + grip[0] / 2, top + grip[1]), (mid - grip[0] / 2, top + grip[1]), (mid - grip[0] / 2, top), (left, top), (left, SHOULDER)], face


def to_print(section, lying):
    """A section as it is drawn to print, and how it got there: (across, out of the wood) of an edge's own frame
    becomes (a, b) = (across - mid, far - out) for a form lying on its back with its working face up, or
    (across - mid, out - near) for one standing on its end."""
    mid = (min(q[0] for q in section) + max(q[0] for q in section)) / 2
    if lying:
        far = max(q[1] for q in section)
        return [(q[0] - mid, far - q[1]) for q in section][::-1], {"mid": mid, "from": far, "flipped": True}
    near = min(q[1] for q in section)
    return [(q[0] - mid, q[1] - near) for q in section], {"mid": mid, "from": near, "flipped": False}


def clear_of(section, outline, m):
    """How far a form's section, laid on the tile by m, is from the wood: the least distance, less than nothing if it
    is in it. The outline's own points are tried against the section too."""
    there = [moved(m, q) for q in section]
    if area2(there) < 0:
        there = there[::-1]
    ring = walked(there + [there[0]], 0.1)
    closed = outline + [outline[0]]
    least = min((-1.0 if inside(q, outline) else 1.0) * nearest(q, closed)[1] for q in ring)
    box = (min(q[0] for q in there), max(q[0] for q in there), min(q[1] for q in there), max(q[1] for q in there))
    for q in outline:
        if box[0] <= q[0] <= box[1] and box[2] <= q[1] <= box[3] and inside(q, there):
            least = min(least, -nearest(q, there + [there[0]])[1])
    return least


def prism(p, name, plane, at, pts, depth, op="new"):
    s = p.sketch(plane, at, name)
    p.polygon(s, pts)
    return p.extrude(s, depth, op=op, name=name)


def main():
    n = json.load(open(SLED))
    outline = [tuple(q) for q in n["tile"]["outline"]]
    if area2(outline) < 0:
        outline = outline[::-1]
    found = shapes(outline)
    bump, flute = found["bump"], found["flute"]
    levels = way_out(outline, flute, 24.0)
    neck = flute_neck(levels, GRIP)
    print(f"the way out of a flute that all seven share is {min(q[2] for q in levels if q[0] < neck[2]) - max(q[1] for q in levels if q[0] < neck[2]):.1f} wide at its narrowest; "
          f"the neck is {neck[1] - neck[0]:.1f} wide and {neck[2]:.1f} long, and the grip is past the lobes there")
    sections = {"bump": bump_section(bump["work"], FILM, BACK), "flute": flute_section(flute["work"], FILM, neck, GRIP)}
    blades = {"bump": bump_section(bump["work"], 0.0, PLATE[0]), "flute": flute_section(flute["work"], 0.0, (neck[0], neck[1], max(PLATE[0], neck[2])))}
    for name in ("bump", "flute"):
        f = found[name]
        print(f"{name}s: {f['wide']:.1f} across and {f['high']:.1f} {'high' if name == 'bump' else 'deep'}, {f['round']:.1f} round; the form covers {f['covered']:.1f} of that; "
              f"laid on each of the seven it is off by at most {max(q['off'] for q in f['places']):.3f} over what it covers, {max(q['off_whole'] for q in f['places']):.2f} over the whole of one")
        # the form, and the blade, on every one of the seven: nothing touches but its own edge
        for what, (section, face), least in (("form", sections[name], FILM), ("blade", blades[name], 0.0)):
            gaps = [clear_of(section, outline, q["move"]) for q in f["places"]]
            f[what + "_clear"] = gaps
            if min(gaps) < least - SAME - CLEAR:
                raise SystemExit(f"the {name} {what} is {min(gaps):.2f} from the wood on one of the seven: it wants {least - SAME:.2f} or more")
            film = [nearest(moved(q["move"], w), outline + [outline[0]])[1] for q in f["places"] for w in face]
            f[what + "_film"] = [min(film), max(film)]
        print(f"   the form on each of the seven: {min(f['form_clear']):.2f} or more from the wood, its face {f['form_film'][0]:.2f} to {f['form_film'][1]:.2f} from it, the film being {FILM}")

    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "finish.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "finish"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Finishing the tile stick"}, {"type": "rename_tab", "tab": 1, "name": "Bump form"}]})
    made, printed = {}, {}
    # Each form as it prints. The bump form lies on its back, its channel up: nothing overhangs but its leaning
    # walls. The flute form is a bulb on a thin neck, which lying down would overhang both ways: it stands on end.
    for k, (name, lying) in enumerate((("bump", True), ("flute", False))):
        title = f"{name.capitalize()} form"
        tab = 1 if k == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tab, title)
        flat, how = to_print(sections[name][0], lying)
        prism(part, f"{name} form", "right" if lying else "top", 0.0, flat, LONG[k])
        made[title] = part.bodies()
        if len(made[title]) != 1:
            raise SystemExit(f"the {name} form is {len(made[title])} bodies")
        mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, f"form_{name}.stl")})
        wide, high = max(q[0] for q in flat) - min(q[0] for q in flat), max(q[1] for q in flat) - min(q[1] for q in flat)
        printed[name] = {"tab": title, "file": f"form_{name}.stl", "plane": "right" if lying else "top", "section": [list(q) for q in flat], "how": how, "long": LONG[k],
                         "volume": made[title][0][1], "across": wide, "deep": high, "strip": along(sections[name][1])[-1]}
        print(f"{title}: {made[title][0][1]:.0f} mm3, {wide:.1f} across and {high:.1f} from its face to its back, {LONG[k]:.0f} long, printed {'lying on its back' if lying else 'standing on its end'}; "
              f"a strip of film {printed[name]['strip']:.1f} wide covers its face")
    # The blades' ends, as plates to scribe round: lying flat, the bump's and then the flute's.
    tab = new_tab(mcp, {"type": "add_part_studio", "name": "Blade templates"})
    part = Part(mcp, tab, "Blade templates")
    plates = {}
    for k, name in enumerate(("bump", "flute")):
        flat, how = to_print(blades[name][0], False)
        flat = [(q[0] + k * 25.0, q[1]) for q in flat]
        prism(part, f"{name} blade", "top", 0.0, flat, PLATE[1])
        plates[name] = {"outline": [list(q) for q in flat], "how": dict(how, mid=how["mid"] - k * 25.0), "file": f"template_{name}.stl", "thick": PLATE[1]}
    made["Blade templates"] = part.bodies()
    if len(made["Blade templates"]) != 2:
        raise SystemExit("the two templates are not two bodies")
    for k, name in enumerate(("bump", "flute")):
        mcp.call("export", {"tab": tab, "format": "stl", "body": k, "path": os.path.join(OUT, f"template_{name}.stl")})
        plates[name]["volume"] = made["Blade templates"][k][1]
    print(mcp.call("export", {"tab": tab, "format": "pdf", "views": ["top"], "scale": 1.0, "sheet": "Letter", "parts": False, "path": os.path.join(OUT, "templates.pdf"),
                              "note": "blade ends, 1:1: bump left, flute right. Print at 100 %"}))
    # Both forms on a length of the stick, each on the edge it was taken from.
    tab = new_tab(mcp, {"type": "add_part_studio", "name": "On the stick"})
    part = Part(mcp, tab, "On the stick")
    prism(part, "stick", "right", 0.0, outline, 120.0)
    for k, name in enumerate(("bump", "flute")):
        there = [moved(found[name]["places"][0]["move"], q) for q in sections[name][0]]
        prism(part, f"{name} form", "right", 12.0 + 30.0 * k, there if area2(there) > 0 else there[::-1], LONG[k])
    made["On the stick"] = part.bodies()
    if len(made["On the stick"]) != 3:
        raise SystemExit("the stick and the two forms on it are not three bodies")
    mcp.call("screenshot", {"tab": tab, "view": "0.75,-0.5,0.45", "width": 1500, "height": 1000, "path": os.path.join(OUT, "finish.png")})
    mcp.call("screenshot", {"tab": tab, "view": "1,0,0", "section": "x:50:flip", "width": 1200, "height": 1200, "path": os.path.join(OUT, "section.png")})
    mcp.close()

    numbers = {
        "film": FILM, "long": LONG, "same": SAME, "shy": SHY, "cover": COVER, "lip": LIP, "back": BACK, "taper": TAPER, "tongue": TONGUE, "air": AIR, "shoulder": SHOULDER, "neck": list(neck), "grip": GRIP, "clear": CLEAR, "plate": PLATE,
        "way_out": [list(q) for q in levels],
        "tile": {"outline": [list(q) for q in outline], "edge": n["edge"], "gap": n["gap"], "bit": n["bit"]},
        "edges": {name: {"wide": f["wide"], "high": f["high"], "round": f["round"], "shared": f["shared"], "covered": f["covered"], "work": [list(q) for q in f["work"]],
                         "face": [list(q) for q in sections[name][1]], "section": [list(q) for q in sections[name][0]], "blade": [list(q) for q in blades[name][0]],
                         "places": f["places"], "form_clear": f["form_clear"], "form_film": f["form_film"], "blade_clear": f["blade_clear"]}
                  for name, f in found.items()},
        "forms": printed, "templates": plates, "parts": [{"tab": t, "bodies": [list(b) for b in made[t]]} for t in made],
    }
    with open(os.path.join(OUT, "finish.json"), "w") as f:
        json.dump(numbers, f, indent=1)


if __name__ == "__main__":
    main()
