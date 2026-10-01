#!/usr/bin/env python3
"""A powered two-wheel shopping cart driven by an EGO Multi-Head power
head: a broadcast spreader run backwards. The power head's 7 mm drive
shaft comes down its attachment tube into a gearbox between the wheels,
three stages of big printed gears take 4800 rpm down to walking pace,
and a solid axle drives two 10-inch hand-truck wheels. A platform above
the wheels carries a large Costco bag each side.

    cargo build --release -p ok-mcp
    python3 examples/ego-cart/build.py [out-dir]

Writes ego_cart.okpart, a PNG per part, the assembly views and the
drawing sheets into out-dir (default examples/ego-cart/out). Every
number that is an assumption about a bought part says so; the README
lists what to measure.
"""
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part  # noqa: E402

# ---------------------------------------------------------------------
# The cart's frame: x across the cart (the axle), +y towards the user
# at the back, z up, origin on the ground under the axle's centre.
# ---------------------------------------------------------------------

# Bought parts. The wheel is a 4.10/3.50-4 pneumatic hand-truck wheel
# with 5/8" ball bearings (254 x 89, hub lengths vary 45..80 by maker:
# measure yours); the axle is 5/8" steel rod.
WHEEL_D, WHEEL_W, HUB_D, HUB_L = 254.0, 89.0, 60.0, 70.0
AXLE_D = 15.875
TRACK = 640.0  # wheel centre to wheel centre
AXLE_L = TRACK + 2 * 60.0
R_WHEEL = WHEEL_D / 2  # 127: the axle's height

# The EGO power head (PH1400/PH1420): 4800 rpm no load at a 7 mm solid
# steel shaft, turning counter-clockwise seen from the attachment end;
# an aluminium tube of a diameter still to be measured (26 assumed), a
# tool-free coupler. The whole head is about a metre long with the motor
# and battery at the user's end, so the head is the cart's handle.
HEAD_RPM = 4800.0
SHAFT_D, TUBE_OD, TUBE_ID = 7.0, 26.0, 20.0  # measure the tube
HEAD_L, HEAD_TUBE_OD = 1000.0, 28.0  # measure
MOTOR_W, MOTOR_H, MOTOR_L = 110.0, 90.0, 240.0  # the motor and battery block, measure
SLOPE = 40.0  # the handle's angle above the ground, degrees
TUBE_L = 300.0  # our attachment tube, coupler to gearbox

# A large Costco bag: 20 x 11.5 footprint, 14 tall, in inches.
BAG_L, BAG_W, BAG_H = 508.0, 292.0, 356.0

# The gear train: a 90 degree bevel pair at the input, where the speed
# is high and the torque low (keep that pinion small: at 4800 rpm a
# 24 mm pitch circle runs 6 m/s), then two spur stages where the torque
# is high and size is strength, module 2.5 and 25 mm faces. Every shaft
# is parallel to the axle and at its height, in a row behind it, so the
# box is long and low and the platform stays just above the wheels.
M_BEVEL, Z_BEVEL_PINION, Z_BEVEL_GEAR = 2.0, 12, 36
M_SPUR, Z_PINION, Z_GEAR2, Z_GEAR_AXLE = 2.5, 15, 60, 63
FACE, FACE_BEVEL = 25.0, 20.0
SHAFT2_D = 12.0
RATIO = (Z_BEVEL_GEAR / Z_BEVEL_PINION) * (Z_GEAR2 / Z_PINION) * (Z_GEAR_AXLE / Z_PINION)
PD = lambda m, z: m * z  # pitch diameter
Y_SHAFT2 = (PD(M_SPUR, Z_PINION) + PD(M_SPUR, Z_GEAR_AXLE)) / 2  # 97.5 behind the axle
Y_SHAFT3 = Y_SHAFT2 + (PD(M_SPUR, Z_PINION) + PD(M_SPUR, Z_GEAR2)) / 2  # 191.25
Z_SHAFTS = R_WHEEL

# The input axis: through the bevel apex on shaft 3's axis, up and back
# at the handle's slope. The bevel gear's pitch circle sits a pinion
# pitch radius from the apex along shaft 3, the pinion's a gear pitch
# radius from it along the input axis.
D_IN = (0.0, math.cos(math.radians(SLOPE)), math.sin(math.radians(SLOPE)))
APEX = (0.0, Y_SHAFT3, Z_SHAFTS)
ALONG = lambda t: (APEX[0] + D_IN[0] * t, APEX[1] + D_IN[1] * t, APEX[2] + D_IN[2] * t)
R_BP, R_BG = PD(M_BEVEL, Z_BEVEL_PINION) / 2, PD(M_BEVEL, Z_BEVEL_GEAR) / 2
T_PINION = R_BG  # the pinion's pitch plane, along the input axis from the apex
T_TUBE0 = T_PINION + FACE_BEVEL + 8.0  # where the attachment tube's end seats
T_COUPLER = T_TUBE0 + TUBE_L

# The gearbox housing: 110 wide (gear faces and clearances), from ahead
# of the axle gear to behind the bevels, bottom 40 mm off the ground,
# its rear wall square to the input axis so the shaft bore and the tube
# seat are a hole normal to a face.
GB_W, GB_Z0, GB_Z1, GB_Y0 = 110.0, 40.0, 215.0, -100.0
GB_REAR_Y = 252.0  # the vertical rear wall, clear of the bevel pinion's rim inside
GB_REAR_T = T_TUBE0 + 4.0  # the rear wall's plane: input-axis coordinate past the pinion
_apex_t = APEX[1] * D_IN[1] + APEX[2] * D_IN[2]
_plane = _apex_t + GB_REAR_T  # y cos + z sin = _plane on the sloped wall
GB_SLOPE_Z0 = (_plane - GB_REAR_Y * D_IN[1]) / D_IN[2]  # where the vertical rear wall meets the slope
GB_SLOPE_Y1 = (_plane - GB_Z1 * D_IN[2]) / D_IN[1]  # where the slope meets the top
GB_PROFILE = [(GB_Y0, GB_Z0), (GB_REAR_Y, GB_Z0), (GB_REAR_Y, GB_SLOPE_Z0), (GB_SLOPE_Y1, GB_Z1), (GB_Y0, GB_Z1)]
# The cavity: the profile drawn in by the wall thickness, 16 at the
# sloped rear wall so the tube seat has material under it, through the
# box but for the side walls that carry the bearings.
GB_WALL, GB_REAR_WALL = 8.0, 16.0
_inner_plane = _plane - GB_REAR_WALL
GB_CAVITY = [
    (GB_Y0 + GB_WALL, GB_Z0 + GB_WALL),
    (GB_REAR_Y - GB_WALL, GB_Z0 + GB_WALL),
    (GB_REAR_Y - GB_WALL, (_inner_plane - (GB_REAR_Y - GB_WALL) * D_IN[1]) / D_IN[2]),
    ((_inner_plane - (GB_Z1 - GB_WALL) * D_IN[2]) / D_IN[1], GB_Z1 - GB_WALL),
    (GB_Y0 + GB_WALL, GB_Z1 - GB_WALL),
]

# The frame: two side plates carry the axle bearings, the gearbox and
# the platform; the platform sits just over the wheels.
PLATE_T, PLATE_Y0, PLATE_Y1, PLATE_Z0 = 12.0, -150.0, 280.0, 60.0
PLATFORM_W, PLATFORM_D, PLATFORM_T = 760.0, 520.0, 12.0
PLATFORM_Z = WHEEL_D + 36.0  # 290: underside, clear of the tyres
BAG_X = GB_W / 2 + 5.0 + PLATE_T + BAG_W / 2  # a bag outboard of each side plate

# What the shop wants to know.
WHEEL_RPM = HEAD_RPM / RATIO
SPEED = WHEEL_RPM / 60.0 * math.pi * WHEEL_D / 1000.0  # m/s at no load
LOAD_KG, GRADE, ROLLING = 80.0, 0.15, 0.02  # cart, two full bags and a hand on it; a 15 % hill
F_HILL = LOAD_KG * 9.81 * (GRADE + ROLLING)
T_AXLE = F_HILL * R_WHEEL / 1000.0
T_INPUT = T_AXLE / RATIO
P_HILL = F_HILL * SPEED * 0.85  # at the no-load speed, say


# ---------------------------------------------------------------------
# Parts: each built along its own z axis at the origin, placed later.
# ---------------------------------------------------------------------
def cylinder(p, d, h, name, z0=0.0, bore=None):
    """A disc or a ring: with a bore the outer circle is extruded and
    the bore drilled, so a thin tube does not come out as its core."""
    s = p.sketch("top", z0, name)
    p.circle(s, (0.0, 0.0), d / 2)
    p.extrude(s, h, name=name)
    if bore:
        s = p.sketch("top", z0, f"{name} bore")
        p.point(s, (0.0, 0.0))
        p.hole(s, bore, direction="normal", name=f"{name} bore")


def wheel(p):
    """Tyre and rim as one disc, the hub through it, bored for the axle."""
    cylinder(p, WHEEL_D, WHEEL_W, "tyre", bore=AXLE_D + 0.1)
    s = p.sketch("top", (WHEEL_W - HUB_L) / 2, "hub")
    p.circle(s, (0.0, 0.0), HUB_D / 2)
    p.circle(s, (0.0, 0.0), (AXLE_D + 0.1) / 2)
    p.extrude(s, HUB_L, profiles="largest", op="add", name="hub")


def axle(p):
    cylinder(p, AXLE_D, AXLE_L, "5/8 axle")


def gear(z, m, face, bore, hub_d=None, hub_l=0.0):
    def build(p):
        cylinder(p, PD(m, z), face, f"{z}t gear, module {m}", bore=bore + 0.1)
        if hub_d:
            # The hub on the far side from the next pinion.
            s = p.sketch("top", -hub_l, "hub")
            p.circle(s, (0.0, 0.0), hub_d / 2)
            p.circle(s, (0.0, 0.0), (bore + 0.1) / 2)
            p.extrude(s, hub_l, profiles="largest", op="add", name="hub")

    return build


def shaft2(p):
    cylinder(p, SHAFT2_D, GB_W, "12 mm shaft")


def drive_shaft(p):
    cylinder(p, SHAFT_D, T_COUPLER - T_PINION + 10.0, "7 mm drive shaft")


def attachment_tube(p):
    cylinder(p, TUBE_OD, TUBE_L, "attachment tube", bore=TUBE_ID)


def power_head(p):
    """The EGO head as a ghost: its tube from the coupler to the motor
    block, the block with the battery and the rear grip."""
    cylinder(p, HEAD_TUBE_OD, HEAD_L, "head tube", bore=TUBE_ID)
    s = p.sketch("top", HEAD_L, "motor block")
    p.rect(s, (-MOTOR_W / 2, -MOTOR_H / 2), (MOTOR_W / 2, MOTOR_H / 2))
    p.extrude(s, MOTOR_L, op="add", name="motor and battery")


def gearbox(p):
    """The housing: the y-z profile extruded across, bored for the three
    shafts, with the drive shaft's bore and the tube's seat drilled
    normal to the sloped rear wall."""
    s = p.sketch("right", -GB_W / 2, "profile")
    p.polygon(s, GB_PROFILE)
    p.extrude(s, GB_W, name="housing")
    # The bearing bores before the cavity: a hole into a box with an
    # enclosed cavity fails in the kernel today (docs/ROADMAP.md).
    s = p.sketch("right", -GB_W / 2 - 1.0, "shaft bores")
    for y in (0.0, Y_SHAFT2, Y_SHAFT3):
        p.point(s, (y, Z_SHAFTS))
    p.hole(s, 28.0, direction="normal", name="bearing bores")
    s = p.sketch("right", -GB_W / 2 + GB_WALL, "cavity")
    p.polygon(s, GB_CAVITY)
    p.cut(s, GB_W - 2 * GB_WALL, name="cavity")
    # The sloped wall: the face whose normal is the input direction.
    rep = p.report()
    wall = next(
        f for f in rep["bodies"][0]["faces"]
        if abs(f["normal"]["y"] - D_IN[1]) < 1e-6 and abs(f["normal"]["z"] - D_IN[2]) < 1e-6
    )
    s = p.sketch_on(wall["reference"], "drive shaft bore")
    plane = next(sk["plane"] for sk in p.report()["sketches"] if sk["feature"] == s)
    o, u, v = plane["origin"], plane["x_axis"], plane["y_axis"]
    hit = ALONG(GB_REAR_T)  # the input axis through the wall
    d = (hit[0] - o["x"], hit[1] - o["y"], hit[2] - o["z"])
    p.point(s, (d[0] * u["x"] + d[1] * u["y"] + d[2] * u["z"], d[0] * v["x"] + d[1] * v["y"] + d[2] * v["z"]))
    p.hole(s, SHAFT_D + 3.0, depth=GB_REAR_WALL + 2.0, counterbore={"diameter": TUBE_OD + 0.5, "depth": 12.0}, name="shaft bore and tube seat")


def side_plate(p):
    s = p.sketch("right", 0.0, "plate")
    p.rect(s, (PLATE_Y0, PLATE_Z0), (PLATE_Y1, PLATFORM_Z))
    p.extrude(s, PLATE_T, name="side plate")
    s = p.sketch("right", -1.0, "axle bore")
    p.point(s, (0.0, Z_SHAFTS))
    p.hole(s, AXLE_D + 1.0, direction="normal", name="axle bore")


def platform(p):
    s = p.sketch("top", 0.0, "deck")
    p.rect(s, (-PLATFORM_W / 2, -PLATFORM_D / 2), (PLATFORM_W / 2, PLATFORM_D / 2))
    p.extrude(s, PLATFORM_T, name="platform")


def bag(p):
    s = p.sketch("top", 0.0, "bag")
    p.rect(s, (-BAG_W / 2, -BAG_L / 2), (BAG_W / 2, BAG_L / 2))
    p.extrude(s, BAG_H, name="Costco bag")


PARTS = [
    ("Wheel", "wheel", wheel),
    ("Axle", "axle", axle),
    ("Gearbox", "gearbox", gearbox),
    ("Axle gear", "axle_gear", gear(Z_GEAR_AXLE, M_SPUR, FACE, AXLE_D, HUB_D, 15.0)),
    ("Pinion", "pinion", gear(Z_PINION, M_SPUR, FACE, SHAFT2_D)),
    ("Gear 2", "gear2", gear(Z_GEAR2, M_SPUR, FACE, SHAFT2_D)),
    ("Bevel gear", "bevel_gear", gear(Z_BEVEL_GEAR, M_BEVEL, FACE_BEVEL, SHAFT2_D)),
    ("Bevel pinion", "bevel_pinion", gear(Z_BEVEL_PINION, M_BEVEL, 12.0, SHAFT_D)),
    ("Shaft", "shaft", shaft2),
    ("Drive shaft", "drive_shaft", drive_shaft),
    ("Attachment tube", "attachment_tube", attachment_tube),
    ("Power head", "power_head", power_head),
    ("Side plate", "side_plate", side_plate),
    ("Platform", "platform", platform),
    ("Bag", "bag", bag),
]
PRINTED = {"Gearbox", "Axle gear", "Pinion", "Gear 2", "Bevel gear", "Bevel pinion"}

# Rotations are degrees about x, y, z. A part built along +z lies along
# +x after ry = 90, and along the input direction after rx = SLOPE - 90.
RY_X = 90.0
RX_IN = SLOPE - 90.0


def instances():
    at = lambda tab, name, x, y, z, rx=0.0, ry=0.0, rz=0.0: (tab, name, (x, y, z), (rx, ry, rz))
    along = lambda tab, name, t: at(tab, name, *ALONG(t), rx=RX_IN)
    out = [
        at("Wheel", "left wheel", -TRACK / 2 - WHEEL_W / 2, 0, Z_SHAFTS, ry=RY_X),
        at("Wheel", "right wheel", TRACK / 2 - WHEEL_W / 2, 0, Z_SHAFTS, ry=RY_X),
        at("Axle", "axle", -AXLE_L / 2, 0, Z_SHAFTS, ry=RY_X),
        at("Gearbox", "gearbox", 0, 0, 0),
        at("Axle gear", "axle gear", -FACE / 2, 0, Z_SHAFTS, ry=RY_X),
        at("Pinion", "pinion 3", -FACE / 2, Y_SHAFT2, Z_SHAFTS, ry=RY_X),
        at("Gear 2", "gear 2", FACE / 2 + 2.0, Y_SHAFT2, Z_SHAFTS, ry=RY_X),
        at("Pinion", "pinion 2", FACE / 2 + 2.0, Y_SHAFT3, Z_SHAFTS, ry=RY_X),
        at("Bevel gear", "bevel gear", -R_BP - FACE_BEVEL, Y_SHAFT3, Z_SHAFTS, ry=RY_X),
        at("Shaft", "shaft 2", -GB_W / 2, Y_SHAFT2, Z_SHAFTS, ry=RY_X),
        at("Shaft", "shaft 3", -GB_W / 2, Y_SHAFT3, Z_SHAFTS, ry=RY_X),
        along("Bevel pinion", "bevel pinion", T_PINION),
        along("Drive shaft", "drive shaft", T_PINION - 10.0),
        along("Attachment tube", "attachment tube", T_TUBE0),
        along("Power head", "power head", T_COUPLER),
        at("Side plate", "left plate", -GB_W / 2 - 5.0 - PLATE_T, 0, 0),
        at("Side plate", "right plate", GB_W / 2 + 5.0, 0, 0),
        at("Platform", "platform", 0, 0, PLATFORM_Z),
        at("Bag", "left bag", -BAG_X, 0, PLATFORM_Z + PLATFORM_T),
        at("Bag", "right bag", BAG_X, 0, PLATFORM_Z + PLATFORM_T),
    ]
    return out


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "out")
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, "ego_cart.okpart")
    if os.path.exists(path):
        os.remove(path)
    print(
        f"gear train {Z_BEVEL_PINION}:{Z_BEVEL_GEAR} bevel, {Z_PINION}:{Z_GEAR2} and {Z_PINION}:{Z_GEAR_AXLE} spur, module {M_BEVEL} and {M_SPUR}: {RATIO:.1f}:1\n"
        f"wheels {WHEEL_RPM:.0f} rpm at the head's {HEAD_RPM:.0f}: {SPEED:.2f} m/s ({SPEED * 3.6:.1f} km/h) at no load, the trigger below that\n"
        f"{LOAD_KG:.0f} kg up a {GRADE * 100:.0f} % hill: {F_HILL:.0f} N at the tyres, {T_AXLE:.1f} Nm at the axle, {T_INPUT:.2f} Nm at the head, about {P_HILL:.0f} W\n"
        f"shafts behind the axle at y = {Y_SHAFT2:.2f} and {Y_SHAFT3:.2f}; the coupler at {ALONG(T_COUPLER)[1]:.0f} back and {ALONG(T_COUPLER)[2]:.0f} up, the rear grip near {ALONG(T_COUPLER + HEAD_L)[2]:.0f} up"
    )
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "ego_cart"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "EGO cart"}]})
    tabs = {}
    for i, (title, stem, build) in enumerate(PARTS):
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
        print(f"{title:<16} tab {tab:>2}: " + ", ".join(f"{n} {v:.0f} mm3" for n, v in bodies))
        part.screenshot(os.path.join(out, f"{stem}.png"))
        if title in PRINTED:
            part.export(os.path.join(out, f"{stem}.stl"))
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Cart"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops = [
        {
            "type": "add_instance",
            "studio": tabs[tab_title],
            "body": 0,
            "name": name,
            "fixed": True,
            "placement": {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": rx, "y": ry, "z": rz}},
        }
        for tab_title, name, (x, y, z), (rx, ry, rz) in instances()
    ]
    text = mcp.call("apply", {"ops": ops, "tab": asm})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    report = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
    print(f"Cart assembly tab {asm}: {len(report['instances'])} instances, {len(report['bodies'])} bodies")
    for view, name in (("iso", "assembly_iso"), ("right", "assembly_side"), ("front", "assembly_front"), ("-0.7,0.6,0.4", "assembly_rear")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    mcp.call("screenshot", {"tab": asm, "view": "right", "section": "x:0:flip", "width": 1200, "height": 900, "path": os.path.join(out, "assembly_section.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "note": "concept", "path": os.path.join(out, "assembly.pdf")}))
    print(mcp.call("export", {"tab": tabs["Gearbox"], "format": "pdf", "sheet": "A3", "views": ["front", "top", "right", "section-side@0"], "note": "concept", "path": os.path.join(out, "gearbox.pdf")}))
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
