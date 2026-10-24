#!/usr/bin/env python3
"""A bayonet cap for the bottom of a mesh finch feeder, built through the
MCP server. The feeder's mesh tube ends in a thin plastic ring with
three ears outside; its tray base twists onto the ears, collects rain
and spilt seed, and perches birds that cannot cling. The cap keeps the
tray's lock and drops the tray: a plate under the ring with a short
plug up into the ring's bore, its top level with the ring's and domed
so water runs to the mesh, and three hooks outside the ring that the
ears twist under, each hook's lip stepped down along its arc so the ear
jams tight as the tray's did. The ring is modelled from the numbers
below, read off photographs with a rule and to be confirmed with
calipers (the README says which), so the cap can be checked against it;
a stepped gauge finds the bore by feel.

    cargo build --release -p ok-mcp
    python3 examples/finch-plug/build.py
    cargo test -p ok-render --test finch_plug    # what CI runs

Writes out/finch_plug.okpart, out/cap.stl and out/gauge.stl (the parts
to print), a section of the cap on the ring, a sheet, and out/plug.json.
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
RING_T = 3.0            # the ring's thickness, mouth to its top face; the ears are the same
D_MESH = 50.0           # inside the mesh tube just above the ring
RING_OD = 64.0          # the ring's flange
EAR_W, EAR_R = 16.0, 4.0   # the three ears: width across, and how far past the flange they reach
EAR_AT = 60.0           # the ears' angles are EAR_AT + k * 120: where they sit locked under the hooks
TUBE_H = 25.0           # how much of the mesh tube the model draws
# -- the cap ----------------------------------------------------------------
CLEAR = 0.3             # the plug off the bore
PLATE_T = 2.5           # the plate under the ring
DOME = 1.5              # the plug's top rises this much to the centre, so water runs to the mesh
PLATE_R = RING_OD / 2   # the plate reaches the ring's edge, no further, but for the hook lobes
HOOK_GAP = 0.4          # between an ear's tip and the hook's wall
HOOK_WALL = 2.0         # the hook's wall, outside the ears
HOOK_ARC = 36.0         # degrees of arc a hook spans
LIP_T = 2.0             # the lip over the ear
LIP_OVER = 3.0          # how far the lip reaches in over the ear's tip
RAMP = (0.6, 0.4, 0.2, 0.0)     # the lip's clearance over the ear, step by step along the arc, down to touching: the ear wedges as it turns
STOP_T = 1.5            # the end stop's thickness, in mm of arc
DRAIN_D, DRAINS = 1.0, 6   # drain holes in the plug, smaller than a nyjer seed
# -- the gauge: a stepped tube to find the ring's bore by feel ---------------
GAUGE_MIN, GAUGE_MAX, GAUGE_STEP, GAUGE_H, GAUGE_WALL = 46.5, 49.5, 0.25, 3.0, 1.5

R_EAR = RING_OD / 2 + EAR_R          # the ears' tips
R_WALL0 = R_EAR + HOOK_GAP           # the hook wall's inside
R_WALL1 = R_WALL0 + HOOK_WALL        # and outside: the plate's lobes reach here
R_LIP = R_EAR - LIP_OVER             # the lip's inner edge
HOOK_H = RING_T + RAMP[0] + LIP_T    # the hook's height above the plate
HOOKS = 3


def r_bore(z):
    """The ring's bore radius at height z above its mouth, drafted to its top."""
    return (D_MOUTH + (D_TOP - D_MOUTH) * max(0.0, min(1.0, z / RING_T))) / 2.0


def rot(x, y, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return (x * c - y * s, x * s + y * c)


def sector(part, sk, r0, r1, a, half_deg):
    """An annular sector of radii r0..r1 about the z axis, centred on
    angle a, half_deg each side: two arcs and two radial lines."""
    a0, a1 = a - half_deg, a + half_deg
    part.draw(sk, {"type": "add_arc", "center": v2(0.0, 0.0), "start": v2(*rot(r0, 0.0, a0)), "end": v2(*rot(r0, 0.0, a1))})
    part.draw(sk, {"type": "add_arc", "center": v2(0.0, 0.0), "start": v2(*rot(r1, 0.0, a0)), "end": v2(*rot(r1, 0.0, a1))})
    for ang in (a0, a1):
        part.draw(sk, {"type": "add_line", "a": v2(*rot(r0, 0.0, ang)), "b": v2(*rot(r1, 0.0, ang))})


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "finch_plug.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "finch_plug"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Finch feeder cap"}, {"type": "rename_tab", "tab": 1, "name": "cap"}]})
    # -- the cap: plate and plug, one revolve ------------------------------
    # The frame: z up the tube, the ring's mouth at z = 0, so the plate is
    # below zero and the plug above it.
    p = Part(mcp, 1, "cap")
    s = p.sketch("front", 0.0, "plate and plug")
    r0, r1 = r_bore(0.0) - CLEAR, r_bore(RING_T) - CLEAR
    cup = [(0.0, -PLATE_T), (PLATE_R, -PLATE_T), (PLATE_R, 0.0), (r0, 0.0), (r1, RING_T), (D_TOP / 2 - 0.5, RING_T), (0.0, RING_T + DOME)]
    p.polygon(s, cup)
    body = p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "plate and plug"})
    p.apply({"type": "rename_part", "source": body, "name": "cap"}, {"type": "set_part_material", "source": body, "material": {"name": "PETG", "density": 1.27}})
    # The hooks, centred where the ears sit once turned home: a lobe of
    # the plate out to the wall, the wall, the lip in steps whose
    # underside comes down along the arc, and a stop at the far end.
    half = HOOK_ARC / 2
    for k in range(HOOKS):
        a = EAR_AT + k * 360.0 / HOOKS
        s = p.sketch("top", -PLATE_T, f"lobe {k + 1}")
        sector(p, s, PLATE_R - 1.0, R_WALL1, a, half + 2.0)
        p.extrude(s, PLATE_T, op="add", name=f"lobe {k + 1}")
        s = p.sketch("top", 0.0, f"hook wall {k + 1}")
        sector(p, s, R_WALL0, R_WALL1, a, half)
        p.extrude(s, HOOK_H, op="add", name=f"hook wall {k + 1}")
        # The lip: the ear enters at the arc's start (a - half) and turns
        # toward the stop at a + half; each step's underside is lower.
        n = len(RAMP)
        for i, c in enumerate(RAMP):
            seg_half = half / n
            centre = a - half + (i + 0.5) * 2 * seg_half
            s = p.sketch("top", RING_T + c, f"lip {k + 1}.{i + 1}")
            sector(p, s, R_LIP, R_WALL0 + 0.01, centre, seg_half + 0.01)
            p.extrude(s, HOOK_H - RING_T - c, op="add", name=f"lip {k + 1}.{i + 1}")
        s = p.sketch("top", 0.0, f"stop {k + 1}")
        stop_half = math.degrees(STOP_T / 2 / R_LIP)
        sector(p, s, R_LIP, R_WALL0 + 0.01, a + half + stop_half, stop_half)
        p.extrude(s, HOOK_H, op="add", name=f"stop {k + 1}")
    # Drain holes near the plug's edge, between the hooks.
    s = p.sketch("top", RING_T + DOME + 1.0, "drains")
    for i in range(DRAINS):
        ang = (i + 0.5) * 360.0 / DRAINS
        p.point(s, rot(D_TOP / 2 - 3.5, 0.0, ang))
    p.hole(s, DRAIN_D, name="drain holes")
    rep = p.report()
    bad = [(f["name"], f["error"]) for f in rep["features"] if f.get("error")]
    if bad:
        raise RuntimeError(bad)
    cap = rep["bodies"][0]
    lo, hi = cap["bounds"]
    print(f"cap: {len(rep['bodies'])} body, {hi['x'] - lo['x']:.1f} across, {lo['z']:.1f} to {hi['z']:.1f} tall, {cap['volume'] / 1000:.1f} cm3")
    # -- the gauge ------------------------------------------------------------
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "gauge"}]})
    gauge_tab = int(re.search(r"tab (\d+)", text).group(1))
    g = Part(mcp, gauge_tab, "gauge")
    steps = [round(GAUGE_MIN + i * GAUGE_STEP, 3) for i in range(int(round((GAUGE_MAX - GAUGE_MIN) / GAUGE_STEP)) + 1)]
    s = g.sketch("front", 0.0, "steps")
    pts = [(GAUGE_MIN / 2 - GAUGE_WALL, 0.0), (GAUGE_MAX / 2, 0.0)]
    z = 0.0
    for d in reversed(steps):
        pts += [(d / 2, z), (d / 2, z + GAUGE_H)]
        z += GAUGE_H
    pts += [(GAUGE_MIN / 2 - GAUGE_WALL, z)]
    g.polygon(s, pts)
    f = g.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "gauge"})
    g.apply({"type": "rename_part", "source": f, "name": "gauge"}, {"type": "set_part_material", "source": f, "material": {"name": "PETG", "density": 1.27}})
    for i, d in enumerate(reversed(steps)):
        if abs(d - round(d)) < 1e-9:
            s = g.sketch("front", 0.0, f"groove {d:g}")
            z0 = i * GAUGE_H + GAUGE_H / 2 - 0.3
            g.polygon(s, [(d / 2 - 0.4, z0), (d / 2 + 0.5, z0), (d / 2 + 0.5, z0 + 0.6), (d / 2 - 0.4, z0 + 0.6)])
            g.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "remove", "name": f"groove {d:g}"})
    grep = g.report()
    bad = [(ft["name"], ft["error"]) for ft in grep["features"] if ft.get("error")]
    if bad:
        raise RuntimeError(bad)
    print(f"gauge: {len(steps)} steps of {GAUGE_H:g} from {GAUGE_MIN:g} at the top to {GAUGE_MAX:g} at the bottom, {z:.0f} tall")
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
    for k in range(HOOKS):
        a = EAR_AT + k * 360.0 / HOOKS
        s = c.sketch("top", 0.0, f"ear {k + 1}")
        sector(c, s, RING_OD / 2 - 1.0, R_EAR, a, math.degrees(EAR_W / 2 / R_EAR))
        c.extrude(s, RING_T, op="add", name=f"ear {k + 1}")
    crep = c.report()
    bad = [(f["name"], f["error"]) for f in crep["features"] if f.get("error")]
    if bad:
        raise RuntimeError(bad)
    # -- the cap on the ring, locked, and offered up before the turn --------
    place = lambda deg: {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": deg}}
    asms = {}
    for name, deg in (("cap on collar", 0.0), ("cap offered up", 60.0)):
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": name}]})
        asms[name] = int(re.search(r"tab (\d+)", text).group(1))
        mcp.call("apply", {"tab": asms[name], "ops": [{"type": "add_instance", "studio": collar_tab, "body": 0, "name": "collar", "fixed": True, "placement": place(0.0)},
                                                      {"type": "add_instance", "studio": 1, "body": 0, "name": "cap", "fixed": True, "placement": place(deg)}]})
    # -- pictures and files -----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "cap_iso", view="iso")
    shot(1, "cap_below", view="0.5,-0.6,-0.6")
    # The section through the ear on the x axis (the one at EAR_AT + 120)
    # and its hook, looking from the front at the far half.
    shot(asms["cap on collar"], "cap_on_collar_section", view="front", section="y:0")
    shot(asms["cap on collar"], "cap_on_collar_below", view="0.5,-0.6,-0.6")
    shot(asms["cap offered up"], "cap_offered_up_below", view="0,0,-1")
    shot(gauge_tab, "gauge", view="iso")
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "cap.stl")}))
    print(mcp.call("export", {"tab": gauge_tab, "format": "stl", "path": os.path.join(OUT, "gauge.stl")}))
    print(mcp.call("export", {"tab": asms["cap on collar"], "format": "pdf", "sheet": "A4", "hidden": True, "parts": False, "views": ["section@0", "top", "iso"],
                              "note": f"cap on the ring, locked: bore {D_MOUTH:g} at the mouth, {D_TOP:g} at the top, ring {RING_T:g} thick, ears to {2 * R_EAR:g}; the lip comes down from {RAMP[0]:g} to {RAMP[-1]:g} over the ear",
                              "path": os.path.join(OUT, "cap_on_collar.pdf")}))
    numbers = {
        "collar": {"d_mouth": D_MOUTH, "d_top": D_TOP, "ring_t": RING_T, "d_mesh": D_MESH, "od": RING_OD, "ear_w": EAR_W, "ear_r": EAR_R, "ear_at": EAR_AT, "tube_h": TUBE_H},
        "cap": {"clear": CLEAR, "plate_t": PLATE_T, "dome": DOME, "plate_r": PLATE_R, "hook_gap": HOOK_GAP, "hook_wall": HOOK_WALL, "hook_arc": HOOK_ARC,
                "lip_t": LIP_T, "lip_over": LIP_OVER, "ramp": list(RAMP), "stop_t": STOP_T, "drain_d": DRAIN_D, "drains": DRAINS, "hooks": HOOKS,
                "r_ear": R_EAR, "r_wall0": R_WALL0, "r_wall1": R_WALL1, "r_lip": R_LIP, "hook_h": HOOK_H, "offer": 60.0},
        "gauge": {"min": GAUGE_MIN, "max": GAUGE_MAX, "step": GAUGE_STEP, "h": GAUGE_H, "wall": GAUGE_WALL, "steps": steps},
    }
    with open(os.path.join(OUT, "plug.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
