#!/usr/bin/env python3
"""The finch feeder cap (examples/finch-plug) with a perch ring for the
small birds that cannot cling to the mesh, sized so the Eurasian
collared doves cannot use it: the same plate, plug, hooks and drains,
plus a thin ring of perch round the cap on three spokes from the hook
lobes, standing off the mesh by less than a dove's body and offering
nothing flat. The cap builds through the original script's functions,
so a change there (the ring's numbers from calipers, the ramp's bite)
carries into this one.

    cargo build --release -p ok-mcp
    python3 examples/finch-perch/build.py
    cargo test -p ok-render --test finch_perch   # what CI runs

Writes out/finch_perch.okpart, out/perch_cap.stl (the part to print),
pictures, a sheet, and out/perch.json.
"""
import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

# The original cap: its numbers, its builders and its ring.
_spec = importlib.util.spec_from_file_location("finch_plug", os.path.join(os.path.dirname(HERE), "finch-plug", "build.py"))
plug = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(plug)

OUT = os.path.join(HERE, "out")
# -- the perch, mm -----------------------------------------------------------
# A small bird's perch is a pencil: finch feeders use 5 to 6.5 mm pegs
# and the ring perches on commercial finch feeders are 4 to 5 mm wire.
# A collared dove is 150 to 200 g, 65 to 70 mm through the body and
# 30 cm long; what keeps it off is not the perch's thickness (doves sit
# on wires) but the room it has: standing on the ring, facing in, its
# chest would be where the mesh tube is, and its tail would hit the
# ground on a feeder hung low. So the ring stands off the tube by less
# than a dove is deep and the plate above it, the ring's own size, is
# nothing to stand on either.
PERCH_R = 51.0          # the perch's centre line from the axis: 108 mm across the outside
PERCH_W = 6.0           # the perch's width: the 1/4 in dowel finch feeders use
PERCH_H = 6.0           # its height: a flat bottom level with the plate's, straight sides, a round top
SPOKES = plug.HOOKS     # one spoke per hook, continuing its lobe
SPOKE_W = 8.0           # the spokes' width
SPOKE_T = plug.PLATE_T + 1.0   # and thickness, from the plate's underside: a millimetre proud of the plate, so their tops end inside the hook walls rather than on the walls' undersides, which the union cannot take
SPOKE_R0 = plug.R_WALL1 - 1.0  # the spokes start inside the hook walls' footprint, on the lobes

PERCH_IN, PERCH_OUT = PERCH_R - PERCH_W / 2, PERCH_R + PERCH_W / 2
TUBE_R = plug.D_MESH / 2 + 1.0             # the mesh tube's outside, as the collar models it
STAND_OFF = PERCH_IN - TUBE_R              # from the tube to the perch's inner edge: a finch's room, not a dove's
SPOKE_AT = [plug.EAR_AT + plug.HOOK_SHIFT + k * 360.0 / SPOKES for k in range(SPOKES)]   # the hooks' centres


def add_perch(p):
    """The perch ring and its spokes on the cap in `p`: the ring a D in
    section revolved about the axis, its flat bottom level with the
    plate's so the whole cap still prints flat, hooks up; each spoke a
    bar from the hook's lobe, under its wall, out into the ring."""
    s = p.sketch("front", 0.0, "perch ring")
    z0 = -plug.PLATE_T                      # the plate's underside
    z1 = z0 + PERCH_H - PERCH_W / 2         # where the round top starts
    p.draw(s, {"type": "add_line", "a": v2(PERCH_IN, z0), "b": v2(PERCH_OUT, z0)})
    p.draw(s, {"type": "add_line", "a": v2(PERCH_OUT, z0), "b": v2(PERCH_OUT, z1)})
    p.draw(s, {"type": "add_arc", "center": v2(PERCH_R, z1), "start": v2(PERCH_OUT, z1), "end": v2(PERCH_IN, z1)})
    p.draw(s, {"type": "add_line", "a": v2(PERCH_IN, z1), "b": v2(PERCH_IN, z0)})
    p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "add", "name": "perch ring"})
    for k, a in enumerate(SPOKE_AT):
        s = p.sketch("top", z0, f"spoke {k + 1}")
        r0, r1 = SPOKE_R0, PERCH_R
        p.polygon(s, [plug.rot(r0, -SPOKE_W / 2, a), plug.rot(r1, -SPOKE_W / 2, a), plug.rot(r1, SPOKE_W / 2, a), plug.rot(r0, SPOKE_W / 2, a)])
        p.extrude(s, SPOKE_T, op="add", name=f"spoke {k + 1}")


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "finch_perch.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "finch_perch"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Finch feeder cap with perch"}, {"type": "rename_tab", "tab": 1, "name": "cap"}]})
    p = Part(mcp, 1, "cap")
    plain = plug.build_cap(p)["bodies"][0]["volume"]
    add_perch(p)
    rep = plug.checked(p)
    cap = rep["bodies"][0]
    lo, hi = cap["bounds"]
    ring_vol = (PERCH_W * (PERCH_H - PERCH_W / 2) + math.pi * (PERCH_W / 2) ** 2 / 2) * 2 * math.pi * PERCH_R
    print(f"cap with perch: {len(rep['bodies'])} body, {hi['x'] - lo['x']:.1f} across, {lo['z']:.1f} to {hi['z']:.1f} tall, {cap['volume'] / 1000:.1f} cm3 "
          f"({plain / 1000:.1f} the cap, {ring_vol / 1000:.1f} the ring, {(cap['volume'] - plain - ring_vol) / 1000:.1f} the spokes); "
          f"perch {PERCH_W:g} wide, {PERCH_H:g} tall, {2 * PERCH_OUT:g} across, {STAND_OFF:.1f} off the tube, {PERCH_IN - plug.R_WALL1:.1f} off the hooks")
    collar_tab = plug.build_collar(mcp)
    asms = {name: plug.add_assembly(mcp, name, collar_tab, 1, deg) for name, deg in (("cap on collar", 0.0), ("cap offered up", plug.OFFER))}
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "perch_cap_iso", view="iso")
    shot(1, "perch_cap_below", view="0.5,-0.6,-0.6")
    shot(asms["cap on collar"], "perch_on_collar_section", view="front", section="y:0")
    shot(asms["cap on collar"], "perch_on_collar", view="0.5,-0.6,0.35")
    shot(asms["cap offered up"], "perch_offered_up_below", view="0,0,-1")
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "perch_cap.stl")}))
    print(mcp.call("export", {"tab": asms["cap on collar"], "format": "pdf", "sheet": "A4", "hidden": True, "parts": False, "views": ["section@0", "top", "iso"],
                              "note": f"cap with perch on the ring, locked: perch {PERCH_W:g} x {PERCH_H:g} at {PERCH_R:g} from the axis, {STAND_OFF:.1f} off the mesh tube, "
                                      f"on {SPOKES} spokes; the cap as examples/finch-plug",
                              "path": os.path.join(OUT, "perch_on_collar.pdf")}))
    numbers = dict(plug.NUMBERS)
    numbers["perch"] = {"r": PERCH_R, "w": PERCH_W, "h": PERCH_H, "r_in": PERCH_IN, "r_out": PERCH_OUT, "tube_r": TUBE_R, "stand_off": STAND_OFF,
                        "spokes": SPOKES, "spoke_w": SPOKE_W, "spoke_t": SPOKE_T, "spoke_at": SPOKE_AT, "spoke_r0": SPOKE_R0, "ring_volume": ring_vol, "plain_volume": plain}
    with open(os.path.join(OUT, "perch.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
