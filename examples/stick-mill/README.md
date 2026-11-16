# An indexed stick mill

A reduced duplicator that mills the leaning half-ellipse Spectre's
whole outline round one solid stick, so a tile is one piece of wood
with no glue lines inside it. The stick turns between a headstock and
a tail centre. A printed twelve-pointed star on its spindle (a hexagon
and the same hexagon turned 30°) drops into one cradle at any of
twelve stops, 30° apart. A trim router in its plunge base stands on a shelf out
from a ply square that rides two 20 mm shafts, one above the other
behind the stick, each held at both ends, with a bearing block behind
each corner of the square. On the same spindle as the stick, against
its end, is a printed pattern: the tile's section at full size.

```sh
cargo build --release -p ok-mcp
python3 examples/stick-mill/build.py
cargo test -p ok-render --test stick_mill   # what CI runs
```

![The mill: the back wall with its two shafts, the square and its shelf over the pattern, the star in its cradle at the near end](out/mill.png)

## A pass

1. Lift the star 8 mm out of its cradle, turn it a point, drop it
   back and clamp it. Every stop is the same move.
2. Bring the carriage over the pattern.
3. Turn the cross screw to the pass's place: every pass is on a mark
   of the handwheel, 0.125 mm.
4. With the router off, plunge until the bit itself touches the
   pattern, and lock the plunge. That is the depth. Nothing is
   measured.
5. Switch on, run the carriage down the stick and back.

The pattern does not have to be followed along the stick, since the
section does not change: set once, lock, cut.

## The star and its cradle

The cradle is the negative of the star's lower three points. The star
bears on six flanks of 15.5 mm: a flat seat under each outer point
that carries the weight, a shallow V (150°) under the bottom point,
and a steep flank each side, 73 mm apart, that sets the angle. No
flank is upright, so it drops straight in. A 1.5 mm round at each
point's seat keeps a blunt printed point from bottoming, and the
cradle's two crests are eased 1 mm clear of the star's inside corners.

The star is what sets the axis's height: its point is 57.7 under the
axis, then the relief and 6.8 mm of cradle, so the axis is 66 over
the base (it was 119 when a cradle stood in a tray under a hexagon
with room to spare). Everything above came down with it.

## The plan

`mill.json` lists every pass: its stop, its place across, and the
height the bit will find. For the tile at a 10.56 mm edge, 0.8 of a
semicircle deep, a 0.7 mm line and a 3 mm ball-nose:

| | |
|---|---|
| passes a stick | 260, 16 to 28 at each stop |
| ridge left between passes | 0.05 mm, to sand |
| outline within 0.08 mm of a pass | 95 % |
| the rest | up to 0.34 mm proud, in the tightest corners, for a file |
| the bit's tip | from 25.4 above the stick's axis to 10.6 below it |

Each point of the outline goes to the stop that faces it most squarely
among those whose bit can get down to it; where a neighbouring lobe is
in the way, the next stop round cuts it with the side of the ball.

## Measure these first

| what | in the model | check |
|---|---|---|
| the bit | a tapered ball-nose, 1.5 radius, 6 mm shank, 50 mm out of the collet | the deepest pass is 49.6 under the base's sole; a shorter bit needs the cross slide lower, which the blank's swing limits |
| the bit's taper in the dents | not modelled: the plan treats the bit as 3 mm all the way up | try the pattern by hand at the deep passes before cutting wood |
| the plunge base | 50 mm of travel, sole 100 square | the Colt's plunge base, or the cross slide's hole and travel change |
| the blank | sawn within 2 mm of the tile, swinging 29 | 33 between the axis and the cross slide |
| the star | two hexagons 100 across their flats, on six flanks in one cradle | a tenth of a millimetre at a steep flank is 0.07 at the tile's edge; if one stop rocks, file the high flank |
| the room under the stick | 37 between a blank's swing and the base | enough for chips and a brush; the carriage square's lower edge is 42 over the base |
| the hold-down | not drawn | a bar over the star on two studs with wing nuts, or a toggle clamp |
| the shelf | 195 out from the square, a gusset over each edge | push down on its front edge: if the bit moves, deepen the gussets |
| the stick's stiffness | not checked | 200 mm between centres with necks of 4.5 mm in its section; halve the stick if it sings |

## Parts

| part | size, mm | |
|---|---|---|
| base | 576 x 393 ply | |
| back wall | 556 x 262, two plies, a brace in front at each end, 120 x 200 | the four shaft supports bolt to its face; the braces stand outside the shafts' ends |
| shafts, 2 | 20 dia x 506 | cut from the 700s; one 150 above the other, an SK20 at each end |
| carriage square | 200 x 200 ply, upright | an SC20UU behind each corner: the least that takes two shafts 150 apart and their 50 mm blocks, the blocks 145 apart along the shafts |
| shelf | 150 x 195 ply, two gussets | out over the stick, the cross slide on it |
| cross slide | 165 x 90 x 6 aluminium | the plunge base on it, an M8 screw from the front |
| headstock | two plies, 140 wide | M12 studding in a slot, in a slot 9 tall over it: the star lifts 8.0 to turn |
| star | 115.5 over its points x 20, printed | on the spindle, outside the shelf's reach |
| cradle | 150 wide x 29.4 high x 20, printed | two screws down into the base |
| pattern | the tile's section x 20, printed | on the spindle against the stick |
| tail | one ply, an M10 pointed bolt | |

## What the test checks

`crates/ok-render/tests/stick_mill.rs`: each shaft runs through a
support at both ends and the carriage's blocks stay between the
supports wherever the bit goes; the pattern and the stick regenerate
to the tile's section; the star is two hexagons' worth, sits in its cradle without passing
through it on six flanks that all run across, rises clear inside the
spindle's slot to turn, and leaves 4 to 10 mm of cradle under its
point, which is what makes the axis as low as it can be; with the carriage over the pattern the bit stands on it and
the square, the shelf and the cross slide clear the headstock, the
star, its cradle, the pattern and the wall; and the plan's passes are a few hundred over all
twelve stops, on the handwheel's marks, within the bit's reach, the
plunge and the cross travel, with 95 % of the outline cut and nothing
left more than 0.4 mm proud.
