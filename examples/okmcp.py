"""The example build scripts' MCP client: `ok-mcp` driven over stdio the
way a language model drives it, and a `Part` that turns a part studio's
ops into method calls and parses the ids out of the `apply` text.

    sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
    from okmcp import Mcp, Part, new_tab, v2

`Part` is one tab; `new_tab(mcp, {"type": "add_part_studio", "name":
...})` (or `add_assembly`) adds another and returns its id, parsed from
the `tab N` in the reply, for a second `Part` or the assembly helpers
at the end of this file. `examples/README.md` has the ten-line tour.

Needs only Python's standard library and the release `ok-mcp` binary
(`cargo build --release -p ok-mcp`, or `OK_MCP=/path/to/ok-mcp`). The
photo tooling is the same: `measuring-sheet/build.py` and
`finch-plug/trace/trace.py` are standard library too; the Node scripts
beside the latter want Playwright's Chromium, which `apps/web`'s `npm
ci` installs (`NODE_PATH=apps/web/node_modules node grab.js ...`), and
`turntable-scan/build.py` wants `ffmpeg` on the path.
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


# -- flip books: the `flipbook` tool's frames -------------------------------------


def clip_to(poly, convex):
    """`poly` cut down to what of it is inside the convex polygon
    `convex`, either way round (Sutherland and Hodgman)."""
    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(convex, convex[1:] + convex[:1]))
    edge = list(convex) if area > 0 else list(convex)[::-1]
    out = list(poly)
    for a, b in zip(edge, edge[1:] + edge[:1]):
        side = lambda p: (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        was, out = out, []
        for p, q in zip(was, was[1:] + was[:1]):
            sp, sq = side(p), side(q)
            if sp >= 0:
                out.append(p)
            if (sp >= 0) != (sq >= 0):
                t = sp / (sp - sq)
                out.append((p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])))
        if not out:
            return []
    return out


def tint(colour, toward, t):
    """A colour "#rrggbb" mixed toward another by t, 0 to 1."""
    a, b = (tuple(int(c[k:k + 2], 16) for k in (1, 3, 5)) for c in (colour, toward))
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(a, b))


def cutting_book(blank, cuts, tail=48, shades=12, ms=70, hold=(1200, 3000), blank_colour="#e2be8a", cut_colour="#1f4fd8",
                 background="#ffffff", line=1.5):
    """Frames for the `flipbook` tool that show a blank being cut, a
    frame a cut. The background is plain; what is left of the blank is
    one colour; the cut being made is solid in another; the cuts before
    it are outlines in that colour, fainter the older they are, gone
    into the background after `tail` of them. The fading is in `shades`
    steps: an outline then changes only now and then, and since the
    file holds only what changes from frame to frame, a dozen shades
    make it a third the size of a shade a cut.

    `blank` is a convex polygon. `cuts` is a list of polygons, each the
    cutter's section where that cut has it (it may run on past the
    blank and out of the picture), or of (polygon, ms) to hold a cut
    longer than the rest. After the last cut the outlines fade away, a
    shade a frame, and the finished piece is held.

    Returns the frames and the number of the first cut's frame: cut k
    is frame k + that. Pass the frames to `flipbook` with `background`
    and a `view` about the blank."""
    r3 = lambda poly: [[round(x, 3), round(y, 3)] for x, y in poly]
    shapes, times = [], []
    for c in cuts:
        held = len(c) == 2 and isinstance(c[1], (int, float))
        shapes.append(r3(c[0] if held else c))
        times.append(int(c[1]) if held else ms)
    edges = [r3(clip_to(s, blank)) for s in shapes]         # what of each cut is in the blank: its outline once it is made
    rubbed = lambda k: {"points": shapes[k], "fill": background}

    each = max(1, round(tail / max(1, shades)))             # cuts to a shade
    most = -(-tail // each)                                 # shades, in the end

    def before(k, older=0):
        """The cuts before cut k, the oldest first, each as faint as it is old."""
        out = []
        for j in range(max(0, k - tail + older), k):
            shade = -(-(k - j + older) // each)
            if len(edges[j]) > 1 and shade <= most:
                out.append({"points": edges[j], "stroke": tint(cut_colour, background, shade / (most + 1)), "width": line})
        return out

    frames = [{"keep": [{"points": r3(blank), "fill": blank_colour}], "ms": hold[0]}]
    for k, s in enumerate(shapes):
        frames.append({"keep": [rubbed(k - 1)] if k else [], "draw": before(k) + [{"points": s, "fill": cut_colour}], "ms": times[k]})
    for older in range(0, tail, each):
        frames.append({"keep": [rubbed(len(shapes) - 1)] if older == 0 and shapes else [], "draw": before(len(shapes), older), "ms": ms})
    frames.append({"ms": hold[1]})
    return frames, 1
