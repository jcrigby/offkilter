"""The stick sled's planning: which cuts make the tile, and where the
three motors stand for each. Pure Python, no kernel: build.py hands it
the tile's outline and writes what it returns.

A cut is (t, across, tip): the stick turned so that the direction t of
its section points at the router, the bit's axis `across` above the
stick's, its tip `tip` from the stick's axis toward the router
(negative: past the axis). The stick only turns. The router is held in
the two-axis positioner (examples/router-positioner), lying on its
side with the bit pointing at the stick: the positioner's X is the
depth, its Z the height.
"""
import math

# The tile's cuts.
BIT = 3.0                       # the finishing bit: a ball-nose, 1.5 radius
SHANK = 6.35                    # its shank, a quarter inch: the owner's Colt is the North American one, and that is the collet it has. The taper stops at this diameter
TAPER_L = 30.5                  # the length its flank takes to widen from the ball to the shank
TAPER = math.degrees(math.atan((SHANK - BIT) / 2 / TAPER_L))   # degrees a side its flank leans out above the ball: the common tapered ball-nose, 3 mm at the tip to its shank over 30.5 (0 for a straight bit)
RIDGE = 0.05                    # what a pass may leave standing between it and the next, to sand
FACE_MIN = 0.2                  # the ball cuts no further round its side than this: the cosine between the bit and the face (78 deg)
OFF_CENTRE = 15.0               # degrees: a pass meets its face this far round the ball from its tip, where it can. The tip's dead centre does not turn; 15 deg round, the edge passes at a metre a second
Y_LIM = 19.0                    # the planner keeps the cutter this near the axis; the positioner's height travel has twice that
ROUGH_BIT, ROUGH_BITE = 6.35, 3.0   # roughing: a straight quarter-inch bit, and the most it takes at a bite
ROUGH_STEP = 3.0                # its places across, a little under half its width apart
ROUGH_ANGLES = 24               # the directions it roughs from, 15 deg apart
ALLOW = 0.4                     # what roughing leaves on the tile for the ball-nose
BALL_BITE = 1.5                 # the most the ball-nose takes at a bite
BLANK = 2.0                     # the blank: a rectangle about the tile, at least this much over on every side
CELL = 0.2                      # the grid the waste is followed on
DIRS = 720                      # directions tried for a pass: every half degree


def frame(t):
    """d points from the axis at the router; e is across."""
    return (math.cos(t), math.sin(t)), (-math.sin(t), math.cos(t))


def inside(q, poly):
    hit = False
    for i in range(len(poly)):
        a, b = poly[i - 1], poly[i]
        if (a[1] > q[1]) != (b[1] > q[1]) and q[0] < a[0] + (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]):
            hit = not hit
    return hit


def near(q, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0.0, min(1.0, ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(q[0] - a[0] - t * dx, q[1] - a[1] - t * dy)


def room(q, outline):
    """The largest circle about q inside the outline; nothing if q is outside."""
    return min(near(q, outline[i - 1], outline[i]) for i in range(len(outline))) if inside(q, outline) else -1.0


def widest(outline):
    """Where the tile's largest inscribed circle is: the place for the axis with the most room round it for a spur and a master."""
    xs, ys = [q[0] for q in outline], [q[1] for q in outline]
    best = max(((room((x, y), outline), x, y) for x in (min(xs) + 0.5 * i for i in range(int((max(xs) - min(xs)) * 2) + 1))
                for y in (min(ys) + 0.5 * j for j in range(int((max(ys) - min(ys)) * 2) + 1))))
    step = 0.25
    while step > 1e-4:
        cand = max((room((best[1] + dx * step, best[2] + dy * step), outline), best[1] + dx * step, best[2] + dy * step) for dx in (-1, 0, 1) for dy in (-1, 0, 1))
        if cand[0] > best[0] + 1e-12:
            best = cand
        else:
            step /= 2
    return (round(best[1], 2), round(best[2], 2))


def hull(points):
    pts = sorted(set(points))

    def half(seq):
        out = []
        for q in seq:
            while len(out) >= 2 and (out[-1][0] - out[-2][0]) * (q[1] - out[-2][1]) - (out[-1][1] - out[-2][1]) * (q[0] - out[-2][0]) <= 0:
                out.pop()
            out.append(q)
        return out
    lower, upper = half(pts), half(pts[::-1])
    return lower[:-1] + upper[:-1]


def normals(outline):
    n = len(outline)
    a2 = sum(outline[i][0] * outline[(i + 1) % n][1] - outline[(i + 1) % n][0] * outline[i][1] for i in range(n))
    sgn = 1.0 if a2 > 0 else -1.0
    out = []
    for i in range(n):
        p0, p2 = outline[i - 1], outline[(i + 1) % n]
        tx, ty = p2[0] - p0[0], p2[1] - p0[1]
        ln = math.hypot(tx, ty)
        out.append((sgn * ty / ln, -sgn * tx / ln))
    return out


def flank(off, r):
    """The ball-nose's outline: how far toward the router from the
    ball's centre its surface is, `off` from its axis. The ball, then
    a cone leaning TAPER, then the shank. None beyond the shank."""
    a = math.radians(TAPER)
    if off <= r * math.cos(a):
        return -math.sqrt(r * r - off * off)
    if a > 0.0 and off < SHANK / 2:
        return (off * math.cos(a) - r) / math.sin(a)
    return None


def drop(outline, t, y, r, ball=True):
    """Where a cutter coming at the stick along t, y across, stops on
    the tile: the ball's centre, or a flat cutter's end. None if it
    passes the tile by."""
    d, e = frame(t)
    best = None
    for q in outline:
        off = abs(q[0] * e[0] + q[1] * e[1] - y)
        if ball:
            f = flank(off, r)
            if f is None:
                continue
            h = q[0] * d[0] + q[1] * d[1] - f
        elif off < r:
            h = q[0] * d[0] + q[1] * d[1]
        else:
            continue
        if best is None or h > best:
            best = h
    return best


def open_dirs(q, outline, r):
    """For a ball centred at q, the directions its shank can come from:
    every point of the outline shuts those within the shank's half
    width of it. A count per half degree; zero is open."""
    diff = [0] * (DIRS + 1)
    step = 2 * math.pi / DIRS
    for p in outline:
        vx, vy = p[0] - q[0], p[1] - q[1]
        rho = math.hypot(vx, vy)
        if rho < r - 0.06:
            return None                                   # the ball itself is in the tile here
        w = math.asin(min(1.0, (r - 0.03) / rho)) + math.radians(TAPER)      # a tapered flank shuts that much more either side
        phi = math.atan2(vy, vx)
        a, b = math.floor((phi - w) / step), math.ceil((phi + w) / step)
        if b - a >= DIRS:
            return None
        a %= DIRS
        b %= DIRS
        if a <= b:
            diff[a] += 1
            diff[b + 1] -= 1
        else:
            diff[a] += 1
            diff[DIRS] -= 1
            diff[0] += 1
            diff[b + 1] -= 1
    out, c = [], 0
    for k in range(DIRS):
        c += diff[k]
        out.append(c)
    return out


def best_dir(q, n, outline, r, face_min):
    """The open direction that meets the face nearest OFF_CENTRE round the ball from its tip, the cutter within Y_LIM of the axis and a degree clear of anything."""
    shut = open_dirs(q, outline, r)
    if shut is None:
        return None
    best = None
    for k in range(DIRS):
        if any(shut[(k + j) % DIRS] for j in (-2, -1, 0, 1, 2)):
            continue
        t = 2 * math.pi * k / DIRS
        d, e = frame(t)
        facing = n[0] * d[0] + n[1] * d[1]
        if facing < face_min:
            continue
        if abs(q[0] * e[0] + q[1] * e[1]) > Y_LIM:
            continue
        near_it = -abs(math.degrees(math.acos(min(1.0, facing))) - OFF_CENTRE)
        if best is None or near_it > best[0]:
            best = (near_it, t)
    return best[1] if best else None


def finish(outline):
    """The finishing passes: round the outline, a pass wherever the
    passes so far leave more than the ridge, each from the direction
    that meets the face a little off the ball's tip among those the
    whole bit can come from. No two need share a direction: a motor
    turns the stick."""
    r, n = BIT / 2, len(outline)
    ns = normals(outline)
    left = [9.0] * n
    passes = []

    def add(i, face_min):
        q = (outline[i][0] + r * ns[i][0], outline[i][1] + r * ns[i][1])
        t = best_dir(q, ns[i], outline, r, face_min)
        if t is None:
            return False
        d, e = frame(t)
        y = q[0] * e[0] + q[1] * e[1]
        c = drop(outline, t, y, r)                        # never deeper than the tile allows
        centre = (y * e[0] + c * d[0], y * e[1] + c * d[1])
        got = math.hypot(outline[i][0] - centre[0], outline[i][1] - centre[1]) - r
        if got > left[i] - 0.02:
            return False
        passes.append({"t": t, "across": y, "tip": c - r})
        for j, p in enumerate(outline):
            left[j] = min(left[j], math.hypot(p[0] - centre[0], p[1] - centre[1]) - r)
        return True

    for i in range(n):
        if left[i] > RIDGE:
            add(i, FACE_MIN)
    for i in range(n):                                    # what is still standing: any direction that reaches it at all
        if left[i] > RIDGE + 0.03:
            add(i, 0.02)
    # What nothing reached is counted at the end: a point its own pass could not better may be taken by its neighbour's.
    orphans = sum(1 for v in left if v > RIDGE + 0.03)
    return passes, left, orphans


_STOCK = {}


def stock(outline):
    """The blank's section: the least rectangle about the tile with BLANK over on every side, its sides rounded up to
    whole millimetres. A rectangle because a saw and a planer make one; the tile's hull and 2 mm over, which the
    cuts were first planned from, has six flats at odd angles and round corners between, and nothing makes that.
    Its corners in the tile's frame, its two sides, how far it is turned there, and where the axis is from its
    first corner along each side."""
    key = (len(outline), outline[0], outline[len(outline) // 2])
    if key not in _STOCK:
        h_, best = hull(outline), None
        for i in range(len(h_)):
            a = math.atan2(h_[i][1] - h_[i - 1][1], h_[i][0] - h_[i - 1][0])
            u = [(q[0] * math.cos(a) + q[1] * math.sin(a), -q[0] * math.sin(a) + q[1] * math.cos(a)) for q in h_]
            x0, x1, y0, y1 = min(q[0] for q in u), max(q[0] for q in u), min(q[1] for q in u), max(q[1] for q in u)
            if best is None or (x1 - x0) * (y1 - y0) < best[0]:
                best = ((x1 - x0) * (y1 - y0), a, x0, x1, y0, y1)
        _, a, x0, x1, y0, y1 = best
        size = (float(math.ceil(x1 - x0 + 2 * BLANK - 1e-9)), float(math.ceil(y1 - y0 + 2 * BLANK - 1e-9)))
        low = ((x0 + x1 - size[0]) / 2, (y0 + y1 - size[1]) / 2)          # its first corner, in its own frame
        corners = [(low[0] + dx, low[1] + dy) for dx, dy in ((0.0, 0.0), (size[0], 0.0), (size[0], size[1]), (0.0, size[1]))]
        _STOCK[key] = {"corners": [(q[0] * math.cos(a) - q[1] * math.sin(a), q[0] * math.sin(a) + q[1] * math.cos(a)) for q in corners],
                       "size": size, "turn": a, "axis": (-low[0], -low[1])}
    return _STOCK[key]


def blank_reach(outline, t):
    """How far the blank stands toward the router when the stick is turned to t."""
    d, _ = frame(t)
    return max(q[0] * d[0] + q[1] * d[1] for q in stock(outline)["corners"])


def rough_places(outline):
    """Where the straight bit could rough: from ROUGH_ANGLES directions,
    at places ROUGH_STEP apart, each as deep as the tile allows less
    ALLOW. Which of them are worth making, and in how many bites,
    depends on the order they are made in: bites() says."""
    fine = []
    for i in range(len(outline)):
        a, b = outline[i - 1], outline[i]
        m = max(1, int(math.hypot(b[0] - a[0], b[1] - a[1]) / 0.05))
        fine.extend((a[0] + (b[0] - a[0]) * s / m, a[1] + (b[1] - a[1]) * s / m) for s in range(m))
    r6, places = ROUGH_BIT / 2, []
    for k in range(ROUGH_ANGLES):
        t = 2 * math.pi * k / ROUGH_ANGLES
        d, e = frame(t)
        line = [(q[0] * e[0] + q[1] * e[1], q[0] * d[0] + q[1] * d[1]) for q in fine]
        n = int(Y_LIM / ROUGH_STEP)
        for j in range(-n, n + 1):
            y = j * ROUGH_STEP
            under = [h for u, h in line if abs(u - y) <= r6]
            if under:                                     # else beside the tile from here: another direction faces it
                places.append({"t": t, "across": y, "touch": max(under), "tip": max(under) + ALLOW})
    return places


def waste_cells(outline):
    """The waste, the blank less the tile, as the middles of CELL squares, and the blank's corners."""
    c_ = stock(outline)["corners"]
    lo_x, hi_x = min(q[0] for q in c_), max(q[0] for q in c_)
    lo_y, hi_y = min(q[1] for q in c_), max(q[1] for q in c_)
    cells = []
    for i in range(int((hi_x - lo_x) / CELL) + 1):
        for j in range(int((hi_y - lo_y) / CELL) + 1):
            q = (lo_x + (i + 0.5) * CELL, lo_y + (j + 0.5) * CELL)
            if inside(q, c_) and not inside(q, outline):
                cells.append(q)
    return cells, c_


def bites(outline, places, passes):
    """The bites, followed on a grid of the waste in the order the cuts
    will be made: `places` as given, then `passes` as given. That order
    is the table's, and must be: what a cut meets is what the cuts
    before it left. A roughing place that would take under a square
    millimetre is dropped; the others go down in bites of ROUGH_BITE.
    Then every finishing pass takes what still stands over it in bites
    of BALL_BITE. Returns the places kept, and fills in every cut's
    depth and bites."""
    cells, _ = waste_cells(outline)
    waste = len(cells) * CELL * CELL
    alive = bytearray([1]) * len(cells)

    def cut(t, y, tip, r, ball, dry=False):
        """What a cutter at y, its tip at tip, takes with the stick at t: the area, and how far the waste stood over it."""
        d, e = frame(t)
        took, top = [], 0.0
        for i, q in enumerate(cells):
            if not alive[i]:
                continue
            off = abs(q[0] * e[0] + q[1] * e[1] - y)
            h = q[0] * d[0] + q[1] * d[1]
            if ball:
                f = flank(off, r)
                if f is None or h < tip + r + f:
                    continue
                top = max(top, h - (tip + r + f))
            else:
                if off > r or h < tip:
                    continue
                top = max(top, h - tip)
            took.append(i)
        if not dry:
            for i in took:
                alive[i] = 0
        return len(took) * CELL * CELL, top

    kept, rough_area = [], 0.0
    for q in places:
        area, depth = cut(q["t"], q["across"], q["tip"], ROUGH_BIT / 2, False, dry=True)
        if area < 1.0:
            continue
        cut(q["t"], q["across"], q["tip"], ROUGH_BIT / 2, False)
        rough_area += area
        kept.append(dict(q, depth=depth, bites=max(1, math.ceil(depth / ROUGH_BITE - 1e-9)), area=area))
    left_after_rough = sum(alive) * CELL * CELL
    for q in passes:
        _, depth = cut(q["t"], q["across"], q["tip"], BIT / 2, True)
        q["depth"] = depth
        q["bites"] = max(1, math.ceil(depth / BALL_BITE - 1e-9))
    return kept, {"waste": waste, "rough_area": rough_area, "left_after_rough": left_after_rough, "left_at_end": sum(alive) * CELL * CELL}


# -- the positioner's two axes and the spindle --------------------------------

def rot(a, v):
    return (v[0] * math.cos(a) - v[1] * math.sin(a), v[0] * math.sin(a) + v[1] * math.cos(a))


def turned(a):
    return math.atan2(math.sin(a), math.cos(a))


class Axes:
    """Where the bit's tip is from the stick's axis, by the positioner's two screws. `depth` is the quill run out
    from its home, drawn back; `height` the stage run up from its home, down. With both at home the tip is
    `tip_home` from the stick's axis toward the router and the bit's axis `across_home` above the stick's.

    As drawn a millimetre of screw is a millimetre and the two travels are square. A built one is not quite:
    `scale` is each screw's true travel for one asked, depth then height, and `shear` how far the bit rises for
    a millimetre run out. Taking the height's travel as straight up loses nothing: a tile turned about its axis is
    the same tile."""

    def __init__(self, tip_home, across_home, scale=(1.0, 1.0), shear=0.0):
        self.tip_home, self.across_home = tip_home, across_home
        self.scale, self.shear = scale, shear

    def tip(self, depth, height):
        """(tip, across) of the bit's tip."""
        return self.tip_home - self.scale[0] * depth, self.across_home + self.scale[1] * height + self.shear * depth

    def screws(self, tip, across):
        """(depth, height) that put the tip there."""
        depth = (self.tip_home - tip) / self.scale[0]
        return depth, (across - self.across_home - self.shear * depth) / self.scale[1]

    def row(self, t, across, tip):
        """A cut as the table has it: depth, height, and the spindle's angle in degrees."""
        depth, height = self.screws(tip, across)
        return depth, height, math.degrees(-t) % 360.0

    def cutter(self, depth, height, turn):
        """Back from a row to the cut: (across, tip, t). The section sits in the machine turned by the spindle's
        angle, with d toward the router and e up, so t is that angle the other way."""
        tip, across = self.tip(depth, height)
        return across, tip, (-math.radians(turn)) % (2 * math.pi)
