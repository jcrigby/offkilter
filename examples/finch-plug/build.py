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
three rings find the bore by feel.

    cargo build --release -p ok-mcp
    python3 examples/finch-plug/build.py
    cargo test -p ok-render --test finch_plug    # what CI runs

Writes out/finch_plug.okpart, out/cap.stl, out/fit_ring.stl and out/gauge.stl (the parts
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
D_MOUTH = 61.0          # the ring's bore at the mouth, by rule (the first gauge, to 49.5, fell straight through)
D_TOP = 60.0            # the bore at the ring's top edge, inside the tube: a slight draft, guessed
RING_T = 3.0            # the ring's skirt, mouth to the ridge it ends in inside the tube: the plug stops there
EAR_T = 0.8             # the sheet the ring is stamped from, so the flange's and the ears' thickness: under a millimetre
D_MESH = 62.5           # inside the mesh tube just above the ring
RING_OD = 71.0          # the ring's flange: 71.5 and 72.0 to the pencil's centre on the two traced outlines (trace/), less the lead
EAR_W, EAR_R = 35.0, 4.0   # the three ears: width across (54 degrees, 36 mm at the tip to the pencil, on the traces) and how far past the flange (4.0 to 4.4 on the traces)
EAR_AT = 60.0           # the ears' angles are EAR_AT + k * 120: where they sit locked under the hooks
TUBE_H = 25.0           # how much of the mesh tube the model draws
# -- the cap ----------------------------------------------------------------
CLEAR = 0.5             # the plug off the bore: the bore is by rule, so room for half a millimetre of error
PLATE_T = 2.5           # the plate under the ring
DOME = 1.5              # the plug's top rises this much to the centre, so water runs to the mesh
PLATE_R = RING_OD / 2   # the plate reaches the ring's edge, no further, but for the hook lobes
HOOK_GAP = 0.4          # between an ear's tip and the hook's wall
HOOK_WALL = 2.0         # the hook's wall, outside the ears
HOOK_ARC = 36.0         # degrees of arc a hook spans
LIP_T = 2.0             # the lip over the ear
LIP_OVER = 3.0          # how far the lip reaches in over the ear's tip
RAMP = (0.6, 0.4, 0.2, -0.15)   # the lip's clearance over the ear, step by step along the arc, to a bite at the stop: the ear wedges as it turns
FIT_T = 2.0             # the fit ring: the hooks on a bare ring of plate, to print first and try on the tube
FIT_IN = RING_OD / 2 - 3.0   # its inner edge, 3 mm in under the flange
STOP_T = 1.5            # the end stop's thickness, in mm of arc
DRAIN_D, DRAINS = 1.0, 6   # drain holes in the plug, smaller than a nyjer seed
# -- the gauge: three rings to find the ring's bore by feel ------------------
GAUGE_SIZES = (60.0, 61.0, 62.0)   # one too small, one right by rule, one too big; notched 1, 2, 3 on the rim
GAUGE_H, GAUGE_WALL = 6.0, 1.2     # a thin wall and no bottom: a gauge, not a part

R_EAR = RING_OD / 2 + EAR_R          # the ears' tips
R_WALL0 = R_EAR + HOOK_GAP           # the hook wall's inside
R_WALL1 = R_WALL0 + HOOK_WALL        # and outside: the plate's lobes reach here
R_LIP = R_EAR - LIP_OVER             # the lip's inner edge
HOOK_H = EAR_T + RAMP[0] + LIP_T     # the hook's height above the plate: the ear, the entry clearance, the lip
HOOKS = 3
EAR_HALF = math.degrees(EAR_W / 2 / R_EAR)   # half an ear, in degrees at its tip
# Locked, an ear's leading edge rests against the stop at the hook's far
# end, so the hook sits ahead of the ear's centre by this much; offered
# up, the cap is turned so the ears sit mid-gap between the hooks.
HOOK_SHIFT = EAR_HALF + 0.5 - HOOK_ARC / 2
OFFER = 180.0 / HOOKS - HOOK_SHIFT


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


def add_hooks(p, plate_r, plate_t):
    """The hooks, centred where the ears sit once turned home: a lobe of
    the plate out to the wall, the wall, the lip in steps whose
    underside comes down along the arc, and a stop at the far end."""
    half = HOOK_ARC / 2
    for k in range(HOOKS):
        a = EAR_AT + HOOK_SHIFT + k * 360.0 / HOOKS
        s = p.sketch("top", -plate_t, f"lobe {k + 1}")
        sector(p, s, plate_r - 1.0, R_WALL1, a, half + 2.0)
        p.extrude(s, plate_t, op="add", name=f"lobe {k + 1}")
        s = p.sketch("top", 0.0, f"hook wall {k + 1}")
        sector(p, s, R_WALL0, R_WALL1, a, half)
        p.extrude(s, HOOK_H, op="add", name=f"hook wall {k + 1}")
        # The lip: the ear enters at the arc's start (a - half) and turns
        # toward the stop at a + half; each step's underside is lower.
        n = len(RAMP)
        for i, c in enumerate(RAMP):
            seg_half = half / n
            centre = a - half + (i + 0.5) * 2 * seg_half
            s = p.sketch("top", EAR_T + c, f"lip {k + 1}.{i + 1}")
            sector(p, s, R_LIP, R_WALL0 + 0.01, centre, seg_half + 0.01)
            p.extrude(s, HOOK_H - EAR_T - c, op="add", name=f"lip {k + 1}.{i + 1}")
        s = p.sketch("top", 0.0, f"stop {k + 1}")
        stop_half = math.degrees(STOP_T / 2 / R_LIP)
        sector(p, s, R_LIP, R_WALL0 + 0.01, a + half + stop_half, stop_half)
        p.extrude(s, HOOK_H, op="add", name=f"stop {k + 1}")


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
    add_hooks(p, PLATE_R, PLATE_T)
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
    # -- the fit ring: the hooks on a bare ring, to print first ---------------
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "fit ring"}]})
    fit_tab = int(re.search(r"tab (\d+)", text).group(1))
    fr = Part(mcp, fit_tab, "fit ring")
    s = fr.sketch("front", 0.0, "ring")
    fr.polygon(s, [(FIT_IN, -FIT_T), (PLATE_R, -FIT_T), (PLATE_R, 0.0), (FIT_IN, 0.0)])
    f = fr.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "ring"})
    fr.apply({"type": "rename_part", "source": f, "name": "fit ring"}, {"type": "set_part_material", "source": f, "material": {"name": "PLA", "density": 1.24}})
    add_hooks(fr, PLATE_R, FIT_T)
    frep = fr.report()
    bad = [(ft["name"], ft["error"]) for ft in frep["features"] if ft.get("error")]
    if bad:
        raise RuntimeError(bad)
    print(f"fit ring: {len(frep['bodies'])} body, {frep['bodies'][0]['volume'] / 1000:.1f} cm3")
    print(f"cap: {len(rep['bodies'])} body, {hi['x'] - lo['x']:.1f} across, {lo['z']:.1f} to {hi['z']:.1f} tall, {cap['volume'] / 1000:.1f} cm3; "
          f"ears {2 * EAR_HALF:.1f} degrees wide, hooks {HOOK_ARC:g} shifted {HOOK_SHIFT:.1f} ahead, offered up at {OFFER:.1f}, gaps of {360.0 / HOOKS - HOOK_ARC:.0f}")
    # -- the gauge: three rings in one print ----------------------------------
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "gauge"}]})
    gauge_tab = int(re.search(r"tab (\d+)", text).group(1))
    g = Part(mcp, gauge_tab, "gauge")
    pitch = max(GAUGE_SIZES) + 8.0
    for j, d in enumerate(GAUGE_SIZES):
        cx = (j - (len(GAUGE_SIZES) - 1) / 2) * pitch
        s = g.sketch("top", 0.0, f"ring {d:g}")
        g.circle(s, (cx, 0.0), d / 2)
        f = g.extrude(s, GAUGE_H, op="new", name=f"ring {d:g}")
        g.apply({"type": "rename_part", "source": f, "name": f"ring {d:g}"}, {"type": "set_part_material", "source": f, "material": {"name": "PLA", "density": 1.24}})
        s = g.sketch("top", 0.0, f"bore {d:g}")
        g.circle(s, (cx, 0.0), d / 2 - GAUGE_WALL)
        g.extrude(s, GAUGE_H, op="remove", name=f"bore {d:g}")
        # j + 1 notches in the rim, 2 wide and 1.5 deep, to tell the rings apart
        s = g.sketch("top", GAUGE_H - 1.5, f"notches {d:g}")
        for i in range(j + 1):
            ang = 90.0 + (i - j / 2) * 12.0
            u, t = rot(1.0, 0.0, ang), rot(0.0, 1.0, ang)
            c = (cx + u[0] * (d / 2 - GAUGE_WALL / 2), u[1] * (d / 2 - GAUGE_WALL / 2))
            g.polygon(s, [(c[0] + su * u[0] * (GAUGE_WALL / 2 + 0.5) + st * t[0], c[1] + su * u[1] * (GAUGE_WALL / 2 + 0.5) + st * t[1])
                          for su, st in ((-1, -1), (1, -1), (1, 1), (-1, 1))])
        g.extrude(s, 1.5, op="remove", name=f"notches {d:g}")
    grep = g.report()
    bad = [(ft["name"], ft["error"]) for ft in grep["features"] if ft.get("error")]
    if bad:
        raise RuntimeError(bad)
    print(f"gauge: rings of {', '.join(f'{d:g}' for d in GAUGE_SIZES)}, {GAUGE_H:g} tall, {GAUGE_WALL:g} wall, {len(grep['bodies'])} bodies")
    # -- the ring it fits, from the photographs' numbers --------------------
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "collar"}]})
    collar_tab = int(re.search(r"tab (\d+)", text).group(1))
    c = Part(mcp, collar_tab, "collar")
    s = c.sketch("front", 0.0, "ring")
    # Stamped sheet: a flange, a drafted skirt up into the tube, a ridge
    # where the mesh starts, then the mesh tube.
    collar = [(D_MOUTH / 2, 0.0), (RING_OD / 2, 0.0), (RING_OD / 2, EAR_T), (D_MOUTH / 2 + EAR_T, EAR_T), (D_TOP / 2 + EAR_T, RING_T),
              (D_MESH / 2 + 1.0, RING_T), (D_MESH / 2 + 1.0, TUBE_H), (D_MESH / 2, TUBE_H), (D_MESH / 2, RING_T + EAR_T), (D_TOP / 2, RING_T + EAR_T)]
    c.polygon(s, collar)
    f = c.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "ring"})
    c.apply({"type": "rename_part", "source": f, "name": "collar"}, {"type": "set_part_material", "source": f, "material": {"name": "ABS", "density": 1.05}})
    for k in range(HOOKS):
        a = EAR_AT + k * 360.0 / HOOKS
        s = c.sketch("top", 0.0, f"ear {k + 1}")
        sector(c, s, RING_OD / 2 - 1.0, R_EAR, a, EAR_HALF)
        c.extrude(s, EAR_T, op="add", name=f"ear {k + 1}")
    crep = c.report()
    bad = [(f["name"], f["error"]) for f in crep["features"] if f.get("error")]
    if bad:
        raise RuntimeError(bad)
    # -- the cap on the ring, locked, and offered up before the turn --------
    place = lambda deg: {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": deg}}
    asms = {}
    for name, studio, deg in (("cap on collar", 1, 0.0), ("cap offered up", 1, OFFER), ("fit ring on collar", fit_tab, 0.0)):
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": name}]})
        asms[name] = int(re.search(r"tab (\d+)", text).group(1))
        mcp.call("apply", {"tab": asms[name], "ops": [{"type": "add_instance", "studio": collar_tab, "body": 0, "name": "collar", "fixed": True, "placement": place(0.0)},
                                                      {"type": "add_instance", "studio": studio, "body": 0, "name": "cap", "fixed": True, "placement": place(deg)}]})
    # -- pictures and files -----------------------------------------------
    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "cap_iso", view="iso")
    shot(1, "cap_below", view="0.5,-0.6,-0.6")
    # The section through the ear on the x axis (the one at EAR_AT + 120)
    # and its hook, looking from the front at the far half.
    shot(asms["cap on collar"], "cap_on_collar_section", view="front", section="y:0")
    shot(asms["cap on collar"], "cap_on_collar_below", view="0.5,-0.6,-0.6")
    shot(asms["cap offered up"], "cap_offered_up_below", view="0,0,-1")
    shot(asms["fit ring on collar"], "fit_ring_on_collar", view="0.5,-0.6,-0.6")
    shot(gauge_tab, "gauge", view="iso")
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "cap.stl")}))
    print(mcp.call("export", {"tab": gauge_tab, "format": "stl", "path": os.path.join(OUT, "gauge.stl")}))
    print(mcp.call("export", {"tab": fit_tab, "format": "stl", "path": os.path.join(OUT, "fit_ring.stl")}))
    print(mcp.call("export", {"tab": asms["cap on collar"], "format": "pdf", "sheet": "A4", "hidden": True, "parts": False, "views": ["section@0", "top", "iso"],
                              "note": f"cap on the ring, locked: bore {D_MOUTH:g} at the mouth, {D_TOP:g} at the top, skirt {RING_T:g} deep, sheet {EAR_T:g}, ears to {2 * R_EAR:g}; the lip comes down from {RAMP[0]:g} over the ear to {-RAMP[-1]:g} into it",
                              "path": os.path.join(OUT, "cap_on_collar.pdf")}))
    numbers = {
        "collar": {"d_mouth": D_MOUTH, "d_top": D_TOP, "ring_t": RING_T, "ear_t": EAR_T, "d_mesh": D_MESH, "od": RING_OD, "ear_w": EAR_W, "ear_r": EAR_R, "ear_at": EAR_AT, "tube_h": TUBE_H},
        "cap": {"clear": CLEAR, "plate_t": PLATE_T, "dome": DOME, "plate_r": PLATE_R, "hook_gap": HOOK_GAP, "hook_wall": HOOK_WALL, "hook_arc": HOOK_ARC,
                "lip_t": LIP_T, "lip_over": LIP_OVER, "ramp": list(RAMP), "stop_t": STOP_T, "drain_d": DRAIN_D, "drains": DRAINS, "hooks": HOOKS,
                "r_ear": R_EAR, "r_wall0": R_WALL0, "r_wall1": R_WALL1, "r_lip": R_LIP, "hook_h": HOOK_H, "ear_half": EAR_HALF,
                "hook_shift": HOOK_SHIFT, "fit_t": FIT_T, "fit_in": FIT_IN, "hook_at": [EAR_AT + HOOK_SHIFT + k * 360.0 / HOOKS for k in range(HOOKS)], "offer": OFFER},
        "gauge": {"sizes": list(GAUGE_SIZES), "h": GAUGE_H, "wall": GAUGE_WALL, "pitch": pitch},
        "trace": {"flange_od_pencil": [71.5, 72.0], "ear_tip_r_pencil": [39.8, 39.8, 39.95, 40.35], "ear_proud": [4.0, 4.1, 4.2, 4.4], "ear_deg": [54, 54, 54, 52],
                  "ear_chord_pencil": [36.2, 36.3, 35.4], "squares_per_inch": 5, "pencil": 0.5},
    }
    with open(os.path.join(OUT, "plug.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
