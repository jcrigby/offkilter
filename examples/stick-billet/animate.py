#!/usr/bin/env python3
"""The billet, every step, as one captioned animation.

    python3 examples/stick-sled/build.py        # the sled's tile and table
    python3 examples/stick-billet/build.py      # out/billet.json, which this reads
    python3 examples/stick-billet/animate.py

Writes out/billet.gif and seven stills (out/billet_1.png to
billet_7.png), one for each step:

1. the pieces sawn, 55 x 48 and 3 mm, a wood each if the owner likes;
2. drilled in the jig: four holes outside the tile, and the tail
   centre's mark on the cap;
3. stacked on the four rods, seen from the side: the cap, the pieces
   one by one, the spacer and the far plate, the rods going in and the
   nuts going on;
4. the first stage on the sled, seen along the stick as the machine
   has it, the stick turning under a level bit: the rods are drawn,
   and a tab of wood stays from each to the tile;
5. off the sled: the comb's three fingers go into the grooves, three
   rods go through them, the four old rods come out;
6. the second stage: the tabs and corners, the fingers holding;
7. the nuts off and the rods out, seen from the side again: the comb
   slides off and the stack comes apart into tiles.

The passes are `okmcp.cutting_book`'s, as in the sled's own animation,
the bits' outlines and colours its `animate.py`'s, and the captions are
`okmcp.lettering`'s strokes. out/billet_anim.json says what is where,
for the test.
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
MARGIN = 6.0                                    # the picture's edge beyond the blank's furthest corner, mm
BANDS = (7.0, 11.0)                             # a caption's band at the top, one line, and at the bottom, two
LETTER, STROKE, INK = 3.0, 3.0, "#111111"
PAUSE = 4200                                    # a captioned pause, ms
STEP_MS = 140                                   # a step of something done by hand: a hole, a rod, a finger
STACK_MS = 45                                   # a piece put on the stack
WOODS = ["#e9cf9f", "#7a5233", "#c0703f", "#d8b26a"]   # maple, walnut, cherry, oak: four woods, so no two pieces that touch are alike
CAP, PLATE_C = "#a98552", "#8aa1b8"             # the cap's wood, the far plate's print
BRASS, BRASS_EDGE = "#c9a227", "#8a6d12"        # the rods and nuts
COMB, COMB_EDGE = "#c026d3", "#86198f"          # the comb: magenta, which the bits' blue is not
JIG, JIG_EDGE = "#cbd5e1", "#64748b"            # the drilling jig
HOLE = "#ffffff"
TAIL, SHADES = 12, 4                           # a pass's outline fades in four steps over twelve passes: half the sled's, which keeps a file of twice its passes near its size


def r3(pts):
    return [[round(x, 3), round(y, 3)] for x, y in pts]


def circle(c, r, n=28):
    return [(c[0] + r * math.cos(2 * math.pi * k / n), c[1] + r * math.sin(2 * math.pi * k / n)) for k in range(n)]


def main():
    b = json.load(open(os.path.join(OUT, "billet.json")))
    n = json.load(open(os.path.join(SLED, "out", "sled.json")))
    blank = [tuple(q) for q in b["piece"]["corners"]]
    outline = [tuple(q) for q in b["outline"]]
    rods1 = [tuple(q) for q in b["first"]["rods"]]
    rods2 = [tuple(q) for q in b["second"]["rods"]]
    fingers = [[tuple(q) for q in f["loop"]] for f in b["second"]["fingers"]]
    rod_r, nut_r = b["rod"]["d"] / 2, b["rod"]["nut"]
    half = math.ceil(max(math.hypot(*q) for q in blank) + MARGIN)
    load = next(r for r in n["rows"] if r["kind"] == "load")
    turn = lambda t: math.degrees(-t) % 360.0
    lies = turn(load["t"])

    def caption(top=(), bottom=(), backed=True):
        """Lines of lettering in the bands at the top and the bottom, level whichever way the work is turned, on white."""
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

    def disc(c, r, fill, edge=None, still=False):
        s = {"points": r3(circle(c, r)), "fill": fill, "closed": True}
        if edge:
            s.update(stroke=edge, width=1.2)
        if still:
            s["still"] = True
        return s

    def poly(pts, fill, edge=None, width=1.2, still=False):
        s = {"points": r3(pts), "fill": fill, "closed": True}
        if edge:
            s.update(stroke=edge, width=width)
        if still:
            s["still"] = True
        return s

    frames = []

    # -- 1 and 2: a piece, and the jig ----------------------------------------
    # Seen along the stick, as the sled will hold it: the piece turned as the table's first row hangs the blank.
    cover = {"points": [[-half, -half], [half, -half], [half, half], [-half, half]], "fill": A.WHITE, "still": True}
    piece = poly(blank, WOODS[0], "#9a7b4f")
    one = ("1 OF 7: SAW THE PIECES",)
    frames.append({"draw": [piece] + caption(one, ("55 X 48 AND 3 MM THICK", "A WOOD EACH IF YOU LIKE")), "ms": PAUSE, "turn": lies})
    still_1 = len(frames) - 1
    jig_out = [(x * 1.0, y * 1.0) for x, y in _grown(blank, 0.3 + 5.0)]
    jig = [poly(jig_out, JIG, JIG_EDGE), poly(_grown(blank, 0.3), A.WHITE), piece]
    two = ("2 OF 7: DRILL THEM IN THE JIG",)
    frames.append({"draw": jig + caption(two, ("SIX AT A TIME, THE MARKED", "CORNER TO THE JIG'S")), "ms": 1800, "turn": lies})
    holes = []
    for k, c in enumerate(rods1):
        holes.append(disc(c, b["piece"]["hole"] / 2, HOLE, "#555555"))
        frames.append({"draw": jig + holes + [disc(c, 2.6, "#94a3b8", "#334155")] + caption(two, (f"HOLE {k + 1} OF 4, A #27 DRILL",)), "ms": STEP_MS * 3, "turn": lies})
    frames.append({"draw": jig + holes + [disc((0.0, 0.0), 0.9, "#334155")] + caption(two, ("AND ON THE CAP, AN AWL", "WHERE THE TAIL CENTRE GOES")), "ms": PAUSE // 2, "turn": lies})
    still_2 = len(frames) - 1

    # -- 3: stacked, seen from the side ------------------------------------------
    lay = b["layout"]
    x0, x1 = -14.0, lay["boss"][1] + 4.0
    k = (2 * half - 8.0) / (x1 - x0)                  # the side view's scale
    sx = lambda x: -half + 4.0 + (x - x0) * k
    a = math.atan2(blank[1][1] - blank[0][1], blank[1][0] - blank[0][0])
    up = (-math.sin(a), math.cos(a))                  # the piece's short side, which the side view shows upright
    height = lambda q: (q[0] * up[0] + q[1] * up[1]) * k
    lo_y, hi_y = min(height(q) for q in blank), max(height(q) for q in blank)
    side_y = 2.0                                      # the side view sits a little above the middle, under the top caption

    def rect(xa, xb, ya, yb, fill, edge=None, width=0.8):
        s = {"points": r3([(sx(xa), side_y + ya), (sx(xb), side_y + ya), (sx(xb), side_y + yb), (sx(xa), side_y + yb)]), "fill": fill, "closed": True, "still": True}
        if edge:
            s.update(stroke=edge, width=width)
        return s

    def far_plate():
        sb = [tuple(q) for q in b["piece"]["sled_blank"]]
        return [rect(*lay["far_plate"], lo_y, hi_y, PLATE_C, "#475569"),
                rect(*lay["boss"], min(height(q) for q in sb), max(height(q) for q in sb), PLATE_C, "#475569")]

    def stack(count, gap=0.0, tile=False):
        """The cap and `count` pieces, each `gap` apart; as tiles, their height the tile's."""
        lo, hi = (min(height(q) for q in outline), max(height(q) for q in outline)) if tile else (lo_y, hi_y)
        out = [rect(0.0, lay["near_cap"][1], lo, hi, CAP, "#6b4f2a")]
        for j in range(count):
            xa = lay["pieces"][0] + j * (b["piece"]["thick"] + gap)
            out.append(rect(xa, xa + b["piece"]["thick"], lo, hi, WOODS[j % len(WOODS)], "#3f2d1a", 0.5))
        return out

    def rods_side(xa, xb, which):
        return [rect(xa, xb, height(c) - 0.6, height(c) + 0.6, BRASS, BRASS_EDGE, 0.6) for c in which]

    three = ("3 OF 7: STACK THEM ON FOUR RODS",)
    count = b["piece"]["count"]
    frames.append({"draw": [cover] + stack(0) + caption(three, ("THE CAP FIRST: THE RODS", "THREAD INTO IT")), "ms": 1600, "turn": lies})
    for j in range(1, count + 1):
        frames.append({"draw": [cover] + stack(j) + caption(three, (f"{j} OF {count} PIECES",)), "ms": STACK_MS, "turn": lies})
    sp = lay["spacer"]
    whole = stack(count) + [rect(sp[0], sp[1], lo_y, hi_y, "#e5e7eb", "#6b7280", 0.5)] + far_plate()
    frames.append({"draw": [cover] + whole + caption(three, ("A SPACER, THE FAR PLATE",)), "ms": 1200, "turn": lies})
    rod_x = (lay["near_cap"][1] - 10.0, lay["boss"][0] + 3.4)
    for s in range(1, 9):
        frames.append({"draw": [cover] + whole + rods_side(rod_x[1] - (rod_x[1] - rod_x[0]) * s / 8, rod_x[1], rods1) + caption(three, ("FOUR #6-32 BRASS RODS",)), "ms": STEP_MS, "turn": lies})
    nuts = [rect(lay["boss"][0], lay["boss"][0] + 2.4, height(c) - 1.4, height(c) + 1.4, BRASS, BRASS_EDGE, 0.6) for c in rods1]
    frames.append({"draw": [cover] + whole + rods_side(*rod_x, rods1) + nuts + caption(three, ("NUTS IN THE PLATE'S NOTCHES", "AND THE STACK IS ONE BILLET")), "ms": PAUSE, "turn": lies})
    still_3 = len(frames) - 1

    # -- 4 to 6: the two stages on the sled, one book of passes -------------------
    rows = [(c[0], c[1], c[2], c[3]) for c in b["cuts"]["first"]] + [(c[0], c[1], c[2], c[3]) for c in b["cuts"]["second"]]
    first = len(b["cuts"]["first"])
    bits = {tool: A.profile(tool, 3.0 * half) for tool in ("straight", "ball")}
    cuts = [A.placed(bits["ball" if r[0] == "b" else "straight"], r[1], r[2], r[3]) for r in rows]
    book, where = cutting_book(blank, cuts, tail=TAIL, shades=SHADES, ms=A.MS, hold=(1500, 3000), blank_colour=WOODS[0], cut_colour=A.CUT,
                               background=A.WHITE, turns=[turn(r[1]) for r in rows], first_turn=lies, turn_step=A.TURN_STEP)
    held_rods = [disc(c, rod_r, BRASS, BRASS_EDGE) for c in rods1]
    old_holes = [disc(c, b["piece"]["hole"] / 2, HOLE) for c in rods1]          # no outline: where the wood round it is gone it is gone too
    comb = [poly(f, COMB, COMB_EDGE) for f in fingers]
    new_rods = [disc(c, rod_r, BRASS, BRASS_EDGE) for c in rods2]
    at = where[first - 1] + 1                          # the frame after the first stage's last pass
    four, six = ("4 OF 7: FIRST STAGE ON THE SLED",), ("6 OF 7: SECOND STAGE",)
    for j, f in enumerate(book):
        f.setdefault("draw", [])
        if j < at:
            f["draw"] = f["draw"] + held_rods + caption(four, ("CUT ALL BUT A TAB TO EACH ROD",) if j > 0 else ("THE PIECES' FACES ARE THE", "TILES' FACES: FACE GRAIN"))
        else:
            f["draw"] = f["draw"] + old_holes + comb + new_rods + caption(six, ("THE FINGERS HOLD IT NOW",) if j < len(book) - 1 else ("THE TILE, ON EVERY PIECE",))
    book[0]["draw"] = book[0]["draw"]
    turned = book[at - 1]["turn"]
    # A pause after the first stage, and then the comb going on: drawn into the frames before `at`.
    after = book[at]
    clean = [s for s in after.get("draw", []) if s.get("fill") != A.CUT and not s.get("still") and s not in comb + new_rods + old_holes]
    swap = []
    swap.append({"keep": after.get("keep", []), "draw": clean + held_rods + caption(("FIRST STAGE DONE: " + str(first) + " PASSES",), ("EVERY PIECE STILL ON ITS", "RODS, BY A TAB OF WOOD")), "ms": PAUSE, "turn": turned})
    five = ("5 OF 7: COMB IN, RODS SWAPPED",)
    for m in range(1, 4):
        swap.append({"draw": clean + held_rods + comb[:m] + caption(five, (f"FINGER {m} OF 3 SLIDES INTO", "ITS GROOVE FROM THE CAP'S END")), "ms": STEP_MS * 5, "turn": turned})
    for m in range(1, 4):
        swap.append({"draw": clean + held_rods + comb + new_rods[:m] + caption(five, (f"NEW ROD {m} OF 3, INTO AN", "INSERT IN THE FAR PLATE")), "ms": STEP_MS * 4, "turn": turned})
    for m in range(1, 5):
        swap.append({"draw": clean + held_rods[m:] + old_holes[:m] + comb + new_rods + caption(five, (f"OLD ROD {m} OF 4 OUT",)), "ms": STEP_MS * 4, "turn": turned})
    still_4 = at
    still_5 = at + len(swap) - 1
    # The comb going on, from the side too: between the pause and the fingers seen end on.
    side2 = []
    shorter = [rect(0.0, lay["pieces"][1], lo_y, hi_y, "#d6c3a0", "#6b4f2a")] + [rect(sp[0], sp[1], lo_y, hi_y, "#e5e7eb", "#6b7280", 0.5)] + far_plate()
    comb_len = lay["comb"][1] - lay["comb"][0]
    for s in range(0, 11):
        dx = -(comb_len + 10.0) * (1 - s / 10)
        side2.append({"draw": [cover] + shorter + rods_side(*rod_x, rods1) + nuts +
                      [rect(lay["comb"][0] + dx, lay["comb"][1] + dx, height(c) - 2.2, height(c) + 2.2, COMB, COMB_EDGE, 0.6) for c in rods2] +
                      [rect(lay["comb"][0] + dx, dx, lo_y * 0.8, hi_y * 0.8, COMB, COMB_EDGE, 0.6)] +
                      caption(five, ("THE COMB SLIDES ON", "OVER THE CAP")), "ms": STEP_MS if s < 10 else 1500, "turn": turned})
    swap = swap[:1] + side2 + swap[1:]
    still_5 = at + len(swap) - 1
    book[at:at] = swap
    where = [w if w < at else w + len(swap) for w in where]
    frames_before = len(frames)
    frames += book
    still_4 += frames_before
    still_5 += frames_before
    still_6 = len(frames) - 1

    # -- 7: apart -----------------------------------------------------------------
    seven = ("7 OF 7: NUTS OFF, RODS OUT",)
    lies_end = frames[-1]["turn"]
    tiles = stack(count, tile=True)
    spread = max(0.3, (x1 - 4.0 - lay["pieces"][0]) / count - b["piece"]["thick"])   # as far apart as the picture has room for
    for s in range(0, 9):
        dx = -(comb_len + 10.0) * s / 8
        frames.append({"draw": [cover] + tiles + far_plate() + rods_side(-PLATE_OUT, lay["far_plate"][0] + 6.0, rods2) +
                       [rect(lay["comb"][0] + dx, lay["comb"][1] + dx, height(c) - 2.2, height(c) + 2.2, COMB, COMB_EDGE, 0.6) for c in rods2] +
                       caption(seven, ("THE COMB COMES OFF",)), "ms": STEP_MS, "turn": lies_end})
    for s in range(1, 13):
        gap = spread * s / 12
        frames.append({"draw": [cover] + stack(count, gap=gap, tile=True) + caption(seven, ("AND IT FALLS APART",)), "ms": STEP_MS, "turn": lies_end})
    frames.append({"draw": [cover] + stack(count, gap=spread, tile=True) + caption((f"{count} TILES",), ("FACE GRAIN, NO SAW, NO KERF", "AND THE CAP'S A THICK ONE")), "ms": PAUSE + 1500, "turn": lies_end})
    still_7 = len(frames) - 1

    stills = {str(i + 1): f for i, f in enumerate((still_1, still_2, still_3, still_4, still_5, still_6, still_7))}
    view = [-half, -half, half, half]
    with tempfile.TemporaryDirectory() as scratch:
        mcp = Mcp(os.path.join(scratch, "none.okpart"))        # the tool needs no document; the server wants one to open
        print(mcp.call("flipbook", {"path": os.path.join(OUT, "billet.gif"), "width": SIZE, "view": view, "background": A.WHITE, "frames": frames,
                                    "stills": [{"frame": f, "path": os.path.join(OUT, f"billet_{name}.png")} for name, f in stills.items()]}).replace(OUT + os.sep, "out/"))
        mcp.close()
    seconds = sum(f.get("ms", A.MS) for f in frames) / 1000.0
    said = {"size": SIZE, "view": view, "frames": len(frames), "seconds": seconds, "passes": len(rows), "first": first, "second": len(rows) - first,
            "cut_frames": [w + frames_before for w in where], "stills": stills, "swap": len(swap), "pieces": count}
    with open(os.path.join(OUT, "billet_anim.json"), "w") as fh:
        json.dump(said, fh)
    print(f"   {len(rows)} passes ({first} and {len(rows) - first}) and the steps by hand, {len(frames)} frames, {seconds:.0f} s a showing")


PLATE_OUT = 7.0                                 # the second stage's rods stand this far out past the comb's plate, drawn


def _grown(c, d):
    """A rectangle given by its corners, grown by d on every side."""
    def unit(p, q):
        n = math.hypot(q[0] - p[0], q[1] - p[1])
        return ((q[0] - p[0]) / n, (q[1] - p[1]) / n)
    a, b = unit(c[0], c[1]), unit(c[0], c[3])
    return [(c[k][0] + d * (sa * a[0] + sb * b[0]), c[k][1] + d * (sa * a[1] + sb * b[1])) for k, (sa, sb) in enumerate(((-1, -1), (1, -1), (1, 1), (-1, 1)))]


if __name__ == "__main__":
    main()
