"""Scratch-stock fixtures for the curvy Spectre's sticks, and for the
points of the flat one. The hex-edged tile is half a hexagon on every
edge; bow each of its flats into the arc of the circle the hexagon is
inscribed in (radius half an edge, about the edge's middle) and every
edge is a true semicircle, a bump or a dent: a curvy Spectre, still
built of the same lattice sticks. A stick on a bump is then a 60°
sector, its outer face a convex arc about its own apex; a stick on a
dent is a triangle with its outer face hollowed to the arc. Tiles stand
the line apart, so the arcs are the eroded tile's: half an edge less
half the line on a bump, plus half the line in a dent.

Those faces are scraped, not sawn: the stick lies in a printed block,
apex down in a V, and a printed sled carrying a filed scraper blade is
pushed along it. The block's rails are the depth stop and its sides
the fence, so the stick's size is the block's geometry and not the
hand's: the sled starts riding on the blade, its heel on the rails,
and stops cutting when its sole lies flat. A third block holds a
stick point up and a straight blade takes the point off, which the
flat-edged tile's cut list already asks of a third of its sticks. The
blades are cut from a 0.81 mm card scraper and filed to a printed
master, which is also what the blade is set on: the finished face and
the rails it is measured from, printed on end so both are one outline.

    cargo build --release -p ok-mcp
    python3 examples/spectre-planes/build.py
    cargo test -p ok-render --test spectre_planes   # what CI runs

Outputs under out/: planes.okpart (the three blocks, the sled, the
two masters, a stick and a blade for each, and the stations assembled),
block_bump.stl, block_dent.stl, block_point.stl, sled.stl (print
three, sole up as exported), master_bump.stl and master_dent.stl (on
end as exported), stations.png and a section through each station,
stations.svg (the three sections drawn with their numbers) and
blades.svg (the blades to file), planes.pdf, and planes.json (every
number the test checks).
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")

# The tile, as the mosaic lid's and the cauls'.
EDGE = 10.56                                    # the tile's edge
GAP, INNER = 0.7, 0.1                           # the line between tiles, the owner's pick; the glue line between a tile's sticks
STICK = 200.0                                   # a stick's length, as the cauls'

# The stock. Strips are planed to a thickness and sawn on the 30° sled; these two leave a hair for the blade.
STRIP_BUMP = 4.9                                # the bump stick's strip: its finished top is 4.83 over the apex
STRIP_DENT = 3.9                                # the dent stick's: its finished corners are 3.88 over the apex

# The blade: a DFM card scraper, 0.032 in blue spring steel at 50 HRC, cut into blanks.
BLADE_T = 0.81
BLADE_W, BLADE_H = 12.0, 30.0                   # a blank; twenty from one 2.5 x 5 in scraper
LEAN = 10.0                                     # the blade's top leans toward the cut, degrees: a scraper's attitude, built in so the sole can lie flat
NOTCH = 1.1                                     # how far the hollow is filed up from the blade's shoulders, and the cove's tongue down from its: 1.0 to 1.3 does

# The sled, one body for all three blades. Lengths along the cut, heights from the sole.
SLED_L = 40.0
BODY_H = 16.0                                   # sole to top: the blade slot's length, and something to hold
SLOT_X = 14.0                                   # the slot's front wall at the sole, from the sled's front
SLOT_T, SLOT_W = 1.1, 12.4                      # the slot: a printed slot under a millimetre closes up; the screw puts the blade on the back wall
TUNNEL_W, TUNNEL_H = 9.0, 2.0                   # the stick's way through the sole; its height is the blade's free length
SKIRT_T, SKIRT_D = 4.0, 12.0                    # the skirts down the block's sides: the fence
PLAY = 0.15                                     # a skirt off the block's side, each side
SCREW_Z = 8.0                                   # the clamp screw over the sole: M4 x 16 from the front, onto the blade
SCREW_D = 4.4                                   # its bore, clearance
SCREW_L = 13.1                                  # the bore's depth: into the slot, short of its back wall at every height
NUT_X, NUT_T, NUT_W = 5.0, 3.4, 7.2             # the nut trap behind the front face: an M4 nut is 3.2 thick, 7.0 across flats
NUT_DEEP = SCREW_Z + 8.08 / 2 + 0.06            # from the top, so the nut rests with its thread on the bore

# The blocks. Widths across, heights from the bench.
BLOCK_L, BLOCK_W = 240.0, 30.0                  # 240 goes on a 256 mm bed without being turned
RAIL_Z = 18.0                                   # the rails the sole lands on, and a layer height's multiple
LAND_W, LAND_DROP = 15.0, 0.5                   # between the rails the top is lower, so no blade shoulder ever touches it
FLANGE_W, FLANGE_T = 60.0, 3.0                  # a foot under the skirts for four screws
HOLE_D, HOLE_IN, HOLE_Y = 4.5, 8.0, 25.0
STOP = 10.0                                     # the groove ends this short of the far end: the stick's stop
RELIEF_W, RELIEF_D = 0.8, 0.6                   # a slot down the V's bottom: the nozzle cannot print the corner, and dust needs somewhere
CHANNEL_PLAY = 0.08                             # the point block's channel past the stick's base, each side

# The masters: a finished face and the rails it is measured from, printed on end so the face is an outline.
MASTER_L = 40.0                                 # the sled's length
MASTER_H = 14.0                                 # flat back to rail edges: the skirts hang clear of the bench
FOOT, FOOT_IN = 2.0, 0.5                        # the end on the bed is drawn in, so an elephant's foot never lifts the sled
PLATEAU = 0.9                                   # the dent master's plateau over its rail edges, under the cove's shoulders
NOTCH_W = 3.0                                   # notches in a block's foot at the stop end say which it is: one, two, three

# Derived: the sticks.
T30 = math.tan(math.radians(30.0))
TL = math.tan(math.radians(LEAN))
SIDE = EDGE / 2.0                               # the lattice triangle, and the radius of the circle through a hex edge's corners
ALT = SIDE * math.sqrt(3.0) / 2.0
R_BUMP = SIDE - GAP / 2.0                       # the eroded tile's arc on a bump
R_DENT = SIDE + GAP / 2.0                       # and in a dent
APEX = INNER                                    # a 60° corner with both sides in by half the glue line is in by all of it
POINT_OFF = GAP / 2.0 - INNER                   # a point on the tile's edge comes off at the eroded tile
INNER_SIDE = SIDE - INNER * math.sqrt(3.0)      # the inner triangle, 5.11
INNER_ALT = INNER_SIDE * math.sqrt(3.0) / 2.0
DENT_C = 2.0 * ALT - APEX                       # the dent's centre over a dent stick's apex


def arc(cx, cy, r, a0, a1, n=32):
    """Points on a circle from angle a0 to a1, degrees, ends included."""
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * k / n)), cy + r * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]


def area(p):
    return 0.5 * sum(p[i][0] * p[(i + 1) % len(p)][1] - p[(i + 1) % len(p)][0] * p[i][1] for i in range(len(p)))


def clip_below(poly, top):
    """The part of a polygon at or under height `top`."""
    out = []
    for i, p in enumerate(poly):
        q = poly[(i + 1) % len(poly)]
        if p[1] <= top:
            out.append(p)
        if (p[1] < top) != (q[1] < top) and abs(q[1] - p[1]) > 1e-12 and p[1] != top and q[1] != top:
            t = (top - p[1]) / (q[1] - p[1])
            out.append((p[0] + (q[0] - p[0]) * t, top))
    return out


def bump():
    """A stick on a bump, apex at the origin: a sector of the eroded
    arc about the bump's middle, which is the glue line below the
    apex. Returns its numbers and its section."""
    # where the arc meets a flank, x = y tan 30
    a, b, c = 1.0 + T30 * T30, 2.0 * APEX, APEX * APEX - R_BUMP * R_BUMP
    y = (-b + math.sqrt(b * b - 4.0 * a * c)) / (2.0 * a)
    x = y * T30
    at = math.degrees(math.atan2(y + APEX, x))
    section = [(0.0, 0.0)] + arc(0.0, -APEX, R_BUMP, at, 180.0 - at)
    stock = [(0.0, 0.0), (STRIP_BUMP * T30, STRIP_BUMP), (-STRIP_BUMP * T30, STRIP_BUMP)]
    return {"top": R_BUMP - APEX, "corner": (x, y), "centre": (0.0, -APEX), "radius": R_BUMP, "section": section, "stock": stock}


def dent():
    """A stick in a dent, apex at the origin: the triangle with its
    outer face hollowed to the eroded arc about the dent's middle,
    which is two altitudes less the glue line above the apex."""
    a, b, c = 1.0 + T30 * T30, -2.0 * DENT_C, DENT_C * DENT_C - R_DENT * R_DENT
    y = (-b - math.sqrt(b * b - 4.0 * a * c)) / (2.0 * a)
    x = y * T30
    at = math.degrees(math.atan2(y - DENT_C, x))
    section = [(0.0, 0.0)] + arc(0.0, DENT_C, R_DENT, at, -180.0 - at)
    stock = [(0.0, 0.0), (STRIP_DENT * T30, STRIP_DENT), (-STRIP_DENT * T30, STRIP_DENT)]
    return {"low": DENT_C - R_DENT, "corner": (x, y), "centre": (0.0, DENT_C), "radius": R_DENT, "section": section, "stock": stock}


def point():
    """An inner triangle base down, its point off at the eroded tile."""
    h = INNER_ALT - POINT_OFF
    f = POINT_OFF * T30
    section = [(-INNER_SIDE / 2, 0.0), (INNER_SIDE / 2, 0.0), (f, h), (-f, h)]
    stock = [(-INNER_SIDE / 2, 0.0), (INNER_SIDE / 2, 0.0), (0.0, INNER_ALT)]
    return {"flat": h, "flat_w": 2.0 * f, "section": section, "stock": stock}


BUMP, DENT, POINT = bump(), dent(), point()

# A station is a block and the blade that goes with it. `rail` is the rail plane over the stick's seat (the V's
# apex, or the channel's floor), chosen so each blade is set where it can be seen to be right: the hollow with
# its arc's top where the bump's is, the cove with its tip on the rail plane, the straight one flush with the sole.
RAIL_BUMP = 3.9                                 # under the bump stick's corners (4.19), so the whole arc is above the rails
STATIONS = [
    {"key": "bump", "name": "bump", "rail": RAIL_BUMP, "stick": BUMP, "strip": STRIP_BUMP, "blade": "hollow"},
    {"key": "dent", "name": "dent", "rail": DENT["low"], "stick": DENT, "strip": STRIP_DENT, "blade": "cove"},
    {"key": "point", "name": "point", "rail": POINT["flat"], "stick": POINT, "strip": INNER_ALT, "blade": "straight"},
]
for st in STATIONS:
    st["seat"] = RAIL_Z - st["rail"]            # the seat's height over the bench


def edge_of(blade, n=32):
    """A blade's edge at its set, across the block and over the rail
    plane, left to right, to its full width."""
    w = BLADE_W / 2.0
    if blade == "hollow":
        cy = -APEX - RAIL_BUMP                  # the bump's centre under the rail plane
        top = R_BUMP + cy                       # the arc's top over it
        sh = top - NOTCH                        # the shoulders
        m = math.sqrt(R_BUMP * R_BUMP - (sh - cy) ** 2)
        a = math.degrees(math.atan2(sh - cy, m))
        return [(-w, sh)] + arc(0.0, cy, R_BUMP, 180.0 - a, a, n) + [(w, sh)]
    if blade == "cove":
        m = math.sqrt(R_DENT * R_DENT - (R_DENT - NOTCH) ** 2)
        a = math.degrees(math.atan2(-(R_DENT - NOTCH), m))
        return [(-w, NOTCH)] + arc(0.0, R_DENT, R_DENT, -180.0 - a, a, n) + [(w, NOTCH)]
    return [(-w, 0.0), (w, 0.0)]


def lift_of(st):
    """How far over the rails the sole starts, the blade resting on the stock."""
    if st["blade"] == "hollow":
        x = STRIP_BUMP * T30
        return STRIP_BUMP - math.sqrt(R_BUMP * R_BUMP - x * x) + APEX
    return st["strip"] - st["rail"]


# -- sections of the printed parts ----------------------------------------

def block_outline():
    w, f, land = BLOCK_W / 2, FLANGE_W / 2, RAIL_Z - LAND_DROP
    return [(-f, 0.0), (f, 0.0), (f, FLANGE_T), (w, FLANGE_T), (w, RAIL_Z), (LAND_W / 2, RAIL_Z), (LAND_W / 2, land),
            (-LAND_W / 2, land), (-LAND_W / 2, RAIL_Z), (-w, RAIL_Z), (-w, FLANGE_T), (-f, FLANGE_T)]


def groove_of(st):
    """What is cut out of a block for its stick, drawn past the top."""
    top, z0 = RAIL_Z + 1.0, st["seat"]
    if st["key"] == "point":
        w = INNER_SIDE / 2 + CHANNEL_PLAY
        return [(-w, z0), (w, z0), (w, top), (-w, top)]
    m, r = (top - z0) * T30, RELIEF_W / 2
    return [(-m, top), (-r, z0 + r / T30), (-r, z0 - RELIEF_D), (r, z0 - RELIEF_D), (r, z0 + r / T30), (m, top)]


def sled_outline():
    """The sled as it is printed, sole up: across, and up from its top face."""
    i, o = BLOCK_W / 2 + PLAY, BLOCK_W / 2 + PLAY + SKIRT_T
    return [(-o, 0.0), (o, 0.0), (o, BODY_H + SKIRT_D), (i, BODY_H + SKIRT_D), (i, BODY_H), (TUNNEL_W / 2, BODY_H), (TUNNEL_W / 2, BODY_H - TUNNEL_H),
            (-TUNNEL_W / 2, BODY_H - TUNNEL_H), (-TUNNEL_W / 2, BODY_H), (-i, BODY_H), (-i, BODY_H + SKIRT_D), (-o, BODY_H + SKIRT_D)]


def master_outline(kind, n=32):
    """A master on its back: rail edges MASTER_H up, the land between
    them, and standing on it the bump's arc as the hollow meets it, or
    the dent's in a plateau as the cove does."""
    w, h, land = BLOCK_W / 2, MASTER_H, MASTER_H - LAND_DROP
    if kind == "bump":
        cy = h - APEX - RAIL_BUMP
        m = math.sqrt(R_BUMP * R_BUMP - (land - cy) ** 2)
        a = math.degrees(math.atan2(land - cy, m))
        face = arc(0.0, cy, R_BUMP, a, 180.0 - a, n)                    # right to left over the top
    else:
        t = math.sqrt(R_DENT * R_DENT - (R_DENT - PLATEAU) ** 2)
        b = math.degrees(math.atan2(R_DENT - PLATEAU, t))
        pw = t + 0.55
        face = [(pw, land), (pw, h + PLATEAU)] + arc(0.0, h + R_DENT, R_DENT, -b, b - 180.0, n) + [(-pw, h + PLATEAU), (-pw, land)]
    return [(-w, 0.0), (w, 0.0), (w, h), (LAND_W / 2, h), (LAND_W / 2, land)] + face + [(-LAND_W / 2, land), (-LAND_W / 2, h), (-w, h)]


def notches_of(k):
    """Rectangles out of the foot's edge near the stop: k + 1 of them."""
    return [((HOLE_IN + 8.0 + 2 * NOTCH_W * i, FLANGE_W / 2 - NOTCH_W), (HOLE_IN + 8.0 + 2 * NOTCH_W * i + NOTCH_W, FLANGE_W / 2 + 1.0)) for i in range(k + 1)]


def slot_at(z):
    """The slot's front wall, from the sled's front, at height z over the sole."""
    return SLOT_X - z * TL


def checks():
    """What has to hold for the fixture to work; a number that breaks one stops the build."""
    out = {}
    for st in STATIONS:
        e = edge_of(st["blade"])
        land = -LAND_DROP
        # the blade never reaches the block: over the land everywhere, and the block is no higher than the land under it
        out[st["key"] + "_blade_over_land"] = min(y for _, y in e) - land if st["blade"] != "cove" else min(y for x, y in e) - land
        # the tunnel's roof clears the stock when the sled starts
        out[st["key"] + "_tunnel_over_stock"] = TUNNEL_H - (st["strip"] - st["rail"])
        out[st["key"] + "_lift"] = lift_of(st)
    # the hollow's mouth is wider than the V at the rail plane
    cy = -APEX - RAIL_BUMP
    out["hollow_mouth"] = 2.0 * math.sqrt(R_BUMP * R_BUMP - cy * cy)
    out["v_at_rails"] = 2.0 * RAIL_BUMP * T30
    out["skirt_over_flange"] = RAIL_Z - SKIRT_D - FLANGE_T
    out["bore_into_slot"] = SCREW_L - slot_at(SCREW_Z - SCREW_D / 2)             # past the front wall where it is furthest back
    out["bore_short_of_back"] = slot_at(SCREW_Z + SCREW_D / 2) + SLOT_T - SCREW_L  # short of the back wall where it is furthest forward
    out["nut_to_slot"] = slot_at(BODY_H) - (NUT_X + NUT_T)
    out["nut_to_tunnel"] = BODY_H - TUNNEL_H - NUT_DEEP
    out["run_in"] = BLOCK_L - STOP - STICK - (SLED_L - SLOT_X)                  # rail left behind the sled's heel at the stick's near end
    bad = {k: v for k, v in out.items() if not k.endswith("_lift") and k not in ("hollow_mouth", "v_at_rails") and v <= 0.0}
    if bad or out["hollow_mouth"] <= out["v_at_rails"]:
        raise SystemExit(f"the fixture does not clear itself: {bad or out}")
    return out


# -- the drawings ------------------------------------------------------------

WOOD, STOCK, PLA, SLED, STEEL, INK, DIM = "#c9a46c", "#f1e3c6", "#d5d9de", "#9db4cf", "#3d4650", "#222", "#1a4fb3"


def svg_stations(path):
    """The three stations in section at the end of the cut, with their numbers."""
    sc, pw, ph, gapx, top = 36.0, 760, 470, 30, 118
    w, h = 3 * pw + 4 * gapx, top + ph + 70
    s = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="Helvetica, Arial, sans-serif" font-size="15">',
         '<rect width="100%" height="100%" fill="#fff"/>',
         f'<text x="{gapx}" y="34" font-size="21" font-weight="bold" fill="{INK}">The three stations in section, the sled down on its rails at the end of the cut</text>',
         f'<text x="{gapx}" y="58" fill="#555">edge {EDGE:g}, line {GAP:g} between tiles, glue line {INNER:g}; heights are over the stick\'s seat; dashed is the stock as sawn, dark the blade\'s edge</text>']

    def panel(k, st):
        left = gapx + k * (pw + gapx)
        ox, oy = left + pw / 2, top + ph - 95
        z0 = st["seat"]

        def X(p):
            return ox + p[0] * sc, oy - (p[1] - z0) * sc

        def path_of(pts):
            return "M " + " L ".join(f"{X(p)[0]:.1f},{X(p)[1]:.1f}" for p in pts) + " Z"

        def up(pts):
            return [(x, y + z0) for x, y in pts]

        s.append(f'<text x="{left}" y="{top - 28}" font-size="18" font-weight="bold" fill="{INK}">{st["name"]} block, {st["blade"]} blade</text>')
        s.append(f'<text x="{left}" y="{top - 8}" fill="#555">the sole starts {lift_of(st):.2f} over the rails and comes down onto them</text>')
        s.append(f'<clipPath id="c{k}"><rect x="{left}" y="{top}" width="{pw}" height="{ph}"/></clipPath><g clip-path="url(#c{k})">')
        s.append(f'<path d="{path_of(block_outline())}" fill="{PLA}" stroke="{INK}" stroke-width="1.2"/>')
        s.append(f'<path d="{path_of(groove_of(st))}" fill="#fff" stroke="{INK}" stroke-width="1.2"/>')
        s.append(f'<rect x="{left}" y="{top}" width="{pw}" height="{X((0, RAIL_Z))[1] - top:.1f}" fill="#fff"/>')
        hi = RAIL_Z + 3.4
        so = [(-BLOCK_W / 2 - PLAY, RAIL_Z), (-TUNNEL_W / 2, RAIL_Z), (-TUNNEL_W / 2, RAIL_Z + TUNNEL_H), (TUNNEL_W / 2, RAIL_Z + TUNNEL_H), (TUNNEL_W / 2, RAIL_Z),
              (BLOCK_W / 2 + PLAY, RAIL_Z), (BLOCK_W / 2 + PLAY, hi), (-BLOCK_W / 2 - PLAY, hi)]
        s.append(f'<path d="{path_of(block_outline())}" fill="none" stroke="{INK}" stroke-width="1.2"/>')
        s.append(f'<path d="{path_of(so)}" fill="{SLED}" fill-opacity="0.6" stroke="{INK}" stroke-width="1.2"/>')
        e = [(x, y + RAIL_Z) for x, y in edge_of(st["blade"])]
        s.append(f'<path d="{path_of(e + [(BLADE_W / 2, hi), (-BLADE_W / 2, hi)])}" fill="{STEEL}" fill-opacity="0.3" stroke="none"/>')
        s.append(f'<path d="{path_of(up(st["stick"]["section"]))}" fill="{WOOD}" stroke="{INK}" stroke-width="1.3"/>')
        s.append(f'<path d="{path_of(up(st["stick"]["stock"]))}" fill="none" stroke="#7a5a2a" stroke-width="1.2" stroke-dasharray="5 4"/>')
        s.append('<path d="M ' + " L ".join(f"{X(p)[0]:.1f},{X(p)[1]:.1f}" for p in e) + f'" fill="none" stroke="{STEEL}" stroke-width="3"/>')
        s.append('</g>')
        s.append(f'<rect x="{left}" y="{top}" width="{pw}" height="{ph}" fill="none" stroke="#bbb"/>')

        def label(p, text, dx, dy, anchor="start"):
            x, y = X(p)
            s.append(f'<path d="M {x:.1f},{y:.1f} L {x + dx:.1f},{y + dy:.1f}" stroke="{DIM}" stroke-width="1"/><circle cx="{x:.1f}" cy="{y:.1f}" r="2.6" fill="{DIM}"/>')
            s.append(f'<text x="{x + dx + (5 if anchor == "start" else -5):.1f}" y="{y + dy + 5:.1f}" text-anchor="{anchor}" fill="{DIM}">{text}</text>')

        label((9.3, RAIL_Z), f"rail plane, {st['rail']:.2f} over the seat", -45, 96, "end")
        label((-5.8, RAIL_Z - LAND_DROP), f"land, {LAND_DROP:g} under", 10, 78)
        label((-TUNNEL_W / 2 + 0.5, RAIL_Z + TUNNEL_H), f"tunnel {TUNNEL_W:g} x {TUNNEL_H:g}", -70, -92, "end")
        if st["key"] == "bump":
            label((0.0, z0 + BUMP["top"]), f"top {BUMP['top']:.2f}; radius {R_BUMP:.2f}, centre {APEX:g} under the apex", -40, -168)
            label((BUMP["corner"][0], z0 + BUMP["corner"][1]), f"corner {BUMP['corner'][1]:.2f}", 104, 62)
            label((STRIP_BUMP * T30, z0 + STRIP_BUMP), f"strip {STRIP_BUMP:g}", 96, -116)
            label((0.0, z0), "apex in the 60° V", 70, 34)
        elif st["key"] == "dent":
            label((0.0, z0 + DENT["low"]), f"bottom {DENT['low']:.2f}, on the rail plane; radius {R_DENT:.2f}", -40, -176)
            label((DENT["corner"][0], z0 + DENT["corner"][1]), f"corner {DENT['corner'][1]:.2f}", 110, 74)
            label((STRIP_DENT * T30, z0 + STRIP_DENT), f"strip {STRIP_DENT:g}", 110, -122)
            label((0.0, z0), "apex in the 60° V", 70, 34)
        else:
            label((0.0, z0 + POINT["flat"]), f"flat {POINT['flat']:.2f}: the point {POINT_OFF:g} off, {POINT['flat_w']:.2f} wide", -40, -150)
            label((INNER_SIDE / 2, z0), f"base {INNER_SIDE:.2f}, channel {INNER_SIDE + 2 * CHANNEL_PLAY:.2f}", 34, 44)
            label((0.0, z0 + INNER_ALT), f"strip {INNER_ALT:.2f}", 120, -110)

    for k, st in enumerate(STATIONS):
        panel(k, st)
    y = top + ph + 38
    for i, (col, text) in enumerate(((PLA, "block"), (WOOD, "stick, finished"), ("#fff", "stock, dashed"), (SLED, "sled"), (STEEL, "blade"))):
        s.append(f'<rect x="{gapx + i * 190}" y="{y - 14}" width="24" height="17" fill="{col}" stroke="#444" stroke-width="0.8"/><text x="{gapx + i * 190 + 32}" y="{y}" fill="{INK}">{text}</text>')
    s.append("</svg>")
    with open(path, "w") as f:
        f.write("\n".join(s))


def svg_blades(path):
    """The three blades to file, ten times size, edge down."""
    sc, bw = 30.0, 420
    w, h = 3 * bw + 60, 560
    s = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="Helvetica, Arial, sans-serif" font-size="15">',
         '<rect width="100%" height="100%" fill="#fff"/>',
         f'<text x="30" y="34" font-size="21" font-weight="bold" fill="{INK}">The blades: {BLADE_W:g} x {BLADE_H:g} blanks of {BLADE_T} mm scraper steel, edge down, filed square across</text>',
         '<text x="30" y="58" fill="#555">scribe the curve from the master\'s end, file to the line, and try it on the master against the light; only the curve has to be right, the depth is set on the master</text>']
    notes = {
        "hollow": [f"hollow, radius {R_BUMP:.2f}", f"filed {NOTCH:g} up from the shoulders (1.0 to 1.3)", f"{2 * math.sqrt(R_BUMP ** 2 - (R_BUMP - NOTCH) ** 2):.1f} wide at the mouth", "cuts the bump stick's round face"],
        "cove": [f"tongue, radius {R_DENT:.2f}", f"{NOTCH:g} proud of the shoulders (1.0 to 1.3)", f"{2 * math.sqrt(R_DENT ** 2 - (R_DENT - NOTCH) ** 2):.1f} wide at the root", "cuts the dent stick's hollow face"],
        "straight": ["straight", "set flush with the sole on anything flat", "", "takes the point off"],
    }
    for k, blade in enumerate(("hollow", "cove", "straight")):
        ox, oy = 30 + k * bw + bw / 2, 430
        e = edge_of(blade)
        lo = min(y for _, y in e)
        pts = [(x, y - lo) for x, y in e] + [(BLADE_W / 2, 10.0), (-BLADE_W / 2, 10.0)]
        s.append('<path d="M ' + " L ".join(f"{ox + x * sc:.1f},{oy - y * sc:.1f}" for x, y in pts) + f' Z" fill="{STEEL}" fill-opacity="0.85" stroke="#000"/>')
        zig = " ".join(f"l {BLADE_W * sc / 24:.1f},{-12 if i % 2 == 0 else 12}" for i in range(24))
        s.append(f'<path d="M {ox - BLADE_W / 2 * sc:.1f},{oy - 10 * sc + 6:.1f} {zig}" fill="none" stroke="#fff" stroke-width="5"/>')
        for i, t in enumerate(notes[blade]):
            s.append(f'<text x="{ox}" y="{oy + 34 + i * 21}" text-anchor="middle" fill="{DIM if i == 0 else INK}" font-weight="{"bold" if i == 0 else "normal"}">{t}</text>')
    s.append("</svg>")
    with open(path, "w") as f:
        f.write("\n".join(s))


# -- the parts ---------------------------------------------------------------
# Blocks, sticks and blades are drawn where they are used: x along the
# stick from the stop end, y across, z up from the bench. The sled and
# the master are drawn as they are printed.

SLED_AT = 100.0                                 # where the sled's front stands in the pictures


def block(p, st):
    sk = p.sketch("right", 0.0, "section")
    p.polygon(sk, block_outline())
    p.extrude(sk, BLOCK_L, name=f"{st['name']} block")
    sk = p.sketch("right", STOP, "seat")
    p.polygon(sk, groove_of(st))
    p.cut(sk, BLOCK_L - STOP + 1.0, name="the stick's seat, stopped short of the far end")
    sk = p.sketch("top", FLANGE_T, "screws")
    for x in (HOLE_IN, BLOCK_L - HOLE_IN):
        for y in (-HOLE_Y, HOLE_Y):
            p.point(sk, (x, y))
    p.hole(sk, HOLE_D, name="four screws to the bench")
    sk = p.sketch("top", FLANGE_T + 1.0, "which block")
    for lo, hi in notches_of(STATIONS.index(st)):
        p.rect(sk, lo, hi)
    p.cut(sk, FLANGE_T + 2.0, direction="reverse", name="notches: one bump, two dent, three point")


def sled(p):
    """Sole up, front at x = 0: its top face is on the bed."""
    sk = p.sketch("right", 0.0, "section")
    p.polygon(sk, sled_outline())
    p.extrude(sk, SLED_L, name="sled")
    # The slot leans its top toward the front; here the top is down.
    def front(zp):
        return slot_at(BODY_H - zp)
    sk = p.sketch("front", -SLOT_W / 2, "slot")
    p.polygon(sk, [(front(-1.0), -1.0), (front(-1.0) + SLOT_T, -1.0), (front(BODY_H + 1.0) + SLOT_T, BODY_H + 1.0), (front(BODY_H + 1.0), BODY_H + 1.0)])
    p.cut(sk, SLOT_W, name="blade slot, leaning")
    sk = p.sketch("right", 0.0, "clamp screw")
    p.circle(sk, (0.0, BODY_H - SCREW_Z), SCREW_D / 2)
    p.cut(sk, SCREW_L, name="M4 clearance, into the slot")
    sk = p.sketch("top", 0.0, "nut")
    p.rect(sk, (NUT_X, -NUT_W / 2), (NUT_X + NUT_T, NUT_W / 2))
    p.cut(sk, NUT_DEEP, name="nut trap, from the top")


def master(p, kind):
    """On end: its section is the outline the printer draws best."""
    sk = p.sketch("top", 0.0, "foot")
    p.rect(sk, (-BLOCK_W / 2 + FOOT_IN, FOOT_IN), (BLOCK_W / 2 - FOOT_IN, MASTER_H - 2 * FOOT_IN - LAND_DROP))
    p.extrude(sk, FOOT, name=f"{kind} master")
    sk = p.sketch("top", FOOT, "face")
    p.polygon(sk, master_outline(kind))
    p.extrude(sk, MASTER_L, op="add", name="the face and its rail edges")


def stick(p, st):
    sk = p.sketch("right", STOP, "section")
    p.polygon(sk, [(x, y + st["seat"]) for x, y in st["stick"]["section"]])
    p.extrude(sk, STICK, name=f"{st['name']} stick")


def blade(p, st):
    """In the sled at SLED_AT, on the slot's back wall, at its set."""
    th = BLADE_T / math.cos(math.radians(LEAN))  # the blade along the cut
    lo, hi = -2.0, BLADE_H * math.cos(math.radians(LEAN)) - 0.5

    def back(z):
        return SLED_AT + slot_at(z) + SLOT_T

    sk = p.sketch("front", -BLADE_W / 2, "blank")
    p.polygon(sk, [(back(lo) - th, RAIL_Z + lo), (back(lo), RAIL_Z + lo), (back(hi), RAIL_Z + hi), (back(hi) - th, RAIL_Z + hi)])
    p.extrude(sk, BLADE_W, name=f"{st['blade']} blade")
    e = [(x, y + RAIL_Z) for x, y in edge_of(st["blade"], 16)]
    w = BLADE_W / 2 + 1.0
    sk = p.sketch("right", SLED_AT + SLOT_X - 5.0, "edge")
    p.polygon(sk, [(-w, RAIL_Z + lo - 1.0), (w, RAIL_Z + lo - 1.0), (w, e[-1][1])] + e[::-1] + [(-w, e[0][1])])
    p.cut(sk, 12.0, name="filed to the master")


def faces_per_tile(mcp):
    """Which sticks of the kernel's cover carry a curved face: the hex
    tile's outline is three chords of a semicircle on every edge, a
    bump or a dent, and a stick with a side on a chord gets that arc."""
    o = [tuple(q) for q in json.loads(mcp.call("spectre_tile", {"edge": 1.0, "curve": "hex", "gap": 0.0}))["outline"]]
    cover = json.loads(mcp.call("spectre_mosaic", {"edge": EDGE, "gap": GAP, "inner": INNER}))["tile"]["pieces"]
    # the cover is in the tile's own frame, the outline is centred: one shift apart
    dx = min(q[0] for c in cover for q in c["outline"]) - min(q[0] for q in o)
    dy = min(q[1] for c in cover for q in c["outline"]) - min(q[1] for q in o)
    o = [(x + dx, y + dy) for x, y in o]
    if area(o) < 0:
        o.reverse()

    def inside(q):
        c = False
        for i, a in enumerate(o):
            b = o[(i + 1) % len(o)]
            if (a[1] > q[1]) != (b[1] > q[1]) and q[0] < a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]):
                c = not c
        return c

    def on(q, u, w):
        ab = (w[0] - u[0], w[1] - u[1])
        t = max(0.0, min(1.0, ((q[0] - u[0]) * ab[0] + (q[1] - u[1]) * ab[1]) / (ab[0] ** 2 + ab[1] ** 2)))
        return math.hypot(q[0] - u[0] - ab[0] * t, q[1] - u[1] - ab[1] * t) < 1e-6

    # the outline's corners in threes: the phase at which every fourth point is an edge's length on
    phase = next(ph for ph in range(3) if all(abs(math.dist(o[(ph + 3 * k) % 42], o[(ph + 3 * k + 3) % 42]) - 1.0) < 1e-9 for k in range(14)))
    out = {"bumps": 0, "dents": 0, "bump_sectors": 0, "dent_triangles": 0, "dent_chords_otherwise": 0, "bump_chords_otherwise": 0}
    for k in range(14):
        a, b = o[(phase + 3 * k) % 42], o[(phase + 3 * k + 3) % 42]
        m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
        chords = [(o[(phase + 3 * k + j) % 42], o[(phase + 3 * k + j + 1) % 42]) for j in range(3)]
        mid = ((chords[1][0][0] + chords[1][1][0]) / 2, (chords[1][0][1] + chords[1][1][1]) / 2)
        is_bump = inside((m[0] + (mid[0] - m[0]) * 0.9, m[1] + (mid[1] - m[1]) * 0.9))
        out["bumps" if is_bump else "dents"] += 1
        for u, w in chords:
            whole = [c for c in cover if c["shape"] == "triangle" and any(
                (math.dist(c["outline"][i], u) < 1e-6 and math.dist(c["outline"][(i + 1) % 3], w) < 1e-6)
                or (math.dist(c["outline"][i], w) < 1e-6 and math.dist(c["outline"][(i + 1) % 3], u) < 1e-6) for i in range(3))]
            if is_bump and whole and any(math.dist(q, m) < 1e-6 for q in whole[0]["outline"]):
                out["bump_sectors"] += 1
            elif not is_bump and whole:
                out["dent_triangles"] += 1
            else:
                out["bump_chords_otherwise" if is_bump else "dent_chords_otherwise"] += 1
    out["sticks"] = len(cover)
    return out


def placed(x, y, z, rx=0.0, rz=0.0):
    return {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": rx, "y": 0.0, "z": rz}}


def main():
    os.makedirs(OUT, exist_ok=True)
    ok = checks()
    path = os.path.join(OUT, "planes.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "planes"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Scratch-stock fixtures for the Spectre's sticks"}, {"type": "rename_tab", "tab": 1, "name": "Sled"}]})
    bodies, tabs = {}, {"Sled": 1}
    part = Part(mcp, 1, "Sled")
    sled(part)
    bodies["Sled"] = part.bodies()
    for kind in ("bump", "dent"):
        title = f"Master {kind}"
        tabs[title] = new_tab(mcp, {"type": "add_part_studio", "name": title})
        part = Part(mcp, tabs[title], title)
        master(part, kind)
        bodies[title] = part.bodies()
    for st in STATIONS:
        for title, make in ((f"Block {st['key']}", block), (f"Stick {st['key']}", stick), (f"Blade {st['blade']}", blade)):
            tabs[title] = new_tab(mcp, {"type": "add_part_studio", "name": title})
            part = Part(mcp, tabs[title], title)
            make(part, st)
            bodies[title] = part.bodies()
    for title, made in bodies.items():
        print(f"{title}: " + ", ".join(f"{name} {vol:.0f} mm3" for name, vol in made))
    faces = faces_per_tile(mcp)
    print(f"one tile of {faces['sticks']} sticks: {faces['bumps']} bumps and {faces['dents']} dents; {faces['bump_sectors']} sectors on the bumps, {faces['dent_triangles']} triangles in the dents, "
          f"{faces['dent_chords_otherwise']} dent chords on other sticks")
    # The stations side by side, a stick in each and a sled half way along it.
    asm = new_tab(mcp, {"type": "add_assembly", "name": "Stations"})
    pitch = 80.0
    ops = []
    for k, st in enumerate(STATIONS):
        y = k * pitch
        for title in (f"Block {st['key']}", f"Stick {st['key']}", f"Blade {st['blade']}"):
            ops.append({"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[title], "body": 0, "name": title, "fixed": True, "placement": placed(0.0, y, 0.0)}})
        # the sled turned over onto the rails: half a turn about x keeps its front toward the stop
        ops.append({"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": 1, "body": 0, "name": f"Sled, {st['blade']}", "fixed": True,
                                                              "placement": placed(SLED_AT, y, RAIL_Z + BODY_H, 180.0)}})
    # The masters as they come off the bed, on end, their faces to the front.
    for k, kind in enumerate(("bump", "dent")):
        ops.append({"type": "assembly", "tab": asm, "op": {"type": "add_instance", "studio": tabs[f"Master {kind}"], "body": 0, "name": f"Master {kind}", "fixed": True,
                                                              "placement": placed(40.0 + 60.0 * k, -70.0, 0.0, rz=180.0)}})
    text = mcp.call("apply", {"ops": ops})
    if "ERROR:" in text:
        raise SystemExit(text)
    mcp.call("screenshot", {"tab": asm, "view": "-0.5,-0.72,0.48", "width": 1600, "height": 1000, "path": os.path.join(OUT, "stations.png")})
    cut = SLED_AT + SLOT_X - 0.5                    # through the sled in front of the slot: the blade is seen down the tunnel
    for k, st in enumerate(STATIONS):
        y = k * pitch
        mcp.call("screenshot", {"tab": asm, "view": "-1,0,0", "section": f"x:{cut}", "fit": f"{cut},{y - 21},{RAIL_Z - 9},{cut + 1},{y + 21},{RAIL_Z + 12}",
                                "width": 1400, "height": 800, "path": os.path.join(OUT, f"section_{st['key']}.png")})
    mcp.call("screenshot", {"tab": 1, "view": "0.5,-0.6,0.62", "width": 1200, "height": 900, "path": os.path.join(OUT, "sled.png")})
    for kind in ("bump", "dent"):
        mcp.call("screenshot", {"tab": tabs[f"Master {kind}"], "view": "0.4,0.55,0.75", "width": 1000, "height": 900, "path": os.path.join(OUT, f"master_{kind}.png")})
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "path": os.path.join(OUT, "planes.pdf"),
                              "note": f"scratch-stock stations for the Spectre's sticks at edge {EDGE:g}: bump radius {R_BUMP:.2f}, dent radius {R_DENT:.2f}, point {POINT_OFF:g} off; blades of {BLADE_T} mm scraper steel"}))
    for title, name in (("Sled", "sled"), ("Master bump", "master_bump"), ("Master dent", "master_dent")) + tuple((f"Block {st['key']}", f"block_{st['key']}") for st in STATIONS):
        print(mcp.call("export", {"tab": tabs[title], "format": "stl", "path": os.path.join(OUT, f"{name}.stl")}))
    mcp.close()
    svg_stations(os.path.join(OUT, "stations.svg"))
    svg_blades(os.path.join(OUT, "blades.svg"))
    numbers = {
        "edge": EDGE, "gap": GAP, "inner": INNER, "stick": STICK, "side": SIDE, "alt": ALT, "r_bump": R_BUMP, "r_dent": R_DENT, "apex": APEX, "point_off": POINT_OFF,
        "inner_side": INNER_SIDE, "inner_alt": INNER_ALT,
        "blade": {"t": BLADE_T, "w": BLADE_W, "h": BLADE_H, "lean": LEAN, "notch": NOTCH},
        "bump": {"top": BUMP["top"], "corner": BUMP["corner"], "centre": BUMP["centre"], "strip": STRIP_BUMP, "section": BUMP["section"]},
        "dent": {"low": DENT["low"], "corner": DENT["corner"], "centre": DENT["centre"], "strip": STRIP_DENT, "section": DENT["section"]},
        "point": {"flat": POINT["flat"], "flat_w": POINT["flat_w"], "strip": INNER_ALT, "section": POINT["section"]},
        "stations": [{"key": st["key"], "blade": st["blade"], "rail": st["rail"], "seat": st["seat"], "lift": lift_of(st),
                      # the seat as far as the block goes: under the land
                      "seat_cut": clip_below(groove_of(st), RAIL_Z - LAND_DROP), "edge": edge_of(st["blade"], 16)} for st in STATIONS],
        "block": {"l": BLOCK_L, "w": BLOCK_W, "rail_z": RAIL_Z, "land_w": LAND_W, "land_drop": LAND_DROP, "flange_w": FLANGE_W, "flange_t": FLANGE_T, "stop": STOP,
                  "hole_d": HOLE_D, "holes": [[x, y] for x in (HOLE_IN, BLOCK_L - HOLE_IN) for y in (-HOLE_Y, HOLE_Y)], "relief": [RELIEF_W, RELIEF_D], "notch_w": NOTCH_W, "outline": block_outline()},
        "sled": {"l": SLED_L, "body_h": BODY_H, "slot_x": SLOT_X, "slot_t": SLOT_T, "slot_w": SLOT_W, "tunnel": [TUNNEL_W, TUNNEL_H], "skirt": [SKIRT_T, SKIRT_D], "play": PLAY,
                 "screw": {"z": SCREW_Z, "d": SCREW_D, "l": SCREW_L}, "nut": {"x": NUT_X, "t": NUT_T, "w": NUT_W, "deep": NUT_DEEP}, "at": SLED_AT, "outline": sled_outline()},
        "master": {"l": MASTER_L, "h": MASTER_H, "foot": FOOT, "foot_in": FOOT_IN, "plateau": PLATEAU,
                   "foot_area": (BLOCK_W - 2 * FOOT_IN) * (MASTER_H - 3 * FOOT_IN - LAND_DROP), "bump": master_outline("bump"), "dent": master_outline("dent")},
        "checks": ok, "per_tile": faces,
        "bodies": {k: [list(b) for b in v] for k, v in bodies.items()},
    }
    with open(os.path.join(OUT, "planes.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    print(f"bump stick: top {BUMP['top']:.2f}, corners {BUMP['corner'][1]:.2f}, radius {R_BUMP:.2f}; dent stick: bottom {DENT['low']:.2f}, corners {DENT['corner'][1]:.2f}, radius {R_DENT:.2f}; point {POINT_OFF:g} off at {POINT['flat']:.2f}")
    print(f"blocks {BLOCK_L:.0f} x {FLANGE_W:.0f} x {RAIL_Z:.0f}, sled {SLED_L:.0f} x {BLOCK_W + 2 * (PLAY + SKIRT_T):.1f} x {BODY_H + SKIRT_D:.0f}, masters {BLOCK_W:.0f} x {MASTER_H + BUMP['top'] - RAIL_BUMP:.1f} x {MASTER_L + FOOT:.0f}")


if __name__ == "__main__":
    main()
