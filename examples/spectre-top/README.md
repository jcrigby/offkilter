# A Spectre puzzle top, built through MCP

The puzzle box's other top: the checkerboard's board (304 x 228 x 12)
tiled with the Spectre, the aperiodic monotile of Smith, Myers, Kaplan
and Goodman-Strauss, in its curved form, whose edges alternate in and
out round every tile like a jigsaw's. Half-inch edges make a tile about
60 x 41 mm; the board gets 65 pieces, the outer ones cut by its edge,
with a 1.5 mm gap between them for a contrasting resin fill. Maple for
the field and walnut for the mystics, the pairs of tiles that stand in
for a ninth supertile and give the tiling away; a 5 mm alignment web
in the gaps for the glue-up; and the printable pattern plate that cuts
every piece from one board on the duplicator, as the checkerboard's
does.

```sh
cargo build --release -p ok-mcp
python3 examples/spectre-top/build.py
cargo test -p ok-render --test spectre_top     # what CI runs
```

- `build.py`: one `add_spectre` op makes the top. The board's edge
  falls where it falls on the outer tiles, so the script tries eight
  slides of the tiling under the board and keeps the one with the most
  wood in pieces; a piece under 150 mm², the slivers the edge leaves,
  goes to the resin instead (`scrap`). Then the screenshots, the
  templates and the STLs, a look at the straight-sided tile, and the
  pattern plate.
- `out/spectre_top.okpart`: the document, a dozen numbers. Open it in
  the web app: the Spectre panel has the board, the tile edge, the
  gap, the curve, the shift, the scrap area, the woods, the web and
  the groove; slide the tiling and the pieces follow.
- `out/templates.dxf`: every piece outline at 1:1, lines at 10° along
  the curves.
- `out/pieces.stl`: the pieces, for the volumes and the look.
- `out/pattern_plate.stl` (and `pattern_plate.png`): the plate, the
  `pattern` layout: every gap as a groove 12.5 mm deep in a slab 10 mm
  wider than the board on a 3 mm floor, with the reference hole 5 mm
  inside its front-left corner. A 1.5 mm pilot in the grooves and a
  1.5 mm bit beside it on the same slide cut every piece from one board
  in one sitting, the kerf being the gap.
- `out/*.png`: the top, from above, the front and in iso; and
  `straight_top.png`, the same board with the straight-sided tile
  (`bulge: 0`), which is Tile(1,1) and no jigsaw at all.

## The numbers

| | |
|---|---|
| Board | 304 x 228 x 12 mm, the checkerboard's |
| Tile edge | 12.7 mm; a tile is 1322 mm², about 60 x 41 |
| Pieces | 65: 47 maple, 18 walnut; 182 to 1137 mm² |
| Wood | maple 514 cm³ (360 g), walnut 198 cm³ (127 g) |
| Resin | 13.9 % of the board: the 1.5 mm lattice and the scrap pockets |
| Web | 5 mm high, the connected lattice |
| Plate | 324 x 248 x 15.5, 1121 cm³, grooves 12.5 deep |

## How the pieces come out

Each edge of the tile is a cubic whose control points sit a third and
two thirds along it, 0.6 of its length to one side, the side
alternating round the tile; a neighbour runs the same edge the other
way with the other parity, so the curve is the same from both sides
and the curved tiles still tile. Every corner of the curved tile keeps
the straight tile's angle (90°, 120°, 180°, 240° or 270°), since both
curves at a corner are turned by the same amount; so the pieces have
real inside corners, which the bit rounds to its radius, and the
outlines are polylines rather than arcs (a biarc with the curve's end
tangents is a flatter curve than Kaplan's).

The kernel lays the shallowest substitution patch that covers the board
(depth 3, 559 tiles, for this board: the 71-tile patch is barely bigger
than the board and no rectangle fits its bays) with its most interior
point, the tile centre furthest from its boundary, at the board's
centre and slid by `shift`, then erodes every tile the board reaches by half
the gap (reflex corners rounded, the rest mitred, as the bit will) and
clips it to the board with the sketch region finder. A tile the edge
cuts in two is two pieces; a part under `scrap` is none. What is left
between the pieces is one lattice a gap wide, plus a few pockets: where
a lobe of a tile beyond the edge reached into a piece's concavity and
was scrapped, the board's edge closes the gap off there. The web is the
lattice proper; the pockets are resin; the plate grooves both.

The rounded Spectre is a jigsaw cut or at least sort of one: the lobes
nest, but their sides lean out at 61° from the edge rather than in, so
nothing undercuts and the pieces lift straight out. The resin and the
substrate hold the top, as they hold the checkerboard; this is a
puzzle to look at, cut with the puzzle's tooling.

## Why maple and walnut, and which pieces

No two woods can alternate on this tiling: three tiles meet at most
vertices, so any two of them touch. Three woods cannot colour it
properly either, since a tile with seven neighbours round it needs
four. On a gapped top the resin outlines every piece anyway, so the
colouring is free to say something, and it says what Kaplan's
"mystics" scheme says: the Gamma pairs dark, the rest light, two of
every nine tiles. The pairs are where the tiling's aperiodicity lives;
a top in one wood with the lattice in resin would be the other honest
choice, and is one `dark: null` away.

## What building it found

- A first draft laid the pattern out as sketches through the MCP
  server: thirty thousand lines in the document for one top. It became
  a kernel feature instead (`docs/DECISIONS.md`).
- The sketch region finder compared every pair of segments when
  splitting crossings and merging points; the lattice of a top's
  polylines took minutes. It now works through a grid, and a top
  regenerates in a few seconds in release, the pattern plate included.
- The board's edge always leaves slivers somewhere, whatever the
  shift: eight slides of the tiling gave smallest pieces of 0 to 8 mm²
  before `scrap` existed. A sliver is resin now, and the shift search
  keeps the most wood rather than chasing the smallest piece.
- Curves sampled at the document's 5° facet angle gave 25 pieces per
  edge and 14 s a regeneration; 10° is 13 pieces, a chord error under
  a tenth of a millimetre on these edges, and three times faster.

## Attribution

The tile, its curve and the substitution rules follow Craig S. Kaplan's
Spectre application (`cs.uwaterloo.ca/~csk/spectre`) and the paper
*A chiral aperiodic monotile* (Smith, Myers, Kaplan, Goodman-Strauss,
2023). The geometry is reimplemented in `crates/ok-sketch/src/spectre.rs`;
no code from the application is included.
