# Trim Router Table + Lift — Build Instructions

Rev C table/lift and rev C hinged pin attachment, 2026-09-18.
Companion files: `trim_router_lift.scad`, `pin_arm.scad`, the two shop drawings, `carriage.stl`, `ring_*.stl`, `pin_post_round.stl`, `pin_post_slot.stl`, `pin_chuck.stl`, `ring_align.stl`, `arm_template.dxf`.

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

## Part B — Overarm pin attachment (hinged, rev C)

### B1. Print

| Part | Qty | Notes |
|---|---|---|
| pin_post_round | 1 | 40 × 40 × 75, round socket. Print upright, 100 % infill. |
| pin_post_slot | 1 | Same, socket slotted 4 mm in X. |
| pin_chuck | 1 | Split clamp for the 1/4" guide pin. |
| ring_align | 1 | Add to the table's ring set: 1/4" centre hole for the daily check. |

### B2. Arm and rail

1. **Arm.** Laminate two 3/4" pieces, print `arm_template.dxf` full size, and cut the plan shape: 250 wide at the back, 80 wide at the nose, 290 long, 38 thick. Both faces flat; the underside is a reference.
2. **Rail.** Glue up ply to 250 × 60 × 75 (four layers of 3/4" or two of 1-1/2"). Its top must be flat and square to the front face.
3. **Chuck screw pilots** in the arm's underside: two 3.5 mm holes at ±12 mm either side of the bit axis, 0 mm fore-aft. Don't fit the chuck yet.

### B3. Rail and hinge on the top

1. Screw the rail to the top along the back edge, centred left-right, with its back face flush with the top's back edge. Its front face is the hinge line, **180 mm behind the bit axis**.
2. Piano hinge, 250 long: knuckle on the rail's front top corner. One leaf down the rail's front face, the other under the arm's tail. Elongate the screw holes in the leaf that goes on the arm by a millimetre each way before fitting; the hinge must not be the thing that locates the arm.
3. Fit the arm. Down, its tail lies on the rail top and the nose sits over the bit at 75 mm above the table. Lift it; it should swing freely past 90°.

### B4. Guide pin and alignment ring

1. Chuck onto the nose with two #8 × 1" screws through its flange, split toward the back. Slide a 1/4" × 75 pin into it with 45 mm exposed, snug the M4.
2. Blank ring out, alignment ring in.
3. Lower the arm slowly. The pin should drop into the ring's centre hole. If it lands off, that offset is the arm's position error and the posts (next step) will absorb it, so don't chase it here.

### B5. Set the registration posts

This is the one-time alignment. The posts get fixed wherever the arm lands when the guide pin is centred.

1. Press the two 10 mm dowel pins 20 mm into the arm's underside at **±60 mm either side of the bit axis, 70 mm behind it**, chamfered ends out. (A 9.9 mm hole gives a press fit in ply.)
2. Set the round post on the left and the slotted post on the right, loosely on the top with their sockets under the pins, screws not yet driven.
3. Chuck a 1/4" pin in the router and raise it 20 mm above the table. Alignment ring out.
4. Lower the arm. Slide a 1/4" ID bronze sleeve from the router's pin up onto the guide pin. When it passes both without binding, the pin is over the bit.
5. With the arm down and the sleeve in place, nudge each post until its socket is centred on its dowel and the arm's underside sits flat on both post tops. Drive the two countersunk screws in each post.
6. Lift and drop the arm a dozen times. It should seat with a click and the sleeve should still pass. Now the posts locate the arm every time; the hinge just swings.

### B6. Hold-up and catch

Fit a lid stay between the rail and the arm so it stays open at about 60°, or a magnet on a short post behind the rail that catches a washer on the arm's tail. A second magnet at the nose, set into the post top, stops the arm bouncing on a chattery cut.

### B7. Pin depth

Slide the guide pin in the chuck so its tip sits 2–3 mm above the workpiece surface with the template on top. Snug the M4. Change it when the template thickness changes.

### B8. Daily check

Alignment ring in, lower the arm. The pin drops into the centre hole. If it doesn't, something moved; go back to B5.

---

## Operating notes

- Bit height: 1 full turn of the 9/16" socket = 2 mm. A quarter turn = 0.5 mm.
- Backlash never shows because the router's weight keeps the nut on one flank. Set final height by raising if you want the habit.
- Bit changes: blank ring out, raise the collet above the top, two wrenches, lower, ring in.
- Pin routing: arm up, template on the work, arm down, pin in the groove. To hop to another groove: lift, move, drop. Alignment ring for a five-second check whenever you doubt it.
- Router removal: blank ring out, loosen the two clamp bolts, lift the motor out through the top.
- Dust: the box is open front and back on purpose. Don't panel it in.
- Check the SK20 clamp screws and the collar set screws after the first hour of use; they seat.
