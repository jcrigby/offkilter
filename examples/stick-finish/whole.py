#!/usr/bin/env python3
"""The whole job on one stick, as one animation with captions.

    python3 examples/stick-sled/build.py           # the table, in its out/sled.json
    python3 examples/stick-finish/build.py         # the forms, in out/finish.json
    python3 examples/stick-finish/whole.py

Writes out/whole.gif, seen along the stick as the machine has it, the
bit level and the stick turning:

1. the coarse passes: the table's roughing rows, the straight bit;
2. a pause, with what is done and what is next written on it;
3. the detail passes: its finishing rows, the tapered ball tip;
4. a pause, saying that sanding is next;
5. the sanding, by hand: the bump form on each of the seven bumps and
   then the flute form on each of the seven flutes, a pause after each.
   The tile's outline is amber while it is still to do, and each
   stretch a form has been along turns the form's colour, teal for the
   bump form and magenta for the flute form, so the last frame is the
   tile with what the two forms cover marked on it and the fourteen
   corners they leave still amber.

Also four stills (out/whole_coarse.png, whole_detail.png,
whole_sanding.png, whole_done.png) and out/whole.json, which says
what is in the file for the test.

The passes are `okmcp.cutting_book`'s, as in the sled's own
animations, and the captions are `okmcp.lettering`'s: a flip book
draws shapes and no text, so the letters are strokes.
"""
import importlib.util
import json
import math
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp, cutting_book, lettering, lettering_wide, tint  # noqa: E402

OUT = os.path.join(HERE, "out")
SLED = os.path.join(os.path.dirname(HERE), "stick-sled")
_spec = importlib.util.spec_from_file_location("sled_animate", os.path.join(SLED, "animate.py"))
A = importlib.util.module_from_spec(_spec)          # the sled's own animation: its bits' outlines, its colours, its pace
_spec.loader.exec_module(A)

SIZE = 640                                      # pixels each way
BAND = 11.0                                     # a band for a caption above the stick and one below, mm: two lines and air round them
LETTER = 3.3                                    # a capital's height, mm
STROKE = 2.6                                    # and its strokes' width, pixels
INK = "#111111"
PAUSE = 4500                                    # a pause with a caption, ms: long enough to read two lines twice
USE = 1100                                      # a form on an edge
REST = 500                                      # and the pause after it
END = 6000                                      # the last frame
TODO = "#f59e0b"                                # the tile's outline while it is still to be sanded: amber
FORM = {"bump": "#0d9488", "flute": "#c026d3"}  # the bump form and where it has been: teal. The flute form: magenta. Not green and red, which many eyes cannot tell apart
EDGE = 5.0                                      # those outlines' width, pixels


def moved(m, q):
    """A point of an edge's own frame put on the tile, as the build's `moved`."""
    x = -q[0] if m[0] else q[0]
    return (m[1] * x - m[2] * q[1] + m[3], m[2] * x + m[1] * q[1] + m[4])


def r3(line):
    return [[round(x, 3), round(y, 3)] for x, y in line]


def main():
    n = json.load(open(os.path.join(SLED, "out", "sled.json")))
    fin = json.load(open(os.path.join(OUT, "finish.json")))
    rows = [r for r in n["rows"] if r["kind"] in ("rough", "finish")]
    last_rough = max(i for i, r in enumerate(rows) if r["kind"] == "rough")
    coarse, detail = last_rough + 1, len(rows) - last_rough - 1
    outline = [tuple(q) for q in fin["tile"]["outline"]]
    # Every use of a form: which, on which of its seven, its section there and the stretch of the tile it covers.
    uses = []
    for name in ("bump", "flute"):
        e = fin["edges"][name]
        for k, place in enumerate(e["places"]):
            uses.append({"form": name, "edge": k + 1, "section": [moved(place["move"], q) for q in e["section"]], "stretch": [moved(place["move"], q) for q in e["work"]]})
    # The picture: the blank turning, the forms where they reach furthest, and a band above and below for the captions.
    reach = max(math.hypot(*q) for u in uses for q in u["section"])
    half = math.ceil(max(n["blank_swing"] + A.MARGIN, reach + 1.0) + BAND)
    edge = half - BAND                                  # where the bands begin

    def caption(top=(), bottom=(), backed=False):
        """Lines of lettering in the top band and in the bottom one, level whichever way the stick is turned. On a
        white ground where a form may be behind them."""
        out = []
        for lines, up in ((top, True), (bottom, False)):
            if not lines:
                continue
            if backed:
                wide = max(lettering_wide(t, LETTER) for t in lines) / 2 + 2.0
                y = (edge, half) if up else (-half, -edge)
                out.append({"points": [[-wide, y[0]], [wide, y[0]], [wide, y[1]], [-wide, y[1]]], "fill": A.WHITE, "still": True})
            step = LETTER * 1.45
            first = (edge + (BAND + (len(lines) - 1) * step - LETTER) / 2) if up else (-edge - (BAND - (len(lines) - 1) * step + LETTER) / 2)
            for i, text in enumerate(lines):
                out += lettering(text, (0.0, first - i * step), LETTER, INK, STROKE)
        return out

    # -- the machine's passes, as the sled's own animation has them ---------------------------------
    bits = {tool: A.profile(tool, 3.0 * half) for tool in ("straight", "ball")}
    cuts = [A.placed(bits[r["tool"]], r["t"], r["across"], r["tip"]) for r in rows]
    turn = lambda r: math.degrees(-r["t"]) % 360.0
    load = next(r for r in n["rows"] if r["kind"] == "load")
    frames, where = cutting_book([tuple(c) for c in n["blank"]["corners"]], cuts, tail=A.MACHINE_TAIL, shades=A.MACHINE_SHADES, ms=A.MS, hold=(1500, PAUSE),
                                 blank_colour=A.BLANK, cut_colour=A.CUT, background=A.WHITE, turns=[turn(r) for r in rows], first_turn=turn(load), turn_step=A.TURN_STEP)
    # A pause after the last coarse pass: the frame that follows it, with no cutter in it, held, and written on.
    at = where[last_rough] + 1
    after = frames[at]
    pause_1 = {"keep": after.get("keep", []), "draw": [s for s in after.get("draw", []) if s.get("fill") != A.CUT], "ms": PAUSE, "turn": frames[at - 1]["turn"]}
    frames.insert(at, pause_1)
    where = [w if w < at else w + 1 for w in where]
    for k, f in enumerate(frames):
        f.setdefault("draw", [])
        if k < at:
            f["draw"] = f["draw"] + caption(top=("1 OF 3: COARSE PASSES",))
        elif k > at and k < len(frames) - 1:
            f["draw"] = f["draw"] + caption(top=("2 OF 3: DETAIL PASSES",))
    pause_1["draw"] = pause_1["draw"] + caption(top=(f"COARSE PASSES DONE: {coarse}",), bottom=("NEXT: CHANGE THE BIT", f"THEN {detail} DETAIL PASSES"))
    # The last frame of the passes is the tile, held: the second pause.
    pause_2 = len(frames) - 1
    frames[pause_2]["draw"] = caption(top=(f"DETAIL PASSES DONE: {detail}",), bottom=("NEXT: SANDING BY HAND", f"TWO FORMS, {len(uses)} EDGES"))
    lies = frames[pause_2]["turn"]

    # -- the sanding ---------------------------------------------------------------------------------
    # The whole outline is to do. Then each form on each of its edges, and the stretch it has been along.
    frames.append({"keep": [{"points": r3(outline), "stroke": TODO, "width": EDGE, "closed": True}], "draw": caption(top=("3 OF 3: SANDING",), bottom=("ALONG THE STICK, BY HAND",), backed=True),
                   "ms": 1800, "turn": lies})
    shown = []
    for u in uses:
        name, what = u["form"], f"{u['form'].upper()} FORM: {u['form'].upper()} {u['edge']} OF 7"
        colour = FORM[name]
        form = {"points": r3(u["section"]), "fill": tint(colour, A.WHITE, 0.55), "stroke": colour, "width": 2.0, "closed": True}
        done = {"points": r3(u["stretch"]), "stroke": colour, "width": EDGE, "closed": False}
        u["show"] = len(frames)
        frames.append({"draw": [form] + caption(top=("3 OF 3: SANDING",), bottom=(what,), backed=True), "ms": USE, "turn": lies})
        u["rest"] = len(frames)
        frames.append({"keep": [done], "draw": caption(top=("3 OF 3: SANDING",), bottom=(what + " DONE",), backed=True), "ms": REST, "turn": lies})
        shown.append(u)
    round_it = sum(math.dist(outline[i], outline[(i + 1) % len(outline)]) for i in range(len(outline)))
    covered = sum(fin["edges"][name]["covered"] for name in ("bump", "flute")) * 7
    frames.append({"draw": caption(top=("SANDED: 7 BUMPS, 7 FLUTES",), bottom=(f"LEFT: {len(uses)} CORNERS, {round_it - covered:.0f} MM", "FOR A FOLDED EDGE OF FILM"), backed=True), "ms": END, "turn": lies})

    stills = {"coarse": at, "detail": pause_2, "sanding": uses[len(uses) // 2 + 2]["show"], "done": len(frames) - 1}
    view = [-half, -half, half, half]
    with tempfile.TemporaryDirectory() as scratch:
        mcp = Mcp(os.path.join(scratch, "none.okpart"))        # the tool needs no document; the server wants one to open
        print(mcp.call("flipbook", {"path": os.path.join(OUT, "whole.gif"), "width": SIZE, "view": view, "background": A.WHITE, "frames": frames,
                                    "stills": [{"frame": f, "path": os.path.join(OUT, f"whole_{name}.png")} for name, f in stills.items()]}).replace(OUT + os.sep, "out/"))
        mcp.close()
    seconds = sum(f.get("ms", A.MS) for f in frames) / 1000.0
    said = {"size": SIZE, "view": view, "band": BAND, "letter": LETTER, "ink": INK, "blank": A.BLANK, "cut": A.CUT, "background": A.WHITE, "todo": TODO, "form": FORM,
            "form_fill": {name: tint(c, A.WHITE, 0.55) for name, c in FORM.items()}, "ms": A.MS, "pause": PAUSE, "use": USE, "rest": REST, "end": END,
            "frames": len(frames), "seconds": seconds, "coarse": coarse, "detail": detail, "cut_frames": where, "pauses": [at, pause_2], "sanding_from": pause_2 + 1,
            "uses": [{"form": u["form"], "edge": u["edge"], "show": u["show"], "rest": u["rest"]} for u in shown], "stills": stills,
            "round": round_it, "covered": covered, "left": round_it - covered}
    with open(os.path.join(OUT, "whole.json"), "w") as f:
        json.dump(said, f)
    print(f"   {coarse} coarse passes, a pause, {detail} detail passes, a pause, {len(uses)} uses of the two forms; {len(frames)} frames, {seconds:.0f} s a showing; "
          f"the forms cover {covered:.0f} of the {round_it:.0f} mm round the tile")


if __name__ == "__main__":
    main()
