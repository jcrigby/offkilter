#!/usr/bin/env python3
"""Every pass of the table, a frame each, seen along the stick.

    python3 examples/stick-sled/build.py        # writes out/sled.json, which this reads
    python3 examples/stick-sled/animate.py

Writes two animations of the same passes: the blank's section in one
colour on white, the cut being made solid in another, the cuts before
it as outlines fading to white.

out/passes_machine.gif is what the machine does, seen from the
headstock's end with the positioner on the right: the bit stays level
and moves in and out and up and down, and the stick turns to meet it.
out/passes.gif is fixed to the stick instead, so the tile stays still
and the bit comes at it from every side in turn.

Also three stills of each (out/passes_rough.png and the like) and
out/passes_anim.json, which says what is in the files for the test.

The drawing is the `flipbook` tool's and the frames are
`okmcp.cutting_book`'s: both take any blank and any list of cuts, so
another machine's passes are the fifteen lines of `main` below.
"""
import json
import math
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)
from okmcp import Mcp, cutting_book  # noqa: E402
import plan as P  # noqa: E402

OUT = os.path.join(HERE, "out")
SIZE = 640                                      # pixels each way
MARGIN = 6.0                                    # the picture's edge from the blank's furthest corner, mm
MS = 70                                         # a pass
TAIL = 48                                       # how many passes back an outline can still be seen
SHADES = 8                                      # and in how many steps it fades: six passes to a shade. Twelve make the file a quarter larger
TOOL_CHANGE = 900                               # the last roughing pass is held this long: the bit is changed and set
TURN_STEP = 6.0                                 # on the machine's view, a turn of more than this many degrees is shown on the way, this much a frame
MACHINE_TAIL, MACHINE_SHADES = 24, 6            # and there the outlines turn with the stick, so all of them are drawn again at every new angle: half as many keep the file near 4 MB
BLANK, CUT, WHITE = "#e2be8a", "#1f4fd8", "#ffffff"


def profile(tool, long):
    """A bit's outline about its own axis: (u toward the router from its tip, v across), out to `long`."""
    if tool != "ball":
        r = P.ROUGH_BIT / 2
        return [(long, -r), (0.0, -r), (0.0, r), (long, r)]
    r, half, a = P.BIT / 2, P.SHANK / 2, math.radians(P.TAPER)
    side = []
    for k in range(24):
        off = half * k / 24
        side.append((r + P.flank(off, r), off))                # the ball, then the cone
    side += [(r + (half * math.cos(a) - r) / math.sin(a), half), (long, half)]     # where the cone meets the shank, and the shank
    return [(u, -v) for u, v in reversed(side)] + side[1:]


def placed(shape, t, across, tip):
    """A bit's outline where a row has it, in the stick's own frame."""
    d, e = (math.cos(t), math.sin(t)), (-math.sin(t), math.cos(t))
    return [((tip + u) * d[0] + (across + v) * e[0], (tip + u) * d[1] + (across + v) * e[1]) for u, v in shape]


def main():
    n = json.load(open(os.path.join(OUT, "sled.json")))
    rows = [r for r in n["rows"] if r["kind"] in ("rough", "finish")]
    half = math.ceil(n["blank_swing"] + MARGIN)
    bits = {tool: profile(tool, 3.0 * half) for tool in ("straight", "ball")}
    cuts = [placed(bits[r["tool"]], r["t"], r["across"], r["tip"]) for r in rows]
    last_rough = max(i for i, r in enumerate(rows) if r["kind"] == "rough")
    cuts[last_rough] = (cuts[last_rough], TOOL_CHANGE)
    blank = [tuple(c) for c in n["blank"]["corners"]]
    style = dict(tail=TAIL, shades=SHADES, ms=MS, blank_colour=BLANK, cut_colour=CUT, background=WHITE)
    # Twice. Fixed to the stick: the tile stays still and the bit comes at it from every side. And as the machine has
    # it, seen from the headstock's end with the positioner on the right: the bit stays level and moves in and out and
    # up and down, and the stick turns to meet it. A row's turn is the angle its cut comes from, the other way; the
    # first frame is the blank as the table's first row hangs it, a wide face level and underneath.
    turn = lambda r: math.degrees(-r["t"]) % 360.0
    load = next(r for r in n["rows"] if r["kind"] == "load")
    books = {"passes": cutting_book(blank, cuts, **style),
             "passes_machine": cutting_book(blank, cuts, turns=[turn(r) for r in rows], first_turn=turn(load), turn_step=TURN_STEP, **{**style, "tail": MACHINE_TAIL, "shades": MACHINE_SHADES})}
    said = {"size": SIZE, "view": [-half, -half, half, half], "blank": BLANK, "cut": CUT, "background": WHITE, "ms": MS, "tail": TAIL, "shades": SHADES, "tool_change": TOOL_CHANGE,
            "turn_step": TURN_STEP, "machine_tail": MACHINE_TAIL, "cuts": len(cuts), "last_rough": last_rough}
    with tempfile.TemporaryDirectory() as scratch:
        mcp = Mcp(os.path.join(scratch, "none.okpart"))        # the tool needs no document; the server wants one to open
        for name, (frames, where) in books.items():
            stills = {"rough": where[last_rough * 3 // 5], "finish": where[last_rough + (len(rows) - last_rough) // 2], "done": len(frames) - 1}
            print(mcp.call("flipbook", {"path": os.path.join(OUT, f"{name}.gif"), "width": SIZE, "view": [-half, -half, half, half], "background": WHITE, "frames": frames,
                                        "stills": [{"frame": f, "path": os.path.join(OUT, f"{name}_{still}.png")} for still, f in stills.items()]})
                  .replace(OUT + os.sep, "out/"))
            seconds = sum(f.get("ms", MS) for f in frames) / 1000.0
            said[name] = {"frames": len(frames), "cut_frames": where, "stills": stills, "seconds": seconds, "turns": [f.get("turn", 0.0) for f in frames]}
            print(f"   {len(cuts)} passes in {len(frames)} frames: {last_rough + 1} roughing, {len(cuts) - last_rough - 1} finishing; {seconds:.0f} s a showing")
        mcp.close()
    with open(os.path.join(OUT, "passes_anim.json"), "w") as f:
        json.dump(said, f)


if __name__ == "__main__":
    main()
