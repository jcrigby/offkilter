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

![The sled on a table: the lift plate over the stick, hinged at the back on the riser and held by a screw at the front; the stick hanging under it over the cutter](out/sled.png)

Nothing here has been built or run on a motor. What is checked is
checked by calculation and by the test; what is estimated says so.

## How it works

The cutter stays where it is, up through the table. The sled slides
along the fence, and the stick hangs under it between a spindle and a
tail centre.

![From the spindle end, cut through the middle: the lift plate on its hinge, its screw and link at the far side, the frame hanging from the second hinge, the level screw and its link to the tail hanger, the stick's section on the cutter](out/end.png)

- **Height:** a ply plate hinged along the sled's right side and held
  at its left by an upright screw. The stick is between hinge and
  screw, so the plate is a bridge, not an arm.
- **Left and right:** a frame hangs from a second hinge under the
  plate, above the stick. A second screw, level, fixed to the sled,
  swings it by its tail hanger.
- **Each screw's nose is a block on a linear rail's carriage**, so it
  can neither turn with the screw nor move across it.
- **Held both ways.** Each screw's nose is tied to what it moves by a
  short link with a pin at each end: 30 mm to a clevis under the
  plate, 60 mm to the tail hanger. A lead screw does not run back
  under load, so the stick is held against a cut that lifts it or
  pushes it either way. A light spring on each only keeps the slack
  in nut and pins to one side.
- **Rotation:** a motor on the hanging frame turns the spindle through
  a belt, 16 teeth to 60.

Swinging the plate also shifts and tilts the stick a little, and so
does swinging the frame, and a link leans as its far pin moves. All
of it is exact, so `plan.py` works the hinges and links backwards for
every cut: the table holds three numbers a row, where the two noses
and the spindle stand.

## A pass

1. Pull the sled back to its stop. The cutter is under the master, a
   sleeve on the spindle between the stick and the hanger.
2. Press the foot switch. The motors go to the next row and stop.
3. Push the sled through and pull it back.

The controller will not move a motor unless the sled is back: a switch
the sled closes there. With the sled back the master is over the
cutter, so a move between passes must not carry it through the
cutter, nor turn the blank into the table. The safe way is up to a
height at which the master clears the cutter wherever the frame
swings, then across and round, then down. Most moves do not need it:
the build follows each one along its own path, swing, turn, lift, and
marks it straight if the master passes the cutter by 1.5 mm and the
blank the table by 1 mm all the way. 408 of the 502 are.

A stop is clamped to the fence at each end of the travel, in the
right runner's way. Against the back one the cutter is 12 mm behind
the stick's end, under the master, and a switch on the runner's end
is closed. Against the forward one it is 6 mm past the stick's far
end. That is 218 mm of travel. The stops are forgiving: back, the
cutter may be anywhere under the master, 29 mm of it; forward, past
the stick's end there is only the tail centre for 30 mm, and no pass
comes near it.

## The table: 502 rows a stick

| rows | what | cutter |
|---|---|---|
| 1 | put the blank on | |
| 1 | set the straight bit | 6 mm straight |
| 200 | roughing: 82 places from 24 directions, in bites of 3 mm | |
| 1 | set the ball-nose | 3 mm ball-nose, tapered |
| 299 | finishing: 244 passes, the deep ones in bites of 1.5 mm | |

- **The blank is a plain rectangle,** 49 x 42 mm and the stick's
  length: the least rectangle about the tile with 2 mm over on every
  side, to the whole millimetre. Mark the centre on each end 19.5 mm
  from one wide face and 24.8 from one narrow face, the same two
  faces at both ends. The first row turns the spindle to where the
  blank goes on with that wide face level and underneath and that
  narrow face toward the fence. (The cuts were first planned from the
  tile's hull with 2 mm over: 63 fewer pushes, but six flats at odd
  angles with round corners between, which nothing makes.)

- **Every finishing pass has its own direction.** With a motor on the
  spindle nothing needs twelve stops. The passes reach the whole
  outline: every point within 0.05 mm of one. Twelve stops left 5 %
  for a file.
- **Off the ball's dead centre.** The very tip of a ball-nose does not
  turn, it only rubs. So each pass comes at its face 15° round the
  ball from the tip, where the edge passes at about a metre a second,
  wherever the tile lets the bit in that way: all but 2 of the 244
  do. Square-on, 162 of them cut on the dead centre. It costs
  nothing: the same passes, the same reach, the same bit height.
- **The cutter stays within 19 mm of the axis**, which is all the
  frame has to swing.
- **Roughing takes 87 % of the waste** with the straight bit, to
  0.4 mm of the tile. The ball-nose's deepest cut is 10.0 mm, in
  seven bites.
- **The order is settled first, the bites after.** Roughing and then
  finishing go round the stick in the order of the spindle's angle,
  so it turns a degree or two at a time, 952° in all, and comes up to
  every angle forwards. What a cut meets is what the cuts before it
  left, so the bites are counted in that order and no other.
- **Setting a bit.** The second row, and the row after roughing, bring
  the master to where the cutter's tip should be, and the router is
  raised until the bit touches it. Homing may land a hair differently
  each day; setting the bit on the master takes that up.

A stick is about two and a half hours: 499 pushes at a quarter of a
minute, and half an hour of the motors moving between them (5.9 m of
lift screw at 800 steps a millimetre; 8.6 m if every move went by
the safe height). Estimated.

## The bit

Planned for the common tapered ball-nose: 1.5 mm radius at the tip,
widening 2.8° a side to a 6 mm shank over 30.5 mm. The planner keeps
the whole of it out of the tile, flank and shank too, and still
reaches all of the outline. Planned as if 3 mm all the way up, as it
first was, that bit would have cut into the tile on 120 rows, by up to
0.33 mm.

- A straight 3.0 mm ball-nose fits inside the tapered one, so the same
  table is safe for it. A 1/8 in one is 0.09 mm fatter and is not.
- The tapered bit is also about thirteen times stiffer at this
  stick-out: 1.5 µm a newton at its tip against 19.
- A steeper taper reaches less: at 5° a side, 98 % of the outline.
  Change `TAPER` and `SHANK` in `plan.py` for the bit bought.
- It stands 42 mm over the table, so about 52 out of the collet: the
  rectangle's corners have to pass under the stick. The straight bit
  stands as high.

## How stiff, how hard: estimates

Taking hardwood at 50 J/cm³ to cut, pushed at 20 mm/s, at 25,000 rpm,
with E 10 GPa along the grain and G 0.7 GPa:

| | force | what gives |
|---|---|---|
| roughing, 6 mm bit, a 3 mm bite | 2.3 N | the stick twists 14 µm at its edge, the belt and motor give 30 |
| ball-nose in a full slot, 1.5 mm | 1.2 N | 7 and 15 µm |
| finishing, 0.4 mm left | 0.2 N | 1 and 3 µm |

- **The stick is not limp.** Its section has second moments of 56,000
  and 110,000 mm⁴: between centres, with nothing under it, the
  roughing cut bends it under a micrometre. So it is held at its two
  ends and nowhere else. (It was first drawn with two turned journals
  on steadies, for a limpness that was never calculated; a journal's
  second moment is 7,000, so they were its weakest places.)
- **The necks hold.** A lobe on its 4.5 mm neck splits at about 25 N
  over 20 mm of stick, ten times the roughing cut.
- **The plate's weight** on the lift screw is about 25 N, ten times
  any cut.
- The force that matters is the one these numbers leave out: a bit
  that grabs.

## The controller

`firmware/main.py` and `firmware/config.py` are MicroPython for a
Raspberry Pi Pico; `out/passes.csv` is the table. Copy the three to
the Pico. `config.py` has the pins, the steps for a millimetre and a
degree, and the probing's numbers. The same program runs the two
tables that set a built sled up, `out/bands.csv` and `out/probe.csv`,
each copied to the Pico as `passes.csv` in its turn.

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
| home switches | one each pusher and a mark once round the spindle, wired normally closed: a wire that comes off reads as home and stops the motor. Homing gives up if a switch has not spoken in its travel |
| other switches | sled back, and the foot switch |
| supply | 12 V |

It says what it wants in four short lines, on a small display if one
is fitted and always over USB: which row, which bit, and what to do.
Without the display's driver or the display it carries on over USB.

It reads the table a row at a time, so a table's length is no matter
to the Pico's memory.

It keeps its place. A row is done when the sled has left its back
stop and come back; the row's number then goes into `at.txt` on the
Pico. Switched on with a stick part done, it offers to go on from the
next row (hold the foot switch three seconds to start over), homes,
and takes its first move by the safe height. The bit is still set, so
long as the router has not been touched.

| pin (GP) | to |
|---|---|
| 2, 3 | lift driver: step, direction |
| 4, 5 | swing driver: step, direction |
| 6, 7 | spindle driver: step, direction |
| 8 | all three drivers' enable |
| 10, 11, 12 | home switches: lift, swing, the spindle's mark; each to ground, normally closed |
| 13 | sled-back switch, to ground, closing when the sled is back |
| 14 | foot switch, to ground |
| 15 | the probe: a clip lead to the pin or block being touched; a second lead from ground to the spindle |
| 16, 17 | display: SDA, SCL |
| 25 | the Pico's own LED: lit while a row waits to be cut, and while the master touches a block being slid up to it |

## Setting up a built sled

The table is worked out from the sled as drawn. A built sled differs:
half a millimetre in a hinge's place or a link's length, the home
switches wherever they were screwed down, the cutter not quite on the
sled's line. Run from the drawing's table, such a sled cuts 1.6 mm
out. So a built sled is measured before it cuts a tile, in two steps.
The controller runs both from tables `build.py` writes, and
`calibrate.py` reads the results.

Needed: a 6 mm ground pin 60 or 70 long (a dowel pin), a 1-2-3 block,
two clip leads, callipers, and for the master a ground steel sleeve.
A printed master is not round enough and does not conduct.

### 1. Does the stick run true: `out/bands.csv`

The stick's axis has to run parallel to the sled's travel: a far end
0.05 mm high is a tile 0.05 mm bigger at that end. Put a scrap stick
between the centres and clamp a stop to the fence 25 mm in from each
end of it. The table sets the straight bit and brings the stick down
in three bites; at each the sled is pushed to the far stop and drawn
back to the near one, and at each stop the spindle turns the stick
once round over the cutter while the foot switch is held. That leaves
two round bands. Then the same with the bit beside the stick, the
stops moved 10 mm. Four bands, four diameters:

```sh
python3 calibrate.py align UNDER_NEAR UNDER_FAR BESIDE_NEAR BESIDE_FAR
```

says how far to move the tail centre, up or down and across. Under
the stick a band's radius is the axis's height over the bit, beside
it the axis's distance from the bit, so the difference between two
diameters is twice what the axis is out between them. Callipers tell
0.03 mm of that.

### 2. What the sled is: `out/probe.csv`

The controller can feel the master touch metal: a clip lead from GP15
to the thing touched, another from ground to the spindle, and a touch
closes the contact. The router is unplugged and the sled clamped at
its back stop. The master is brought

| onto | where | touches |
|---|---|---|
| a 6 mm pin in the collet, its top about 42 mm up | down on its top, and against each side at four heights; then all of it again with the spindle half a turn round | 18 |
| the block lying flat | down on it at three places across | 3 |
| the block on its long edge | down on it at three places, and against both faces at four heights | 11 |
| the block on end, on the far side of the cutter and then on the fence's | against one face at four heights, to the top of the lift | 8 |

A block that is touched on a face is put in its place by the sled:
the controller holds the master where the face should be and the
block is slid up to it until the Pico's lamp lights. Each place is
touched three times, and three that disagree by 0.08 mm stop the
probing. Stopped part way down a face and switched on again, it goes
back to where that run of touches began. About twenty minutes, most
of it the motors.

```sh
python3 calibrate.py fit probe_log.csv --master 15.98
python3 build.py
```

`fit` finds the sled that all 40 touches fit: thirteen dimensions of
hinges and links, where the two home switches are, the cutter's place
across, the pin's height, the master's runout on the spindle and
where the blocks stood. That is 22 unknowns, each held loosely to the
drawing. It writes `built.json`, and `build.py` then plans every row
for that sled. `--master` is the sleeve's diameter by micrometer.

`calibrate.py study` tries this on 300 sleds built wrong at random:
every dimension up to 0.5 mm out, the home switches 2 mm, the cutter
1 mm, the master running out up to 0.1 mm, a touch repeating to
0.01 mm, the pin and block within 0.005 mm of their sizes. The worst
finishing cut of each sled:

| | median | 95 in 100 | worst of 300 |
|---|---|---|---|
| run from the drawing's table | 1.59 mm | 3.38 | 4.91 |
| probed and planned for | 0.021 mm | 0.037 | 0.052 |

None of the 300 stopped the probing or brought the master onto
anything unawares. One of them is recorded in `out/calibration.json`
as probed by the controller's own program run on a computer, and the
test works it through again with its own hinges: 2.42 mm out as
built, 0.023 mm planned for.

What each set-up is for was found by leaving it out, a hundred sleds
each:

| left out | median | worst |
|---|---|---|
| nothing | 0.021 mm | 0.052 |
| the block on end | 0.045 | 0.13 |
| the block on edge | 0.038 | 0.10 |
| the block lying flat | 0.039 | 0.12 |
| the pin's second half turn | 0.107 | 0.26 |
| two touches of each three | 0.030 | 0.08 |

Nothing but the block on end reaches above 59 mm, and finishing cuts
go to 71. Without the second half turn the master's runout passes
for a cutter out of place.

This is simulation. It takes the table for flat, the block for
square and a touch for clean. A real sled also flexes, and has play
that no fit takes out.

### Play

Play is not fitted out, so it is kept out. The swing screw's link
leans up to 21°, because the frame rises and falls 40 mm past a screw
that stays put, and whatever its nose moves across the screw comes
through to the stick at half of itself: 0.2 mm of rattle in a printed
bore was 0.1 mm at the stick. So the nose is a block on an MGN12H
carriage on a 150 mm rail, which has about a hundredth of a
millimetre of play: 0.005 mm at the stick. The carriage also keeps
the nose from turning with the screw. The lift's link leans only 7°,
but its nose is on a rail too, upright on a post, for the same two
reasons.

Along the screws, slack in a nut comes through whole, a little more
than whole on the swing. The plate's weight keeps the lift's nut on
one flank. The hanging frame's does not: left of plumb it pulls on
the swing screw and right of plumb it pushes, so slack there would
show as a step in the middle of the travel. Both screws take
anti-backlash nuts, and a light spring keeps the frame's load one
way.

## Measure these first

| what | in the model | check |
|---|---|---|
| the cutter over the table | 42.0 mm | the blank's corners clear the table by 2.9 at the lowest; a shorter bit needs a smaller tile or a hole in the table |
| the table | 760 mm along the fence for the sled and its 218 mm of travel, and a stop beyond each end | on a shorter table the sled overhangs the ends and the stops want outriggers |
| the fence | 290 mm right of the cutter | a table whose fence will not go that far wants a bar in its mitre slot under the left runner |
| the lead screws | T8, 2 mm lead | the common T8 is 8 mm lead: a quarter the steps a millimetre, and it runs back under load |
| the shafts, supports and blocks | the 20 mm set from the duplicator: two shafts of 490 and 416, four SK20, four SC20UU | the blocks turn on the shafts as hinges; collars keep them from sliding |
| the hinges and links | hinge 1 at 150 right and 209 up, hinge 2 150 left of it and 26 down, the axis 130 under hinge 2; links 30 and 60 | built within half a millimetre; what they really are is found by probing, see Setting up a built sled |
| the rails | MGN12 with H carriages, both 150 long: level for the swing, its seat 53 over the table; upright on a ply post for the lift. The blocks' bores for the pins and nuts are not drawn, nor the motors' mounts | each rail square to the stick; where it really is goes in `build.py` |
| the pins | not drawn | 5 mm shoulder bolts, a close fit |
| slack | a screw's nut and two pins a link | anti-backlash nuts on both screws; a light spring from the plate to the sled and from the tail hanger to the riser, not drawn |
| the pulleys and belt | 60 and 16 teeth, 52 between centres: a closed belt of 184 | a 60 with a 12 mm bore is not the common one |
| the drive | a 14 mm spur on the spindle's end, the tail centre pushing the stick onto it | not drawn in detail. The tail centre's hole wants a little slack under its nuts: setting up moves it by tenths |
| the master | a ground steel sleeve, 16 round, 12 bore, 29 long | its diameter by micrometer goes to `calibrate.py fit`; its runout on the studding is found by the probing |
| the blank | 49 x 42, square, its centre marked 19.5 from a wide face and 24.8 from a narrow one | on its centres within a millimetre and level within a couple of degrees: the 2 mm over is for that |
| the feed | 20 mm/s at 25,000 rpm is a chip of 0.02 mm a tooth | that is fine dust, and may burn: the router's slowest speed, and a brisk push |
| the frame | not checked | lean on the stick with the motors holding: if the cutter's mark moves, stiffen what gave |

## Parts

| part | size, mm | |
|---|---|---|
| sled | 540 x 480 ply: two runners, a tie at each end, and a shelf beyond the stick's tail for the swing rail | open under the stick; its right edge on the fence |
| riser | 540 x 220 ply, a gusset at one end and 40 mm in from the other | hinge 1's two supports on its face |
| lift plate | 420 x 327 ply | two blocks under its right edge on hinge 1; hinge 2's supports under its middle |
| beam | 350 x 60 ply | two blocks on hinge 2; the hangers under it |
| head hangers, 2 | ply, 70 wide, round-ended | a 6201 bearing in each, 43 apart |
| tail hanger | ply, 70 wide, round-ended | the tail centre, and link 2's pin 25 over it |
| spindle | M12 studding | the 60-tooth pulley outboard, the master and a drive spur inboard |
| master | a ground steel sleeve, 16 round x 29, bought | on the spindle between hanger and spur; the cutter is under it with the sled back, a bit is set on it, and the sled is probed with it. The spur beside it is 14, so a block lying under both touches only the master |
| lift screw | NEMA 17 on the left runner, shaft up; a T8 screw of 2 mm lead with an anti-backlash nut | 86 mm of travel cutting, 98 with the probing's reach |
| lift rail | MGN12 x 150 with an MGN12H carriage, on a ply post 60 x 220 on the left runner | carries the lift's nose block, printed, 27 x 24 x 68: link 1's pin near its top, the nut in its foot |
| swing screw | NEMA 17 on a printed seat on the right runner, 78 over the table; a T8 screw of 2 mm lead with an anti-backlash nut | 32 mm of travel cutting, 51 with the probing's reach |
| swing rail | MGN12 x 150 with an MGN12H carriage, on a printed seat on a shelf of the sled | carries the nose block, which holds link 2's pin and the screw's nut |
| nose block | printed, 27 x 46 x 24 | on the carriage |
| links, 2 | strips, 30 and 60 between pins, and a clevis under the plate | each nose to what it moves |
| spindle motor | NEMA 17 on the head hanger, 52 over the spindle | a closed GT2 belt |
| controller | a box on the right runner | Pico, three drivers |
| stops, 2 | blocks 40 x 40 x 60 | clamped to the fence, 758 apart: the sled's length and its travel |
| sled-back switch | a microswitch on the right runner's end | the back stop closes it |
| blank | 49 x 42 x 200, sawn and planed | between the spur and the tail centre |

To buy, besides ply, screws and the 20 mm shafts, supports and blocks
the duplicator already has:

| what | how many | |
|---|---|---|
| Raspberry Pi Pico | 1 | MicroPython on it |
| TMC2209 driver modules | 3 | step and direction |
| NEMA 17 motors, 200 steps | 3 | |
| 12 V supply, 3 A | 1 | |
| SSD1306 display, 128 x 64, I2C | 1 | optional |
| microswitches | 4 | lift home, swing home, the spindle's mark, sled back; the three home switches wired normally closed |
| foot switch | 1 | |
| T8 lead screw, 2 mm lead, 150 long, with an anti-backlash nut | 2 | and a 5 to 8 mm coupling each |
| MGN12 rail, 150 long, with an MGN12H carriage | 2 | |
| GT2 pulleys, 6 mm belt: 60 teeth bored 12, 16 teeth bored 5 | 1 each | and a closed belt of 184 |
| 6201 bearings | 2 | |
| M12 studding, nuts and washers | 350 mm | the spindle; a pointed length of it is the tail centre |
| ground steel sleeve, 16 x 12 x 29 | 1 | the master |
| 5 mm shoulder bolts | 4 | the links' pins |
| light springs | 2 | |
| tapered ball-nose, 1.5 mm radius, 2.8° a side, 6 mm shank | 1 | reaching 42 mm over the table |
| straight bit, 6 mm | 1 | the same reach |
| 6 mm ground pin, 60 or 70 long | 1 | setting up |
| 1-2-3 block | 1 | setting up |
| clip leads | 2 | setting up |

## What the test checks

`crates/ok-render/tests/stick_sled.rs` has the hinges and links
written a second time, in Rust, and works every row forward through
them:

- the cutter is where the plan wants it to a fiftieth of a millimetre;
  the finishing bit, its flank and shank too, is never in the tile,
  and a roughing bite never within its allowance;
- the last bites of the finishing passes together reach the whole
  outline, from nearly as many directions as passes;
- the waste, on a grid of the test's own, with the table's cutting
  rows run over it in the table's order: no row meets more than a
  hair-wide fin standing higher than its bit's bite;
- at every row the blank clears the table, so does the pulley, and
  each link is its length from pin to pin and leans no more than 30°
  off its screw;
- the controller's program, simulated, ends every row on the table's
  steps, turns the spindle under four turns in all coming up to its
  angles, goes straight exactly where the table says, and otherwise
  never swings the stick below the height at which the master clears
  the cutter;
- every move marked straight, followed along its own path with the
  test's own geometry: the master clear of the cutter, the blank clear
  of the table at every angle it turns through;
- the document is the sled at one finishing row, the cutter's tip on
  the stick and no part through another;
- the blank is a rectangle of whole millimetres with the tile 2 mm
  inside every face, and the first row hangs it with a wide face
  level and underneath, the centre where the row says it is;
- against the back stop the cutter is under the master with the whole
  of the widest bit clear of hanger and spur, against the forward one
  past the stick's end, and the switch is where the back stop meets
  the sled;
- the probing table, on the sled as drawn: every touch is a touch, on
  a top or a side and not a corner, with a window of its screw to
  spare each way, and on its way there the master passes the pin by
  6 mm and a block by 4;
- the recorded sled built wrong: its 40 touches are touches on it, the
  fitted sled leaves them 5 µm out where the drawn one is 1.7 mm
  from them, and the table planned for the fitted sled, run on the
  real one with the bit set as the table sets it, lands every
  finishing cut within 0.03 mm;
- the bands are turned with the cutter straight under the axis and
  with the bit's side level with it, in bites of 4 mm, and a turn of
  the spindle is a whole number of steps.

The build itself tries the sled at 356 poses, the rows, the ways
between them and every place the probing takes it, with every part as
its outline along the stick, and stops if any is in another or in the
cutter's lane.
