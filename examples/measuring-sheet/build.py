#!/usr/bin/env python3
"""Writes the measuring sheets under out/ through the MCP server, and
measures a synthetic scan of one so the README's numbers are what the
tool says. Needs `cargo build --release -p ok-mcp` first."""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(HERE, "out")
MCP = os.path.join(ROOT, "target", "release", "ok-mcp")


class Mcp:
    def __init__(self, doc):
        self.p = subprocess.Popen(
            [MCP, "--file", doc], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
        )
        self.n = 0
        self.call("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "build", "version": "0"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def call(self, method, params):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())

    def tool(self, name, **args):
        r = self.call("tools/call", {"name": name, "arguments": args})["result"]
        text = "\n".join(c["text"] for c in r["content"] if c["type"] == "text")
        if r.get("isError"):
            sys.exit(f"{name}: {text}")
        return text


os.makedirs(OUT, exist_ok=True)
m = Mcp(os.path.join(OUT, "scratch.okpart"))
for size in ("Letter", "A4", "A3", "Tabloid"):
    print(m.tool("measuring_sheet", path=os.path.join(OUT, f"measuring-sheet-{size}.pdf"), sheet=size))
# scan.png is made by crates/ok-photo/tests/example.rs: a Letter sheet
# printed at 97 % with a plate, a washer, a disc and a photo scale on it,
# photographed askew. The sheet size comes from the dots by the origin
# mark; the scale's 10 mm bars give the print scale.
print(m.tool("measure_photo", path=os.path.join(HERE, "scan.png"), reference="bars 10", out=os.path.join(OUT, "measured.png")))
