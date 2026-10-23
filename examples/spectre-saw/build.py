#!/usr/bin/env python3
"""The straight Spectre stick on a table saw, built through the MCP
server: a sled whose blade is set once, and a printed cassette per cut
that holds the stick the only way it fits, turned so the face stands
vertical, placed so the face's line falls on the blade, its floor
putting the inside corner at the blade's top. The kernel's `spectre_saw`
simulates the ten rips in order; this script builds the cassettes from
what it reports, a base the stick drops into and a lid that comes down
on it, so every offcut is captured, the stick after each cut with what
came off, and a row of the sections for reading the program.

    cargo build --release -p ok-mcp
    python3 examples/spectre-saw/build.py
    cargo test -p ok-render --test spectre_saw    # what CI runs

Writes out/spectre_saw.okpart, screenshots, the cassettes as STL (a
base and a lid each), and out/saw.json with the program and the
cassettes' numbers the test checks.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(HERE, "out")
# The tile the spectre-cage's stack is cut to: the spectre-box's top at
# a 9 mm edge, a 0.7 mm gap between the pieces; the straight tile, which
# a saw makes exactly.
EDGE, GAP = 9.0, 0.7
KERF = 3.175                                    # a full-kerf 10 in blade, 1/8 in
MARGIN = 2.0                                    # blank past the notched end
FLOOR = 4.0                                     # the thinnest cassette floor, under the through cuts
BIAS = 0.1                                      # a rabbet's blade aimed this far past the corner
# The stick: the cage's stack, one laminate per piece and two spares,
# between two sacrificial plugs the blade cuts with it, so they come out
# as the tile too and locate the stack in every cassette.
LAM, COUNT, PLUG = 3.0, 26, 8.0
L = COUNT * LAM + 2 * PLUG
# The cassettes.
CLEAR = 0.15                                    # the nests off the stick, each way
GROOVE = 1.0                                    # the lid's groove off the kerf, each side, and over the blade's top
CAP = 6.0                                       # the lid over its groove
WALL = 6.0                                      # the base's side and end walls
PITCH = 110.0                                   # the sequence rows: five cuts to a row
# Every cassette's base is two halves, one each side of the blade, a
# hair into its path so the first pass trims them to zero clearance and
# never cuts them apart. Each half stands on the sled's printed plate:
# a tongue along the feed in a groove sets it across the bed (the one
# fit that decides where the face lands), a short cross key at its front
# end in a cross groove sets it along the feed, and a toggle clamp on
# the 3/4 in bed presses a flange along its outer edge down at one
# height for every cassette. The plate is thinner than the bed, sits on
# the saw table in a pocket routed from the bed's underside, and is
# screwed up into the bed through slotted holes in its lip.
TRIM = 0.25                                     # each half's inner edge this far into the blade's path
KEY_W, KEY_H = 6.0, 3.0                         # the tongue and the cross key, wide and deep
KEY_CLEAR = 0.1                                 # the grooves off the keys, each side; the grooves 0.2 deeper than the keys
KEY_OFF = {"left": 16.0, "right": 22.0}         # tongue centres from the blade's faces; unequal, so a half fits one side only
CROSS_AT, CROSS_W, CROSS_LEN = 4.0, 6.0, 14.0   # the cross key: from the half's front face, along the feed, outward from the tongue
LEDGE_H, LEDGE_W = 16.0, 10.0                   # the clamp flange: its top over the plate, its width outside the half
PLATE_T = 6.0                                   # the printed plate, on the saw table
PLY_T = 19.05                                   # the sliding bed, 3/4 in ply, flush with the plate underneath
LIP = 15.0                                      # the plate past the bed's opening, all round, under the bed
OPEN_MARGIN = 2.0                               # the bed's opening off the flanges and the halves' ends
SCREW = 4.5                                     # the lip's screw slots, wide, and 8 long across the blade
BLADE_D = 254.0                                 # a 10 in blade
STOP = 5.0                                      # the sled stops with the stick's rear end this far past the blade's top
PATH_CLEAR = 0.5                                # the model's slots past the blade's circle (the blade cuts its own)
CLAMP_AT = (0.25, 0.75)                         # the clamps along each half, as fractions of the stick


def prism(p, name, pts, x0, x1, material=None):
    """A polygon in the station's frame (across the sled, up) extruded
    along the feed, named after its feature."""
    s = p.sketch("right", x0, name)
    p.polygon(s, pts)
    f = p.extrude(s, x1 - x0, name=name)
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        ops.append({"type": "set_part_material", "source": f, "material": material})
    p.apply(*ops)
    return f


def union(p, target, tools, name=None):
    return p.feature({"type": "add_boolean", "op": "union", "targets": [target], "tools": tools, "name": name})


def subtract(p, target, tools, name=None):
    return p.feature({"type": "add_boolean", "op": "subtract", "targets": [target], "tools": tools, "name": name})


def shifted(pts, dx, dy):
    return [(q[0] + dx, q[1] + dy) for q in pts]


def offset(c):
    """Where cut `c`'s frame lands in the sled's: the program puts the
    blade's near face at 0 with the kerf to the right or the left; on
    the sled the blade is fixed, its body from 0 to the kerf."""
    return 0.0 if c["waste_right"] else KERF


def placed(c, key):
    return shifted(c[key], offset(c), 0.0)


def cassette(c, p, Z):
    """Cut `c`'s cassette, in the sled's frame: the nest is the region
    over the stick's lower envelope, taken out of the base so the stick
    drops in; the lid fills from the stick's upper envelope up, between
    the base's walls, less a groove over the blade's path up past its
    top, so the blade never meets it. Both are the clearance off the
    stick."""
    lower, upper = placed(c, "lower"), placed(c, "upper")
    xl, xr = lower[0][0], lower[-1][0]
    H = p["height"]
    drop = [(xl - CLEAR, Z + 1.0), (xl - CLEAR, lower[0][1] - CLEAR)] + shifted(lower, 0.0, -CLEAR) + [(xr + CLEAR, lower[-1][1] - CLEAR), (xr + CLEAR, Z + 1.0)]
    lid = [(xl, upper[0][1] + CLEAR)] + shifted(upper, 0.0, CLEAR) + [(xr, upper[-1][1] + CLEAR), (xr, Z), (xl, Z)]
    groove = [(-GROOVE, -1.0), (KERF + GROOVE, -1.0), (KERF + GROOVE, H + GROOVE), (-GROOVE, H + GROOVE)]
    return drop, lid, groove


def rect(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


class Station:
    """The interface every cassette shares, in the sled's frame: across
    the bed `x` (the blade's body from 0 to the kerf), up `y` from the
    plate's top, along the feed `z` (the stick from 0 to L, its front
    end first into the blade)."""

    def __init__(self, cuts, H):
        self.H = H
        mid = KERF / 2
        reach = max(abs(q[0] + offset(c) - mid) for c in cuts for q in c["held"])
        self.B = math.ceil(reach + CLEAR + WALL)
        self.xa, self.xb = mid - self.B, mid + self.B               # the halves' outer faces
        self.inner = {"left": TRIM, "right": KERF - TRIM}            # their inner edges, in the blade's path
        self.tongue = {"left": -KEY_OFF["left"], "right": KERF + KEY_OFF["right"]}
        self.out = {"left": -1.0, "right": 1.0}                      # outward, across
        self.z0, self.z1 = -WALL, L + WALL                           # the halves along the feed
        self.ox0, self.ox1 = self.xa - LEDGE_W - OPEN_MARGIN, self.xb + LEDGE_W + OPEN_MARGIN
        self.oz0 = self.z0 - OPEN_MARGIN
        # The blade's path: at height y its edge reaches c(y) behind its
        # top, the circle's half chord, and the sled stops with the
        # stick's rear end STOP past the top.
        R = BLADE_D / 2
        self.R = R
        self.end = L + STOP
        self.reach = lambda y: math.sqrt(max(R * R - (y - (H - R)) ** 2, 0.0))
        slot_end = self.end + self.reach(-PLATE_T) + PATH_CLEAR
        self.pz1 = max(self.z1 + OPEN_MARGIN + LIP, math.ceil(slot_end + 25.0))
        self.oz1 = self.pz1 - LIP
        self.pz0 = self.oz0 - LIP
        self.px0, self.px1 = self.ox0 - LIP, self.ox1 + LIP
        self.fz0, self.fz1 = self.pz0 - 60.0, self.pz1 + 60.0         # the bed along the feed
        self.fx0, self.fx1 = self.px0 - 60.0, self.px1 + 60.0
        self.ply_top = PLY_T - PLATE_T
        assert self.xa + LEDGE_W < self.tongue["left"] - KEY_W / 2 - CROSS_LEN, "the cross key runs off the left half"
        assert self.tongue["right"] + KEY_W / 2 + CROSS_LEN < self.xb, "the cross key runs off the right half"
        assert LEDGE_H > self.ply_top, "the flange's top has to stand above the bed for the clamp"

    def side_x(self, side):
        return (self.xa, self.inner["left"]) if side == "left" else (self.inner["right"], self.xb)

    def key(self, side, clear=0.0, deep=0.0):
        """The tongue and the cross key under a half (or, with clearance,
        the grooves for them): the tongue along the half's whole length,
        the cross key outward from it at the front end."""
        t, o = self.tongue[side], self.out[side]
        y0 = -KEY_H - deep
        tongue = rect(t - KEY_W / 2 - clear, y0, t + KEY_W / 2 + clear, 0.0)
        e = t + o * (KEY_W / 2)
        cx0, cx1 = sorted((e - o * clear, e + o * (CROSS_LEN + clear)))
        cross = rect(cx0, y0, cx1, 0.0)
        cz = (self.z0 + CROSS_AT - clear, self.z0 + CROSS_AT + CROSS_W + clear)
        return tongue, cross, cz

    def pads(self, side):
        """Where the clamps press, across and along the feed: on the
        flange's middle, at the given fractions of the stick."""
        x = self.xa - LEDGE_W / 2 if side == "left" else self.xb + LEDGE_W / 2
        return [(x - 4.0, x + 4.0, f * L - 6.0, f * L + 6.0) for f in CLAMP_AT]

    def sweep(self, y_bottom):
        """The blade's path in the feed-up plane, from the bed's front
        edge to where it reaches at the sled's stop, down to y_bottom."""
        top = [(self.fz0 - 1.0, y_bottom), (self.fz0 - 1.0, self.H + PATH_CLEAR)]
        ys = [self.H - k * 1.0 for k in range(int(self.H - y_bottom) + 1)] + [y_bottom]
        arc = [(self.end + self.reach(y) + PATH_CLEAR, y) for y in ys]
        return top + [(self.end + PATH_CLEAR, self.H + PATH_CLEAR)] + arc


def half(t, st, side, c, Z, drop):
    """One half of a base: its block from the outer face to its edge in
    the blade's path, end walls included, the nest out of it, the
    flange along its outer edge, and the keys under it."""
    x0, x1 = st.side_x(side)
    name = f"Base {side}"
    body = prism(t, name, rect(x0, 0.0, x1, Z), st.z0, st.z1)
    fx = (st.xa - LEDGE_W, st.xa) if side == "left" else (st.xb, st.xb + LEDGE_W)
    flange = prism(t, f"{name} flange", rect(fx[0], 0.0, fx[1], LEDGE_H), st.z0, st.z1)
    tongue, cross, cz = st.key(side)
    keys = [prism(t, f"{name} tongue", tongue, st.z0, st.z1), prism(t, f"{name} cross key", cross, *cz)]
    union(t, body, [flange] + keys, name=name)
    subtract(t, body, [prism(t, f"{name} nest", drop, 0.0, L)], name=name)
    return body


def blade_cut(t, name, poly):
    """The blade's path as a solid, its kerf wide across the bed."""
    s = t.sketch("front", -KERF, name)
    t.polygon(s, poly)
    f = t.extrude(s, KERF, name=name)
    t.apply({"type": "rename_part", "source": f, "name": name})
    return f


def plate(t, st):
    """The printed plate: the bed's opening and its lip, the grooves for
    every half's keys, slotted screw holes in the lip, and the slot the
    blade cuts through it, which stops short of its rear edge."""
    body = prism(t, "Plate", rect(st.px0, -PLATE_T, st.px1, 0.0), st.pz0, st.pz1)
    cuts = []
    for side in ("left", "right"):
        tongue, cross, cz = st.key(side, KEY_CLEAR, 0.2)
        cuts.append(prism(t, f"Plate groove {side}", tongue, st.oz0, st.oz1))
        cuts.append(prism(t, f"Plate cross groove {side}", cross, *cz))
    s = t.sketch("top", -PLATE_T - 1.0, "Plate screw slots")
    zs = [st.pz0 + LIP / 2, (st.pz0 + st.pz1) / 2, st.pz1 - LIP / 2]
    for z in zs:
        for x in (st.px0 + LIP / 2, st.px1 - LIP / 2):
            t.slot(s, (z, x - 4.0), (z, x + 4.0), SCREW)
    for z in (st.pz0 + LIP / 2, st.pz1 - LIP / 2):
        for x in (st.xa / 2, (st.xb + KERF) / 2):
            t.slot(s, (z, x - 4.0), (z, x + 4.0), SCREW)
    slots = t.extrude(s, PLATE_T + 2.0, name="Plate screw slots")
    t.apply({"type": "rename_part", "source": slots, "name": "Plate screw slots"})
    cuts += [slots, blade_cut(t, "Plate blade path", st.sweep(-PLATE_T - 1.0))]
    subtract(t, body, cuts, name="Plate")
    return body


def bed(t, st):
    """The 3/4 in sliding bed: the opening the halves stand in, the
    pocket under it the plate fills, the blade's path."""
    body = prism(t, "Bed", rect(st.fx0, -PLATE_T, st.fx1, st.ply_top), st.fz0, st.fz1, material={"name": "Birch ply", "density": 0.68})
    opening = prism(t, "Bed opening", rect(st.ox0, -PLATE_T - 1.0, st.ox1, st.ply_top + 1.0), st.oz0, st.oz1)
    pocket = prism(t, "Bed pocket", rect(st.px0, -PLATE_T - 1.0, st.px1, 0.0), st.pz0, st.pz1)
    path = blade_cut(t, "Bed blade path", st.sweep(-PLATE_T - 1.0))
    subtract(t, body, [opening, pocket, path], name="Bed")
    return body


def clamp(t, st, side, k, pad):
    """A vertical toggle clamp, closed: its base screwed to the bed
    outside the opening, its arm over the flange, its spindle's pad on
    the flange's top."""
    x0, x1, z0, z1 = pad
    o = st.out[side]
    edge = st.ox0 if side == "left" else st.ox1
    b0, b1 = sorted((edge + o * 8.0, edge + o * 40.0))
    zc = (z0 + z1) / 2
    name = f"Clamp {side} {k + 1}"
    base = prism(t, name, rect(b0, st.ply_top, b1, st.ply_top + 8.0), zc - 12.0, zc + 12.0)
    post_x = (b1 - 10.0, b1) if side == "left" else (b0, b0 + 10.0)
    post = prism(t, f"{name} post", rect(post_x[0], st.ply_top + 8.0, post_x[1], LEDGE_H + 22.0), zc - 8.0, zc + 8.0)
    ax0, ax1 = sorted((x1 if side == "left" else x0, post_x[1] if side == "left" else post_x[0]))
    arm = prism(t, f"{name} arm", rect(ax0, LEDGE_H + 16.0, ax1, LEDGE_H + 22.0), zc - 6.0, zc + 6.0)
    spindle = prism(t, f"{name} pad", rect(x0, LEDGE_H, x1, LEDGE_H + 16.0), z0, z1)
    union(t, base, [post, arm, spindle], name=name)
    return base


def load(t, st, c, p, Z):
    """Cut `c`'s cassette on the plate: the two halves, the lid, the
    stick after the cut and what came off."""
    drop, lid, groove = cassette(c, p, Z)
    for side in ("left", "right"):
        half(t, st, side, c, Z, drop)
    lid_body = prism(t, "Lid", lid, CLEAR, L - CLEAR)
    subtract(t, lid_body, [prism(t, "Lid groove", groove, CLEAR - 1.0, L - CLEAR + 1.0)], name="Lid")
    prism(t, "Stick", placed(c, "after"), 0.0, L, material={"name": "Stack", "density": 0.65})
    for k, o in enumerate(c["offcuts"]):
        prism(t, f"Offcut {k + 1}", shifted(o, offset(c), 0.0), 0.0, L, material={"name": "Stack", "density": 0.65})


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_saw.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "spectre_saw"})
    p = json.loads(mcp.call("spectre_saw", {"edge": EDGE, "gap": GAP, "kerf": KERF, "margin": MARGIN, "floor": FLOOR, "bias": BIAS}))
    cuts = p["cuts"]
    print(f"blank {p['width']:.2f} wide, {p['thickness']:.2f} thick; blade {p['height']:.2f} over the plate; {len(cuts)} cuts; "
          f"{p['nicked']:.2f} mm2 nicked past the corners, {p['left']:.4f} left")
    for i, c in enumerate(cuts):
        print(f"  {i + 1:2d}. face {c['face']:2d} {c['kind']:7s} turn {c['turn']:3.0f} kerf {'right' if c['waste_right'] else 'left '} bed {c['bed']:5.2f}"
              + (f" corner {c['corner']}" if c["corner"] is not None else "") + f" offcuts {len(c['offcuts'])}" + ("" if c["thin"] is None else f" thinnest {c['thin']:.2f}")
              + (" takes the blank either way round" if c["symmetric"] else ""))
    # One footprint and one interface for every cassette; each as tall
    # as its stick and the lid's cap over the blade need.
    st = Station(cuts, p["height"])
    heights = [math.ceil(max(max(q[1] for q in c["upper"]) + CLEAR, p["height"] + GROOVE) + CAP) for c in cuts]
    print(f"cassettes {st.xb - st.xa + 2 * LEDGE_W:.0f} across with their flanges, {L + 2 * WALL:.0f} long, {min(heights):.0f} to {max(heights):.0f} tall")
    print(f"plate {st.px1 - st.px0:.0f} x {st.pz1 - st.pz0:.0f} x {PLATE_T:g}; blade {p['height'] + PLATE_T:.1f} over the saw table; "
          f"the blade's path ends {st.end + st.reach(-PLATE_T):.0f} along the feed, {st.pz1 - st.end - st.reach(-PLATE_T):.0f} short of the plate's rear edge")
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Spectre saw"}, {"type": "rename_tab", "tab": 1, "name": "Sequence"}]})
    tabs = {"Sequence": 1}
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "Station"}]})
    tabs["Station"] = int(re.search(r"tab (\d+)", text).group(1))
    for i, c in enumerate(cuts):
        name = f"Cut {i + 1}"
        text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": name}]})
        tabs[name] = int(re.search(r"tab (\d+)", text).group(1))
    parts = []
    # -- the sequence: each cut's stock after it, what came off, and the
    #    blade, in a row --------------------------------------------------
    seq = Part(mcp, tabs["Sequence"], "Sequence")
    for i, c in enumerate(cuts):
        dy, dz = (i % 5) * PITCH, -(i // 5) * PITCH
        prism(seq, f"Stick {i + 1}", shifted(c["after"], dy, dz), 0.0, 10.0)
        for k, o in enumerate(c["offcuts"]):
            prism(seq, f"Offcut {i + 1}.{k + 1}", shifted(o, dy, dz), 0.0, 10.0)
        x0, x1 = (0.0, KERF) if c["waste_right"] else (-KERF, 0.0)
        prism(seq, f"Blade {i + 1}", [(x0 + dy, dz), (x1 + dy, dz), (x1 + dy, p["height"] + dz), (x0 + dy, p["height"] + dz)], -5.0, 15.0)
    parts.append(seq)
    # -- the cassettes, loaded, after the cut, each on the plate -------------
    for i, c in enumerate(cuts):
        name = f"Cut {i + 1}"
        t = Part(mcp, tabs[name], name)
        load(t, st, c, p, heights[i])
        plate(t, st)
        parts.append(t)
    # -- the station: the bed, the plate, the clamps, the fourth cassette ----
    t = Part(mcp, tabs["Station"], "Station")
    bed(t, st)
    plate(t, st)
    load(t, st, cuts[3], p, heights[3])
    for side in ("left", "right"):
        for k, pad in enumerate(st.pads(side)):
            clamp(t, st, side, k, pad)
    parts.append(t)
    for part in parts:
        r = part.report()
        bad = [(ft["name"], ft["error"]) for ft in r["features"] if ft.get("error")]
        if bad:
            raise RuntimeError(f"{part.name}: {bad}")
    # -- pictures and files ----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    export = lambda tab, body, name: mcp.call("export", {"tab": tab, "format": "stl", "body": body, "path": os.path.join(OUT, f"{name}.stl")})
    shot(tabs["Sequence"], "sequence", view="right")
    shot(tabs["Station"], "station_iso", view="iso")
    shot(tabs["Station"], "station_section", view="right", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 1"], "cut_1_iso", view="iso", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 2"], "cut_2_section", view="right", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 4"], "cut_4_section", view="right", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 10"], "cut_10_section", view="right", section=f"x:{L / 2:.1f}:flip")
    for i in range(len(cuts)):
        export(tabs[f"Cut {i + 1}"], "Base left", f"cassette_{i + 1}_left")
        export(tabs[f"Cut {i + 1}"], "Base right", f"cassette_{i + 1}_right")
        export(tabs[f"Cut {i + 1}"], "Lid", f"cassette_{i + 1}_lid")
    export(tabs["Station"], "Plate", "plate")
    print(mcp.call("export", {"tab": tabs["Sequence"], "format": "pdf", "sheet": "A3", "views": ["right"], "parts": False,
                              "note": f"Spectre {EDGE:g} mm edge, gap {GAP:g}, kerf {KERF:g}: the stick after each cut, the blade at {p['height']:.1f}",
                              "path": os.path.join(OUT, "sequence.pdf")}))
    numbers = {
        "program": p,
        "laminate": LAM, "count": COUNT, "plug": PLUG, "length": L,
        "clear": CLEAR, "groove": GROOVE, "cap": CAP, "wall": WALL, "heights": heights, "pitch": PITCH,
        "offsets": [offset(c) for c in cuts],
        "station": {
            "trim": TRIM, "key_w": KEY_W, "key_h": KEY_H, "key_clear": KEY_CLEAR, "key_off": KEY_OFF,
            "cross_at": CROSS_AT, "cross_w": CROSS_W, "cross_len": CROSS_LEN, "ledge_h": LEDGE_H, "ledge_w": LEDGE_W,
            "plate_t": PLATE_T, "ply_t": PLY_T, "lip": LIP, "blade_d": BLADE_D, "stop": STOP,
            "half": {"left": list(st.side_x("left")), "right": list(st.side_x("right"))}, "z": [st.z0, st.z1],
            "tongue": st.tongue, "plate": [st.px0, st.px1, st.pz0, st.pz1], "bed": [st.fx0, st.fx1, st.fz0, st.fz1],
            "opening": [st.ox0, st.ox1, st.oz0, st.oz1], "end": st.end,
            "pads": {side: st.pads(side) for side in ("left", "right")},
        },
    }
    with open(os.path.join(OUT, "saw.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
