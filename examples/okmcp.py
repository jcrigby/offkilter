"""The example build scripts' MCP client: `ok-mcp` driven over stdio the
way a language model drives it, and a `Part` that turns a part studio's
ops into method calls and parses the ids out of the `apply` text.

    sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
    from okmcp import Mcp, Part, v2
"""
import json
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
             direction="normal", bore=0.0, rim=0.0, angle=0.0, backlash=0.0, op="new", name=None):
        """An involute spur gear as a body: external with a `bore`, or
        internal (teeth inside a ring of outer diameter `rim`) when `rim`
        is given. `plane` is a plane reference; otherwise a standard
        plane `base` at `offset`. `angle` turns the first tooth's
        centreline; a mating gear with an even tooth count wants
        180 / teeth."""
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
                "op": op,
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
