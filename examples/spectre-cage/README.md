# A cage for routing a stack of Spectre pieces

Every tile of a Spectre top is the same shape, so the top's pieces can
be cut as one prism. Each piece is a hex blank of 1.5 mm veneer glued
to a 1.5 mm ply backer; the blanks are stacked dry, nothing glued
across the stack, in a hexagonal tube and clamped end to end by six
M4 rods along its corners. The tube's six sides come off one at a time:
through the open window a bit on the duplicator routs the profile,
following a printed master indexed in the same hex, and the side goes
back with fins that fill what was cut, so the stack is never
unsupported. When the rods come out, the pieces fall apart finished,
with the faces they were glued up with.

Two masters share the cage and the stack. The curved tile, arcs of
sagitta 0.3, is routed with a ball-nose bit, which follows the arcs
and leaves its own radius in the inside corners, and its convex
corners are rounded to the bit too, the knob template-routed pieces
have. The straight tile is routed with a square-end bit and comes out
exactly, sharp corners and all: every edge of it lies on the 30 degree
grid, so at one of the twelve indices it stands parallel to the bit or
flat to its floor.

```sh
cargo build --release -p ok-mcp
python3 examples/spectre-cage/build.py
cargo test -p ok-render --test spectre_cage     # what CI runs
```

![The loaded cage, its top window open](out/cage_iso.png)

![The six finned panels round the curved master](out/fins_curved_end.png)

![The six finned panels round the straight master](out/fins_straight_end.png)

## What the script does

- Asks the kernel for each master with `spectre_tile`: the spectre-box's
  tile at a 9 mm edge, eroded by half the 0.7 mm gap the top's pieces
  will have between them. The curved one has arcs of sagitta 0.3, its
  four inside corners filleted to the 3/16 in bit and its ten convex
  corners rounded to it as well (`round`; 0 keeps the Spectre's own
  corners). A piece routed to its template has the corners that seat in
  a neighbour's fillet rounded and the rest sharp, which corners
  depending on where the tile sits; one master cannot do that, so it
  rounds all of them, which is the softer knob, at the cost of a sliver
  of resin where three tiles meet, which a top with a gap has anyway.
  The straight one has no fillets and no rounding. The blank hex is the
  larger master's circle plus a margin: 48 mm across flats.
- Clocks each profile in the hex. The two cradles give twelve indices:
  on the flat one a panel's window faces straight up, on the V one the
  hex rests on a corner and the window either side of the top corner
  is 30 degrees off vertical, so the bit comes down a corner's
  direction through one of two panels. For each turn of the profile
  the script checks every edge of the outline by its bit's rule. A
  ball-nose bit cuts an edge that does not face away from it when, with
  the ball on the edge, the column its shank sweeps up to the window is
  clear of the profile and of the five panels still in place. A
  square-end bit cuts an edge only parallel to it, by its side, or flat
  to its floor, with the same column clear; an edge shorter than half a
  millimetre, the gap's rounding of a corner, it leaves as the corner.
  It keeps the turn that reaches the whole outline. Six flat indices
  alone do not, for either: the curved tile's deep bays open toward the
  hex's corners, and the straight tile has edges on every other line of
  the grid.
- Lays out the top itself (`add_spectre`, four woods, no two
  neighbours alike) to count the pieces and read their woods, and
  writes the stack order, one laminate per piece plus two spares, to
  `out/stack.txt`.
- Builds each master (the profile over the stack's length with a hex
  end at each end, the flanges' hex, for the cradles) and its six finned
  panels round it, the loaded cage (five panels with the top window
  open, the plugs, the rods and nuts, the laminates by wood, the flat
  cradles under the flanges), and the two cradles on their own.
- Exports the printable parts as STL and the numbers as `out/cage.json`.

## The parts

| | |
|---|---|
| blank | hex 48 across flats, 3 mm: 1.5 veneer on 1.5 ply |
| stack | 26 laminates (24 pieces and 2 spares), 78 mm |
| panels | 2 mm plates, edges mitred at 60 degrees, a hair short of each other |
| rods | M4, in the V between two panels' outer faces at each corner, nuts outside the flanges |
| plugs | a hex boss 15 deep into the tube, a 10 mm flange outside it, 66 across flats |
| masters | the profile over 78 mm, a 25 mm hex end each side, 66 across flats; one curved, one straight |
| cradles | a flat socket (the lower half of the 66 hex) and a 120 degree V, 24 long |
| fins | a set per master, 0.3 mm off the profile every way, and off each other at the sector lines |

## The sequence

For each panel: take it off, set the cage in the flat cradle with the
window up and rout; set it in the V cradle leaning one way and rout,
then the other way; put the finned panel on. Eighteen settings, six
panel changes. The master sits in its own pair of cradles beside the
cage at the same index every time. The curved master with the ball-nose
bit and pilot, the straight master with the square-end pair, each with
its own finned panels.

## What the test checks

`crates/ok-render/tests/spectre_cage.rs` reads the document and
`out/cage.json` and checks, for each master, that it is the top's tile
centred in a hex that holds both, the curved one's knobs rounded and
the straight one's edges all on the 30 degree grid, with the body's
volume that profile over the stack plus its ends; that every edge of
the clocked profile is reachable by its bit at one of the twelve
indices, by the test's own sweep of the bit's body up from the window,
and that six flat indices alone are not enough; that from above a ball
bit leaves no residual on the curved profile, the fillets being its
own; that every finned panel clears its master and the other five fins
and fills its sector's column to within the clearance; and that the
loaded cage reads back as the stack with its woods, the rods clear of
the panels, the stack and the plugs, the flanges the masters' hex,
resting on the cradles' floors, and the two cradles' troughs half a
hex and a corner.

## Notes

- The spares are at the ends on purpose: a pilot cannot reach into the
  corner where the profile meets the master's hex end, so the copy
  carries a fillet there, on the first and last laminate.
- The curved master wants a ball-nose bit and pilot. A square-end bit
  can only leave vertical walls and flat floors; the arcs would come
  out as steps. The straight tile is nothing but walls and floors once
  there are twelve directions to present them from, so the square-end
  bit makes it exactly, and leaves the inside corners sharp where a ball
  would fillet them.
- Fins thinner than 1.5 mm at their thickest are left off; they would
  not print and would not support anything.
