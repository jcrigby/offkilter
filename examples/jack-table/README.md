# A minimal router table with a scissor-jack lift

Preliminary. A plywood box (top, two sides, a back, and a front across
half of it) with the old Makita 3612BR plunge router screwed under the
top. A $23 Harbor Freight scissor jack (Haul-Master 1-1/2 ton) is bolted
upside down under the top beside the router. It lifts the router
through a lever:

1. Turning the crank drives the jack's saddle **down** onto one end of a
   hardwood lever.
2. The lever turns on a bolt through a hanger under the top, so its
   other end comes **up**.
3. That end pushes up on a **drop arm** screwed to the router's left
   handle boss in place of the handle. This plunges the motor and
   raises the bit.

The crank is a 6 in socket extension that comes out through a slot in
the front. Turn it with a ratchet or a cordless drill.

```sh
cargo build --release -p ok-mcp
python3 examples/jack-table/build.py         # the document, pictures, sheets, parts list and poses
cargo test -p ok-render --test jack_table    # what CI runs
```

![The table](out/iso.png)

## Why it is laid out this way

- **The jack hangs from the top, beside the router.** A jack under the
  router would add its own height (3-3/8 in at least) to the table. Here
  the table is 13-3/4 in tall, set by the router: the motor's end is
  1-3/4 in off the bench when it is retracted.
- **The lever is 2 to 1.** The pin under the saddle is 220 mm from the
  pivot and the pin under the drop arm is 110 mm. The jack therefore
  moves 130 mm for the router's whole 65 mm of plunge, and each turn
  moves the bit half as far as it moves the saddle.
- **The jack works in its middle.** A scissor jack is fast and coarse
  when nearly closed and slow when open. Its rise per turn is the
  screw's pitch times the cotangent of the arms' angle. This table
  runs the jack from 5.5 to 10.6 in (its range is 3-3/8 to 15-1/8), so
  the arms stay between 14 and 38 degrees.
- **Nothing is linked.** Each end of the lever has a 3/8 bolt across
  it:
  - At the router end, the drop arm's forked foot straddles the lever
    and its two tines sit on that bolt's ends.
  - At the jack end, the saddle's groove straddles the lever and its
    lips sit on that bolt's ends.

  The router's weight (5.5 kg) and its plunge springs push the motor
  down. That holds the drop arm on the lever, and the lever's other end
  up against the saddle. Both contacts are flat on round, so the pins
  can slide a few millimetres as the lever turns (5 at the router, 10
  at the jack). The fork and the groove keep the lever in line.
- **The crank moves.** The screw sits at the scissor's knuckles, halfway
  between the foot and the saddle, so it drops half as far as the
  saddle: 65 mm over the travel. It also moves 34 mm back as the
  knuckles close. The front's slot is vertical and 65 mm long, plus the
  extension's width, and the extension simply slides with the screw.
- **The front stops at the hanger.** The router's half of the box is
  open at the front for the switch, the plunge lock, the cord, and your
  hands at bit changes.

| Retracted | Fully plunged |
|---|---|
| ![Through the lever, retracted](out/section_retracted.png) | ![Through the lever, plunged](out/section_plunged.png) |
| ![Through the jack, retracted](out/jack_retracted.png) | ![Through the jack, plunged](out/jack_plunged.png) |

## What the numbers give

These are from the build's printout and the test:

| plunge | bit over the table | jack (foot to saddle) | arms | bit per crank turn |
|---|---|---|---|---|
| 0 | −24 mm | 140 mm (5.51 in) | 13.6° | 4.1 mm |
| 16 | −8 | 172 (6.79) | 19.3° | 2.9 |
| 32.5 | 8 | 205 (8.07) | 25.2° | 2.1 |
| 49 | 25 | 238 (9.35) | 31.5° | 1.6 |
| 65 | 41 | 270 (10.63) | 38.1° | 1.3 |

- Going from retracted to fully plunged takes about 32 turns, with a
  2 mm screw pitch guessed.
- Where bits are usually set (around the table's surface), one turn
  moves the bit about 2 mm. A quarter turn of a ratchet is half a
  millimetre.
- The router's own plunge lock holds the height. Crank to height, then
  lock it; the lever only has to hold while you set it.

## The parts

`out/parts.csv` lists the wood and the hardware. In short:

- **Box:** 3/4 ply, glued and screwed.
  - **Top:** 28 x 24. Its 2 in hole over the bit is 7 in from the right
    edge.
  - **Sides:** 24 x 13.
  - **Back:** 26-1/2 x 13, between the sides.
  - **Front:** 10-1/4 x 13, across the jack's bay only, with the crank
    slot in it.
- **Router:** take off the clear sub-base and screw the base under the
  top through its own sub-base holes, with longer flat-head screws
  countersunk from above. The bit comes up through the 2 in hole; the
  collet nut's top reaches 9 mm under the surface at full plunge.
- **Jack:** upside down, its foot bolted under the top with two 5/16
  flat-head bolts through holes drilled in the foot's flat. On the eye,
  fit a scissor-jack drill adapter (a socket with a slot and a cross pin
  for the eye; any auto parts store has one), and on the adapter a 3/8
  drive 6 in extension.
- **Hanger:** two 3/4 ply cheeks, 2-1/2 wide, with a hardwood spacer
  between them at the top, screwed down through the top. The pivot is
  a 3/8 bolt with a washer each side of the lever, about 8-1/4 in below the
  top.
- **Lever:** hardwood, 3/4 x 1-1/2 x 14-3/4. It has three 3/8 holes on
  its centre line:
  - the pivot;
  - 110 mm one way, the router pin;
  - 220 mm the other way, the jack pin.

  Each pin is a 3/8 x 2 bolt with a nut.
- **Drop arm:** hardwood, 1-1/2 square x 6. It replaces the left handle
  on the handle's own screw (or one 3/4 in longer). Its foot has a 7/8
  wide, 1-1/2 deep slot up it for the lever.

`out/jack_table.pdf` is the A3 sheet. `out/lift.pdf` (and `lift.png`)
shows the table from the front at retracted, half plunged and fully
plunged. The front panel hides the jack there; the sections above show
it. `out/jack_table.okpart` is the document. It has:

- one part studio per part;
- the router motor, bit and drop arm as a sub-assembly on a slider
  (`plunge`) along a post;
- the lever on a revolute (`lever`) about the pivot bolt;
- the jack's arms, saddle, screw and nut trunnion on revolutes about
  their pins, chained from the foot.

`out/poses.json` holds every mate's value at five plunge positions. The
script computes them from the lever and the scissor geometry, and the
test sets them and checks that the parts agree: the rear arms meet,
the screw stays level, and the lever touches both feet without passing
into them.

## What is measured and what is guessed

Every dimension of the router and the jack is a named constant at the
top of `build.py`, each marked S (the maker's specification), M
(measured) or G (guessed from the photos). Replace a guess with a
reading, rebuild, and the test says whether it still works. The
guesses that matter:

| | value | from | what it moves |
|---|---|---|---|
| plunge | 65 mm | S: the manual | the jack's travel, at 2 to 1 |
| base face to motor end | 287 mm | S: overall height | the table's height |
| across the handles | 276 mm | S | where the drop arm goes |
| handle boss face from the axis | 98 mm | G | the lever's position |
| handle screw below the base face, retracted | 105 mm | G | the drop arm's length |
| handle screw thread | ? | | the drop arm's screw |
| motor diameter, posts | 100 mm, 20 mm at ±64 | G | clearances only |
| jack arm, pin to pin | 170 mm | G: from 17 in long, 15-1/8 high | turns per mm, the slot |
| foot and saddle pins from their faces | 30 mm | G | the jack's heights |
| jack screw pitch | 2 mm | G | turns per mm only |
| saddle groove across the screw, 22 wide x 25 deep | | G | whether the lever fits between the lips |

To measure on the router, have it retracted (posts fully out) and the
base on a flat surface:

1. The height of the handle screw's centre above the base's face.
2. The distance across from one handle boss's face to the other.
3. The plunge from fully out to fully in.

On the jack:

1. The distance between the arm pins.
2. The number of crank turns to go from 5-1/2 in to 10-5/8 in.
3. Which way the saddle's groove runs, and how wide and deep it is.

## Open

- [ ] **The saddle.** If the groove runs along the screw rather than
      across it, or it is narrower than 7/8 in, bolt a hardwood shoe with
      a 7/8 slot across it under the saddle. Two hose clamps round the
      saddle's lips will hold it.
- [ ] **One-sided push.** The drop arm pushes on one handle boss, beside
      one post. If the motor cocks and binds on the posts, fork the
      lever's router end into a yoke under both bosses. That needs the
      right handle off too.
- [ ] **A depth stop for repeat setups:** the router's own turret stop
      still works upside down.
