"""The EGO cart, rev B: a powered tricycle for two Costco bags.

One 20 inch bicycle wheel at the back drives the cart; two swivel
casters ahead of it on a wide track carry the front. The EGO power head
is the handle, on a mast beside the wheel: the cut-off male end of an
EGO extension pole plugs into its coupler, a flex shaft runs from that
stub under the deck to a bought worm gearbox, and the gearbox's output
pinion drives a printed ring gear bolted to the wheel hub's six-bolt
disc mount. Both printed gears are `add_gear` features with real
involute teeth, so they export to STL as they print.

    cargo build --release -p ok-mcp
    python3 examples/ego-cart/build.py          # the document, pictures and sheets
    cargo test -p ok-render --test ego_cart     # what CI runs

Frame: x across the cart (left negative), y towards the user, z up,
origin on the ground under the drive wheel's contact. Every part is
built in place in that frame, so the assembly only names them.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, v2  # noqa: E402

# ---------------------------------------------------------------------
# Bought parts (every number here is an assumption to measure).
# ---------------------------------------------------------------------
HEAD_RPM = 4800.0  # EGO PH1400 power head, no load
HEAD_TUBE_OD, HEAD_L = 28.0, 1000.0  # its tube, coupler to motor block
MOTOR_W, MOTOR_H, MOTOR_L = 110.0, 90.0, 240.0  # motor, battery, rear grip
STUB_OD, STUB_L = 26.0, 150.0  # the cut male end of an EP7500 extension pole
HEAD_SLOPE = 50.0  # degrees above horizontal, the handle's angle

WHEEL_D, TYRE_W, RIM_ID = 508.0, 45.0, 400.0  # 20 inch bicycle front wheel
HUB_D, HUB_W, AXLE_D, AXLE_L = 36.0, 100.0, 10.0, 150.0  # six-bolt disc hub
ROTOR_X, ROTOR_BOSS_D, ROTOR_BCD, ROTOR_BOLT = -40.0, 50.0, 44.0, 5.5  # ISO six-bolt mount
R_WHEEL = WHEEL_D / 2

WORM_RATIO = 20  # NMRV040 worm gearbox, hollow 18 mm output, 14 mm input
WORM_W, WORM_D, WORM_H = 60.0, 100.0, 100.0  # across the output, along the input, tall
WORM_OUT_D, WORM_IN_D, WORM_IN_L = 18.0, 14.0, 40.0
WORM_EFFICIENCY = 0.7

CASTER_D, CASTER_W, CASTER_TRAIL = 200.0, 50.0, 40.0  # pneumatic swivel casters
CASTER_X, CASTER_Y = 380.0, -340.0  # the swivel axes
FLEX_D, FLEX_GAP = 16.0, 25.0  # flex shaft casing, and the couplings at its ends

# ---------------------------------------------------------------------
# Printed gears and the drive geometry.
# ---------------------------------------------------------------------
M, Z_RING, Z_PINION, FACE, PRESSURE, BACKLASH = 3.0, 80, 17, 20.0, 20.0, 0.15
RING_RIM, WEB_T, WEB_HOLE = 265.0, 5.0, 36.5
RING_SEGMENTS = 6  # printable sectors, cut by radial planes
DOWEL_D, DOWEL_DEPTH = 3.2, 8.0  # a 3 mm pin in each rim joint
PINION_BORE, PINION_HUB_D, PINION_HUB_L = WORM_OUT_D + 0.2, 40.0, 15.0
R_RING, R_PINION = M * Z_RING / 2, M * Z_PINION / 2
CENTRE = R_RING - R_PINION  # a pinion inside a ring: the difference
PINION_DIR = -121.5  # degrees from +y in the y-z plane, from the axle to the pinion: low enough to clear the axle beam
PINION_Y = CENTRE * math.cos(math.radians(PINION_DIR))
PINION_Z = R_WHEEL + CENTRE * math.sin(math.radians(PINION_DIR))
# The ring's first tooth points along +y, and the pinion's direction is
# a whole number of ring pitches from it, so a ring tooth points at the
# pinion's centre: the pinion turns half a pitch so a space faces it.
assert abs(PINION_DIR / (360.0 / Z_RING) - round(PINION_DIR / (360.0 / Z_RING))) < 1e-9
PINION_ANGLE = PINION_DIR + 180.0 / Z_PINION
RING_X0 = ROTOR_X - WEB_T - FACE  # the teeth, outboard of the web on the rotor face
WORM_X1 = RING_X0 - PINION_HUB_L - 5.0  # the gearbox's inner face, clear of the pinion's hub
WORM_X0 = WORM_X1 - WORM_W
WORM_Y0, WORM_Y1 = PINION_Y - WORM_D / 2, PINION_Y + WORM_D / 2
WORM_Z0, WORM_Z1 = PINION_Z - WORM_H / 2, PINION_Z + WORM_H / 2
WORM_IN_END = (WORM_X0 + WORM_W / 2, WORM_Y1 + WORM_IN_L, PINION_Z)

# ---------------------------------------------------------------------
# Deck, frame and handle.
# ---------------------------------------------------------------------
PLATFORM_W, PLATFORM_T = 860.0, 12.0
PLATFORM_Y0, PLATFORM_Y1, PLATFORM_Z = -520.0, 260.0, 270.0
SLOT_X0, SLOT_X1, SLOT_Y0 = -80.0, 55.0, -270.0  # the wheel and ring through the deck
RAIL, RAIL_X = 40.0, 240.0  # square tube rails under the deck
RAIL_Y0, RAIL_Y1 = PLATFORM_Y0, 300.0  # the rear member sits behind the tyre and the deck
DROPOUT_X, DROPOUT_T, DROPOUT_HALF = 70.0, 6.0, 40.0
BRACKET_T = 6.0
BAG_L, BAG_W, BAG_H = 508.0, 292.0, 356.0
BAG_X = -SLOT_X0 + 10.0 + BAG_W / 2  # clear of the slot
BAG_Y = -250.0  # the load centre, well ahead of the drive wheel (see stability)
MAST_D, MAST_L, MAST_BASE = 32.0, 300.0, (WORM_IN_END[0], RAIL_Y1 - RAIL / 2, PLATFORM_Z)
HEAD_OFFSET = 70.0  # the head rides behind and below the mast, parallel to it
D_HEAD = (0.0, math.cos(math.radians(HEAD_SLOPE)), math.sin(math.radians(HEAD_SLOPE)))
PERP = (0.0, D_HEAD[2], -D_HEAD[1])  # behind and below, square to the head


def add(a, b, s=1.0):
    return (a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2])


HEAD_BASE = add(MAST_BASE, PERP, HEAD_OFFSET)  # the coupler
STUB_END = add(HEAD_BASE, D_HEAD, -STUB_L)  # where the flex shaft arrives
MASS = {  # kg, the cart's own parts estimated, the bags as carried
    "drive wheel": 2.5, "axle": 0.1, "pinion": 0.15, "output shaft": 0.2,
    **{f"ring segment {k + 1}": 0.9 / RING_SEGMENTS for k in range(RING_SEGMENTS)},
    "worm box": 2.6, "flex shaft": 0.5, "EGO stub": 0.3, "power head": 4.5, "frame": 6.0,
    "platform": 4.0, "left caster": 1.5, "right caster": 1.5, "left bag": 20.0, "right bag": 20.0,
}

# ---------------------------------------------------------------------
# The numbers.
# ---------------------------------------------------------------------
RATIO = WORM_RATIO * Z_RING / Z_PINION
WHEEL_RPM = HEAD_RPM / RATIO
SPEED = WHEEL_RPM / 60.0 * math.pi * WHEEL_D / 1000.0
LOAD_KG, GRADE, ROLLING = 80.0, 0.15, 0.02
F_HILL = LOAD_KG * 9.81 * (GRADE + ROLLING)
T_AXLE = F_HILL * R_WHEEL / 1000.0
F_TOOTH = T_AXLE / (R_RING / 1000.0)
T_PINION = F_TOOTH * R_PINION / 1000.0
T_HEAD = T_PINION / (WORM_RATIO * WORM_EFFICIENCY)
P_HEAD = T_HEAD * HEAD_RPM * 2 * math.pi / 60.0
GRIP_KG = {"dry": F_HILL / 0.6 / 9.81, "wet": F_HILL / 0.4 / 9.81}


# ---------------------------------------------------------------------
# Builders, all in the cart's frame.
# ---------------------------------------------------------------------
def along(p, a, b, d, name, op="new"):
    """A cylinder from point `a` to point `b` (same x), built on a top
    plane turned about x to face along the line."""
    e = (b[1] - a[1], b[2] - a[2])
    length = math.hypot(*e)
    e = (e[0] / length, e[1] / length)
    theta = math.atan2(-e[0], e[1])  # the turned plane's normal is (0, -sin, cos)
    v = a[1] * math.cos(theta) + a[2] * math.sin(theta)
    offset = a[1] * e[0] + a[2] * e[1]
    s = p.feature({"type": "add_sketch", "plane": {"type": "rotated", "base": "top", "axis": "x", "angle": math.degrees(theta), "offset": offset}, "name": name})
    p.circle(s, (a[0], v), d / 2)
    return p.extrude(s, length, op=op, name=name)


def drive_wheel(p):
    """Tyre and rim as one ring, the hub through it with the rotor boss
    of the disc mount, bored for the axle. Spokes are left out."""
    s = p.sketch("right", -TYRE_W / 2, "tyre")
    p.circle(s, (0.0, R_WHEEL), R_WHEEL)
    p.extrude(s, TYRE_W, name="tyre and rim")
    s = p.sketch("right", -TYRE_W / 2, "rim bore")
    p.point(s, (0.0, R_WHEEL))
    p.hole(s, RIM_ID, direction="normal", name="inside the rim")
    s = p.sketch("right", -1.5, "spokes")
    p.circle(s, (0.0, R_WHEEL), RIM_ID / 2 + 1.0)
    p.extrude(s, 3.0, op="add", name="spokes, as a disc")
    s = p.sketch("right", -HUB_W / 2, "hub")
    p.circle(s, (0.0, R_WHEEL), HUB_D / 2)
    p.extrude(s, HUB_W, op="add", name="hub")
    s = p.sketch("right", ROTOR_X, "rotor boss")
    p.circle(s, (0.0, R_WHEEL), ROTOR_BOSS_D / 2)
    p.extrude(s, 10.0, op="add", name="disc mount boss")
    s = p.sketch("right", -HUB_W / 2 - 1.0, "axle bore")
    p.point(s, (0.0, R_WHEEL))
    p.hole(s, AXLE_D + 0.2, direction="normal", name="axle bore")


def axle(p):
    s = p.sketch("right", -AXLE_L / 2, "axle")
    p.circle(s, (0.0, R_WHEEL), AXLE_D / 2)
    p.extrude(s, AXLE_L, name="10 mm axle")


def ring_gear(p):
    """Internal teeth outboard of a web that bolts to the disc mount,
    cut into sectors that fit a printer, each rim joint pinned by a
    dowel across the cut (half a hole in each sector)."""
    p.gear(M, Z_RING, FACE, base="right", offset=RING_X0, center=(0.0, R_WHEEL), pressure_angle=PRESSURE, rim=RING_RIM, backlash=BACKLASH, name=f"{Z_RING}t internal gear, module {M}")
    s = p.sketch("right", RING_X0 + FACE, "web")
    p.circle(s, (0.0, R_WHEEL), RING_RIM / 2)
    p.extrude(s, WEB_T, op="add", name="web")
    s = p.sketch("right", RING_X0 + FACE - 1.0, "web hole")
    p.point(s, (0.0, R_WHEEL))
    p.hole(s, WEB_HOLE, depth=WEB_T + 2.0, direction="normal", name="over the hub")
    s = p.sketch("right", RING_X0 + FACE - 1.0, "bolt holes")
    for i in range(6):
        a = math.radians(60.0 * i + 30.0)  # between the sector cuts, not on them
        p.point(s, (ROTOR_BCD / 2 * math.cos(a), R_WHEEL + ROTOR_BCD / 2 * math.sin(a)))
    p.hole(s, ROTOR_BOLT, depth=WEB_T + 2.0, direction="normal", name="six M5 clearance holes")
    # A dowel hole across every joint before the cuts, centred on the
    # cut plane and running along the rim, so each sector keeps half of
    # it: the hole is drilled from a plane square to the rim's direction
    # at that angle (a top plane turned about x by the angle has that
    # normal), starting a dowel's depth before the cut.
    r_dowel = (RING_RIM / 2 + M * Z_RING / 2 - M) / 2
    x_dowel = RING_X0 + FACE / 2
    for k in range(RING_SEGMENTS):
        a = 360.0 / RING_SEGMENTS * k
        s = p.feature({"type": "add_sketch", "plane": {"type": "rotated", "base": "top", "axis": "x", "angle": a, "offset": R_WHEEL * math.cos(math.radians(a)) - DOWEL_DEPTH}, "name": f"dowel at {a:.0f} degrees"})
        p.point(s, (x_dowel, r_dowel + R_WHEEL * math.sin(math.radians(a))))
        p.hole(s, DOWEL_D, depth=2 * DOWEL_DEPTH, direction="normal", name=f"dowel hole at {a:.0f} degrees")
    # Radial planes through the axle: a top plane turned about x by
    # the cut's angle passes through the axle once offset to it.
    for k in range(RING_SEGMENTS // 2):
        a = 360.0 / RING_SEGMENTS * k
        p.split({"type": "rotated", "base": "top", "axis": "x", "angle": a, "offset": R_WHEEL * math.cos(math.radians(a))}, name=f"cut at {a:.0f} degrees")


def pinion(p):
    p.gear(M, Z_PINION, FACE, base="right", offset=RING_X0, center=(PINION_Y, PINION_Z), pressure_angle=PRESSURE, bore=PINION_BORE, angle=PINION_ANGLE, backlash=BACKLASH, name=f"{Z_PINION}t gear, module {M}")
    s = p.sketch("right", RING_X0, "hub")
    p.circle(s, (PINION_Y, PINION_Z), PINION_HUB_D / 2)
    p.circle(s, (PINION_Y, PINION_Z), PINION_BORE / 2)
    p.extrude(s, PINION_HUB_L, direction="reverse", profiles="largest", op="add", name="hub")


def output_shaft(p):
    s = p.sketch("right", WORM_X0, "shaft")
    p.circle(s, (PINION_Y, PINION_Z), WORM_OUT_D / 2)
    p.extrude(s, RING_X0 + FACE - WORM_X0, name="18 mm output shaft")


def worm_box(p):
    """The NMRV040 as a block with its hollow output bore across and
    its input shaft out of the rear face."""
    s = p.sketch("right", WORM_X0, "housing")
    p.rect(s, (WORM_Y0, WORM_Z0), (WORM_Y1, WORM_Z1))
    p.extrude(s, WORM_W, name="housing")
    s = p.sketch("right", WORM_X0 - 1.0, "output bore")
    p.point(s, (PINION_Y, PINION_Z))
    p.hole(s, WORM_OUT_D + 0.2, direction="normal", name="hollow output bore")
    s = p.sketch("front", -WORM_Y1, "input")
    p.circle(s, (WORM_IN_END[0], PINION_Z), WORM_IN_D / 2)
    p.extrude(s, WORM_IN_L, direction="reverse", op="add", name="input shaft")


def flex_shaft(p):
    """The casing as a straight line from the stub to the worm's input,
    stopping a coupling's length short of each; the real one curves
    between the same ends."""
    e = (WORM_IN_END[1] - STUB_END[1], WORM_IN_END[2] - STUB_END[2])
    n = math.hypot(*e)
    e = (0.0, e[0] / n, e[1] / n)
    along(p, add(STUB_END, e, FLEX_GAP), add(WORM_IN_END, e, -FLEX_GAP), FLEX_D, "flex shaft casing")


def stub(p):
    along(p, STUB_END, HEAD_BASE, STUB_OD, "cut EP7500 end")


def power_head(p):
    top = add(HEAD_BASE, D_HEAD, HEAD_L)
    along(p, HEAD_BASE, top, HEAD_TUBE_OD, "head tube")
    along(p, top, add(top, D_HEAD, MOTOR_L), 1.0, "motor axis", op="add")  # a pin to hang the block on
    # The motor block as a box across the head's axis.
    s = p.sketch("right", top[0] - MOTOR_W / 2, "motor block")
    c = (top[1] + D_HEAD[1] * MOTOR_L / 2, top[2] + D_HEAD[2] * MOTOR_L / 2)
    p.rect(s, (c[0] - MOTOR_H / 2, c[1] - MOTOR_L / 2), (c[0] + MOTOR_H / 2, c[1] + MOTOR_L / 2))
    p.extrude(s, MOTOR_W, op="add", name="motor, battery and grip")


def frame(p):
    """Rails and cross members under the deck, the axle beams and
    dropout plates for the drive wheel, the gearbox bracket and the
    mast, all one welded body."""
    s = p.sketch("top", PLATFORM_Z - RAIL, "rails")
    p.rect(s, (-RAIL_X - RAIL / 2, RAIL_Y0), (-RAIL_X + RAIL / 2, RAIL_Y1))
    p.rect(s, (RAIL_X - RAIL / 2, RAIL_Y0), (RAIL_X + RAIL / 2, RAIL_Y1))
    p.extrude(s, RAIL, name="rails")
    s = p.sketch("top", PLATFORM_Z - RAIL, "cross members")
    p.rect(s, (-RAIL_X - RAIL / 2, RAIL_Y0), (RAIL_X + RAIL / 2, RAIL_Y0 + RAIL))
    p.rect(s, (-RAIL_X - RAIL / 2, RAIL_Y1 - RAIL), (RAIL_X + RAIL / 2, RAIL_Y1))
    p.rect(s, (-RAIL_X, -RAIL / 2), (-DROPOUT_X - DROPOUT_T, RAIL / 2))
    p.rect(s, (DROPOUT_X, -RAIL / 2), (RAIL_X, RAIL / 2))
    p.extrude(s, RAIL, op="add", name="cross members and axle beams")
    for x in (-DROPOUT_X - DROPOUT_T, DROPOUT_X):
        s = p.sketch("right", x, "dropout")
        p.rect(s, (-DROPOUT_HALF, R_WHEEL - 30.0), (DROPOUT_HALF, PLATFORM_Z))
        p.extrude(s, DROPOUT_T, op="add", name="dropout plate")
        s = p.sketch("right", x - 1.0, "axle hole")
        p.point(s, (0.0, R_WHEEL))
        p.hole(s, AXLE_D + 0.5, depth=DROPOUT_T + 2.0, direction="normal", name="axle slot")
    s = p.sketch("right", WORM_X0 - BRACKET_T, "bracket")
    p.rect(s, (WORM_Y0, WORM_Z0), (WORM_Y1, PLATFORM_Z))
    p.extrude(s, BRACKET_T, op="add", name="gearbox bracket")
    along(p, MAST_BASE, add(MAST_BASE, D_HEAD, MAST_L), MAST_D, "mast", op="add")


def platform(p):
    s = p.sketch("top", PLATFORM_Z, "deck")
    p.rect(s, (-PLATFORM_W / 2, PLATFORM_Y0), (PLATFORM_W / 2, PLATFORM_Y1))
    p.extrude(s, PLATFORM_T, name="deck")
    s = p.sketch("top", PLATFORM_Z, "wheel slot")
    p.rect(s, (SLOT_X0, SLOT_Y0), (SLOT_X1, PLATFORM_Y1 + 10.0))
    p.cut(s, PLATFORM_T, name="wheel slot")


def caster(p):
    """Built about its own swivel axis at the origin, placed twice."""
    s = p.sketch("right", -CASTER_W / 2, "wheel")
    p.circle(s, (CASTER_TRAIL, CASTER_D / 2), CASTER_D / 2)
    p.extrude(s, CASTER_W, name="caster wheel")
    s = p.sketch("top", CASTER_D, "yoke")
    p.rect(s, (-35.0, -40.0), (35.0, CASTER_TRAIL + 40.0))
    p.extrude(s, 30.0, op="add", name="yoke")
    s = p.sketch("top", CASTER_D + 30.0, "stem")
    p.circle(s, (0.0, 0.0), 10.0)
    p.extrude(s, PLATFORM_Z - 6.0 - (CASTER_D + 30.0), op="add", name="swivel stem")
    s = p.sketch("top", PLATFORM_Z - 6.0, "plate")
    p.rect(s, (-50.0, -50.0), (50.0, 50.0))
    p.extrude(s, 6.0, op="add", name="top plate")


def bag(p):
    s = p.sketch("top", 0.0, "bag")
    p.rect(s, (-BAG_W / 2, -BAG_L / 2), (BAG_W / 2, BAG_L / 2))
    p.extrude(s, BAG_H, name="Costco bag")


PARTS = [
    ("Drive wheel", "drive_wheel", drive_wheel),
    ("Axle", "axle", axle),
    ("Ring gear", "ring_gear", ring_gear),
    ("Pinion", "pinion", pinion),
    ("Output shaft", "output_shaft", output_shaft),
    (f"Worm box NMRV040 {WORM_RATIO}:1", "worm_box", worm_box),
    ("Flex shaft", "flex_shaft", flex_shaft),
    ("EGO stub", "ego_stub", stub),
    ("Power head", "power_head", power_head),
    ("Frame", "frame", frame),
    ("Platform", "platform", platform),
    ("Caster", "caster", caster),
    ("Bag", "bag", bag),
]
PRINTED = {"Ring gear", "Pinion"}


def instances():
    at = lambda tab, name, x=0.0, y=0.0, z=0.0: (tab, name, (x, y, z), 0)
    return [
        at("Drive wheel", "drive wheel"),
        at("Axle", "axle"),
            *[(f"Ring gear", f"ring segment {k + 1}", (0.0, 0.0, 0.0), k) for k in range(RING_SEGMENTS)],
        at("Pinion", "pinion"),
        at("Output shaft", "output shaft"),
        at(f"Worm box NMRV040 {WORM_RATIO}:1", "worm box"),
        at("Flex shaft", "flex shaft"),
        at("EGO stub", "EGO stub"),
        at("Power head", "power head"),
        at("Frame", "frame"),
        at("Platform", "platform"),
        at("Caster", "left caster", -CASTER_X, CASTER_Y, 0.0),
        at("Caster", "right caster", CASTER_X, CASTER_Y, 0.0),
        at("Bag", "left bag", -BAG_X, BAG_Y, PLATFORM_Z + PLATFORM_T),
        at("Bag", "right bag", BAG_X, BAG_Y, PLATFORM_Z + PLATFORM_T),
    ]


def stability(report, skip=()):
    """Centre of mass from the placed bodies' centroids and MASS (less
    the bodies in `skip`), the drive wheel's share of the weight, and
    the side slope that tips the cart over the edge from the drive
    wheel's contact to the nearer caster's."""
    total, cg = 0.0, [0.0, 0.0, 0.0]
    for b in report["bodies"]:
        if b["name"] in skip:
            continue
        m = MASS[b["name"]]
        c = b["centroid"]
        total += m
        for i, k in enumerate("xyz"):
            cg[i] += m * c[k]
    cg = [v / total for v in cg]
    contact_y = CASTER_Y + CASTER_TRAIL
    share = (cg[1] - contact_y) / (0.0 - contact_y)
    edge = (-CASTER_X if cg[0] < 0.0 else CASTER_X, contact_y)  # the nearer side's edge
    tip = math.degrees(math.atan2(abs(edge[0] * cg[1] - edge[1] * cg[0]) / math.hypot(*edge), cg[2]))
    return total, cg, share, tip


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "out")
    os.makedirs(out, exist_ok=True)
    for stale in os.listdir(out):
        os.remove(os.path.join(out, stale))
    path = os.path.join(out, "ego_cart.okpart")
    print(
        f"drive: worm {WORM_RATIO}:1 then {Z_PINION}:{Z_RING} ring, module {M}: {RATIO:.1f}:1\n"
        f"wheel {WHEEL_RPM:.0f} rpm at the head's {HEAD_RPM:.0f}: {SPEED:.2f} m/s ({SPEED * 3.6:.1f} km/h) at no load, the trigger below that\n"
        f"{LOAD_KG:.0f} kg up a {GRADE * 100:.0f} % hill: {F_HILL:.0f} N at the tyre, {T_AXLE:.1f} Nm at the wheel, {F_TOOTH:.0f} N on the ring's teeth, "
        f"{T_PINION:.1f} Nm at the pinion, {T_HEAD:.2f} Nm at the head, about {P_HEAD:.0f} W\n"
        f"grip needs {GRIP_KG['dry']:.0f} kg on the drive wheel dry, {GRIP_KG['wet']:.0f} kg wet\n"
        f"pinion {CENTRE:.1f} mm from the axle at {PINION_DIR:.0f} degrees; ring teeth at x {RING_X0:.0f} to {RING_X0 + FACE:.0f}, web on the rotor face at {ROTOR_X:.0f}"
    )
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "ego_cart"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "EGO cart"}]})
    tabs = {}
    for i, (title, stem_, build) in enumerate(PARTS):
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]})
            tab = 1
        else:
            text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": title}]})
            tab = int(re.search(r"tab (\d+)", text).group(1))
        tabs[title] = tab
        part = Part(mcp, tab, title)
        build(part)
        bodies = part.bodies()
        print(f"{title:<24} tab {tab:>2}: " + ", ".join(f"{n} {v:.0f} mm3" for n, v in bodies))
        part.screenshot(os.path.join(out, f"{stem_}.png"), view="-0.7,-0.5,0.5" if title in PRINTED else "iso")
        if title in PRINTED:
            part.export(os.path.join(out, f"{stem_}.stl"))
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Cart"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops = [
        {
            "type": "add_instance",
            "studio": tabs[tab_title],
            "body": body,
            "name": name,
            "fixed": True,
            "placement": {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}},
        }
        for tab_title, name, (x, y, z), body in instances()
    ]
    text = mcp.call("apply", {"ops": ops, "tab": asm})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    report = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
    print(f"Cart assembly tab {asm}: {len(report['instances'])} instances, {len(report['bodies'])} bodies")
    total, cg, share, tip = stability(report)
    _, _, _, one_bag = stability(report, skip=("right bag",))
    _, _, _, empty = stability(report, skip=("left bag", "right bag"))
    print(
        f"{total:.0f} kg with the bags, centre of mass {-cg[1]:.0f} mm ahead of the drive wheel and {cg[2]:.0f} mm up: "
        f"{share * 100:.0f} % on the drive wheel, tips sideways at {tip:.1f} degrees; "
        f"on the hill the mass centre moves {GRADE * cg[2]:.0f} mm towards the wheel\n"
        f"one bag on the left tips at {one_bag:.1f} degrees, the empty cart at {empty:.1f}"
    )
    for view, name in (("iso", "assembly_iso"), ("right", "assembly_side"), ("front", "assembly_front"), ("-0.7,0.6,0.4", "assembly_rear")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    mcp.call("screenshot", {"tab": asm, "view": "right", "section": f"x:{RING_X0 + FACE / 2}:flip", "width": 1200, "height": 900, "path": os.path.join(out, "assembly_section.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "note": "rev B concept", "path": os.path.join(out, "assembly.pdf")}))
    print(mcp.call("export", {"tab": tabs["Ring gear"], "format": "pdf", "sheet": "A3", "views": ["right", "front", "section@0"], "note": f"{RING_SEGMENTS} printed sectors, {DOWEL_D:.0f} mm dowels in the rim joints", "path": os.path.join(out, "ring_gear.pdf")}))
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
