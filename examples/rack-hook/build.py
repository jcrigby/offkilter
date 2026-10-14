"""A coat hook that hangs on the front rail of a wire shelving rack:
the over-the-door hook's shape with its 1-1/4 in door slot shrunk to a
1/4 in slot that drops over the rail wire. A printed band, 18 wide
and 4 thick, in one piece: a U over the top rail, a short leg behind
it, a long leg down the front that leans on the lower rail when a coat
pulls on it, and two J hooks on that leg, one halfway and one at the
end.

    cargo build --release -p ok-mcp
    python3 examples/rack-hook/build.py       # the document, the STL, pictures and sheets
    cargo test -p ok-render --test rack_hook  # what CI runs

Millimetres throughout; the rail is the one inch number. Frame: the
top rail's axis is the x axis (x along the rail, the band from x = 0
to its width), y out from the shelf toward the room, z up.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4

RAIL = 0.25 * IN            # the rack's rail wire: 1/4 in; measure yours
LOWER_RAIL = 25.0           # the second front rail, this far below the top one in the same plane
SLOT = 7.5                  # the U's inside: the rail and 1.15 of clearance
T = 4.0                     # the band's thickness
W = 18.0                    # the band's width, along the rail: between the deck wires, which are an inch apart
BACK = 25.0                 # the leg behind the rail, from the rail's axis down
LEG = 110.0                 # the leg down the front, from the rail's axis down: past the lower rail to the bottom hook
HOOK_R = 10.0               # the J hooks' inside radius: a hanger's hook or a coat loop
TIP = 20.0                  # the hooks' tips, up from their centres
HOOKS = (LEG / 2, LEG)      # the hooks' centres, down the front leg
MATERIALS = {"petg": ("PETG", 1.27), "steel": ("plated steel", 7.85)}

RI, RO = SLOT / 2, SLOT / 2 + T          # the U's radii
Y_FRONT = (RI, RO)                        # the front leg's faces
Y_HOOK = RO + HOOK_R                      # the hooks' centres: their inner band is the front leg
Y_TIP = (Y_HOOK + HOOK_R, Y_HOOK + HOOK_R + T)


def part_is(p, f, name, material=None):
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        m, density = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": m, "density": density}})
    p.apply(*ops)


def band(p):
    """The band as a side profile in the y-z plane, extruded W along
    x: discs and rectangles unioned, the inside of each bend and the
    open half of each ring cut away."""
    def add(name, draw, op="add"):
        s = p.sketch("right", 0.0, name)
        draw(s)
        return p.extrude(s, W, op=op, name=name)

    def cut(name, draw):
        s = p.sketch("right", -1.0, name)
        draw(s)
        p.cut(s, W + 2.0, name=name)

    # The U over the rail: a ring about the rail's axis, its lower half
    # between the legs removed.
    f = add("U over the rail", lambda s: p.circle(s, (0.0, 0.0), RO), op="new")
    cut("the rail's room", lambda s: p.circle(s, (0.0, 0.0), RI))
    cut("the U's opening", lambda s: p.rect(s, (-RI, -RO - 1.0), (RI, 0.0)))
    add("leg behind the rail", lambda s: p.rect(s, (-RO, -BACK), (-RI, 0.0)))
    add("its rounded end", lambda s: p.circle(s, (-(RI + RO) / 2, -BACK), T / 2))
    add("leg down the front", lambda s: p.rect(s, (Y_FRONT[0], -LEG), (Y_FRONT[1], 0.0)))
    # The hooks: a ring whose inner band is the front leg, its upper
    # half cut away, then the tip up from its far side, rounded.
    for n, zc in enumerate(HOOKS, 1):
        add(f"hook {n}", lambda s: p.circle(s, (Y_HOOK, -zc), HOOK_R + T))
        cut(f"hook {n}, inside", lambda s: p.circle(s, (Y_HOOK, -zc), HOOK_R))
        cut(f"hook {n}, throat", lambda s: p.rect(s, (Y_FRONT[1], -zc), (Y_TIP[1] + 1.0, -zc + HOOK_R + T)))
        add(f"hook {n}, tip", lambda s: p.rect(s, (Y_TIP[0], -zc), (Y_TIP[1], -zc + TIP)))
        add(f"hook {n}, rounded end", lambda s: p.circle(s, ((Y_TIP[0] + Y_TIP[1]) / 2, -zc + TIP), T / 2))
    part_is(p, f, f"rack hook, {RAIL / IN:g} in rail", "petg")


def rails(p):
    """The front of a wire shelf: the top rail and the lower one 25
    below it in the same plane, along x."""
    for n, z in enumerate((0.0, -LOWER_RAIL)):
        s = p.sketch("right", -40.0, f"rail {n + 1}")
        p.circle(s, (0.0, z), RAIL / 2)
        f = p.extrude(s, 100.0, op="new", name=f"rail {n + 1}")
        part_is(p, f, "top rail, 1/4 wire" if n == 0 else "lower rail, 1/4 wire", "steel")


def instance(studio, body, name, z=0.0, ry=0.0):
    return {"type": "add_instance", "studio": studio, "body": body, "name": name, "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": z}, "rotation": {"x": 0.0, "y": ry, "z": 0.0}}}


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "rack_hook.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "rack_hook"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Coat hook for a wire rack's rail"},
                               {"type": "rename_tab", "tab": 1, "name": "hook"},
                               {"type": "add_part_studio", "name": "rails"},
                               {"type": "add_assembly", "name": "on the rail"},
                               {"type": "add_assembly", "name": "print"}]})
    for tab, build in ((1, band), (2, rails)):
        p = Part(mcp, tab, str(tab))
        build(p)
        r = p.report()
        for b in r["bodies"]:
            lo, hi = b["bounds"]
            print(f"tab {tab}: {b['name']}: x {lo['x']:.1f}..{hi['x']:.1f}, y {lo['y']:.2f}..{hi['y']:.2f}, z {lo['z']:.2f}..{hi['z']:.2f} mm, {b['volume'] / 1000:.2f} cm3")
        bad = [f["name"] for f in r["features"] if f.get("error")]
        assert not bad, bad
    mcp.call("apply", {"ops": [instance(2, 0, "top rail"), instance(2, 1, "lower rail"), instance(1, 0, "hook")], "tab": 3})
    # The print: turned so the band's side profile lies on the bed and
    # the band stands its width tall, the strongest way for the hooks.
    mcp.call("apply", {"ops": [instance(1, 0, "hook, on its side", z=W, ry=90.0)], "tab": 4})
    print(mcp.call("export", {"tab": 4, "format": "stl", "path": os.path.join(OUT, "hook.stl")}))
    mcp.call("screenshot", {"tab": 1, "view": "0.8,0.5,0.3", "width": 1200, "height": 1200, "path": os.path.join(OUT, "hook.png")})
    mcp.call("screenshot", {"tab": 1, "view": "right", "width": 900, "height": 1400, "path": os.path.join(OUT, "hook_profile.png")})
    mcp.call("screenshot", {"tab": 3, "view": "0.7,0.6,0.35", "width": 1400, "height": 1200, "path": os.path.join(OUT, "on_the_rail.png")})
    mcp.call("screenshot", {"tab": 4, "view": "0.4,-0.6,0.7", "width": 1400, "height": 1000, "path": os.path.join(OUT, "hook_print.png")})
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "views": ["right", "front", "iso"],
                              "note": f"printed band {W:g} wide x {T:g} thick; slot {SLOT:g} for a {RAIL / IN:g} in rail, legs {BACK:g} behind and {LEG:g} down the front, hooks R{HOOK_R:g} inside at {HOOKS[0]:g} and {HOOKS[1]:g}",
                              "path": os.path.join(OUT, "hook.pdf")}))
    print(mcp.call("export", {"tab": 3, "format": "pdf", "sheet": "A4", "views": ["iso", f"section-side@{W / 2:g}"],
                              "note": "on the top rail of a wire shelf; the front leg leans on the lower rail under load",
                              "path": os.path.join(OUT, "on_the_rail.pdf")}))


if __name__ == "__main__":
    main()
