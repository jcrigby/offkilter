"""A folding torsion-box truck ramp: two 31-7/8 x 48 in panels of 7/16 in
OSB skins on 1-15/16 birch ribs, hinged on two 1/2 in bolts through
steel lug plates outside the rails, which pull to separate the halves.
The brief is README.md in this directory; this script is the document
it asked for, with the changes its section 9 records.

    cargo build --release -p ok-mcp
    python3 examples/ramp/build.py            # the document, pictures, sheets and cut list
    cargo test -p ok-render --test ramp       # what CI runs

The shop works in inches, the document in millimetres like the other
examples: every dimension is a named constant in inches and the helpers
convert. Frame of a panel: x across the width (0 to 31-7/8), y along the
length (0 at the hinge end, 48 at the free end), z up from the bottom
face of the bottom skin. The two panels are alike but for the free
end (A's 2x8 stubs run bare for the end plates, B's stop flush for the
ground angle): the hinge is a 1/4 in steel lug plate screwed to the
outside of each rail at the hinge end, flat on the left rail and on a
3/8 spacer on the right, so that turned, B's lugs land beside A's and
a 1/2 in bolt through each pair is the pivot. Panel B is placed turned
180 degrees about z and moved 31-7/8 in x. Each panel is
built in place in its own frame; the bolts, end plates and ground
angle are built in the ramp's frame.
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
IN = 25.4

# ---------------------------------------------------------------------
# The panels, in inches.
# ---------------------------------------------------------------------
OSB_SHEET = (95.875, 47.875)       # a 4x8 of OSB sheathing is sized for spacing: 95-7/8 x 47-7/8
BIRCH_SHEET = (96.0, 48.0)         # the hardwood ply is a true 4x8
KERF = 0.125                       # the cut sheets: a saw kerf between pieces
W, L = 31.875, 48.0                # a panel, over the box: 31-7/8 for a 29 in aerator, a rip along the OSB sheet so its strength axis runs along the ramp; 16 in of the sheet left for the shims
SKIN = 0.4375                      # 7/16 OSB
LUMBER = 1.5                       # a 2x4 or 2x8 flat
RIB = LUMBER + SKIN                # the box's core, and the ribs' height, 1-15/16: the box is SKIN + RIB + SKIN, and the lumber in it carries a shim of the skin stock to make it up
RIB_W = 0.75                       # the ribs and cross blocks: strips ripped from the 3/4 birch, on edge
BOX = 2 * SKIN + RIB               # 2.8125
SKIN_END = 45.0                    # the skins stop here; panel A's stubs run bare to 48, panel B's stop flush
CURB = 1.0                         # the rails stand this much above the deck
PIN_DROP = 1.5                     # pipe centre below the bottom skin: lever arm against hang, and the most the lugs' radius can be
BIRCH = 0.75                       # the rails, ribs, cross blocks and hinge blocks: one ply
RAIL_H = BOX + CURB                # 3.8125 along the length
# The hinge: two straight bars a side, 1/4 steel. A band 2 x 11-3/4 flat
# along the rail's face carries the screws into the rail, the edge rib
# and the hinge block; a strut 2-1/2 wide runs from the pivot hole
# below the box up at STRUT_ANGLE onto the band, lapped over it, and
# four of the band's screws pass through it. On the left side the
# band is against the rail and the strut outside it; on the right the
# strut is against the rail and the band outside, so that turned, the
# other panel's strut lands beside this one's on the bolt with nothing
# between. A 1/2 bolt through each pair of struts is the pivot.
BAR_T = 0.25                                  # the bars' thickness, both
BAND_W, BAND_L, BAND_Z0 = 2.0, 11.75, 0.4      # the band: 2 x 1/4 bar, from the joint back 11-3/4, its bottom 0.4 above the panel's bottom
STRUT_W, STRUT_L, STRUT_ANGLE = 2.5, 10.75, 25.0  # the strut: 2-1/2 x 1/4 bar, 10-3/4 overall with a rounded pivot end, at 25 degrees
PACKER_Y0 = 9.25                              # the right side's band is outside the strut: a packer of the same bar under its far end from here
GAP = 0.0                                     # the two struts of a pair touch on the bolt
BOLT_D, HOLE_D, BOLT_L = 0.5, 0.53125, 1.5    # the pivot bolt and the struts' 17/32 hole
HEAD_D, HEAD_T, NUT_T = 0.75, 0.3125, 0.4375  # hex head across flats, as a cylinder
WASHER_OD, WASHER_T = 1.0625, 0.09            # 1/2 SAE flat washers under the head and nut
SCREW_D, SCREW_L = 0.3125, 3.125              # 5/16 x 3-1/8 structural screws (GRK RSS or equal)
SCREW_Y = (4.5, 5.75, 7.0, 8.25, 9.75, 11.0)  # along the band behind the joint block; the first four cross the strut, the last two clear its end
HINGE_BLOCK_T, HINGE_BLOCK_L = 2 * BIRCH, BAND_L  # two more plies behind each edge rib, joint block to the band's end
CHAMFER = 0.375                    # the bottom skin's hinge-end edge, which sweeps a 1-1/2 in radius about the pin
RIBS = (RIB_W / 2, 8.0, 13.5, W - 13.5, W - 8.0, W - RIB_W / 2)   # rib centres, both panels, symmetric: the edge ribs flush with the sides behind the rails (the lug screws go through them), the second and fifth clearing the 2x8 stubs by 3/8
CROSS_Y = (16.0, 32.0)             # cross block rows
JOINT = 3.5                        # the 2x4 flat across the hinge end, between the skins
STUB_W, STUB_L, STUB_Y0 = 7.25, 12.0, 36.0   # 2x8 flat at the free end from y 36: 12 long on panel A, bare past 45; 9 on panel B, flush
BEVEL = 30.0                       # degrees, on the ground end
PLATE_L = 2.0                      # the end plates' ghost
ANGLE_T, ANGLE_LEG = 0.125, 2.0    # the aluminium angle on the ground end: 2 in legs, so both screw rows clear the top skin and the shim
ANGLE_SCREW_D, ANGLE_SCREW_Z = 0.1875, (1.1, 1.7)     # #10 clearance; the two rows below the top, both in the stubs' end grain under the shim

# Panel B's placement in the ramp: 180 degrees about z, moved W in x:
# its x runs 31-7/8 to 0, its y 0 to -48.
PLACE_B = ((W * IN, 0.0, 0.0), (0.0, 0.0, 180.0))
HINGE_CUT = -(BIRCH + 1.5 * BAR_T)  # the hinge section: through panel A's left strut
MATERIALS = {"osb": ("7/16 OSB", 0.62), "birch": ("3/4 birch", 0.68), "steel": ("steel", 7.85), "aluminium": ("aluminium", 2.7)}
# The strut's frame: s along it from the pivot, t across it.
STRUT_D = (math.cos(math.radians(STRUT_ANGLE)), math.sin(math.radians(STRUT_ANGLE)))
STRUT_N = (-STRUT_D[1], STRUT_D[0])


def on_strut(y, z):
    """(s, t) of a point in the y-z plane in the strut's frame."""
    dy, dz = y, z + PIN_DROP
    return dy * STRUT_D[0] + dz * STRUT_D[1], dy * STRUT_N[0] + dz * STRUT_N[1]


def strut_covers(y, z, margin):
    """Whether a hole of radius `margin` at (y, z) lies within the
    strut's bar."""
    s_, t = on_strut(y, z)
    return margin <= s_ <= STRUT_L - STRUT_W / 2 - margin and abs(t) <= STRUT_W / 2 - margin   # the parts list's material and g/cm3


def mm(*v):
    return tuple(c * IN for c in v)


def inch(v):
    """Inches as the shop writes them: 1.9375 is 1-15/16."""
    whole, frac = divmod(round(v * 16), 16)
    if frac == 0:
        return f"{whole:g}"
    n, d = frac, 16
    while n % 2 == 0:
        n, d = n // 2, d // 2
    return f"{whole}-{n}/{d}" if whole else f"{n}/{d}"


def box(p, name, x, y, z, op="new"):
    """A block, inches."""
    s = p.sketch("top", z[0] * IN, name)
    p.rect(s, mm(x[0], y[0]), mm(x[1], y[1]))
    return p.extrude(s, (z[1] - z[0]) * IN, op=op, name=name)


def part_is(p, f, name, material=None):
    """Names the body feature `f` made and sets its material: what the
    parts lists and cut sheets call it."""
    ops = [{"type": "rename_part", "source": f, "name": name}]
    if material:
        material, density = MATERIALS[material]
        ops.append({"type": "set_part_material", "source": f, "material": {"name": material, "density": density}})
    p.apply(*ops)


def profile_x(p, name, x0, x1, pts, circle=None, op="new"):
    """A polygon in the y-z plane (inches) extruded from x0 to x1, with
    a circle unioned onto it for a half-round end."""
    s = p.sketch("right", x0 * IN, name)
    p.polygon(s, [mm(*pt) for pt in pts])
    f = p.extrude(s, (x1 - x0) * IN, op=op, name=name)
    if circle:
        (cy, cz), r = circle
        s = p.sketch("right", x0 * IN, f"{name} round")
        p.circle(s, mm(cy, cz), r * IN)
        p.extrude(s, (x1 - x0) * IN, op="add", name=f"{name} round")
    return f
    return f


def bore_x(p, name, x0, x1, cy, cz, d):
    """A hole along x through everything on the line, inches."""
    s = p.sketch("right", (x0 - 0.5) * IN, name)
    p.point(s, mm(cy, cz))
    p.hole(s, d * IN, depth=(x1 - x0 + 1.0) * IN, direction="normal", name=name)


# -- parts both panels share -------------------------------------------

def skin_top(p):
    f = box(p, "top skin, 7/16 OSB", (0.0, W), (0.0, SKIN_END), (BOX - SKIN, BOX))
    part_is(p, f, f"top skin {W:g} x {SKIN_END:g}", "osb")
    p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#skin"})


def skin_bottom(p):
    """The bottom skin, its hinge-end bottom edge chamfered since it
    sweeps a radius about the pin."""
    f = profile_x(p, "bottom skin, 7/16 OSB", 0.0, W, [(CHAMFER, 0.0), (SKIN_END, 0.0), (SKIN_END, SKIN), (0.0, SKIN), (0.0, CHAMFER)])
    part_is(p, f, f"bottom skin {W:g} x {SKIN_END:g}", "osb")


def rib_span(c):
    """Where a rib runs: from the joint block to the free end, or to the
    stubs where it would meet one."""
    y1 = STUB_Y0 if (c - RIB_W / 2 < STUB_W or c + RIB_W / 2 > W - STUB_W) else SKIN_END
    return JOINT, y1


def ribs(p):
    for i, c in enumerate(RIBS):
        y0, y1 = rib_span(c)
        f = box(p, f"rib, 3/4 birch strip x {y1 - y0:g}", (c - RIB_W / 2, c + RIB_W / 2), (y0, y1), (SKIN, SKIN + RIB), op="new")
        part_is(p, f, f"{RIB:g} x {y1 - y0:g}", "birch")
        if i == 0:
            p.apply({"type": "set_binding", "id": f, "field": "depth", "expression": "#rib"})


def cross_blocks(p):
    for y in CROSS_Y:
        for a, b in zip(RIBS, RIBS[1:]):
            f = box(p, f"cross block, 3/4 birch strip x {b - a - RIB_W:g}", (a + RIB_W / 2, b - RIB_W / 2), (y - RIB_W / 2, y + RIB_W / 2), (SKIN, SKIN + RIB), op="new")
            part_is(p, f, f"{RIB:g} x {b - a - RIB_W:g}", "birch")


def joint_block(p):
    """The 2x4 flat across the hinge end, with a strip of the skin
    stock over it to make up the box's core."""
    f = box(p, f"joint block, 2x4 flat x {W:g}", (0.0, W), (0.0, JOINT), (SKIN, SKIN + LUMBER))
    part_is(p, f, f"joint block 2x4 x {W:g}")
    f = box(p, f"joint shim, 7/16 OSB strip x {W:g}", (0.0, W), (0.0, JOINT), (SKIN + LUMBER, SKIN + RIB), op="new")
    part_is(p, f, f"joint shim {JOINT:g} x {W:g}", "osb")


def stubs(end):
    """2x8 flat at the free end's outer positions: panel A's run 3 in
    bare past the skins for the end plates; panel B's stop flush with
    the skins' end, where the angle goes. Each carries a strip of the
    skin stock inside the box to make up the core; past the skins the
    stub is bare. The ground bevel is a suppressed feature on panel
    B's: on in the stub's own sheet, off in the ramp."""
    def build(p):
        name = "tailgate stub" if end > SKIN_END else "ground stub"
        for x0 in (0.0, W - STUB_W):
            f = box(p, f"{name}, 2x8 flat x {end - STUB_Y0:g}", (x0, x0 + STUB_W), (STUB_Y0, end), (SKIN, SKIN + LUMBER), op="new")
            part_is(p, f, f"{name} 2x8 x {end - STUB_Y0:g}")
        for x0 in (0.0, W - STUB_W):
            f = box(p, f"stub shim, 7/16 OSB strip x {SKIN_END - STUB_Y0:g}", (x0, x0 + STUB_W), (STUB_Y0, SKIN_END), (SKIN + LUMBER, SKIN + RIB), op="new")
            part_is(p, f, f"stub shim {STUB_W:g} x {SKIN_END - STUB_Y0:g}", "osb")
        if end <= SKIN_END:
            back = RIB * math.tan(math.radians(BEVEL))
            s = p.sketch("right", -1.0 * IN, "bevel")
            p.polygon(s, [mm(end - back, SKIN + RIB), mm(end + 0.1, SKIN + RIB), mm(end + 0.1, SKIN)])
            f = p.cut(s, (W + 2.0) * IN, direction="normal", name=f"ground bevel, {BEVEL:g} degrees")
            p.apply({"type": "set_suppressed", "id": f, "suppressed": True})
    return build


# -- the hinge: panel A's rails with lugs, panel B's plain rails and stubs

def rails_plain(p):
    """Two flush rails, ending flat on the joint plane; the lug plates
    screw to their outer faces."""
    for i, x0 in enumerate((-BIRCH, W)):
        f = box(p, "rail, 3/4 birch", (x0, x0 + BIRCH), (0.0, SKIN_END), (0.0, RAIL_H), op="new")
        part_is(p, f, f"rail {SKIN_END:g} x {RAIL_H:g}", "birch")


def hinge_blocks(p):
    """Two more plies glued behind each edge rib from the joint block
    back, so the lug screws bite the rail, the rib and these: 3 in of
    plywood face grain."""
    for x0 in (RIB_W, W - RIB_W - HINGE_BLOCK_T):
        for k in range(2):
            f = box(p, "hinge block, 3/4 birch strip", (x0 + k * BIRCH, x0 + (k + 1) * BIRCH), (JOINT, HINGE_BLOCK_L), (SKIN, SKIN + RIB), op="new")
            part_is(p, f, f"{RIB:g} x {HINGE_BLOCK_L - JOINT:g}", "birch")


def hinge(right):
    """One side's hinge bars on the outside of the rail: the band flat
    along the rail's face and the strut from the pivot hole up onto it,
    one against the rail and the other outside it (the band inside on
    the left, the strut inside on the right); a 17/32 pivot hole in the
    strut, 11/32 screw holes through whatever each screw crosses, a
    packer under the right band's far end, and the screws as bodies."""
    def plane(k):
        """The x range of the k-th bar out from the rail's face."""
        x0 = W + BIRCH + k * BAR_T if right else -BIRCH - (k + 1) * BAR_T
        return x0, x0 + BAR_T

    def build(p):
        band_x, strut_x = (plane(1), plane(0)) if right else (plane(0), plane(1))
        f = box(p, "hinge band, 2 x 1/4 bar", band_x, (0.0, BAND_L), (BAND_Z0, BAND_Z0 + BAND_W), op="new")
        part_is(p, f, f"hinge band 1/4 x {BAND_W:g} x {BAND_L:g}", "steel")
        h, l = STRUT_W / 2, STRUT_L - STRUT_W / 2
        pt = lambda s_, t: (s_ * STRUT_D[0] + t * STRUT_N[0], -PIN_DROP + s_ * STRUT_D[1] + t * STRUT_N[1])
        f = profile_x(p, "hinge strut, 2-1/2 x 1/4 bar", strut_x[0], strut_x[1], [pt(0.0, -h), pt(l, -h), pt(l, h), pt(0.0, h)], circle=((0.0, -PIN_DROP), h))
        part_is(p, f, f"hinge strut 1/4 x {STRUT_W:g} x {STRUT_L:g}", "steel")
        bore_x(p, "pivot hole, 17/32", strut_x[0], strut_x[1], 0.0, -PIN_DROP, HOLE_D)
        if right:
            f = box(p, "packer, 2 x 1/4 bar", plane(0), (PACKER_Y0, BAND_L), (BAND_Z0, BAND_Z0 + BAND_W), op="new")
            part_is(p, f, f"hinge packer 1/4 x {BAND_W:g} x {BAND_L - PACKER_Y0:g}", "steel")
        # The screw holes through every bar on each screw's line, from
        # just outside the stack's outer face.
        outer = plane(1)[1] if right else plane(1)[0]
        s = p.sketch("right", (outer + (0.5 if right else -0.5)) * IN, "screw holes")
        for y in SCREW_Y:
            p.point(s, mm(y, SKIN + RIB / 2))
        p.hole(s, (SCREW_D + 0.03125) * IN, depth=(2 * BAR_T + 1.0) * IN, direction="reverse" if right else "normal", name="screw holes, 11/32")
        # The screws, head on the outermost bar at each one, in through
        # the bars, the rail, the edge rib and the hinge block.
        for y in SCREW_Y:
            crosses = strut_covers(y, SKIN + RIB / 2, (SCREW_D + 0.03125) / 2)
            assert crosses == (y <= SCREW_Y[3]), f"screw at {y}: {'crosses' if crosses else 'misses'} the strut"
            head = outer if (crosses or right) else band_x[0]
            s = p.sketch("right", head * IN, "screw")
            p.circle(s, mm(y, SKIN + RIB / 2), SCREW_D / 2 * IN)
            f = p.extrude(s, SCREW_L * IN, direction="reverse" if right else "normal", op="new", name="hinge screw, 5/16 x 3-1/8")
            part_is(p, f, "hinge screw 5/16 x 3-1/8", "steel")
    return build


# -- ramp parts, in the ramp's frame ----------------------------------

def hinge_bolts(p):
    """The two pivots: on each side a 1/2 x 1-1/2 bolt through the pair
    of struts, head out, a washer under the head and the nut, a nylock
    nut inboard under the box; one body a side, the washers and nut
    unioned on. On the left the outer strut is panel A's, the inner
    panel B's right one (turned); on the right the other way about."""
    for side in (-1.0, 1.0):
        outer = -(BIRCH + 2 * BAR_T) if side < 0 else W + BIRCH + 2 * BAR_T   # the outer strut's outer face
        at = lambda d: outer + side * d                                                       # d out from that face, negative in
        def cyl(name, a, b, d, op="add"):
            s = p.sketch("right", min(a, b) * IN, name)
            p.circle(s, mm(0.0, -PIN_DROP), d / 2 * IN)
            return p.extrude(s, abs(b - a) * IN, op=op, name=name)
        f = cyl("hinge bolt, 1/2 x 1-1/2 hex", at(WASHER_T), at(WASHER_T + HEAD_T), HEAD_D, op="new")
        cyl("shank", at(WASHER_T), at(WASHER_T - BOLT_L), BOLT_D)
        cyl("washer under the head", at(0.0), at(WASHER_T), WASHER_OD)
        inner = -(2 * BAR_T + GAP)                                                            # the inner strut's inner face
        cyl("washer under the nut", at(inner), at(inner - WASHER_T), WASHER_OD)
        cyl("nylock nut", at(inner - WASHER_T), at(inner - WASHER_T - NUT_T), HEAD_D)
        part_is(p, f, "hinge bolt 1/2 x 1-1/2 hex, grade 5, nylock, 2 washers", "steel")


def end_plates(p):
    """Ghosts of the ramp end plates on panel A's bare stubs: a block
    with the 1-1/2 x 7-1/4 pocket, 2 in long, no casting."""
    for x0 in (0.0, W - STUB_W):
        box(p, "end plate (ghost)", (x0 - 0.25, x0 + STUB_W + 0.25), (L - PLATE_L, L), (0.0, BOX), op="new")
    for x0 in (0.0, W - STUB_W):
        s = p.sketch("front", -(L + 0.5) * IN, "pocket")
        p.rect(s, mm(x0, SKIN), mm(x0 + STUB_W, SKIN + RIB))
        p.cut(s, (PLATE_L + 1.0) * IN, direction="normal", name="board pocket")


def ground_angle(p):
    """1/8 x 2 aluminium angle on panel B's ground end, at the
    ramp's y = -45, where the stubs stop flush with the skins: one leg
    flat against the end face, screwed into the stubs' end grain, the
    other out past the end flush with the top skin, the lip the wheel
    rolls off. With the bevel on, that end face stands plumb and the
    lip lies on the ground."""
    y = -SKIN_END
    top = BOX
    box(p, f"ground angle, 1/8 x 2 aluminium x {W:g}", (0.0, W), (y - ANGLE_LEG, y), (top - ANGLE_T, top))
    box(p, "leg down the end", (0.0, W), (y - ANGLE_T, y), (top - ANGLE_LEG, top), op="add")
    # Two screws per stub through the leg into the end grain, one high
    # and one low (both below the top skin), 3 in apart.
    s = p.sketch("front", -(y - 0.5) * IN, "screw holes")
    for x0 in (0.0, W - STUB_W):
        xc = x0 + STUB_W / 2
        for dx, dz in ((-1.5, ANGLE_SCREW_Z[0]), (1.5, ANGLE_SCREW_Z[1])):
            p.point(s, mm(xc + dx, top - dz))
    p.hole(s, ANGLE_SCREW_D * IN, depth=1.0 * IN, direction="reverse", name="screw holes, #10")


SHARED = [("skin_top", skin_top), ("skin_bottom", skin_bottom), ("rib", ribs), ("cross_block", cross_blocks), ("joint_block", joint_block), ("rail", rails_plain),
          ("hinge_block", hinge_blocks), ("hinge_left", hinge(False)), ("hinge_right", hinge(True))]
PANEL_A = [("stub", stubs(L))]
PANEL_B = [("stub_ground", stubs(SKIN_END))]
RAMP_PARTS = [("hinge_bolt", hinge_bolts), ("end_plate", end_plates), ("ground_angle", ground_angle)]


def sheet(t, material, size):
    """A 4 x 8 sheet, its top at z = 0, for the cut sheets to lay the
    pieces on."""
    def build(p):
        f = box(p, f"4 x 8 sheet, {material}", (0.0, size[0]), (0.0, size[1]), (-t, 0.0))
        part_is(p, f, f"4 x 8 sheet of {material}, {size[0]:g} x {size[1]:g}")
    return build


SHEETS = [("sheet_osb", sheet(SKIN, "7/16 OSB", OSB_SHEET)), ("sheet_birch", sheet(BIRCH, "3/4 birch", BIRCH_SHEET))]

# The cut list: what to cut, in inches, per the shop's stock.
def rib_lines():
    """The ribs and cross blocks by length, both panels."""
    ribs, blocks = {}, {}
    for c in RIBS:
        y0, y1 = rib_span(c)
        ribs.setdefault(y1 - y0, []).append(c)
    for a, b in zip(RIBS, RIBS[1:]):
        blocks.setdefault(b - a - RIB_W, []).append((a, b))
    out = [("rib", f"3/4 birch strip, {inch(RIB)} x {n:g}", 2 * len(cs), f"at {', '.join(f'{c:g}' for c in cs)}; " + (
        "joint block to the stubs" if n < SKIN_END - JOINT else "joint block to the skins' end") + "; glued and brad-nailed through the skins") for n, cs in sorted(ribs.items())]
    out += [("cross block", f"3/4 birch strip, {inch(RIB)} x {n:g}", 2 * len(CROSS_Y) * len(gs),
             "between the ribs at " + " and ".join(f"{a:g} and {b:g}" for a, b in gs)) for n, gs in sorted(blocks.items())]
    return out


CUTLIST = [
    ("top skin", f"{W:g} x {SKIN_END:g} x 7/16 OSB", 2, f"rip each sheet to {W:g} along its length (the strength axis, which must run along the ramp) and crosscut two {SKIN_END:g}s from the rip, a top and a bottom skin a sheet; 16 x 95-7/8 left for the shims"),
    ("bottom skin", f"{W:g} x {SKIN_END:g} x 7/16 OSB", 2, "from the same rips; the hinge-end bottom edge chamfered 3/8"),
    ("joint shim", f"{JOINT:g} x {W:g} x 7/16 OSB", 2, "from the first sheet's offcut, glued over the joint block to make up the core's 1-15/16"),
    ("stub shim", f"{STUB_W:g} x {SKIN_END - STUB_Y0:g} x 7/16 OSB", 4, "from the same offcut, glued over each stub inside the box; the tailgate stubs stay bare past the skins"),
] + rib_lines() + [
    ("joint block", f"2x4 flat x {W:g}", 2, "lumber, under its shim: the decks butt on it under load; the hinge blocks start behind it"),
    ("tailgate stub", "2x8 x 12", 2, "panel A, lumber: the end plates clamp 1-1/2 x 7-1/4; 3 in bare past the skins"),
    ("ground stub", f"2x8 x {SKIN_END - STUB_Y0:g}", 2, "panel B, lumber: flush with the skins' end; the 30 degree bevel is cut across the whole end"),
    ("rail", f"3/4 birch, {SKIN_END:g} x {inch(RAIL_H)}", 4, "two per panel, flush both sides, glued and brad-nailed to the skins' edges and the edge ribs"),
    ("hinge block", f"3/4 birch strip, {inch(RIB)} x {HINGE_BLOCK_L - JOINT:g}", 8, "two glued face to face behind each edge rib from the joint block back, so the hinge screws bite 3 in of ply"),
    ("hinge band", f"2 x 1/4 steel flat bar x {BAND_L:g}", 4, "flat along the rail's outer face from the joint back, its bottom 0.4 above the panel's bottom; six 11/32 holes at 4-1/2, 5-3/4, 7, 8-1/4, 9-3/4 and 11; on the left against the rail, on the right outside the strut"),
    ("hinge strut", f"2-1/2 x 1/4 steel flat bar x {STRUT_L:g}", 4, f"from the pivot hole up at {STRUT_ANGLE:g} degrees onto the band, lapped over it, four of the band's screws through it; a 17/32 hole 1-1/4 from one end, that end rounded; on the left outside the band, on the right against the rail"),
    ("hinge packer", f"2 x 1/4 steel flat bar x {BAND_L - PACKER_Y0:g}", 2, "under the right band's far end, where the strut is not behind it"),
    ("hinge screw", "5/16 x 3-1/8 structural screw, GRK RSS or equal", 24, "six per side through the bars, the rail, the edge rib and the hinge block; the first four through both bars"),
    ("hinge bolt", "1/2 x 1-1/2 grade 5 hex bolt, nylock nut, two 1/2 SAE washers", 2, "the pivot, one each side, through the two struts; left a hair loose; pull the two bolts and the halves separate"),
    ("ground angle", f"1/8 x 2 aluminium angle x {W:g}", 1, "one leg against panel B's flush end, the other out past it flush with the top skin"),
    ("angle screw", "#10 x 2 pan head", 4, "through the angle's leg into each stub's end grain, two per stub, one high and one low"),
]


def _v(d):
    return (d["x"], d["y"], d["z"])


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _norm(a):
    k = 1.0 / math.sqrt(_dot(a, a))
    return (a[0] * k, a[1] * k, a[2] * k)


def _rotation(rx, ry, rz):
    """A placement's rotation matrix: x, then y, then z (degrees)."""
    a, b, c = (math.radians(t) for t in (rx, ry, rz))
    ca, sa, cb, sb, cc, sc = math.cos(a), math.sin(a), math.cos(b), math.sin(b), math.cos(c), math.sin(c)
    return ((cc * cb, cc * sb * sa - sc * ca, cc * sb * ca + sc * sa), (sc * cb, sc * sb * sa + cc * ca, sc * sb * ca - cc * sa), (-sb, cb * sa, cb * ca))


def _apply(m, p):
    return tuple(_dot(row, p) for row in m)


def _frame(cyl, face):
    """A cylindrical face's connector frame as the kernel builds it (see
    docs/OPS.md): origin at the face's middle on the axis, z along the
    axis, x and y canonical for z."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    t = _dot(_sub(c, o), z)
    origin = (o[0] + z[0] * t, o[1] + z[1] * t, o[2] + z[2] * t)
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    return origin, _cross(y, z), y, z


def _placed(frame, placement):
    """The frame in the world once its body is placed."""
    (x, y, z), (rx, ry, rz) = placement
    m = _rotation(rx, ry, rz)
    o, fx, fy, fz = frame
    po = _apply(m, o)
    return (po[0] + x, po[1] + y, po[2] + z), _apply(m, fx), _apply(m, fy), _apply(m, fz)


def cylinder_frames(body, r, placement):
    """Every cylinder of radius r on a studio body, each with its placed
    frame."""
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    out = []
    for c in body["cylinders"]:
        if abs(c["radius"] - r) < 1e-6:
            out.append((c, _placed(_frame(c, faces[json.dumps(c["reference"], sort_keys=True)]), placement)))
    return out


def coaxial(a_body, ra, a_placement, b_body, rb, b_placement):
    for ca, ta in cylinder_frames(a_body, ra, a_placement):
        for cb, tb in cylinder_frames(b_body, rb, b_placement):
            d = _sub(tb[0], ta[0])
            off = _sub(d, tuple(ta[3][i] * _dot(d, ta[3]) for i in range(3)))
            if math.sqrt(_dot(_cross(ta[3], tb[3]), _cross(ta[3], tb[3]))) < 1e-9 and math.sqrt(_dot(off, off)) < 1e-6:
                return ca, cb, ta, tb
    raise RuntimeError(f"no coaxial cylinders of radii {ra} and {rb}")


def mate_parameters(target, moving):
    """offset, angle, flip that put `moving` where the mate rule puts it
    from `target`: z opposed unless flip, x turned by angle about z,
    origin offset along the target's z."""
    ot, xt, _, zt = target
    om, xm, _, zm = moving
    flip = _dot(zt, zm) > 0.0
    z = zt if flip else tuple(-c for c in zt)
    return _dot(_sub(om, ot), zt), math.degrees(math.atan2(_dot(_cross(xt, xm), z), _dot(xt, xm))), flip


def apply(mcp, ops, tab):
    text = mcp.call("apply", {"ops": ops, "tab": tab})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    return text


IDENTITY = ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0))


def instance_ops(tabs, bodies, titles, fixed, placement=IDENTITY):
    (x, y, z), (rx, ry, rz) = placement
    return [{"type": "add_instance", "studio": tabs[t], "body": k, "name": t if len(bodies[t]) == 1 else f"{t} {k + 1}", "fixed": fixed,
             "placement": {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": rx, "y": ry, "z": rz}}} for t in titles for k in range(len(bodies[t]))]


def instance_ids(mcp, tab):
    return {i["name"]: i["id"] for i in json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["instances"]}


# -- cut sheets ---------------------------------------------------------
#
# A piece is (part title, body index, x0, y0, lay): where its near
# corner goes on the sheet (inches, the sheet 96 along x) and how it
# lies, turned so its long way runs along x and its thickness up. `lay`
# says which way the thickness runs as built: "x" for the profiles
# extruded along x and the ribs, "y" for the cross blocks, "z" for the
# skins turned to run along the sheet, "flat" for the skins as built
# (their width along the sheet). The placement turns
# about x, then y, then z, then moves.

def placement_flat(body, x0, y0, lay):
    lo, hi = body["bounds"]
    X0, Y0 = x0 * IN, y0 * IN
    if lay == "x":      # about y by 90 then z by -90: x' = y, y' = -z, z' = -x
        pos, rot = (X0 - lo["y"], Y0 + hi["z"], hi["x"]), (0.0, 90.0, -90.0)
    elif lay == "y":    # about x by -90: x' = x, y' = z, z' = -y
        pos, rot = (X0 - lo["x"], Y0 - lo["z"], hi["y"]), (-90.0, 0.0, 0.0)
    elif lay == "z":    # about z by -90: x' = y, y' = -x
        pos, rot = (X0 - lo["y"], Y0 + hi["x"], -lo["z"]), (0.0, 0.0, -90.0)
    else:               # "flat": as built, x along x
        pos, rot = (X0 - lo["x"], Y0 - lo["y"], -lo["z"]), (0.0, 0.0, 0.0)
    return {"position": dict(zip("xyz", pos)), "rotation": dict(zip("xyz", rot))}


def flat_size(body, lay):
    """The piece's footprint on the sheet (along x, across y), inches."""
    lo, hi = body["bounds"]
    d = {k: (hi[k] - lo[k]) / IN for k in "xyz"}
    return {"x": (d["y"], d["z"]), "y": (d["x"], d["z"]), "z": (d["y"], d["x"]), "flat": (d["x"], d["y"])}[lay]


def nest_osb(shims):
    """An OSB sheet ripped 31-7/8 along its length, the strength axis,
    and crosscut into a top and a bottom skin; on the first sheet the
    offcut strip also gives the two joint shims and the four stub
    shims."""
    pieces = [("skin_top", 0, 0.0, 0.0, "z"), ("skin_bottom", 0, SKIN_END + KERF, 0.0, "z")]
    if shims:
        y = W + KERF
        for _ in range(2):
            pieces.append(("joint_block", 1, 0.0, y, "flat"))
            y += JOINT + KERF
        for n, (part, k) in enumerate((("stub", 2), ("stub", 3), ("stub_ground", 2), ("stub_ground", 3))):
            pieces.append((part, k, n * (SKIN_END - STUB_Y0 + KERF), y, "z"))
    return pieces


def nest_birch(reports):
    """Rips across the birch: the four rails in two rips, then 1-15/16
    strips for the ribs, cross blocks and hinge blocks, first-fit by
    length."""
    pieces, y = [], 0.0
    def rip(items):
        nonlocal y
        width, x = 0.0, 0.0
        for part, k, lay in items:
            along, across = flat_size(reports[part][k], lay)
            pieces.append((part, k, x, y, lay))
            width, x = max(width, across), x + along + KERF
        assert x - KERF <= BIRCH_SHEET[0] + 1e-9, f"rip of {items[0][0]} runs {x - KERF:g} in"
        y += width + KERF
    rip([("rail", 0, "x"), ("rail", 1, "x")])
    rip([("rail", 0, "x"), ("rail", 1, "x")])

    # Strips: every rib and cross block body, twice (one per panel).
    strips = ([("rib", k, "x") for k in range(len(reports["rib"]))] + [("cross_block", k, "y") for k in range(len(reports["cross_block"]))]
              + [("hinge_block", k, "x") for k in range(len(reports["hinge_block"]))]) * 2
    strips.sort(key=lambda it: -flat_size(reports[it[0]][it[1]], it[2])[0])
    rows = []
    for it in strips:
        along = flat_size(reports[it[0]][it[1]], it[2])[0]
        for row in rows:
            if row[0] + KERF + along <= BIRCH_SHEET[0]:
                row[0] += KERF + along
                row[1].append(it)
                break
        else:
            rows.append([along, [it]])
    for _, items in rows:
        rip(items)
    assert y - KERF <= BIRCH_SHEET[1], f"the birch nest is {y - KERF:g} in across"
    return pieces


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "ramp.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "ramp"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Folding torsion-box truck ramp"}]})
    tabs, bodies, reports = {}, {}, {}
    for i, (title, fn) in enumerate(SHARED + PANEL_A + PANEL_B + RAMP_PARTS + SHEETS):
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]}); tab = 1
        else:
            text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": title}]})
            tab = int(re.search(r"tab (\d+)", text).group(1))
        tabs[title] = tab
        part = Part(mcp, tab, title)
        # The tunable numbers, as document variables: the skin and rib
        # extrudes are bound to them; the sketched profiles carry their
        # values and are redrawn by this script.
        apply(mcp, [{"type": "add_variable", "name": n, "expression": f"{v * IN:g}"} for n, v in
                    (("skin", SKIN), ("rib", RIB), ("curb", CURB), ("pin_drop", PIN_DROP))], tab)
        fn(part)
        bodies[title] = part.bodies()
        reports[title] = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))["bodies"]
        print(f"{title:<18} tab {tab:>2}: {len(bodies[title])} bodies")
    # The two panels, every part fixed in place in the panel's frame.
    panels = {}
    for name, parts in (("panel A", SHARED + PANEL_A), ("panel B", SHARED + PANEL_B)):
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": name}]})
        panels[name] = int(re.search(r"tab (\d+)", text).group(1))
        apply(mcp, instance_ops(tabs, bodies, [t for t, _ in parts], True), panels[name])
        print(f"{name:<18} tab {panels[name]:>2}: {len(instance_ids(mcp, panels[name]))} instances")
    b_ids = instance_ids(mcp, panels["panel B"])
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "ramp"}]})
    ramp = int(re.search(r"tab (\d+)", text).group(1))
    ops = [{"type": "add_instance", "studio": panels["panel A"], "body": 0, "name": "panel A", "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}},
           {"type": "add_instance", "studio": panels["panel B"], "body": 0, "name": "panel B", "fixed": False,
            "placement": {"position": dict(zip("xyz", PLACE_B[0])), "rotation": dict(zip("xyz", PLACE_B[1]))}}]
    ops += instance_ops(tabs, bodies, [t for t, _ in RAMP_PARTS], True)
    apply(mcp, ops, ramp)
    ids = instance_ids(mcp, ramp)
    # The fold: a revolute between the left bolt and panel B's right
    # lug, which turned lands on the left beside A's.
    ca, cb, ta, tb = coaxial(reports["hinge_bolt"][0], BOLT_D / 2 * IN, IDENTITY, reports["hinge_right"][1], HOLE_D / 2 * IN, PLACE_B)
    offset, angle, flip = mate_parameters(ta, tb)
    apply(mcp, [{"type": "add_mate", "kind": "revolute", "a": {"instance": ids["hinge_bolt 1"], "face": ca["reference"]},
                 "b": {"instance": ids["panel B"], "sub": b_ids["hinge_right 2"], "face": cb["reference"]},
                 "offset": offset, "angle": angle, "flip": flip, "name": "fold"}], ramp)
    report = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
    for m in report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")
    for i in report["instances"]:
        if i.get("error"):
            raise RuntimeError(f"instance {i['name']}: {i['error']}")
    pb = next(i for i in report["instances"] if i["name"] == "panel B")["placed"]
    drift = max(abs(_v(pb["position"])[k] - PLACE_B[0][k]) for k in range(3))
    fold = next(m for m in report["mates"] if m["name"] == "fold")
    print(f"ramp tab {ramp}: {len(ids)} instances, the fold resolved to the drawn pose (worst {drift:.1e} mm)")

    def low_b(angle_):
        apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": angle_}], ramp)
        rep = json.loads(mcp.call("report", {"tab": ramp, "detail": "full"}))
        return min(b["bounds"][0]["z"] for b in rep["bodies"] if b["name"].startswith("panel B /"))

    # Which way folds under: panel B swings down and under panel A,
    # bottoms together, so its free end drops.
    sign = 1.0 if low_b(fold["angle"] + 30.0) < low_b(fold["angle"]) - 1.0 else -1.0
    at = lambda deg: fold["angle"] + sign * deg

    for view, name in (("iso", "ramp_iso"), ("-0.6,-0.7,0.4", "ramp_front"), ("right", "ramp_side")):
        mcp.call("screenshot", {"tab": ramp, "view": view, "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{name}.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "section": f"x:{HINGE_CUT * IN}:flip", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_lug_section.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "-0.5,-0.6,-0.6", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_below.png")})
    # The ground angle on panel B's end: the leg on the stubs' end faces
    # with its screw holes, the lip out flush with their top.
    window = ",".join(f"{v * IN:g}" for v in (-2.0, -SKIN_END - 4.0, -1.0, W + 2.0, -SKIN_END + 6.0, 3.0))
    mcp.call("screenshot", {"tab": ramp, "view": "-0.5,-0.8,0.5", "fit": window, "width": 1400, "height": 900, "path": os.path.join(OUT, "ramp_angle.png")})
    for name, stem in (("panel A", "panel_a"), ("panel B", "panel_b")):
        mcp.call("screenshot", {"tab": panels[name], "view": "0.5,-0.7,-0.5", "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{stem}_below.png")})
        # The box from below with the bottom skin cut away: the lug feet
        # against their ribs, the joint block, the top skin intact above.
        mcp.call("screenshot", {"tab": panels[name], "view": "0.3,-0.4,-0.85", "section": f"z:{(SKIN + 0.05) * IN}", "width": 1600, "height": 1000, "path": os.path.join(OUT, f"{stem}_cutaway.png")})
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(180.0)}], ramp)
    mcp.call("screenshot", {"tab": ramp, "view": "iso", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded.png")})
    mcp.call("screenshot", {"tab": ramp, "view": "right", "width": 1600, "height": 1000, "path": os.path.join(OUT, "ramp_folded_side.png")})
    # The hinge at 90 degrees, cut through panel A's left strut and
    # fitted to the hinge, not the panels: the strut hatched, the bolt
    # through it, the band and panel B's strut behind it.
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(90.0)}], ramp)
    window = ",".join(f"{v * IN:g}" for v in (HINGE_CUT - 3.0, -4.0, -9.0, HINGE_CUT + 2.0, BAND_L + 2.0, 4.0))
    mcp.call("screenshot", {"tab": ramp, "view": "right", "section": f"x:{HINGE_CUT * IN}:flip", "fit": window, "width": 1400, "height": 1200,
                            "path": os.path.join(OUT, "ramp_hinge_section.png")})
    # The same cut as a sheet with hidden lines (the screws in the rail
    # and block, panel B's strut where it passes behind), and each
    # side's bars drawn by themselves beneath it, in the same pose.
    print(mcp.call("export", {"tab": ramp, "format": "pdf", "sheet": "A3", "hidden": True, "window": window, "parts": False,
                              "views": [f"section-side@{HINGE_CUT * IN:g}", "right of panel A / hinge_left", "right of panel B / hinge_right"],
                              "note": f"folded 90 degrees, cut through panel A's left strut at {HINGE_CUT:g} in; hidden lines dashed; each side's band, strut and screws alone below",
                              "path": os.path.join(OUT, "hinge_section.pdf")}))
    apply(mcp, [{"type": "set_mate", "id": fold["id"], "angle": at(0.0)}], ramp)
    # Sheets: each panel with a section across the width at mid-length
    # and one along the length through the hinge block; the ramp with
    # balloons; the ground stub with its bevel on.
    for name, stem, lug in (("panel A", "panel_a", RIB_W + BIRCH), ("panel B", "panel_b", RIB_W + BIRCH)):
        print(mcp.call("export", {"tab": panels[name], "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", f"section@{SKIN_END / 2 * IN}", f"section-side@{lug * IN}"],
                                  "note": f"{name}: {W:g} x 48 x {inch(BOX)} torsion box, 7/16 OSB on 3/4 birch, rails 3/4 birch, hinge bars 1/4 steel", "path": os.path.join(OUT, f"{stem}.pdf")}))
    print(mcp.call("export", {"tab": ramp, "format": "pdf", "sheet": "A3", "note": "two panels on two 1/2 bolts, open; pull the bolts to separate them", "path": os.path.join(OUT, "ramp.pdf")}))
    stub_report = json.loads(mcp.call("report", {"tab": tabs["stub_ground"], "detail": "full"}))
    bevel = next(f["id"] for f in stub_report["features"] if f["name"].startswith("ground bevel"))
    apply(mcp, [{"type": "set_suppressed", "id": bevel, "suppressed": False}], tabs["stub_ground"])
    print(mcp.call("export", {"tab": tabs["stub_ground"], "format": "pdf", "sheet": "A4", "note": "the ground panel's stubs, flush with the skins, beveled 30 degrees", "path": os.path.join(OUT, "stub.pdf")}))
    apply(mcp, [{"type": "set_suppressed", "id": bevel, "suppressed": True}], tabs["stub_ground"])
    # The fold from the side, 0 to 180 degrees.
    positions = [{"fold": at(d), "label": f"{d:g} degrees" if d else "open"} for d in (0.0, 45.0, 90.0, 135.0, 180.0)]
    print(mcp.call("range_of_motion", {"tab": ramp, "positions": positions, "view": "right", "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "fold.pdf")}))
    mcp.call("range_of_motion", {"tab": ramp, "positions": positions, "view": "right", "width": 2000, "height": 500, "path": os.path.join(OUT, "fold.png")})
    # Cut sheets: the plywood pieces, the real bodies, laid flat on a
    # 4 x 8 with a kerf between, each sheet an assembly drawn from the
    # top with its pieces numbered.
    for name, stem, title, t, size, pieces in (("cut sheet, OSB 1", "cut_sheet_osb", "sheet_osb", SKIN, OSB_SHEET, nest_osb(True)),
                                               ("cut sheet, OSB 2", "cut_sheet_osb_2", "sheet_osb", SKIN, OSB_SHEET, nest_osb(False)),
                                               ("cut sheet, birch", "cut_sheet_birch", "sheet_birch", BIRCH, BIRCH_SHEET, nest_birch(reports))):
        text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": name}]})
        tab = int(re.search(r"tab (\d+)", text).group(1))
        ops = instance_ops(tabs, bodies, [title], True)
        for n, (part, k, x0, y0, lay) in enumerate(pieces):
            ops.append({"type": "add_instance", "studio": tabs[part], "body": k, "name": f"{part} {k + 1} #{n + 1}", "fixed": True,
                        "placement": placement_flat(reports[part][k], x0, y0, lay)})
        apply(mcp, ops, tab)
        r = json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))
        for b in r["bodies"]:
            if b["name"].startswith(title):
                continue
            lo, hi = b["bounds"]
            assert -1e-6 <= lo["z"] and hi["z"] <= t * IN + 1e-6, f"{b['name']} is not flat on the sheet: z {lo['z']:.2f}..{hi['z']:.2f}"
            assert -1e-6 <= lo["x"] and hi["x"] <= size[0] * IN + 1e-6 and -1e-6 <= lo["y"] and hi["y"] <= size[1] * IN + 1e-6, f"{b['name']} is off the sheet"
        print(f"{name:<18} tab {tab:>2}: {len(pieces)} pieces")
        print(mcp.call("export", {"tab": tab, "format": "pdf", "sheet": "A3", "views": ["top"], "hidden": False,
                                  "note": f"{size[0]:g} x {size[1]:g} of {t:g} in ply, long way along x; {KERF:g} in kerf between pieces; numbered as listed", "path": os.path.join(OUT, f"{stem}.pdf")}))
    with open(os.path.join(OUT, "cutlist.csv"), "w") as f:
        f.write("item,size,qty,note\n")
        for item, size, qty, note in CUTLIST:
            f.write(f'"{item}","{size}",{qty},"{note}"\n')
    print(f"cut list: {sum(q for _, _, q, _ in CUTLIST)} pieces in {len(CUTLIST)} lines")
    mcp.close()


if __name__ == "__main__":
    main()
