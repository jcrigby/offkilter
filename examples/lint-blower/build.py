#!/usr/bin/env python3
"""A leaf blower adapter for an LG dryer's lint duct, built through the
MCP server. The starting point is Xypher2000's Samsung adapter on
Thingiverse (thing 7021003): a tapered socket the blower's nozzle jams
into, a thin-walled wedge that spreads the air to a half-moon end the
size of the Samsung filter, which goes down the filter's slot and blows
the lint out through the dryer's own blower and vent. An LG filter
(ADQ56656401) is a near-square frame that drops into a straight slot at
the bottom of the door opening, so the end is redrawn: a rectangular
stub the slot's size, open at the bottom, with a flange that rests on
the slot's lip, and the wedge lofts from it up to the socket, which is
the Samsung part's, measured off its STL. The LG numbers are the
published ones for the filter; the slot itself is to be measured, and
a comb of teeth from 10 to 30 mm thick prints first to find its width.

    cargo build --release -p ok-mcp
    python3 examples/lint-blower/build.py
    cargo test -p ok-render --test lint_blower    # what CI runs

Writes out/lint_blower.okpart, out/adapter.stl and out/slot_gauge.stl
(the parts to print), pictures, a sheet and out/adapter.json.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")
# -- the LG side, mm: the published filter, the slot to be measured ---------
FILTER_W = 303.0        # 11-15/16 in across the filter's top bar (ADQ56656401); older LG filters are 11.24 in (285.5)
FILTER_T = 28.6         # 1-1/8 in through the top bar
SLOT_L = FILTER_W       # the slot with the filter out, along the door opening: measure it
SLOT_W = FILTER_T       # across the slot: measure it, or read it off the gauge
STUB_CLEAR = 1.0        # the stub off the slot, each side, each way
STUB_D = 60.0           # how far the stub goes down the slot
# -- the duct ---------------------------------------------------------------
WALL = 2.0              # every wall, as the Samsung part's
FLANGE_T = 3.0          # the flange that stops on the slot's lip
FLANGE_OUT = 8.0        # how far it reaches past the stub, at the front and the ends; the back stays flush
LOFT_H = 90.0           # the wedge, slot to socket (the Samsung's is 65 and steeper)
# -- the blower socket, read off the Samsung STL ---------------------------
SOCKET_W = 90.0         # the box along the slot (z 110..200 in the STL)
SOCKET_D = 100.0        # and across, back face included (y -50..50)
SOCKET_BACK = 53.0      # the box's length at the back (x -126.5 to -73)
SOCKET_FRONT = 18.0     # and at the front (x -91 to -73): the mouth tilts toward the user
THROAT_D = 65.0         # the cone where it meets the duct (r 32.5 at x -74)
CONE_HALF = 19.0        # the cone's half angle (r 32.5 to 38.4 over 17 mm)
AXIS_Y = 50.0           # the bore's axis from the back face (y 0, z 155 in the STL)
BACK_OPEN = True        # the STL's socket is hollow behind the axis: a box, not a cone, on that side
# -- the gauge: teeth to drop in the slot -----------------------------------
TEETH = [10.0 + 2.0 * k for k in range(11)]   # 10 to 30 mm thick
TOOTH_W, TOOTH_GAP, TOOTH_D = 8.0, 4.0, 20.0
SPINE_T = 5.0

STUB_L = SLOT_L - 2 * STUB_CLEAR
STUB_T = SLOT_W - 2 * STUB_CLEAR
Z_LOFT = FLANGE_T                 # the wedge starts on the flange
Z_SOCKET = Z_LOFT + LOFT_H        # and the socket box on the wedge
Z_TOP = Z_SOCKET + SOCKET_BACK
MOUTH_TILT = math.degrees(math.atan((SOCKET_BACK - SOCKET_FRONT) / SOCKET_D))
# The wedge's walls lean, so the in-plane inset that leaves WALL through
# them is WALL over the cosine of the lean.
LEAN_FRONT = math.atan((SOCKET_D - STUB_T) / LOFT_H)
LEAN_END = math.atan((STUB_L - SOCKET_W) / 2 / LOFT_H)
IN_FRONT = WALL / math.cos(LEAN_FRONT)
IN_END = WALL / math.cos(LEAN_END)
R_THROAT = THROAT_D / 2
R_CONE_MAX = SOCKET_W / 2 - WALL  # the cone stops at the box's end walls
H_CONE = (R_CONE_MAX - R_THROAT) / math.tan(math.radians(CONE_HALF))


def checked(part):
    bodies = part.bodies()
    print(f"{part.name}: {len(bodies)} bodies", ", ".join(f"{n or 'body'} {v / 1000:.1f} cm3" for n, v in bodies))
    return bodies


def build_adapter(p):
    """The stub, the flange, the wedge and the socket as one solid, then
    the duct through them and the socket's bore."""
    s = p.sketch("top", -STUB_D, "stub")
    p.rect(s, (-STUB_L / 2, 0.0), (STUB_L / 2, STUB_T))
    p.extrude(s, STUB_D, name="stub")
    s = p.sketch("top", 0.0, "flange")
    p.rect(s, (-STUB_L / 2 - FLANGE_OUT, 0.0), (STUB_L / 2 + FLANGE_OUT, STUB_T + FLANGE_OUT))
    p.extrude(s, FLANGE_T, op="add", name="flange")
    a = p.sketch("top", Z_LOFT, "wedge at the slot")
    p.rect(a, (-STUB_L / 2, 0.0), (STUB_L / 2, STUB_T))
    b = p.sketch("top", Z_SOCKET, "wedge at the socket")
    p.rect(b, (-SOCKET_W / 2, 0.0), (SOCKET_W / 2, SOCKET_D))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "add", "name": "wedge"})
    s = p.sketch("top", Z_SOCKET, "socket")
    p.rect(s, (-SOCKET_W / 2, 0.0), (SOCKET_W / 2, SOCKET_D))
    p.extrude(s, SOCKET_BACK, op="add", name="socket")
    # The mouth: a plane from the back's top edge down to the front's,
    # cut away as a prism along the slot.
    s = p.sketch("right", -SOCKET_W / 2 - 1.0, "mouth")
    zb, zf = Z_TOP, Z_SOCKET + SOCKET_FRONT
    slope = (zf - zb) / SOCKET_D
    p.polygon(s, [(-1.0, zb - slope), (SOCKET_D + 1.0, zf + slope), (SOCKET_D + 1.0, zb + 10.0), (-1.0, zb + 10.0)])
    p.cut(s, SOCKET_W + 2.0, name="mouth")
    # The duct: the stub's bore up through the flange, the wedge's bore
    # lofted to the socket, and the socket's hollow behind the axis.
    s = p.sketch("top", -STUB_D - 1.0, "stub bore")
    p.rect(s, (-STUB_L / 2 + WALL, WALL), (STUB_L / 2 - WALL, STUB_T - WALL))
    p.cut(s, STUB_D + 1.0 + Z_LOFT + 0.01, name="stub bore")
    a = p.sketch("top", Z_LOFT, "wedge bore at the slot")
    p.rect(a, (-STUB_L / 2 + IN_END, WALL), (STUB_L / 2 - IN_END, STUB_T - IN_FRONT))
    b = p.sketch("top", Z_SOCKET + 0.01, "wedge bore at the socket")
    p.rect(b, (-SOCKET_W / 2 + IN_END, WALL), (SOCKET_W / 2 - IN_END, SOCKET_D - IN_FRONT))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "remove", "name": "wedge bore"})
    if BACK_OPEN:
        s = p.sketch("top", Z_SOCKET - 0.01, "socket hollow")
        p.rect(s, (-SOCKET_W / 2 + WALL, WALL), (SOCKET_W / 2 - WALL, AXIS_Y))
        p.cut(s, SOCKET_BACK + 1.0, name="socket hollow")
    # The cone: its half profile on the plane through the axis, turned
    # about it; past the end walls it runs on as a cylinder.
    s = p.sketch("front", -AXIS_Y, "cone")
    p.polygon(s, [(0.0, Z_SOCKET - 0.5), (R_THROAT, Z_SOCKET - 0.5), (R_THROAT, Z_SOCKET), (R_CONE_MAX, Z_SOCKET + H_CONE),
                  (R_CONE_MAX, Z_TOP + 1.0), (0.0, Z_TOP + 1.0)])
    p.revolve_cut(s, name="cone")


def build_gauge(g):
    """A comb: a spine with a tooth per thickness hanging under it, the
    thinnest first, the thickest that drops into the slot being its width."""
    length = len(TEETH) * TOOTH_W + (len(TEETH) - 1) * TOOTH_GAP
    spine_w = max(TEETH) + 2.0
    s = g.sketch("top", 0.0, "spine")
    g.rect(s, (0.0, 0.0), (length, spine_w))
    g.extrude(s, SPINE_T, name="spine")
    for k, t in enumerate(TEETH):
        x = k * (TOOTH_W + TOOTH_GAP)
        s = g.sketch("top", -TOOTH_D, f"tooth {t:g}")
        g.rect(s, (x, (spine_w - t) / 2), (x + TOOTH_W, (spine_w + t) / 2))
        g.extrude(s, TOOTH_D, op="add", name=f"tooth {t:g}")
    # A notch over the thinnest tooth, to read the comb from that end.
    s = g.sketch("top", SPINE_T, "notch")
    g.rect(s, (TOOTH_W / 2 - 1.0, -1.0), (TOOTH_W / 2 + 1.0, spine_w + 1.0))
    g.cut(s, 1.5, direction="reverse", name="notch")


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "lint_blower.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "lint_blower"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "LG lint duct blower adapter"}, {"type": "rename_tab", "tab": 1, "name": "adapter"}]})
    adapter = Part(mcp, 1, "adapter")
    build_adapter(adapter)
    bodies = checked(adapter)
    assert len(bodies) == 1, bodies
    gauge_tab = new_tab(mcp, {"type": "add_part_studio", "name": "slot gauge"})
    gauge = Part(mcp, gauge_tab, "slot gauge")
    build_gauge(gauge)
    assert len(checked(gauge)) == 1

    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "adapter_iso")
    shot(1, "adapter_front", view="front")
    shot(1, "adapter_section", view="front", section=f"y:{AXIS_Y}")
    shot(1, "adapter_section_iso", view="-1,-1,0.8", section=f"y:{AXIS_Y}")
    shot(1, "adapter_mouth", view="0.6,0.5,1")
    shot(gauge_tab, "slot_gauge")
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "adapter.stl")}))
    print(mcp.call("export", {"tab": gauge_tab, "format": "stl", "path": os.path.join(OUT, "slot_gauge.stl")}))
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "hidden": True, "views": ["front", f"section@{AXIS_Y}", "top", "iso"],
                              "note": f"stub {STUB_L:g} x {STUB_T:g} for a slot {SLOT_L:g} x {SLOT_W:g}, {STUB_D:g} deep; socket {THROAT_D:g} at the throat, {CONE_HALF:g} deg",
                              "path": os.path.join(OUT, "adapter.pdf")}))
    numbers = {
        "slot": {"l": SLOT_L, "w": SLOT_W, "filter_w": FILTER_W, "filter_t": FILTER_T},
        "adapter": {"stub_l": STUB_L, "stub_t": STUB_T, "stub_d": STUB_D, "clear": STUB_CLEAR, "wall": WALL,
                    "flange_t": FLANGE_T, "flange_out": FLANGE_OUT, "loft_h": LOFT_H, "z_socket": Z_SOCKET, "z_top": Z_TOP,
                    "socket_w": SOCKET_W, "socket_d": SOCKET_D, "socket_back": SOCKET_BACK, "socket_front": SOCKET_FRONT,
                    "mouth_tilt": MOUTH_TILT, "throat_d": THROAT_D, "cone_half": CONE_HALF, "r_cone_max": R_CONE_MAX,
                    "h_cone": H_CONE, "axis_y": AXIS_Y, "back_open": BACK_OPEN, "in_front": IN_FRONT, "in_end": IN_END,
                    "volume": bodies[0][1]},
        "gauge": {"teeth": TEETH, "tooth_w": TOOTH_W, "tooth_gap": TOOTH_GAP, "tooth_d": TOOTH_D, "spine_t": SPINE_T},
    }
    with open(os.path.join(OUT, "adapter.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"stub {STUB_L:g} x {STUB_T:g}, {STUB_D:g} deep; wedge {LOFT_H:g} tall leaning {math.degrees(LEAN_FRONT):.0f} deg at the front and {math.degrees(LEAN_END):.0f} at the ends; mouth tilted {MOUTH_TILT:.0f} deg; cone {THROAT_D:g} to {2 * R_CONE_MAX:g} over {H_CONE:.0f}")


if __name__ == "__main__":
    main()
