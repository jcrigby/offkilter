# A checkerboard jigsaw top, built through MCP

A puzzle box top: an 8 x 6 jigsaw of 38 mm pieces, maple and walnut in
a checkerboard, each colour pin-routed from a single board so the grain
runs across the whole field of that colour. Two tabs: `Top` has a
1.5 mm gap between the pieces for a contrasting resin fill and a 5 mm
alignment web in the gaps that lines the pieces up on the substrate
during the glue-up; `Tight top` is cut with no gap at all, a strip one
bit wide between the rows, and a printable tray (`out/fixture.stl`)
with a pocket for every piece that does the lining up instead.

```sh
cargo build --release -p ok-mcp
python3 examples/puzzle-top/build.py
cargo test -p ok-render --test puzzle_top     # what CI runs
```

- `build.py`: one `add_puzzle` op does the lot; then it shows the design
  rules refusing a half-inch bit, hand-edits a tab's direction, another
  tab's size and position and one corner, and exports.
- `out/puzzle_top.okpart`: the document. Open it in the web app: the
  Puzzle panel has the grid, sizes, gap, bit, lock angle, grain, web
  height, corner jitter and a Reseed button, and a plan of the puzzle
  in which a click on a tab's head flips it, a shift-click selects it
  for its own size, neck and position, and a corner drags. A design the
  rules refuse still draws, with the rules listed under it.
- `out/templates.dxf`, `out/tight_templates.dxf`: every piece outline
  at 1:1, the routing templates. Each outline is lines and tangent
  arcs, so it is what the pin follows.
- `out/fixture.stl`: the tray for the tight top, exported on its own
  with `export {body: "Printing fixture"}`.
- `out/*.png`: the screenshots.

## How the pieces come out

Every interior edge carries one tab: two shoulder arcs, two walls that
lean in by the lock angle, and a head arc, all tangent. A tab that is
`out` bulges towards the piece above or to the right, so the piece
below or to the left owns it; the seed decides, and `set_puzzle_tab`
flips, scales, narrows or slides any one of them. Interior corners
wander by up to the jitter, and `set_puzzle_corner` places any one
exactly. The board's outer edges stay straight and flush; the gap is
between pieces only, taken off both sides, which turns each arc into an
arc of a different radius rather than a spline.

Because the colours alternate, no two pieces cut from the same board
share an edge. Each piece is routed to its own outline and the bit's
kerf comes out of its neighbour in that board, which is a waste piece,
so the fit between a maple piece and a walnut one is as tight as the
templates, not as tight as the kerf.

That holds along the edges and fails at the corners. Going round a
convex corner the bit sweeps a quarter disc of radius one bit diameter
on the far side, and on a checkerboard the far side is the same
colour's diagonal neighbour, in the same board: a tight fit gets a
round hole about two bit diameters across at every interior node. The
feature says so in a note under the plan. A gap of 0.7 bit diameters
keeps the neighbour clear; so does a strip between rows (`row_gap`) at
least one bit wide, which is what `Tight top` does. The rows then are
bands with tabs only along them, and a strip narrower than the bit
leaves every corner rounded by the difference, which the note also
reports.

The rules the feature checks, all on the outlines as cut (with the gap
taken off), naming every failure at once:

- the socket's opening and its head radius are at least the bit;
- the tab's shoulder radius is at least the bit's radius;
- the neck as cut is at least 2 mm or 5 % of the pitch, half again
  more when it runs across the grain;
- a tab's head keeps 15 % of the edge clear of each corner and stands
  no more than 45 % of the pitch proud;
- the gap leaves a shoulder and a head on the tab side;
- every cell stays convex with edges at least 60 % of the pitch, corners
  move at most 30 % of it;
- no piece's tabs or sockets run into each other.

## For a later version

- Veneer instead of solid stock: two consecutive leaves off one flitch
  are near enough identical that, with one left natural and the other
  dyed or toned, the grain runs across every colour boundary and the
  top reads as one board with a pattern laid over it. The templates
  are the same for both leaves, the colour split is by parity as now,
  and the corner rule applies unchanged; the piece thickness becomes
  the veneer's, on a substrate the fixture would then be sized for.

## What building it found

- A 3 mm corner jitter on 38 mm pieces sent neighbouring sockets into
  each other with the first tab proportions, which the rules did not
  catch. There is now a self-intersection check per piece, and the
  default tab is a third of the pitch tall instead of two fifths.
- One `add_puzzle` on an 8 x 6 grid regenerates in a couple of seconds
  (three in a debug build), most of it the alignment web: one sketch of
  every outline plus the short segments closing the gaps along the
  board's edges, from which the region finder yields the lattice. Fine
  for editing; a bigger puzzle would want the web built from strips.
- The pieces of one feature share its name and material in the parts
  list; the light and dark sets cannot yet be given maple and walnut
  separately. Materials per body, or a colour split into two features,
  is the next step.
