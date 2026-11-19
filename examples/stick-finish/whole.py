#!/usr/bin/env python3
"""The whole job on one stick, as one animation with captions.

    python3 examples/stick-sled/build.py           # the table, in its out/sled.json
    python3 examples/stick-finish/build.py         # the forms, in out/finish.json
    python3 examples/stick-finish/whole.py

Writes out/whole.gif, seen along the stick as the machine has it, the
bit level and the stick turning:

1. the coarse passes: the table's roughing rows, the straight bit;
2. a pause, with what is done and what is next written on it;
3. the detail passes, the tapered ball tip, and fewer of them than the
   sled's table has: where a sanding form will go along afterwards a
   pass may leave a ridge of COARSE, and only at the corners the forms
   do not reach is it the table's own 0.05 mm. So the tile comes off
   the machine with ridges on it that can be seen;
4. a pause, saying that sanding is next;
5. the sanding, by hand: the bump form on each of the seven bumps and
   then the flute form on each of the seven flutes, a pause after each.
   A line just inside the tile's edge is amber while that edge is
   still to do; where a form has been along, the ridges are gone and
   the line is the form's colour, teal for the bump form and magenta
   for the flute form. The last frame is the tile with what the two
   forms cover marked on it, and the fourteen corners they leave,
   which the machine cut fine, still amber.

This is not the sled's table. The table still finishes the whole
outline to 0.05 mm, and a stick milled by it needs no sanding to
speak of; this is the plan the owner means to try afterwards, drawn
to see what it asks of the forms. out/whole.json has how many passes
it saves.

Also four stills (out/whole_coarse.png, whole_detail.png,
whole_sanding.png, whole_done.png).

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

P = A.P                                         # and its planner

COARSE = 0.4                                    # what a detail pass may leave where a form goes along afterwards, mm. A guess at what the film takes off without labour: to be tried
ON = 0.03                                       # a point of the outline is on a form's stretch if it is this near it
SIZE = 800                                      # pixels each way: a ridge of COARSE is four of them
BANDS = (7.0, 11.0)                             # a caption's band at the top, one line, and at the bottom, two, mm. They lie over the picture's edges
LETTER = 3.0                                    # a capital's height, mm
STROKE = 3.0                                    # and its strokes' width, pixels
INK = "#111111"
PAUSE = 4500                                    # a pause with a caption, ms: long enough to read two lines twice
USE = 1100                                      # a form on an edge
REST = 500                                      # and the pause after it
END = 6000                                      # the last frame
TODO = "#f59e0b"                                # the line inside an edge still to be sanded: amber
FORM = {"bump": "#0d9488", "flute": "#c026d3"}  # the bump form and where it has been: teal. The flute form: magenta. Not green and red, which many eyes cannot tell apart
EDGE = 3.0                                      # those lines' width, pixels
INSIDE = 0.45                                   # and how far inside the tile's edge they are drawn, so that the edge itself is seen: ridged, then clean


def moved(m, q):
    """A point of an edge's own frame put on the tile, as the build's `moved`."""
    x = -q[0] if m[0] else q[0]
    return (m[1] * x - m[2] * q[1] + m[3], m[2] * x + m[1] * q[1] + m[4])


def r3(line):
    return [[round(x, 3), round(y, 3)] for x, y in line]


def near(q, line):
    """How far a point is from an open line."""
    best = 9e9
    for a, b in zip(line[:-1], line[1:]):
        d = (b[0] - a[0], b[1] - a[1])
        l2 = d[0] * d[0] + d[1] * d[1]
        t = max(0.0, min(1.0, ((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / l2)) if l2 > 1e-18 else 0.0
        best = min(best, math.hypot(q[0] - a[0] - d[0] * t, q[1] - a[1] - d[1] * t))
    return best


def pushed(line, by, closed=False):
    """A line moved `by` to its left as it is walked."""
    out, n = [], len(line)
    for k, q in enumerate(line):
        a, b = (line[(k - 1) % n], line[(k + 1) % n]) if closed else (line[max(k - 1, 0)], line[min(k + 1, n - 1)])
        t = (b[0] - a[0], b[1] - a[1])
        l = math.hypot(*t)
        out.append((q[0] - by * t[1] / l, q[1] + by * t[0] / l))
    return out


def main():
    n = json.load(open(os.path.join(SLED, "out", "sled.json")))
    fin = json.load(open(os.path.join(OUT, "finish.json")))
    outline = [tuple(q) for q in n["tile"]["outline"]]
    # Every use of a form: which, on which of its seven, its section there and the stretch of the tile it covers,
    # walked with the wood on its right.
    uses = []
    for name in ("bump", "flute"):
        e = fin["edges"][name]
        for k, place in enumerate(e["places"]):
            uses.append({"form": name, "edge": k + 1, "section": [moved(place["move"], q) for q in e["section"]], "stretch": [moved(place["move"], q) for q in e["work"]]})

    # -- the plan: the table's roughing, and detail passes that are coarse where a form will go ---------------
    rough = [r for r in n["rows"] if r["kind"] == "rough"]
    on = [any(near(q, u["stretch"]) <= ON for u in uses) for q in outline]
    passes, left, orphans = P.finish(outline, [COARSE if o else P.RIDGE for o in on])
    if orphans:
        raise SystemExit(f"{orphans} points of the outline are reached by no pass")
    passes.sort(key=lambda q: math.degrees(-q["t"]) % 360.0)
    places = []                                         # the table's roughing places again, each at its last bite
    for r in rough:
        if places and (places[-1]["t"], places[-1]["across"]) == (r["t"], r["across"]):
            places[-1]["tip"] = min(places[-1]["tip"], r["tip"])
        else:
            places.append({"t": r["t"], "across": r["across"], "tip": r["tip"]})
    kept, waste = P.bites(outline, places, passes)
    if sum(q["bites"] for q in kept) != len(rough):
        raise SystemExit("the roughing, counted again, is not the table's")
    detail = [{"tool": "ball", "t": q["t"], "across": q["across"], "tip": q["tip"] + q["depth"] * (q["bites"] - b) / q["bites"]} for q in passes for b in range(1, q["bites"] + 1)]
    rows = [{"tool": "straight", "t": r["t"], "across": r["across"], "tip": r["tip"]} for r in rough] + detail
    last_rough, coarse = len(rough) - 1, len(rough)
    full = sum(1 for r in n["rows"] if r["kind"] == "finish")
    ridges = (max(v for v, o in zip(left, on) if o), max(v for v, o in zip(left, on) if not o))

    # The picture: the blank turning and the forms where they reach furthest. The captions lie over its edges.
    reach = max(math.hypot(*q) for u in uses for q in u["section"])
    half = math.ceil(max(n["blank_swing"] + A.MARGIN, reach + 1.0))

    def caption(top=(), bottom=(), backed=False):
        """Lines of lettering in the top band and in the bottom one, level whichever way the stick is turned. On a
        white ground where a form may be behind them."""
        out = []
        for lines, up, band in ((top, True, BANDS[0]), (bottom, False, BANDS[1])):
            if not lines:
                continue
            edge = half - band
            if backed:
                wide = max(lettering_wide(t, LETTER) for t in lines) / 2 + 2.0
                y = (edge, half) if up else (-half, -edge)
                out.append({"points": [[-wide, y[0]], [wide, y[0]], [wide, y[1]], [-wide, y[1]]], "fill": A.WHITE, "still": True})
            step = LETTER * 1.5
            first = (edge + (band + (len(lines) - 1) * step - LETTER) / 2) if up else (-edge - (band - (len(lines) - 1) * step + LETTER) / 2)
            for i, text in enumerate(lines):
                out += lettering(text, (0.0, first - i * step), LETTER, INK, STROKE)
        return out

    # -- the machine's passes, drawn as the sled's own animation draws its table's ------------------------
    bits = {tool: A.profile(tool, 3.0 * half) for tool in ("straight", "ball")}
    cuts = [A.placed(bits[r["tool"]], r["t"], r["across"], r["tip"]) for r in rows]
    turn = lambda r: math.degrees(-r["t"]) % 360.0
    load = next(r for r in n["rows"] if r["kind"] == "load")
    blank = [tuple(c) for c in n["blank"]["corners"]]
    frames, where = cutting_book(blank, cuts, tail=A.MACHINE_TAIL, shades=A.MACHINE_SHADES, ms=A.MS, hold=(1500, PAUSE),
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
    pause_1["draw"] = pause_1["draw"] + caption(top=(f"COARSE PASSES DONE: {coarse}",), bottom=("NEXT: CHANGE THE BIT", f"THEN {len(detail)} DETAIL PASSES"))
    # The last frame of the passes is the tile, held, its ridges on it: the second pause.
    pause_2 = len(frames) - 1
    frames[pause_2]["draw"] = caption(top=(f"DETAIL PASSES DONE: {len(detail)}",), bottom=("NEXT: SAND THE RIDGES OFF", f"TWO FORMS, {len(uses)} EDGES"))
    lies = frames[pause_2]["turn"]

    # -- the sanding ---------------------------------------------------------------------------------
    # A line inside the whole edge: all of it is to do. Then each form on each of its edges; after it the ridges
    # along its stretch are gone, rubbed out to COARSE and a little past the tile's edge, and the line there is
    # the form's colour.
    inner = pushed(outline, INSIDE if sum(outline[i][0] * outline[(i + 1) % len(outline)][1] - outline[(i + 1) % len(outline)][0] * outline[i][1] for i in range(len(outline))) > 0 else -INSIDE, closed=True)
    frames.append({"keep": [{"points": r3(inner), "stroke": TODO, "width": EDGE, "closed": True}], "draw": caption(top=("3 OF 3: SANDING",), bottom=("ALONG THE STICK, BY HAND",), backed=True),
                   "ms": 1800, "turn": lies})
    for u in uses:
        name, what = u["form"], f"{u['form'].upper()} FORM: {u['form'].upper()} {u['edge']} OF 7"
        colour = FORM[name]
        form = {"points": r3(u["section"]), "fill": tint(colour, A.WHITE, 0.55), "stroke": colour, "width": 2.0, "closed": True}
        rubbed = {"points": r3(u["stretch"] + pushed(u["stretch"], COARSE + 0.4)[::-1]), "fill": A.WHITE}
        done = {"points": r3(pushed(u["stretch"], -INSIDE)), "stroke": colour, "width": EDGE, "closed": False}
        u["show"] = len(frames)
        frames.append({"draw": [form] + caption(top=("3 OF 3: SANDING",), bottom=(what,), backed=True), "ms": USE, "turn": lies})
        u["rest"] = len(frames)
        frames.append({"keep": [rubbed, done], "draw": caption(top=("3 OF 3: SANDING",), bottom=(what + " DONE",), backed=True), "ms": REST, "turn": lies})
    round_it = sum(math.dist(outline[i], outline[(i + 1) % len(outline)]) for i in range(len(outline)))
    covered = sum(fin["edges"][name]["covered"] for name in ("bump", "flute")) * 7
    frames.append({"draw": caption(top=("SANDED: 7 BUMPS, 7 FLUTES",), bottom=(f"NOT SANDED: {len(uses)} CORNERS", "THE MACHINE CUT THEM FINE"), backed=True), "ms": END, "turn": lies})

    stills = {"coarse": at, "detail": pause_2, "sanding": uses[len(uses) // 2 + 2]["show"], "done": len(frames) - 1}
    view = [-half, -half, half, half]
    with tempfile.TemporaryDirectory() as scratch:
        mcp = Mcp(os.path.join(scratch, "none.okpart"))        # the tool needs no document; the server wants one to open
        print(mcp.call("flipbook", {"path": os.path.join(OUT, "whole.gif"), "width": SIZE, "view": view, "background": A.WHITE, "frames": frames,
                                    "stills": [{"frame": f, "path": os.path.join(OUT, f"whole_{name}.png")} for name, f in stills.items()]}).replace(OUT + os.sep, "out/"))
        mcp.close()
    seconds = sum(f.get("ms", A.MS) for f in frames) / 1000.0
    said = {"size": SIZE, "view": view, "bands": BANDS, "letter": LETTER, "ink": INK, "blank": A.BLANK, "cut": A.CUT, "background": A.WHITE, "todo": TODO, "form": FORM,
            "form_fill": {name: tint(c, A.WHITE, 0.55) for name, c in FORM.items()}, "ms": A.MS, "pause": PAUSE, "use": USE, "rest": REST, "end": END,
            "frames": len(frames), "seconds": seconds, "coarse": coarse, "detail": len(detail), "cut_frames": where, "pauses": [at, pause_2], "sanding_from": pause_2 + 1,
            "uses": [{"form": u["form"], "edge": u["edge"], "show": u["show"], "rest": u["rest"]} for u in uses], "stills": stills,
            "round": round_it, "covered": covered, "left": round_it - covered,
            "plan": {"coarse_ridge": COARSE, "fine_ridge": P.RIDGE, "on": sum(on), "points": len(on), "passes": len(passes), "table_passes": n["plan"]["passes"], "rows": len(detail), "table_rows": full,
                     "ridge_where_sanded": ridges[0], "ridge_at_corners": ridges[1], "left_at_end": waste["left_at_end"], "tile_area": n["tile"]["area"],
                     "detail": [[r["t"], r["across"], r["tip"]] for r in detail]}}
    with open(os.path.join(OUT, "whole.json"), "w") as f:
        json.dump(said, f)
    print(f"   {coarse} coarse passes, a pause, {len(detail)} detail passes ({len(passes)} places, where the table has {n['plan']['passes']} in {full} rows), a pause, {len(uses)} uses of the two forms; "
          f"{len(frames)} frames, {seconds:.0f} s a showing")
    print(f"   a ridge of up to {ridges[0]:.2f} mm is left where a form goes along, {ridges[1]:.3f} at the corners; the forms cover {covered:.0f} of the {round_it:.0f} mm round the tile")


if __name__ == "__main__":
    main()
