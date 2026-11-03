"""Supports for turntable scans: three 20 mm cubes to stand the object
on, and 40 mm posts for the same object turned over with its stem
down. Set each on a bullseye mark of the measuring field and it is a
block of known height at a known place: the lift takes the object's
base out of the table's glow and the line's grazing angle there,
`min_height` 22 (or 42) drops the supports and the table out of the
cloud, and the laser's line on a block's top face is a check on the
laser's plane, since that face is at 20 (or 40) mm wherever the block
stands. Print three of each, white or light grey, matte.

    cargo build --release -p ok-mcp
    python3 examples/scan-supports/build.py       # the document, the STLs, pictures, numbers
    cargo test -p ok-render --test scan_supports  # what CI runs

Frame: each block centred on the origin in plan, its base at z = 0,
so the block's centre in plan is the mark's.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, Part, new_tab  # noqa: E402

OUT = os.path.join(HERE, "out")

SIDE = 20.0   # the block in plan, and the cube's height: a hand's worth of lift, and under a pumpkin's base three fit inside its outline
POST = 40.0   # the post's height: a pumpkin turned over rests on its shoulders with the stem down, and stems run to 35 mm


def block(p, height, name):
    s = p.sketch("top", 0.0, "footprint")
    p.rect(s, (-SIDE / 2, -SIDE / 2), (SIDE / 2, SIDE / 2))
    f = p.extrude(s, height, op="new", name=name)
    p.apply({"type": "rename_part", "source": f, "name": name},
            {"type": "set_part_material", "source": f, "material": {"name": "PLA", "density": 1.24}})


def main():
    os.makedirs(OUT, exist_ok=True)
    doc = os.path.join(OUT, "scan_supports.okpart")
    if os.path.exists(doc):
        os.remove(doc)
    mcp = Mcp(doc)
    mcp.call("create_document", {"name": "scan_supports"})
    numbers = {"side": SIDE, "post": POST}
    tabs = {"cube": 1, "post": new_tab(mcp, {"type": "add_part_studio", "name": "post"})}
    mcp.call("apply", {"ops": [{"type": "rename_tab", "tab": 1, "name": "cube"}]})
    for name, height in (("cube", SIDE), ("post", POST)):
        p = Part(mcp, tabs[name], name)
        block(p, height, f"{name}, {SIDE:g} square, {height:g} tall")
        for b in p.report()["bodies"]:
            print(name, b["name"], b.get("volume"))
            numbers[f"{name}_volume"] = b.get("volume")
        p.export(os.path.join(OUT, f"{name}{height:g}.stl"))
        p.screenshot(os.path.join(OUT, f"{name}.png"), view="iso")
    with open(os.path.join(OUT, "scan_supports.json"), "w") as f:
        json.dump(numbers, f, indent=1)
    mcp.close()


if __name__ == "__main__":
    main()
