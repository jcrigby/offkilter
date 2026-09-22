#!/usr/bin/env python3
"""Builds the trim-router lift and pin-router project as an offkilter
document by driving `ok-mcp` over stdio, the way a language model does:
every part is a tab of ops (sketch, extrude, hole, revolve, ...), applied
through the MCP `apply` tool, checked through `report`, pictured through
`screenshot` and exported through `export`.

    cargo build --release -p ok-mcp
    python3 examples/router-lift/build.py [out-dir]

Writes router_lift.okpart, a PNG per part and an STL per printed part
into out-dir (default: examples/router-lift/out). The dimensions are
those of the rev C OpenSCAD models in reference/ (trim_router_lift.scad,
pin_arm.scad); the STLs there are the references the CI test compares
the kernel's parts against.
"""

import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
MCP = os.environ.get("OK_MCP", os.path.join(ROOT, "target", "release", "ok-mcp"))


class Mcp:
    """A minimal MCP client over stdio: initialize, then tools/call."""

    def __init__(self, path):
        self.proc = subprocess.Popen(
            [MCP, "--file", path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.next_id = 0
        self.rpc(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "router-lift build", "version": "0"},
            },
        )
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, msg):
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def rpc(self, method, params):
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("ok-mcp exited")
            msg = json.loads(line)
            if msg.get("id") == self.next_id:
                if "error" in msg:
                    raise RuntimeError(msg["error"])
                return msg["result"]

    def call(self, tool, args):
        result = self.rpc("tools/call", {"name": tool, "arguments": args})
        text = "".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
        if result.get("isError"):
            raise RuntimeError(f"{tool}: {text}")
        return text

    def close(self):
        self.proc.stdin.close()
        self.proc.wait()


def v2(x, y):
    return {"x": x, "y": y}


class Part:
    """One part studio tab: ops go through `apply`, ids come back parsed
    from its text."""

    def __init__(self, mcp, tab, name):
        self.mcp, self.tab, self.name = mcp, tab, name

    def apply(self, *ops):
        text = self.mcp.call("apply", {"ops": list(ops), "tab": self.tab})
        if "ERROR:" in text:
            failed = text.split("ERROR:", 1)[1].splitlines()[0].strip()
            raise RuntimeError(f"{self.name}: {failed}")
        ids, self.entities = [], []
        for line in text.splitlines():
            m = re.match(r"op \d+: ok(?: \((.*)\))?", line)
            if not m:
                continue
            made = m.group(1) or ""
            f = re.search(r"feature (\d+)", made)
            ids.append(int(f.group(1)) if f else None)
            e = re.search(r"entities ([\d,]+)", made)
            self.entities.append([int(x) for x in e.group(1).split(",")] if e else [])
        return ids

    def feature(self, op):
        return self.apply(op)[0]

    # -- sketches -----------------------------------------------------

    def sketch(self, base, offset, name):
        return self.feature(
            {
                "type": "add_sketch",
                "plane": {"type": "standard", "base": base, "offset": offset},
                "name": name,
            }
        )

    def draw(self, sketch, op):
        self.apply({"type": "sketch", "id": sketch, "op": op})

    def rect(self, s, a, b):
        self.draw(s, {"type": "add_rectangle", "a": v2(*a), "b": v2(*b)})

    def circle(self, s, c, r):
        self.draw(s, {"type": "add_circle", "center": v2(*c), "radius": r})

    def point(self, s, p):
        self.draw(s, {"type": "add_point", "pos": v2(*p)})

    def polygon(self, s, pts):
        """Lines around `pts`; returns their entity ids."""
        ops = []
        for i, a in enumerate(pts):
            b = pts[(i + 1) % len(pts)]
            ops.append({"type": "sketch", "id": s, "op": {"type": "add_line", "a": v2(*a), "b": v2(*b)}})
        self.apply(*ops)
        return [e[0] for e in self.entities]

    def hexagon(self, s, c, circumradius):
        """A hexagon with a vertex along the sketch's y axis (an OpenSCAD
        six-sided cylinder turned onto its side has one along Z)."""
        self.draw(
            s,
            {"type": "add_polygon", "center": v2(*c), "vertex": v2(c[0], c[1] + circumradius), "sides": 6},
        )

    def slot(self, s, a, b, width):
        self.draw(s, {"type": "add_slot", "a": v2(*a), "b": v2(*b), "width": width})

    # -- features -----------------------------------------------------

    def extrude(self, s, depth, direction="normal", profiles="all", op="new", name=None, through_all=False):
        prof = {"type": profiles} if profiles in ("all", "largest") else {"type": "indices", "indices": profiles}
        return self.feature(
            {
                "type": "add_extrude",
                "sketch": s,
                "depth": depth,
                "direction": direction,
                "end": {"type": "through_all"} if through_all else {"type": "blind"},
                "profiles": prof,
                "op": op,
                "name": name,
            }
        )

    def cut(self, s, depth, direction="normal", name=None, through_all=False):
        return self.extrude(s, depth, direction, "all", "remove", name, through_all)

    def hole(self, s, diameter, depth=0.0, direction="reverse", counterbore=None, countersink=None, name=None):
        """Drills every point of the sketch; `counterbore` is
        {diameter, depth} and `countersink` {diameter, angle} (the
        diameter at the sketch plane and the included angle in degrees)."""
        return self.feature(
            {
                "type": "add_hole",
                "sketch": s,
                "diameter": diameter,
                "depth": depth,
                "through_all": depth <= 0.0,
                "direction": direction,
                "counterbore": counterbore,
                "countersink": countersink,
                "name": name,
            }
        )

    def revolve_cut(self, s, name=None):
        """Revolves the sketch's closed profile about the sketch's y axis
        and removes it: a cone, countersink or turned socket."""
        return self.feature(
            {"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "remove", "name": name}
        )

    # -- readback -----------------------------------------------------

    def report(self):
        return json.loads(self.mcp.call("report", {"tab": self.tab, "detail": "full"}))

    def bodies(self):
        r = self.report()
        return [(b.get("name"), b.get("volume")) for b in r.get("bodies", [])]

    def screenshot(self, path, view="iso", section=None):
        args = {"tab": self.tab, "view": view, "width": 900, "height": 700, "path": path}
        if section:
            args["section"] = section
        self.mcp.call("screenshot", args)

    def export(self, path, fmt="stl", view=None, hidden=False):
        args = {"tab": self.tab, "format": fmt, "path": path}
        if view:
            args["view"] = view
            args["hidden"] = hidden
        self.mcp.call("export", args)


# ---------------------------------------------------------------------
# Dimensions (trim_router_lift.scad and pin_arm.scad, rev C defaults).
# ---------------------------------------------------------------------
ROUTER_D, ROUTER_CLR, CLAMP_H = 65.0, 0.3, 66.0
BLK_L, BLK_BX, BLK_BZ, BLK_GAP, BLK_W = 45.0, 40.0, 35.0, 10.0, 50.0
BLK_BOLT = 5.0
WALL = 8.0
NUT_FLANGE_D, NUT_BODY_D, NUT_FLANGE_T, NUT_PCD = 22.0, 10.2, 3.5, 16.0
SLOT_W, CLAMP_BOLT_D, CLAMP_EAR = 2.5, 5.4, 14.0
NUT_AF, NUT_T, TAP3 = 8.4, 4.6, 2.5
DISC_D, RABBET_W, RABBET_D = 90.0, 8.0, 5.0

LS_Y = ROUTER_D / 2 + WALL + NUT_FLANGE_D / 2 + 2  # 53.5
CAR_W = ROUTER_D + 2 * WALL + 20  # 101
CAR_H = 2 * BLK_L + BLK_GAP  # 100
BODY_Y0 = -(ROUTER_D / 2 + WALL)  # -40.5
BODY_Y1 = LS_Y + NUT_FLANGE_D / 2 + WALL  # 72.5
EAR_W = 2 * (SLOT_W / 2 + WALL + CLAMP_BOLT_D)  # 29.3
OPEN_D = DISC_D - 2 * RABBET_W  # 74

POST_W, POST_H, SOCKET_D, SLOT_LEN, CONE_H, CONE_D = 40.0, 75.0, 10.3, 4.0, 6.0, 16.0
CONE_ANGLE = 2 * math.degrees(math.atan((CONE_D - SOCKET_D) / 2 / CONE_H))  # included, 50.8
CHUCK_W, CHUCK_DP, CHUCK_H, PIN_D = 36.0, 30.0, 25.0, 6.35


def carriage(p):
    """Router clamp with flat side faces for the SC20UU blocks and the
    T8 nut mount (module carriage())."""
    zc = CAR_H / 2
    bore_r = ROUTER_D / 2 + ROUTER_CLR
    # Body outline with the clamp ears, minus the router bore.
    s = p.sketch("top", 0.0, "outline")
    p.polygon(
        s,
        [
            (-CAR_W / 2, BODY_Y0),
            (-EAR_W / 2, BODY_Y0),
            (-EAR_W / 2, BODY_Y0 - CLAMP_EAR),
            (EAR_W / 2, BODY_Y0 - CLAMP_EAR),
            (EAR_W / 2, BODY_Y0),
            (CAR_W / 2, BODY_Y0),
            (CAR_W / 2, BODY_Y1),
            (-CAR_W / 2, BODY_Y1),
        ],
    )
    p.circle(s, (0.0, 0.0), bore_r)
    p.extrude(s, CAR_H, profiles="largest", name="body")
    # Clamp slot from the front of the ears into the bore.
    s = p.sketch("top", -1.0, "slot")
    p.rect(s, (-SLOT_W / 2, BODY_Y0 - CLAMP_EAR - 1), (SLOT_W / 2, BODY_Y0 + WALL + 1))
    p.cut(s, CAR_H + 2, name="clamp slot")
    # Leadscrew nut: body clearance through, flange recess in the top face.
    s = p.sketch("top", CAR_H, "nut")
    p.point(s, (0.0, LS_Y))
    p.hole(
        s,
        NUT_BODY_D + 0.6,
        counterbore={"diameter": NUT_FLANGE_D + 0.6, "depth": NUT_FLANGE_T + 0.3},
        name="nut bore + recess",
    )
    s = p.sketch("top", CAR_H, "nut screws")
    for a in (45.0, 135.0, 225.0, 315.0):
        p.point(s, (NUT_PCD / 2 * math.cos(math.radians(a)), LS_Y + NUT_PCD / 2 * math.sin(math.radians(a))))
    p.hole(s, TAP3, depth=12.0, name="M3 tap holes")
    # SC20UU bolt holes, eight per side face, 18 deep, and the nut traps:
    # one slot per bolt column, from the top face for the upper block and
    # the bottom face for the lower one, the nut 8.3 mm behind the face.
    zs = [zc + i * (BLK_L + BLK_GAP) / 2 + zz * BLK_BZ / 2 for i in (-1, 1) for zz in (-1, 1)]
    for offset, direction, label in ((CAR_W / 2, "reverse", "+X"), (-CAR_W / 2, "normal", "-X")):
        s = p.sketch("right", offset, f"block holes {label}")
        for y in (-BLK_BX / 2, BLK_BX / 2):
            for z in zs:
                p.point(s, (y, z))
        p.hole(s, BLK_BOLT + 0.5, depth=18.0, direction=direction, name=f"M5 clearance {label}")
    xi, xo = CAR_W / 2 - 6 - NUT_T, CAR_W / 2 - 6
    z_in = zc + (BLK_L + BLK_GAP) / 2 - BLK_BZ / 2 - 5
    z_out = zc - (BLK_L + BLK_GAP) / 2 + BLK_BZ / 2 + 5
    s = p.sketch("top", CAR_H, "upper nut traps")
    for sx in (-1, 1):
        for y in (-BLK_BX / 2, BLK_BX / 2):
            p.rect(s, (sx * xi, y - NUT_AF / 2), (sx * xo, y + NUT_AF / 2))
    p.cut(s, CAR_H - z_in, direction="reverse", name="upper nut traps")
    s = p.sketch("top", -1.0, "lower nut traps")
    for sx in (-1, 1):
        for y in (-BLK_BX / 2, BLK_BX / 2):
            p.rect(s, (sx * xi, y - NUT_AF / 2), (sx * xo, y + NUT_AF / 2))
    p.cut(s, z_out + 1, name="lower nut traps")
    # Clamp bolts through both ears along X, hex nut traps on the -X ear.
    bolt_y = BODY_Y0 - CLAMP_EAR / 2
    bolt_z = (zc - CLAMP_H * 0.22, zc + CLAMP_H * 0.22)
    s = p.sketch("right", -EAR_W / 2 - 5, "clamp bolts")
    for z in bolt_z:
        p.point(s, (bolt_y, z))
    p.hole(s, CLAMP_BOLT_D, direction="normal", name="M5 clamp bolts")
    s = p.sketch("right", -EAR_W / 2, "hex traps")
    for z in bolt_z:
        p.hexagon(s, (bolt_y, z), (8 / math.cos(math.radians(30)) + 0.4) / 2)
    p.cut(s, 4.0, name="M5 nut traps")
    # Lightening pockets from below between the bore and the side faces,
    # leaving 8 mm skins.
    s = p.sketch("top", -1.0, "pockets")
    for sx in (-1, 1):
        p.rect(s, (sx * (CAR_W / 2 - 9) - 3, -BLK_W / 2 + 10), (sx * (CAR_W / 2 - 9) + 3, BLK_W / 2 - 10))
    p.cut(s, CAR_H - 8, name="lightening pockets")


def ring(p, opening):
    """Flush reducer ring for the 90 mm opening (module ring()): a disc
    in the rabbet on a spigot in the through-opening, two spanner holes.
    `opening` 0 is the blank; 6.65 the alignment ring's pin hole. Print
    orientation, spigot down from z = 0, as the reference STLs are."""
    s = p.sketch("top", 4.0, "disc")
    p.circle(s, (0.0, 0.0), (DISC_D - 0.4) / 2)
    p.extrude(s, RABBET_D, name="disc")
    s = p.sketch("top", 0.0, "spigot")
    p.circle(s, (0.0, 0.0), (OPEN_D - 0.6) / 2)
    p.extrude(s, 4.01, op="add", name="spigot")
    if opening > 0:
        s = p.sketch("top", 4.0 + RABBET_D, "opening")
        p.point(s, (0.0, 0.0))
        p.hole(s, opening, name="opening")
    s = p.sketch("top", 4.0 + RABBET_D, "spanner holes")
    for x in (DISC_D / 2 - 9, -(DISC_D / 2 - 9)):
        p.point(s, (x, 0.0))
    p.hole(s, 5.0, name="spanner holes")


def cone_cut(p, plane, offset, profile, name):
    """A body of revolution removed about an axis through the sketch's
    origin: `profile` is the half section as (radius, height) pairs, the
    axis edge closing it. `plane` "front" with `offset` puts the axis at
    y = -offset, "right" at x = offset."""
    s = p.sketch(plane, offset, name)
    pts = [(0.0, profile[0][1])] + list(profile) + [(0.0, profile[-1][1])]
    p.polygon(s, pts)
    p.revolve_cut(s, name)


def post(p, slot):
    """Registration post (module post()): a 40 x 40 x 75 block whose top
    carries a socket for a 10 mm dowel with a conical entry, slotted 4 mm
    in X on one of the pair, and two countersunk screws to the table."""
    s = p.sketch("top", 0.0, "block")
    p.rect(s, (-POST_W / 2, -POST_W / 2), (POST_W / 2, POST_W / 2))
    p.extrude(s, POST_H, name="block")
    # Straight socket: 17 tall under the cone (16 below the cone's base),
    # with a conical entry from the socket diameter up to cone_d at the
    # top face. On the round post that is the hole's countersink; the
    # slotted post hulls two of them 4 mm apart, which is two revolved
    # cones plus the section swept between them.
    s = p.sketch("top", POST_H, "socket")
    if slot:
        p.slot(s, (-SLOT_LEN / 2, 0.0), (SLOT_LEN / 2, 0.0), SOCKET_D)
        p.cut(s, CONE_H + 16, direction="reverse", name="socket")
        cone = [(SOCKET_D / 2, POST_H - CONE_H), (CONE_D / 2, POST_H + 0.01)]
        for x in (-SLOT_LEN / 2, SLOT_LEN / 2):
            cone_cut(p, "right", x, cone, "cone entry")
    else:
        p.point(s, (0.0, 0.0))
        p.hole(s, SOCKET_D, depth=CONE_H + 16, countersink={"diameter": CONE_D, "angle": CONE_ANGLE}, name="socket")
    if slot:
        s = p.sketch("right", -SLOT_LEN / 2, "cone sweep")
        p.polygon(
            s,
            [
                (-SOCKET_D / 2, POST_H - CONE_H),
                (SOCKET_D / 2, POST_H - CONE_H),
                (CONE_D / 2, POST_H + 0.01),
                (-CONE_D / 2, POST_H + 0.01),
            ],
        )
        p.cut(s, SLOT_LEN, name="cone sweep")
    # Two screws to the table top, 4.5 through with the SCAD's cone at the
    # bottom face (as modelled there: d 4.5 at z = 0 to d 9 at z = 3).
    s = p.sketch("top", POST_H, "screws")
    for y in (-13.0, 13.0):
        p.point(s, (0.0, y))
    p.hole(s, 4.5, name="screw holes")
    for y in (-13.0, 13.0):
        cone_cut(p, "front", -y, [(2.25, -1.0), (2.25, 0.0), (4.5, 3.0)], "screw cone")


def chuck(p):
    """Split clamp for the 1/4" guide pin under the arm's nose (module
    chuck()): bore, split to the back face, M4 clamp bolt across, two
    countersunk screws up into the arm."""
    s = p.sketch("top", 0.0, "block")
    p.rect(s, (-CHUCK_W / 2, -CHUCK_DP / 2), (CHUCK_W / 2, CHUCK_DP / 2))
    p.extrude(s, CHUCK_H, name="block")
    s = p.sketch("top", CHUCK_H, "bore")
    p.point(s, (0.0, 0.0))
    p.hole(s, PIN_D + 0.2, name="pin bore")
    s = p.sketch("top", -1.0, "split")
    p.rect(s, (-1.0, 0.0), (1.0, CHUCK_DP / 2 + 1))
    p.cut(s, CHUCK_H + 2, name="split")
    s = p.sketch("right", -CHUCK_W / 2 - 1, "clamp bolt")
    p.point(s, (CHUCK_DP / 2 - 8, CHUCK_H / 2))
    p.hole(s, 4.3, direction="normal", name="M4 clamp bolt")
    # The screws' countersinks open on the bottom face (the SCAD's cone is
    # d 9 at z = -1 to d 4.5 at z = 3), so the holes are drilled up from it.
    s = p.sketch("top", 0.0, "screws")
    for x in (-12.0, 12.0):
        p.point(s, (x, 0.0))
    p.hole(s, 4.5, direction="normal", countersink={"diameter": 7.875, "angle": 2 * math.degrees(math.atan(2.25 / 4.0))}, name="countersunk screws")



# ---------------------------------------------------------------------
# Wood parts (build-instructions.md sizes; part frames as in the SCAD).
# ---------------------------------------------------------------------
TOP_W, TOP_D, TOP_T, TOP_Y_OFF = 400.0, 380.0, 38.0, 50.0
LS_D, BRG_OD, BRG_T = 8.0, 22.0, 7.0
PLY_T, RAIL_T, BASE_DP, BASE_Y0 = 19.0, 19.0, 145.0, -60.0
SK_T, SK_H, SK_HOLE, SK_W, SK_HTOT = 20.0, 51.0, 42.0, 60.0, 70.0
SHAFT_D, TRAVEL = 20.0, 45.0
BLK_H, BLK_C = 42.0, 25.0
SHAFT_X = CAR_W / 2 + BLK_C  # 75.5
RAIL_X = SHAFT_X + SK_H  # 126.5
BASE_W = 2 * RAIL_X  # 253
Z_TOP = SK_T + 3 + CAR_H + TRAVEL + 3 + SK_T  # 191: underside of the top
Z_CAR = SK_T + 3 + TRAVEL / 2  # 45.5: carriage bottom, mid travel
RAIL_H = Z_TOP + PLY_T  # 210
ARM_T, ARM_CLEAR, RAIL_D, RAIL_W, RAIL_Y1 = 38.0, 75.0, 60.0, 250.0, 240.0
NOSE_W, NOSE_Y, REG_X, REG_Y = 80.0, -50.0, 60.0, 70.0
RAIL_Y0 = RAIL_Y1 - RAIL_D  # 180: the hinge line


def top(p):
    """The table top: laminated ply with the ring rabbet, the through
    opening, the leadscrew hole and the 608 pocket in its underside."""
    s = p.sketch("top", 0.0, "blank")
    p.rect(s, (-TOP_W / 2, -TOP_D / 2 + TOP_Y_OFF), (TOP_W / 2, TOP_D / 2 + TOP_Y_OFF))
    p.extrude(s, TOP_T, name="top")
    s = p.sketch("top", TOP_T, "opening")
    p.point(s, (0.0, 0.0))
    p.hole(s, OPEN_D, counterbore={"diameter": DISC_D + 0.4, "depth": RABBET_D}, name="opening + rabbet")
    s = p.sketch("top", TOP_T, "leadscrew")
    p.point(s, (0.0, LS_Y))
    p.hole(s, LS_D + 4, name="leadscrew hole")
    s = p.sketch("top", 0.0, "bearing pocket")
    p.point(s, (0.0, LS_Y))
    p.hole(s, BRG_OD, depth=BRG_T, direction="normal", name="608 pocket")


def baseplate(p):
    """The box floor: leadscrew hole (13, so only the bearing's outer
    race bears on the pocket floor) and the 608 pocket in its top face."""
    s = p.sketch("top", 0.0, "blank")
    p.rect(s, (-BASE_W / 2, BASE_Y0), (BASE_W / 2, BASE_Y0 + BASE_DP))
    p.extrude(s, PLY_T, name="baseplate")
    s = p.sketch("top", PLY_T, "leadscrew")
    p.point(s, (0.0, LS_Y))
    p.hole(s, 13.0, counterbore={"diameter": BRG_OD, "depth": BRG_T}, name="leadscrew hole + 608 pocket")


def side_rail(p):
    """A box side: the SK20 supports bolt to its inner face."""
    s = p.sketch("top", 0.0, "blank")
    p.rect(s, (-RAIL_T / 2, BASE_Y0), (RAIL_T / 2, BASE_Y0 + BASE_DP))
    p.extrude(s, RAIL_H, name="rail")
    s = p.sketch("right", -RAIL_T / 2 - 1, "SK20 holes")
    for z in (PLY_T + SK_T / 2, PLY_T + Z_TOP - SK_T / 2):
        for y in (-SK_HOLE / 2, SK_HOLE / 2):
            p.point(s, (y, z))
    p.hole(s, 6.6, direction="normal", name="M6 holes")


def arm(p):
    """The hinged pin arm: a laminated plate, wide at the hinge, tapering
    to the nose, with the registration dowel holes and the chuck screw
    pilots in its underside."""
    s = p.sketch("top", ARM_CLEAR, "plan")
    p.polygon(
        s,
        [
            (-RAIL_W / 2, RAIL_Y1),
            (RAIL_W / 2, RAIL_Y1),
            (RAIL_W / 2, RAIL_Y0),
            (NOSE_W / 2, NOSE_Y),
            (-NOSE_W / 2, NOSE_Y),
            (-RAIL_W / 2, RAIL_Y0),
        ],
    )
    p.extrude(s, ARM_T, name="arm")
    s = p.sketch("top", ARM_CLEAR, "dowels")
    for x in (-REG_X, REG_X):
        p.point(s, (x, REG_Y))
    p.hole(s, 9.9, depth=20.0, direction="normal", name="dowel press fits")
    s = p.sketch("top", ARM_CLEAR, "chuck screws")
    for x in (-12.0, 12.0):
        p.point(s, (x, 0.0))
    p.hole(s, 3.5, depth=29.0, direction="normal", name="chuck screw pilots")


def rear_rail(p):
    """The hinge rail along the back edge of the top."""
    s = p.sketch("top", 0.0, "blank")
    p.rect(s, (-RAIL_W / 2, RAIL_Y0), (RAIL_W / 2, RAIL_Y1))
    p.extrude(s, ARM_CLEAR, name="rail")


# ---------------------------------------------------------------------
# Bought parts, as simply as the assembly needs them.
# ---------------------------------------------------------------------
def cylinder(p, d, h, name, z0=0.0, bore=None):
    s = p.sketch("top", z0, name)
    p.circle(s, (0.0, 0.0), d / 2)
    if bore:
        p.circle(s, (0.0, 0.0), bore / 2)
    p.extrude(s, h, profiles="largest" if bore else "all", name=name)


def shaft(p):
    cylinder(p, SHAFT_D, Z_TOP, "20 mm shaft")


def leadscrew(p):
    cylinder(p, LS_D, 286.0, "T8 leadscrew")


def bearing_608(p):
    cylinder(p, BRG_OD, BRG_T, "608 bearing", bore=LS_D)


def collar(p):
    cylinder(p, 16.0, 9.0, "shaft collar", bore=LS_D)


def coupling_nut(p):
    s = p.sketch("top", 0.0, "hex")
    p.hexagon(s, (0.0, 0.0), 14.3 / math.cos(math.radians(30)) / 2)
    p.circle(s, (0.0, 0.0), (LS_D + 0.5) / 2)
    p.extrude(s, 28.6, profiles="largest", name="coupling nut")


def t8_nut(p):
    cylinder(p, NUT_BODY_D, 12.0, "nut body")
    s = p.sketch("top", 12.0, "flange")
    p.circle(s, (0.0, 0.0), NUT_FLANGE_D / 2)
    p.extrude(s, NUT_FLANGE_T, op="add", name="flange")
    s = p.sketch("top", 12.0 + NUT_FLANGE_T, "bore")
    p.point(s, (0.0, 0.0))
    p.hole(s, LS_D, name="bore")


def sc20uu(p):
    """Closed pillow block: base at x = 0 facing -X, the shaft bore
    vertical at x = 25, four M5 bolt holes in the base."""
    s = p.sketch("front", 0.0, "block")
    p.rect(s, (0.0, -BLK_L / 2), (BLK_H, BLK_L / 2))
    p.extrude(s, BLK_W, direction="symmetric", name="block")
    s = p.sketch("top", BLK_L / 2, "bore")
    p.point(s, (BLK_C, 0.0))
    p.hole(s, SHAFT_D, name="shaft bore")
    s = p.sketch("right", -1.0, "bolts")
    for y in (-BLK_BX / 2, BLK_BX / 2):
        for z in (-BLK_BZ / 2, BLK_BZ / 2):
            p.point(s, (y, z))
    p.hole(s, BLK_BOLT, depth=15.0, direction="normal", name="M5 bolt holes")


def sk20(p):
    """Shaft support: base at x = 0 facing -X, shaft bore vertical at
    x = 51, split clamp beyond it, two M6 holes through the base."""
    s = p.sketch("top", -SK_T / 2, "outline")
    p.rect(s, (0.0, -SK_W / 2), (12.0, SK_W / 2))
    p.extrude(s, SK_T, name="base")
    s = p.sketch("top", -SK_T / 2, "body")
    p.rect(s, (0.0, -16.0), (SK_HTOT, 16.0))
    p.extrude(s, SK_T, op="add", name="body")
    s = p.sketch("top", SK_T / 2, "bore")
    p.point(s, (SK_H, 0.0))
    p.hole(s, SHAFT_D, name="shaft bore")
    s = p.sketch("top", SK_T / 2 + 1, "split")
    p.rect(s, (SK_H, -1.0), (SK_HTOT + 1, 1.0))
    p.cut(s, SK_T + 2, direction="reverse", name="clamp split")
    s = p.sketch("right", -1.0, "holes")
    for y in (-SK_HOLE / 2, SK_HOLE / 2):
        p.point(s, (y, 0.0))
    p.hole(s, 6.6, depth=15.0, direction="normal", name="M6 holes")


def router_body(p):
    """The trim router motor, collet and a 1/4 inch bit, as a ghost."""
    cylinder(p, ROUTER_D, CLAMP_H + 60, "motor")
    s = p.sketch("top", CLAMP_H + 60, "collet")
    p.circle(s, (0.0, 0.0), 12.0)
    p.extrude(s, 14.0, op="add", name="collet")
    s = p.sketch("top", CLAMP_H + 74, "bit")
    p.circle(s, (0.0, 0.0), PIN_D / 2)
    p.extrude(s, 12.0, op="add", name="bit")


def reg_pin(p):
    """A 10 mm dowel, 40 long, one end chamfered 2 mm: a revolved
    profile, the chamfer a sketch op on the profile's corner."""
    s = p.sketch("front", 0.0, "profile")
    side, end = p.polygon(s, [(0.0, 0.0), (5.0, 0.0), (5.0, 40.0), (0.0, 40.0)])[1:3]
    p.draw(s, {"type": "chamfer", "a": side, "b": end, "distance": 2.0})
    p.feature({"type": "add_revolve", "sketch": s, "axis": {"type": "y_axis"}, "angle": 360, "op": "new", "name": "dowel"})


def guide_pin(p):
    cylinder(p, PIN_D, 75.0, "guide pin")


PARTS = [
    # (tab title, file stem, builder); the printed parts have reference STLs
    ("Carriage", "carriage", carriage),
    ("Ring blank", "ring_blank", lambda p: ring(p, 0.0)),
    ("Ring 30", "ring_30", lambda p: ring(p, 30.0)),
    ("Ring 40", "ring_40", lambda p: ring(p, 40.0)),
    ("Ring 55", "ring_55", lambda p: ring(p, 55.0)),
    ("Ring align", "ring_align", lambda p: ring(p, 6.6)),
    ("Post round", "pin_post_round", lambda p: post(p, False)),
    ("Post slot", "pin_post_slot", lambda p: post(p, True)),
    ("Chuck", "pin_chuck", chuck),
    ("Top", "top", top),
    ("Baseplate", "baseplate", baseplate),
    ("Side rail", "side_rail", side_rail),
    ("Arm", "arm", arm),
    ("Rear rail", "rear_rail", rear_rail),
    ("Shaft", "shaft", shaft),
    ("Leadscrew", "leadscrew", leadscrew),
    ("608 bearing", "bearing_608", bearing_608),
    ("Collar", "collar", collar),
    ("Coupling nut", "coupling_nut", coupling_nut),
    ("T8 nut", "t8_nut", t8_nut),
    ("SC20UU", "sc20uu", sc20uu),
    ("SK20", "sk20", sk20),
    ("Router", "router", router_body),
    ("Dowel pin", "reg_pin", reg_pin),
    ("Guide pin", "guide_pin", guide_pin),
]
PRINTED = {"Carriage", "Ring blank", "Ring 30", "Ring 40", "Ring 55", "Ring align", "Post round", "Post slot", "Chuck"}

# The assembly: every instance placed as the SCAD assembly() places it,
# in the lift's frame (z = 0 the baseplate's top face, the bit axis the
# origin, +Y towards the back). The pin attachment's frame has z = 0 at
# the table surface, which is z_top + top_t here.
TABLE = Z_TOP + TOP_T  # 229
LS_Z0 = -(PLY_T + 9.0 + 1.0)


def instances():
    at = lambda tab, name, x, y, z, rx=0.0, ry=0.0, rz=0.0: (tab, name, (x, y, z), (rx, ry, rz))
    out = [
        at("Baseplate", "baseplate", 0, 0, -PLY_T),
        at("Side rail", "left rail", -(RAIL_X + RAIL_T / 2), 0, -PLY_T),
        at("Side rail", "right rail", RAIL_X + RAIL_T / 2, 0, -PLY_T),
        at("Top", "top", 0, 0, Z_TOP),
        at("Ring 40", "ring", 0, 0, TABLE - RABBET_D - 4.0),
        at("Shaft", "left shaft", -SHAFT_X, 0, 0),
        at("Shaft", "right shaft", SHAFT_X, 0, 0),
        at("Leadscrew", "leadscrew", 0, LS_Y, LS_Z0),
        at("608 bearing", "lower bearing", 0, LS_Y, -BRG_T),
        at("608 bearing", "upper bearing", 0, LS_Y, Z_TOP),
        at("Collar", "upper collar", 0, LS_Y, 0),
        at("Collar", "lower collar", 0, LS_Y, -PLY_T - 9.0),
        at("Carriage", "carriage", 0, 0, Z_CAR),
        at("T8 nut", "T8 nut", 0, LS_Y, Z_CAR + CAR_H - NUT_FLANGE_T - 12.0),
        at("Coupling nut", "coupling nut", 0, LS_Y, TABLE + 2),
        at("Router", "router", 0, 0, Z_CAR + CAR_H / 2 - CLAMP_H / 2 - 10),
        at("Rear rail", "rear rail", 0, 0, TABLE),
        at("Arm", "arm", 0, 0, TABLE),
        at("Post round", "left post", -REG_X, REG_Y, TABLE),
        at("Post slot", "right post", REG_X, REG_Y, TABLE),
        at("Chuck", "chuck", 0, 0, TABLE + ARM_CLEAR - CHUCK_H),
        at("Guide pin", "guide pin", 0, 0, TABLE + ARM_CLEAR - CHUCK_H - 45.0),
    ]
    for sx, side in ((-1, "left"), (1, "right")):
        for i, level in ((-1, "lower"), (1, "upper")):
            z = Z_CAR + CAR_H / 2 + i * (BLK_L + BLK_GAP) / 2
            out.append(at("SC20UU", f"{side} {level} block", sx * CAR_W / 2, 0, z, rz=180.0 if sx < 0 else 0.0))
        for z, level in ((SK_T / 2, "lower"), (Z_TOP - SK_T / 2, "upper")):
            out.append(at("SK20", f"{side} {level} support", sx * RAIL_X, 0, z, rz=180.0 if sx > 0 else 0.0))
        out.append(at("Dowel pin", f"{side} dowel", sx * REG_X, REG_Y, TABLE + ARM_CLEAR + 20.0, rx=180.0))
    return out


# ---------------------------------------------------------------------
# Mates: the carriage rides its pillow blocks on the shafts.
#
# Everything bolted to the carriage moves with it. One slider mate
# between a block's bore and its shaft carries the travel; fastened
# mates through bolt holes, the router bore and the nut pocket hang the
# rest off it. The mate parameters are derived from the placements in
# instances(), which stay on the instances as the initial guess, so the
# resolved assembly must land exactly where the fixed one did.
# ---------------------------------------------------------------------
MOVING = {
    "carriage",
    "T8 nut",
    "router",
    "left lower block",
    "left upper block",
    "right lower block",
    "right upper block",
}

# (kind, placed instance, moving instance, radius on each, name). The
# cylinders are matched by radius and by being coaxial once placed.
MATES = [
    ("slider", "left shaft", "left upper block", SHAFT_D / 2, SHAFT_D / 2, "carriage travel"),
    ("fastened", "left upper block", "carriage", BLK_BOLT / 2, (BLK_BOLT + 0.5) / 2, "carriage on left upper block"),
    ("fastened", "carriage", "left lower block", (BLK_BOLT + 0.5) / 2, BLK_BOLT / 2, "left lower block"),
    ("fastened", "carriage", "right upper block", (BLK_BOLT + 0.5) / 2, BLK_BOLT / 2, "right upper block"),
    ("fastened", "carriage", "right lower block", (BLK_BOLT + 0.5) / 2, BLK_BOLT / 2, "right lower block"),
    ("fastened", "carriage", "router", ROUTER_D / 2 + ROUTER_CLR, ROUTER_D / 2, "router in clamp"),
    ("fastened", "carriage", "T8 nut", NUT_BODY_D / 2 + 0.3, NUT_BODY_D / 2, "nut in pocket"),
]


def _v(d):
    return (d["x"], d["y"], d["z"])


def _add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _scale(a, k):
    return (a[0] * k, a[1] * k, a[2] * k)


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _norm(a):
    return _scale(a, 1.0 / math.sqrt(_dot(a, a)))


def _rotation(rx, ry, rz):
    """The matrix of a placement's rotation: x, then y, then z (degrees)."""
    a, b, c = (math.radians(t) for t in (rx, ry, rz))
    ca, sa, cb, sb, cc, sc = math.cos(a), math.sin(a), math.cos(b), math.sin(b), math.cos(c), math.sin(c)
    return (
        (cc * cb, cc * sb * sa - sc * ca, cc * sb * ca + sc * sa),
        (sc * cb, sc * sb * sa + cc * ca, sc * sb * ca - cc * sa),
        (-sb, cb * sa, cb * ca),
    )


def _apply(m, p):
    return tuple(_dot(row, p) for row in m)


def _connector_frame(cyl, face):
    """The connector frame of a cylindrical face, as the kernel builds it:
    origin at the face's middle projected onto the axis, z along the axis,
    x and y canonical for that axis (Plane::from_origin_normal)."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    origin = _add(o, _scale(z, _dot(_sub(c, o), z)))
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    x = _cross(y, z)
    return origin, x, y, z


def _placed_frame(frame, placement):
    (x, y, z), (rx, ry, rz) = placement
    m = _rotation(rx, ry, rz)
    o, fx, fy, fz = frame
    return _add(_apply(m, o), (x, y, z)), _apply(m, fx), _apply(m, fy), _apply(m, fz)


def _mate_parameters(target, moving):
    """offset, angle and flip that put `moving` (a world frame) where the
    mate rule puts it from `target`: z opposed unless flip, x turned by
    angle about z, origin `offset` along the target's normal."""
    ot, xt, _, zt = target
    om, xm, _, zm = moving
    flip = _dot(zt, zm) > 0.0
    z = zt if flip else _scale(zt, -1.0)
    offset = _dot(_sub(om, ot), zt)
    angle = math.degrees(math.atan2(_dot(_cross(xt, xm), z), _dot(xt, xm)))
    return offset, angle, flip


def _coaxial(a_report, a_placement, b_report, b_placement, ra, rb):
    """The first pair of cylinders of radius ra on a and rb on b whose axes
    coincide once both are placed, with their placed connector frames."""
    faces = lambda rep: {json.dumps(f["reference"], sort_keys=True): f for f in rep["bodies"][0]["faces"]}
    fa, fb = faces(a_report), faces(b_report)
    for ca in a_report["bodies"][0]["cylinders"]:
        if abs(ca["radius"] - ra) > 1e-6:
            continue
        ta = _placed_frame(_connector_frame(ca, fa[json.dumps(ca["reference"], sort_keys=True)]), a_placement)
        for cb in b_report["bodies"][0]["cylinders"]:
            if abs(cb["radius"] - rb) > 1e-6:
                continue
            tb = _placed_frame(_connector_frame(cb, fb[json.dumps(cb["reference"], sort_keys=True)]), b_placement)
            parallel = math.sqrt(_dot(_cross(ta[3], tb[3]), _cross(ta[3], tb[3]))) < 1e-9
            d = _sub(tb[0], ta[0])
            off_axis = _sub(d, _scale(ta[3], _dot(d, ta[3])))
            if parallel and math.sqrt(_dot(off_axis, off_axis)) < 1e-6:
                return ca, cb, ta, tb
    raise RuntimeError(f"no coaxial cylinders of radii {ra} and {rb}")


def mates(mcp, asm, tabs, placements, reports):
    """Adds MATES to the assembly and returns the id of the travel slider."""
    asm_report = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
    ids = {i["name"]: i["id"] for i in asm_report["instances"]}
    studio = {i["name"]: i["studio"] for i in asm_report["instances"]}
    title = {tab: t for t, tab in tabs.items()}
    ops = []
    for kind, a, b, ra, rb, name in MATES:
        ca, cb, ta, tb = _coaxial(reports[title[studio[a]]], placements[a], reports[title[studio[b]]], placements[b], ra, rb)
        offset, angle, flip = _mate_parameters(ta, tb)
        ops.append(
            {
                "type": "add_mate",
                "kind": kind,
                "a": {"instance": ids[a], "face": ca["reference"]},
                "b": {"instance": ids[b], "face": cb["reference"]},
                "offset": offset,
                "angle": angle,
                "flip": flip,
                "name": name,
            }
        )
    text = mcp.call("apply", {"ops": ops, "tab": asm})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    # Every instance must have resolved to the placement it was drawn at.
    asm_report = json.loads(mcp.call("report", {"tab": asm, "detail": "full"}))
    for m in asm_report["mates"]:
        if m.get("error"):
            raise RuntimeError(f"mate {m['name']}: {m['error']}")
    worst, moved = 0.0, []
    for i in asm_report["instances"]:
        if i.get("error"):
            raise RuntimeError(f"instance {i['name']}: {i['error']}")
        (x, y, z), (rx, ry, rz) = placements[i["name"]]
        got = i["placed"]
        d = _sub(_v(got["position"]), (x, y, z))
        want, have = _rotation(rx, ry, rz), _rotation(*_v(got["rotation"]))
        err = max(math.sqrt(_dot(d, d)), max(abs(want[r][c] - have[r][c]) for r in range(3) for c in range(3)))
        worst = max(worst, err)
        if err > 1e-6:
            moved.append(f"{i['name']}: at {_v(got['position'])} rotated {_v(got['rotation'])}, drawn at {(x, y, z)} rotated {(rx, ry, rz)}")
    if moved:
        raise RuntimeError("mates moved instances from their placements:\n  " + "\n  ".join(moved))
    slider = next(m["id"] for m in asm_report["mates"] if m["name"] == "carriage travel")
    print(f"mates         tab {asm:>2}: {len(ops)} mates, {len(MOVING)} moving instances resolved to their placements (worst {worst:.1e} mm)")
    return slider, ops[0]["offset"]


def dxf_lines(path):
    """The LINE entities of a DXF as ((x1, y1), (x2, y2)) pairs."""
    toks = [t.strip() for t in open(path).read().splitlines()]
    out, i = [], 0
    while i + 1 < len(toks):
        if toks[i] == "0" and toks[i + 1] == "LINE":
            vals, j = {}, i + 2
            while j + 1 < len(toks) and toks[j] != "0":
                vals[toks[j]] = toks[j + 1]
                j += 2
            out.append(((float(vals["10"]), float(vals["20"])), (float(vals["11"]), float(vals["21"]))))
            i = j
        else:
            i += 1
    return out


def check_template(exported, reference):
    """Every line of the shop's arm template must be a line of the
    exported plan view (either way round)."""
    got = dxf_lines(exported)
    same = lambda p, q: abs(p[0] - q[0]) < 1e-6 and abs(p[1] - q[1]) < 1e-6
    missing = [
        (a, b)
        for a, b in dxf_lines(reference)
        if not any((same(a, c) and same(b, d)) or (same(a, d) and same(b, c)) for c, d in got)
    ]
    if missing:
        raise RuntimeError(f"arm template lines missing from the exported plan: {missing}")
    return len(got)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "out")
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, "router_lift.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    # The file backend names the file after the document.
    mcp.call("create_document", {"name": "router_lift"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Router lift"}]})
    tabs, reports = {}, {}
    for i, (title, stem, build) in enumerate(PARTS):
        if i == 0:
            mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": title}]})
            tab = 1
        else:
            text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": title}]})
            m = re.search(r"tab (\d+)", text)
            if not m:
                raise RuntimeError(f"no tab id in: {text}")
            tab = int(m.group(1))
        tabs[title] = tab
        part = Part(mcp, tab, title)
        build(part)
        reports[title] = part.report()
        bodies = [(b.get("name"), b.get("volume")) for b in reports[title].get("bodies", [])]
        print(f"{title:<13} tab {tab:>2}: " + ", ".join(f"{v:.0f} mm3" for _, v in bodies))
        part.screenshot(os.path.join(out, f"{stem}.png"))
        if title in PRINTED:
            part.export(os.path.join(out, f"{stem}.stl"))
        if title == "Arm":
            # The plan view at 1:1 is the template the instructions say to
            # print full size; the reference DXF's lines must all be in it.
            part.export(os.path.join(out, "arm_template.dxf"), "dxf", view="top")
            n = check_template(os.path.join(out, "arm_template.dxf"), os.path.join(HERE, "reference", "arm_template.dxf"))
            print(f"{'':<13} arm_template.dxf: {n} lines, the reference template's 6 among them")
    # The assembly.
    text = mcp.call("apply", {"ops": [{"type": "add_assembly", "name": "Lift assembly"}]})
    asm = int(re.search(r"tab (\d+)", text).group(1))
    ops, placements = [], {}
    for tab_title, name, (x, y, z), (rx, ry, rz) in instances():
        placements[name] = ((x, y, z), (rx, ry, rz))
        ops.append(
            {
                "type": "add_instance",
                "studio": tabs[tab_title],
                "body": 0,
                "name": name,
                "fixed": name not in MOVING,
                "placement": {"position": {"x": x, "y": y, "z": z}, "rotation": {"x": rx, "y": ry, "z": rz}},
            }
        )
    text = mcp.call("apply", {"ops": ops, "tab": asm})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    print(f"assembly      tab {asm:>2}: {len(ops)} instances")
    slider, mid = mates(mcp, asm, tabs, placements, reports)
    for view, name in (("iso", "assembly_iso"), ("-1,-0.4,0.35", "assembly_front")):
        mcp.call("screenshot", {"tab": asm, "view": view, "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    # Cut at the bit axis, keeping the far half so the cut faces face the camera.
    mcp.call("screenshot", {"tab": asm, "view": "iso", "section": "x:0:flip", "width": 1200, "height": 900, "path": os.path.join(out, "assembly_section.png")})
    # Shop drawings: the assembly sheet with balloons and a parts list,
    # and the carriage's own sheet with its holes called out.
    print(mcp.call("export", {"tab": asm, "format": "pdf", "sheet": "A3", "note": "rev C", "path": os.path.join(out, "assembly.pdf")}))
    print(mcp.call("export", {"tab": tabs["Carriage"], "format": "pdf", "path": os.path.join(out, "carriage.pdf")}))
    # The carriage at the top of its travel: the slider's offset is the
    # one number that moves it and everything bolted to it.
    for offset, name in ((mid + TRAVEL / 2, "assembly_raised"), (mid, None)):
        text = mcp.call("apply", {"ops": [{"type": "set_mate", "id": slider, "offset": offset}], "tab": asm})
        if "ERROR:" in text:
            raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
        if name:
            mcp.call("screenshot", {"tab": asm, "view": "iso", "section": "x:0:flip", "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
