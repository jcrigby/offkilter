# A leaf blower adapter for an LG dryer's lint duct

Xypher2000's [Samsung Dryer Lint Trap Leaf Blower
Adapter](https://www.thingiverse.com/thing:7021003) jams a leaf
blower's nozzle into a tapered socket and spreads the air through a
thin-walled wedge into a half-moon end the size of the Samsung filter,
which goes down the filter's slot. That end is the part that matters:
its full plate blanks the housing's grille, so the air has no way back
into the drum and goes down the duct, out through the dryer's own
blower and vent hose. An LG dryer's filter (ADQ56656401) is a
near-square frame 18 mm thick standing in a straight slot at the
bottom of the door opening, so this redraws the end for that slot: a
blade the frame's width and thickness less clearance, 180 deep, an
inch past the grille's last holes, a closed box open only at its top
and its bottom edge so it goes in either way round, up into a sleeve
under the wedge. And since the owner connects the blower through a
length of leaf vac hose, not by its nozzle, the Samsung part's funnel
becomes a round spigot the hose's cuff slides over, with a bead. Two
printed parts, `out/adapter.stl` and `out/blade.stl`, or the same in
five for a 256 mm bed.

```sh
cargo build --release -p ok-mcp
python3 examples/lint-blower/build.py
cargo test -p ok-render --test lint_blower      # what CI runs
```

![the adapter with the blade](out/adapter_iso.png)

![cut through the bore's axis](out/adapter_section_iso.png)

## How it works

Pull the filter out. The slot it stood in has two ways out: down the
duct to the blower, and straight back through the grille into the drum
and the open door. The blade goes down the slot in the filter's place
and blanks the grille from inside; the sleeve and flange sit on the
lip where the filter's top edge was flush; the blower's air comes down
the wedge into the blade's open top and leaves at its open bottom edge,
below the grille, with nowhere to go but the duct. Both faces of the
blade are solid, so there is no wrong way to put it in.

## The numbers, and where they came from

| what | mm | from |
|---|---|---|
| `FRAME_T`, the frame's thickness | 18, the same everywhere | caliper |
| `FRAME_W`, the frame's width, rim to rim | 285 | a steel rule across it, read off photographs, gave 285 to 286; the fit test's two slices at 284 together were a little wider than the frame, so 285; the field photograph had read 288 to 294, the frame's 18 mm height adding parallax the tool does not know about |
| `EARS_W`, across the ears at the top corners | 306 | the field photograph; LG publishes 11-15/16 in |
| `FRAME_H`, bottom edge to the ears | 287 | the field photograph |
| `LIP_DIP`, how far the top edge's arc dips below the ears at the centre | 47 (R 218) | the field photograph; the lip is that arc |
| `CUFF_ID`, the hose cuff's inside diameter | 100, the hose's size across its wire coil: to measure | a rule across the smooth cuff's mouth, inside |
| `CUFF_DEPTH`, how far the cuff slides on | 40: to measure | the cuff's smooth length before the corrugations start |
| `GRILLE_D`, how far below the lip's ends the grille's last holes reach | 152 | 6 in by the owner's eye; 130 to 135 at the arc's centre on the cover guide's supplier photo on its inch grid, less at the ends |
| the frame's bottom corners | R 80, starting 80 above the bottom edge | the field photograph; the blade ends 100 above them, so they do not figure |

The field photograph is the filter on the 2x2 measuring field
(`out/measuring_field_2x2.pdf`: four Letter pages at 100 %, cut on the
dashed lines, overlapped and taped into one 467 × 354 mm field, marks
every 50 mm). `measure_photo` squared it up on all 39 marks at 0.31 mm
per pixel but did not pick the light grey frame off the white paper as
a part, so the outline was traced off its rectified picture; a filter
this size covers the marks of a single sheet, which is what the field
is for.

The first right half was printed at 144 from the joint, from the
field photograph's 290, and laid on the frame it showed the frame to
be narrower; a rule put the frame at 286. So the blade's right rail is
pinned where that half put it (`RIGHT_HALF_W`) and the frame's width
sets the left half alone, the joint 2.5 mm off centre.

Tried in the dryer, the two halves at 283 together go down the slot
and stop where the housing's left corner begins to turn, which on this
dryer is higher than the right one, so 180 is the depth to keep: the
blade seats on the housing's own corner, with the flange on the lip
above it.

The blade is drawn 1 mm inside the frame's width each side and 0.5
inside its thickness each face: the frame is what fits the slot, so
the blade copies it with room for a print's error. If the blade binds,
`CLEAR_W` and `CLEAR_T` at the top of `build.py` are the numbers to
open; if it rattles, close them.

## What it is

Coordinates are the dryer's: x along the slot, y from the back face
toward the user, z up with the slot's lip at zero.

| part | size | what it does |
|------|------|--------------|
| blade | 283 × 17 outside, 2 mm walls, 180 below the lip and 19 up the sleeve, two ribs face to face, square corners | the dummy filter: blanks the grille, carries the air down, open at the top and the bottom edge only |
| sleeve | 292.4 × 21.4 outside, 20 tall, standing on the lip | the blade slides up into it 0.2 a side and glues |
| flange | 3 thick, 8 past the sleeve at the front and both ends, flush at the back | rests on the lip's ends, where the filter's ears do |
| wedge | a loft, 100 tall, from the sleeve's section to a 104 square, the spigot's size | the Samsung part's wedge, drawn longer so the front leans 40° and the ends 43°, under the 45° a print without support allows; the walls are inset 2.6 and 2.8 in the plane so they are 2 through |
| square to round | a loft, 30 tall, from the square to a circle of 104 | the corners come in 21 mm over 30, 36° |
| spigot | a tube 100 outside, 2.5 wall, 45 tall, a bead 1 proud and 3 wide 6 below its top, a 45° cone up to the bead | the hose's cuff slides over it to the bead and past; the bead is what the cuff holds on |

The Samsung STL (1108 facets, 253 × 100 × 310 mm, 293 cm³) sectioned
every millimetre with `trimesh` gave its socket, which the first
drafts kept: a box z 110..200 by y −50..50, x −126.5..−73 at the back
and −91..−73 at the front; a bore of radius 32.5 at x −74 growing
0.345 per mm, 19°, to 38.4 at x −91, a funnel a nozzle jams into; the
back half of the box hollow; the back face one flat plate. Its end is
a D 310 across and 134 deep, the Samsung filter's outline, a plate
behind and a shorter skin in front, open round the arc: the plate is
what blanks that dryer's grille, and the blade here does its job.

## Printing

The parts are modelled as they sit in the dryer and print standing
that way, which puts nothing inside the ducts.

**The adapter** (`out/adapter.stl`, 303 wide, 195 tall) stands on the
sleeve's foot with a brim; the wedge leans 40° at the front and 43° at
the ends, the square rounds to the spigot at 36°, the bead has a 45°
cone under it, and the tabs start on the bed: nothing needs support
but the flange lip, 8 mm out at 3 mm up. **The blade** (`out/blade.stl`, 283 × 199 × 17) stands
on its bottom edge too, as exported, so nothing bridges: both faces,
the rails, the ribs and the halves' lap strips are vertical and start
on the bed. Lying on a face would make the upper face a bridge the
width of a bay. PETG, 0.2 mm layers,
three perimeters, which makes the 2 mm walls solid. About 217 and
249 cm³, 280 and 320 g in PETG.

**Five pieces for a 256 mm bed** such as a Bambu P1S:

| piece | size | |
|---|---|---|
| `out/middle.stl` | 124 × 104 × 195 | the spigot, the square and the wedge's middle and the sleeve's, with four tabs |
| `out/left_wing.stl`, `out/right_wing.stl` | 97 and 102 × 104 × 120 | the wedge's ends, the sleeve's and the flange's |
| `out/left_blade.stl`, `out/right_blade.stl` | 139 and 152 × 17 × 199 | the blade halves, standing, the right one with two lap strips |

The wedge's cuts are at the square's sides, under the spigot. The middle piece
carries four 2 mm tabs down its back and front inner walls, fused
10 mm into its own walls and reaching 10 mm into the wings 0.2 inside
theirs, so the wings slide on and glue along four strips. The blade's
halves meet at the middle; the right half's two lap strips, 8 wide
and 1.2 thick inside each face, go under the left half's plates and
glue, and the sleeve holds both halves at the top. Glue with CA or
epoxy, the blade's halves first, then the wings to the middle, then
the blade up the sleeve, and let it cure before the first blow.

**Fit test first.** `out/fit_test.stl` is the bottom 20 mm of both
halves, standing 12 mm apart in two rows on the plate, 32 g and under half an
hour: slide them together on the laps, drop them in the slot, and the
width and the thickness are tried before the halves are printed.

![the fit test](out/fit_test.png)

![the middle piece with its tabs](out/middle_iso.png)

![the right half of the blade with its laps](out/right_blade_iso.png)
