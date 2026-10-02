# A powered cart from an EGO power head, rev B

A cart for carrying two large Costco bags up the hill home, driven by
the EGO Multi-Head power head that is already in the garage. Rev A
was a two-wheel cart with a 50:1 gearbox of printed gears between the
wheels; this is what the design became once the bought parts were
looked at. One 20 inch bicycle wheel at the back drives the cart, two
swivel casters ahead of it on a wide track carry the front, and the
power head is the handle, clamped to a mast beside the wheel. The cut
male end of an EGO extension pole plugs into the head's coupler, a
flex shaft runs from that stub under the deck to a bought worm gearbox,
and the gearbox's output pinion drives a printed ring gear bolted to
the wheel hub's six-bolt disc mount. The two printed gears are
`add_gear` features with involute teeth, so the STL files print as
drawn.

```sh
cargo build --release -p ok-mcp
python3 examples/ego-cart/build.py          # the document, pictures and sheets
cargo test -p ok-render --test ego_cart     # what CI runs
```

Every number about a bought part is an assumption to measure; the
constants at the top of `build.py` say which.

## The numbers

| | |
|---|---|
| Power head | EGO PH1400/PH1420: 4800 rpm no load, 7 mm solid steel shaft with a splined end |
| Coupling | the male end of an EP7500 extension pole, cut 150 mm long, in the head's own coupler |
| Flex shaft | 16 mm casing, 233 mm end to end, a 25 mm coupling at each end |
| Worm gearbox | NMRV040, 20:1, 14 mm input, hollow 18 mm output, about 70 % efficient |
| Drive wheel | 20 inch bicycle front wheel, 508 mm tyre, 100 mm six-bolt disc hub, 10 mm axle |
| Ring gear | 80 teeth, module 3, internal, 265 mm rim, 20 mm face, 0.15 mm backlash; printed as six sectors, a 3 mm dowel in each rim joint |
| Pinion | 17 teeth, module 3, 20 mm face, 18 mm bore; printed |
| Ratio | 20 × 80/17 = 94.1:1 |
| Speed | 51 rpm at the wheel, 1.36 m/s (4.9 km/h) at no load; the trigger below that |
| Load case | 80 kg up a 15 % hill with 2 % rolling resistance |
| At the wheel | 133 N; 33.9 Nm at the axle, 282 N on the ring's teeth, 7.2 Nm at the pinion |
| At the head | 0.51 Nm, about 260 W: a quarter of what the head has |
| Grip | needs 23 kg on the drive wheel dry, 34 kg wet; it carries 40 |
| Casters | 200 mm pneumatic swivel, 760 mm track, contacts 300 mm ahead of the drive wheel |
| Deck | 860 × 780 × 12, 270 mm off the ground, a slot for the wheel and ring |
| Handle | the head at 50°, its coupler 334 mm behind and 225 mm above the ground, the grip 1.18 m up |

## Why a ring gear on the wheel

Rev A had both wheels fixed to a live axle with a gear keyed to it,
which standard wheels are not built for: they come with bearings and
expect a dead axle. With the gear on the wheel itself the axle is a
plain rod, the wheel stays as sold, and the big printed gear is where
the torque is, which is where Kris's advice that printed parts get
strength from size applies. An internal mesh is also the kindest to
printed teeth: several carry the load and the contact is convex on
concave. A bicycle hub's six-bolt disc mount is the bolt pattern the
ring needs, machined by someone else, and a 20 inch wheel rolls over
curb cuts the hand-truck wheels of rev A bounced on.

The ring sits outboard of the rotor face, where the spokes are not,
and the pinion meets it below and ahead of the axle so the gearbox
hangs under the deck with 123 mm of ground clearance. The first stage
is a bought worm box rather than printed bevels: the head's 4800 rpm
is too fast for a printed first pinion, the worm turns the drive 90°
in the same box, and at 20:1 it very nearly holds the cart on the
hill by itself. The flex shaft is what lets the head sit where the
hands want it rather than where the gearbox input points.

## Why a tricycle, and where the load goes

One driven wheel needs no differential and turns about itself; the
casters follow. The price is stability, and the numbers in this model
set the layout. A wide caster track alone does not make the cart
stable: the support polygon is a triangle from the drive wheel's
contact to the two casters', and it is a point at the drive wheel. The
mass centre has to sit well ahead of the wheel, towards the casters,
or the cart tips over the slanted edge of that triangle at a few
degrees of side slope, while traction wants weight on the drive
wheel. With the bags centred 250 mm ahead of the wheel and the
casters 300 mm ahead:

| | |
|---|---|
| Total, two full bags | 65 kg, mass centre 117 mm ahead of the drive wheel, 423 mm up |
| On the drive wheel | 61 % (40 kg; a wet hill needs 34) |
| Side slope that tips it | 10.9° with two bags, 15.9° empty |
| One bag on one side | 4.9°: load a single bag over the wheel, not beside it |
| On the 15 % hill | the mass centre moves 63 mm towards the wheel and stays ahead of it: no hand force |

Sidewalks cross-slope two or three degrees and curb ramps about
eight, so two bags are fine and one bag wants the middle. The handle
is the long lever of a wheelbarrow for everything fore and aft: press
down to lift the casters over a curb and roll the big wheel up it.

## The layout

The cart's frame has x across the cart (left negative), y towards the
user, z up, with the origin on the ground under the drive wheel's
contact. Every part is built in place in that frame, which is why the
assembly only names them. Two square-tube rails under the deck with a
cross member at each end, two axle beams from the rails to the
dropout plates either side of the wheel, a bracket hanging the worm
box, and the mast rising from the rear member make one welded frame.
The head rides 70 mm behind and below the mast on two clamps (not
drawn), its coupler just behind the deck's rear edge, and the stub
points down under the deck where the flex shaft meets it.

The ring is 265 mm across, more than most printers, so the model
drills a 3.2 mm hole 16 mm long along the rim across each of six
joints, then cuts the ring into six 60° sectors with three radial
planes through the axle, so every sector ends in half a dowel hole.
Drilling before cutting is what makes that robust: the holes are
placed by angle, not by finding cut faces afterwards. Each sector is
about 130 × 115 mm, the web's bolt holes sit between the cuts, and
the six glue up on the bolt pattern with a 3 mm pin across every
joint; `ring_gear.stl` holds all six in place.

What the model leaves out: spokes (a disc stands in), the casters'
forks, the clamps on the mast, a pawl, and the curve of the flex
shaft, which is drawn straight between its two couplings.

## What the model checks

`crates/ok-render/tests/ego_cart.rs` regenerates the document and
requires every part closed, the drive to be a worm box into a ring
gear at walking pace, the pinion's axis a pitch radius difference
from the wheel's with the teeth meshing (no overlap by boolean
intersection), the ring inside the tyre and outboard of the rotor
face, the bags on the deck inside its edges, the three wheels on the
ground, the gearbox clear of it, the grip at hand height, and the
stability numbers above from the same masses `build.py` prints: the
drive wheel's share, the side tip angle with two bags and with one,
and the mass centre ahead of the wheel on the hill; the ring's six
sectors alike in volume, each inside a 220 mm bed, each with a dowel
hole at both ends, and together meshing with the pinion. Then every
pair of placed bodies must not intersect, the contacts that touch by
design excepted. That check found five collisions while this revision
was laid out.

## Decisions still open

- **Rolling back.** A 20:1 worm is on the edge of self-locking; a
  pawl on the pinion shaft makes the hill a place you can stop. If
  the wheel gets a freewheel hub instead, the worm holds it and the
  cart pushes forward freely with no pawl and no clutch.
- **Pushing by hand.** Backdriving the worm through the head's motor
  is heavy without a freewheel. A swing arm for the gearbox that
  lifts the pinion out of the ring is a clutch with no extra parts.
- **The handle.** It sits 115 mm left of centre, over the gearbox. A
  bent mast could bring the grip to the middle.

## To measure

- The EP7500's tube diameter, the position of the locating hole from
  the coupler end, and the spline on its shaft, before cutting it; the
  measuring sheet (`examples/measuring-sheet/`) does the tube.
- The head's length from the coupler to the rear grip, and the motor
  block, for the handle geometry and the balance.
- The hub's rotor face offset and flange positions, so the ring's web
  clears the spokes by what the model assumes.
- The NMRV040's real envelope and shaft lengths.
