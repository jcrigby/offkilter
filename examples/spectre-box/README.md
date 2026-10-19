# A Spectre top for a 4 in puzzle box, built through MCP

The small box's top: a 4 in (101.6 mm) square tiled with the Spectre,
the aperiodic monotile, at a 9 mm edge, about 25 pieces of 43 x 29 mm,
maple for the field and walnut for the mystic pairs. Every piece is
routed one at a time to its own template with a 3/16 in micro-bearing
flush trim bit, so the design is shaped by that bit and nothing else:
the edges are circular arcs the bit can follow, the tile's four inside
corners are filleted to the bit's radius, and the convex corners that
meet them are rounded to the same circle, so the pieces still nest,
tight, with no gap.

```sh
cargo build --release -p ok-mcp
python3 examples/spectre-box/build.py
cargo test -p ok-render --test spectre_box     # what CI runs
```

- `build.py`: one `add_spectre` op with `curve: "arc"`, `bulge: 0.3`
  and `bit: 4.7625`. The board's edge falls where it falls on the
  outer tiles, so the script tries eight slides of the tiling and keeps
  the one whose smallest piece is biggest; a piece under 40 mm² goes to
  `scrap`. Then the screenshots, the templates, the STL, and a look at
  the rule the bit imposes.
- `out/spectre_box.okpart`: the document, a dozen numbers. Open it in
  the web app: the Spectre panel has the board, the edge, the curve and
  its bulge, the bit, the shift and the woods.
- `out/templates.dxf`: one closed outline per piece at 1:1, the
  routing templates, the fillets and rounds in them.
- `out/pieces.stl`: the pieces.
- `out/box_top.png`, `out/box_iso.png`: the top; `out/sharp_top.png`
  the same top with `bit: 0`, for what the bit changes.

## The numbers

| | |
|---|---|
| Board | 101.6 x 101.6 x 12 mm |
| Tile edge | 9 mm; a tile is 664 mm², about 43 x 29 |
| Curve | arcs of sagitta 0.3 edge: radius 5.1 mm everywhere |
| Bit | 3/16 in (4.76 mm): fillets of 2.38 mm |
| Pieces | 25: 19 maple, 6 walnut; 94 to 666 mm² |
| Wood | maple 99 cm³ (69 g), walnut 25 cm³ (16 g) |

## What the bit decides

A flush trim bit follows its template exactly except where the
template turns inside a circle the size of the bit: there it leaves a
fillet of its own radius. The pieces are the templates, so the model
has to draw what the bit will do, and its mate has to accept it.

- **The curves.** Each edge is convex on one piece and concave on its
  neighbour, so every curve has to clear the bit's radius on its
  concave side. Kaplan's cubic bottoms out at 0.28 of the edge at its
  apex, 2.5 mm at a 9 mm edge, which is over the bit by a tenth of a
  millimetre, and under it at 8 mm. A circular arc of sagitta 0.3 edge
  meets the corners at the same 62° Kaplan's curve does and has one
  radius all along, 0.57 of the edge, 5.1 mm here: twice the bit. The
  feature refuses a curve tighter than the bit with both numbers, which
  the script shows by asking for the cubic at 8 mm.
- **The inside corners.** The curved tile keeps the straight tile's
  angles, so it has four inside corners, two of 240° and two of 270°.
  Each sits at a vertex where exactly two tiles meet, against a 120° or
  a 90° convex corner of its neighbour. The kernel finds the circle of
  the bit's radius tangent to both curves at such a vertex, replaces
  the inside corner by that arc (what the bit leaves) and the convex
  corner by the same arc (what the mate then needs). Convex corners at
  vertices where three or four tiles meet stay sharp; the bit cuts
  those as drawn. The sum of the pieces' areas still equals the board:
  what a fillet adds to one piece its mate's rounding takes off.
- **The bays.** The pockets between lobes are an edge wide at the
  mouth, and the narrowest neck of a piece is 0.87 of the edge, so
  nothing else limits the bit above a 6 mm edge.

Each piece is routed from a blank bandsawn to a millimetre or two
outside its template; the templates are the DXF. The gap is zero: the
pieces nest as cut, and a gap for a resin line can be set in the panel
if wanted (the web and the pattern plate need one).

## What building it found

- The three curves hardly differ to the eye at this size; the arc is
  the one with margin.
- Slivers the board's edge leaves under the scrap area are dropped:
  three here, 21 mm² together, which the neighbouring pieces' blanks
  can simply be left to cover, or the resin can fill.
- A top regenerates in a third of a second; the fillets are found by
  bisection on each corner, two hundred of them on this board.
