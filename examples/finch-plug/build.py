#!/usr/bin/env python3
"""A snap-in plug for the bottom of a mesh finch feeder, built through
the MCP server. The feeder's mesh tube ends in a thin plastic ring with
three ears outside; its bore is a hair narrower than the mesh above it,
so the ring's inner edge is a lip inside the tube. The tray base twists
onto the ears, collects rain and spilt seed, and perches birds that
cannot cling. The plug replaces it: a cup through the ring's bore with
a flange under the ring and three snap tabs cut from its skirt that
pass the ring and catch over its top, so the ring is held between them;
its floor level with the ring's top and domed so water runs to the
mesh; nothing outside the tube for a bird to stand on but the flange
hidden under the ring. The ring is modelled from the numbers below,
read off photographs with a rule and to be confirmed with calipers (the
README says which), so the plug can be checked against it.

    cargo build --release -p ok-mcp
    python3 examples/finch-plug/build.py
    cargo test -p ok-render --test finch_plug    # what CI runs

Writes out/finch_plug.okpart, out/plug.stl (the part to print), a
section of the plug in the collar, a sheet, and out/plug.json.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

OUT = os.path.join(HERE, "out")
# -- the ring, mm, as read off the photographs: confirm with calipers -----
D_MOUTH = 48.5          # the ring's bore at the mouth
D_TOP = 47.5            # the bore at the ring's top edge, inside the tube: a slight draft
RING_T = 3.0            # the ring's thickness, mouth to its top face
D_MESH = 50.0           # inside the mesh tube just above the ring
RING_OD = 64.0
EAR_W, EAR_R = 16.0, 3.0   # the three bayonet ears: width and radial reach, the ring's thickness
TUBE_H = 25.0           # how much of the mesh tube the model draws
# -- the plug ---------------------------------------------------------------
CLEAR = 0.3             # the skirt off the bore
WALL = 2.0              # the skirt
FLOOR_T = 2.4           # the floor
DOME = 1.5              # the floor's top rises this much to the centre, so water runs to the mesh
EXT = 10.0              # the skirt hangs this far below the mouth: the tabs' length lives here
ROOT = 2.5              # the skirt's bottom band, which the tabs grow from
FLANGE_W, FLANGE_T = 2.0, 1.5   # a flange under the ring, hidden by it: the stop against pushing the plug up
TAB_W, TAB_T, SLIT = 10.0, 1.2, 0.8   # three tabs cut from the skirt
TAB_CLEAR = 0.2         # a tab's face inside the ring's top edge
ENGAGE = 0.6            # the catch reaches this far over the ring's top
PLAY = 0.15             # between the catch and the ring's top
LEAD = 0.5              # the catch's vertical band under its lead-in
DRAIN_D, DRAINS = 1.0, 6   # drain holes in the floor, smaller than a nyjer seed
TABS = 3
TAB_ANGLES = [k * 360.0 / TABS for k in range(TABS)]


def r_bore(z):
    """The ring's bore radius at height z above the mouth, drafted to its top."""
    return (D_MOUTH + (D_TOP - D_MOUTH) * max(0.0, min(1.0, z / RING_T))) / 2.0


def rot(x, y, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return (x * c - y * s, x * s + y * c)


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "finch_plug.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "finch_plug"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Finch feeder plug"}, {"type": "rename_tab", "tab": 1, "name": "plug"}]})
    # -- the plug ----------------------------------------------------------
    p = Part(mcp, 1, "plug")
    r_bot, r_top = r_bore(0.0) - CLEAR, r_bore(RING_T) - CLEAR
    r_floor_in = r_bore(RING_T - FLOOR_T) - CLEAR - WALL
    z_floor = RING_T - FLOOR_T
    # The cup: a profile in (r, z) revolved about the axis. The skirt is
    # straight below the mouth, carries the flange just under the ring,
    # and follows the ring's draft through it; the floor's top is level
    # with the ring's top at the edge and a shallow cone inside.
    s = p.sketch("front", 0.0, "cup")
    cup = [(r_bot, -EXT), (r_bot, -FLANGE_T), (r_bot + CLEAR + FLANGE_W, -FLANGE_T), (r_bot + CLEAR + FLANGE_W, 0.0), (r_bot, 0.0), (r_top, RING_T),
           (D_TOP / 2 - 0.5, RING_T), (0.0, RING_T + DOME), (0.0, z_floor), (r_floor_in, z_floor), (r_bot - WALL, -EXT)]
    p.polygon(s, cup)
    body = p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "cup"})
    p.apply({"type": "rename_part", "source": body, "name": "plug"}, {"type": "set_part_material", "source": body, "material": {"name": "PETG", "density": 1.27}})
    # The tabs: a window through the skirt, the flange and the floor's
    # edge for each, from the root band up and out the top; then the tab
    # itself, curved to the ring's bore: a root block fused into the
    # skirt's bottom band, a plate a hair inside the ring's top edge, the
    # catch band reaching over the ring's top, and a lead-in lofted from
    # the catch's face to the plate's.
    r_tab = D_TOP / 2 - TAB_CLEAR
    r_catch = r_tab + ENGAGE + TAB_CLEAR
    z_catch = RING_T + PLAY
    z_top = z_catch + LEAD + (r_catch - r_tab)
    half = math.degrees(TAB_W / 2 / r_tab)

    def sector(part, sk, r0, r1, a, half_deg):
        """An annular sector of radii r0..r1 about the z axis, centred on
        angle a, half_deg each side: two arcs and two radial lines."""
        a0, a1 = a - half_deg, a + half_deg
        part.draw(sk, {"type": "add_arc", "center": v2(0.0, 0.0), "start": v2(*rot(r0, 0.0, a0)), "end": v2(*rot(r0, 0.0, a1))})
        part.draw(sk, {"type": "add_arc", "center": v2(0.0, 0.0), "start": v2(*rot(r1, 0.0, a0)), "end": v2(*rot(r1, 0.0, a1))})
        for ang in (a0, a1):
            part.draw(sk, {"type": "add_line", "a": v2(*rot(r0, 0.0, ang)), "b": v2(*rot(r1, 0.0, ang))})

    for k, a in enumerate(TAB_ANGLES):
        s = p.sketch("top", -EXT + ROOT, f"window {k + 1}")
        sector(p, s, r_tab - TAB_T - SLIT, r_bot + FLANGE_W + 2.0, a, math.degrees((TAB_W / 2 + SLIT) / r_tab))
        p.extrude(s, RING_T + DOME + 1.0 - (-EXT + ROOT), op="remove", name=f"window {k + 1}")
    for k, a in enumerate(TAB_ANGLES):
        s = p.sketch("top", -EXT, f"tab {k + 1} root")
        sector(p, s, r_tab - TAB_T, r_bot - 0.3, a, half)
        p.extrude(s, ROOT + 0.8, op="add", name=f"tab {k + 1} root")
        s = p.sketch("top", -EXT + ROOT, f"tab {k + 1}")
        sector(p, s, r_tab - TAB_T, r_tab, a, half)
        p.extrude(s, z_top - (-EXT + ROOT), op="add", name=f"tab {k + 1}")
        s = p.sketch("top", z_catch, f"catch {k + 1}")
        sector(p, s, r_tab, r_catch, a, half)
        p.extrude(s, LEAD, op="add", name=f"catch {k + 1}")
        s = p.sketch("top", z_catch + LEAD, f"lead-in {k + 1} foot")
        sector(p, s, r_tab, r_catch, a, half)
        s2 = p.sketch("top", z_top, f"lead-in {k + 1} top")
        sector(p, s2, r_tab - 0.4, r_tab, a, half)
        p.feature({"type": "add_loft", "sketch": s, "sketch_b": s2, "op": "add", "name": f"lead-in {k + 1}"})
    # Drain holes near the floor's edge, between the tabs.
    s = p.sketch("top", RING_T + DOME + 1.0, "drains")
    for i in range(DRAINS):
        a = (i + 0.5) * 360.0 / DRAINS
        p.point(s, rot(r_floor_in - 2.5, 0.0, a))
    p.hole(s, DRAIN_D, name="drain holes")
    rep = p.report()
    bad = [(f["name"], f["error"]) for f in rep["features"] if f.get("error")]
    if bad:
        raise RuntimeError(bad)
    plug_body = rep["bodies"][0]
    lo, hi = plug_body["bounds"]
    print(f"plug: {len(rep['bodies'])} body, {hi['x'] - lo['x']:.2f} across, {lo['z']:.1f} to {hi['z']:.1f} tall, {plug_body['volume'] / 1000:.1f} cm3, {plug_body.get('mass', 0):.0f} g")
    # -- the ring it fits, from the photographs' numbers --------------------
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "collar"}]})
    collar_tab = int(re.search(r"tab (\d+)", text).group(1))
    c = Part(mcp, collar_tab, "collar")
    s = c.sketch("front", 0.0, "ring")
    collar = [(D_MOUTH / 2, 0.0), (RING_OD / 2, 0.0), (RING_OD / 2, RING_T), (D_MESH / 2 + 1.0, RING_T), (D_MESH / 2 + 1.0, TUBE_H), (D_MESH / 2, TUBE_H),
              (D_MESH / 2, RING_T), (D_TOP / 2, RING_T)]
    c.polygon(s, collar)
    f = c.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "ring"})
    c.apply({"type": "rename_part", "source": f, "name": "collar"}, {"type": "set_part_material", "source": f, "material": {"name": "ABS", "density": 1.05}})
    for k, a in enumerate(TAB_ANGLES):
        s = c.sketch("top", 0.0, f"ear {k + 1}")
        r0, r1, hw = RING_OD / 2 - 1.0, RING_OD / 2 + EAR_R, EAR_W / 2
        c.polygon(s, [rot(r0, -hw, a + 60.0), rot(r1, -hw, a + 60.0), rot(r1, hw, a + 60.0), rot(r0, hw, a + 60.0)])
        c.extrude(s, RING_T, op="add", name=f"ear {k + 1}")
    crep = c.report()
    bad = [(f["name"], f["error"]) for f in crep["features"] if f.get("error")]
    if bad:
        raise RuntimeError(bad)
    # -- the plug in the collar ---------------------------------------------
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "plug in collar"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    place = {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}
    mcp.call("apply", {"tab": asm, "ops": [{"type": "add_instance", "studio": collar_tab, "body": 0, "name": "collar", "fixed": True, "placement": place},
                                          {"type": "add_instance", "studio": 1, "body": 0, "name": "plug", "fixed": True, "placement": place}]})
    # -- pictures and files -----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "plug_iso", view="iso")
    shot(1, "plug_below", view="0.5,-0.6,-0.6")
    shot(asm, "plug_in_collar_section", view="front", section="y:0")
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "plug.stl")}))
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A4", "hidden": True, "parts": False, "views": ["section@0", "top", "iso"],
                              "note": f"plug in the ring: bore {D_MOUTH:g} at the mouth, {D_TOP:g} at the top, ring {RING_T:g} thick; tabs catch {ENGAGE:g} over it",
                              "path": os.path.join(OUT, "plug_in_collar.pdf")}))
    # The tab as a cantilever: its free length from the root band to the
    # catch, and the strain in bending it past the ring's top edge.
    length = z_catch - (-EXT + ROOT)
    deflect = ENGAGE + TAB_CLEAR
    strain = 1.5 * deflect * TAB_T / length ** 2
    print(f"tabs: {length:.1f} long, {TAB_T:g} thick, deflect {deflect:.2f} to pass the ring: {100 * strain:.1f} % strain")
    numbers = {
        "collar": {"d_mouth": D_MOUTH, "d_top": D_TOP, "ring_t": RING_T, "d_mesh": D_MESH, "od": RING_OD, "ear_w": EAR_W, "ear_r": EAR_R, "tube_h": TUBE_H},
        "plug": {"clear": CLEAR, "wall": WALL, "floor_t": FLOOR_T, "dome": DOME, "ext": EXT, "root": ROOT, "flange_w": FLANGE_W, "flange_t": FLANGE_T,
                 "tab_w": TAB_W, "tab_t": TAB_T, "slit": SLIT,
                 "tab_clear": TAB_CLEAR, "engage": ENGAGE, "play": PLAY, "lead": LEAD, "drain_d": DRAIN_D, "drains": DRAINS, "tabs": TABS,
                 "r_tab": r_tab, "z_catch": z_catch, "z_top": z_top, "length": length, "deflect": deflect, "strain": strain},
    }
    with open(os.path.join(OUT, "plug.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
