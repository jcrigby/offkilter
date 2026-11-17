#!/usr/bin/env python3
"""What a built stick sled is, found by touching; and whether its stick runs true.

A sled built from the drawing is off it by what a saw and a drill are
off: half a millimetre in a hinge's place or a link's length, and the
home switches wherever they were screwed down. Cut from the drawing's
table, the tile is then out by several tenths. So the built sled is
measured, with its own motors, and the table planned for what it is.

The master on the spindle is a ground sleeve, and the controller can
feel it touch metal: a clip lead on the spindle, another on the thing
touched, and the touch closes a contact. It brings the master down on a
6 mm pin in the router's collet and against both its sides at four
heights, with the spindle at 0 and at 180 deg; down on a 1-2-3 block
lying on the table at three places across; on the block stood on its
long edge, down at three places and against both faces at four
heights; and against one face of the block stood on end, at four
heights up to the top of the lift, from each side. Each place is
touched three times. Where the two screws stood goes into
probe_log.csv. Everything touched is a known size, square, at a known
height over the table, so the touches say where the stick's axis
really was, and the sled that fits them all is the sled as built:
thirteen dimensions, the two home switches, the cutter's place across,
the master's own runout.

    python3 calibrate.py fit probe_log.csv [--master 15.98]
        the log -> built.json. Then run build.py: it plans for that sled.
    python3 calibrate.py align UNDER_NEAR UNDER_FAR BESIDE_NEAR BESIDE_FAR [SPAN]
        the four band diameters bands.csv turned -> how far to move the tail centre.
    python3 calibrate.py bands RADIUS
        out/bands.csv again, the bands turned to another radius: a millimetre under the last, to try again.
    python3 calibrate.py study [TRIALS]
        how well the probing works, tried on sleds built wrong on purpose -> out/calibration.json.

build.py writes the two tables, out/probe.csv and out/bands.csv, from
the functions here. Pure Python: nothing to install.
"""
import json
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import plan as P  # noqa: E402

OUT = os.path.join(HERE, "out")
BUILT = os.path.join(HERE, "built.json")

PIN_R = 3.0                                     # a 6 mm ground pin in the collet
BLOCK = (25.4, 50.8, 76.2)                      # a 1-2-3 block
TOUCH = 0.01                                    # how well a touch is taken to repeat
SIDE_TIPS = (-4.0, -6.5, -9.0, -11.5)           # the axis this far under the pin's top, for its sides
FLAT_ACROSS = (-22.0, 0.0, 22.0)                # where the lying block's top is touched, from the cutter's line
EDGE_ACROSS = (-6.0, 0.0, 6.0)                  # and the standing block's, from its own middle
FACE_HEIGHTS = (30.0, 35.5, 41.0, 46.5)         # the axis over the table, for the faces of the block on edge
POST_HEIGHTS = (33.0, 46.0, 59.0, 72.0)         # and for the one face of the block on end: higher than anything else reaches
POST_FACE = 18.0                                # that face this far from the cutter's line, the block beyond it
TOUCHES = 3                                     # each place is touched this many times
BROUGHT = 0.2                                   # a block slid up to the master by hand is left within this of touching it
DESCENT_CLEAR = 6.0                             # the master passes the pin at least this far from it, on the drawn sled: the built one may be 3 mm out there
BROUGHT_CLEAR = 4.0                             # and a block this far: it was slid up to the master, so it is where the built sled thinks it is
BAND_R = 10.0                                   # the bands turned on a scrap stick to see whether it runs true, in bites of 4
BAND_SPAN = 150.0                               # between the near stop and the far one
# What is fitted, and how far off the drawing each is expected to be: a dimension half a millimetre, a switch or the
# cutter's place anything, the master's runout a few tenths. The touches outvote these wherever they have a say.
NAMES = ["hinge 1 across", "hinge 1 up", "hinge 2 across from hinge 1", "hinge 2 up from hinge 1", "hinge 2 down to the axis", "lift screw across",
         "lift link's pin across", "lift link's pin up", "lift link", "swing screw up", "swing link's pin across", "swing link's pin up", "swing link",
         "lift home switch", "swing home switch", "cutter across", "pin's top", "master off centre, along", "master off centre, across",
         "block on edge, across", "block on end far side, across", "block on end fence side, across"]
PRIOR = [0.5] * 13 + [5.0, 5.0, 5.0, 5.0, 0.3, 0.3, 10.0, 10.0, 10.0]
SIZES = {"pin_r": PIN_R, "flat_top": BLOCK[0], "flat_half": BLOCK[2] / 2, "flat_y": 0.0, "edge_top": BLOCK[1], "edge_half": BLOCK[0] / 2, "post_top": BLOCK[2], "post_wide": BLOCK[0]}


class Built:
    """A sled as built: the drawn one, `d`, and what is off it, `v`, in the order of NAMES.
    `sizes` are the things touched, where they are not the sizes they are sold as."""

    def __init__(self, d, v=None, sizes=None):
        v = list(v) if v is not None else []
        v += [0.0] * (len(PRIOR) - len(v))
        self.d, self.v = d, v
        self.sled = P.Sled((d["h1"][0] + v[0], d["h1"][1] + v[1]), (d["arm"][0] + v[2], d["arm"][1] + v[3]), d["drop"] + v[4], d["p1_y"] + v[5],
                           (d["e1"][0] + v[6], d["e1"][1] + v[7]), d["l1"] + v[8], d["p2_z"] + v[9], (d["e2"][0] + v[10], d["e2"][1] + v[11]), d["l2"] + v[12])
        self.sled.cutter_y = v[15]
        self.home = (d["c1_home"] + v[13], d["c2_home"] + v[14])
        self.ecc = (v[17], v[18])
        self.sizes = dict(SIZES, master_r=d["master_r"])
        self.sizes.update(sizes or {})

    def axis(self, lift, swing):
        """Where the stick's axis is with the screws this far from their home switches, and how the frame hangs."""
        a = self.sled.alpha_of(self.home[0] - lift)
        b = self.sled.beta_of(self.home[1] - swing, a)
        ya, za = self.sled.axis(a, b)
        return ya, za, a + b

    def master(self, lift, swing, turn):
        """The master's middle: the axis, and its runout turned with the spindle."""
        ya, za, hang = self.axis(lift, swing)
        t = -(math.radians(turn) + hang) - math.pi / 2
        m = self.ecc
        return ya - m[0] * math.sin(t) + m[1] * math.cos(t), za - m[0] * math.cos(t) - m[1] * math.sin(t)

    def thing(self, what):
        """What is touched, as the master meets it end on: from, to, and its top over the table."""
        s = self.sizes
        if what == "pin":
            return self.v[15] - s["pin_r"], self.v[15] + s["pin_r"], self.d["bit_up"] + self.v[16]
        if what == "flat":
            return s["flat_y"] - s["flat_half"], s["flat_y"] + s["flat_half"], s["flat_top"]
        if what == "post_far":                      # the block on end beyond the cutter's line from the fence, its fence-side face
            return -POST_FACE + self.v[20] - s["post_wide"], -POST_FACE + self.v[20], s["post_top"]
        if what == "post_near":                     # and on the fence's side, its far face
            return POST_FACE + self.v[21], POST_FACE + self.v[21] + s["post_wide"], s["post_top"]
        y = self.d["edge_y"] + self.v[19]
        return y - s["edge_half"], y + s["edge_half"], s["edge_top"]

    def gap(self, lift, swing, turn, what):
        """Between the master and a thing: nothing at a touch."""
        y0, y1, top = self.thing(what)
        c = self.master(lift, swing, turn)
        return math.hypot(max(y0 - c[0], 0.0, c[0] - y1), max(c[1] - top, 0.0)) - self.sizes["master_r"]

    def screws(self, ya, za):
        """The screws that put the axis there."""
        a, b = self.sled.angles(ya, za)
        return self.home[0] - self.sled.pusher1(a), self.home[1] - self.sled.pusher2(a, b), a + b

    def row(self, t, across, tip):
        """A cut as the table has it: lift, swing, turn."""
        q = self.sled.pose(t, across, tip, self.d["bit_up"])
        return self.home[0] - q["c1"], self.home[1] - q["c2"], q["gamma"]

    def setting(self):
        """The cut that is the master's lowest point on the bit's tip, where a bit is set."""
        off = P.rot(-math.pi / 2, self.ecc)
        return 0.0, off[0], self.sizes["master_r"] - off[1]


def drawn_of(numbers):
    """The drawn sled out of sled.json."""
    return dict(numbers["sled"]["as_drawn"])


def lines(*text):
    """Up to four lines of sixteen for the display, as one label."""
    if any(len(t) > 16 or "," in t for t in text) or len(text) > 4:
        raise ValueError(f"does not fit the display: {text}")
    return "".join(t.ljust(16) for t in text).rstrip()


def top_swing(sled):
    """The swing that puts the axis over the cutter's line with the lift at home."""
    lo, hi = 0.0, sled.d["swing_most"]
    for _ in range(40):
        mid = (lo + hi) / 2
        if sled.axis(0.0, mid)[0] > 0.0:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


def probe_rows(d, windows):
    """The probing table for the drawn sled, and where the block on edge goes: its middle across from the cutter's line.
    Every touch is where the drawn sled would make it; the controller starts a window of the screw short of that:
    `windows` are the lift's and the swing's."""
    best = None
    for tenth in range(-80, 81, 5):
        try:
            rows, room = _probe_rows(dict(d, edge_y=tenth / 10.0), windows)
        except ValueError:
            continue
        if best is None or room[0] > best[2][0]:
            best = (rows, tenth / 10.0, room)
    if best is None or best[2][0] < 0.0:
        raise SystemExit("the probing does not fit the screws' travel" + ("" if best is None else f": short by {-best[2][0]:.1f} mm, {best[2][1]}"))
    return best[0], best[1]


def _probe_rows(d, windows):
    sled = Built(d)
    r = d["master_r"]
    rows = []
    room = [1e9, ""]
    last = [None]                                   # where the last probe left the screws: its start

    def needs(what, *rooms):
        if min(rooms) < room[0]:
            room[0], room[1] = min(rooms), what

    def add(kind, what, lift=0.0, swing=0.0, turn=0.0, label="", via="h"):
        rows.append({"n": len(rows) + 1, "kind": kind, "tool": what, "lift": round(lift, 4), "swing": round(swing, 4), "turn": turn, "via": via, "label": label})

    def down(what, ya, turn):
        lift, swing, _ = sled.screws(ya, sled.thing(what)[2] + r)
        needs(f"the lift coming down on {what} at {ya:+.0f}", lift - windows[0], d["lift_most"] - lift - windows[0])
        add("down", what, lift, swing, turn, f"down on {what}")
        last[0] = None

    def side(what, s, za, turn, via="h", bring=None):
        """Against the face on side s of a thing: +1 the fence's side, come at with the swing running out.
        By the top of the lift, or straight from the last touch on the same face. With `bring`, not a touch: the
        master put there for a block to be slid up to it."""
        y0, y1, _ = sled.thing(what)
        lift, swing, _ = sled.screws((y1 if s > 0 else y0) + s * r, za)
        start = swing - s * windows[1]
        needs(f"the screws at {what}'s {'fence' if s > 0 else 'far'} side, the axis {za:.0f} up", start - 0.5, d["swing_most"] - start, swing + s * windows[1] - 0.5, d["swing_most"] - (swing + s * windows[1]), d["lift_most"] - lift)
        if bring is not None:
            add("br+" if s > 0 else "br-", what, lift, swing, turn, bring)
        else:
            if via == "h":                          # coming down beside it, the master stays clear
                path = [(k, start) for k in range(int(lift) + 1)]
            else:                                   # across at the last height, then to this one
                path = [(last[0][0], start)] + [(last[0][0] + (lift - last[0][0]) * k / 12, start) for k in range(13)]
            needs(f"the master's way to {what}'s {'fence' if s > 0 else 'far'} side, the axis {za:.0f} up", min(sled.gap(k, w, turn, what) for k, w in path) - (DESCENT_CLEAR if what == "pin" else BROUGHT_CLEAR))
            add("sw+" if s > 0 else "sw-", what, lift, swing, turn, f"{what} {'fence' if s > 0 else 'far'} side", via)
        last[0] = (lift, start)

    def ordered(what, s, heights):
        """Up a face or down it: whichever way keeps the master further off it, going straight from one height to the next
        (the swing first, then the lift, as the controller moves)."""
        def clear(hs):
            worst = 1e9
            for a, b in zip(hs, hs[1:]):
                face = sled.thing(what)[1 if s > 0 else 0] + s * r
                la, lb, sb = sled.screws(face, a)[0], sled.screws(face, b)[0], sled.screws(face, b)[1] - s * windows[1]
                worst = min(worst, min(sled.gap(la + (lb - la) * k / 12, sb, 0.0, what) for k in range(13)))
            return worst
        return list(heights) if clear(list(heights)) >= clear(list(heights)[::-1]) else list(heights)[::-1]

    top = top_swing(sled)
    add("say", "", label=lines("router UNPLUGGED", "6 mm pin in the", "collet its top", f"{d['bit_up']:.0f} over table"))
    add("say", "", label=lines("sled clamped at", "its back stop.", "clips on pin", "and spindle"))
    add("leads", "", label="pin")
    for turn in (0.0, 180.0):                       # down each side of the pin a step at a time: coming down from the top beside
        down("pin", 0.0, turn)                      # its foot, a sled a few millimetres out would come down on it
        for s in (1.0, -1.0):
            for i, tip in enumerate(SIDE_TIPS):
                side("pin", s, d["bit_up"] + tip, turn, "h" if i == 0 else "d")
    add("place", "", 0.0, top, 0.0, "to the top")
    add("say", "", label=lines("pin down under", "the table. block", "lying flat its", "3 in. across"))
    add("say", "", label=lines("under the master", "2 mm clear of", "the hanger. clip", "on the block"))
    add("leads", "", label="block")
    for ya in FLAT_ACROSS:
        down("flat", ya, 0.0)
    # the block on its long edge: slid up to the master, then its fence side going up, its top, its far side coming down
    heights = ordered("edge", 1.0, FACE_HEIGHTS)
    side("edge", 1.0, heights[0], 0.0, bring=lines("block on its 3", "in. edge along", "the stick: slide"))
    for za in heights:
        side("edge", 1.0, za, 0.0, "d")
    for off in EDGE_ACROSS:
        down("edge", d["edge_y"] + off, 0.0)
    for i, za in enumerate(ordered("edge", -1.0, FACE_HEIGHTS)):
        side("edge", -1.0, za, 0.0, "h" if i == 0 else "d")
    # the block on end, each side of the cutter's line: one face, as high as the lift goes
    for what, s, where in (("post_far", 1.0, "the far side:"), ("post_near", -1.0, "the fence side:")):
        heights = ordered(what, s, POST_HEIGHTS)
        side(what, s, heights[0], 0.0, bring=lines("block on end on", where, "slide it"))
        for za in heights:
            side(what, s, za, 0.0, "d")
    add("place", "", 0.0, top, 0.0, "to the top")
    add("say", "", label=lines("done. copy", "probe_log.csv", "run calibrate.py", "fit"))
    for row in rows:
        if row["kind"] in ("down", "sw+", "sw-", "br+", "br-") and abs(sled.gap(row["lift"], row["swing"], row["turn"], row["tool"])) > 1e-3:
            raise SystemExit(f"probing row {row['n']} is not a touch")
    return rows, room


def band_rows(sled, radius=BAND_R):
    """The table that turns round bands on a scrap stick, under the cutter and beside it, near the head and near the tail.
    `sled` is a Built; rows as build.py's."""
    setting, bites = sled.setting(), (radius + 8.0, radius + 4.0, radius)
    rows = []

    def add(kind, tool, cut=None, label=""):
        lift, swing, turn = sled.row(*cut) if cut else (0.0, 0.0, 0.0)
        rows.append({"n": len(rows) + 1, "kind": kind, "tool": tool, "lift": round(lift, 4), "swing": round(swing, 4), "turn": round(turn, 4), "via": "s", "label": label,
                     "across": cut[1] if cut else 0.0, "tip": cut[2] if cut else 0.0, "t": cut[0] if cut else 0.0})

    add("say", "", label=lines("a scrap stick 30", "square or less", "between centres.", "straight bit"))
    add("say", "", label=lines("a stop clamped", "25 mm in from", "each end of the", "stick"))
    add("set", "straight", setting, "set the straight bit on the master")
    for tip in bites:
        add("place", "straight", (0.0, 0.0, tip), f"under {tip:.0f}")
        add("round", "", label=lines("push to the far", "stop and hold"))
        add("round", "", label=lines("back to the near", "stop and hold"))
        add("say", "", label=lines("pull right back"))
    add("say", "", label=lines("move both stops", "10 mm toward the", "tail"))
    for tip in bites:
        add("place", "straight", (0.0, tip + P.ROUGH_BIT / 2, -P.ROUGH_BIT / 2), f"beside {tip:.0f}")
        add("round", "", label=lines("push to the far", "stop and hold"))
        add("round", "", label=lines("back to the near", "stop and hold"))
        add("say", "", label=lines("pull right back"))
    add("say", "", label=lines("measure the four", "bands. run", "calibrate.py", "align"))
    return rows


def write_table(path, rows, safe):
    with open(path, "w") as f:
        f.write(f"#safe,{safe}\n")
        f.write("n,kind,tool,lift,swing,turn,via,label\n")
        for r in rows:
            f.write(f"{r['n']},{r['kind']},{r['tool']},{r['lift']},{r['swing']},{r['turn']},{r['via']},{r['label']}\n")


# -- the fit -------------------------------------------------------------------

def solve(a, b):
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for i in range(n):
        p = max(range(i, n), key=lambda k: abs(m[k][i]))
        m[i], m[p] = m[p], m[i]
        for k in range(i + 1, n):
            f = m[k][i] / m[i][i]
            if f:
                for j in range(i, n + 1):
                    m[k][j] -= f * m[i][j]
    x = [0.0] * n
    for i in range(n - 1, -1, -1):
        x[i] = (m[i][n] - sum(m[i][j] * x[j] for j in range(i + 1, n))) / m[i][i]
    return x


def fit(d, touches, sizes=None):
    """The sled that fits the touches: what is off the drawing, in the order of NAMES, and how far the touches are
    left from it (root mean square, and the worst). Each touch is (what, lift, swing, turn).
    Least squares, damped: every touch a gap that should be nothing, every unknown held loosely to the drawing."""
    n = len(PRIOR)

    def gaps(v):
        s = Built(d, v, sizes)
        return [s.gap(lift, swing, turn, what) for what, lift, swing, turn in touches]

    def residuals(v):
        return [g / TOUCH for g in gaps(v)] + [v[i] / PRIOR[i] for i in range(n)]

    v = [0.0] * n
    r = residuals(v)
    cost = sum(x * x for x in r)
    damp = 1e-3
    for _ in range(80):
        jac = []
        for i in range(n):
            w = v[:]
            w[i] += 1e-6
            jac.append([(a - b) / 1e-6 for a, b in zip(residuals(w), r)])
        grad = [sum(a * b for a, b in zip(jac[i], r)) for i in range(n)]
        hess = [[sum(a * b for a, b in zip(jac[i], jac[j])) for j in range(n)] for i in range(n)]
        step = None
        while damp < 1e9:
            lhs = [[hess[i][j] + (damp * hess[i][i] if i == j else 0.0) for j in range(n)] for i in range(n)]
            try_step = solve(lhs, [-g for g in grad])
            w = [a + b for a, b in zip(v, try_step)]
            try:
                r2 = residuals(w)
                c2 = sum(x * x for x in r2)
            except ValueError:                      # a link out of reach: too far
                c2 = float("inf")
            if c2 < cost:
                step, v, r, gain, cost = try_step, w, r2, cost - c2, c2
                damp = max(damp / 10.0, 1e-9)
                break
            damp *= 10.0
        if step is None or (max(abs(x) for x in step) < 1e-7) or gain < 1e-9 * cost:
            break
    g = gaps(v)
    return v, math.sqrt(sum(x * x for x in g) / len(g)), max(abs(x) for x in g)


# -- how well it works, tried on sleds built wrong ----------------------------------

def mis_built(d, rng, spread=0.5):
    """A sled built wrong at random, and the things it is probed with not quite their sizes either."""
    v = [rng.uniform(-spread, spread) for _ in range(13)] + [rng.uniform(-2.0, 2.0), rng.uniform(-2.0, 2.0), rng.uniform(-1.0, 1.0), rng.uniform(-1.0, 1.0)]
    a, m = rng.uniform(0.0, 2 * math.pi), rng.uniform(0.0, 0.1)
    v += [m * math.cos(a), m * math.sin(a), 0.0, 0.0, 0.0]
    sizes = {"master_r": d["master_r"] + rng.uniform(-0.01, 0.01), "pin_r": PIN_R + rng.uniform(-0.004, 0.004), "flat_top": BLOCK[0] + rng.uniform(-0.005, 0.005),
             "flat_y": rng.uniform(-5.0, 5.0), "edge_top": BLOCK[1] + rng.uniform(-0.005, 0.005), "edge_half": BLOCK[0] / 2 + rng.uniform(-0.0025, 0.0025),
             "post_top": BLOCK[2] + rng.uniform(-0.005, 0.005)}
    return v, sizes


def bring(true, rows, rng):
    """The blocks where a hand leaves them: each slid up to the master where the controller holds it, and let go."""
    for row in rows:
        if row["kind"] not in ("br+", "br-"):
            continue
        s = 1.0 if row["kind"] == "br+" else -1.0
        face = true.master(row["lift"], row["swing"], row["turn"])[0] - s * (true.sizes["master_r"] + rng.uniform(0.0, BROUGHT))
        if row["tool"] == "edge":
            true.v[19] = face - s * true.sizes["edge_half"] - true.d["edge_y"]
        elif row["tool"] == "post_far":
            true.v[20] = face + POST_FACE
        else:
            true.v[21] = face - POST_FACE


def touch(true, row, windows, noise, before=None):
    """Where the controller finds a touch on the sled `true`: the screw run from a window short of the row to a window
    past it. `before` is where the last probe left the screws, for a row gone to straight. None if nothing is met;
    ValueError if the master is on the thing, or through it, before the probe begins."""
    kind = row["kind"]
    way = -1.0 if kind == "sw-" else 1.0
    if kind == "down":
        at = lambda u: true.gap(u, row["swing"], row["turn"], row["tool"]) - noise
        want, window = row["lift"], windows[0]
    else:
        at = lambda u: true.gap(row["lift"], u, row["turn"], row["tool"]) - noise
        want, window = row["swing"], windows[1]
        start = want - way * window
        path = [(float(k), start) for k in range(int(row["lift"]) + 1)] if row["via"] == "h" else [(before[0] + (row["lift"] - before[0]) * k / 12, start) for k in range(13)]
        if min(true.gap(k, w, row["turn"], row["tool"]) for k, w in path) < 0.5:
            raise ValueError("the master comes on the thing on its way")
    u0, u1 = max(0.0, want - way * window), want + way * window
    if at(u0) <= 0.0:
        raise ValueError("touching before the probe began")
    prev = u0
    for k in range(1, 49):
        u = u0 + (u1 - u0) * k / 48
        if at(u) <= 0.0:
            lo, hi = prev, u
            for _ in range(30):
                mid = (lo + hi) / 2
                if at(mid) <= 0.0:
                    hi = mid
                else:
                    lo = mid
            return hi
        prev = u
    return None


def touches_of(true, rows, windows, rng, noise=TOUCH):
    """Every touch of the probing table on the sled `true`, its blocks brought up by hand, as fit() wants them."""
    bring(true, rows, rng)
    out, before = [], None
    for row in rows:
        if row["kind"] in ("br+", "br-"):
            before = (row["lift"], row["swing"] - (1.0 if row["kind"] == "br+" else -1.0) * windows[1])
        if row["kind"] not in ("down", "sw+", "sw-"):
            continue
        u = touch(true, row, windows, sum(rng.gauss(0.0, noise) for _ in range(TOUCHES)) / TOUCHES, before)
        if u is None:
            raise ValueError("no touch")
        out.append((row["tool"], u, row["swing"], row["turn"]) if row["kind"] == "down" else (row["tool"], row["lift"], u, row["turn"]))
        before = (row["lift"], row["swing"] - (-1.0 if row["kind"] == "sw-" else 1.0) * windows[1]) if row["kind"] != "down" else None
    return out


def cut_error(true, planned, cuts, ball_r):
    """The worst a finishing cut is from where it was wanted, on the sled `true` run from a table planned for the sled
    `planned`: the bit set on the master as the table's own setting row has it, the cuts made, the whole section turned
    to sit best on the one wanted (a tile turned about its axis is the same tile)."""
    lift, swing, turn = planned.row(*planned.setting())
    my, mz = true.master(lift, swing, turn)
    r = true.sizes["master_r"]
    tip_z = mz - math.sqrt((r + ball_r) ** 2 - (true.v[15] - my) ** 2) + ball_r       # the ball's tip brought up to the master
    got = []
    for t, across, tip in cuts:
        lift, swing, turn = planned.row(t, across, tip)
        ya, za, hang = true.axis(lift, swing)
        got.append((true.v[15] - ya, za - tip_z + ball_r, -(math.radians(turn) + hang) - math.pi / 2))
    twist = sum(P.turned(g[2] - c[0]) for g, c in zip(got, cuts)) / len(cuts)
    worst = 0.0
    for g, (t, across, tip) in zip(got, cuts):
        u = g[2] - twist
        q = (-g[0] * math.sin(u) + g[1] * math.cos(u), g[0] * math.cos(u) + g[1] * math.sin(u))
        p = (-across * math.sin(t) + (tip + ball_r) * math.cos(t), across * math.cos(t) + (tip + ball_r) * math.sin(t))
        worst = max(worst, math.hypot(q[0] - p[0], q[1] - p[1]))
    return worst


def firmware_touches(true, rows, windows, rng):
    """The probing table run by the controller's own program, on a computer, its contact closed by the sled `true`."""
    sys.path.insert(0, os.path.join(HERE, "firmware"))
    import main as fw
    if (fw.C.PROBE_LIFT_MM, fw.C.PROBE_SWING_MM) != tuple(windows) or fw.C.PROBE_TOUCHES != TOUCHES:
        raise SystemExit("the probing table was made for other windows than the controller's")
    fw.SCREEN.quiet = True
    machine = fw.Sled(0.0)
    bring(true, rows, rng)
    by_n = {row["n"]: row for row in rows}
    noise = {(row["n"], k): rng.gauss(0.0, TOUCH) for row in rows for k in range(TOUCHES)}

    def contact():
        row = by_n[machine.row]
        return true.gap(machine.lift.at / machine.lift.per_unit, machine.swing.at / machine.swing.per_unit, machine.turn.at / machine.turn.per_unit, row["tool"]) <= noise[(row["n"], machine.touch)]
    fw.CONTACT = contact
    machine.run([(r["n"], r["kind"], r["tool"], r["lift"], r["swing"], r["turn"], r["via"], r["label"]) for r in rows], None, 0)
    fw.CONTACT = None
    return [[n, kind, by_n[n]["tool"], lift, swing, turn, apart] for n, kind, lift, swing, turn, apart in machine.probed]


def percentiles(v):
    v = sorted(v)
    return [round(v[len(v) // 2], 4), round(v[min(len(v) - 1, int(0.95 * len(v)))], 4), round(v[-1], 4)]


def study(trials):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    d = drawn_of(numbers)
    window = numbers["probe"]["windows"]
    rows = numbers["probe"]["rows"]
    ball_r = numbers["bit"] / 2
    cuts = [(r["t"], r["across"], r["tip"]) for r in numbers["rows"] if r["kind"] == "finish" and r["label"].split()[2] == r["label"].split()[4]]
    drawn = Built(d)
    rng = random.Random(11)
    before, after, left, stopped = [], [], [], 0
    for k in range(trials):
        v, sizes = mis_built(d, rng)
        true = Built(d, v, sizes)
        try:
            found = touches_of(true, rows, window, rng)
        except ValueError:
            stopped += 1
            continue
        w, rms, _ = fit(d, found)
        before.append(cut_error(true, drawn, cuts, ball_r))
        after.append(cut_error(true, Built(d, w), cuts, ball_r))
        left.append(rms)
        if (k + 1) % 20 == 0:
            print(f"  {k + 1} sleds: as built {percentiles(before)}, probed and planned for {percentiles(after)}", flush=True)
    # What each set-up is for: the same, a hundred sleds, with one left out.
    without = {}
    for name, keep in (("the block on end", lambda r: not r["tool"].startswith("post")), ("the block on edge", lambda r: r["tool"] != "edge"),
                       ("the block lying flat", lambda r: r["tool"] != "flat"), ("the pin's second half turn", lambda r: not (r["tool"] == "pin" and r["turn"] != 0.0)),
                       ("two touches of each three", None)):
        rng = random.Random(23)
        errors = []
        for k in range(100):
            v, sizes = mis_built(d, rng)
            true = Built(d, v, sizes)
            if keep is None:
                found = touches_of(true, rows, window, rng, TOUCH * math.sqrt(TOUCHES))
            else:
                found = [t for t, row in zip(touches_of(true, rows, window, rng), [r for r in rows if r["kind"] in ("down", "sw+", "sw-")]) if keep(row)]
            errors.append(cut_error(true, Built(d, fit(d, found)[0]), cuts, ball_r))
        without[name] = percentiles(errors)
        print(f"  without {name}: {without[name]}", flush=True)
    # One sled, the whole way: the controller's own program probing it, the fit, the table for what was found.
    rng = random.Random(4)
    v, sizes = mis_built(d, rng)
    true = Built(d, v, sizes)
    log = firmware_touches(true, rows, window, rng)
    w, rms, worst = fit(d, [(what, lift, swing, turn) for _, _, what, lift, swing, turn, _ in log])
    fitted = Built(d, w)
    table = [["set", *fitted.setting(), *fitted.row(*fitted.setting())]] + [["finish", *c, *fitted.row(*c)] for c in cuts]
    example = {"true": true.v, "sizes": true.sizes, "touches": log, "fitted": w, "rms": rms, "worst_touch": worst, "table": table,
               "error_as_built": cut_error(true, drawn, cuts, ball_r), "error": cut_error(true, fitted, cuts, ball_r)}
    out = {"drawn": d, "names": NAMES, "prior": PRIOR, "touch": TOUCH, "touches": TOUCHES, "windows": window, "pin_r": PIN_R, "block": BLOCK, "post_face": POST_FACE, "ball_r": ball_r,
           "spread": {"dimensions": 0.5, "home": 2.0, "cutter": 1.0, "pin_top": 1.0, "runout": 0.1, "brought": BROUGHT, "master_r": 0.01, "pin_r": 0.004, "block": 0.005},
           "study": {"trials": trials, "stopped": stopped, "touches": len(log), "as_built": percentiles(before), "fitted": percentiles(after), "rms": percentiles(left), "without": without},
           "example": example}
    with open(os.path.join(OUT, "calibration.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(f"{trials} sleds built up to 0.5 mm out, switches 2: {stopped} the probing could not finish. Worst finishing cut, mm (median, 95th, worst): "
          f"as built {percentiles(before)}; probed with {len(log)} touches and planned for {percentiles(after)}")
    print(f"one sled through the controller's own program: {len(log)} touches, left {1000 * rms:.0f} um from the sled fitted; worst cut {example['error_as_built']:.3f} as built, {example['error']:.3f} planned for")


# -- the owner's two commands -----------------------------------------------------

def fit_log(path, master_d=None):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    d = drawn_of(numbers)
    if master_d:
        d["master_r"] = master_d / 2
    what = {r["n"]: r["tool"] for r in numbers["probe"]["rows"] if r["kind"] in ("down", "sw+", "sw-")}
    found = {}
    for line in open(path):
        p = line.strip().split(",")
        if len(p) >= 5 and p[0].isdigit() and int(p[0]) in what:
            found[int(p[0])] = (what[int(p[0])], float(p[2]), float(p[3]), float(p[4]))        # a row probed twice: the later
    missing = sorted(set(what) - set(found))
    if missing:
        raise SystemExit(f"{path} has no touch for rows {missing}: probe them and fit again")
    v, rms, worst = fit(d, [found[n] for n in sorted(found)])
    print(f"{len(found)} touches; the sled that fits them leaves them {1000 * rms:.0f} um out (root mean square), the worst {1000 * worst:.0f}")
    if rms > 0.03:
        print("THAT IS NOT A GOOD FIT: a touch taken on a chip, a clip that slipped, a block that moved, play in a screw's nut. Probe again before trusting it.")
    for i in sorted(range(len(v)), key=lambda i: -abs(v[i] / PRIOR[i]))[:8]:
        print(f"   {NAMES[i]:28s} {v[i]:+.3f}")
    with open(BUILT, "w") as f:
        json.dump({"dims": v[:13], "home": v[13:15], "cutter_y": v[15], "ecc": v[17:19], "master_r": d["master_r"], "pin_top": v[16], "blocks": v[19:],
                   "touches": len(found), "rms": rms, "worst": worst}, f, indent=1)
    print(f"wrote {BUILT}. Now run build.py: it plans the table for this sled. Delete the file to go back to the drawing.")


def as_it_is(numbers):
    """The sled build.py last planned for: the drawn one, or the one built.json says."""
    d = drawn_of(numbers)
    if not os.path.exists(BUILT):
        return Built(d)
    b = json.load(open(BUILT))
    return Built(dict(d, master_r=b["master_r"]), b["dims"] + b["home"] + [b["cutter_y"], 0.0] + b["ecc"])


def align(under_near, under_far, beside_near, beside_far, span=BAND_SPAN):
    stick = json.load(open(os.path.join(OUT, "sled.json")))["stick"]
    up = (under_far - under_near) / 2             # under the stick, a band's radius is the axis's height over the bit
    fence = -(beside_far - beside_near) / 2       # beside it, on the fence's side, the axis's distance from the bit
    again = False
    for off, name, ways, cures in ((up, "under the stick", ("high", "low"), ("down", "up")),
                                   (fence, "beside it", ("toward the fence", "away from the fence"), ("away from the fence", "toward the fence"))):
        way = 0 if off > 0 else 1
        line = f"{name}: over the {span:.0f} mm between the bands the stick's far end runs {abs(off):.2f} mm {ways[way]}."
        if abs(off) < 0.03:
            print(line + " Within what callipers can tell: leave it.")
        else:
            again = True
            print(line + f" Move the tail centre {abs(off) * stick / span:.2f} mm {cures[way]}.")
    if again:
        print("Then `calibrate.py bands` a millimetre under the last radius, turn them again and measure again.")


def bands(radius):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    write_table(os.path.join(OUT, "bands.csv"), band_rows(as_it_is(numbers), radius), numbers["sled"]["safe"])
    print(f"wrote out/bands.csv: bands {2 * radius:.0f} across")


if __name__ == "__main__":
    if len(sys.argv) >= 3 and sys.argv[1] == "fit":
        fit_log(sys.argv[2], float(sys.argv[4]) if len(sys.argv) >= 5 and sys.argv[3] == "--master" else None)
    elif len(sys.argv) >= 6 and sys.argv[1] == "align":
        align(*[float(a) for a in sys.argv[2:7]])
    elif len(sys.argv) >= 3 and sys.argv[1] == "bands":
        bands(float(sys.argv[2]))
    elif len(sys.argv) >= 2 and sys.argv[1] == "study":
        study(int(sys.argv[2]) if len(sys.argv) > 2 else 200)
    else:
        print(__doc__)
