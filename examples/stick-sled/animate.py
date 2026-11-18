#!/usr/bin/env python3
"""Every pass of the table, a frame each, seen along the stick.

    python3 examples/stick-sled/build.py        # writes out/sled.json, which this reads
    python3 examples/stick-sled/animate.py

Writes out/passes.gif: the blank's section in one colour on white, the
cut being made solid in another, the cuts before it as outlines fading
to white. The picture is fixed to the stick, so the bit comes at it
from every side in turn; on the machine it is the stick that turns and
the bit always comes from the positioner's side. Also three stills
(out/passes_rough.png, passes_finish.png, passes_done.png) and
out/passes_anim.json, which says what is in the file for the test.

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
    frames, first = cutting_book([tuple(c) for c in n["blank"]["corners"]], cuts, tail=TAIL, shades=SHADES, ms=MS, blank_colour=BLANK, cut_colour=CUT, background=WHITE)
    stills = {"rough": first + last_rough * 3 // 5, "finish": first + last_rough + (len(rows) - last_rough) // 2, "done": len(frames) - 1}
    with tempfile.TemporaryDirectory() as scratch:
        mcp = Mcp(os.path.join(scratch, "none.okpart"))        # the tool needs no document; the server wants one to open
        print(mcp.call("flipbook", {"path": os.path.join(OUT, "passes.gif"), "width": SIZE, "view": [-half, -half, half, half], "background": WHITE, "frames": frames,
                                    "stills": [{"frame": f, "path": os.path.join(OUT, f"passes_{name}.png")} for name, f in stills.items()]})
              .replace(OUT + os.sep, "out/"))
        mcp.close()
    with open(os.path.join(OUT, "passes_anim.json"), "w") as f:
        json.dump({"size": SIZE, "view": [-half, -half, half, half], "blank": BLANK, "cut": CUT, "background": WHITE, "ms": MS, "tail": TAIL, "shades": SHADES, "tool_change": TOOL_CHANGE,
                   "frames": len(frames), "first_cut": first, "cuts": len(cuts), "last_rough": last_rough, "stills": stills,
                   "seconds": sum(f.get("ms", MS) for f in frames) / 1000.0}, f, indent=1)
    print(f"{len(cuts)} passes in {len(frames)} frames: {last_rough + 1} roughing, {len(cuts) - last_rough - 1} finishing; {sum(f.get('ms', MS) for f in frames) / 1000.0:.0f} s a showing")


if __name__ == "__main__":
    main()
