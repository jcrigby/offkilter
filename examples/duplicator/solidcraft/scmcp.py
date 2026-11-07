"""A small stdio MCP client for `solidcraft-cli mcp`, the SolidCraft
counterpart of `examples/okmcp.py`: start the headless server, call its
tools, and a `Part` that collects the primitives of one part and sends
them as one `batch`.

SolidCraft has no part studios: every body lives in one design and a
cut takes every body it meets unless `targets` names the ones it may
touch, so `Part` passes its own bodies as the targets of its cuts and
joins, which is what a studio gave offkilter for free. Bodies are then
moved into a component named after the part, and components nest for
sub-assemblies.
"""
import json
import os
import subprocess
import sys
import time


class Mcp:
    def __init__(self, exe, allow, extra=()):
        self.proc = subprocess.Popen(
            [exe, "mcp", "--headless", "--allow-path", allow, *extra],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(os.path.join(allow, "mcp.stderr"), "w"), text=True, bufsize=1,
        )
        self.id = 0
        self.calls, self.seconds = 0, 0.0
        self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "duplicator", "version": "1"}})
        self.notify("notifications/initialized")

    def notify(self, method, params=None):
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def rpc(self, method, params):
        self.id += 1
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n")
        self.proc.stdin.flush()
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("solidcraft-cli mcp closed its stdout")
            msg = json.loads(line)
            if msg.get("id") == self.id:
                if "error" in msg:
                    raise RuntimeError(json.dumps(msg["error"]))
                return msg["result"]

    def call(self, tool, args=None, raw=False):
        """Call a tool; the parsed JSON of its text block, or raise on isError."""
        t = time.time()
        r = self.rpc("tools/call", {"name": tool, "arguments": args or {}})
        self.calls += 1
        self.seconds += time.time() - t
        texts = [c["text"] for c in r.get("content", []) if c.get("type") == "text"]
        if r.get("isError"):
            raise RuntimeError(f"{tool}: " + "\n".join(texts)[:4000])
        if raw:
            return r
        try:
            return json.loads(texts[0]) if texts else None
        except (json.JSONDecodeError, IndexError):
            return texts

    def execute(self, command, params=None, verbose=False):
        return self.call("execute", {"command": command, "params": params or {}, "verbose": verbose})

    def batch(self, commands, verbose=False, expect=None):
        args = {"commands": commands, "verbose": verbose}
        if expect:
            args["expect"] = expect
        r = self.call("batch", args)
        if not r.get("ok", True):
            f = r.get("failed", {})
            raise RuntimeError(f"batch step {f.get('index')} {f.get('command')}: {f.get('error')}")
        return r

    def close(self):
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=30)
        except Exception:
            self.proc.kill()


class Part:
    """The bodies of one part, built in place in world coordinates, as a
    list of primitive commands sent as one batch. `op` is offkilter's:
    "new" starts a body, "add" joins the last new body, and the cuts take
    the part's bodies as their targets."""

    def __init__(self, mcp, title, parent="root"):
        self.mcp, self.title, self.parent = mcp, title, parent
        self.cmds, self.bodies, self.current = [], [], None
        self.n = 0

    def _name(self, name):
        self.n += 1
        return f"{self.title} / {self.n} {name}"

    def _solid(self, cmd, params, name, op):
        if op == "new":
            body = self._name(name)
            params.update(operation="new", body_name=body, name=body)
            self.bodies.append(body)
            self.current = body
        elif op == "add":
            params.update(operation="join", targets=[self.current], name=self._name(name))
        elif op == "cut":
            params.update(operation="cut", targets=list(self.bodies), name=self._name(name))
        else:
            raise ValueError(op)
        self.cmds.append({"command": cmd, "params": params})

    def box(self, name, x, y, z, op="new"):
        self._solid("solid.box", {"length": x[1] - x[0], "width": y[1] - y[0], "height": z[1] - z[0], "corner": [x[0], y[0], z[0]]}, name, op)

    def cyl(self, name, base, axis, d, h, op="new"):
        self._solid("solid.cylinder", {"diameter": d, "height": h, "base": list(base), "axis": list(axis)}, name, op)

    def vcyl(self, name, cx, cy, z0, z1, d, op="new"):
        self.cyl(name, (cx, cy, z0), (0, 0, 1), d, z1 - z0, op)

    def xcyl(self, name, x0, x1, cy, cz, d, op="new"):
        self.cyl(name, (x0, cy, cz), (1, 0, 0), d, x1 - x0, op)

    def ycyl(self, name, y0, y1, cx, cz, d, op="new"):
        self.cyl(name, (cx, y0, cz), (0, 1, 0), d, y1 - y0, op)

    def bore(self, name, center, axis, d, length):
        """A cylinder cut through the part's bodies, `length` long centred on `center` along `axis`."""
        a = axis
        base = [c - a_i * length / 2 for c, a_i in zip(center, a)]
        self.cyl(name, base, a, d, length, op="cut")

    def cut_box(self, name, x, y, z):
        self.box(name, x, y, z, op="cut")

    def prism_x(self, name, x0, pts, thickness, op="new"):
        """A polygon in the plane x = x0, points as (y, z), extruded `thickness` along +x."""
        sk = self._name(name + " sketch")
        self.cmds.append({"command": "sketch.create", "params": {"plane": "YZ", "offset": x0, "name": sk}})
        self.cmds.append({"command": "sketch.line", "params": {"points": [[p[0], p[1]] for p in pts], "closed": True}})
        self.cmds.append({"command": "sketch.finish", "params": {}})
        self._solid("solid.extrude", {"distance": thickness, "sketch": sk, "direction": "positive"}, name, op)

    def build(self):
        """Send the batch, move the bodies into the part's component, and
        return the measure summary of its bodies."""
        self.cmds.append({"command": "component.create", "params": {"name": self.title, "parent": self.parent, "activate": False, "grounded": False}})
        self.cmds.append({"command": "component.move_bodies", "params": {"bodies": list(self.bodies), "component": self.title}})
        r = self.mcp.batch(self.cmds)
        return r
