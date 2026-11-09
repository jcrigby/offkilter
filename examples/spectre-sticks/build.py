#!/usr/bin/env python3
"""The Spectre lid's five prisms as sticks, and the fixtures that cut
them, built through the MCP server. The blade stands at 90 degrees for
good and every face of every stick is a through rip on a one-sided
sled: a printed cassette per cut holds the stick turned so the face
stands vertical on the blade's near side, the offcut falling away on
the far side. A flute is sawn at its chord and a bump as a gable, two
flats tangent to the arc at its ends. On the router table a flute is
one pass of a 1/2 in core box with the stick chord down in a cradle,
and a bump two passes of a 1/4 in roundover, one from each gable flat,
the stick standing in a cradle with the flat against the fence. The
kernel's `spectre_sticks` simulates every cut and pass; this script
builds the cassettes and cradles from what it reports.

    cargo build --release -p ok-mcp
    python3 examples/spectre-sticks/build.py
    cargo test -p ok-render --test spectre_sticks    # what CI runs

Writes out/spectre_sticks.okpart, screenshots, every cassette and
cradle as STL (regenerable, not committed), a sequence PDF and
out/sticks.json with the plan and the fixtures' numbers the test reads.
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
# The lid's tile: five sixteenths of a circle at a 10.56 mm edge, the
# 1/4 in roundover and 1/2 in core box; the glue lines are the search's.
EDGE, CURVE, BULGE = 10.56, "arc", 0.2673
GAP = 0.2                                       # the flutes this much deeper than the bumps stand: glue clearance
# The glue lines, in tile units, as the owner chose them in the explorer
# (its Copy cuts, six decimals; the kernel snaps them to the grid). They
# are the search's own answer for this curve; None takes the search.
CUTS = [[[2.366025, 0.633975], [2.133975, 1.5]], [[1, 0], [2.133975, 1.5]], [[1, 0], [0.633975, 2.366025]], [[0, 1], [0.775991, 1.448018]]]
KERF = 3.175                                    # a full-kerf 10 in blade
FLOOR = 4.0                                     # the thinnest cassette floor
MARGIN = 2.0                                    # blank past the section, every side but the datum
SHOULDER = 3.0                                  # the roundover's flat past its arc
# The sticks and their fixtures.
STICK = 240.0                                   # a stick's length: what a printer's bed takes; 16 pieces at 12 + a kerf
THICK = 12.0                                    # the lid's pieces, for the count
CLEAR = 0.15                                    # the nests off the stick, each way
GROOVE = 1.0                                    # the lid's groove off the kerf, each side, and over the blade's top
CAP = 6.0                                       # a lid over its groove or nest
WALL = 6.0                                      # walls either side and at the ends
BEARING = 6.35                                  # the 1/4 in roundover's bearing radius (a 1/2 in bearing): the bit's axis this far behind the gable face
PITCH = 90.0                                    # fixtures in a row, this far apart
# One footprint for every saw cassette and one for every router cradle,
# so the sled has one pocket each and the clamps one setting: worked
# out from the widest stock before anything is built (see main).


def prism(p, name, pts, x0, x1, material=None):
    """A polygon in the station's frame (across the feed, up) extruded
    along the feed, named after its feature."""
    s = p.sketch("right", x0, name)
    p.polygon(s, pts)
    f = p.extrude(s, x1 - x0, name=name)
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        ops.append({"type": "set_part_material", "source": f, "material": material})
    p.apply(*ops)
    return f


def subtract(p, target, tools, name=None):
    return p.feature({"type": "add_boolean", "op": "subtract", "targets": [target], "tools": tools, "name": name})


def shifted(pts, dx, dy):
    return [(q[0] + dx, q[1] + dy) for q in pts]


def nest_below(lower, z_top):
    """The region over a lower envelope, up to z_top: what comes out of a
    base so the stick drops in."""
    xl, xr = lower[0][0], lower[-1][0]
    return [(xl - CLEAR, z_top), (xl - CLEAR, lower[0][1] - CLEAR)] + shifted(lower, 0.0, -CLEAR) + [(xr + CLEAR, lower[-1][1] - CLEAR), (xr + CLEAR, z_top)]


def clip_left(env, xl):
    """An envelope from x = xl on, the first segment cut there."""
    out = []
    for i, q in enumerate(env):
        if q[0] >= xl - 1e-9:
            if not out and i > 0:
                a, b = env[i - 1], q
                t = (xl - a[0]) / (b[0] - a[0])
                out.append((xl, a[1] + (b[1] - a[1]) * t))
            out.append(q)
    return out


def lid_over(upper, z_top, xl=None, xr=None):
    """A lid from an upper envelope up to z_top, between xl and xr."""
    xl = upper[0][0] if xl is None else xl
    xr = upper[-1][0] if xr is None else xr
    upper = clip_left(upper, xl)
    return [(xl, upper[0][1] + CLEAR)] + shifted(upper, 0.0, CLEAR) + [(xr, upper[-1][1] + CLEAR), (xr, z_top), (xl, z_top)]


def clip_right(env, xr):
    """An envelope up to x = xr, the last segment cut there."""
    out = []
    for i, q in enumerate(env):
        if q[0] <= xr + 1e-9:
            out.append(q)
        else:
            if out and i > 0:
                a, b = env[i - 1], q
                t = (xr - a[0]) / (b[0] - a[0])
                out.append((xr, a[1] + (b[1] - a[1]) * t))
            break
    return out


def saw_cassette(t, c, plan, dy, label, foot):
    """A saw cut's cassette, all on the sled's side of the blade, in the
    one footprint `foot` (width B from the blade's near face, height Z):
    a base the kept stock drops into, its nest under the stock's lower
    envelope up to the blade's near face (the blade trims the base's
    edge on the first pass: zero clearance), a lid filling the base's
    walls down to the stock and stopping a groove's width short of the
    blade, its top at Z for every cassette so the clamps never move, the
    stock after the cut, and the offcut where it was, hanging past the
    blade with nothing under it."""
    lower, upper = c["lower"], c["upper"]
    B, Z = foot["B"], foot["Z"]
    H = plan["height"]
    kept_low = clip_right(lower, 0.0)
    base = prism(t, f"{label} base", shifted([(-B, 0.0), (0.0, 0.0), (0.0, Z), (-B, Z)], dy, 0), -WALL, STICK + WALL)
    nest = [(kept_low[0][0] - CLEAR, Z + 1.0), (kept_low[0][0] - CLEAR, kept_low[0][1] - CLEAR)] + shifted(kept_low, 0.0, -CLEAR) + [(1.0, kept_low[-1][1] - CLEAR), (1.0, Z + 1.0)]
    subtract(t, base, [prism(t, f"{label} base nest", shifted(nest, dy, 0), 0.0, STICK)], name=f"{label} base")
    kept_up = clip_right(upper, -GROOVE)
    lid_poly = lid_over(kept_up, Z, kept_low[0][0], -GROOVE)
    lid = prism(t, f"{label} lid", shifted(lid_poly, dy, 0), CLEAR, STICK - CLEAR)
    prism(t, f"{label} stick", shifted(c["after"], dy, 0), 0.0, STICK, material={"name": "Maple", "density": 0.7})
    for k, o in enumerate(c["offcuts"]):
        prism(t, f"{label} offcut {k + 1}", shifted(o, dy, 0), 0.0, STICK, material={"name": "Maple", "density": 0.7})
    blade = [(0.0, 0.0), (KERF, 0.0), (KERF, H), (0.0, H)]
    return {"xl": -B, "xr": 0.0, "height": Z, "base": [(-B, 0.0), (0.0, 0.0), (0.0, Z), (-B, Z)], "nest": nest, "lid": lid_poly, "blade": blade, "stock": c["after"], "offcuts": c["offcuts"]}


def bit_frame(q):
    """A pass's polygons in the router pocket's frame: the bit's axis at
    x = 0 for a flute (as the kernel has it) and for a bump too, where
    the kernel puts the gable face at x = 0 and the bit's axis a bearing
    radius behind it."""
    dx = BEARING if q["kind"] == "bump" else 0.0
    f = lambda poly: shifted(poly, dx, 0.0)
    return {k: f(q[k]) for k in ("before", "after", "lower", "upper")}, [f(o) for o in q["offcuts"]]


def router_cradle(t, q, plan, dy, label, foot):
    """A router pass's cradle in the one pocket `foot`: x from -A to
    W - A with the bit's axis at x = 0, every cradle Z tall. A flute: the
    stick lies chord down on the sled's plate over the bit's slot and a
    hold-down comes down on it, standing on the plate either side. A
    bump: the gable flat stands a bearing radius right of the bit's axis
    facing it, on a base under the stick and under a hold-down, both
    from just behind the flat to the pocket's far wall, which is what
    the cradle registers on; the bit's sweep runs open past the plate's
    slot, since the bit has to enter and leave along the whole stick."""
    r = plan["radius"]
    A, W, Z = foot["A"], foot["W"], foot["Z"]
    xl, xr = -A, W - A
    g, offcuts = bit_frame(q)
    lower, upper = g["lower"], g["upper"]
    parts = {"xl": xl, "xr": xr, "height": Z, "stock": g["after"], "offcuts": offcuts}
    if q["kind"] == "flute":
        hold = prism(t, f"{label} hold-down", shifted([(xl, 0.0), (xr, 0.0), (xr, Z), (xl, Z)], dy, 0), 0.0, STICK)
        nest = [(lower[0][0] - CLEAR, -1.0)] + shifted(upper, 0.0, CLEAR) + [(lower[-1][0] + CLEAR, -1.0)]
        subtract(t, hold, [prism(t, f"{label} nest", shifted(nest, dy, 0), -1.0, STICK + 1.0)], name=f"{label} hold-down")
        bit = [(-r, -3.0), (r, -3.0), (r, plan["flute_depth"] - r)] + [(r * math.cos(a), plan["flute_depth"] - r + r * math.sin(a)) for a in [math.radians(d) for d in range(10, 180, 10)]] + [(-r, plan["flute_depth"] - r)]
        prism(t, f"{label} bit", shifted(bit, dy, 0), STICK / 2 - 1.0, STICK / 2 + 1.0, material={"name": "Steel", "density": 7.8})
        prism(t, f"{label} stick", shifted(g["after"], dy, 0), 0.0, STICK, material={"name": "Maple", "density": 0.7})
        parts.update({"lid": [(xl, 0.0), (xr, 0.0), (xr, Z), (xl, Z)], "lid_nest": nest, "bit": bit})
        return parts
    # a bump: the base and hold-down start just behind the gable flat; the bit's sweep (BEARING + r + SHOULDER from its axis) stays left of them
    foot_y = q["height"]
    base_box = [(BEARING + CLEAR, 0.0), (xr, 0.0), (xr, Z), (BEARING + CLEAR, Z)]
    base = prism(t, f"{label} base", shifted(base_box, dy, 0), 0.0, STICK)
    nest = nest_below(lower, Z + 1.0)
    subtract(t, base, [prism(t, f"{label} nest", shifted(nest, dy, 0), -1.0, STICK + 1.0)], name=f"{label} base")
    lid_poly = lid_over(upper, Z, max(BEARING + CLEAR, upper[0][0]), lower[-1][0])
    prism(t, f"{label} hold-down", shifted(lid_poly, dy, 0), CLEAR, STICK - CLEAR)
    arc = [(BEARING - (r - r * math.cos(math.radians(d))), foot_y - r * math.sin(math.radians(d))) for d in range(90, -1, -10)]
    bit = [(BEARING, foot_y + 4.0), (-BEARING, foot_y + 4.0), (-BEARING, foot_y - r - 2.0), (BEARING - r - SHOULDER, foot_y - r - 2.0), (BEARING - r - SHOULDER, foot_y - r)] + arc
    prism(t, f"{label} bit", shifted(bit, dy, 0), STICK / 2 - 1.0, STICK / 2 + 1.0, material={"name": "Steel", "density": 7.8})
    prism(t, f"{label} stick", shifted(g["after"], dy, 0), 0.0, STICK, material={"name": "Maple", "density": 0.7})
    for k, o in enumerate(offcuts):
        prism(t, f"{label} offcut {k + 1}", shifted(o, dy, 0), 0.0, STICK, material={"name": "Maple", "density": 0.7})
    parts.update({"base": base_box, "nest": nest, "lid": lid_poly, "bit": bit})
    return parts


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "spectre_sticks.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "spectre_sticks"})
    plan = json.loads(mcp.call("spectre_sticks", {"edge": EDGE, "curve": CURVE, "bulge": BULGE, "gap": GAP, "kerf": KERF, "floor": FLOOR, "margin": MARGIN, "shoulder": SHOULDER, **({"cuts": CUTS} if CUTS else {})}))
    print(f"arc {plan['theta']:.1f} degrees, radius {plan['radius']:.2f} mm, sagitta {plan['sagitta']:.2f}, flute {plan['flute_depth']:.2f} deep, ridge {plan['ridge']:.2f}; blade {plan['height']:.1f} over the plate")
    print(f"bits: {plan['bits']}")
    per_stick = int((STICK - 2 * WALL) // (THICK + KERF))
    for st in plan["sticks"]:
        faces = " ".join(f"{f['kind']}{'' if f['side'] is None else f['side']} {f['len']:.1f}" for f in st["faces"])
        print(f"stick {st['prism'] + 1} (shape {st['shape']}): blank {st['blank'][1] - st['blank'][0]:.1f} x {st['blank'][2]:.1f} on glue line {st['datum']}; faces {faces}")
        for c in st["saw"]:
            print(f"   saw  {c['kind']:5s} face {c['face']}: turn {c['turn']:5.1f}" + (" end for end" if c["mirrored"] else "") + f", offcut {len(c['offcuts'])}" + (f" {c['thin']:.1f} thin, above" if c["offcuts"] else ""))
        for q in st["router"]:
            print(f"   rout {q['kind']:5s} face {q['face']}: turn {q['turn']:5.1f}" + (" end for end" if q["mirrored"] else "") + f", bit {q['height']:.2f} up" + (f", bed {q['bed']:.1f}" if q["bed"] else ""))
        print(f"   off the tile's prism by {st['off']:.2f} mm2 (the flutes {GAP} deeper)")
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Spectre sticks"}, {"type": "rename_tab", "tab": 1, "name": "Sequence"}]})
    tabs = {"Sequence": 1}
    for st in plan["sticks"]:
        name = f"Stick {st['prism'] + 1}"
        text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": name}]})
        tabs[name] = int(re.search(r"tab (\d+)", text).group(1))
    # -- the footprints: one for the saw, one for the router -----------------
    saw_cuts = [c for st in plan["sticks"] for c in st["saw"]]
    B = math.ceil(max(-c["lower"][0][0] for c in saw_cuts) + CLEAR + WALL)
    Zs = math.ceil(max(max(max(v[1] for v in c["upper"]) + CLEAR, plan["height"] + GROOVE) for c in saw_cuts) + CAP)
    saw_foot = {"B": B, "Z": Zs}
    passes = [bit_frame(q)[0] for st in plan["sticks"] for q in st["router"]]
    A = math.ceil(max(-g["lower"][0][0] for g in passes) + CLEAR + WALL)
    W = A + math.ceil(max(g["lower"][-1][0] for g in passes) + CLEAR + WALL)
    Zr = math.ceil(max(max(v[1] for v in g["upper"]) for g in passes) + CLEAR + CAP)
    router_foot = {"A": A, "W": W, "Z": Zr}
    print(f"saw cassettes: one footprint, {B} wide from the blade by {STICK + 2 * WALL:.0f} long, {Zs} tall, blade {plan['height']:.1f} over the plate")
    print(f"router cradles: one pocket, {W} wide by {STICK:.0f} long with the bit's axis {A} in from the near wall, {Zr} tall; core box up {plan['flute_depth']:.2f}, roundover bearing {plan['bump_height']:.2f} up")
    parts = []
    # -- the sequence: each stick a row, the stock after every cut and pass --
    seq = Part(mcp, tabs["Sequence"], "Sequence")
    for j, st in enumerate(plan["sticks"]):
        dz = -j * 60.0
        stages = [(f"Stick {j + 1} blank", [(st["blank"][0], 0.0), (st["blank"][1], 0.0), (st["blank"][1], st["blank"][2]), (st["blank"][0], st["blank"][2])])]
        for c in st["saw"]:
            back = lambda q, c=c: rot(q, -c["turn"], c["shift"], c["mirrored"])
            stages.append((f"Stick {j + 1} after saw {c['face']}", [back(q) for q in c["after"]]))
        stages.append((f"Stick {j + 1} routed", st["finished"]))
        for i, (name, poly) in enumerate(stages):
            prism(seq, name, shifted(poly, i * 45.0, dz), 0.0, 10.0, material={"name": "Maple", "density": 0.7})
    parts.append(seq)
    # -- the fixtures, a tab per stick, in a row --------------------------
    fixtures = []
    for st in plan["sticks"]:
        name = f"Stick {st['prism'] + 1}"
        t = Part(mcp, tabs[name], name)
        row = []
        dy = 0.0
        for i, c in enumerate(st["saw"]):
            fx = saw_cassette(t, c, plan, dy, f"Saw {i + 1}", saw_foot)
            row.append({"kind": "saw", "face": c["face"], "dy": dy, **fx})
            dy += PITCH
        for i, q in enumerate(st["router"]):
            fx = router_cradle(t, q, plan, dy, f"Pass {i + 1}", router_foot)
            row.append({"kind": q["kind"], "face": q["face"], "dy": dy, **fx})
            dy += PITCH
        fixtures.append(row)
        parts.append(t)
    for part in parts:
        r = part.report()
        bad = [(ft["name"], ft["error"]) for ft in r["features"] if ft.get("error")]
        if bad:
            raise RuntimeError(f"{part.name}: {bad}")
    # -- pictures and files ----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(tabs["Sequence"], "sequence", view="right")
    shot(tabs["Stick 1"], "stick_1_fixtures", view="iso")
    span = max(B + 30, W + 10)
    shot(tabs["Stick 1"], "stick_1_saw_1", view="right", section=f"x:{STICK / 2:.1f}:flip", fit=f"-60,{fixtures[0][0]['dy'] - B - 5},-5,10,{fixtures[0][0]['dy'] - B - 5 + span},{Zs + 5}")
    first_flute = next(f for f in fixtures[0] if f["kind"] == "flute")
    first_bump = next(f for f in fixtures[0] if f["kind"] == "bump")
    shot(tabs["Stick 1"], "stick_1_flute", view="right", section=f"x:{STICK / 2:.1f}:flip", fit=f"-60,{first_flute['dy'] - A - 5},-5,10,{first_flute['dy'] - A - 5 + span},{Zr + 5}")
    shot(tabs["Stick 1"], "stick_1_bump", view="right", section=f"x:{STICK / 2:.1f}:flip", fit=f"-60,{first_bump['dy'] - A - 5},-5,10,{first_bump['dy'] - A - 5 + span},{Zr + 5}")
    for st, row in zip(plan["sticks"], fixtures):
        tab = tabs[f"Stick {st['prism'] + 1}"]
        bodies = [b["name"] for b in Part(mcp, tab, "x").report()["bodies"]]
        for b in bodies:
            if b.endswith(" base") or b.endswith(" lid") or b.endswith(" hold-down"):
                mcp.call("export", {"tab": tab, "format": "stl", "body": b, "path": os.path.join(OUT, f"stick_{st['prism'] + 1}_{b.lower().replace(' ', '_')}.stl")})
    print(mcp.call("export", {"tab": tabs["Sequence"], "format": "pdf", "sheet": "A3", "views": ["right"], "parts": False,
                              "note": f"Spectre {EDGE:g} mm edge, five sixteenths arcs: each stick after every cut, blade {plan['height']:.1f} over the plate",
                              "path": os.path.join(OUT, "sequence.pdf")}))
    numbers = {"plan": plan, "stick": STICK, "thick": THICK, "per_stick": per_stick, "clear": CLEAR, "groove": GROOVE, "cap": CAP, "wall": WALL, "bearing": BEARING, "pitch": PITCH,
               "saw_footprint": saw_foot, "router_footprint": router_foot, "fixtures": fixtures}
    with open(os.path.join(OUT, "sticks.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


def rot(q, deg, shift, mirrored=False):
    """Back from a station frame: unshift, unmirror, then turn."""
    x, y = q[0] - shift[0], q[1] - shift[1]
    if mirrored:
        x = -x
    a = math.radians(deg)
    return (x * math.cos(a) - y * math.sin(a), x * math.sin(a) + y * math.cos(a))


if __name__ == "__main__":
    main()
