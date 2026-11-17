"""The stick sled's planning: which cuts make the tile, and where the
three motors stand for each. Pure Python, no kernel: build.py hands it
the tile's outline and writes what it returns.

A cut is (t, across, tip): the stick turned so that the direction t of
its section points down at the cutter, the cutter `across` from the
axis, its tip `tip` from the axis toward the router (negative: past the
axis). The sled's two hinges and its spindle put the stick there.
"""
import math

# The tile's cuts.
BIT = 3.0                       # the finishing bit: a ball-nose, 1.5 radius
RIDGE = 0.05                    # what a pass may leave standing between it and the next, to sand
FACE_MIN = 0.2                  # the ball cuts no further round its side than this: the cosine between the bit and the face (78 deg)
Y_LIM = 19.0                    # the planner keeps the cutter this near the axis; the hinge has a little more
ROUGH_BIT, ROUGH_BITE = 6.0, 3.0   # roughing: a straight bit, and the most it takes at a bite
ROUGH_STEP = 3.0                # its places across, half its width apart
ROUGH_ANGLES = 24               # the directions it roughs from, 15 deg apart
ALLOW = 0.4                     # what roughing leaves on the tile for the ball-nose
BALL_BITE = 1.5                 # the most the ball-nose takes at a bite
BLANK = 2.0                     # the blank: sawn to the tile's hull and this much over
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
    """Where the tile's largest inscribed circle is: the place for the axis that gives the fattest journal."""
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


def drop(outline, t, y, r, ball=True):
    """Where a cutter coming at the stick along t, y across, stops on
    the tile: the ball's centre, or a flat cutter's end. None if it
    passes the tile by."""
    d, e = frame(t)
    best = None
    for q in outline:
        u = q[0] * e[0] + q[1] * e[1]
        if abs(u - y) < r:
            h = q[0] * d[0] + q[1] * d[1] + (math.sqrt(r * r - (u - y) ** 2) if ball else 0.0)
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
        w = math.asin(min(1.0, (r - 0.03) / rho))
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
    """The open direction that meets the face most squarely, the cutter within Y_LIM of the axis and a degree clear of anything."""
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
        if best is None or facing > best[0]:
            best = (facing, t)
    return best[1] if best else None


def finish(outline):
    """The finishing passes: round the outline, a pass wherever the
    passes so far leave more than the ridge, each from the direction
    that meets the face most squarely among those the shank can come
    from. No two need share a direction: a motor turns the stick."""
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
    orphans = 0
    for i in range(n):                                    # what is still standing: any direction that reaches it at all
        if left[i] > RIDGE + 0.03 and not add(i, 0.02):
            orphans += 1
    return passes, left, orphans


def blank_reach(outline, t):
    """How far the blank stands toward the router when the stick is turned to t."""
    d, _ = frame(t)
    return max(q[0] * d[0] + q[1] * d[1] for q in outline) + BLANK


def rough(outline, passes):
    """The bites before the finish, followed on a grid of the waste
    (the blank less the tile) in the order the cuts are made. The
    straight bit first, from ROUGH_ANGLES directions at places
    ROUGH_STEP apart: at each as deep as the tile allows less ALLOW,
    in bites of ROUGH_BITE. Then every finishing pass takes what still
    stands over it in bites of BALL_BITE. Returns the roughing places
    and fills in each pass's depth and bites."""
    h_ = hull(outline)
    lo_x, hi_x = min(q[0] for q in h_) - BLANK, max(q[0] for q in h_) + BLANK
    lo_y, hi_y = min(q[1] for q in h_) - BLANK, max(q[1] for q in h_) + BLANK
    cells = []
    for i in range(int((hi_x - lo_x) / CELL) + 1):
        for j in range(int((hi_y - lo_y) / CELL) + 1):
            q = (lo_x + (i + 0.5) * CELL, lo_y + (j + 0.5) * CELL)
            if not inside(q, outline) and (inside(q, h_) or min(near(q, h_[m - 1], h_[m]) for m in range(len(h_))) <= BLANK):
                cells.append(q)
    waste = len(cells) * CELL * CELL
    alive = bytearray([1]) * len(cells)
    fine = []
    for i in range(len(outline)):
        a, b = outline[i - 1], outline[i]
        m = max(1, int(math.hypot(b[0] - a[0], b[1] - a[1]) / 0.05))
        fine.extend((a[0] + (b[0] - a[0]) * s / m, a[1] + (b[1] - a[1]) * s / m) for s in range(m))

    def cut(t, y, tip, r, ball, dry=False):
        """What a cutter at y, its tip at tip, takes with the stick at t: the area, and how far the waste stood over its tip."""
        d, e = frame(t)
        took, top = [], tip
        for i, q in enumerate(cells):
            if not alive[i]:
                continue
            u = q[0] * e[0] + q[1] * e[1]
            if abs(u - y) > r:
                continue
            h = q[0] * d[0] + q[1] * d[1]
            if h >= (tip + r - math.sqrt(r * r - (u - y) ** 2) if ball else tip):
                took.append(i)
                top = max(top, h)
        if not dry:
            for i in took:
                alive[i] = 0
        return len(took) * CELL * CELL, top - tip

    r6 = ROUGH_BIT / 2
    places, rough_area = [], 0.0
    for k in range(ROUGH_ANGLES):
        t = 2 * math.pi * k / ROUGH_ANGLES
        d, e = frame(t)
        line = [(q[0] * e[0] + q[1] * e[1], q[0] * d[0] + q[1] * d[1]) for q in fine]
        n = int(Y_LIM / ROUGH_STEP)
        for j in range(-n, n + 1):
            y = j * ROUGH_STEP
            under = [h for u, h in line if abs(u - y) <= r6]
            if not under:
                continue                                  # beside the tile from here: another direction faces it
            touch = max(under)
            area, depth = cut(t, y, touch + ALLOW, r6, False, dry=True)
            if area < 1.0:
                continue
            cut(t, y, touch + ALLOW, r6, False)
            rough_area += area
            places.append({"t": t, "across": y, "touch": touch, "tip": touch + ALLOW, "depth": depth, "bites": max(1, math.ceil(depth / ROUGH_BITE - 1e-9)), "area": area})
    left_after_rough = sum(alive) * CELL * CELL
    rb = BIT / 2
    for q in passes:
        _, depth = cut(q["t"], q["across"], q["tip"], rb, True)
        q["depth"] = depth
        q["bites"] = max(1, math.ceil(depth / BALL_BITE - 1e-9))
    return places, {"waste": waste, "rough_area": rough_area, "left_after_rough": left_after_rough, "left_at_end": sum(alive) * CELL * CELL, "hull": h_}


# -- the two hinges and the spindle ------------------------------------------

def rot(a, v):
    c, s = math.cos(a), math.sin(a)
    return (v[0] * c - v[1] * s, v[0] * s + v[1] * c)


class Sled:
    """The sled's geometry in its own frame: y to the right (the lift
    hinge's side), z up from the table. A lift plate swings on hinge 1;
    under it, on hinge 2, hangs the frame that carries the stick; the
    spindle turns the stick in that frame. A pusher under the plate's
    free edge sets the lift; a second, level, on the sled, bears on
    the hanging frame's edge and sets the swing."""

    def __init__(self, h1, arm, drop_len, p1_y, plate_under, ball, p2_z, pad_y):
        self.h1 = h1                    # hinge 1
        self.arm = arm                  # hinge 1 to hinge 2 in the plate's frame
        self.drop = drop_len            # hinge 2 down to the stick's axis
        self.p1_y = p1_y                # pusher 1's line, upright
        self.plate_under = plate_under  # the plate's underside over hinge 1's centre
        self.ball = ball                # a pusher's ball end, its radius
        self.p2_z = p2_z                # pusher 2's line over the table, level
        self.pad_y = pad_y              # the edge it bears on, right of the hanging frame's middle

    def axis(self, alpha, beta):
        h2 = rot(alpha, self.arm)
        p = rot(alpha + beta, (0.0, -self.drop))
        return (self.h1[0] + h2[0] + p[0], self.h1[1] + h2[1] + p[1])

    def hinge2(self, alpha):
        h2 = rot(alpha, self.arm)
        return (self.h1[0] + h2[0], self.h1[1] + h2[1])

    def angles(self, ya, za):
        """The plate's and the frame's angles that put the axis at (ya, za)."""
        v = (ya - self.h1[0], za - self.h1[1])
        la = math.hypot(*self.arm)
        c = (v[0] ** 2 + v[1] ** 2 - la * la - self.drop ** 2) / (2 * la * self.drop)
        if abs(c) > 1:
            raise ValueError("out of the hinges' reach")
        base = math.atan2(-self.drop, 0.0) - math.atan2(self.arm[1], self.arm[0])      # the frame against the arm, hanging square
        turn = math.acos(c)
        beta = min((s * turn - base for s in (1.0, -1.0)), key=lambda b: abs(math.atan2(math.sin(b), math.cos(b))))
        beta = math.atan2(math.sin(beta), math.cos(beta))
        w = (self.arm[0] + rot(beta, (0.0, -self.drop))[0], self.arm[1] + rot(beta, (0.0, -self.drop))[1])
        alpha = math.atan2(v[1], v[0]) - math.atan2(w[1], w[0])
        return math.atan2(math.sin(alpha), math.cos(alpha)), beta

    def pusher1(self, alpha):
        """The height of pusher 1's ball centre under the plate."""
        return self.h1[1] + (self.plate_under - self.ball + math.sin(alpha) * (self.p1_y - self.h1[0])) / math.cos(alpha)

    def pusher2(self, alpha, beta):
        """Pusher 2's ball centre along its line: the frame's edge is a plane pad_y from hinge 2, the ball a radius beyond it."""
        h2, th = self.hinge2(alpha), alpha + beta
        return h2[0] + (self.pad_y + self.ball - (self.p2_z - h2[1]) * math.sin(th)) / math.cos(th)

    def pad_at(self, alpha, beta):
        """How far down the frame's edge from hinge 2 the ball bears."""
        h2, th = self.hinge2(alpha), alpha + beta
        c = (self.pusher2(alpha, beta) - h2[0], self.p2_z - h2[1])
        return -c[0] * math.sin(th) + c[1] * math.cos(th)

    def alpha_of(self, c1):
        lo, hi = -0.6, 0.6                              # pusher 1 up, the plate's free edge up: alpha falls
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if self.pusher1(mid) > c1 else (lo, mid)
        return (lo + hi) / 2

    def beta_of(self, c2, alpha):
        lo, hi = -0.7 - alpha, 0.7 - alpha               # the frame hangs within 40 deg of upright; pusher out, the frame swings left, its angle falls
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if self.pusher2(alpha, mid) < c2 else (lo, mid)
        return (lo + hi) / 2

    def pose(self, t, across, tip, bit_up):
        """The joints for a cut: the pushers' ball centres and the spindle's angle in its frame, degrees."""
        ya, za = -across, bit_up + tip
        alpha, beta = self.angles(ya, za)
        psi = -t - math.pi / 2                           # the stick's section in the sled's frame
        gamma = math.degrees(psi - alpha - beta) % 360.0
        return {"c1": self.pusher1(alpha), "c2": self.pusher2(alpha, beta), "gamma": gamma, "alpha": alpha, "beta": beta, "ya": ya, "za": za}

    def cutter(self, c1, c2, gamma, bit_up):
        """Back from the joints to where the cutter's tip is in the stick's section: (across, tip, t)."""
        alpha = self.alpha_of(c1)
        beta = self.beta_of(c2, alpha)
        ya, za = self.axis(alpha, beta)
        psi = math.radians(gamma) + alpha + beta
        t = -psi - math.pi / 2
        d, e = frame(t)
        # the tip is at (0, bit_up) in the sled's frame; a point p of the section is at (ya + p.e, za - p.d)
        return (0.0 - ya, za - bit_up, t % (2 * math.pi))
