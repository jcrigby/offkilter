# An indexed stick mill

A reduced duplicator that mills the leaning half-ellipse Spectre's
whole outline round one solid stick, so a tile is one piece of wood
with no glue lines inside it. The stick turns between a headstock and
a tail centre and is held at one of twelve stops, 30° apart. A trim
router in its plunge base rides a carriage on two 20 mm shafts that
run the stick's length and are held at both ends. On the same spindle
as the stick, against its end, is a printed pattern: the tile's
section at full size.

```sh
cargo build --release -p ok-mcp
python3 examples/stick-mill/build.py
cargo test -p ok-render --test stick_mill   # what CI runs
```

![The mill: the carriage over the pattern, the stick beyond it, the index plate at the near end](out/mill.png)

## A pass

1. Pull the pin, turn the stick to the stop, drop the pin.
2. Bring the carriage over the pattern.
3. Turn the cross screw to the pass's place: every pass is on a mark
   of the handwheel, 0.125 mm.
4. With the router off, plunge until the bit itself touches the
   pattern, and lock the plunge. That is the depth. Nothing is
   measured.
5. Switch on, run the carriage down the stick and back.

The pattern does not have to be followed along the stick, since the
section does not change: set once, lock, cut.

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
| the index plate | twelve 5.2 holes on a 100 mm circle | 0.1 mm of slop at the pin is 0.05 at the tile's edge |
| the stick's stiffness | not checked | 200 mm between centres with necks of 4.5 mm in its section; halve the stick if it sings |

## Parts

| part | size, mm | |
|---|---|---|
| base | 586 x 340 ply | |
| risers, 2 | 60 x 320 x 57, three plies | the shaft supports stand on them |
| shafts, 2 | 20 dia x 516 | cut from the 700s; an SK20 at each end |
| carriage | four SC20UU bases up, two ply bolsters, two guides | 150 along |
| cross slide | 300 x 150 x 6 aluminium | the plunge base on it, an M8 screw across |
| headstock | two plies, 150 wide | M12 studding through it |
| index plate | 120 dia x 6, printed | outboard, over the left riser, with a pin post |
| pattern | the tile's section x 20, printed | on the spindle against the stick |
| tail | one ply, an M10 pointed bolt | |

## What the test checks

`crates/ok-render/tests/stick_mill.rs`: each shaft runs through a
support at both ends and the carriage's blocks stay between the
supports wherever the bit goes; the pattern and the stick regenerate
to the tile's section; the index plate is a disc less its spindle
hole and twelve stops; with the carriage over the pattern the bit
stands on it and the carriage clears the headstock, the index plate
and the pattern; and the plan's passes are a few hundred over all
twelve stops, on the handwheel's marks, within the bit's reach, the
plunge and the cross travel, with 95 % of the outline cut and nothing
left more than 0.4 mm proud.
