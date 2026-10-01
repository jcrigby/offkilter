# A powered shopping cart from an EGO power head

A two-wheel cart for carrying two large Costco bags up the hill home,
driven by the EGO Multi-Head power head that is already in the garage.
It is a broadcast spreader run backwards: a spreader takes motion from
its wheels, gears it up and spins the disc; this takes the power head's
shaft, gears it down fifty to one in a box of big printed gears between
the wheels, and drives the axle. The power head is also the handle: its
motor, battery and trigger are at the user's end of its metre-long
tube, so the cart's attachment is a short tube that plugs into the
head's coupler and brings the 7 mm shaft down into the gearbox.

```sh
cargo build --release -p ok-mcp
python3 examples/ego-cart/build.py          # the document, pictures and sheets
cargo test -p ok-render --test ego_cart     # what CI runs
```

This is the first increment: the layout, the gear train and the
numbers, with every part as simple as it can be and still occupy its
space. Gears are discs at their pitch diameters; the housing is a box
with the bores. Teeth, bearings, the clutch and the handle fittings
come next.

## The numbers

| | |
|---|---|
| Power head | EGO PH1400/PH1420: 4800 rpm no load, 7 mm solid steel shaft, counter-clockwise |
| Wheels | 4.10/3.50-4 pneumatic hand-truck wheels, 254 × 89, 5/8" ball bearings, 300 lb each |
| Axle | 5/8" steel rod, solid, both wheels fixed to it |
| Gear train | 12:36 bevel (module 2), then 15:60 and 15:63 spur (module 2.5, 25 mm faces): 50.4:1 |
| Speed | 95 rpm at the wheels, 1.27 m/s (4.6 km/h) at no load; the trigger below that |
| Load case | 80 kg (cart, two full bags, a hand leaning on it) up a 15 % hill with 2 % rolling resistance |
| At the tyres | 133 N; 16.9 Nm at the axle; 0.34 Nm at the head; about 140 W |
| Platform | 720 × 520, 290 mm off the ground, a 508 × 292 × 356 bag each side of the gearbox |
| Handle | the power head at 40 degrees, its coupler 470 mm behind and 360 mm above the axle, the rear grip about a metre up |

The head has ten times the power the hill needs, so the gearing is set
by walking pace, not by torque. Kris is right that printed parts get
their strength from size, and that holds for the two spur stages, where
the torque is: the axle gear is 157 mm across with 25 mm of face and
sees 130 N at its teeth. It does not hold for the first stage, which
sees 4800 rpm and almost no torque: a 24 mm bevel pinion there already
runs 6 m/s at its pitch line, which is as fast as printed plastic
should go. That stage wants to be small and smooth, and a bought steel
bevel pair there would be money well spent.

## The layout

The cart's frame has x across the axle, y towards the user, z up, with
the origin on the ground under the axle. Every shaft in the gearbox is
parallel to the axle and at its height, in a row behind it, so the box
is long and low (345 × 110 × 175) and the platform sits just above the
tyres rather than above the gears. The input axis comes down the
attachment tube at the handle's slope and meets shaft 3 at the bevel
apex; the housing's rear wall is cut square to that axis so the shaft's
bore and the tube's seat are one hole drilled normal to a face.

The frame is the least finished part: two side plates at the gearbox
carry the platform, and the axle runs out to the wheels unsupported
beyond them. Uprights at the wheels, with the axle bearings there and
the gearbox hung between, are the frame increment, along with whether
the bags ride over the wheels as drawn or between them on a narrower
platform with the wheels outside.

## What the model checks

`crates/ok-render/tests/ego_cart.rs` regenerates the document and
requires every part closed, every spur pair's shafts a pitch radius sum
apart, the bags on the platform and inside its edges, the gearbox clear
of the ground, and no body placed inside another, the pitch-circle
contact of the bevels and the bags' feet on the platform excepted.

## Decisions still open

- **Rolling back.** A 50:1 spur train is not self-locking, so with the
  trigger released a loaded cart on the hill rolls back and spins the
  head. A pawl on the axle, forward free and reverse locked, is the
  simplest cure, and it makes the hill a place you can stop.
- **Pushing by hand.** Backdriving the train through the head's motor
  is heavy. A sliding dog clutch on the axle gives a neutral for the
  store and the kitchen door; with the pawl it still holds on the hill.
- **Turning.** Two wheels fixed to one axle scrub in a turn. At walking
  pace on a 640 mm track that is what every garden cart does, and it
  stays until it annoys. The alternatives are a freewheel on each wheel
  (the outer one overruns) or driving one wheel.
- **The first stage.** Printed bevels, bought bevels, or a belt from the
  shaft to the first spur pinion, which would also take the shock when
  the trigger is squeezed.

## To measure

- The power head's tube diameter at the coupler, how the coupler locks,
  and the end form of the 7 mm shaft (square, spline or D). The
  measuring sheet (`examples/measuring-sheet/`) does the first; the
  shaft end wants calipers.
- The power head's length from the coupler to the rear grip, and the
  motor block's size, for the handle geometry and the balance.
- The wheels' hub length, which varies from 45 to 80 mm by maker, before
  the axle is cut.
