# A stick sled

The leaning half-ellipse Spectre milled round one solid stick. The
stick is held between a printed socket on a spindle and the point of
a second, idle spindle, on a flat sled that slides on the base
between two fences, and it only turns. Beside it, fixed to the base,
lies the [two-axis router positioner](../router-positioner/) with its
bit pointing at the stick: the positioner's X sets how deep the bit
reaches, its Z how high. Three motors go to the next row of a table,
and the sled is pushed past the bit and drawn back by hand.

It replaces the [hinged sled](../stick-sled-hinged/), which kept the
cutter fixed in a router table and hung the stick from two hinges.
Two straight screws now do what the hinges and their links did.

```sh
cargo build --release -p ok-mcp
python3 examples/router-positioner/build.py   # this reads the positioner's sizes from its out/positioner.json
python3 examples/stick-sled/build.py
cargo test -p ok-render --test stick_sled   # what CI runs
```

![The sled between its two fences: the tail spindle at the near end where the bit waits, the stick, its far end in the socket, the headstock, and the wheel and motor hanging off the far end; the positioner standing beside it on its lid with its column out behind](out/sled.png)

Nothing here has been built or run on a motor, and the positioner's
sizes rest on a router nobody has measured yet. What is checked is
checked by calculation and by the test.

## How it works

![Cut across the middle of the stick, looking along it: the positioner's box on the base, the bit reaching out of its slot to the tile's section, the sled's plate on two slips between the fences with a lip over each of its edges](out/end.png)

- **The stick turns and does nothing else.** Its far end sits in a
  printed socket on a spindle; its near end is on the point of a
  second spindle that turns with it. A motor turns the first through
  a belt, 20 teeth to a wheel of 160. Every joint of that chain is in
  [The stick's drive](#the-sticks-drive-joint-by-joint).
- **The positioner places the bit.** It lies on its lid with its Z
  column standing up, so the bit's axis is a stick's height over the
  base. Its face is a wall 54 mm from the stick's axis; the bit comes
  out of the slot in it, and for the deeper cuts the collet nut too.
- **Depth** is the positioner's X: the quill run out along the bit.
  **Height** is its Z. A cut is a direction round the stick, a height
  of the bit over the stick's axis, and a depth. Each is one motor,
  and none of the three moves any other.
- **The sled is a flat plate between two fences.** It slides on the
  base with its far edge along one fence, and each fence has a lip
  over the sled's edge so that it cannot lift. What holds the stick
  is screwed to it. More in [The slide](#the-slide).
- **The wheel is past the sled's far end.** At 105 mm across it
  comes within 2 mm of the positioner's face, so it sits beyond the
  positioner's box. At 2.7 times the radius of the
  60-tooth pulley it replaces, the same runout turns the stick 2.7
  times less far wrong, and by my estimate the belt gives about a
  fifth as much. The ratio is eight to one.

## A pass

1. Draw the sled back to its stop. The bit is beside the master: the
   tail spindle's rod, bare between the tailstock and the stick.
2. Press the foot switch. The motors go to the next row and stop.
3. Push the sled to its forward stop and draw it back. The bit comes
   onto the stick from its near end and stops 21 mm short of its far
   end, with the collet nut 2 mm short of the socket.

The controller will not move a motor unless the sled is back: a
switch on the back stop. With the sled back the bit is beside the
master, so a move between passes must not carry it through the
master. The safe way is to draw the bit back until its tip is 2 mm
clear of the master, then up or down and round, then out. Most moves
do not need it: the build follows each one along its own path and
marks it straight if the whole bit passes the master by 1.5 mm. 430
of the 485 are.

## The slide

The sled is one piece of 19 mm ply, 424 x 219, and everything that
holds the stick is screwed to it. It slides on the base between two
fences. There are no shafts and no bearing blocks under it.

- **The far fence is the datum.** The sled's far edge runs along it.
  Across the sled is the way the bit reaches, so whatever bow that
  fence or that edge has is cut into the stick, one for one. Make the
  pair as straight as you can: a metal bar or an aluminium extrusion
  for the fence, a factory edge for the sled.
- **Two spring plungers in the sled's near edge** push off the near
  fence and keep the far edge on the datum. So there is no fit to get
  right between the fences: the near one stands 1 mm off the sled. A
  cut pushes the stick away from the bit, which is the same way.
- **Each fence has a lip over the sled's edge,** 0.3 mm clear of its
  top: 10 mm over at the datum, 7 at the near fence. They stop the
  sled lifting or tipping. They are not what holds it down: its
  weight and the hand on it do that, and by my estimate a roughing
  cut tips it with about two thirds of what its weight resists.
- **It rides on two slippery strips** on the base, one under each
  edge, 3 mm thick. A chip on a strip lifts the sled, and the sled's
  height is the bit's height on the stick: brush them, or fix a felt
  wiper to each end of the sled.
- **The near fence stands against the positioner's face.** It is
  14 mm wide, which is all the room there is, and the uprights are
  50 mm wide to stay inside its lip.
- **A stop block** on the base meets the sled at each end of its
  188 mm; the back one carries the sled-back switch.

With nothing under it the sled is low: the positioner lies on the
base itself, the stick's axis is 89.5 mm over the sled, and the
uprights are 130 mm tall. How straight the datum fence is and how
flat the base is are not in the study under Setting up: the ground
shafts this replaces were taken to be true, and a fence is as true as
it is made.

## The stick's drive, joint by joint

| the drive end, cut along the stick's axis | the tail end, where the bit waits |
|---|---|
| ![From the left: the stick's uncut end in the socket's pocket with room behind it, the socket's boss with the rod in it, a flange bearing and its upright twice, the home sensor with the wheel's fin in it, the wheel and its hub](out/drive.png) | ![From the left: the tail spindle's rod through two uprights with a flange bearing on the stick's side of each, bare for 21 mm, and its end at the stick's end](out/tail.png) |

From the motor to the far end of the stick, what each part is joined
to the next by:

| from | to | joined by |
|---|---|---|
| motor, a NEMA 17 on a bracket on the plate | its pulley, 20 teeth | the pulley's two set screws, one on the shaft's flat |
| pulley | wheel, 160 teeth, printed | a closed GT2 belt of 400 mm, 7.1 of the pulley's teeth in it. The bracket's slots tension it |
| wheel | hub | four M4 bolts through slack holes into the flange of a bought 12 mm hub on the wheel's outer face. The wheel's own bore is 13 and does not touch the rod |
| hub | spindle, a 12 mm ground rod | the hub's set screws, on a flat ground on the rod |
| spindle | headstock | two KFL001 flange bearing units, each bolted through its upright on the stick's side, long way up. Their set screws hold the rod endways. The uprights are drilled 20 mm for the rod to pass: no plywood is bored to a fit |
| spindle | socket, printed | the rod's end, roughened, epoxied 22 mm into the socket's boss |
| socket | stick | a pocket the blank's end stands 12 mm into, 0.3 mm slack all round, with two screws: one through the floor that jacks the blank toward the tail, one through a wall that locks it against the wall opposite |
| stick | tail spindle | a point of three flats ground on the end of a second length of the rod |
| tail spindle | tailstock | two more KFL001, the same way. The rod is bare for 21 mm between the inner one and the stick, and that is the master a bit is set against |
| spindle | controller | a fin on the wheel's inner face, 34 mm from the axis, through an optical endstop on the outer upright once a turn |

The socket is the owner's idea, and it does three things a spur did
not: it cannot slip, it puts the blank on the axis without a centre
mark at that end, and it says which way round the blank is. What it
costs is the stick's far end. A socket big enough for the blank
cannot be where the bit comes onto the stick, so the bit waits at the
tail and each pass stops before the socket: 21 mm of every stick is
not cut.

To buy: four KFL001, a 12 mm flanged hub, a 20-tooth GT2 pulley of
5 mm bore, a 400 mm closed GT2 belt, an optical endstop, 12 mm ground
rod for a 147 mm length and a 100, two M5 set screws and threaded
inserts for the socket, two M6 spring plungers and inserts for the
sled, epoxy. The sizes in `build.py` are the
catalogue's: measure the ones that come before drilling for them. To
make: the printed wheel and socket, a flat on the rod for the hub,
and three flats for the tail's point.

- **Gluing the socket on.** Which way round it is on the rod is which
  way round the blank is to the controller, so glue it with the
  spindle where the table's first row puts it: run that row, turn the
  socket until a wide wall of its pocket lies flat on a block stood
  on the plate, and leave it there while the epoxy sets.
- **Putting a blank on.** Mark the tail end's centre, 19.5 mm from one
  wide face and 24.8 from one narrow face. Push the other end into
  the socket as far as it goes, bring the mark to the tail spindle's
  point, turn the jack screw until the point is 2 or 3 mm into the
  wood, then tighten the lock screw. Before the jack screw, turn the
  tail spindle so the line scribed on it is uppermost: the master's
  runout was measured that way round.
- **The tail spindle turns with the stick,** which is why its point
  has flats. It is the master, and the half turn that finds its
  runout is the motor's, through the stick.
- **Truing the wheel.** Snug its four bolts, hold a dial indicator on
  the wheel where the belt runs, turn the spindle, and tap the wheel
  until the needle moves 0.04 mm or less. Then tighten. That 0.04 is
  0.02 off centre, and what it buys is in Setting up.
- **Lining the stick up with the sled's travel** (`out/bands.csv`) is
  moving the tail spindle's two bearings on their uprights: their
  bolt holes are slack.
- **The home fin** is read 34 mm from the axis, so 0.02 mm there is
  0.017 mm at the tile's edge. It matters only when a stick is taken
  up again after the controller has been off. The sensor says home
  over the fin's whole 10°, so an axis found already at home is first
  run away until the sensor lets go, and 5° more, and then brought
  back: home is the same edge from the same side every time. The
  build runs the controller's homing from nine places to check it.

## The table: 485 rows a stick

| rows | what | bit |
|---|---|---|
| 1 | put the blank on | |
| 1 | set the straight bit | 1/4 in straight |
| 185 | roughing: 83 places from 24 directions, in bites of 3 mm | |
| 1 | set the ball-nose | 3 mm ball-nose, tapered to a 1/4 in shank |
| 297 | finishing: 244 passes, the deep ones in bites of 1.5 mm | |

Both bits have 1/4 in shanks: the router is the North American Colt,
and that is its collet.

The cuts are the hinged sled's, planned by the same `plan.py`: every
finishing pass from its own direction, the whole of a tapered bit
kept out of the tile, each pass meeting its face 15° round the ball
from its dead centre, roughing to 0.4 mm of the tile, the order
settled before the bites are counted. Its README has the reasons.
What changed is where the three numbers of a row come from.

- **The blank is a plain rectangle,** 49 x 42 mm, and 200 long of
  which 179 are cut. Its far end goes in the socket; the centre is
  marked on its near end only, 19.5 mm from one wide face and 24.8
  from one narrow face. The first row turns the spindle to where the
  socket holds it with that wide face level and underneath and that
  narrow face toward the router.
- **A bit is set by touch, by the controller.** The probe's clip goes
  on the bit with the router off; the bit is run out at the master,
  level with the stick's axis, until it touches, three times; from
  then on every depth is taken from there. So a bit may stand 2 mm
  further out of the collet than the table takes it to, or less far,
  and it makes no difference.
- **The clip has a home.** It is a post wired to ground: parked there
  the probe reads closed, and the controller will not go to a cut
  until it does. The router is never started with the clip on its
  bit.
- **Every move ends going the same way,** away from home, as the
  positioner's brief asks: a move back goes half a millimetre past
  and comes up to its place.

A stick is about two and a half hours: 500 pushes at a quarter of a
minute, and under half an hour of the motors (3.3 m of the depth's
screw and 4.7 m of the height's, at 1280 steps a millimetre).
Estimated.

## What the positioner has to be, for this job

| | here | |
|---|---|---|
| X travel | 12.6 to 58.0 mm used of 60 | all of it: the deepest cut is 2 mm short of the end |
| Z travel | 21.0 to 59.0 mm used of 80 | 60 would do |
| how it lies | on its lid, on the base | the stage's weight then pulls away from its column, onto the collar that has no thrust bearing yet |
| its face from the stick's axis | 54 mm | the Colt's collet nut is 34 mm in front of the quill, so at the deepest cut it is 20.5 out of the face, and the uncut blank's corners swing 33.5. The nut sets the distance, not the face |
| the bit out of the collet nut | 45.5 mm | the blank's swing, and 12 mm past the stick's axis. The router does not change it: the positioner stands back by what the nut stands out. A 3 in tapered ball-nose on a 1/4 in shank does; the 1/4 in straight bit must be as long, 3 in overall |
| the depth's screw | M8 rod, as the positioner draws | a rod's pitch is nobody's promise, but it matters little here: 0.2 % off costs under a hundredth of a millimetre in the study |
| its two travels, square | not known until it is built | 2 mm in a metre out is 0.07 mm on the tile: the largest thing left. See Setting up |

The collet nut never comes within the blank's swing: at the deepest
cut its face is 33.5 mm from the axis.

## The controller

`firmware/main.py` and `firmware/config.py` are MicroPython for a
Raspberry Pi Pico; `out/passes.csv` is the table. It is the hinged
sled's program with its two screws renamed and two things added: a
bit set by touch, and every move ending the same way. It reads the
table a row at a time, says what it wants on a small display and over
USB, and keeps its place: taken up part way through a stick, it homes
and sets the bit again before it cuts.

| | |
|---|---|
| controller | Raspberry Pi Pico |
| drivers | three TMC2209 modules; 8 microsteps for the screws, 16 for the spindle |
| a screw's step | 0.00078 mm (M8 x 1.25) |
| the spindle's step | 0.014° |
| home switches | the quill drawn back and the stage at its home end (down, as it lies here), wired normally closed; and an optical endstop that the wheel's fin passes once a turn, which has to read high with the fin in it, as an opened switch does |

| pin (GP) | to |
|---|---|
| 2, 3 | depth driver: step, direction |
| 4, 5 | height driver: step, direction |
| 6, 7 | spindle driver: step, direction |
| 8 | all three drivers' enable |
| 10, 11, 12 | home switches: depth, height, the spindle's mark |
| 13 | sled-back switch, on the back stop |
| 14 | foot switch |
| 15 | the probe's lead and clip; the spindle and the clip's parking post to ground |
| 16, 17 | display: SDA, SCL |

Only the spindle's motor and its home switch ride on the sled; the
rest of the wiring stays still.

## Setting up a built machine

Two straight screws leave little to find, and none of it needs a fit.
The controller runs both steps from tables `build.py` writes;
`calibrate.py` reads the results.

**Depth needs nothing.** Setting a bit by touch measures it every time
one goes in.

### 1. Where the bit's height is: `out/probe.csv`

A ground 1/4 in pin goes in the collet in place of a bit, with the probe's
clip on it, and a scrap blank on the sled, put on as any blank is, so
that the motor turns the tail spindle. The controller brings the pin
against the master from the front, from below and from above, and
again with the spindle half a turn round: six places, three touches
each, a few minutes.

```sh
python3 calibrate.py fit probe_log.csv --master 12.00 --pin 6.35
python3 build.py
```

- The middle of where the pin met the master from below and from
  above is the height at which the bit's axis is level with the
  stick's.
- Those two are the master and the pin apart, whatever the screw
  says, so they give the height screw's true travel.
- What half a turn of the spindle changes is the master's runout,
  and the table's setting rows allow for it.

`calibrate.py study` tries it on 300 machines built wrong at random:
the height's home anywhere in 3 mm, the height's screw 0.3 % off its
pitch and the depth's 0.2 %, the two travels 2 mm in a metre out of
square, the master 0.05 mm off the spindle's centre, a touch
repeating to 0.01 mm. The worst finishing cut of each:

| | median | 95 in 100 | worst of 300 |
|---|---|---|---|
| from the drawing's table | 1.62 mm | 2.99 | 3.22 |
| after the six touches, the spindle turning true | 0.045 mm | 0.072 | 0.083 |
| the same, were the depth's screw true | 0.040 | 0.068 | 0.077 |
| and the two travels square as well | 0.015 | 0.028 | 0.038 |
| after the six touches, the wheel trued to 0.02 off centre and the pulley 0.03 | 0.051 | 0.081 | 0.089 |
| the same, the wheel left 0.1 off as printed | 0.066 | 0.104 | 0.132 |

One machine is recorded in `out/calibration.json` as probed by the
controller's own program on a computer: 0.80 mm out from the drawing's
table, 0.039 planned for. The test works it again.

**The touches cannot find how far the spindle has turned.** The
master is round. A wheel off its turning centre lets the belt in and
out once a turn, and the stick is that much over the wheel's 50.9 mm
pitch radius from where the motor sent it: 0.01 mm off centre is
0.006 mm at the tile's edge. The motor's pulley does the same once a
turn of its own, and the ratio brings it to the same figure. So the
last two rows: with the wheel trued under an indicator the turning
costs 0.006 mm on the median, and left as printed it costs 0.02,
more than anything else on the list but the travels' squareness.

**What the touches do not find is whether the two travels are
square**, and that is most of what is left. The pin is level with
the stick's axis at the depth the touches were made, 50 mm out; a
bit run in or out from there rises or falls with whatever the quill's
travel is out of square with the stage's. Two millimetres in a metre
is 0.07 mm over the cuts' 45 mm of depth. It can be measured, with
the ball-nose's tip on the top and the side of a 1-2-3 block stood on
the sled's plate; that is not written yet. Until it is, it is the
positioner's to be built square, and its builder's to check with a
square.

### 2. Whether the stick runs true: `out/bands.csv`

The stick's axis has to run parallel to the sled's travel. A scrap
stick goes on with a stop clamped 25 mm in from each
end. The table sets the straight bit and, in three bites, has the
sled pushed to the far stop and drawn back to the near one, the
spindle turning the stick once round at each while the foot switch is
held: first with the bit's end at the stick from the front, then with
its side over the stick. Four bands, four diameters:

```sh
python3 calibrate.py align FRONT_NEAR FRONT_FAR TOP_NEAR TOP_FAR
```

says how far to move the tail spindle, toward or away from the router
and up or down. All of it is simulation.

## Measure these first

| what | in the model | check |
|---|---|---|
| the router | its body is 70.71 mm, measured; the rest is read off two photographs, with allowances where it matters | see the positioner's own table |
| the probe's pin | 6.35 mm, a 1/4 in dowel pin or drill rod | measure it and give `fit` what it is with `--pin` |
| the datum fence, and the sled's edge on it | straight | lay a straightedge along each: their bow is cut into the stick, one for one |
| the base under the two slips | flat along the travel | the sled's height is the bit's height on the stick, so a hump is cut in too |
| the bit's length | 45.5 mm out of the collet nut, give or take 2 | the controller measures what it is, and stops if a row is out of the quill's reach |
| the wheel | 160 teeth, printed; a closed belt of 400; the motor 100.1 from the spindle, in slots | it has to run true: 0.1 mm off centre is 0.11° once a turn, 0.06 mm at the tile's edge. True it on its hub under an indicator |
| the two spindles | 12 mm ground rod, 147 long for the drive and 100 for the tail | linear shaft or drill rod. Nothing on it is threaded. On a hardened shaft the flats are ground, not filed |
| the bought parts | KFL001 16 thick, 38 x 63, bolts 48 apart; the hub's flange 36 across, bolts on a 28 circle | catalogue sizes, and makers differ: measure before drilling, and before printing the wheel's bolt holes |
| the blank's end in the socket | 49 x 42 with 0.3 mm all round | saw and plane the blanks to one size and print the pocket to that; the lock screw takes up what is left |

## Parts

| part | size, mm | |
|---|---|---|
| base | 722 x 433 ply | the positioner lies on it, its column overhanging behind |
| datum fence | 692 long, 30 wide, 22 high, and a lip 6 thick reaching 10 over the sled | as straight as can be had: metal for choice |
| near fence | 692 long, 14 wide, 22 high, and a lip 6 thick reaching 7 over the sled | against the positioner's face |
| slips, 2 | 612 x 30 x 3 | something slippery: UHMW tape on a strip, or HDPE |
| sled plate | 424 x 219 ply | between the fences; two M6 spring plungers in its near edge |
| tailstock, headstock | each two uprights 50 wide, 19 thick, 130 high, tied at the foot | a KFL001 flange bearing on the stick's side of each upright |
| tail spindle | 12 mm ground rod, 100 long | a three-flat point on its end; 21 mm bare as the master |
| spindle | 12 mm ground rod, 147 long | its end in the socket's boss; the wheel and hub outboard |
| blank socket | printed, 58 x 51 x 26 with a boss 26 across and 22 long | a pocket 20 deep for the blank's end; a jack screw in its floor, a lock screw in a wall |
| wheel | 160 teeth, 105 across, 12 wide, bore 13, a fin on its inner face | on a 12 mm flanged hub; a 20-tooth pulley on a NEMA 17 on a bracket on the plate |
| home sensor | an optical endstop | on the outer upright's outer face, over the axis |
| stops, 2 | blocks 40 x 40 x 22 on the base, between the fences | the back one carries the sled-back switch |
| router positioner | 440 x 176 x 453 | its own example |

## What the test checks

`crates/ok-render/tests/stick_sled.rs` has the positioner's two axes
written a second time, in Rust, and works every row forward:

- the bit is where the plan wants it; the finishing bit, its flank and
  shank too, is never in the tile, and a roughing bite never within
  its allowance;
- the last bites of the finishing passes together reach the whole
  outline, from 179 directions;
- the waste, on a grid of the test's own, with the table's rows run
  over it in the table's order: no row meets more than a hair-wide
  fin standing higher than its bit's bite;
- the blank turns clear of the positioner's face and 10 mm clear of
  the sled's plate; the uprights pass that face; the wheel is clear
  of the positioner's box with the sled back; every row is within the
  positioner's travel with the bit's margin; the collet nut, which
  comes 20.5 mm out of that face, never comes within the blank's
  swing, and the face stands no further off than that takes;
- every move marked straight, followed along the controller's own
  path, passes the master by 1.5 mm, and drawn back to the safe depth
  the bit is clear of it at any height;
- the controller's program, simulated, ends every row on the table's
  steps, never changes height with the bit further out than the safe
  depth except where the table says, and taken up half way sets the
  bit again before it cuts;
- the blank is a rectangle with the tile 2 mm inside every face, and
  the first row hangs it with a wide face level and underneath;
- the document is the sled at one finishing row, the bit's tip on the
  stick and none of its 36 bodies through another; and with the sled
  at each end of its travel nothing that rides on it is through
  anything that stands on the base. Lifted a millimetre there the
  plate is in a lip, and pushed a millimetre across it is in a fence;
- the stick's drive, from the motor to the tail spindle: seven of the
  pulley's teeth in the belt and the controller's steps for a turn
  the two pulleys' ratio; the hub on the wheel's outer face; each
  flange bearing against the stick's side of its upright; the rod its
  whole length, its end in the socket's boss; the socket's pocket
  round the blank's end with its fit, holding it across, and its
  corners clear of the plate and the positioner as it turns; the tail
  spindle's end at the stick's end, bare where the bit is set; the
  sensor short of the wheel at the fin's radius;
- the slide: the plate's far edge on the datum fence, a spring's gap
  at the near one with the plungers' noses in it, a lip over each
  edge a hair clear of its top, two slips under it, the fences
  reaching past both ends of its travel and the near one against the
  positioner's face; and the stops leave the bit beside the master at
  one end and its collet nut 2 mm short of the socket at the other;
- for setting up: a recorded machine built wrong, its six touches
  touches on it, the arithmetic made again, the height found within
  2 µm at the depth it was probed, and the table planned for it run
  on the real machine with the bit set by touch; what a wheel off
  centre costs, worked from the furthest cut and compared with the
  study's; and the probing table and the bands are where the reading
  of them assumes.
