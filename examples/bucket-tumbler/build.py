"""A parts tumbler from a 5-gallon bucket: tilted about 25 degrees and
turned at 20-35 rpm by a 12 V worm gearmotor on the centre of its
bottom, with two fixed casters under the upper wall carrying most of
its weight. It eases the edges of about 40 small wood inlay pieces per
batch to a 0.3-0.5 mm radius. The brief is README.md in this
directory. This script builds the document the brief describes and
checks the brief's numbers on it.

    cargo build --release -p ok-mcp
    python3 examples/bucket-tumbler/build.py        # the document, pictures, sheets and numbers
    cargo test -p ok-render --test bucket_tumbler   # what CI runs

Everything is in inches, as the shop works, and converted to
millimetres at the edge. There are two part studios:

- "on the deck" is built with the deck flat. x runs along the deck
  from its hinged (motor) end, y across it, and z up, measured from
  the bucket's axis, so the axis is the world x axis. That way the
  bucket is a revolve about x and the battens a circular pattern
  about it.
- "base" holds the base, the hinge and the prop leg in the world
  frame: z = 0 is the top of the base and the hinge line is the y
  axis.

The "propped at 25 deg" assembly turns every body of the deck about
the hinge. The bucket, motor, casters and the rest are taken from
catalogue sizes and listings. They are envelopes to check fits
against, not drawings of a particular product.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, apply  # noqa: E402

OUT = os.path.join(HERE, "out")
IN = 25.4
MM = 1.0 / IN

# -- the bucket: which one, and its sizes (typical; measure yours) ------
# "2gal" is the owner's choice: a Lowe's 2-gallon general bucket, listed
# 10.25 x 10.25 x 9.25 in (rim across, rim across, height); the floor's
# diameter is not listed. "5gal" is the brief's first design, kept so
# the two can be compared: change BUCKET and run the script again.
BUCKETS = {
    "2gal": dict(
        name="2 gal",
        H=9.25,                 # outside, floor to the top of the rim (listing)
        R_RIM=10.25 / 2,        # the rim's lip (listing)
        R_TOP=9.6 / 2,          # wall just under the rim (fit)
        R_BOTTOM=8.4 / 2,       # wall at the floor (fit: not listed)
        RIM_BAND=0.6,           # how far down the wall the lip and its hoop reach
        WALL=0.07,
        BELOW_RIM=3.0,          # the casters' line below the rim, under the hoops (fit)
        BATTEN_L=7.5,           # battens, floor up
        LOAD_OFFSET=1.6,        # how far off the axis the load rides as it climbs (0.35 of the radius, as the brief's 2 in of 6)
    ),
    "5gal": dict(
        name="5 gal", H=14.5, R_RIM=6.2, R_TOP=11.9 / 2, R_BOTTOM=10.33 / 2, RIM_BAND=0.75, WALL=0.09,
        BELOW_RIM=5.2, BATTEN_L=12.0, LOAD_OFFSET=2.0,
    ),
}
BUCKET = "2gal"
B = BUCKETS[BUCKET]
BUCKET_H, R_RIM, R_TOP, R_BOTTOM = B["H"], B["R_RIM"], B["R_TOP"], B["R_BOTTOM"]
RIM_BAND, WALL = B["RIM_BAND"], B["WALL"]

# -- the brief's rules ---------------------------------------------------
PLY = 0.75                  # 3/4 in plywood: base, deck, motor plate, gussets
RIM_CLEAR = 1.3             # the rim clears the deck by about this (brief)
TILT = 25.0                 # degrees, set by the prop leg
BASE_BEHIND = 1.5           # how far the base runs past the hinge, under the motor
GUSSET = 4.0                # the gussets' legs
CONTACT = 35.0              # degrees each side of bottom centre where the wheels touch (brief)
CASTER_DROP = 1.5           # 2 in caster: mounting plate to axle (fit: measure yours)
RISER_W = 3.0               # caster riser's width along the deck; its thickness makes up the axle height
DISC = (4.0, 0.5)           # bottom discs: diameter, thickness (1/2 ply)
BOLT_CIRCLE = 3.0           # the four 1/4 carriage bolts through discs and bucket bottom
BOLT_HOLE = 0.28
BATTEN = (0.75, B["BATTEN_L"])   # section, length; three at 120 deg
LOAD_KG = 1.5               # pieces plus media, worst case (brief)
LOAD_OFFSET = B["LOAD_OFFSET"]
MOTOR_TORQUE_KGCM = 20.0    # the motor to buy, rated torque at least this
FILL = 1 / 3                # pieces and media fill under a third of the bucket (brief)
MEDIA_PER_PIECE = 3.0       # media volume per volume of pieces (brief)


def up_to_half(x):
    return math.ceil(x * 2) / 2


AXIS_H = up_to_half(R_RIM + RIM_CLEAR)     # bucket axis above the deck's top face
PLATE_H = AXIS_H + 2.5                       # motor plate height above the deck
WIDTH = up_to_half(2 * R_RIM + 1.5)          # deck, plate and base width

# -- bought parts: envelopes from listings ------------------------------
GEARBOX = (58 * MM, 40 * MM, 35 * MM)   # 5840-31ZY worm gearbox: along z, along y, depth along x (listings)
MOTOR_CAN = (31 * MM, 57 * MM)          # its motor: diameter, length (listings)
SHAFT = (8 * MM, 15 * MM)               # output D-shaft: diameter, length out of the gearbox face (length NOT in listings: fit)
POCKET_CLEAR = 0.02                     # all round the gearbox in its pocket
POCKET = 0.5                            # the gearbox sits this deep in a pocket in the plate's back, so the short shaft reaches through
SHAFT_HOLE = 0.5
HUB = (15 * MM, 12 * MM)                # rigid flange coupling, 8 mm bore: hub diameter, length
FLANGE = (32 * MM, 5 * MM)              # its flange: diameter, thickness (check the listing)
HUB_GAP = 1 * MM                        # between the plate's inner face and the hub
WHEEL = (2.0, 0.75)                     # 2 in fixed caster: wheel diameter, tread width
CASTER_PLATE = (1.9, 2.5, 0.1)          # mounting plate: along x, along y, thick
CHEEK = 0.08                            # fork cheeks' thickness
AXLE = 5 / 16

# -- materials (g/cm3) ---------------------------------------------------
PLYWOOD = ("birch plywood", 0.68)
HDPE = ("HDPE", 0.95)
MAPLE = ("hard maple", 0.71)
STEEL = ("steel", 7.85)
RUBBER = ("polyurethane", 1.2)
MOTOR = ("gearmotor (envelope, ~0.5 kg real)", 1.6)

# -- derived -------------------------------------------------------------
DECK_TOP = -AXIS_H                                  # z of the deck's top face in the deck frame
PLATE_IN = PLY                                      # x of the motor plate's inner face
HUB_X = PLATE_IN + HUB_GAP
FLANGE_X = HUB_X + HUB[1]                           # the flange's face against the outer disc
OUTER_DISC_X = FLANGE_X + FLANGE[1]
FLOOR_X = OUTER_DISC_X + DISC[1]                    # the bucket floor's outside face
RIM_X = FLOOR_X + BUCKET_H
CASTER_X = RIM_X - B["BELOW_RIM"]
CASTER_LINE = CASTER_X - PLATE_IN                  # caster axles from the motor plate's inner face
DECK = (up_to_half(RIM_X + 2.0), WIDTH)            # deck: 2 in past the rim
BASE = (DECK[0] + 2.0, WIDTH)
TAPER = (R_TOP - R_BOTTOM) / (BUCKET_H - RIM_BAND)  # radius per inch up the wall


def r_out(x):
    """The wall's outside radius at deck station x."""
    return R_BOTTOM + TAPER * (x - FLOOR_X)


# Each wheel touches the wall on its outer (rim-side) edge, where the
# tapered wall is fattest, so it touches along that edge and clears
# everywhere else.
WHEEL_REACH = r_out(CASTER_X + WHEEL[1] / 2) + WHEEL[0] / 2     # bucket axis to wheel axle
WHEEL_Y = WHEEL_REACH * math.sin(math.radians(CONTACT))
WHEEL_Z = -WHEEL_REACH * math.cos(math.radians(CONTACT))
RISER = (RISER_W, WHEEL_Z - CASTER_DROP - DECK_TOP)   # the riser's thickness puts the axles where the wheels touch
assert RISER[1] >= 0.25, f"casters too tall for this bucket: riser {RISER[1]:.2f} in"
RISER_TOP = DECK_TOP + RISER[1]
GEARBOX_FACE = POCKET                                # the gearbox's shaft face, on the pocket floor
SHAFT_END = GEARBOX_FACE + SHAFT[1]


def v(x, y, z=None):
    return (x * IN, y * IN) if z is None else (x * IN, y * IN, z * IN)


class Studio(Part):
    """A Part that takes inches."""

    def sk(self, base, offset, name):
        return self.sketch(base, offset * IN, name)

    def box(self, name, x, y, z, material, op="new"):
        """An axis-aligned block from (x0, x1), (y0, y1), (z0, z1)."""
        s = self.sk("top", z[0], name)
        self.rect(s, v(x[0], y[0]), v(x[1], y[1]))
        f = self.extrude(s, (z[1] - z[0]) * IN, op=op, name=name)
        if op == "new":
            self.name_it(f, name, material)
        return f

    def rod(self, name, x, yz, d, material, op="new"):
        """A cylinder along x from x[0] to x[1], centred on (y, z)."""
        s = self.sk("right", x[0], name)
        self.circle(s, v(*yz), d / 2 * IN)
        f = self.extrude(s, (x[1] - x[0]) * IN, op=op, name=name)
        if op == "new":
            self.name_it(f, name, material)
        return f

    def name_it(self, f, name, material):
        self.density = getattr(self, "density", {})
        self.density[name] = material[1]
        self.apply({"type": "rename_part", "source": f, "name": name},
                   {"type": "set_part_material", "source": f, "material": {"name": material[0], "density": material[1]}})


def deck_and_plate(p):
    p.box("deck", (0, DECK[0]), (-DECK[1] / 2, DECK[1] / 2), (DECK_TOP - PLY, DECK_TOP), PLYWOOD)
    f = p.box("motor plate", (0, PLY), (-DECK[1] / 2, DECK[1] / 2), (DECK_TOP, DECK_TOP + PLATE_H), PLYWOOD)
    # The pocket for the gearbox, from the back, and the shaft hole through what is left.
    s = p.sk("right", 0.0, "gearbox pocket")
    c = POCKET_CLEAR
    p.rect(s, v(-GEARBOX[1] / 2 - c, -GEARBOX[0] / 2 - c), v(GEARBOX[1] / 2 + c, GEARBOX[0] / 2 + c))
    p.cut(s, POCKET * IN, name="gearbox pocket")
    s = p.sk("right", 0.0, "shaft hole")
    p.circle(s, (0.0, 0.0), SHAFT_HOLE / 2 * IN)
    p.cut(s, PLY * IN, name="shaft hole")
    # Gussets on the bucket side, at the plate's ends, square to the deck.
    for side, off in (("left", -DECK[1] / 2), ("right", DECK[1] / 2 - PLY)):
        s = p.sk("front", -(off + PLY), f"gusset {side}")
        p.polygon(s, [v(PLY, DECK_TOP), v(PLY + GUSSET, DECK_TOP), v(PLY, DECK_TOP + GUSSET)])
        g = p.extrude(s, PLY * IN, name=f"gusset {side}")
        p.name_it(g, f"gusset {side}", PLYWOOD)
    return f


def casters(p):
    p.box("caster riser", (CASTER_X - RISER[0] / 2, CASTER_X + RISER[0] / 2), (-DECK[1] / 2, DECK[1] / 2),
          (DECK_TOP, RISER_TOP), PLYWOOD)
    for side, y in (("left", -WHEEL_Y), ("right", WHEEL_Y)):
        n = f"caster {side}"
        top = RISER_TOP + CASTER_PLATE[2]
        f = p.box(n, (CASTER_X - CASTER_PLATE[0] / 2, CASTER_X + CASTER_PLATE[0] / 2),
                  (y - CASTER_PLATE[1] / 2, y + CASTER_PLATE[1] / 2), (RISER_TOP, top), STEEL)
        for x0 in (CASTER_X - WHEEL[1] / 2 - 0.06 - CHEEK, CASTER_X + WHEEL[1] / 2 + 0.06):
            p.box(f"{n} cheek", (x0, x0 + CHEEK), (y - 0.5, y + 0.5), (top, WHEEL_Z + 0.4), STEEL, op="add")
        p.rod(f"{n} axle", (CASTER_X - WHEEL[1] / 2 - 0.2, CASTER_X + WHEEL[1] / 2 + 0.2), (y, WHEEL_Z), AXLE, STEEL, op="add")
        p.rod(f"{n} wheel", (CASTER_X - WHEEL[1] / 2, CASTER_X + WHEEL[1] / 2), (y, WHEEL_Z), WHEEL[0], RUBBER)


def bucket(p):
    """The bucket's half-section above the axis on the front plane
    (sketch x = world x, sketch y = world z = radius), revolved about
    the axis. The bolt holes go through its floor."""
    s = p.sk("front", 0.0, "bucket section")
    band = RIM_X - RIM_BAND
    p.polygon(s, [v(FLOOR_X, 0), v(FLOOR_X, R_BOTTOM), v(band, r_out(band)), v(band, R_RIM), v(RIM_X, R_RIM),
                  v(RIM_X, r_out(band) - WALL), v(FLOOR_X + WALL, R_BOTTOM - WALL), v(FLOOR_X + WALL, 0)])
    f = p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "x_axis"}, "angle": 360, "op": "new", "name": "bucket"})
    p.name_it(f, f"bucket, {B['name']}", HDPE)
    return f


def bottom_flange(p):
    p.rod("outer disc", (OUTER_DISC_X, FLOOR_X), (0, 0), DISC[0], PLYWOOD)
    p.rod("inner disc", (FLOOR_X + WALL, FLOOR_X + WALL + DISC[1]), (0, 0), DISC[0], PLYWOOD)
    # The coupling: flange and hub as two bodies made one, so the hub
    # cannot union with anything else it touches; then its bore.
    f = p.rod("flange coupling", (FLANGE_X, OUTER_DISC_X), (0, 0), FLANGE[0], STEEL)
    h = p.rod("flange coupling hub", (HUB_X, FLANGE_X + 0.05), (0, 0), HUB[0], STEEL)
    p.feature({"type": "add_boolean", "op": "union", "targets": [f], "tools": [h], "name": "coupling in one"})
    s = p.sk("right", HUB_X, "coupling bore")
    p.circle(s, (0.0, 0.0), (SHAFT[0] / 2 + 0.004) * IN)
    p.cut(s, (FLANGE_X - HUB_X + 0.01) * IN, name="coupling bore")
    s = p.sk("right", OUTER_DISC_X - 0.01, "carriage bolt holes")
    for k in range(4):
        a = math.radians(45 + 90 * k)
        p.point(s, v(BOLT_CIRCLE / 2 * math.cos(a), BOLT_CIRCLE / 2 * math.sin(a)))
    p.hole(s, BOLT_HOLE * IN, depth=(DISC[1] * 2 + WALL + 0.02) * IN, direction="normal", name="carriage bolt holes")


def battens(p):
    """One batten on the wall above the axis, its outer face on the
    tapered wall, then two more at 120 deg about the axis."""
    band = RIM_X - RIM_BAND
    xa, ra = FLOOR_X + WALL, R_BOTTOM - WALL       # the inside wall, as bucket() draws it
    xb, rb = RIM_X, r_out(band) - WALL
    half = BATTEN[0] / 2

    def r_in(x):
        """Where the batten's flat back sits: on the wall at its two
        edges, the wall's sagitta over its width clear in the middle."""
        r = ra + (rb - ra) * (x - xa) / (xb - xa)
        return math.sqrt(r * r - half * half) - 0.01   # and a facet's sagitta: the revolve is faceted

    x0, x1 = FLOOR_X + WALL, FLOOR_X + WALL + BATTEN[1]
    for k in range(3):
        n = f"batten {k + 1}"
        # The front plane turned about the bucket's axis (world x): its
        # sketch y is the radius toward this batten.
        s = p.feature({"type": "add_sketch", "name": n, "plane": {"type": "rotated", "base": "front", "axis": "x",
                                                                  "angle": 120.0 * k, "offset": -BATTEN[0] / 2 * IN}})
        p.polygon(s, [v(x0, r_in(x0) - BATTEN[0]), v(x1, r_in(x1) - BATTEN[0]), v(x1, r_in(x1)), v(x0, r_in(x0))])
        f = p.extrude(s, BATTEN[0] * IN, name=n)
        p.name_it(f, n, MAPLE)


def motor(p):
    """The gearmotor behind the plate: its gearbox in the pocket, the
    can along the gearbox and down, the shaft forward through the plate
    into the coupling's hub."""
    gb_back = GEARBOX_FACE - GEARBOX[2]
    f = p.box("gearmotor", (gb_back, GEARBOX_FACE), (-GEARBOX[1] / 2, GEARBOX[1] / 2), (-GEARBOX[0] / 2, GEARBOX[0] / 2), MOTOR)
    # The can hangs below the gearbox, behind the plate (the real one
    # is offset to the back of the gearbox the same way).
    s = p.sk("top", -GEARBOX[0] / 2 - MOTOR_CAN[1], "motor can")
    p.circle(s, v(-MOTOR_CAN[0] / 2 - 0.05, 0), MOTOR_CAN[0] / 2 * IN)
    p.extrude(s, (MOTOR_CAN[1] + 0.1) * IN, op="add", name="motor can")
    # The shaft reaches through the plate, whose bounding box an "add"
    # would union it with; so a new body, joined to the gearbox by name.
    sh = p.rod("output shaft", (GEARBOX_FACE - 0.05, SHAFT_END), (0, 0), SHAFT[0], STEEL)
    p.feature({"type": "add_boolean", "op": "union", "targets": [f], "tools": [sh], "name": "shaft into the gearbox"})
    return f


def base(p):
    """The world frame: the base's top at z = 0, the hinge on the y axis."""
    p.box("base", (-BASE_BEHIND, BASE[0] - BASE_BEHIND), (-BASE[1] / 2, BASE[1] / 2), (-PLY, 0), PLYWOOD)
    s = p.sk("front", -1.5, "hinge")
    p.circle(s, (0.0, 0.0), 0.15 * IN)
    f = p.extrude(s, 3.0 * IN, name="hinge pin")
    p.name_it(f, "hinge", STEEL)
    # The prop leg stands under the deck's far end; its top is cut to the deck's underside.
    t = math.tan(math.radians(TILT))
    far = DECK[0] * math.cos(math.radians(TILT))
    x1 = far - 0.2
    x0 = x1 - PLY
    s = p.sk("front", -1.5, "prop leg")
    p.polygon(s, [v(x0, 0), v(x1, 0), v(x1, x1 * t), v(x0, x0 * t)])
    f = p.extrude(s, 3.0 * IN, name="prop leg")
    p.name_it(f, "prop leg", PLYWOOD)
    return x0 * t, x1 * t


def placed():
    """Deck frame to world: turned -TILT about y (the far end rises),
    then moved so the hinge line (deck x = 0, z = deck bottom) lands on
    the world y axis."""
    th = math.radians(-TILT)
    hz = (DECK_TOP - PLY) * IN
    # R_y(th) applied to (0, 0, hz): x' = hz sin th, z' = hz cos th
    return {"position": {"x": -hz * math.sin(th), "y": 0.0, "z": -hz * math.cos(th)},
            "rotation": {"x": 0.0, "y": -TILT, "z": 0.0}}


def show(p, label):
    r = p.report()
    bad = [f["name"] for f in r["features"] if f.get("error")]
    assert not bad, bad
    for b in r["bodies"]:
        lo, hi = b["bounds"]
        print(f"{label}: {b['name']:<22} x {lo['x'] * MM:6.2f}..{hi['x'] * MM:6.2f}  y {lo['y'] * MM:6.2f}..{hi['y'] * MM:6.2f}"
              f"  z {lo['z'] * MM:6.2f}..{hi['z'] * MM:6.2f} in  {b['volume'] / 1000:8.1f} cm3")
    return r["bodies"]


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "bucket_tumbler.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "bucket_tumbler"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Bucket parts tumbler"},
                               {"type": "rename_tab", "tab": 1, "name": "on the deck"},
                               {"type": "add_part_studio", "name": "base"},
                               {"type": "add_assembly", "name": f"propped at {TILT:g} deg"}]})
    p = Studio(mcp, 1, "on the deck")
    deck_and_plate(p)
    casters(p)
    bucket(p)
    bottom_flange(p)
    battens(p)
    motor(p)
    deck_bodies = show(p, "deck")
    b = Studio(mcp, 2, "base")
    leg = base(b)
    base_bodies = show(b, "base")

    pose = placed()
    ops = [{"type": "add_instance", "studio": 2, "body": i, "name": bb["name"], "fixed": True,
            "placement": {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}}
           for i, bb in enumerate(base_bodies)]
    ops += [{"type": "add_instance", "studio": 1, "body": i, "name": bb["name"], "fixed": True, "placement": pose}
            for i, bb in enumerate(deck_bodies)]
    apply(mcp, ops, 3)

    # -- the brief's checks, from the model ------------------------------
    g = 9.81
    turning = [bb for bb in deck_bodies if bb["name"] in (f"bucket, {B['name']}", "inner disc", "outer disc", "flange coupling")
               or bb["name"].startswith("batten")]
    turning_kg = sum(bb["volume"] / 1000 * p.density[bb["name"]] for bb in turning) / 1000
    torque_nm = LOAD_KG * g * LOAD_OFFSET * IN / 1000
    axial_kgf = (turning_kg + LOAD_KG) * math.sin(math.radians(TILT))
    gap_rim = AXIS_H - R_RIM

    # Room: the inside as a frustum, a third of it for pieces and media,
    # a quarter of that the pieces.
    def inside_litres(bk):
        h = bk["H"] - bk["WALL"]
        r1, r2 = bk["R_BOTTOM"] - bk["WALL"], bk["R_TOP"] - bk["WALL"]
        return math.pi * h / 3 * (r1 * r1 + r1 * r2 + r2 * r2) * IN ** 3 / 1e6

    # Speed: the same tumble at the same fraction of the speed at which
    # the load would stick to the wall, which goes as 1 / sqrt(diameter);
    # the brief's 25-35 rpm was for the 5-gallon.
    def mean_d(bk):
        return bk["R_BOTTOM"] + bk["R_TOP"] - 2 * bk["WALL"]

    def critical_rpm(bk):
        return 42.3 / math.sqrt(mean_d(bk) * IN / 1000)

    speed = math.sqrt(mean_d(BUCKETS["5gal"]) / mean_d(B))
    litres = inside_litres(B)
    numbers = {
        "units": "inches unless named",
        "axis_above_deck": AXIS_H,
        "rim_clearance": gap_rim,
        "floor_x": FLOOR_X, "rim_x": RIM_X, "caster_x": CASTER_X,
        "wall_radius_at_casters": r_out(CASTER_X),
        "rim_to_casters": RIM_X - CASTER_X,
        "wheel_reach": WHEEL_REACH,
        "wheel_spacing": 2 * WHEEL_Y,
        "axle_above_deck": WHEEL_Z - DECK_TOP,
        "axle_above_riser": WHEEL_Z - RISER_TOP,
        "axis_above_axles": -WHEEL_Z,
        "contact_deg": CONTACT,
        "wheel_d": WHEEL[0], "wheel_w": WHEEL[1],
        "r_bottom": R_BOTTOM, "taper": TAPER, "wall": WALL,
        "shaft_end": SHAFT_END, "hub_x": HUB_X, "flange_x": FLANGE_X, "plate_in": PLATE_IN,
        "shaft_into_hub": SHAFT_END - HUB_X, "hub_len": HUB[1],
        "deck": DECK, "ply": PLY, "tilt_deg": TILT,
        "prop_leg_top": leg,
        "far_end_rise": DECK[0] * math.sin(math.radians(TILT)),
        "batten": BATTEN,
        "torque_nm": torque_nm,
        "torque_kgcm": torque_nm / g * 100,
        "motor_margin": MOTOR_TORQUE_KGCM / (torque_nm / g * 100),
        "turning_kg_model": turning_kg,
        "axial_kgf": axial_kgf,
        "bucket": B["name"], "bucket_h": BUCKET_H, "r_rim": R_RIM, "below_rim": B["BELOW_RIM"],
        "plate": (WIDTH, PLATE_H), "riser": RISER, "caster_drop": CASTER_DROP,
        "inside_litres": litres, "fill_litres": litres * FILL,
        "pieces_litres_max": litres * FILL / (1 + MEDIA_PER_PIECE),
        "critical_rpm": critical_rpm(B),
        "rpm": (25 * speed, 35 * speed),
    }
    with open(os.path.join(OUT, "bucket_tumbler.json"), "w") as fh:
        json.dump(numbers, fh, indent=1)
    for k, val in numbers.items():
        print(f"  {k}: {val if not isinstance(val, float) else round(val, 3)}")

    shot = lambda tab, view, name, **kw: mcp.call("screenshot", {"tab": tab, "view": view, "width": 1400, "height": 1000,
                                                                  "path": os.path.join(OUT, name), **kw})
    shot(3, "0.9,-0.8,0.6", "tumbler.png")
    shot(3, "front", "tumbler_side.png")
    shot(1, "front", "deck_section.png", section="y:0")
    shot(1, "right", "casters_section.png", section=f"x:{CASTER_X * IN:.2f}")
    shot(1, "-0.8,-0.6,0.5", "motor_end.png")
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "views": ["front", "top", "right", "iso"],
                              "note": f"deck flat; bucket axis {AXIS_H:g} in above the deck; casters {CASTER_LINE:g} in from the plate, "
                                      f"{2 * WHEEL_Y:.2f} in apart, axles {WHEEL_Z - DECK_TOP:.2f} in above the deck",
                              "path": os.path.join(OUT, "deck.pdf")}))


if __name__ == "__main__":
    main()
