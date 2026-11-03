#!/usr/bin/env python3
"""The lint blower adapter again, in 3 mm ABS sheet with printed joints:
an exercise in how much printing flat stock saves. The duct is five flat
pieces of sheet: the back, the blade's face and the wedge's panel in
one; the front, the same in one piece bent at the lip, since the sheet
heat-forms; and the two ends. Printed are four bars the faces glue to,
which set the blade's thickness, and the cap, a square ring with a
groove the panels' top edges drop into, rounding to the tapered spigot
the leaf vac hose jams onto. The numbers are build.py's; this takes
them from it.

    cargo build --release -p ok-mcp
    python3 examples/lint-blower/build_sheet.py
    cargo test -p ok-render --test lint_blower_sheet    # what CI runs

Writes out/lint_blower_sheet.okpart; out/bar.stl (print four) and
out/cap.stl; out/panels.dxf and out/panels.pdf, the four panels and
two rib strips flat on three 12 x 16 sheets, 1:1 in the DXF, the
front's bend line marked; pictures; and out/sheet.json.
"""
import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

spec = importlib.util.spec_from_file_location("build", os.path.join(HERE, "build.py"))
B = importlib.util.module_from_spec(spec)
spec.loader.exec_module(B)

OUT = os.path.join(HERE, "out")
# -- the sheet --------------------------------------------------------------
SHEET_T = 3.0           # 0.12 in ABS
SHEET_W, SHEET_L = 304.8, 406.4   # 12 x 16 in
GROOVE = SHEET_T + 0.2  # the cap's groove, the sheet and a glue gap
GROOVE_D = 6.0          # how far the panels go up into the cap
# -- the blade, in sheet: the faces are the sheet, the bars between them set the thickness
BLADE_W, BLADE_T, BLADE_D = B.BLADE_W, B.BLADE_T, B.BLADE_D
BAR_W = 10.0            # a bar into the blade from its end, or either side of a rib's line
BAR_T = BLADE_T - 2 * SHEET_T     # between the faces
RIBS = 2
# -- the wedge, in sheet: from the blade's outline at the lip to the cap's square
WEDGE_H = 100.0
CAP_WALL = 2.0          # the cap's walls either side of its groove
SQ = B.SPIGOT_FOOT + 2 * B.WALL   # the wedge's outside at the top: the spigot's foot and a wall, the cap's circle
CAP_SQ = SQ + 2 * CAP_WALL        # the cap's square outside: the groove's outer wall round the panels
AXIS_Y = SQ / 2         # the spigot's axis: the square and the circle sit on the back panel's plane
# -- the cap
CAP_BASE_T = GROOVE_D + 2.0
LOFT2_H = 30.0

Z_LIP = 0.0
Z_TOP_PANEL = WEDGE_H
Z_CAP0 = Z_TOP_PANEL - GROOVE_D
Z_ROUND = Z_CAP0 + CAP_BASE_T
Z_SPIGOT = Z_ROUND + LOFT2_H
Z_TOP = Z_SPIGOT + B.SPIGOT_H
HW = BLADE_W / 2
HS = SQ / 2
LEAN_FRONT = math.atan((SQ - BLADE_T) / WEDGE_H)      # the front panel off the vertical
LEAN_END = math.atan((HW - HS) / WEDGE_H)             # the end panels off the vertical
FOLD_ANGLE = 180.0 - math.degrees(LEAN_FRONT)         # the bend in the front sheet, outside: a 42° bend

# The panels' corners in the dryer's frame, on their outer surfaces: x
# along the slot, y from the back face toward the user, z up from the lip.
PANELS = {
    "back": [(-HW, 0.0, -BLADE_D), (HW, 0.0, -BLADE_D), (HW, 0.0, 0.0), (HS, 0.0, WEDGE_H), (-HS, 0.0, WEDGE_H), (-HW, 0.0, 0.0)],
    # the front, one piece bent at the lip: the face below, the panel above
    "front": [(-HW, BLADE_T, -BLADE_D), (HW, BLADE_T, -BLADE_D), (HW, BLADE_T, 0.0), (HS, SQ, WEDGE_H), (-HS, SQ, WEDGE_H), (-HW, BLADE_T, 0.0)],
    # the ends sit between the back and front panels, inside their faces
    "right end": [(HW, SHEET_T, 0.0), (HW, BLADE_T - SHEET_T, 0.0), (HS, SQ - SHEET_T, WEDGE_H), (HS, SHEET_T, WEDGE_H)],
    "left end": [(-HW, BLADE_T - SHEET_T, 0.0), (-HW, SHEET_T, 0.0), (-HS, SHEET_T, WEDGE_H), (-HS, SQ - SHEET_T, WEDGE_H)],
}


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def norm(a):
    k = math.sqrt(dot(a, a))
    return (a[0] / k, a[1] / k, a[2] / k)


def flatten(verts):
    """A planar polygon's corners in its own plane: u along the first
    edge, v square to it, from the first corner."""
    u = norm(sub(verts[1], verts[0]))
    w = sub(verts[-1], verts[0])
    v = norm(sub(w, tuple(c * dot(w, u) for c in u)))
    return [(dot(sub(p, verts[0]), u), dot(sub(p, verts[0]), v)) for p in verts]


def flatten_front(verts):
    """The front, bent at the lip: the face flat, then the panel unfolded
    about the bend line, so its corners lie as far past the line as they
    are up the panel."""
    face = [(x, z) for x, _, z in verts[:3]] + [(verts[5][0], verts[5][2])]
    # the panel's height up its surface: the top edge's rise and its lean
    height = math.hypot(verts[3][1] - verts[2][1], verts[3][2] - verts[2][2])
    return [face[0], face[1], face[2], (verts[3][0], height), (verts[4][0], height), face[3]]


def edge_lengths(verts):
    return [math.dist(verts[i], verts[(i + 1) % len(verts)]) for i in range(len(verts))]


def polygon_area(pts):
    return abs(sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))) / 2


def checked(part):
    bodies = part.bodies()
    print(f"{part.name}: {len(bodies)} bodies", ", ".join(f"{n or 'body'} {v / 1000:.1f} cm3" for n, v in bodies))
    return bodies


def build_bar(p, length, name="bar"):
    """A bar the faces glue to: BAR_W into the blade, BAR_T between the
    faces, `length` down it; printed lying on a face."""
    s = p.sketch("top", 0.0, name)
    p.rect(s, (0.0, 0.0), (length, BAR_W))
    p.extrude(s, BAR_T, name=name)


def groove_rect(z, inset):
    """The panels' outside at height z in the cap, `inset` inside it: the
    back vertical, the ends spreading outward below the top and the
    front, which leans toward the user going up, falling back."""
    down = Z_TOP_PANEL - z
    ex = down * math.tan(LEAN_END)
    ey = down * math.tan(LEAN_FRONT)
    return (-HS - ex + inset, inset), (HS + ex - inset, SQ - ey - inset)


def build_cap(p):
    """The cap: a skirt round the panels' tops with a groove in its
    underside they go up into, leaning as they lean, a plate over it,
    then the square rounding to the spigot's circle and the tapered
    spigot, one bore through."""
    half = CAP_SQ / 2
    (ax, ay), (bx, by) = groove_rect(Z_CAP0, -CAP_WALL)
    a = p.sketch("top", Z_CAP0, "skirt foot")
    p.rect(a, (ax, ay), (bx, by))
    b = p.sketch("top", Z_TOP_PANEL, "skirt top")
    p.rect(b, (-half, AXIS_Y - half), (half, AXIS_Y + half))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "new", "name": "skirt"})
    s = p.sketch("top", Z_TOP_PANEL, "cap plate")
    p.rect(s, (-half, AXIS_Y - half), (half, AXIS_Y + half))
    p.extrude(s, CAP_BASE_T - GROOVE_D, op="add", name="cap plate")
    # the groove: the panels' band, a hair past the skirt's foot, up to
    # the plate; then its core put back
    (ax, ay), (bx, by) = groove_rect(Z_CAP0 - 1.0, 0.0)
    a = p.sketch("top", Z_CAP0 - 1.0, "groove foot")
    p.rect(a, (ax, ay), (bx, by))
    b = p.sketch("top", Z_TOP_PANEL, "groove top")
    p.rect(b, (-HS, 0.0), (HS, SQ))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "remove", "name": "groove"})
    (ax, ay), (bx, by) = groove_rect(Z_CAP0, GROOVE)
    a = p.sketch("top", Z_CAP0, "core foot")
    p.rect(a, (ax, ay), (bx, by))
    b = p.sketch("top", Z_TOP_PANEL, "core top")
    p.rect(b, (-HS + GROOVE, GROOVE), (HS - GROOVE, SQ - GROOVE))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "add", "name": "groove core"})
    g_in = HS - GROOVE
    b = p.sketch("top", Z_ROUND, "round from the square")
    p.rect(b, (-half, AXIS_Y - half), (half, AXIS_Y + half))
    c = p.sketch("top", Z_SPIGOT, "round to the circle")
    p.circle(c, (0.0, AXIS_Y), SQ / 2)
    p.feature({"type": "add_loft", "sketch": b, "sketch_b": c, "op": "add", "name": "square to round"})
    s = p.sketch("front", -AXIS_Y, "spigot")
    p.polygon(s, [(0.0, Z_SPIGOT), (B.SPIGOT_FOOT / 2, Z_SPIGOT), (B.SPIGOT_TIP / 2, Z_TOP), (0.0, Z_TOP)])
    p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "add", "name": "spigot"})
    # The bore: the opening inside the groove's inner wall up to the
    # round, the square to the circle (centred, as the kernel's loft
    # wants), and the spigot's.
    o = g_in - CAP_WALL
    s = p.sketch("top", Z_CAP0 - 2.0, "opening")
    p.rect(s, (-o, AXIS_Y - o), (o, AXIS_Y + o))
    p.cut(s, 2.0 + CAP_BASE_T + 0.01, name="opening")
    b = p.sketch("top", Z_ROUND - 0.01, "bore from the square")
    p.rect(b, (-o + 0.02, AXIS_Y - o + 0.02), (o - 0.02, AXIS_Y + o - 0.02))
    c = p.sketch("top", Z_SPIGOT + 0.01, "bore to the circle")
    p.circle(c, (0.0, AXIS_Y), B.R_BORE)
    p.feature({"type": "add_loft", "sketch": b, "sketch_b": c, "op": "remove", "name": "round bore"})
    s = p.sketch("top", Z_SPIGOT - 0.01, "spigot bore")
    p.circle(s, (0.0, AXIS_Y), B.R_BORE)
    p.cut(s, B.SPIGOT_H + 1.0, name="spigot bore")


def build_assembled(p):
    """Everything in place: the panels as plates (the leaning ones lofted
    between their bottom and top edges, the thickness measured across the
    duct; the front as its face and its panel), the bars and the cap."""
    t = SHEET_T
    # The cap first: its adds would merge with any body their bounds
    # touch, and the panels reach up into its groove.
    build_cap(p)
    # the back, one piece, and the front face: plates on the front plane
    s = p.sketch("front", 0.0, "back")
    p.polygon(s, [(x, z) for x, _, z in PANELS["back"]])
    p.extrude(s, t, direction="reverse", name="back")
    s = p.sketch("front", -(BLADE_T - t), "front face")
    p.polygon(s, [(x, z) for x, _, z in PANELS["front"][:3]] + [(PANELS["front"][5][0], PANELS["front"][5][2])])
    p.extrude(s, t, direction="reverse", name="front face")
    # the leaning panels: lofts between their bottom and top edges
    a = p.sketch("top", 0.0, "front panel foot")
    p.rect(a, (-HW, BLADE_T - t), (HW, BLADE_T))
    b = p.sketch("top", WEDGE_H, "front panel top")
    p.rect(b, (-HS, SQ - t), (HS, SQ))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "new", "name": "front panel"})
    for name, sx in (("right end", 1.0), ("left end", -1.0)):
        a = p.sketch("top", 0.0, f"{name} foot")
        p.rect(a, (min(sx * HW, sx * (HW - t)), t), (max(sx * HW, sx * (HW - t)), BLADE_T - t))
        b = p.sketch("top", WEDGE_H, f"{name} top")
        p.rect(b, (min(sx * HS, sx * (HS - t)), t), (max(sx * HS, sx * (HS - t)), SQ - t))
        p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "new", "name": name})
    # the bars: two at the ends and two ribs, the blade's depth
    for name, x0, z0, length in (("right bar", HW - BAR_W, -BLADE_D, BLADE_D), ("left bar", -HW, -BLADE_D, BLADE_D),
                                 ("right rib", HW / 3 - BAR_W / 2, -BLADE_D, BLADE_D), ("left rib", -HW / 3 - BAR_W / 2, -BLADE_D, BLADE_D)):
        s = p.sketch("top", z0, name)
        p.rect(s, (x0, t), (x0 + BAR_W, t + BAR_T))
        p.extrude(s, length, name=name)


def build_panels(p):
    """The panels flat, as plates on three 12 x 16 sheets laid side by
    side, for the DXF to cut by: the back on one, the front on another
    with its bend line scored as a shallow groove, the ends nested head
    to tail on the third with the two rib strips beside them."""
    flats = {k: (flatten_front(v) if k == "front" else flatten(v)) for k, v in PANELS.items()}
    gap = 10.0
    rib = [(0.0, 0.0), (BAR_T, 0.0), (BAR_T, BLADE_D), (0.0, BLADE_D)]   # standing, beside the ends

    def box(pts):
        xs, ys = [q[0] for q in pts], [q[1] for q in pts]
        return min(xs), min(ys), max(xs), max(ys)

    def put(pts, at, turn=False):
        """`pts` with their bounding box's corner at `at`, turned 180° first if `turn`."""
        x0, y0, x1, y1 = box(pts)
        if turn:
            pts = [(x1 - (x - x0), y1 - (y - y0)) for x, y in pts]
            x0, y0, x1, y1 = box(pts)
        return [(at[0] + x - x0, at[1] + y - y0) for x, y in pts]

    pitch = SHEET_W + 20.0
    for i in range(3):
        s = p.sketch("top", -1.0, f"sheet {i + 1}")
        p.rect(s, (i * pitch, 0.0), (i * pitch + SHEET_W, SHEET_L))
        p.extrude(s, 0.5, name=f"sheet {i + 1}")
    placed = [("back", put(flats["back"], (gap, gap)))]
    front = put(flats["front"], (pitch + gap, gap))
    placed.append(("front", front))
    # the ends are right trapezoids, the back corner's edge square to the
    # bottom, mirror images of each other: the left one is cut face down,
    # so its pattern is the right's, and turned 180° its slant runs
    # parallel to the right's; the two nest with the left's box starting
    # the bottom's length and a gap past the right's box's start
    right = put(flats["right end"], (2 * pitch + gap, gap))
    bottom = math.dist(flats["right end"][0], flats["right end"][1])
    rx0, _, rx1, _ = box(right)
    mirrored = [(-x, y) for x, y in flats["left end"]]
    left = put(mirrored, (rx0 + bottom + gap, gap), turn=True)
    placed += [("right end", right), ("left end", left)]
    _, _, lx1, _ = box(left)
    for k in range(RIBS):
        placed.append(("rib", put(rib, (lx1 + gap + k * (BAR_T + gap), gap))))
    for name, pts in placed:
        s = p.sketch("top", 0.0, f"{name} flat")
        p.polygon(s, pts)
        p.extrude(s, SHEET_T, name=f"{name} flat")
    # the front's bend line: a shallow groove across it at the lip, to score by
    fx0, fy0, fx1, _ = box(front)
    s = p.sketch("top", SHEET_T, "bend line")
    p.rect(s, (fx0 - 1.0, fy0 + BLADE_D - 0.3), (fx1 + 1.0, fy0 + BLADE_D + 0.3))
    p.cut(s, 0.5, direction="reverse", name="bend line")
    # nothing over anything else, everything on its sheet
    for i in range(len(placed)):
        for j in range(i + 1, len(placed)):
            a, b = box(placed[i][1]), box(placed[j][1])
            apart = a[2] <= b[0] or b[2] <= a[0] or a[3] <= b[1] or b[3] <= a[1]
            if not apart:
                # the nested ends share a box but not a point: check the slants
                assert {placed[i][0], placed[j][0]} == {"right end", "left end"}, (placed[i][0], placed[j][0])
    for name, pts in placed:
        x0, y0, x1, y1 = box(pts)
        sheet = int(x0 // pitch)
        assert x1 <= sheet * pitch + SHEET_W + 1e-9 and y1 <= SHEET_L + 1e-9, (name, x0, y0, x1, y1)
    return flats, placed


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "lint_blower_sheet.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "lint_blower_sheet"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "LG lint duct blower adapter, in sheet"}, {"type": "rename_tab", "tab": 1, "name": "assembled"}]})
    asm = Part(mcp, 1, "assembled")
    build_assembled(asm)
    bodies = checked(asm)
    assert len(bodies) == 10, bodies
    parts = {}
    for name, builder in (("bar", lambda p: build_bar(p, BLADE_D)), ("cap", build_cap)):
        tab = new_tab(mcp, {"type": "add_part_studio", "name": name})
        part = Part(mcp, tab, name)
        builder(part)
        got = checked(part)
        assert len(got) == 1, got
        parts[name] = (tab, got[0][1])
    panels_tab = new_tab(mcp, {"type": "add_part_studio", "name": "panels"})
    panels = Part(mcp, panels_tab, "panels")
    flats, placed = build_panels(panels)
    checked(panels)

    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(1, "sheet_iso", view="0.6,-1,0.4")
    shot(1, "sheet_duct_side", view="-0.6,1,0.4")
    shot(1, "sheet_section", view="right", section="x:0")
    shot(parts["cap"][0], "cap_iso", view="0.6,-1,0.5")
    shot(parts["cap"][0], "cap_below", view="0.6,-1,-0.7")
    shot(panels_tab, "panels", view="top")
    for name, (tab, _) in parts.items():
        print(mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, name.replace(" ", "_") + ".stl")}))
    print(mcp.call("export", {"tab": panels_tab, "format": "dxf", "view": "top", "path": os.path.join(OUT, "panels.dxf")}))
    print(mcp.call("export", {"tab": panels_tab, "format": "pdf", "sheet": "A4", "views": ["top"], "path": os.path.join(OUT, "panels.pdf"),
                              "note": f"four panels and two ribs of {SHEET_T:g} mm sheet on three 12 x 16 sheets, the front bent on its line; the DXF is 1:1"}))
    numbers = {
        "sheet": {"t": SHEET_T, "w": SHEET_W, "l": SHEET_L, "groove": GROOVE, "groove_d": GROOVE_D},
        "blade": {"w": BLADE_W, "t": BLADE_T, "d": BLADE_D, "bar_w": BAR_W, "bar_t": BAR_T, "ribs": RIBS, "rib_l": BLADE_D},
        "wedge": {"h": WEDGE_H, "sq": SQ, "cap_sq": CAP_SQ, "axis_y": AXIS_Y, "lean_front": math.degrees(LEAN_FRONT), "lean_end": math.degrees(LEAN_END),
                  "fold_angle": FOLD_ANGLE},
        "cap": {"z0": Z_CAP0, "base_t": CAP_BASE_T, "z_round": Z_ROUND, "z_spigot": Z_SPIGOT, "z_top": Z_TOP, "opening": 2 * (HS - GROOVE - CAP_WALL),
                "spigot_tip": B.SPIGOT_TIP, "spigot_foot": B.SPIGOT_FOOT, "spigot_taper": B.SPIGOT_TAPER, "r_bore": B.R_BORE, "hose_id": B.HOSE_ID},
        "panels": {k: {"corners": v, "flat": flats[k], "edges": edge_lengths(v), "area": polygon_area(flats[k]), "bent": k == "front"} for k, v in PANELS.items()},
        "volumes": {k: v for k, (_, v) in parts.items()},
    }
    with open(os.path.join(OUT, "sheet.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"blade {BLADE_W:g} x {BLADE_T:g} x {BLADE_D:g} in {SHEET_T:g} sheet on {BAR_W:g} x {BAR_T:g} bars; wedge {WEDGE_H:g} tall to a {SQ:g} square, "
          f"front leaning {math.degrees(LEAN_FRONT):.1f} deg (fold {FOLD_ANGLE:.1f}), ends {math.degrees(LEAN_END):.1f}; cap {CAP_SQ:g} square, groove {GROOVE:g} x {GROOVE_D:g}")
    for k, v in PANELS.items():
        print(f"  {k:12s} edges " + ", ".join(f"{e:.1f}" for e in edge_lengths(v)) + f"  area {polygon_area(flats[k]) / 100:.0f} cm2")


if __name__ == "__main__":
    main()
