# Bucket parts tumbler

A tumbler built from a 5-gallon bucket, for easing the edges of about
40 small wood inlay pieces (curly-Spectre jigsaw tiles) per batch to a
soft "pillowed" edge of roughly 0.3–0.5 mm radius. The bucket is
tilted about 25° and turned directly by a slow 12 V worm-gear motor
bolted to the centre of its bottom. Two fixed casters under the upper
wall carry most of the weight. Everything mounts on one flat deck,
which is hinged at the motor end and propped up to set the tilt.

`build.py` makes the document through the MCP server and
`crates/ok-render/tests/bucket_tumbler.rs` checks it. **Status:** design
only, not built or tested.

    cargo build --release -p ok-mcp
    python3 examples/bucket-tumbler/build.py        # the document, pictures, sheet and numbers
    cargo test -p ok-render --test bucket_tumbler   # what CI runs

![the tumbler propped at 25 degrees](out/tumbler.png)

Every dimension is a named constant in inches at the top of
`build.py`. The document is in millimetres like the other examples.
The tab **on the deck** has the deck flat, with the bucket's axis on
the world x axis. **base** holds the base, the hinge and the prop
leg. **propped at 25 deg** turns the deck about the hinge.

## Why direct drive

A friction wheel on the bucket wall was considered and rejected for
three reasons:

- HDPE is slippery, so a single friction wheel is marginal on
  traction with this load.
- Buckets are tapered, so they walk sideways on rollers.
- A worm gearmotor at 30–40 rpm needs no further reduction. Its worm
  also holds the bucket still when it is stopped.

A fan motor was also rejected. It turns at around 1000 rpm or more
with very little torque, so it would need roughly 40:1 reduction. It
also relies on its own airflow for cooling, so it overheats when
enclosed and geared down.

## What the model checks

The build script prints the numbers below, and the test asserts them
on the regenerated bodies.

| Brief says | Model | |
|---|---|---|
| Axis 7.5 in above the deck clears the rim (6.2 in radius) by about 1.3 in | 1.30 in | ✓ |
| Casters about 10.5 in from the plate, about 5 in below the rim | 5.2 in below the rim, where the wall is 5.70 in in radius | ✓ |
| Contact 35° either side of bottom centre, wheels about 7.7 in apart | 7.71 in; each wheel touches the wall without cutting into it (0.5 mm closer and it would) | ✓ |
| Axles about 2.0 in above the deck, axis 5.5 in above the axles | 2.00 in and 5.50 in, with a 1/2 in riser under 2 in casters whose plate-to-axle height is 1.5 in | ✓ |
| Torque 0.7 N·m (7 kg·cm) worst case; a 20 kg·cm motor is about 3× | 1.5 kg at 2 in is 0.75 N·m = 7.6 kg·cm, so the margin is **2.6×** | close |
| Axial load at 25° about 1.3 kgf | 1.28 kgf: the bucket, battens, discs and coupling at the model's densities (1.5 kg) plus 1.5 kg of load, times sin 25° | ✓ |
| Prop leg about 8.5 in for 25° | 8.36 in at its front edge, 0.2 in short of the deck's end; the deck's end rises 8.45 in | ✓ |
| Three battens at 120° | at 0°, 120° and 240°, on the inside wall, flat backs bearing at their edges | ✓ |

The model turned up three things the brief does not say:

1. **The motor shaft may not reach through 3/4 in plywood.** The
   listings for the 5840-31ZY give the gearbox (58 × 40 × 35 mm) and
   the motor can (31 × 57 mm), but not the length of the output
   shaft. If the gearbox sits on the plate's back face, the shaft has
   to cross 19 mm of plywood and still leave enough inside the
   coupling's hub to reach the set screws. In the model the gearbox
   sits in a **58 × 40 mm pocket, 1/2 in deep, in the back of the
   plate**. That leaves a 1/4 in web around the shaft hole, and with
   an assumed 15 mm shaft, 7.6 mm of shaft goes into the hub. Measure
   the real shaft, and skip the pocket if it is longer than about
   30 mm.
2. **The gussets go on the bucket side.** The plate stands on the
   deck's end, so nothing lies behind it for a gusset to sit on. The
   model puts the two 4 in gussets at the plate's ends, inside. The
   bucket's narrow end is 5.2 in in radius there and the gussets are
   6.25 in out, so they are clear.
3. **The battens bear on their edges.** A flat 3/4 in batten on a
   wall of about 5.1 in inside radius stands 0.014 in off the wall in
   the middle. Screwed through the wall, it pulls the wall to it.
   Nothing needs doing.

![section through the bucket's axis](out/deck_section.png)
![looking along the axis: the casters and battens](out/casters_section.png)

## Measure these first *(fit)*

| What | In the model | How to check |
|---|---|---|
| Bucket wall radius at the bottom and just under the rim | 5.17 in and 5.95 in | Wrap a tape around the bucket and divide by 2π |
| Rim lip radius | 6.2 in | Callipers across the rim |
| Gearmotor output shaft length past the gearbox face | 15 mm (not in the listings) | Callipers; this decides whether the plate needs the pocket |
| Coupling hub and flange | hub 15 × 12 mm, flange 32 × 5 mm | The coupling's listing |
| Caster height, mounting plate to axle | 1.5 in | Measure the caster; shim the riser to suit |
| Where the ribbed collar starts below the rim | above the casters at 5.2 in | Look; the wheels must ride on smooth wall |

## Parts to buy

| Part | Spec | Notes |
|---|---|---|
| Gearmotor | 12 V DC worm-gear motor (5840-31ZY family or similar), 30–40 rpm no-load, rated torque ≥ 20 kg·cm, 8 mm D-shaft | The worm holds the bucket still when stopped |
| Speed control | PWM DC motor speed controller, 12 V, ≥ 5 A | Dial in 20–35 rpm |
| Power supply | 12 V DC, 5 A | Sized above stall current for margin |
| Flange coupling | Rigid flange coupling, 8 mm bore | Check the shaft size on the listing |
| Casters | 2 × fixed (non-swivel) 2 in casters, rubber or polyurethane wheel | Swivel casters will not track |
| Bucket | 5-gallon HDPE bucket with lid | Remove the wire handle |
| Hardware | 4 × 1/4 in carriage bolts with nuts and washers; #8 × 3/4 in pan-head screws with washers; one hinge | |

Prices have not been checked. Expect the motor, controller and supply
to be most of the cost.

## Wood parts

| Part | Size | Material |
|---|---|---|
| Base | 14 × 22 in | 3/4 plywood |
| Deck | 14 × 20 in | 3/4 plywood |
| Motor plate | 14 wide × 10 tall, shaft hole 1/2 in at 7.5 in above the deck, gearbox pocket 58 × 40 mm × 1/2 in from the back | 3/4 plywood |
| Gussets | 2 × 4 × 4 in triangles, on the bucket side at the plate's ends | 3/4 plywood |
| Caster riser | 3 × 14 in, about 1/2 in thick *(fit)* | plywood |
| Prop leg | 3 in wide, 8.4 in tall, its top bevelled 25° | scrap, or a hinged leg with a stop |
| Bottom discs | 2 × 4 in diameter, four 1/4 in holes on a 3 in circle | 1/2 plywood |
| Battens | 3 × 3/4 × 3/4 × 12 in | hardwood, exposed edges rounded over |

## Assembly

1. **Battens.** Space three battens 120° apart on the inside wall,
   running from the bottom up. Screw them from outside with pan-head
   screws and washers, because HDPE will not hold glue. Check that no
   screw tips come through.
2. **Bottom flange.** Find the bucket's centre; the moulding dimple on
   the bottom marks it. Screw the flange coupling to one disc,
   centred. Sandwich the bucket bottom between the two discs and clamp
   with the four carriage bolts, domed heads inside.
3. **Motor plate.** Cut the gearbox pocket in the back of the plate
   and drill the shaft hole. Mount the motor with its shaft through
   the hole. Fix the plate square to the low end of the deck, with
   the gussets on the bucket side.
4. **Casters.** Screw the casters to the riser with their axles
   parallel to the bucket's axis. Leave the riser clamped, not
   screwed, for now.
5. **Align.** Slip the flange onto the motor shaft and rest the wall
   on the casters. Slide and shim the riser until the bucket turns by
   hand without wobbling or binding. Then screw the riser down and
   tighten the flange set screws on the shaft's flat.
6. **Tilt.** Hinge the low (motor) end of the deck to the base and
   prop the high end to 25°.
7. **Wiring.** Supply → PWM controller → motor. Add a switch if the
   controller has none.

## Running

- **Speed:** 25–35 rpm. Count turns against a mark on the bucket.
- **Direction:** either; the battens are symmetrical.
- **Fill:** the pieces plus about 3× their volume in media, under one
  third of the bucket.
- **Media:** 2 in squares of 100–120 grit cloth-backed abrasive, plus
  a couple of handfuls of 3/4 in hardwood cubes.
- **Dust:** snap the lid on, or run it outside.
- **Species:** tumble maple and walnut separately.
- **First run:** five scrap pieces, checked every 15–20 minutes.
  Record the time that gives the edge you want.

## Things to watch

- **Axial load on the motor.** About 1.3 kgf along the shaft at 25°.
  If the motor's listing gives a lower axial limit, reduce the tilt to
  15° (0.8 kgf).
- **Pieces riding a batten.** Add more cubes, or make the battens
  lower.
- **Clatter** means the pieces are being thrown. Slow down or add
  media.
- **Bucket bottom flex.** If the bottom oil-cans around the discs, use
  larger (6 in) discs.

## Where the numbers came from

The design numbers are the owner's brief. The bucket sizes are those
of a common 5-gallon HDPE pail, and the motor's from its listings (for
example [RobotShop's 5840-31ZY page](https://www.robotshop.com/products/5840-31zy-12v-27rpm-high-torque-worm-gear-motor)).
The densities are typical: birch plywood 0.68, HDPE 0.95, hard maple
0.71 g/cm³. The motor body is an envelope at 1.6 g/cm³; the real
motor weighs about 0.5 kg.
