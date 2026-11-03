#!/usr/bin/env python3
"""A leaf blower adapter for an LG dryer's lint duct, built through the
MCP server. The starting point is Xypher2000's Samsung adapter on
Thingiverse (thing 7021003): a tapered socket the blower's nozzle jams
into, a thin-walled wedge, and a half-moon end the size of the Samsung
filter that goes down the filter's slot. That end is the part that
matters: its full plate blanks the housing's grille, so the air has no
way back into the drum and goes down the duct, out through the dryer's
own blower and vent. An LG filter (ADQ56656401) is a near-square frame
18 mm thick standing in a straight slot at the bottom of the door
opening, on a guide at the bottom, its top edge an arc flush with the
lip. So the end is a blade the frame's width and thickness less
clearance, deep enough to blank the grille, a closed box open only at
its top into a sleeve under the wedge and at its bottom edge, below
the grille, so it goes in either way round; above the wedge a tapered
round spigot the owner's leaf vac hose jams onto, in place of the
Samsung part's funnel for the blower's nozzle. The frame's numbers
come off a photograph of it on the 2x2 measuring field, a rule and a
caliper; the README says which.

    cargo build --release -p ok-mcp
    python3 examples/lint-blower/build.py
    cargo test -p ok-render --test lint_blower    # what CI runs

Writes out/lint_blower.okpart; out/adapter.stl and out/blade.stl (two
pieces, for a bed over 320); out/middle.stl, out/left_wing.stl,
out/right_wing.stl, out/left_blade.stl and out/right_blade.stl (the
same in five for a 256 bed); pictures, a sheet and out/adapter.json.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")
# -- the LG filter, mm: measured, see the README ---------------------------
FRAME_T = 18.0          # the frame's thickness, the same everywhere (caliper)
FRAME_W = 285.0         # the frame's width, rim to rim, down its straight sides: the fit test's two slices at 284 together were a little wider than the frame, so a millimetre off the 286 a rule gave (the field photo read 288 to 294, the camera's parallax on an 18 mm frame)
EARS_W = 306.0          # across the ears at the top corners (the field photo; LG publishes 11-15/16 in)
FRAME_H = 287.0         # the bottom edge to the ears' tops (the field photo; parallax adds a few mm)
LIP_DIP = 47.0          # the top edge's arc, flush with the lip, dips this far below the ears at the centre (R 218)
GRILLE_D = 152.0        # how far below the lip's ends the grille's last holes reach: 6 in by the owner's eye, 130 to 135 on the cover guide's photo
# -- the blade: the dummy filter -------------------------------------------
CLEAR_W = 1.0           # the blade off the frame's width, each side
CLEAR_T = 0.5           # and off its thickness, each face
BLADE_D = 180.0         # the blade below the ears' level: the grille and 40 to spare, well above the frame's corners
BLADE_R = 0.0           # its bottom corners: square, since a round's underside overhangs when the blade prints standing
RIBS = 2                # ribs inside, face to face, down the blade
WALL = 2.0              # every wall, as the Samsung part's
# -- the sleeve under the wedge, and the wedge ---------------------------------
SLEEVE_D = 20.0         # how far the blade goes up into the sleeve
SLEEVE_FIT = 0.2        # the sleeve off the blade, each side, for the glue
FLANGE_T = 3.0          # the flange that rests on the lip's ends, where the filter's ears do
FLANGE_OUT = 8.0        # how far it reaches past the sleeve, at the front and the ends; the back stays flush
LOFT_H = 100.0          # the wedge, sleeve to a square the spigot's size: long enough that its ends lean under 45°
LOFT2_H = 30.0          # the square to the spigot's circle, above it: the corners come in 21 over this
# -- the spigot the leaf vac hose jams onto --------------------------------
# The hose is a wire-coil corrugated tube, 100 inside, corrugated to its
# end, so there is no cuff to grip a bead: the spigot tapers, a little
# under the hose at the tip to start it and a little over at the foot,
# and the hose screws on a few ribs and wedges tight against the shoulder.
HOSE_ID = 100.0         # the hose's inside diameter (a rule across the end)
SPIGOT_TIP = HOSE_ID - 1.0    # the spigot at its tip: starts in the hose freely
SPIGOT_FOOT = HOSE_ID + 2.0   # and at its foot: the hose wedges before it gets here
SPIGOT_H = 50.0         # the spigot's height: four or so ribs of hose on it
SPIGOT_WALL = 2.0       # the tube's wall at the tip; thicker toward the foot, the bore being straight
# -- pieces for a 256 mm bed (a Bambu P1S) ----------------------------------
X_CUT = SPIGOT_FOOT / 2 + WALL   # the wedge's cuts, at the square's sides under the spigot
TAB_T = 2.0             # a tab's thickness, the wall's
TAB_IN, TAB_OUT = 10.0, 10.0   # how far a tab runs back into the middle piece and out into the wing
TAB_CLEAR = 0.2         # the tab off the wing's inner wall, for the glue
LAP_W, LAP_T = 8.0, 1.2    # the blade halves' lap strip along their joint, on the right half
FIT_TEST_H, FIT_GAP = 20.0, 12.0   # the fit test: the halves' bottom slices, standing this far apart
FIT_SHIFT = 144.0                   # the left slice moved along to lie over the right one, so the file fits a 256 mm plate
RIGHT_HALF_W = 144.0       # the right half as printed, joint to outer rail: the left half makes up the rest of BLADE_W, so a
                           # narrower frame than measured is met by reprinting the left half alone, the joint off centre

BLADE_T = FRAME_T - 2 * CLEAR_T
BLADE_W = FRAME_W - 2 * CLEAR_W
BLADE_X1 = RIGHT_HALF_W            # the blade's outer rails: the right one where the printed half puts it
BLADE_X0 = RIGHT_HALF_W - BLADE_W
# The sleeve sits round the blade, so it is off centre by as much as the blade is.
BORE_X0, BORE_X1 = BLADE_X0 - SLEEVE_FIT, BLADE_X1 + SLEEVE_FIT
STUB_X0, STUB_X1 = BORE_X0 - WALL, BORE_X1 + WALL
STUB_T = BLADE_T + 2 * SLEEVE_FIT + 2 * WALL   # the sleeve outside: it is the stub the wedge starts from
STUB_L = BLADE_W + 2 * SLEEVE_FIT + 2 * WALL
STUB_D = SLEEVE_D
BORE_T = BLADE_T + 2 * SLEEVE_FIT              # the sleeve inside
BORE_L = BLADE_W + 2 * SLEEVE_FIT
# The lip is the datum, z = 0: the flange rests on it, the sleeve stands
# on it (it is thicker than the slot) and the wedge starts on the sleeve.
Z_LOFT = STUB_D
Z_SQUARE = Z_LOFT + LOFT_H        # the wedge ends in a square the spigot's size
Z_SPIGOT = Z_SQUARE + LOFT2_H     # which rounds to the spigot's circle
Z_TOP = Z_SPIGOT + SPIGOT_H
R_OUT = SPIGOT_FOOT / 2 + WALL    # the square's half side and the circle's radius: a wall outside the spigot's foot
R_BORE = SPIGOT_TIP / 2 - SPIGOT_WALL
SPIGOT_TAPER = math.degrees(math.atan((SPIGOT_FOOT - SPIGOT_TIP) / 2 / SPIGOT_H))
AXIS_Y = R_OUT                    # the spigot's axis: the square and the circle sit on the back face
# The wedge's walls lean, so the in-plane inset that leaves WALL through
# them is WALL over the cosine of the lean, taken at the front and the ends.
LEAN_FRONT = math.atan((2 * R_OUT - STUB_T) / LOFT_H)
LEAN_END = math.atan((STUB_L / 2 - R_OUT) / LOFT_H)
IN_FRONT = WALL / math.cos(LEAN_FRONT)
IN_END = WALL / math.cos(LEAN_END)
# The blade reaches up the sleeve to just under the wedge and BLADE_D
# below the lip.
Z_BLADE_TOP = SLEEVE_D - 1.0
Z_BLADE_BOT = -BLADE_D
# The tabs: from the sleeve's foot, on the bed when the piece prints, to
# inner end wall comes in to X_CUT + TAB_OUT.
TAB_Z0 = 0.0
X_IN0, X_IN1 = STUB_L / 2 - IN_END, R_OUT - IN_END
TAB_Z1 = Z_LOFT + (X_IN0 - (X_CUT + TAB_OUT + 1.0)) / (X_IN0 - X_IN1) * LOFT_H - 8.0
Y_IN0 = STUB_T - WALL            # the sleeve bore's front (inside the back wall at WALL)
Y_IN_LOFT0 = STUB_T - IN_FRONT   # the wedge bore's front where it starts, a little inside that
Y_IN_LOFT1 = 2 * R_OUT - IN_FRONT
Z_STEP = Z_LOFT + 0.01           # the sleeve bore ends here and the wedge bore takes over


def y_front_in(z):
    """The duct's inner front face at height z on the cut planes: the
    sleeve bore's up to Z_STEP, the wedge bore's above."""
    if z <= Z_STEP:
        return Y_IN0
    return Y_IN_LOFT0 + (z - Z_LOFT) / LOFT_H * (Y_IN_LOFT1 - Y_IN_LOFT0)


def checked(part):
    bodies = part.bodies()
    print(f"{part.name}: {len(bodies)} bodies", ", ".join(f"{n or 'body'} {v / 1000:.1f} cm3" for n, v in bodies))
    return bodies


def build_adapter(p):
    """The sleeve, the flange, the wedge and the spigot as one solid,
    then the duct through them. The back face, y = 0, is the drum side;
    the spigot's circle is tangent to it."""
    s = p.sketch("top", 0.0, "sleeve")
    p.rect(s, (STUB_X0, 0.0), (STUB_X1, STUB_T))
    p.extrude(s, STUB_D, name="sleeve")
    s = p.sketch("top", 0.0, "flange")
    p.rect(s, (STUB_X0 - FLANGE_OUT, 0.0), (STUB_X1 + FLANGE_OUT, STUB_T + FLANGE_OUT))
    p.extrude(s, FLANGE_T, op="add", name="flange")
    a = p.sketch("top", Z_LOFT, "wedge at the sleeve")
    p.rect(a, (STUB_X0, 0.0), (STUB_X1, STUB_T))
    b = p.sketch("top", Z_SQUARE, "wedge at the square")
    p.rect(b, (-R_OUT, 0.0), (R_OUT, 2 * R_OUT))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "add", "name": "wedge"})
    c = p.sketch("top", Z_SPIGOT, "round at the spigot")
    p.circle(c, (0.0, AXIS_Y), R_OUT)
    p.feature({"type": "add_loft", "sketch": b, "sketch_b": c, "op": "add", "name": "square to round"})
    # The spigot: its half profile on the plane through the axis, turned
    # about it, so the outside is a true cone from the foot to the tip.
    s = p.sketch("front", -AXIS_Y, "spigot")
    p.polygon(s, [(0.0, Z_SPIGOT), (SPIGOT_FOOT / 2, Z_SPIGOT), (SPIGOT_TIP / 2, Z_TOP), (0.0, Z_TOP)])
    p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "add", "name": "spigot"})
    # The duct: the sleeve's bore up through the flange, the wedge's bore
    # lofted to the spigot's, and the spigot's bore out the top.
    s = p.sketch("top", -1.0, "sleeve bore")
    p.rect(s, (BORE_X0, WALL), (BORE_X1, WALL + BORE_T))
    p.cut(s, 1.0 + STUB_D + 0.01, name="sleeve bore")
    a = p.sketch("top", Z_LOFT, "wedge bore at the sleeve")
    p.rect(a, (STUB_X0 + IN_END, WALL), (STUB_X1 - IN_END, STUB_T - IN_FRONT))
    b = p.sketch("top", Z_SQUARE + 0.01, "wedge bore at the square")
    p.rect(b, (-R_OUT + IN_END, WALL), (R_OUT - IN_END, 2 * R_OUT - IN_FRONT))
    p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "remove", "name": "wedge bore"})
    # A rectangle lofted to a circle has to share its centre, or the loft
    # does not close (docs/ROADMAP.md), so this one is a wall inside the
    # square all round, centred on the spigot's axis, a hair inside the
    # wedge bore's top so no face of it lies in a face of that void.
    b2 = p.sketch("top", Z_SQUARE - 0.01, "round bore at the square")
    p.rect(b2, (-R_OUT + WALL + 0.02, WALL + 0.02), (R_OUT - WALL - 0.02, 2 * R_OUT - WALL - 0.02))
    c = p.sketch("top", Z_SPIGOT + 0.01, "round bore at the spigot")
    p.circle(c, (0.0, AXIS_Y), R_BORE)
    p.feature({"type": "add_loft", "sketch": b2, "sketch_b": c, "op": "remove", "name": "round bore"})
    s = p.sketch("top", Z_SPIGOT - 0.01, "spigot bore")
    p.circle(s, (0.0, AXIS_Y), R_BORE)
    p.cut(s, SPIGOT_H + 1.0, name="spigot bore")


def build_tabs(p, side):
    """The two tabs on one end of the middle piece: a strip 2 thick down
    the back wall and one down the front, each run TAB_IN into the
    piece, fused into its wall, and TAB_OUT past the cut a glue gap
    inside the wing's wall."""
    x_cut = side * X_CUT
    for name, x0, depth, c in (("in", x_cut - side * TAB_IN, TAB_IN, -1.0), ("out", x_cut, TAB_OUT, TAB_CLEAR)):
        direction = "normal" if side > 0 else "reverse"
        s = p.sketch("right", x0, f"back tab {name} {side:+d}")
        # Inside the piece the strip overlaps the wall by a millimetre (c = -1);
        # past the cut it stands the clearance off it. Its inner face is one plane.
        p.rect(s, (WALL + c, TAB_Z0), (WALL + TAB_CLEAR + TAB_T, TAB_Z1))
        p.extrude(s, depth, direction=direction, op="add", name=f"back tab {name} {side:+d}")
        s = p.sketch("right", x0, f"front tab {name} {side:+d}")
        # The outline follows the inner face, the step at Z_STEP included,
        # c off it; the inner edge is TAB_CLEAR + TAB_T inside the face.
        face = [(Y_IN0, TAB_Z0), (Y_IN0, Z_STEP), (y_front_in(Z_STEP + 1e-9), Z_STEP), (y_front_in(TAB_Z1), TAB_Z1)]
        outer = [(y - c, z) for y, z in face]
        inner = [(y - TAB_CLEAR - TAB_T, z) for y, z in reversed(face)]
        p.polygon(s, outer + inner)
        p.extrude(s, depth, direction=direction, op="add", name=f"front tab {name} {side:+d}")


def blade_outline(inset=0.0, x_shift=0.0):
    """The blade's outline in the front plane's coordinates (x, z):
    straight sides and top, the bottom corners rounded, between
    BLADE_X0 and BLADE_X1, moved along by `x_shift`."""
    x0, x1, r = BLADE_X0 + inset + x_shift, BLADE_X1 - inset + x_shift, max(BLADE_R - inset, 0.0)
    z0, z1 = Z_BLADE_BOT + inset, Z_BLADE_TOP - inset
    if r <= 0.0:
        return [(x0, z1), (x1, z1), (x1, z0), (x0, z0)]
    pts = [(x0, z1), (x1, z1), (x1, z0 + r)]
    for k in range(1, 7):
        a = -k / 6 * math.pi / 2
        pts.append((x1 - r + r * math.cos(a), z0 + r + r * math.sin(a)))
    for k in range(0, 7):
        a = -math.pi / 2 - k / 6 * math.pi / 2
        pts.append((x0 + r + r * math.cos(a), z0 + r + r * math.sin(a)))
    return pts


def build_blade(p, y_shift=0.0, x_shift=0.0):
    """The dummy filter: a slab the blade's outline hollowed between its
    faces, open at the top edge, where the wedge's air comes in, and at
    the bottom edge, where it leaves below the grille; both faces
    solid, so either face can be the drum side; ribs face to face. It
    reaches up the sleeve to just under the wedge. `y_shift` and
    `x_shift` build it that much further forward and along, for a
    second one beside the first."""
    y0 = WALL + SLEEVE_FIT + y_shift
    s = p.sketch("front", -y0, "blade")
    p.polygon(s, blade_outline(0.0, x_shift))
    p.extrude(s, BLADE_T, direction="reverse", name="blade")   # the front plane's normal is -Y: reverse goes +Y
    # The hollow follows the outline a wall inside it, out through the
    # top, and the bottom opens along the straight edge between the
    # corners, so the rails run round the corners.
    s = p.sketch("front", -(y0 + WALL), "blade hollow")
    pts = blade_outline(WALL, x_shift)
    p.polygon(s, [(x, z + (WALL + 1.0 if z > Z_BLADE_TOP - WALL - 0.01 else 0.0)) for x, z in pts])
    p.extrude(s, BLADE_T - 2 * WALL, direction="reverse", op="remove", name="blade hollow")
    s = p.sketch("front", -(y0 + WALL), "blade mouth")
    m = max(BLADE_R, WALL) + 1.0
    p.rect(s, (BLADE_X0 + m + x_shift, Z_BLADE_BOT - 1.0), (BLADE_X1 - m + x_shift, Z_BLADE_BOT + WALL + 0.5))
    p.extrude(s, BLADE_T - 2 * WALL, direction="reverse", op="remove", name="blade mouth")
    for k in range(RIBS):
        x = BLADE_X0 + (k + 1) * BLADE_W / (RIBS + 1) + x_shift
        s = p.sketch("front", -(y0 + WALL), f"rib {k + 1}")
        # The ribs stop under the sleeve, which holds the top on its own.
        p.rect(s, (x - WALL / 2, Z_BLADE_BOT), (x + WALL / 2, -2.0))
        p.extrude(s, BLADE_T - 2 * WALL, direction="reverse", op="add", name=f"rib {k + 1}")


def build_laps(p, y0):
    """The laps on the right half: a strip inside each face, reaching
    LAP_W past the joint under the left half's plate, standing on the
    right's plate and rooted a millimetre into it over the joint only,
    so the halves glue face to face along the joint. They run from the
    bottom edge, so they print from the bed, not as cantilevers."""
    for name, y, root in (("drum", y0 + WALL, -1.0), ("duct", y0 + BLADE_T - WALL - LAP_T, LAP_T)):
        s = p.sketch("front", -y, f"lap {name}")
        p.rect(s, (-LAP_W, Z_BLADE_BOT), (1.0, Z_BLADE_TOP - 5.0))
        p.extrude(s, LAP_T, direction="reverse", op="add", name=f"lap {name}")
        s = p.sketch("front", -(y + root), f"lap {name} root")
        p.rect(s, (0.0, Z_BLADE_BOT), (1.0, Z_BLADE_TOP - 5.0))
        p.extrude(s, 1.0, direction="reverse", op="add", name=f"lap {name} root")


def build_fit_test(p):
    """The bottom FIT_TEST_H of both halves, the left one built FIT_GAP
    further forward and moved along to lie over the right, so the two
    stand apart on a 256 mm plate: slid together on the laps they try
    the slot's width and thickness in a short print."""
    big = 400.0
    y1 = WALL + SLEEVE_FIT + BLADE_T + 1.0
    build_blade(p)
    s = p.sketch("top", -big, "keep the right half")
    p.rect(s, (0.0, -1.0), (big, y1))
    p.extrude(s, 2 * big, op="intersect", name="keep the right half")
    build_laps(p, WALL + SLEEVE_FIT)
    s = p.sketch("top", -big, "keep the right slice")
    p.rect(s, (-LAP_W - 1.0, -1.0), (big, y1))
    p.extrude(s, big + Z_BLADE_BOT + FIT_TEST_H, op="intersect", name="keep the right slice")
    dy = BLADE_T + FIT_GAP
    build_blade(p, dy, FIT_SHIFT)
    s = p.sketch("top", -big, "keep the left slice")
    p.rect(s, (-big, dy + 1.0), (FIT_SHIFT, dy + WALL + SLEEVE_FIT + BLADE_T + 1.0))
    p.extrude(s, big + Z_BLADE_BOT + FIT_TEST_H, op="intersect", name="keep the left slice")


def build_piece(p, which):
    """The adapter or the blade again, kept only between the cuts: the
    middle with the socket and the tabs, a wing, or half the blade,
    the right half with the lap strip its mate glues to."""
    big = 400.0
    if which in ("middle", "right wing", "left wing"):
        build_adapter(p)
        x0, x1 = {"middle": (-X_CUT, X_CUT), "right wing": (X_CUT, big), "left wing": (-big, -X_CUT)}[which]
    else:
        build_blade(p)
        x0, x1 = {"right blade": (0.0, big), "left blade": (-big, 0.0)}[which]
    s = p.sketch("top", -big, f"keep {which}")
    p.rect(s, (x0, -big), (x1, big))
    p.extrude(s, 2 * big, op="intersect", name=f"keep {which}")
    if which == "middle":
        build_tabs(p, 1)
        build_tabs(p, -1)
    if which == "right blade":
        build_laps(p, WALL + SLEEVE_FIT)


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
    blade_tab = new_tab(mcp, {"type": "add_part_studio", "name": "blade"})
    blade = Part(mcp, blade_tab, "blade")
    build_blade(blade)
    blade_bodies = checked(blade)
    assert len(blade_bodies) == 1, blade_bodies
    # Both together, as they go into the dryer: the blade up the sleeve.
    both_tab = new_tab(mcp, {"type": "add_part_studio", "name": "adapter with blade"})
    both = Part(mcp, both_tab, "adapter with blade")
    build_adapter(both)
    build_blade(both)
    assert len(checked(both)) == 2
    fit_tab = new_tab(mcp, {"type": "add_part_studio", "name": "fit test"})
    fit = Part(mcp, fit_tab, "fit test")
    build_fit_test(fit)
    assert len(checked(fit)) == 2
    pieces = {}
    for which in ("middle", "right wing", "left wing", "right blade", "left blade"):
        tab = new_tab(mcp, {"type": "add_part_studio", "name": which})
        piece = Part(mcp, tab, which)
        build_piece(piece, which)
        got = checked(piece)
        assert len(got) == 1, got
        pieces[which] = (tab, got[0][1])

    shot = lambda tab, name, **kw: mcp.call("screenshot", {"tab": tab, "width": 1400, "height": 1000, "path": os.path.join(OUT, f"{name}.png"), **kw})
    shot(both_tab, "adapter_iso", view="0.5,-1,0.35")
    shot(both_tab, "adapter_duct_side", view="-0.5,1,0.35")
    shot(both_tab, "adapter_front", view="front")
    shot(both_tab, "adapter_section", view="right", section="x:0")
    shot(1, "adapter_section_iso", view="-1,-1,0.8", section=f"y:{AXIS_Y:g}")
    shot(pieces["middle"][0], "middle_iso", view="1,-0.8,0.5")
    shot(pieces["right wing"][0], "right_wing_iso", view="-1,-0.8,0.5")
    shot(pieces["right blade"][0], "right_blade_iso", view="-0.6,1,0.5")
    shot(fit_tab, "fit_test", view="-0.6,-1,0.7")
    print(mcp.call("export", {"tab": fit_tab, "format": "stl", "path": os.path.join(OUT, "fit_test.stl")}))
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "adapter.stl")}))
    print(mcp.call("export", {"tab": blade_tab, "format": "stl", "path": os.path.join(OUT, "blade.stl")}))
    for which, (tab, _) in pieces.items():
        print(mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, which.replace(" ", "_") + ".stl")}))
    print(mcp.call("export", {"tab": both_tab, "format": "pdf", "sheet": "A4", "hidden": True, "views": ["front", "section-side@0", "top", "iso"],
                              "note": f"blade {BLADE_W:g} x {BLADE_T:g}, {BLADE_D:g} below the lip, for a frame {FRAME_W:g} x {FRAME_T:g}; spigot {SPIGOT_TIP:g} to {SPIGOT_FOOT:g} for a hose of {HOSE_ID:g} inside",
                              "path": os.path.join(OUT, "adapter.pdf")}))
    numbers = {
        "frame": {"t": FRAME_T, "w": FRAME_W, "ears_w": EARS_W, "h": FRAME_H, "lip_dip": LIP_DIP, "grille_d": GRILLE_D},
        "blade": {"t": BLADE_T, "w": BLADE_W, "d": BLADE_D, "r": BLADE_R, "ribs": RIBS, "clear_w": CLEAR_W, "x0": BLADE_X0, "x1": BLADE_X1, "clear_t": CLEAR_T,
                  "z_top": Z_BLADE_TOP, "z_bot": Z_BLADE_BOT, "y0": WALL + SLEEVE_FIT, "volume": blade_bodies[0][1],
                  "lap_w": LAP_W, "lap_t": LAP_T},
        "adapter": {"stub_l": STUB_L, "stub_t": STUB_T, "stub_d": STUB_D, "bore_l": BORE_L, "bore_t": BORE_T, "sleeve_fit": SLEEVE_FIT, "wall": WALL,
                    "stub_x0": STUB_X0, "stub_x1": STUB_X1, "bore_x0": BORE_X0, "bore_x1": BORE_X1,
                    "flange_t": FLANGE_T, "flange_out": FLANGE_OUT, "loft_h": LOFT_H, "loft2_h": LOFT2_H, "z_square": Z_SQUARE, "z_spigot": Z_SPIGOT, "z_top": Z_TOP,
                    "r_out": R_OUT, "axis_y": AXIS_Y, "spigot_tip": SPIGOT_TIP, "spigot_foot": SPIGOT_FOOT, "spigot_taper": SPIGOT_TAPER,
                    "spigot_wall": SPIGOT_WALL, "r_bore": R_BORE, "spigot_h": SPIGOT_H, "hose_id": HOSE_ID,
                    "lean_front": math.degrees(LEAN_FRONT), "lean_end": math.degrees(LEAN_END), "in_front": IN_FRONT, "in_end": IN_END,
                    "volume": bodies[0][1]},
        "pieces": {"fit_test_h": FIT_TEST_H, "fit_gap": FIT_GAP, "fit_shift": FIT_SHIFT, "x_cut": X_CUT, "tab_t": TAB_T, "tab_in": TAB_IN, "tab_out": TAB_OUT, "tab_clear": TAB_CLEAR,
                   "tab_z0": TAB_Z0, "tab_z1": TAB_Z1, "z_step": Z_STEP,
                   "volumes": {k: v for k, (_, v) in pieces.items()}},
    }
    with open(os.path.join(OUT, "adapter.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    mcp.close()
    print(f"blade {BLADE_W:g} x {BLADE_T:g}, {BLADE_D:g} deep; sleeve {STUB_L:g} x {STUB_T:g}, {STUB_D:g} deep; "
          f"wedge {LOFT_H:g} tall leaning {math.degrees(LEAN_FRONT):.0f} deg at the front and {math.degrees(LEAN_END):.0f} at the ends; "
          f"spigot {SPIGOT_TIP:g} at the tip to {SPIGOT_FOOT:g} at the foot over {SPIGOT_H:g} ({SPIGOT_TAPER:.1f} deg), {2 * R_BORE:g} bore")


if __name__ == "__main__":
    main()
