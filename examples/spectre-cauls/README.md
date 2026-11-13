# Clamping cauls for a Spectre tile stick, and the layout template

The tile sticks of the hex-edged Spectre glued tight instead of cast,
and the tiles kept as separate units. The sticks of one tile (the
lattice triangles and halves of `examples/spectre-mosaic`) are laid
lengthwise into a printed cradle whose pocket is the tile's outline,
glued with the thinnest line, and a printed plunger is pressed down on
them, so the tile comes out as one solid stick. The stick is sliced
into tiles. The tiles are laid out on a printed template, a pocket
per tile with the 0.7 mm lines standing between them as ridges, taped
across their tops to hold the layout, lifted off, and glued and
clamped to a substrate, the dark glue or a coat of dark resin showing
in the lines. Keeping the tiles separate is what lets a later version
push them.

```sh
cargo build --release -p ok-mcp
python3 examples/spectre-cauls/build.py
cargo test -p ok-render --test spectre_cauls   # what CI runs
```

![The cauls in section: the cradle with the tile in its pocket, the plunger lifted above it](out/section.png)

## The tile is the tile less half the line

Tiles that stand 0.7 mm apart on the tiling (the owner's pick from
the printed strip) are not the tile: they are the tile eroded by 0.35
all round, which is what every layout in this repository has drawn as
a piece since the puzzle top. So the cradle's pocket is the eroded
outline (plus 0.1 of clearance for the saw's error over a few
sticks), and the sticks along the tile's boundary are the lattice
shapes with that side moved in by 0.35. A
side moved in leaves a triangle a triangle and a half a half, only
smaller, so the cut list stays short. A stick that meets the tile's
edge with a point rather than a side, which a third of the sticks at
a reflex corner of the outline do, is cut as the whole shape and has
the point taken off at the eroded tile after: a flat a quarter of a
millimetre deep, a pass over a sanding block along the stick. (The
first cut list listed those as whole inner triangles, which would
have stood 0.25 mm proud into the line and not gone into the pocket;
the kernel now keeps every stick half the line off the outline, and
the test checks it.) For one tile at a 10.56 mm edge, with a 0.1 mm
glue line between sticks:

| count | stick | sides, mm | point taken off |
|---|---|---|---|
| 33 | triangle, one side on the tile's edge | 4.76 | 4 of them a corner, 0.09 |
| 27 | triangle, inside | 5.11 | 21 of them, 0.25 |
| 16 | small half, inside | 1.18, 2.05, 2.37 | 3 of them, 0.21 to 0.25 |
| 12 | half, inside | 2.50, 4.34, 5.01 | 3 of them, 0.16 to 0.21 |
| 10 | half, one side on the edge | 2.20, 3.82, 4.41 | 1, 0.25 |
| 2 | half, cut back further | 2.16, 3.74, 4.31 | |
| 1 | half, its long side on the edge | 2.33, 4.04, 4.66 | 1, 0.21 |
| 1 | trapezoid, inside | 0.89, 2.16, 2.19, 2.52 | |

102 sticks in eight shapes, 33 of them with a point taken off. The small halves are the ones to drop if
they are too fiddly to saw; they become glue and dust, 2 % of the tile.

## Why the lid takes one bump

A lid that comes down onto a bundle is a negative of the part of the
outline above the split, and it only comes down if that part does
nothing but narrow going up: a dent above the split makes the outline
widen again, and the lid's material under that overhang would have to
pass through the sticks. Every turn of the Spectre has a dent close
under its top, so the search (every degree of turn, every split at a
corner, the lowest split that works at each turn, the turn whose lid
takes the most) ends with the lid taking one bump, 7 % of the eroded
tile. That is still the right clamp. The cradle's pocket enforces the
other 93 % exactly, since the sticks go into it one at a time from
above and a bay under an overhang is reached by sliding the stick in
sideways before the sticks above it are placed; and the plunger's
force spreads through the lattice at 60°, so by the floor it covers
the tile's width. The plunger is the slot's full width for stiffness
and the clamps, with the bump's negative cut into its flat underside.
Beside the bump the cradle rises 4 mm above the split in two
shoulders, so its lip over a bay just under the split is never thin
(the first version had that lip 0.04 mm thick at one spot); the bump's
sides, which stand straight up from the chord for 5.4 mm, fill the
slot between them, and the plunger's flat stops a millimetre above the
shoulders when its notch seats on the bump's roof: that millimetre is
its travel. Nothing in the cauls flexes; the squeeze is the pocket's
clearance and the glue lines.

| | |
|---|---|
| turn | 330°, split 38.0 mm up of the tile's 45.9 |
| pocket | the eroded outline plus 0.1 mm each side |
| cradle | 60 x 60 x 50 mm, walls 6, floor 6, the walls 8 above the bundle |
| plunger | 48 x 27 x 50, 0.15 short of the walls each side, 12 above them when down |
| per stick | three of each along a 200 mm stick, like the ladder's stands |

The tile's mirror image needs no second set: a prism's mirror is the
prism turned end for end, so the same cauls make both chiralities.

## The template

![The template: a pocket per tile, the lines standing between them](out/template.png)

`template.stl` is `add_spectre` with the hex curve, the 4 in board,
every tile the board reaches kept whole, and the `template` layout:
a slab 200 x 192 with a 3 mm pocket per tile, 21 of them, the 0.7 mm
lines between them standing as ridges and the slab a margin wider
than the tiles. The sliced tiles drop into the pockets, which puts
every one at its exact place and spacing; tape across the tops holds
them; the taped sheet lifts off and goes down on the glue. The
pockets are shallow so the tape reaches most of a 10 mm tile and the
sheet does not bind on the ridges. A faint scribe, 0.5 wide and 0.4
deep, runs round the lid's trim rectangle across the pocket floors
and the ridge tops, so which part of a tile ends up in the lid is
plain while it is being laid.

## The line sample

![The strip: the same patch of the lid at 1.5, 1.0 and 0.7 mm lines](out/gap_sample.png)

`gap_sample.py` writes `gap_sample.stl`, a 138 mm strip of three
samples of the lid, the same 40 mm patch of tiles in each, with the
line between tiles at 1.5 mm (the current width), 1.0 and 0.7, left
to right. Every tile is a 3 mm pad on a 1.5 mm base, sunk 0.3 into
it, so the gaps between pads are the lines; printed in a dark filament
they read as the dark glue will. Notches in each base's front edge
count the samples, one for the current line, three for the
narrowest. Change `GAPS` in the script for other widths; the one the
eye picks goes into `GAP` here and in the mosaic.

The owner picked 0.7. `gap_sample.py --round` writes
`gap_sample_round.stl`, the 0.7 sample again with every tile's top
edges rounded over by 0.6 mm, as a tile eased with a sanding block
would be, so the line reads deeper than it is. The kernel's fillet
rolls a true radius, but across three hundred facet edges of eroded
corners and the slivers the board's edge leaves it breaks on one of
them, so the roundover is built the way the printer builds it at its
0.2 mm layers: the pad stops its radius short of the top and the last
three layers are each the piece eroded to where the circle stands at
that height. The print is the same stair a sliced fillet would be.

![The rounded sample: the lines between eased edges](out/gap_sample_round.png)

## What the test checks

`crates/ok-render/tests/spectre_cauls.rs` regenerates the cauls'
document and reads `cauls.json`: the cradle's volume is its block less
its pocket and the plunger's is its block less the bump's negative, at
the length; the pocket's lower part and the negative together are the
eroded tile plus the clearance; the part of the outline above the
split only narrows going up, within the slot; the plunger fits the
slot with its play and the cradle's lip over every bay is at least the
shoulder's height; the template regenerates to one body with a pocket
floor per tile and the trim scribe in its floors and ridges; the line sample strip is three samples long at the three widths; and the cut list is a few shapes, triangles the
commonest, every triangle equilateral and every half a 30-60-90.
