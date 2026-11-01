"""The example build scripts' MCP client: `ok-mcp` driven over stdio the
way a language model drives it, and a `Part` that turns a part studio's
ops into method calls and parses the ids out of the `apply` text.

    sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
    from okmcp import Mcp, Part, v2
"""
import json
import math
import os
import re
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
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

    def sketch_on(self, face, name):
        """A sketch on a planar face of a body: its frame is the world
        origin projected onto the face with canonical axes, so on a face
        normal to Z the coordinates are the standard top plane's."""
        return self.feature({"type": "add_sketch", "plane": {"type": "face", "face": face, "offset": 0.0}, "name": name})

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

    def gear(self, module, teeth, width, base="top", offset=0.0, plane=None, center=(0.0, 0.0), pressure_angle=20.0,
             direction="normal", bore=0.0, rim=0.0, angle=0.0, backlash=0.0, shift=0.0, fillet=0.0, cone=0.0, op="new", name=None):
        """An involute spur gear as a body: external with a `bore`, or
        internal (teeth inside a ring of outer diameter `rim`) when `rim`
        is given; a `cone` angle (degrees, atan(teeth / mate's teeth) for
        shafts at 90°) makes it a straight bevel gear with its apex at
        `center` and `width` the face width along the cone. `plane` is a
        plane reference; otherwise a standard plane `base` at `offset`.
        `angle` turns the first tooth's centreline; a mating gear with an
        even tooth count wants 180 / teeth."""
        if plane is None:
            plane = {"type": "standard", "base": base, "offset": offset}
        return self.feature(
            {
                "type": "add_gear",
                "plane": plane,
                "center": v2(*center),
                "module": module,
                "teeth": teeth,
                "pressure_angle": pressure_angle,
                "width": width,
                "direction": direction,
                "bore": bore,
                "rim": rim,
                "angle": angle,
                "backlash": backlash,
                "shift": shift,
                "fillet": fillet,
                "cone": cone,
                "op": op,
                "name": name,
            }
        )

    def split(self, plane, name=None, bodies=None):
        """Splits the studio's bodies (or those from the features in
        `bodies`) by a plane reference; the pieces above the plane
        become new bodies."""
        op = {"type": "add_split", "plane": plane, "name": name}
        if bodies:
            op["bodies"] = bodies
        return self.feature(op)

    def plane_point(self, s, p):
        """A world point in the coordinates of sketch `s`'s plane."""
        plane = next(sk["plane"] for sk in self.report()["sketches"] if sk["feature"] == s)
        o, u, v = plane["origin"], plane["x_axis"], plane["y_axis"]
        d = (p[0] - o["x"], p[1] - o["y"], p[2] - o["z"])
        return (d[0] * u["x"] + d[1] * u["y"] + d[2] * u["z"], d[0] * v["x"] + d[1] * v["y"] + d[2] * v["z"])

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


# -- assemblies ---------------------------------------------------------
# What a build script needs to place bodies and mate them: the ops that
# go round `Part` (new tabs, assembly ops, the full report), and the
# connector frame of a cylindrical face as the kernel builds it (see
# docs/OPS.md, "A connector is a frame on its face"), so a mate's
# offset, angle and flip can be read off two bodies built in place.

IDENTITY = {"position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}}


def apply(mcp, ops, tab):
    """Ops on a tab (an assembly's, usually); raises on the first error."""
    text = mcp.call("apply", {"ops": ops, "tab": tab})
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    return text


def new_tab(mcp, op):
    """Adds a tab (add_part_studio, add_assembly) and returns its id."""
    text = mcp.call("apply", {"ops": [op]})
    return int(re.search(r"tab (\d+)", text).group(1))


def full(mcp, tab):
    return json.loads(mcp.call("report", {"tab": tab, "detail": "full"}))


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


def _frame(cyl, face):
    """A cylindrical face's connector frame as the kernel builds it."""
    o, z = _v(cyl["origin"]), _norm(_v(cyl["axis"]))
    c = _v(face["centroid"])
    t = _dot(_sub(c, o), z)
    origin = (o[0] + z[0] * t, o[1] + z[1] * t, o[2] + z[2] * t)
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = _norm(_cross(z, hint))
    return origin, _cross(y, z), y, z


def cylinder(body, r, near=None):
    """The cylinder of radius r on a body (from a full report), with its
    frame. With `near`, a world point, the one whose axis passes nearest
    it, within a millimetre; without, the first of that radius. Bodies
    are built in place, so the frame is the world one."""
    faces = {json.dumps(f["reference"], sort_keys=True): f for f in body["faces"]}
    best = None
    for c in body["cylinders"]:
        if abs(c["radius"] - r) > 1e-6:
            continue
        if near is None:
            best = (0.0, c)
            break
        o, z = _v(c["origin"]), _norm(_v(c["axis"]))
        d = _sub(near, o)
        t = _dot(d, z)
        off = _sub(d, (z[0] * t, z[1] * t, z[2] * t))
        dist = math.sqrt(_dot(off, off))
        if best is None or dist < best[0]:
            best = (dist, c)
    if best is None or best[0] > 1.0:
        where = f" near {near}" if near is not None else ""
        raise RuntimeError(f"{body['name']}: no cylinder of radius {r}{where}")
    c = best[1]
    return c, _frame(c, faces[json.dumps(c["reference"], sort_keys=True)])


def mate_parameters(target, moving):
    """offset, angle (degrees) and flip for a mate that puts the moving
    frame where it is now relative to the target frame."""
    ot, xt, _, zt = target
    om, xm, _, zm = moving
    flip = _dot(zt, zm) > 0.0
    z = zt if flip else tuple(-c for c in zt)
    return _dot(_sub(om, ot), zt), math.degrees(math.atan2(_dot(_cross(xt, xm), z), _dot(xt, xm))), flip
