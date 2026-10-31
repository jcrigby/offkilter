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
3. That end pulls up a short **link** hung from a tab on a **printed
   replacement for the router's left handle**. This plunges the motor
   and raises the bit.

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
  router would add its own height (3-3/4 in at least) to the table. Here
  the table is 13-3/4 in tall, set by the router: the motor's end is
  1-3/4 in off the bench when it is retracted.
- **The lever is 2 to 1.** The pin under the saddle is 220 mm from the
  pivot and the link's bolt is 110 mm. The jack therefore
  moves 100 mm for the router's 50 mm of plunge, and each turn
  moves the bit half as far as it moves the saddle.
- **The jack works in its middle.** A scissor jack is fast and coarse
  when nearly closed and slow when open. Its rise per turn is the
  screw's pitch times the cotangent of the arms' angle. This table
  runs the jack from 7.1 to 11.0 in (its range is 3-3/4 to 15-1/8), so
  the arms stay between 17 and 38 degrees.
- **The router end is linked; the jack end is not.**
  - **Router end.** The printed handle has a tab below it, and a 3/8
    bolt in the tab carries a 107 mm printed link. The link's other end
    is on a 3/8 bolt through the lever. Both bolts turn freely, so the
    router is held both ways and can't bounce off the lever.
  - **Jack end.** The saddle's groove straddles the lever, and its lips
    sit on the ends of a 3/8 bolt across it. The router's weight
    (5.5 kg) and its plunge springs hold the lever up against the
    saddle, and the groove keeps the lever in line.
- **Straight paths, round lever.** The router and the saddle move
  straight up and down, but the lever's pins swing on arcs about the
  pivot, so each pin also moves sideways, in toward the pivot, as the
  lever tilts away from level. The lever is level at mid-travel, which
  keeps that drift as small as it can be and the same at both ends: it
  tilts 13 degrees each way, so the router pin comes in 2.9 mm and the
  jack pin 5.8 mm. The router end takes it up by the link leaning,
  1.5 degrees at most; the jack end by the pin sliding along the
  saddle's flat lips. Neither changes the 2 to 1: the saddle's lips are
  flat, so the jack end's height is exactly 220 mm times the sine of
  the tilt and the router end's 110 mm times it, less what the link's
  lean costs, under 0.04 % at its worst.
- **Why a printed handle.** The router's handle mount is a lug cast on
  the housing: a rectangular block standing out from it, with a
  threaded hole and a roll pin in its end. The original handle's
  rectangular stub is hollow. It slips over the lug, and its floor has
  a hole for the screw and one for the pin. The printed handle copies
  that, so it goes on exactly as the original does. The load goes
  through the lug, the pin and the screw, as it does in your hand. The
  tab is the only addition. Keep the original handle to put back for
  freehand work.
- **The crank moves.** The screw sits at the scissor's knuckles, halfway
  between the foot and the saddle, so it drops half as far as the
  saddle: 50 mm over the travel. It also moves 34 mm back as the
  knuckles close. The front's slot is vertical and 50 mm long, plus the
  extension's width, and the extension simply slides with the screw.
- **The front stops at the hanger.** The router's half of the box is
  open at the front for the switch, the plunge lock, the cord, and your
  hands at bit changes.

| Retracted | Fully plunged |
|---|---|
| ![Through the lever, retracted](out/section_retracted.png) | ![Through the lever, plunged](out/section_plunged.png) |
| ![Through the jack, retracted](out/jack_retracted.png) | ![Through the jack, plunged](out/jack_plunged.png) |
| ![Through the link, retracted](out/link_retracted.png) | ![Through the link, plunged](out/link_plunged.png) |

## What the numbers give

These are from the build's printout and the test:

| plunge | bit over the table | jack (foot to saddle) | arms | bit per crank turn |
|---|---|---|---|---|
| 0 | 24 mm | 180 mm (7.09 in) | 17.5° | 2.3 mm |
| 12.5 | 37 | 205 (8.07) | 22.2° | 1.7 |
| 25 | 49 | 230 (9.06) | 27.1° | 1.4 |
| 37.5 | 62 | 255 (10.04) | 32.3° | 1.1 |
| 50 | 74 | 280 (11.03) | 37.8° | 0.9 |

The bit's heights are for a bit standing 50 mm out of the Yakamoz
2-1/4 in collet extension.

- **Use the collet extension.** Without it the bit reaches only 17 mm
  above the table: the collet nut sits 64 mm up inside the base, the
  plunge is 50 and the 3/4 ply top takes 19 more. The extension adds
  57 mm, so the travel is 50 mm wherever you want it: set how far the
  bit stands out of the extension so the cut you need falls inside
  it. With 50 mm out, the bit goes from 24 to 74 mm above the table;
  with 25 out, from just under the surface to 49 above. At full plunge
  the extension's own nut stands above the table, so bits change from
  on top, with a wrench through the 2 in hole.

- Going from retracted to fully plunged takes 37 turns: measured, from 7 to 11 in.
- One turn moves the bit 2.3 mm at the bottom of the travel and
  0.9 mm at the top; a quarter turn of a ratchet is a quarter to half
  a millimetre.
- The router's own plunge lock holds the height. Crank to height, then
  lock it; the lever only has to hold while you set it.
- **Both handles are printed short.** Under the table the handles
  stand beside the base with their wide ends up. The lug is 79.75 mm
  below the base face (89 on the bench, less the 9.25 sub-base that
  comes off), and the originals' wide ends reach about 30 mm above it.
  At the router's full 50 mm of plunge the originals would be a
  quarter of a millimetre from the table's underside. The printed
  handles stop 12 mm above the lug's centre, enough to close the stub
  over the lug, and stay 17.8 mm clear at full plunge. No depth stop
  is needed. Keep the originals for freehand work.

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
  collet extension passes through the 2 in hole, and at full plunge its
  nut stands above the table for bit changes.
- **Jack:** upside down, its foot bolted under the top with two 5/16
  flat-head bolts through holes drilled in the foot's flat. On the eye (the manual's rocker head),
  fit a scissor-jack drill adapter (a socket with a slot and a cross pin
  for the eye; any auto parts store has one), and on the adapter a 3/8
  drive 6 in extension.
- **Hanger:** two 3/4 ply cheeks, 2-1/2 wide, with a hardwood spacer
  between them at the top, screwed down through the top. The pivot is
  a 3/8 bolt with a washer each side of the lever, about 8-1/4 in below the
  top.
- **Lever:** oak, 5/8 x 1 x 14-3/4. That is the most the saddle's groove
  (19 wide, 13.4 deep) takes: 1.5 mm a side, and its top edge clear of
  the groove's floor as it tilts. A relief 3 mm deep and 36 mm long in
  its top edge, centred on the jack pin, clears the rivet head on the
  groove's floor (15.44 across, 4.65 tall). It has three 3/8 holes on
  its centre line:
  - the pivot;
  - 110 mm one way, the router pin;
  - 220 mm the other way, the jack pin.

  The jack pin is a 3/8 x 2 bolt with a nut. The router pin is a 3/8 x
  1-1/4 bolt, its head on the lever's back face, through the lever and
  the link, with a nylock.
- **Printed left handle** (`out/handle.stl`, PETG). It has:
  - the original's outline, cut off 12 mm above the lug's centre at
    the wide end;
  - a hollow stub that fits over the lug. The lug tapers, so the
    stub's pocket tapers to match, with 0.3 mm of clearance a side. The
    pocket is 16.37 mm deep, as the original handle's socket, so its floor seats on
    the lug's end and its rim stays clear of the housing;
  - in the stub's floor, a counterbored hole for the original screw and
    a hole for the roll pin;
  - a 10 mm tab 12 mm past the handle's narrow end (toward the motor), on the lever's side,
    for a 3/8 x 1-1/4 bolt to the link.

  Print it lying on its outer face, with 4 walls and 40 % infill.
- **Printed right handle** (`out/right_handle.stl`, PETG): the left
  one's mirror without the tab. On both lugs the screw is toward the
  front (the label's side) and the pin behind it, so the holes match.
- **Link** (`out/link.stl`, PETG): 10 mm thick, 20 wide, with 3/8 holes
  107 mm apart. Print it flat.

- **Fit test** (`out/fit_test.stl`, PETG): the stub alone on a 10 mm
  floor, with the screw hole and the pin hole. Print it first and try
  it on the lug. It should slide on with a little play and seat on the
  lug's end, with the pin in its hole and the screw hole over the
  threaded one. If it's tight or loose, change `CLEAR` in `build.py`;
  if the holes don't line up, change `PIN_SPACING` or `SCREW_SIDE`.

| Outer face | Inner face, over the lug | Fit test |
|---|---|---|
| ![The printed handle](out/handle.png) | ![Its stub, screw and pin holes](out/handle_inner.png) | ![The fit test](out/fit_test.png) |

`out/jack_table.pdf` is the A3 sheet. `out/lift.pdf` (and `lift.png`)
shows the table from the front at retracted, half plunged and fully
plunged. The front panel hides the jack there; the sections above show
it. `out/jack_table.okpart` is the document. It has:

- one part studio per part;
- the router motor, bit and printed handle as a sub-assembly on a slider
  (`plunge`) along a post;
- the link on a revolute (`link`) about the handle's tab bolt;
- the lever on a revolute (`lever`) about the pivot bolt;
- the jack's arms, saddle, screw and nut trunnion on revolutes about
  their pins, chained from the foot.

`out/poses.json` holds every mate's value at five plunge positions. The
script computes them from the link, the lever and the scissor geometry.
The test sets them and checks that the parts agree:
- the link's lower hole lands on the lever's pin;
- the rear arms meet and the screw stays level;
- the lever touches the saddle without passing into it.

## What is measured and what is guessed

Every dimension of the router and the jack is a named constant at the
top of `build.py`, each marked S (the maker's specification), M
(measured) or G (guessed from the photos). Replace a guess with a
reading, rebuild, and the test says whether it still works. The
guesses that matter:

| | value | from | what it moves |
|---|---|---|---|
| plunge | 50 mm | M: on this router (the manual says 65) | the jack's travel, at 2 to 1, and how high the bit comes |
| collet nut's end above the base face, retracted | 63.75 mm | M: 73 from the sub-base's face, less its 9.25 | how high the bit comes |
| collet extension | 3-1/4 overall, 1 in of shank in the collet, so 2-1/4 (57 mm) longer; 20, 25 and 36 mm across in steps | M: the listing; G: the diameters, from its photo | how high the bit comes; its 36 mm nut must pass the 2 in hole |
| base face to motor end | 283.75 mm | M: 293 on the sub-base, less its 9.25 | the table's height |
| across the handles | 276 mm | S | where the link goes |
| the lug across (long side) | 26.34 by the housing, 22.8 by the end, 13.4 apart | M: calipers, at two marker lines | the stub's pocket |
| the lug along the router's axis (short side) | 12.9 by the housing, 11.5 by the end | M: calipers, at the same lines | the stub's pocket |
| the end line from the lug's end | 2.8 mm | G: from the photo of its side | the pocket's floor size |
| how far the lug stands out | 17 mm | G: from the photo of its side; it must be more than the original's 16.37 socket | the lever's position |
| lug end face to lug end face | 200 mm | M | where the handles sit, and the lever, hanger and jack with them |
| the original handle's socket depth | 16.37 mm | M: calipers | the printed handle's pocket |
| screw to roll pin, centre to centre | 13.0 mm | M: calipers, in the original handle's socket | the two holes in the stub's floor |
| which side the screw is on | the front (the label's side), the pin behind it, on both lugs | M: photos | the two holes |
| roll pin: diameter, out from the lug | 5.05 mm, 9.9 mm | M: calipers | the pin's hole (5.65, 11.9 deep) |
| handle outline | 91 tall, 48 wide tapering to 39 | M: calipers | the printed handle |
| handle thickness | 50 overall, 33.63 outer face to socket floor | M: calipers | the printed handle |
| the hollow stub, outside | 32 x 18.5 | the pocket's mouth and a 3 mm wall (the earlier 41 x 21 by eye was too big for this lug) | the printed handle |
| the screw | M6 x 39.7, 10.26 head, 12.6 into the lug | M: calipers | the printed handle's hole (6.5), counterbore (11) and the 27.1 mm of plastic under the head, as the original |
| the stub from the handle's middle | 16 mm toward the wide end | M: by eye | where the tab is |
| which end of the handle is toward the base | the wide end, with the lug | M: photo | where the tab is: off the narrow end, toward the motor |
| the lug's centre above the base face, retracted | 79.75 mm | M: 89 from the bench on the 9.25 sub-base, which comes off | the link's length (107 mm now), and the handles' clearance under the table |
| motor diameter, posts | 100 mm, 20 mm at ±64 | G | clearances only |
| jack arm (fork), rivet to the nut's pivot | 160 mm | M | turns per mm, the slot |
| foot and saddle rivets from their faces | 42 mm | G: with 160 arms, 15-1/8 high needs about 84 between them | the jack's heights |
| jack screw | the knuckles close 1.42 mm a turn | M: fitted to 37 turns from 7 to 11 in | turns per mm only |
| saddle head and groove | 54 across, 32 along the groove; the groove 19 wide, 13.4 deep, turned along the lever (the narrow 7.5 mm one runs across) | M: by rule; the saddle swivels | the lever's size: 5/8 x 1 |
| the saddle's rivet head, on the groove's floor | 15.44 across, 4.65 tall (3.87 with the saddle pulled out; under load it is pushed home) | M: calipers | the relief in the lever's top edge |

Everything that matters is now measured. What is left to guesses moves
only clearances, or, for the jack's rivets (42 mm from the foot and
head faces, worked back from its 15-1/8 in height), the turns per mm.

## Open

- [ ] **The saddle.** It swivels: turn its wide (19 mm) groove to run
      along the lever before bolting the jack up.
- [ ] **One-sided push.** The link pulls on one handle, beside one post.
      If the motor cocks and binds on the posts, print a second handle
      for the right side and fork the lever's router end into a yoke
      under both.
- [ ] **Print the fit test** (`out/fit_test.stl`) and try it on the lug
      before the full handle.
- [ ] **A depth stop for repeat setups:** the router's own turret stop
      still works upside down.
