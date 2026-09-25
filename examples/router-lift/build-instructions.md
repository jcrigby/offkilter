# Trim Router Table + Lift — Build Instructions

Rev C table/lift and rev D shaft-pivot pin attachment, 2026-09-24.
Companion files: `trim_router_lift.scad`, `pin_arm.scad`, the two shop drawings, `carriage.stl`, `ring_*.stl`, `pin_chuck.stl`, `ring_align.stl`, `arm_template.dxf`.

All dimensions in mm unless noted. Coordinates: the bit axis is the origin; "behind" means toward the back edge of the top (where the pin arm mounts); "left/right" as seen standing at the front of the table.

---

## Part A — Table and lift

### A0. Before cutting anything

1. **Measure the router.** Housing diameter at the clamp band (expect 65 for the Bosch Colt). Set `router_d`. Look for any rib or key strip proud of the cylinder; if there is one, measure it and set `rack_w` / `rack_d`.
2. **Measure one pillow block and one shaft support.** Confirm against the defaults: SC20UU 50 wide × 45 long × 42 tall, shaft centre 25 from the base, bolt pattern 40 × 35, M5. SK20 base 60 wide, 20 thick, shaft centre 51 from the base, two 6.6 holes 42 apart. Fix any that differ, re-render, and re-check the echoed numbers.
3. **Re-render and note the echoes.** With the defaults: shafts 191 (×2), leadscrew 286, box inner width 253, box 210 tall below the top.

### A1. Print

| Part | Qty | Orientation | Notes |
|---|---|---|---|
| carriage | 1 | as modelled (bore vertical) | ABS/ASA, 6 walls, 6 top/bottom, 50 %+ gyroid or 100 %. Enclosure closed, bed ≥ 100 °C, brim, cool in the box. The nut-trap slots print as-is; no supports. |
| ring_blank, ring_align, ring_30, ring_40, ring_55 | 1 each | flat, spigot down | ABS/ASA, 100 % infill (thin parts). |

Test-print the top 20 mm of the carriage first if you want to check the router bore and the insert holes before committing 8+ hours.

### A2. Hardware prep

1. **Shafts.** Cut two 191 mm lengths from one 1000 mm shaft. Wrap tape at the cut line, abrasive cut-off wheel, then dress the end with a file and a light chamfer. Wipe down; the surface is what the blocks ride on.
2. **Leadscrew.** Cut the T8 to 286 mm. Chamfer the ends so the nut and bearings start cleanly.
3. **Coupling nut.** Degrease 30 mm of one end of the leadscrew and the bore of the 3/8-24 coupling nut. Loctite 638 (or 680) inside the nut, slide it on flush with the screw end, wipe the squeeze-out, and leave it 24 h. Optional: cross-drill 2 mm through nut and screw and tap in a roll pin.
4. **Nut traps.** The carriage's side faces have 16 M5 clearance holes and eight slots (four in the top face, four in the bottom) that hold the nuts 8 mm behind the face. Clear each slot with a small file so an M5 nut slides in without forcing. No inserts, no heat.

### A3. The top

Laminate two pieces of 3/4" Baltic birch, clamps everywhere, and trim to **400 × 380**. Mark the centreline. All positions below are on the centreline, measured from the **front** edge:

| Feature | Position from front | Spec |
|---|---|---|
| Bit axis | 140 | reference point |
| Ring rabbet | 140 | 90 dia × 5 deep, from the top face |
| Through-opening | 140 | 74 dia |
| Leadscrew hole | 191.5 | 12 dia through |
| Bearing pocket | 191.5 | 22 dia × 7 deep, **underside**, Forstner |

Order of operations:
1. Drill a 6 mm pilot through at the bit axis and at the leadscrew position.
2. Flip the top. Forstner the 22 × 7 bearing pocket on the underside, centred on the leadscrew pilot. Then drill the 12 mm through-hole up from the pocket floor.
3. Flip back. Route the 90 × 5 rabbet with a trammel or a circle jig, then cut the 74 through-opening (jigsaw inside the line and clean up with a flush-trim bit against a circle template, or a circle-cutting jig in one go).
4. Test-fit a ring: it should sit flush with the top and not rock. Ease the rabbet edge with 220 grit.
5. Optional weight relief: rout pockets in the underside anywhere except a 60 mm band around the opening, the 253 × 145 box footprint, and the outer 40 mm where the clamps bite. Stop 8 mm short of the top face.

### A4. The box

**Baseplate** 253 × 145 × 19. Leadscrew hole on the centreline **111.5 from the box front edge**: 22 × 7 Forstner pocket on the **top** face, then **13 mm** through (13, not 12, so only the outer race of the bearing bears on the pocket floor).

**Side rails** (2) 145 wide × 210 tall × 19. Each takes two SK20s on its inner face. Hole layout, measured from the rail's front edge and bottom edge:

| Support | Holes (from front) | Height (from bottom) |
|---|---|---|
| bottom SK20 | 39 and 81 | 29 |
| top SK20 | 39 and 81 | 200 |

Drill 6.6 mm. The easiest way is to clamp both rails together face-to-face and drill through both at once, then use an SK20 as the template for the second pair.

**Assemble the box:**
1. Glue and screw the rails to the ends of the baseplate (three #8 × 1-1/4" up through the baseplate into each rail's bottom edge). Inner faces must be parallel: check 253 at top and bottom on both ends, and square to the baseplate.
2. Do **not** attach the top yet.
3. Pocket-hole four holes on the inside of each rail near the top edge, angled up, for fastening to the top later. No glue at the top; you want to be able to lift it off.

### A5. Carriage sub-assembly

1. Press the T8 flanged nut into the recess on the carriage's top face, four M3 × 10 self-tapping. Leave them **finger-tight** for now.
2. Drop M5 nuts into the traps: for each column, the far nut first (40 mm down), then the near one. A dab of grease on a bolt tip fishes them into place. Bolt one SC20UU to each side face with four M5 × 25, base against the flat face, bore vertical. Bottom block first (nuts in from the carriage's bottom face), then the top block (nuts in from the top); snug, not torqued.
3. Slide a shaft through both blocks on one side. It should glide with no rock. If it binds, loosen the four bolts, work the shaft back and forth, and retighten. Repeat the other side.
4. M5 × 40 clamp bolts into the ears with nuts in the traps, loose.

### A6. Lift assembly

1. Bolt the two **bottom** SK20s to the rails, M6 × 40 through with nuts and washers outside, base bolts snug, clamp screws **loose**.
2. Drop a shaft into each bottom SK20. Slide the carriage (with its blocks) down onto both shafts.
3. Fit the two **top** SK20s over the shaft tops and bolt them to the rails, base bolts snug, clamp screws loose.
4. Run the carriage top to bottom by hand a dozen times. It should move freely the whole way. If it tightens anywhere, loosen the base bolts on the tight end and let the SK20s find their position, then snug them again. Only now tighten the SK20 clamp screws.
5. Press a 608 into the baseplate pocket.
6. From below, thread the leadscrew up through the baseplate and bearing and into the carriage nut. Turn it until 40 mm or so protrudes below the baseplate.
7. **Upper collar**: slide it onto the screw above the bearing so it sits on the inner race, and lock its set screw (blue Loctite).
8. **Lower collar**: under the baseplate, with a thin washer against the ply, pushed up snug, then locked. Turn the screw: it should spin freely with no end float. If it drags, back the lower collar off a hair.
9. Run the carriage through full travel with the screw. It should turn with fingertip torque the whole way. If it stiffens at one end, the nut is fighting the shafts: loosen the four M3s, run it back and forth, tighten them again with the carriage mid-travel.

### A7. Router and top

1. Press the second 608 into the underside pocket in the top.
2. Set the top on the box, feeding the leadscrew up through the 12 mm hole and the bearing. Box front face sits **80 mm behind the top's front edge**, centred left-right. Drive the pocket screws.
3. Turn the screw from above with a 9/16" socket to check the whole stack runs freely.
4. Remove the router motor from its base (quick-release). Slide it up into the carriage bore from below with the switch and speed dial facing the open front, cord exiting downward. Clamp band on the plain cylindrical part of the housing. Tighten the M5 clamp bolts evenly until the motor can't rotate by hand.
5. Cord out the open front to a paddle safety switch. Leave the router's own switch on.
6. Blank ring out, raise the router with the socket until the collet nut is above the top, fit a bit with two wrenches, lower it, ring in. Bit height now adjusts at **2 mm per full turn** of the socket.

### A8. Bench mounting

The top is 380 deep so the rear 155 mm sits on the bench with the box hanging off the front edge. Two F-clamps or holdfasts at the back corners. Check the top is not rocking on the bench before you clamp; shim if it is.

---

## Part B — Overarm pin attachment (shaft pivot, rev D)

Nothing on the table locates the arm. It pivots on a 20 mm shaft in two SK20 supports at the back of the top, with two SC20UU blocks under its tail as the pivot bushings. Stock can be any width and up to 145 mm deep behind the bit; headroom under the arm is 76 mm.

### B1. Print

| Part | Qty | Notes |
|---|---|---|
| pin_chuck | 1 | Split clamp for the 1/4" guide pin. ABS, 100 % infill. |
| ring_align | 1 | Add to the table's ring set: 1/4" centre hole for the daily check. |

### B2. Arm

1. Laminate two 3/4" pieces, print `arm_template.dxf` full size, and cut the plan shape: 250 wide at the tail, 80 at the nose, 275 long, 38 thick. Both faces flat; the underside is the reference.
2. Holes, laid out on the underside with the bit axis as origin and the tail toward +Y:

| Feature | Position | Spec |
|---|---|---|
| block bolts (8) | x = ±60 ± 17.5, y = 170 ± 20 | 5.5 through, heads on top |
| leveling bolt | x = 0, y = 215 | 8.5 through, M8 threaded insert from the top |
| chuck screws (2) | x = ±12, y = 0 | 3.5 pilot, slotted ±3 in Y |

Easiest: clamp each block in place, base up, and drill through its own holes.

### B3. Pivot on the table

1. Cut the shaft to 250 and chamfer the ends.
2. Screw the two SK20s to the top with their bases down, bore axis along X, centres **170 mm behind the bit axis, ±110 mm off the centreline**. Bases must both sit flat and the bores must line up; slide the shaft through both before driving the screws. Clamp screws loose.
3. Bolt the two SC20UU to the arm's underside, bases up, with M5 × 60 and nuts, centred at ±60. Snug, not torqued.
4. Slide the shaft out of one SK20, thread it through both blocks with the arm held level, and back into the support. Fit a shaft collar outside each block, loose.
5. Swing the arm through its range. It must move freely with no bind; if it doesn't, loosen the SK20 base screws and let them settle, then retighten. Now tighten the SK20 clamp screws.
6. Thread the M8 leveling bolt down through the tail insert until its tip touches the table with the arm level (nose 76 mm above the table, check with a block). Jam nut.

### B4. Guide pin and alignment

1. Chuck onto the nose with two #8 × 1" screws through the Y-slots, screws loose. Slide a 1/4" × 75 pin in with 45 mm exposed; snug the M4.
2. Chuck a 1/4" pin in the router and raise it 20 mm above the table.
3. Lower the arm. Slide a 1/4" ID bronze sleeve from the router's pin up onto the guide pin.
4. **X**: slide the arm along the shaft (collars loose) until the sleeve passes freely side to side. Push both collars against their blocks and lock them.
5. **Y**: nudge the chuck in its slots until the sleeve passes freely fore and aft. Tighten the chuck screws.
6. Lift and drop the arm a dozen times. The sleeve should still pass. That's the alignment; it holds until something is unbolted.
7. Alignment ring in. Lower the arm; the pin should drop straight into the centre hole. This is the daily check.

### B5. Hold-up and catch

A lid stay between the arm and the top, or a magnet on a short post behind the pivot that catches a washer on the tail, holds the arm up. The tail hits the table at about 85° of lift, so the stay should hold it around 60–70°. A magnet in the table under the nose stops the arm bouncing on a chattery cut.

### B6. Pin depth

Slide the guide pin in the chuck so its tip sits 2–3 mm above the workpiece surface with the template on top. Snug the M4. Change it when template thickness changes. For bearing-guided profile bits, fit the bronze sleeve that matches the bit's bearing OD (see operating notes).

---

## Operating notes

- Bit height: 1 full turn of the 9/16" socket = 2 mm. A quarter turn = 0.5 mm.
- Backlash never shows because the router's weight keeps the nut on one flank. Set final height by raising if you want the habit.
- Bit changes: blank ring out, raise the collet above the top, two wrenches, lower, ring in.
- Pin routing: arm up, template on the work, arm down, pin on the template. To hop to another groove: lift, move, drop. Alignment ring for a five-second check whenever you doubt it.
- Cut, then profile, from one template: the pin sets where the bit's *axis* goes, so match the pin to the bit. 1/4" pin with a 1/4" upcut; the 1/2" sleeve with a 1/2"-bearing roundover or chamfer (measure your bearings). Templates must be edge patterns the pin rides around, not grooves, for this to work. Bias the sleeve ~0.3 mm under the bearing OD so any error takes a hair more off the edge rather than leaving a step. Don't take the template off the work between the two passes.
- Router removal: blank ring out, loosen the two clamp bolts, lift the motor out through the top.
- Dust: the box is open front and back on purpose. Don't panel it in.
- Check the SK20 clamp screws and the collar set screws after the first hour of use; they seat.
