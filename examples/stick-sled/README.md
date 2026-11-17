# A stick sled

The leaning half-ellipse Spectre milled round one solid stick, on a
router table (or over a saw), with the stick positioned by three small
motors and pushed over the cutter by hand. It replaces the
[stick mill](../stick-mill/): no carriage, no shafts for it to ride,
no pattern to touch the bit on, nothing dialled.

```sh
cargo build --release -p ok-mcp
python3 examples/stick-sled/build.py
cargo test -p ok-render --test stick_sled   # what CI runs
```

![The sled on a table: the lift plate over the stick, hinged at the back on the riser and resting on a pusher at the front; the stick hanging under it over the cutter](out/sled.png)

## How it works

The cutter stays where it is, up through the table. The sled slides
along the fence, and the stick hangs under it between a spindle and a
tail centre.

![From the spindle end, cut through the middle: the lift plate on its hinge and its pusher, the frame hanging from the second hinge, the level pusher against it, the stick's section on the cutter](out/end.png)

- **Height:** a ply plate hinged along the sled's right side, resting
  on a screw-driven pusher at its left. The stick is between hinge
  and pusher, so the plate is a bridge, not an arm.
- **Left and right:** a frame hangs from a second hinge under the
  plate, above the stick. A second pusher, level, fixed to the sled,
  bears on the frame's tail hanger and swings it; a spring holds the
  hanger against it.
- **Rotation:** a motor on the hanging frame turns the spindle through
  a belt, 16 teeth to 60.

Swinging the plate also shifts and tilts the stick a little, and so
does swinging the frame. Both are exact functions of the two pushers,
so `plan.py` works the hinges backwards for every cut: the table holds
three numbers a row, where the two pushers and the spindle stand.

## A pass

1. Pull the sled back. The stick is clear of the cutter.
2. Press the foot switch. The motors go to the next row and stop.
3. Push the sled through and pull it back.

The controller will not move a motor unless the sled is back: a switch
the sled closes there. Between passes it lifts the stick first to a
height at which the spindle clears the cutter, then swings and turns,
then comes down.

## The table: 653 rows a stick

| rows | what | cutter |
|---|---|---|
| 1 | set the straight bit | 6 mm straight |
| 6 + 48 | two round journals, three places each, eight bites a place | |
| 154 | roughing: 88 places from 24 directions, in bites of 3 mm | |
| 1 | set the ball-nose | 3 mm ball-nose |
| 443 | finishing: 242 passes, the deep ones in bites of 1.5 mm | |

- **Every finishing pass has its own direction.** With a motor on the
  spindle nothing needs twelve stops, so each pass comes at its face
  as squarely as the tile lets a bit in. The passes reach the whole
  outline: every point within 0.05 mm of one. Twelve stops left 5 %
  for a file.
- **The cutter stays within 19 mm of the axis**, which is all the
  frame has to swing.
- **Roughing takes 79 % of the waste** with the straight bit, to
  0.4 mm of the tile. The ball-nose's deepest cut is 14.1 mm, in ten
  bites.
- **Journals are turned.** With the sled at a stop and the foot switch
  held, the stick comes down a bite and goes once round, and stops if
  the switch is let go.
- **Setting a bit.** There is nothing to touch off but once a bit: the
  first row, and the row after roughing, bring a round master on the
  spindle to where the cutter's tip should be, and the router is
  raised until the bit touches it.

The rows go round the stick in the order of the spindle's angle, so it
turns a degree or two at a time, 950° in all. It comes up to every
angle forwards, so the belt is always pulled from the same side.

## The controller

`firmware/main.py` and `firmware/config.py` are MicroPython for a
Raspberry Pi Pico; `out/passes.csv` is the table. Copy the three to
the Pico. `config.py` has the pins, the steps for a millimetre and a
degree, and the trims found from a test stick.

`python3 main.py --sim passes.csv steps.json` runs the same program on
a computer, on pins that do nothing. The build does that and the test
checks every row ends on the table's numbers to the step.

| | |
|---|---|
| controller | Raspberry Pi Pico |
| drivers | three TMC2209 modules, step and direction; 8 microsteps for the pushers, 16 for the spindle |
| motors | three NEMA 17, 200 steps |
| a pusher's step | 0.00125 mm (a T8 screw of 2 mm lead) |
| the spindle's step | 0.03° |
| switches | a home for each pusher, a mark once round for the spindle, sled back, and the foot switch |
| supply | 12 V |

No encoders. The lift screw is loaded by the plate's weight and the
swing screw by its spring, each always from the same side.

## Measure these first

| what | in the model | check |
|---|---|---|
| the cutter over the table | 39.0 mm, the bit's stick-out | the blank clears the table by 2.6 at its lowest; a shorter bit needs a smaller tile or a hole in the table |
| the bit | 3 mm ball-nose, taken as 3 mm all the way up | a tapered shank may rub in the deep cuts; try them on scrap |
| the fence | 290 mm right of the cutter | a table whose fence will not go that far wants a bar in its mitre slot under the left runner |
| the lead screws | T8, 2 mm lead | the common T8 is 8 mm lead: `config.py`'s steps per millimetre are a quarter then |
| the shafts, supports and blocks | the 20 mm set from the duplicator: two shafts of 490 and 416, four SK20, four SC20UU | the blocks turn on the shafts as hinges; collars keep them from sliding |
| the hinges' places | hinge 1 at 150 right and 205 up, hinge 2 150 left of it and 26 down, the axis 130 under hinge 2 | measure the built sled and put its numbers in `build.py`; the table follows |
| the home switches | 3 and 2 mm beyond the furthest any row goes | cut a test stick, measure it, and put the differences in `config.py`'s trims |
| the cutter's place across | under the middle of the frame's swing | the same test stick: a cut that is off to one side shows as a tile fat on one side |
| the pushers | drawn as a motor, a screw, a nose and a printed guide | the nose must not turn: a flat or a key in the guide. Not drawn |
| the spring | not drawn | from the tail hanger to the riser, enough to hold it on the pusher against a cut |
| the drive | a spur on the spindle's end, the tail centre pushing the stick onto it | not drawn in detail |
| stiffness | not checked | lean on the stick with the motors holding: if the cutter's mark moves, stiffen what gave |

## Parts

| part | size, mm | |
|---|---|---|
| sled | 540 x 480 ply: two runners and a tie at each end | open under the stick; its right edge on the fence |
| riser | 540 x 216 ply, a gusset at each end | hinge 1's two supports on its face |
| lift plate | 420 x 335 ply | two blocks under its right edge on hinge 1; hinge 2's supports under its middle |
| beam | 350 x 60 ply | two blocks on hinge 2; the hangers under it |
| head hangers, 2 | ply, 70 wide, round-ended | a 6201 bearing in each, 43 apart |
| tail hanger | ply, 70 wide, square-ended | the tail centre; its right edge is what pusher 2 bears on |
| spindle | M12 studding | the 60-tooth pulley outboard, the master and a drive spur inboard |
| steadies, 2 | printed, 23 wide at the journal | caps over the journals from above, where the cutter does not come |
| master | printed, 19.4 round x 12 | on the spindle beside the stick, for setting a bit |
| lift pusher | NEMA 17 on the left runner, a printed tower on its face, a nose of 120 | 86 mm of travel |
| swing pusher | NEMA 17 and a printed guide on the right runner, 60 over the table, a nose of 90 | 37 mm of travel |
| spindle motor | NEMA 17 on the head hanger, 52 over the spindle | a closed GT2 belt |
| controller | a box on the right runner | Pico, three drivers |

## What is still true of the tile

The stick is limp once it is cut, its necks are 4.5 mm, and three
cuts are deep and narrow. The journals and steadies are for the
first; the others are the tile's, not the machine's.

## What the test checks

`crates/ok-render/tests/stick_sled.rs` has the hinges written a second
time, in Rust, and works every row forward through them: the cutter is
where the plan wants it to a fiftieth of a millimetre, a finishing
ball is never in the tile and a roughing bite never within its
allowance; the last bites of the finishing passes together reach the
whole outline, from nearly as many directions as passes; every bite
is within its bit's limit; at every row the blank clears the table,
so do the pulley and the tail hanger's corners, the cutter is at
least a millimetre from a steady, and pusher 2 bears on the hanger's
edge; the controller's program, simulated, ends every row on the
table's steps, turns the spindle under three turns in all, and never
swings the stick below the height at which the spindle clears the
cutter; and the document is the sled at one finishing row, the
cutter's tip on the stick and no part through another.
