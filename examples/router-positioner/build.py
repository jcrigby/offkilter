#!/usr/bin/env python3
"""A two-axis router positioner: boxes sliding in boxes, a screw down the middle of each.

BRIEF.md is the design brief this is built from. A trim router lies on
its side in a square tube, the quill, which slides along the bit's axis
(X, plunge) through a sleeve in a second box, the Z stage, which slides
up and down (Z, bit height) in a fixed outer box. Each axis is one lead
screw on the centre of the box it moves, its nut in that box and its
motor fixed to the end cap of a column: the X column an extension of
the sleeve out of the Z stage's right side, the Z column under the
outer box.

This is the first pass the brief asks for: proportions derived from a
few numbers at the top, the brief's rules enforced, the four corners of
the travel checked, and the envelope reported, so the trade-offs can be
looked at before anything is detailed. What is a guess says so.

It is made of plywood, as the owner asked: every box is flat panels of
1/2 in or 3/4 in sheet, and the build writes the cut list. There is no
joinery in it: every joint is one panel's edge on another's face, glued
and nailed with an air brad nailer, and nothing is grooved, rebated or
let in. What is not plywood is thin or bought.

    cargo build --release -p ok-mcp
    python3 examples/router-positioner/build.py
    cargo test -p ok-render --test router_positioner

Writes out/positioner.okpart (every part in its own tab, and the module
assembled at the four corners of its travel), positioner.png,
section-home.png and section-plunged-down.png, three pictures of the
router's mount (mount-land.png, mount-stops.png, mount-side.png),
positioner.pdf and positioner.json, cutlist.csv, the panels to saw, and
clips/*.stl, printed caps that hold two panels square while they are
nailed, with clips.png.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")

# -- the numbers to set -------------------------------------------------------
# The router: the owner's Bosch Colt, out of its base (the brief's first open question). What is marked measured is by
# the owner's callipers on the router. The rest is read off three photographs the owner took for the purpose. The nose
# from the side, scaled by the body and checked against the router's own millimetre scale: good to half a millimetre.
# The whole router from the side, lying on the measuring sheet, with the camera's place worked out from the sheet's
# grid so that the router's height off the paper is allowed for: good to 2 mm. And the rear end on, for the shape the
# quill has to pass. Lengths are from the rim: the front edge of the aluminium ring, the end the spindle comes out of.
ROUTER_D = 70.71                # the aluminium ring at the front, across. Measured
RING_L = 18.0                   # and how far back it goes, the spindle lock's button flush in its side. Photo
PLASTIC_D = 69.0                # the plastic body behind the ring: the metal stands a little proud of it. Measured
ROUTER_HOUSING = 51.0           # how far back the body stays round and bare, to the nearest edge of the rubber grip, which is on the cord's side. Measured; the photo had 54, and 64 on the other side
ROUTER_GRIP = (76.5, 105.0)     # the grip behind that: across, and where it ends
ROUTER_CAP = 76.0               # the vented cap behind the grip, across: a true circle from the rear but for the two things below
HUMP = (101.0, 119.0, 22.0, 44.5)   # the speed dial's housing, on the side the spindle lock's button is on: from and to behind the rim, half its width, and how far it stands from the axis
CHIN = (88.0, 130.0, 15.0, 45.5)    # and opposite it the swelling the cord comes out of, the same four. From one to the other is 90, measured; how that is shared between them is the photo's, and the chin's width is a guess
ROUTER_ENV = 96.0               # what the quill has to pass, twice 48: the chin's 45.5 and 2.5 for not knowing how the 90 is shared
ROUTER_L = 156.0                # to its rear end; with the nose and the nut, 186
NOSE_D, NOSE_L = 44.0, 14.2     # the bearing's boss, in front of the rim
COLLET_D, COLLET_L = 20.0, 12.5  # the collet nut, 17 across its flats
NUT_FACE = 30.0                 # the collet nut's face, in front of the rim, tightened on a 1/4 in bit. Measured; with no bit in it and the nut slack, as photographed, 34.5
SHAFT_D, SHAFT_L = 13.0, NUT_FACE - NOSE_L - COLLET_L   # what shows of the spindle between the boss and the nut: 3.3, with its two flats
FLATS = 9.49                    # across those flats. Measured: a 3/8 in wrench, which is 9.525
SHANK_IN = 32.0                 # how far a 1/4 in shank goes in from the nut's face before it bottoms. Measured
CORD_AT, CORD_D, CORD_L = 124.0, 18.0, 35.0  # the cord's boot leaves the chin this far back, opposite the dial's hump: where it is through the quill's wall, across, long
CORD_SLACK = 8.0                # that place is good to 3 mm: the slots run this much further each way than it needs
SPRUNG = (18.0, 10.0, 9.4)      # a sprung block at the rim, on the cord's side and opposite the spindle lock's button: wide (a guess, no photo shows it across), thick, and how far it stands out of the rim
SPRUNG_TURN = 0.0               # and how far round from the cord it is, degrees
FACE_PROUD = 3.2                # three screw heads at the rim stand out this much
BIT_D, BIT_L = 6.35, 40.0       # a quarter-inch bit past the nut, for the pictures: the collet is a quarter inch

X_TRAVEL = 80.0                 # plunge. The owner's choice: more than the stick sled uses, so a bit may sit deeper or shallower in the collet, and for whatever the positioner is put to next
Z_TRAVEL = 80.0                 # bit height. Open too

# The plywood. Two thicknesses and no other: nominal here. Sheet goods run thin, so put in what the sheets measure,
# and everything follows.
T12 = 12.7                      # 1/2 in: the sliding tubes, the motor housings, the nut cages, and the outer box's bit-side face
T34 = 19.05                     # 3/4 in: the rest of the outer box, the stage's two cheeks, the bar the X nut is on, the router's collars
SHEET = (1220.0, 2440.0)        # a sheet, for saying how much of one the panels take

GAP = 3.2                       # between a box and its guide all round: 1/8 in, the thickness of the strips that lie in it. Nothing is grooved for them
STRIP_L = 25.0                  # a bearing strip along the travel, one near each end of a box: 1/8 in thick, glued on
GIB = (GAP - 0.2, 19.05)        # a gib: a length of 1/8 x 3/4 in bar loose in the gap, held by its set screws' points in dimples drilled in its back
SET_SCREW = 4.2                 # an M5 set screw's hole, three to a gib: a threaded insert in plywood
VENT = 25.0                     # an air hole: a drill's, not a window
ENGAGE = 1.5                    # a box is in its guide at least this many of its own widths, everywhere in its travel
RAIL = T34                      # the Z stage is this much wider than the tube each side: its cheeks, whose edges ride the outer box's right wall

SCREW_D, SCREW_PITCH = 8.0, 1.25  # M8 rod. Open: a T8 lead screw of 2 mm lead goes in the same holes
NUT_AF, NUT_L = 13.0, 24.0      # an M8 coupling nut, DIN 6334: across flats, long. Brass, or it galls
NUT_FLOAT = 0.5                 # the nut's play across its cage, so a bent rod does not bind
NUT_LEAD = 3.0                  # the screw stands this far through the nut at the far end of the travel
TIP_CLEAR = 4.0                 # and its tip stops this far short of what it is run toward
BEARING = (16.0, 5.0)           # a thrust bearing, F8-16M: across, thick. On a steel washer: plywood would crush under it
COLLAR_L = 8.0                  # a collar on the screw each side of the bulkhead
COUPLING = (19.0, 25.0)         # a rigid coupling, 5 to 8 mm
MOTOR = (42.3, 40.0, 24.0)      # NEMA 17: square, long, and its shaft. Assumed, as the brief does
BULKHEAD, CAP = T12, T12        # the panel the thrust bearing sits against, and the end cap the motor is on
SWITCH = (20.0, 10.0, 6.0)      # a limit switch

# Clips: printed caps that push onto the ends of two panels where they meet and hold them square until the joint is
# nailed. The owner's idea. An L for a corner, a T for an edge on the middle of a face.
CLIP_WALL = 3.2                 # a clip's walls
CLIP_REACH = 24.0               # how far along each panel a clip reaches past the other panel
CLIP_DEEP = 20.0                # how far onto the panels' ends it pushes
CLIP_PLATE = 2.0                # the plate across the ends, which is what joins its walls
CLIP_FIT = 0.4                  # a slot is this much wider than its sheet, so the sheet starts easily
CLIP_RIB = (1.2, 0.5, 3.0)      # ribs on one side of each slot: wide, high, and how far short of the slot's mouth they stop. They are the friction: 0.1 mm of crush
CLIPS = (("L", T12, T12, 8), ("L", T12, T34, 4), ("L", T34, T34, 4), ("T", T12, T12, 4), ("T", T12, T34, 4), ("T", T34, T34, 4))   # kind; an arm's sheet, or the stem's; the other arm's, or the bar's; how many to print

# The router's mount: a block of two thick layers glued up and bored once for the body. The front land is on the
# aluminium ring. The plastic behind the ring is 1.7 mm smaller across, and a land that has passed the ring cannot
# close on it, so a printed liner brings the plastic up to the ring's size and the back land is on that. A saw kerf
# opens the block's top and a bolt across the kerf in each layer closes it: the top corners are notched so that the
# bolts' heads and nuts have a face to bear on and room inside the quill. That is done on the bench. Block and router
# then go into the quill together from the front, as far as two stop plates glued inside it, and four screws through
# the block into inserts in the plates keep them there.
BLOCK_FIT = 0.4                 # the block is this much smaller across than the quill's bore
BODY_FIT = 0.6                  # and its bore this much larger than the body: the kerf closes on what is left
LINER = (0.5, 0.5, 0.1, 24.0)   # the liner: it starts this far behind the ring and stops this far short of the grip; its bore is this much over the plastic; and the gap in it, which goes over the slotted metal plate in the body's side
KERF = 3.0                      # a saw's
NOTCH = (14.0, 16.0)            # cut from each top corner: across and down
PINCH = (5.5, 5.0)              # the bolts across the kerf, M5 x 80 with a washer each end and a locking nut: their hole, and its centre over the top of the bore
STOP_W = 17.0                   # a stop plate: the thick plywood, the bore's whole height, this wide, on each side wall
STOP_RELIEF = 4.0               # and hollowed to a circle this much larger across than the grip, which passes between them
HOLD = (5.5, 8.0, 41.0, 25.0)   # the four screws, M5 x 60: their hole through the block, the hole for an M5 insert in a stop plate, and where they are from the axis, across and up or down

# -- what follows from them ---------------------------------------------------
QI = ROUTER_ENV + 2.0                           # the quill's bore: the router goes in from the front, its cap first
SLOT_W = QI + 2.0                               # the bit slot: wide enough for the router's block, so block and router come out through it
BLOCK = QI - BLOCK_FIT                          # the block, square
BLOCK_L = 2 * T34                               # and long
BLOCK_AT = 1.0                                  # its front face, behind the rim: the front land is on the ring
LINER_X = (RING_L + LINER[0], ROUTER_HOUSING - LINER[1])   # the liner, behind the rim: it is pushed back against the grip's edge
PINCH_Z = (ROUTER_D + BODY_FIT) / 2 + PINCH[1]  # the bolts across the kerf, over the axis
STOP_AT = BLOCK_AT + BLOCK_L                    # the stop plates' front faces, behind the rim: the block's back
QO = QI + 2 * T12                               # the quill, a square tube of the thin plywood
SB = QO + 2 * GAP                               # the bore of the tube it slides in: sleeve, column and X motor housing in one
CO = SB + 2 * T12                               # that tube, outside
CAGE = 2 * T12                                  # a nut's cage: two layers of the thin plywood between two keepers. The nut, 24 long, floats in it
NUT_END = (CAGE - NUT_L) / 2
FOOT = T12 + CAGE + T12                         # the stage's foot: keeper, cage, keeper
REC = COLLAR_L + BEARING[1] + 3.0               # in a motor housing: the collar and bearing on the nut's side of the bulkhead
CHAMBER = COLLAR_L + 2.0 + COUPLING[1] + 0.5    # and the other collar and the coupling, which reaches the cap so the motor's shaft is well into it
DRIVE = REC + BULKHEAD + CHAMBER                # a motor housing, from where it starts to its cap

MX = CO                                         # the Z stage along the bit: as long as the tube is wide
MD = CO + 2 * RAIL                              # and across: the tube and a cheek each side
Z_NEED = (T12 + NUT_END) + NUT_L + NUT_LEAD + Z_TRAVEL + TIP_CLEAR - FOOT   # the Z screw's room over the foot: what its tip needs with the stage down
MH = max(T12 + SB + T12 + Z_NEED + FOOT, float(math.ceil(ENGAGE * max(MX, MD))))   # the stage: the tube, the screw's room, the foot; and tall enough for its width
Z_CLEAR = MH - (2 * T12 + SB + FOOT)
IH = MH + Z_TRAVEL + 2.0 + SWITCH[2]            # the outer box inside: the stage, its travel, a millimetre each end, and the home switch under the lid

# Where things are with both axes at home: X drawn back, Z up. x along the bit from the sleeve's mouth, the bit pointing to -x;
# y front (-) to back (+) from the bit's axis; z up from the outer box's floor.
ZB = Z_TRAVEL + 1.0                             # the stage's foot
AZ = ZB + MH - T12 - SB / 2                     # the bit's axis
XQ = X_TRAVEL                                   # the quill's front: at full plunge it is level with the sleeve's mouth
X_ROUTER_BACK = XQ + ROUTER_L
X_TIP = X_ROUTER_BACK + TIP_CLEAR               # the X screw's tip: it does not move
X_NUT = X_TIP + NUT_LEAD + X_TRAVEL             # the X nut's near end, drawn back
X_SPIDER = X_NUT - NUT_END - T34                # the bar across the quill's back that the nut's cage is on
QL = X_SPIDER + T34 - XQ                        # the quill, so
X_CAGE_END = X_SPIDER + T34 + CAGE + T12        # the back of the cage's keeper
X_COL_END = X_CAGE_END + 3.0                    # where the X motor housing begins, in the same tube
X_TUBE_END = X_COL_END + DRIVE                  # the tube's end, where its cap goes on
X_END = X_TUBE_END + CAP + MOTOR[1]             # the X motor's far end
Z_NUT = ZB + T12 + NUT_END                      # the Z nut's lower end, up
Z_TIP = Z_NUT + NUT_L + NUT_LEAD                # the Z screw's tip
Z_END = -T34 - DRIVE - CAP - MOTOR[1]           # the Z motor's far end
BOX_X = (-GAP - T12, MX + GAP + T34)            # the outer box: its bit-side face is thin, so the collet nut comes out of it
BOX_Y = MD / 2 + GAP + T34
SLOT_Z = (AZ - Z_TRAVEL - SLOT_W / 2, AZ + SLOT_W / 2)
COL_SLOT_Z = (1.0 + MH - CO - 1.5, IH)          # the tube's way through the outer box's right wall
CORD_SLOT = CORD_D + 6.0                        # the cord's slots, across
CORD_BACK = CORD_AT + CORD_SLOT / 2 + CORD_SLACK   # the quill's slot, in its floor: from its front end to here. Open at the front, so the boot rides in it as the router goes in
CORD_X = (0.0, XQ + CORD_BACK)                  # the slot in the tube's floor: from its mouth to where the boot is with the quill drawn back
CORD_HANG = CORD_L - (CO / 2 - CHIN[3])         # the boot's end, below the tube's underside
CORD_NOTCH = max(CORD_HANG, 0.0) + 1.5 + 8.0    # the notch under the tube's slot in the right wall: that, the slot's clearance, and room for the cord to turn

PANELS = []                                     # every piece of plywood, as it is drawn: part, name, thickness, long, wide


def rules():
    """The brief's rules of thumb, as numbers; stops the build if one is broken."""
    found = {}
    # A box in its guide: 1.5 widths at least. The quill is guided by the tube's whole length; the stage by the outer box.
    found["x_engaged"] = QL / QO
    found["z_engaged"] = MH / max(MX, MD)
    # The quill's two bearing zones stay in their guides: the front one in the stage's length of the tube, the back one beyond it.
    front = (XQ - X_TRAVEL + 2.0, XQ + 2.0 + STRIP_L)
    back = (XQ + QL - 2.0 - STRIP_L - X_TRAVEL, XQ + QL - 2.0)
    found["front_strip_in_sleeve"] = min(front[0], MX - front[1])
    found["back_strip_in_column"] = min(back[0] - MX, X_COL_END - back[1])
    # The screws: through the nut at one end of the travel, short of what they run toward at the other.
    found["x_tip_to_router"] = X_TIP - X_ROUTER_BACK
    found["x_through_nut"] = (X_NUT - X_TRAVEL) - X_TIP
    found["z_tip_to_sleeve"] = (1.0 + FOOT + Z_CLEAR) - Z_TIP          # the stage down
    found["z_through_nut"] = Z_TIP - (Z_NUT + NUT_L)                   # the stage up
    # The tube in the outer box's right wall, the stage up and down.
    found["column_in_slot"] = min((1.0 + MH - CO) - COL_SLOT_Z[0], COL_SLOT_Z[1] - (ZB + MH))
    # At full plunge the quill's front is at the sleeve's mouth. The outer box's face is the thin plywood so that
    # the collet nut is still clear of it there, for a wrench; the router's nose stays in the slot.
    found["nose_proud"] = NOSE_L - (GAP + T12)
    found["collet_proud"] = NOSE_L + SHAFT_L - (GAP + T12)             # the nut's back face out of the box's face: the spindle's flats are in the slot's mouth
    found["sprung_in_wall"] = SPRUNG[2] - GAP            # the sprung block's end is this far into the face's thickness, in the slot
    # The mount. The block is on the bare round and passes the bit slot; the stop plates behind it pass the grip, the
    # dial's housing and the chin, and are in front of both; the screws have wood round them in block and plate.
    found["block_on_body"] = min(BLOCK_AT, ROUTER_HOUSING - (BLOCK_AT + BLOCK_L))
    found["front_land_on_ring"] = min(RING_L, BLOCK_AT + T34) - BLOCK_AT
    found["back_land_on_liner"] = min(LINER_X[1], BLOCK_AT + BLOCK_L) - max(LINER_X[0], BLOCK_AT + T34)
    found["liner_wall"] = (ROUTER_D - PLASTIC_D - LINER[2]) / 2
    found["slot_past_block"] = (SLOT_W - BLOCK) / 2
    found["stop_round_grip"] = STOP_RELIEF / 2
    found["stop_beside_rear"] = QI / 2 - STOP_W - max(HUMP[2], CHIN[2])
    found["stop_before_rear"] = min(HUMP[0], CHIN[0]) - (STOP_AT + T34)
    r_hold = math.hypot(HOLD[2], HOLD[3])
    found["hold_in_block"] = min(r_hold - HOLD[0] / 2 - (ROUTER_D + BODY_FIT) / 2, BLOCK / 2 - HOLD[2] - HOLD[0] / 2, (BLOCK / 2 - NOTCH[1]) - HOLD[3] - HOLD[0] / 2)
    found["hold_in_stop"] = min(r_hold - (ROUTER_GRIP[0] + STOP_RELIEF) / 2, QI / 2 - HOLD[2], HOLD[2] - (QI / 2 - STOP_W)) - HOLD[1] / 2
    found["pinch_under_edge"] = BLOCK / 2 - PINCH_Z - PINCH[0] / 2
    found["pinch_seat"] = min(BLOCK / 2 - PINCH_Z, PINCH_Z - (BLOCK / 2 - NOTCH[1])) - 5.0          # a 10 mm washer on the notch's face
    found["rear_in_bore"] = QI / 2 - max(HUMP[3], CHIN[3])                                           # round the chin as photographed
    # The router goes in and comes out with its cord's boot in the two floors' slots and through the bit slot under
    # the tube: the stage has to be within this much of the top of its travel for the boot to pass the slot's end.
    found["boot_through_face"] = Z_TRAVEL + SLOT_W / 2 - (CHIN[3] + CORD_L)
    # The cord's boot comes down through the quill's floor and the tube's. At full plunge it is over the stage's own
    # room, at its open side. The tube's slot runs CORD_SLACK further each way than the boot goes, its back end under
    # where the quill's back strip runs: the strip is far wider than the slot and bears either side of it.
    boot = (XQ - X_TRAVEL + CORD_AT - CORD_D / 2, XQ + CORD_AT + CORD_D / 2)
    found["cord_past_stage"] = boot[0] - MX
    found["slot_under_strip"] = max(0.0, CORD_X[1] - back[0])
    found["cord_slot_short_of_strip"] = (QL - 2.0 - STRIP_L) - CORD_BACK                              # the quill's own slot and its own back strip
    found["strip_beside_slot"] = (QO - 8.0 - CORD_SLOT) / 2                                           # the back strip's bearing each side of the tube's slot
    found["motor_shaft_in_coupling"] = MOTOR[2] - CAP - 0.5                                           # through a plywood cap, what is left of the shaft
    bad = []
    if min(found["x_engaged"], found["z_engaged"]) < ENGAGE:
        bad.append(f"a box is under {ENGAGE} widths in its guide: {found['x_engaged']:.2f} along X, {found['z_engaged']:.2f} along Z")
    for key, least in (("front_strip_in_sleeve", 0.0), ("back_strip_in_column", 0.0), ("x_tip_to_router", 3.0), ("x_through_nut", 2.0), ("z_tip_to_sleeve", 3.0),
                       ("z_through_nut", 2.0), ("column_in_slot", 1.0), ("cord_slot_short_of_strip", 10.0), ("strip_beside_slot", 25.0),
                       ("collet_proud", 1.0), ("motor_shaft_in_coupling", 8.0), ("block_on_body", 1.0), ("front_land_on_ring", 15.0), ("back_land_on_liner", 15.0), ("liner_wall", 0.6), ("slot_past_block", 1.0), ("stop_round_grip", 1.5), ("stop_beside_rear", 5.0),
                       ("stop_before_rear", 5.0), ("hold_in_block", 4.0), ("hold_in_stop", 3.0), ("pinch_under_edge", 4.0), ("pinch_seat", 2.0), ("rear_in_bore", 2.0),
                       ("boot_through_face", 5.0)):
        if found[key] < least - 1e-9:
            bad.append(f"{key} is {found[key]:.1f}, under {least}")
    if bad:
        raise SystemExit("; ".join(bad))
    return found


# -- drawing ------------------------------------------------------------------

def box(p, name, x, y, z, op="new"):
    s = p.sketch("right", x[0], name)
    p.polygon(s, [(y[0], z[0]), (y[1], z[0]), (y[1], z[1]), (y[0], z[1])])
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def panel(p, part, name, x, y, z, layers=1):
    """A piece of plywood: drawn, and written down for the cut list. One of its three sizes is a sheet's thickness,
    or `layers` of them glued up."""
    size = [x[1] - x[0], y[1] - y[0], z[1] - z[0]]
    # its thickness: whichever of the three is a whole number of layers of one of the sheets, the smallest first
    thick = next((v for v in sorted(size) for t in (T12, T34) if abs(v - layers * t) < 1e-6), None)
    if thick is None:
        raise SystemExit(f"{part}, {name}: {sorted(size)} has no side that is {layers} of either plywood")
    size.remove(thick)
    sheet = T12 if abs(thick - layers * T12) < 1e-6 else T34
    for _ in range(layers):
        PANELS.append({"part": part, "name": name, "t": sheet, "long": max(size), "wide": min(size)})
    return box(p, name, x, y, z)


def tube(p, part, along, lo, hi, c, bore, t):
    """A square tube of four panels the same width, each lapping the next, about `c`: along x about (y, z), or
    along z about (x, y)."""
    b = bore / 2
    a = ((-b - t, b), (b, b + t)), ((b, b + t), (-b, b + t)), ((-b, b + t), (-b - t, -b)), ((-b - t, -b), (-b - t, b))
    names = ("lid", "back", "floor", "front") if along == "x" else ("right", "back", "left", "front")
    for name, (u, v) in zip(names, a):
        if along == "x":
            panel(p, part, name, (lo, hi), (c[0] + u[0], c[0] + u[1]), (c[1] + v[0], c[1] + v[1]))
        else:
            panel(p, part, name, (c[0] + v[0], c[0] + v[1]), (c[1] + u[0], c[1] + u[1]), (lo, hi))


def xcyl(p, name, x, c, d, op="new"):
    s = p.sketch("right", x[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def xhex(p, name, x, c, af, op="new", turn=0.0):
    s = p.sketch("right", x[0], name)
    r = af / 2 / math.cos(math.radians(30))
    p.polygon(s, [(c[0] + r * math.cos(math.radians(turn + 60 * k)), c[1] + r * math.sin(math.radians(turn + 60 * k))) for k in range(6)])
    return p.extrude(s, x[1] - x[0], op=op, name=name)


def zbox(p, name, x, y, z, op="new"):
    s = p.sketch("top", z[0], name)
    p.polygon(s, [(x[0], y[0]), (x[1], y[0]), (x[1], y[1]), (x[0], y[1])])
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def zcyl(p, name, z, c, d, op="new"):
    s = p.sketch("top", z[0], name)
    p.circle(s, c, d / 2)
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def zhex(p, name, z, c, af, op="new"):
    s = p.sketch("top", z[0], name)
    r = af / 2 / math.cos(math.radians(30))
    p.polygon(s, [(c[0] + r * math.cos(math.radians(60 * k)), c[1] + r * math.sin(math.radians(60 * k))) for k in range(6)])
    return p.extrude(s, z[1] - z[0], op=op, name=name)


def ybore(p, name, c, d, y_far, depth):
    """A hole along y from y_far toward the front, `depth` deep, at (x, z)."""
    s = p.sketch("front", -y_far, name)
    p.circle(s, c, d / 2)
    p.cut(s, depth, name=name)


def spread(lo, hi, n=3, inset=15.0):
    return [lo + inset + (hi - lo - 2 * inset) * k / (n - 1) for k in range(n)]


# The gibs: which wall of which guide each is let into. The opposite walls are the reference faces.
# The Z stage is pushed back (the feed pushes the bit that way) and to the right, against the right wall either side
# of the tube's slot (the router's weight and the plunge both tip it that way). The quill is pushed back and down.
# The two in the face bear on the left edges of the stage's cheeks, each straight across from where the cheek's
# right edge rides the right wall.
Z_GIBS = [("left", -(CO / 2 + T34 / 2)), ("left", CO / 2 + T34 / 2), ("front", MX / 2)]
Z_SWITCH_Y = (MD / 2 + GAP - 3.0 - SWITCH[1], MD / 2 + GAP - 3.0)
STRIP_Z = ((ZB + 2.0, ZB + 2.0 + STRIP_L), (ZB + MH - 2.0 - STRIP_L, ZB + MH - 2.0))      # the stage's bearing strips, one near each end
QUILL_STRIP_X = ((XQ + 2.0, XQ + 2.0 + STRIP_L), (XQ + QL - 2.0 - STRIP_L, XQ + QL - 2.0))


def outer_box(p):
    """Six panels. Floor and lid take the whole footprint, front and back stand between them the box's whole
    length, and the two ends stand between those: the bit-side face in the thin plywood, the right wall in the thick."""
    part, wy = "Outer box", MD / 2 + GAP
    panel(p, part, "floor", BOX_X, (-BOX_Y, BOX_Y), (-T34, 0.0))
    panel(p, part, "lid", BOX_X, (-BOX_Y, BOX_Y), (IH, IH + T34))
    panel(p, part, "front", BOX_X, (-BOX_Y, -wy), (0.0, IH))
    panel(p, part, "back", BOX_X, (wy, BOX_Y), (0.0, IH))
    panel(p, part, "face", (BOX_X[0], -GAP), (-wy, wy), (0.0, IH))
    panel(p, part, "right wall", (MX + GAP, BOX_X[1]), (-wy, wy), (0.0, IH))
    box(p, "bit slot", (BOX_X[0] - 1.0, -GAP + 1.0), (-SLOT_W / 2, SLOT_W / 2), SLOT_Z, op="remove")
    box(p, "the tube's way", (MX + GAP - 1.0, BOX_X[1] + 1.0), (-CO / 2 - 1.5, CO / 2 + 1.5), COL_SLOT_Z, op="remove")
    # With the stage down and the quill near full plunge the cord trails out under the tube, through this wall.
    box(p, "the cord's notch", (MX + GAP - 1.0, BOX_X[1] + 1.0), (-CORD_SLOT / 2, CORD_SLOT / 2), (COL_SLOT_Z[0] - CORD_NOTCH, COL_SLOT_Z[0] + 1.0), op="remove")
    zcyl(p, "the Z screw's way", (-T34 - 1.0, 1.0), (MX / 2, 0.0), SCREW_D + 1.0, op="remove")
    for wall, at in Z_GIBS:                     # the gibs' set screws: holes, and nothing else, in the walls the gibs lie against
        for z in spread(8.0, IH - 8.0):
            if wall == "left":
                xcyl(p, "set screw", (BOX_X[0] - 1.0, -GAP + 1.0), (at, z), SET_SCREW, op="remove")
            else:
                ybore(p, "set screw", (at, z), SET_SCREW, -wy + 1.0, T34 + 2.0)


def z_gibs(p):
    for wall, at in Z_GIBS:
        if wall == "left":
            box(p, "Z gib", (-GIB[0], 0.0), (at - GIB[1] / 2, at + GIB[1] / 2), (9.0, IH - 9.0))
        else:
            box(p, "Z gib", (at - GIB[1] / 2, at + GIB[1] / 2), (-MD / 2 - GIB[0], -MD / 2), (9.0, IH - 9.0))


def x_tube(p):
    """One square tube of the thin plywood from the sleeve's mouth to the X motor: the quill's guide through the
    stage, the column beyond it, and the motor's housing at its end, with a bulkhead let in for the thrust bearing
    and a cap for the motor. Its being one piece is what joins column to stage."""
    part = "X tube"
    tube(p, part, "x", 0.0, X_TUBE_END, (0.0, AZ), SB, T12)
    a = X_COL_END
    panel(p, part, "bulkhead", (a + REC, a + REC + BULKHEAD), (-SB / 2, SB / 2), (AZ - SB / 2, AZ + SB / 2))
    panel(p, part, "motor's cap", (X_TUBE_END, X_TUBE_END + CAP), (-CO / 2, CO / 2), (AZ - CO / 2, AZ + CO / 2))
    xcyl(p, "the screw's way", (a + REC - 1.0, a + REC + BULKHEAD + 1.0), (0.0, AZ), SCREW_D + 1.0, op="remove")
    xcyl(p, "motor's pilot", (X_TUBE_END - 0.5, X_TUBE_END + CAP + 0.5), (0.0, AZ), 22.5, op="remove")
    # the cord's slot, in the floor, from the mouth: the boot comes in along it with the router, and runs in it with the travel
    zbox(p, "the cord's slot", (CORD_X[0] - 1.0, CORD_X[1]), (-CORD_SLOT / 2, CORD_SLOT / 2), (AZ - CO / 2 - 1.0, AZ - SB / 2 + 1.0), op="remove")
    for sy in (-1, 1):                          # air in through the lid at the back, three drilled holes each side of the gib; it passes the nut's bar to the router
        for k in range(3):
            zcyl(p, "air hole", (AZ + SB / 2 - 1.0, AZ + CO / 2 + 1.0), (X_COL_END - 20.0 - 30.0 * k, sy * (SB / 2 - 6.0 - VENT / 2)), VENT, op="remove")
    # the quill's gibs lie in the gap over it and before it, where its front end runs and where its back end does:
    # all the tube has for them is their set screws' holes
    for lo, hi in X_GIB_X:
        for x in spread(lo, hi):
            zcyl(p, "set screw", (AZ + SB / 2 - 1.0, AZ + CO / 2 + 1.0), (x, 0.0), SET_SCREW, op="remove")
            ybore(p, "set screw", (x, AZ), SET_SCREW, -SB / 2 + 1.0, T12 + 2.0)


X_GIB_X = ((9.0, MX - 9.0), (XQ + QL - X_TRAVEL - STRIP_L - 9.0, X_COL_END - 9.0))


def z_stage(p):
    """What is glued to the tube to make the stage: a cheek of the thick plywood on its front and on its back,
    the stage's whole height, and between them at the bottom a foot of four thin layers with the Z nut caged in it.
    The cheeks' edges are the stage's bearing faces left and right."""
    part, top = "Z stage", ZB + MH
    panel(p, part, "front cheek", (0.0, MX), (-MD / 2, -CO / 2), (ZB, top))
    panel(p, part, "back cheek", (0.0, MX), (CO / 2, MD / 2), (ZB, top))
    slot = NUT_AF / 2 + NUT_FLOAT
    panel(p, part, "foot under the nut", (0.0, MX), (-CO / 2, CO / 2), (ZB, ZB + T12))
    panel(p, part, "foot beside the nut", (0.0, MX), (-CO / 2, -slot), (ZB + T12, ZB + T12 + CAGE), layers=2)
    panel(p, part, "foot beside the nut", (0.0, MX), (slot, CO / 2), (ZB + T12, ZB + T12 + CAGE), layers=2)
    panel(p, part, "foot over the nut", (0.0, MX), (-CO / 2, CO / 2), (ZB + T12 + CAGE, ZB + FOOT))
    zcyl(p, "the screw's way", (ZB - 1.0, ZB + FOOT + 1.0), (MX / 2, 0.0), SCREW_D + 1.0, op="remove")
    for x in spread(*X_GIB_X[0]):                    # the quill's front gib's set screws come through the front cheek too
        ybore(p, "set screw", (x, AZ), SET_SCREW, -CO / 2 + 1.0, T34 + 2.0)


def stage_strips(p):
    """The stage's bearing strips, one near each end of its height: on its cheeks' right edges, against the right
    wall either side of the tube's slot, and on its back cheek, against the back wall."""
    for z in STRIP_Z:
        for y in ((-MD / 2, -CO / 2 - 3.0), (CO / 2 + 3.0, MD / 2)):
            box(p, "strip on the right wall", (MX, MX + GAP), y, z)
        box(p, "strip on the back wall", (2.0, MX - 2.0), (MD / 2, MD / 2 + GAP), z)


def x_gibs(p):
    for x in X_GIB_X:
        box(p, "X gib, over the quill", x, (-GIB[1] / 2, GIB[1] / 2), (AZ + QO / 2, AZ + QO / 2 + GIB[0]))
        box(p, "X gib, before it", x, (-QO / 2 - GIB[0], -QO / 2), (AZ - GIB[1] / 2, AZ + GIB[1] / 2))


def quill(p):
    tube(p, "X quill", "x", XQ, XQ + QL, (0.0, AZ), QI, T12)
    # the cord's slot, in the floor from its front end: the boot rides in it as the router goes in
    zbox(p, "the cord's slot", (XQ - 1.0, XQ + CORD_BACK), (-CORD_SLOT / 2, CORD_SLOT / 2), (AZ - QO / 2 - 1.0, AZ - QI / 2 + 1.0), op="remove")
    # The stop plates, glued to the side walls behind where the block comes to: hollowed for the grip, and with the
    # inserts' holes that the block's four screws go into.
    b, x0 = QI / 2, XQ + STOP_AT
    for sy in (-1, 1):
        panel(p, "X quill", "stop plate", (x0, x0 + T34), tuple(sorted((sy * (b - STOP_W), sy * b))), (AZ - b, AZ + b))
    xcyl(p, "the grip's way", (x0 - 0.5, x0 + T34 + 0.5), (0.0, AZ), ROUTER_GRIP[0] + STOP_RELIEF, op="remove")
    for sy in (-1, 1):
        for sz in (-1, 1):
            xcyl(p, "insert", (x0 - 0.5, x0 + T34 + 0.5), (sy * HOLD[2], AZ + sz * HOLD[3]), HOLD[1], op="remove")


def quill_strips(p):
    for k, x in enumerate(QUILL_STRIP_X):       # the front one is in two, either side of the cord's slot
        for y in (((-QO / 2 + 4.0, QO / 2 - 4.0),) if k else ((-QO / 2 + 4.0, -CORD_SLOT / 2 - 2.0), (CORD_SLOT / 2 + 2.0, QO / 2 - 4.0))):
            box(p, "strip underneath", x, y, (AZ - QO / 2 - GAP, AZ - QO / 2))
        box(p, "strip behind", x, (QO / 2, QO / 2 + GAP), (AZ - QO / 2 + 4.0, AZ + QO / 2 - 4.0))


SPIDER_W = 54.0                                 # the nut's bar, across: the quill is open either side of it, for air


def spider(p):
    """A bar of the thick plywood across the back of the quill, top to bottom, with the X nut caged behind it:
    two strips of two thin layers either side of the nut, and a keeper over them."""
    part, fit, w, slot, h = "Spider", QI / 2 - 0.2, SPIDER_W / 2, NUT_AF / 2 + NUT_FLOAT, 20.0
    x0 = X_SPIDER + T34
    panel(p, part, "bar", (X_SPIDER, x0), (-w, w), (AZ - fit, AZ + fit))
    panel(p, part, "beside the nut", (x0, x0 + CAGE), (-w, -slot), (AZ - h, AZ + h), layers=2)
    panel(p, part, "beside the nut", (x0, x0 + CAGE), (slot, w), (AZ - h, AZ + h), layers=2)
    panel(p, part, "keeper", (x0 + CAGE, X_CAGE_END), (-w, w), (AZ - h, AZ + h))
    xcyl(p, "the screw's way", (X_SPIDER - 1.0, X_CAGE_END + 1.0), (0.0, AZ), SCREW_D + 1.0, op="remove")


def router(p):
    """The Colt as the photographs have it, turned in its block so that the cord's boot points down and the speed
    dial up."""
    xcyl(p, "ring", (XQ, XQ + RING_L), (0.0, AZ), ROUTER_D)
    xcyl(p, "plastic body", (XQ + RING_L, XQ + ROUTER_HOUSING), (0.0, AZ), PLASTIC_D, op="add")
    xcyl(p, "grip", (XQ + ROUTER_HOUSING, XQ + ROUTER_GRIP[1]), (0.0, AZ), ROUTER_GRIP[0], op="add")
    xcyl(p, "cap", (XQ + ROUTER_GRIP[1], X_ROUTER_BACK), (0.0, AZ), ROUTER_CAP, op="add")
    box(p, "dial's housing", (XQ + HUMP[0], XQ + HUMP[1]), (-HUMP[2], HUMP[2]), (AZ, AZ + HUMP[3]), op="add")
    box(p, "chin", (XQ + CHIN[0], XQ + CHIN[1]), (-CHIN[2], CHIN[2]), (AZ - CHIN[3], AZ), op="add")
    xcyl(p, "nose", (XQ - NOSE_L, XQ), (0.0, AZ), NOSE_D, op="add")
    xcyl(p, "spindle", (XQ - NOSE_L - SHAFT_L, XQ - NOSE_L), (0.0, AZ), SHAFT_D, op="add")
    xcyl(p, "collet nut", (XQ - NUT_FACE, XQ - NOSE_L - SHAFT_L), (0.0, AZ), COLLET_D, op="add")
    xcyl(p, "bit", (XQ - NUT_FACE - BIT_L, XQ - NUT_FACE), (0.0, AZ), BIT_D, op="add")
    zcyl(p, "cord's boot", (AZ - CHIN[3] - CORD_L, AZ - CHIN[3] + 10.0), (XQ + CORD_AT, 0.0), CORD_D, op="add")
    # On the front face, at its rim: the screw heads as a low ring, and the sprung block, on the cord's side.
    xcyl(p, "screw heads", (XQ - FACE_PROUD, XQ), (0.0, AZ), ROUTER_D, op="add")
    a, r1 = math.radians(SPRUNG_TURN - 90.0), ROUTER_D / 2
    r0, h = r1 - SPRUNG[1], SPRUNG[0] / 2
    cs, sn = math.cos(a), math.sin(a)
    s = p.sketch("right", XQ - SPRUNG[2], "sprung block")
    p.polygon(s, [(r * cs - t * sn, AZ + r * sn + t * cs) for r, t in ((r0, -h), (r1 - 0.8, -h), (r1 - 0.8, h), (r0, h))])
    p.extrude(s, SPRUNG[2], op="add", name="sprung block")


def block(p):
    """The router's block: two thick layers square in the quill, bored once for the body, kerfed through the top, its
    top corners notched for the two bolts that close the kerf, and drilled for the four screws that hold it against
    the stop plates. The layers are drawn apart as the two pieces of plywood they are cut from."""
    part, fit, x0 = "Router block", BLOCK / 2, XQ + BLOCK_AT
    for name, x in (("front land", x0), ("back land", x0 + T34)):
        panel(p, part, name, (x, x + T34), (-fit, fit), (AZ - fit, AZ + fit))
    ends = (x0 - 0.5, x0 + BLOCK_L + 0.5)
    xcyl(p, "the body's bore", ends, (0.0, AZ), ROUTER_D + BODY_FIT, op="remove")
    box(p, "kerf", ends, (-KERF / 2, KERF / 2), (AZ + ROUTER_D / 2 - 2.0, AZ + fit + 1.0), op="remove")
    for sy in (-1, 1):
        box(p, "notch", ends, tuple(sorted((sy * (fit - NOTCH[0]), sy * (fit + 1.0)))), (AZ + fit - NOTCH[1], AZ + fit + 1.0), op="remove")
        for sz in (-1, 1):
            xcyl(p, "holding screw", ends, (sy * HOLD[2], AZ + sz * HOLD[3]), HOLD[0], op="remove")
    for xc in (x0 + T34 / 2, x0 + BLOCK_L - T34 / 2):               # a bolt across the kerf in the middle of each layer
        ybore(p, "pinch bolt", (xc, AZ + PINCH_Z), PINCH[0], fit + 1.0, BLOCK + 2.0)


def liner(p, standing=False):
    """The liner: a printed band round the plastic of the body, as thick as the ring stands proud of it, open where
    the slotted metal plate is let into the body's side. In place on the router, or standing on its end as it
    prints."""
    long, bore, half = LINER_X[1] - LINER_X[0], PLASTIC_D + LINER[2], LINER[3] / 2
    if standing:
        zcyl(p, "liner", (0.0, long), (0.0, 0.0), ROUTER_D)
        zcyl(p, "bore", (-0.5, long + 0.5), (0.0, 0.0), bore, op="remove")
        zbox(p, "gap", (-half, half), (0.0, ROUTER_D / 2 + 1.0), (-0.5, long + 0.5), op="remove")
        return
    x = (XQ + LINER_X[0], XQ + LINER_X[1])
    xcyl(p, "liner", x, (0.0, AZ), ROUTER_D)
    xcyl(p, "bore", (x[0] - 0.5, x[1] + 0.5), (0.0, AZ), bore, op="remove")
    box(p, "gap", (x[0] - 0.5, x[1] + 0.5), (0.0, ROUTER_D / 2 + 1.0), (AZ - half, AZ + half), op="remove")


def z_drive_housing(p):
    """Under the outer box's floor: the X motor's housing over again, a square tube of the thin plywood with a
    bulkhead for the thrust bearing and a cap for the motor."""
    part, a, c = "Z drive housing", -T34, (MX / 2, 0.0)
    tube(p, part, "z", a - DRIVE, a, c, SB, T12)
    panel(p, part, "bulkhead", (c[0] - SB / 2, c[0] + SB / 2), (-SB / 2, SB / 2), (a - REC - BULKHEAD, a - REC))
    panel(p, part, "motor's cap", (c[0] - CO / 2, c[0] + CO / 2), (-CO / 2, CO / 2), (a - DRIVE - CAP, a - DRIVE))
    zcyl(p, "the screw's way", (a - REC - BULKHEAD - 1.0, a - REC + 1.0), c, SCREW_D + 1.0, op="remove")
    zcyl(p, "motor's pilot", (a - DRIVE - CAP - 0.5, a - DRIVE + 0.5), c, 22.5, op="remove")


def drive_parts(p, axis):
    """What turns: collar, thrust bearing, the other collar, the coupling, the motor, and last the screw through them."""
    bore = SCREW_D + 0.4
    if axis == "x":
        a, cyl, c = X_COL_END, xcyl, (0.0, AZ)
        rng = lambda u0, u1: (a + u0, a + u1)
    else:
        a, cyl, c = -T34, zcyl, (MX / 2, 0.0)
        rng = lambda u0, u1: (a - u1, a - u0)
    rings = (("collar", REC - BEARING[1] - COLLAR_L, REC - BEARING[1], BEARING[0]), ("thrust bearing", REC - BEARING[1], REC, BEARING[0]),
             ("collar", REC + BULKHEAD, REC + BULKHEAD + COLLAR_L, BEARING[0]), ("coupling", REC + BULKHEAD + COLLAR_L + 2.0, REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1], COUPLING[0]))
    for name, u0, u1, d in rings:
        cyl(p, name, rng(u0, u1), c, d)
        cyl(p, "bore", rng(u0 - 0.2, u1 + 0.2), c, bore, op="remove")
    m = MOTOR[0] / 2
    if axis == "x":
        box(p, "motor", rng(DRIVE + CAP, DRIVE + CAP + MOTOR[1]), (-m, m), (AZ - m, AZ + m))
        xcyl(p, "motor shaft", rng(DRIVE + CAP - MOTOR[2], DRIVE + CAP), c, 5.0)      # a body of its own: joined to the motor it would take the coupling round it too
        xcyl(p, "X screw", (X_TIP, a + REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1] / 2 - 1.0), c, SCREW_D)
    else:
        zbox(p, "motor", (MX / 2 - m, MX / 2 + m), (-m, m), rng(DRIVE + CAP, DRIVE + CAP + MOTOR[1]))
        zcyl(p, "motor shaft", rng(DRIVE + CAP - MOTOR[2], DRIVE + CAP), c, 5.0)
        zcyl(p, "Z screw", (a - (REC + BULKHEAD + COLLAR_L + 2.0 + COUPLING[1] / 2 - 1.0), Z_TIP), c, SCREW_D)


def nut(p, axis):
    if axis == "x":
        xhex(p, "X nut", (X_NUT, X_NUT + NUT_L), (0.0, AZ), NUT_AF, turn=30.0)      # its flats to the cage's two strips
        xcyl(p, "thread", (X_NUT - 0.5, X_NUT + NUT_L + 0.5), (0.0, AZ), SCREW_D + 0.4, op="remove")
    else:
        zhex(p, "Z nut", (Z_NUT, Z_NUT + NUT_L), (MX / 2, 0.0), NUT_AF)
        zcyl(p, "thread", (Z_NUT - 0.5, Z_NUT + NUT_L + 0.5), (MX / 2, 0.0), SCREW_D + 0.4, op="remove")


def x_switch(p):
    """Home is the quill drawn back: its end closes the switch, on the tube's floor."""
    x0 = XQ + QL + 1.0
    box(p, "X home switch", (x0, x0 + SWITCH[2]), (SB / 2 - 4.0 - SWITCH[1], SB / 2 - 4.0), (AZ - SB / 2, AZ - SB / 2 + SWITCH[0]))


def z_switch(p):
    """Home is the stage up, away from the end where its screw would meet the tube: its top closes the switch,
    which is screwed to the lid's underside. The stage stops that much short of the lid."""
    box(p, "Z home switch", (8.0, 8.0 + SWITCH[0]), Z_SWITCH_Y, (IH - SWITCH[2], IH))


def clip_shape(kind, ta, tb):
    """A clip, seen from the panels' ends: its plate's outline, its walls, its ribs as (u0, u1, v0, v1), and the
    two sheets it holds, each as its rectangle and the way a thicker sheet would grow. An L holds one sheet along
    u and one along v from a corner at the origin; a T holds a bar along u and a stem up from the middle of it.
    The outer walls are the square the sheets are pushed against; the ribs are on the walls inside."""
    w, fit, (rw, rh, _) = CLIP_WALL, CLIP_FIT, CLIP_RIB
    bite = lambda a, b, c, d: (min(a, b), max(a, b), min(c, d), max(c, d))
    if kind == "L":
        sa, sb, reach = ta + fit, tb + fit, max(ta, tb) + CLIP_REACH        # the sheet along u is ta thick, the one along v tb
        plate = [(-w, -w), (reach, -w), (reach, sa + w), (sb + w, sa + w), (sb + w, reach), (-w, reach)]
        walls = [[(-w, -w), (reach, -w), (reach, 0.0), (0.0, 0.0), (0.0, reach), (-w, reach)],
                 [(sb, sa), (reach, sa), (reach, sa + w), (sb + w, sa + w), (sb + w, reach), (sb, reach)]]
        ribs = [bite(c - rw / 2, c + rw / 2, sa - rh, sa + 0.5) for c in (sb + 5.0, reach - 5.0)] + [bite(sb - rh, sb + 0.5, c - rw / 2, c + rw / 2) for c in (sa + 5.0, reach - 5.0)]
        sheets = [{"at": (0.0, reach, 0.0, ta), "grows": "v"}, {"at": (0.0, tb, ta, reach), "grows": "u"}]
        return plate, walls, ribs, sheets
    ss, sb, half, up = ta + fit, tb + fit, ta / 2 + CLIP_REACH, tb + CLIP_REACH   # the stem is ta thick, the bar tb
    left, right = -ta / 2, -ta / 2 + ss                                      # the stem's slot: its left face is the square one
    plate = [(-half, -w), (half, -w), (half, sb + w), (right + w, sb + w), (right + w, up), (left - w, up), (left - w, sb + w), (-half, sb + w)]
    walls = [[(-half, -w), (half, -w), (half, 0.0), (-half, 0.0)],
             [(-half, sb), (left, sb), (left, up), (left - w, up), (left - w, sb + w), (-half, sb + w)],
             [(right, sb), (half, sb), (half, sb + w), (right + w, sb + w), (right + w, up), (right, up)]]
    ribs = [bite(c - rw / 2, c + rw / 2, sb - rh, sb + 0.5) for c in (-half + 6.0, half - 6.0)] + [bite(right - rh, right + 0.5, c - rw / 2, c + rw / 2) for c in (sb + 8.0, up - 5.0)]
    sheets = [{"at": (-half, half, 0.0, tb), "grows": "v"}, {"at": (left, left + ta, tb, up), "grows": "u"}]
    return plate, walls, ribs, sheets


def clip(p, kind, ta, tb, plane="top", at=0.0, place=lambda q: q):
    """Draw a clip: its plate, its walls standing on it, its ribs. On `plane` at `at`, each point of its outline
    put where `place` says. Flat on its plate is how it prints."""
    plate, walls, ribs, _ = clip_shape(kind, ta, tb)

    def prism(name, pts, lo, hi, op):
        sk = p.sketch(plane, at + lo, name)
        p.polygon(sk, [place(q) for q in pts])
        p.extrude(sk, hi - lo, op=op, name=name)
    prism("plate", plate, 0.0, CLIP_PLATE, "new")
    for k, wall in enumerate(walls):
        prism(f"wall {k + 1}", wall, CLIP_PLATE, CLIP_PLATE + CLIP_DEEP, "add")
    for k, (u0, u1, v0, v1) in enumerate(ribs):
        prism(f"rib {k + 1}", [(u0, v0), (u1, v0), (u1, v1), (u0, v1)], CLIP_PLATE, CLIP_PLATE + CLIP_DEEP - CLIP_RIB[2], "add")


def clip_name(kind, ta, tb):
    return f"Clip {kind} {int(ta)}-{int(tb)}"                # by its sheets' whole millimetres: 12 is the 1/2 in, 19 the 3/4


def clips_in_use(p):
    """The quill's four panels as they are glued up, with an L clip pushed onto each corner of one end. The clips
    are drawn first: a wall added after a panel it touches would be joined to it."""
    b, t = QI / 2, T12
    h = b + t
    for k in range(4):                              # a quarter turn on at each corner, its plate over the panels' ends
        c, sn = (1, 0, -1, 0)[k], (0, 1, 0, -1)[k]
        corner = ((-h, -h), (h, -h), (h, h), (-h, h))[k]
        clip(p, "L", T12, T12, plane="right", at=-CLIP_PLATE, place=lambda q, c=c, sn=sn, corner=corner: (corner[0] + c * q[0] - sn * q[1], corner[1] + sn * q[0] + c * q[1]))
    for name, (y, z) in zip(("lid", "back", "floor", "front"), (((-b - t, b), (b, b + t)), ((b, b + t), (-b, b + t)), ((-b, b + t), (-b - t, -b)), ((-b - t, -b), (-b - t, b)))):
        box(p, name, (0.0, QL), y, z)


# Every part: its tab, what draws it, and what it moves with.
PARTS = [("Outer box", outer_box, "fixed"), ("Z gibs", z_gibs, "fixed"), ("Z drive housing", z_drive_housing, "fixed"),
         ("Z drive", lambda p: drive_parts(p, "z"), "fixed"), ("Z home switch", z_switch, "fixed"),
         ("X tube", x_tube, "z"), ("Z stage", z_stage, "z"), ("Stage strips", stage_strips, "z"), ("Z nut", lambda p: nut(p, "z"), "z"), ("X gibs", x_gibs, "z"),
         ("X drive", lambda p: drive_parts(p, "x"), "z"), ("X home switch", x_switch, "z"),
         ("X quill", quill, "xz"), ("Quill strips", quill_strips, "xz"), ("Spider", spider, "xz"), ("X nut", lambda p: nut(p, "x"), "xz"), ("Router", router, "xz"), ("Body liner", liner, "xz"), ("Router block", block, "xz")]
PLYWOOD = ("Outer box", "Z drive housing", "X tube", "Z stage", "X quill", "Spider", "Router block")   # the tabs whose every body is a piece of plywood
# The four corners of the travel: how far the quill is run out, and how far the stage is run down.
CORNERS = [("Home: drawn back, up", 0.0, 0.0), ("Plunged, up", X_TRAVEL, 0.0), ("Drawn back, down", 0.0, Z_TRAVEL), ("Plunged, down", X_TRAVEL, Z_TRAVEL)]


def cut_list():
    """The panels, like with like: thickness, part, piece, long, wide, how many. And how much of a sheet each
    thickness comes to."""
    rows = {}
    for q in PANELS:
        key = (q["t"], q["part"], q["name"], round(q["long"], 2), round(q["wide"], 2))
        rows[key] = rows.get(key, 0) + 1
    out = [{"t": k[0], "part": k[1], "name": k[2], "long": k[3], "wide": k[4], "count": n} for k, n in sorted(rows.items(), key=lambda kv: (-kv[0][0], PLYWOOD.index(kv[0][1]), -kv[0][3]))]
    area = {t: sum(r["long"] * r["wide"] * r["count"] for r in out if r["t"] == t) for t in (T34, T12)}
    return out, area


def main():
    found = rules()
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "positioner.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "positioner"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Two-axis nested-box router positioner"}, {"type": "rename_tab", "tab": 1, "name": PARTS[0][0]}]})
    tabs, bodies = {}, {}
    for i, (title, make, _) in enumerate(PARTS):
        tabs[title] = 1 if i == 0 else new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        make(part)
        bodies[title] = part.bodies()
        print(f"{title}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies[title]))
    for title in PLYWOOD:
        if len(bodies[title]) != sum(1 for q in PANELS if q["part"] == title) - sum(1 for q in PANELS if q["part"] == title and q["name"].endswith("beside the nut")) // 2:
            raise SystemExit(f"{title}: its bodies are not its panels")
    corners = {}
    for label, dx, dz in CORNERS:
        asm = new_tab(mcp, {"type": "add_assembly", "name": label})
        ops = []
        for title, _, moves in PARTS:
            at = {"x": -dx if moves == "xz" else 0.0, "y": 0.0, "z": -dz if moves in ("z", "xz") else 0.0}
            for k in range(len(bodies[title])):
                ops.append({"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[title], "body": k, "name": title if len(bodies[title]) == 1 else f"{title} {k + 1}",
                                                                      "fixed": True, "placement": {"position": at, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}}})
        text = mcp.call("apply", {"ops": ops})
        if "ERROR:" in text:
            raise SystemExit(text)
        corners[label] = asm
    home, far = corners[CORNERS[0][0]], corners[CORNERS[3][0]]
    mcp.call("screenshot", {"tab": home, "view": "-0.55,-0.7,0.45", "width": 1500, "height": 1100, "path": os.path.join(OUT, "positioner.png")})
    fit = f"{BOX_X[0] - X_TRAVEL - 20},-1,{Z_END - 10},{X_END + 10},1,{IH + T34 + 10}"
    mcp.call("screenshot", {"tab": home, "view": "0,-1,0", "section": "y:0", "fit": fit, "width": 1500, "height": 1100, "path": os.path.join(OUT, "section-home.png")})
    mcp.call("screenshot", {"tab": far, "view": "0,-1,0", "section": "y:0", "fit": fit, "width": 1500, "height": 1100, "path": os.path.join(OUT, "section-plunged-down.png")})
    # The mount, at full plunge with the stage up, which is how the router goes in: from the bit's side, cut through the
    # front land where its bolt is and through the stop plates; and from the front, cut down the middle.
    plunged = corners[CORNERS[1][0]]
    across = f"-30,{-CO / 2 - 28},{AZ - CO / 2 - 28},150,{CO / 2 + 28},{AZ + CO / 2 + 28}"
    for name, cut in (("mount-land", BLOCK_AT + T34 + T34 / 2), ("mount-stops", STOP_AT + T34 / 2)):
        mcp.call("screenshot", {"tab": plunged, "view": "-1,0,0", "section": f"x:{cut:.2f}", "fit": across, "width": 1000, "height": 1000, "path": os.path.join(OUT, name + ".png")})
    mcp.call("screenshot", {"tab": plunged, "view": "0,-1,0", "section": "y:0", "fit": f"{-NUT_FACE - BIT_L - 10},-1,{AZ - CO / 2 - 45},{ROUTER_L + 25},1,{AZ + CO / 2 + 20}", "width": 1500, "height": 1000,
                            "path": os.path.join(OUT, "mount-side.png")})
    print(mcp.call("export", {"tab": home, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "positioner.pdf"),
                              "note": f"two-axis router positioner in 1/2 and 3/4 in plywood, butted, glued and nailed, first pass: X {X_TRAVEL:.0f}, Z {Z_TRAVEL:.0f} of travel; the router's body {ROUTER_D:.2f} by callipers, the rest of it off three photographs"}))
    # -- the clips: each in a tab of its own, flat as it prints, and its STL -------------
    os.makedirs(os.path.join(OUT, "clips"), exist_ok=True)
    clip_set = []
    for kind, ta, tb, count in CLIPS:
        title = clip_name(kind, ta, tb)
        tab = new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tab, title)
        clip(part, kind, ta, tb)
        made = part.bodies()
        if len(made) != 1:
            raise SystemExit(f"{title} is {len(made)} bodies")
        file = f"clips/clip_{kind}_{int(ta)}_{int(tb)}.stl"
        mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, file)})
        plate, _, ribs, sheets = clip_shape(kind, ta, tb)
        clip_set.append({"tab": title, "kind": kind, "sheets": [ta, tb], "count": count, "file": file, "volume": made[0][1],
                         "size": [max(q[0] for q in plate) - min(q[0] for q in plate), max(q[1] for q in plate) - min(q[1] for q in plate), CLIP_PLATE + CLIP_DEEP],
                         "ribs": len(ribs), "holds": sheets})
        print(f"{title}: {made[0][1]:.0f} mm3, {count} to print")
    # -- the liner, standing on its end as it prints -----------------------------------
    tab = new_tab(mcp, {"type": "add_part_studio", "name": "Liner, to print"})
    part = Part(mcp, tab, "Liner, to print")
    liner(part, standing=True)
    lined = part.bodies()
    if len(lined) != 1 or len(bodies["Body liner"]) != 1 or abs(lined[0][1] - bodies["Body liner"][0][1]) > 1e-3 * lined[0][1]:
        raise SystemExit("the liner as it prints is not the liner as it is on the router")
    mcp.call("export", {"tab": tab, "format": "stl", "path": os.path.join(OUT, "liner.stl")})
    print(f"Liner: {lined[0][1]:.0f} mm3, {LINER_X[1] - LINER_X[0]:.1f} long")
    use = new_tab(mcp, {"type": "add_part_studio", "name": "Clips in use"})
    part = Part(mcp, use, "Clips in use")
    clips_in_use(part)
    used = part.bodies()
    if len(used) != 8:
        raise SystemExit(f"the clips in use are {len(used)} bodies, not four clips and four panels")
    mcp.call("screenshot", {"tab": use, "view": "-0.8,-0.45,0.4", "width": 1500, "height": 1000, "path": os.path.join(OUT, "clips.png")})
    mcp.close()

    # -- the cut list ---------------------------------------------------------------
    panels, area = cut_list()
    with open(os.path.join(OUT, "cutlist.csv"), "w") as f:
        f.write("thickness_mm,part,piece,long_mm,wide_mm,count\n")
        for r in panels:
            f.write(f"{r['t']},{r['part']},{r['name']},{r['long']:.1f},{r['wide']:.1f},{r['count']}\n")
    sheet = SHEET[0] * SHEET[1]

    # -- the report the brief asks for -------------------------------------------
    sizes = {"Outer box": (BOX_X[1] - BOX_X[0], 2 * BOX_Y, IH + 2 * T34), "Z stage": (MX + GAP, MD + GAP, MH), "X tube": (X_TUBE_END + CAP, CO, CO),
             "X quill": (QL, QO + GAP, QO + GAP)}
    envelope = {"x": (BOX_X[0], X_END), "y": (-BOX_Y, BOX_Y), "z": (Z_END, IH + T34)}
    numbers = {
        "router": {"d": ROUTER_D, "envelope": ROUTER_ENV, "length": ROUTER_L, "housing": ROUTER_HOUSING, "grip": ROUTER_GRIP, "nose": [NOSE_D, NOSE_L], "shaft": [SHAFT_D, SHAFT_L],
                   "collet": [COLLET_D, COLLET_L], "nut_face": NUT_FACE, "ring": RING_L, "plastic": PLASTIC_D, "flats": FLATS, "shank_in": SHANK_IN, "overall": ROUTER_L + NUT_FACE, "cord": [CORD_AT, CORD_D, CORD_L], "cap": ROUTER_CAP, "hump": HUMP, "chin": CHIN, "sprung": SPRUNG, "sprung_turn": SPRUNG_TURN, "face_proud": FACE_PROUD, "bit": [BIT_D, BIT_L]},
        "mount": {"block": [BLOCK, BLOCK_L], "at": BLOCK_AT, "layers": [T34, T34], "body_fit": BODY_FIT, "liner": LINER, "liner_x": LINER_X, "liner_file": "liner.stl", "kerf": KERF, "notch": NOTCH, "pinch": PINCH, "pinch_z": PINCH_Z,
                  "stop": [STOP_W, STOP_RELIEF], "stop_at": STOP_AT, "hold": HOLD},
        "travel": {"x": X_TRAVEL, "z": Z_TRAVEL}, "ply": [T12, T34], "outer_wall": T34, "face_wall": T12, "gap": GAP, "strip_l": STRIP_L, "gib": GIB, "engage": ENGAGE, "rail": RAIL,
        "screw": {"d": SCREW_D, "pitch": SCREW_PITCH, "per_step": SCREW_PITCH / 200, "nut": [NUT_AF, NUT_L], "float": NUT_FLOAT, "lead": NUT_LEAD, "tip_clear": TIP_CLEAR},
        "quill": {"bore": QI, "outside": QO, "length": QL, "front": XQ}, "sleeve": {"bore": SB, "outside": CO},
        "stage": {"x": MX, "y": MD, "height": MH, "foot": ZB, "screw_room": Z_CLEAR, "boss": FOOT}, "box": {"x": BOX_X, "y": BOX_Y, "inside_height": IH, "slot_z": SLOT_Z, "slot_w": SLOT_W, "column_slot_z": COL_SLOT_Z,
                                                                                                         "cord_slot": [CORD_SLOT, CORD_BACK], "cord_x": CORD_X, "cord_slack": CORD_SLACK, "cord_notch": CORD_NOTCH},
        "axis_z": AZ, "x": {"router_back": X_ROUTER_BACK, "tip": X_TIP, "nut": X_NUT, "spider": X_SPIDER, "cage_end": X_CAGE_END, "column_end": X_COL_END, "tube_end": X_TUBE_END, "end": X_END},
        "z": {"nut": Z_NUT, "tip": Z_TIP, "end": Z_END}, "drive": {"recess": REC, "bulkhead": BULKHEAD, "chamber": CHAMBER, "length": DRIVE, "cap": CAP, "motor": MOTOR},
        "rules": found, "envelope": envelope, "size": [envelope[k][1] - envelope[k][0] for k in "xyz"],
        "behind_router": {"drawn_back": X_END - X_ROUTER_BACK, "plunged": X_END - X_ROUTER_BACK + X_TRAVEL},
        "out_of_the_box": X_END - BOX_X[1], "sizes": sizes,
        "clips": {"wall": CLIP_WALL, "reach": CLIP_REACH, "deep": CLIP_DEEP, "plate": CLIP_PLATE, "fit": CLIP_FIT, "rib": CLIP_RIB, "set": clip_set,
                  "in_use": {"tab": "Clips in use", "bodies": [list(b) for b in used], "clips": 4, "panels": 4}},
        "plywood": PLYWOOD, "panels": panels, "area": {"t34": area[T34], "t12": area[T12], "sheet": sheet},
        "parts": [{"tab": t, "moves": m, "bodies": [list(b) for b in bodies[t]]} for t, _, m in PARTS], "corners": [[c[0], c[1], c[2]] for c in CORNERS],
    }
    with open(os.path.join(OUT, "positioner.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"envelope {numbers['size'][0]:.0f} long x {numbers['size'][1]:.0f} deep x {numbers['size'][2]:.0f} tall; {numbers['out_of_the_box']:.0f} of that is tube and motor out of the box's right side; "
          f"{numbers['behind_router']['drawn_back']:.0f} behind the router drawn back")
    print(f"quill {QO:.0f} square x {QL:.0f}, in its guide {found['x_engaged']:.2f} widths; Z stage {MX:.0f} x {MD:.0f} x {MH:.0f}, {found['z_engaged']:.2f} widths; outer box {sizes['Outer box'][0]:.0f} x {sizes['Outer box'][1]:.0f} x {sizes['Outer box'][2]:.0f}")
    print(f"X screw tip {found['x_tip_to_router']:.0f} from the router's back, drawn back; Z screw tip {found['z_tip_to_sleeve']:.1f} under the tube, down; at full plunge the router's nose {found['nose_proud']:.1f} out of the box's face and the collet nut {found['collet_proud']:.1f} clear of it")
    print(f"the cord's boot: {-found['cord_past_stage']:.0f} inside the stage's end at full plunge, over its open room; the tube's slot runs {CORD_SLACK:.0f} past it and {found['slot_under_strip']:.0f} under "
          f"where the quill's back strip does, which bears {found['strip_beside_slot']:.0f} each side of it; the bore {found['rear_in_bore']:.1f} round the chin as photographed")
    print(f"the block {BLOCK:.1f} square x {BLOCK_L:.1f}, {BLOCK_AT:.1f} behind the rim and {ROUTER_HOUSING - BLOCK_AT - BLOCK_L:.1f} short of the grip, {found['front_land_on_ring']:.1f} of its front land on the ring and "
          f"{found['back_land_on_liner']:.1f} of its back land on a liner {found['liner_wall']:.2f} thick; it passes the bit slot by {found['slot_past_block']:.1f} a side; the stop plates {found['stop_beside_rear']:.0f} beside "
          f"the dial's housing and {found['stop_before_rear']:.1f} in front of the chin; the boot passes the slot's end with the stage in the top {found['boot_through_face']:.0f} of its travel")
    print(f"plywood: {sum(r['count'] for r in panels)} pieces; 3/4 in {area[T34] / 1e6:.2f} m2, {100 * area[T34] / sheet:.0f} % of a sheet; 1/2 in {area[T12] / 1e6:.2f} m2, {100 * area[T12] / sheet:.0f} % of a sheet")


if __name__ == "__main__":
    main()
