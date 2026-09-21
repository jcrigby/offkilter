#!/usr/bin/env python3
"""A checkerboard jigsaw top for a puzzle box, built through the MCP
server, two ways: an 8 x 6 puzzle of 38 mm pieces, maple and walnut by
parity, each colour pin-routed from one board so the grain runs across
the whole field. The first tab has a 1.5 mm gap for a contrasting resin
fill and a 5 mm alignment web in the gaps; the second is cut tight,
with a strip between rows as wide as the bit so the corners route
clean, and a printable tray with a pocket per piece for the glue-up.

    cargo build --release -p ok-mcp
    python3 examples/puzzle-top/build.py

Writes out/puzzle_top.okpart, screenshots, the piece outlines as a DXF
at 1:1 (the routing templates) and the pieces as STL.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "router-lift"))
from build import Mcp  # noqa: E402  (the router lift's MCP client)

COLS, ROWS, PITCH, THICKNESS = 8, 6, 38.0, 12.0
GAP, WEB, BIT, LOCK, JITTER, SEED = 1.5, 5.0, 6.35, 20.0, 3.0, 42


def apply(mcp, ops, tab=1):
    text = mcp.call("apply", {"ops": ops, "tab": tab})
    return text


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "out")
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, "puzzle_top.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "puzzle_top"})
    apply(mcp, [{"type": "rename_document", "name": "Puzzle top"}, {"type": "rename_tab", "tab": 1, "name": "Top"}])
    text = apply(
        mcp,
        [
            {
                "type": "add_puzzle",
                "plane": {"type": "standard", "base": "top", "offset": 0},
                "cols": COLS,
                "rows": ROWS,
                "pitch": PITCH,
                "thickness": THICKNESS,
                "gap": GAP,
                "bit": BIT,
                "lock": LOCK,
                "grain": "x",
                "web": WEB,
                "seed": SEED,
                "jitter": JITTER,
                "name": "Checkerboard puzzle",
            }
        ],
    )
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    feature = int(text.split("feature ", 1)[1].split(")")[0].split(",")[0])
    report = json.loads(mcp.call("report", {"tab": 1, "detail": "full"}))
    bodies = report["bodies"]
    light = [b for b in bodies if b["name"].endswith("light")]
    dark = [b for b in bodies if b["name"].endswith("dark")]
    web = [b for b in bodies if b["name"] == "Alignment web"]
    print(f"{len(bodies)} bodies: {len(light)} maple, {len(dark)} walnut, {len(web)} web")
    print(f"maple  {sum(b['volume'] for b in light) / 1000:.1f} cm3, walnut {sum(b['volume'] for b in dark) / 1000:.1f} cm3, web {web[0]['volume'] / 1000:.1f} cm3")
    # A design the bit cannot cut is refused with every rule that fails.
    text = apply(mcp, [{"type": "set_puzzle", "id": feature, "bit": 12.7}])
    rules = json.loads(mcp.call("report", {"tab": 1, "detail": "full"}))["features"][0].get("error", "")
    print(f"with a 1/2\" bit: {len(rules.splitlines())} rules fail, e.g. {rules.splitlines()[0]!r}")
    apply(mcp, [{"type": "set_puzzle", "id": feature, "bit": BIT}])
    # Hand edits: flip one tab, fatten another, move a corner.
    apply(
        mcp,
        [
            {"type": "set_puzzle_tab", "id": feature, "edge": 3, "out": False},
            {"type": "set_puzzle_tab", "id": feature, "edge": 10, "size": 1.25, "shift": 0.42},
            {"type": "set_puzzle_corner", "id": feature, "node": 8, "offset": {"x": 5.0, "y": -4.0}},
        ],
    )
    report = json.loads(mcp.call("report", {"tab": 1, "detail": "full"}))
    if report["features"][0].get("error"):
        raise RuntimeError(report["features"][0]["error"])
    for view, name in (("iso", "puzzle_iso"), ("top", "puzzle_top"), ("0.3,-1,0.5", "puzzle_front")):
        mcp.call("screenshot", {"tab": 1, "view": view, "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    # The routing templates: every outline at 1:1, and the pieces as STL.
    print(mcp.call("export", {"tab": 1, "format": "dxf", "view": "top", "path": os.path.join(out, "templates.dxf")}))
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(out, "pieces.stl")}))
    # The tight version: no gap within a row, a strip between rows one
    # bit wide, no web, and a tray to print.
    text = mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": "Tight top"}]})
    tight = int(text.split("tab ", 1)[1].split(")")[0].split(",")[0])
    text = apply(
        mcp,
        [
            {
                "type": "add_puzzle",
                "plane": {"type": "standard", "base": "top", "offset": 0},
                "cols": COLS,
                "rows": ROWS,
                "pitch": PITCH,
                "thickness": THICKNESS,
                "gap": 0.0,
                "bit": BIT,
                "lock": LOCK,
                "grain": "x",
                "web": 0.0,
                "seed": SEED,
                "jitter": 2.0,
                "row_gap": BIT,
                "fixture": 6.0,
                "name": "Tight checkerboard",
            }
        ],
        tab=tight,
    )
    if "ERROR:" in text:
        raise RuntimeError(text.split("ERROR:", 1)[1].splitlines()[0])
    report = json.loads(mcp.call("report", {"tab": tight, "detail": "full"}))
    names = [b["name"] for b in report["bodies"]]
    print(f"tight top: {len(names)} bodies, last {names[-1]!r}")
    for view, name in (("iso", "tight_iso"), ("top", "tight_top")):
        mcp.call("screenshot", {"tab": tight, "view": view, "width": 1200, "height": 900, "path": os.path.join(out, f"{name}.png")})
    print(mcp.call("export", {"tab": tight, "format": "dxf", "view": "top", "path": os.path.join(out, "tight_templates.dxf")}))
    print(mcp.call("export", {"tab": tight, "format": "stl", "body": "Printing fixture", "path": os.path.join(out, "fixture.stl")}))
    mcp.close()
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
