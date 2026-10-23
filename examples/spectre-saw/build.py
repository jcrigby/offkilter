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


def cassette(c, p, B, Z):
    """Cut `c`'s cassette: the base is a block with the region over the
    stick's lower envelope taken out, so the stick drops in, with walls
    either side and at both ends; the lid fills from the stick's upper
    envelope up, between the base's walls, less a groove over the kerf
    up past the blade's top, so the blade never meets it. Both are the
    clearance off the stick. The blade cuts its own slit through the
    base's floor and end walls on the first pass: zero clearance."""
    lower, upper = c["lower"], c["upper"]
    xl, xr = lower[0][0], lower[-1][0]
    H = p["height"]
    drop = [(xl - CLEAR, Z + 1.0), (xl - CLEAR, lower[0][1] - CLEAR)] + shifted(lower, 0.0, -CLEAR) + [(xr + CLEAR, lower[-1][1] - CLEAR), (xr + CLEAR, Z + 1.0)]
    lid = [(xl, upper[0][1] + CLEAR)] + shifted(upper, 0.0, CLEAR) + [(xr, upper[-1][1] + CLEAR), (xr, Z), (xl, Z)]
    x0, x1 = (0.0, KERF) if c["waste_right"] else (-KERF, 0.0)
    groove = [(x0 - GROOVE, -1.0), (x1 + GROOVE, -1.0), (x1 + GROOVE, H + GROOVE), (x0 - GROOVE, H + GROOVE)]
    return drop, lid, groove, (x0, x1)


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
    # One footprint for every cassette, so one pocket in the sled; each
    # as tall as its stick and the lid's cap over the blade need.
    B = math.ceil(max(abs(q[0]) for c in cuts for q in c["held"]) + CLEAR + WALL)
    heights = [math.ceil(max(max(q[1] for q in c["upper"]) + CLEAR, p["height"] + GROOVE) + CAP) for c in cuts]
    print(f"cassettes {2 * B:.0f} across, {L + 2 * WALL:.0f} long, {min(heights):.0f} to {max(heights):.0f} tall")
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Spectre saw"}, {"type": "rename_tab", "tab": 1, "name": "Sequence"}]})
    tabs = {"Sequence": 1}
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
    # -- the cassettes, loaded, after the cut --------------------------------
    for i, c in enumerate(cuts):
        name = f"Cut {i + 1}"
        t = Part(mcp, tabs[name], name)
        Z = heights[i]
        drop, lid, groove, (x0, x1) = cassette(c, p, B, Z)
        base = prism(t, "Base", [(-B, 0.0), (B, 0.0), (B, Z), (-B, Z)], -WALL, L + WALL)
        subtract(t, base, [prism(t, "Base nest", drop, 0.0, L)], name="Base")
        lid_body = prism(t, "Lid", lid, CLEAR, L - CLEAR)
        subtract(t, lid_body, [prism(t, "Lid groove", groove, CLEAR - 1.0, L - CLEAR + 1.0)], name="Lid")
        prism(t, "Stick", c["after"], 0.0, L, material={"name": "Stack", "density": 0.65})
        for k, o in enumerate(c["offcuts"]):
            prism(t, f"Offcut {k + 1}", o, 0.0, L, material={"name": "Stack", "density": 0.65})
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
    shot(tabs["Cut 1"], "cut_1_iso", view="iso", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 2"], "cut_2_section", view="right", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 4"], "cut_4_section", view="right", section=f"x:{L / 2:.1f}:flip")
    shot(tabs["Cut 10"], "cut_10_section", view="right", section=f"x:{L / 2:.1f}:flip")
    for i in range(len(cuts)):
        export(tabs[f"Cut {i + 1}"], "Base", f"cassette_{i + 1}_base")
        export(tabs[f"Cut {i + 1}"], "Lid", f"cassette_{i + 1}_lid")
    print(mcp.call("export", {"tab": tabs["Sequence"], "format": "pdf", "sheet": "A3", "views": ["right"], "parts": False,
                              "note": f"Spectre {EDGE:g} mm edge, gap {GAP:g}, kerf {KERF:g}: the stick after each cut, the blade at {p['height']:.1f}",
                              "path": os.path.join(OUT, "sequence.pdf")}))
    numbers = {
        "program": p,
        "laminate": LAM, "count": COUNT, "plug": PLUG, "length": L,
        "clear": CLEAR, "groove": GROOVE, "cap": CAP, "wall": WALL, "half_width": B, "heights": heights, "pitch": PITCH,
    }
    with open(os.path.join(OUT, "saw.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
