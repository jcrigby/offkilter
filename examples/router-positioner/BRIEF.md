# Two-Axis Nested-Box Router Positioner — Design Brief

Status: concept, agreed in design discussion 2026-11-17. Nothing is modelled or built yet.
Audience: a Claude Code session that will turn this into a parametric CAD model.

## 1. What it is

A self-contained, reusable module that holds a trim router and positions it on two axes under stepper control. First use is as the head of a horizontal mortising machine. Because it is a closed box with datum faces, it should also be remountable in other orientations (for example, stood on end as a router lift).

Orientation for the first use, seen from the operator:

- The router lies on its side. Bit points **left**, motor body is on the **right**.
- **X** = left/right, along the bit axis (plunge depth).
- **Z** = up/down (bit height).
- Front/back feed is not part of this module.

## 2. Design decisions already made

| Decision | Choice | Reason |
|---|---|---|
| Guides | Nested boxes sliding in boxes. No rods or linear bearings. | Box walls give long guides for free; simple, enclosed, printable. |
| Drive | One lead screw per axis, direct. No ramps or wedges. | A screw is already a ramp; a wedge adds friction and backlash with no packaging benefit here. |
| Screw position | On the centre of each moving box. | A central push has no twisting moment, so the box does not rack in its sleeve. |
| X screw | On the bit axis, directly behind the router. | True centre. The added length is accepted. |
| Motor mounting | Each motor is fixed to the end cap of a column. Spinning screw, travelling nut. | Motors do not slide; screws are fully enclosed. |
| Nut | Coupling-nut style, embedded in the moving box. | Long thread engagement, hex is easy to capture, cheap. |
| Enclosure | The whole mechanism sits in an outer box. | Makes it a reusable module with datum faces. |

## 3. Assembly, from outside in

1. **Outer box** — fixed. Carries the datum/mounting faces and the bit slot. Its interior is the Z guide.
2. **Middle box (Z stage)** — slides up and down in the outer box. A square sleeve runs left to right through it; that sleeve is the X guide.
3. **Inner box (X quill)** — square tube that holds the router and slides through the sleeve.
4. **X column** — fixed extension of the sleeve, off the right side of the middle box. X motor on its end cap. Moves up and down with Z.
5. **Z column** — fixed extension below the outer box (in the first-use orientation). Z motor on its end cap.

### X drive stack, going right from the bit

router → empty space equal to X travel → spider with nut boss → screw → thrust bearing → motor on column end cap

- The quill is longer than the router by the X travel. The space behind the router, inside the quill, is where the screw goes as the quill retracts.
- The **spider** is a plate across the back of the quill: a central nut boss with three or four ribs out to the quill walls, open between the ribs. Force enters through the walls but its resultant is on the centreline.
- Router cooling air and the cord pass between the ribs (or the cord through a side slot in the quill).
- At full retraction the screw tip must stop a few millimetres short of the router's back cap. The home switch enforces this.
- The square quill is the anti-rotation for the nut. No key is needed.

### Z drive

- Nut boss in the centre of the middle box's bottom wall.
- Clear space under the X sleeve, inside the middle box, equal to the Z travel, for the screw to enter.
- Screw, thrust bearing and motor in the Z column.

## 4. Requirements and rules of thumb

### Sliding fit

- **Engagement length** of each box in its guide: at least 1.5 to 2 times the box width at every point in the travel. Less than this racks like a sticky drawer.
- **Gibs on two adjacent sides** of each sliding pair: a loose strip pressed in by set screws. They push the box into the opposite corner. Those two faces are the reference faces; make them accurate and let the gib sides be loose.
- **Bearing strips, not full-face contact.** Raised strips near each end of the box, faced with UHMW tape or waxed. Leaves room for dust.
- Stiffness falls as the quill extends. Keep the bit close to the sleeve mouth at full plunge.

### Screws and nuts

- **Thrust bearing and collar at the motor end** of each screw so cutting force goes into the frame, not the motor bearings.
- **Coupling type:** jaw or rigid. Not spiral-cut flexible couplers; they compress axially and lose depth accuracy.
- **Anti-backlash on both axes.** Two nuts with a spring between them (or one coupling nut slit crosswise and squeezed by a set screw). The spring should load the nuts in the same direction the cutting force does. Do not rely on gravity for Z, because the module's orientation will change.
- **Radial float.** Hold the nut against rotation and axial movement, but give it about 0.5 mm radial play in its pocket (or float the motor mount). A rigid nut plus a rigid motor mount binds on any misalignment or a bent rod.
- **Only the nut is threaded.** Everything else the screw passes through is a clearance bore.
- **Material pairing:** steel coupling nut on steel threaded rod galls. Use a brass nut, or grease it, or use a T8 lead screw with a brass/acetal nut.
- Reference numbers: M8 x 1.25 threaded rod on a 200 step/rev motor gives about 0.006 mm per full step and roughly 5 to 10 mm/s. Self-locking. Slow for powered plunging, fine for positioning. T8 with 2 mm lead gives 0.01 mm per full step and is faster.

### Outer box

- **Datum faces:** the bit-side face and one adjacent face flat and square to both axes, each with a bolt or T-nut pattern. Mount by the bit-side face where possible to keep the load path short.
- **Bit slot:** vertical, as tall as Z travel plus bit clearance. The front of the middle box acts as a moving shutter behind it.
- **Column clearance:** the X column moves vertically with Z, so the outer box needs a slot or enough depth on the right side for that travel. The Z column exits the bottom.
- **Bit-change access:** reach to the collet with two wrenches. Either X travel far enough to bring the collet out of the front, or a side door.
- **Airflow:** intake at the back, exhaust toward the bit. Three layers of box surround the router.
- **Connector panel:** both motors, two limit switches and router power on one face.

### Controls

- One limit/home switch per axis, positioned relative to the outer box's datum faces so bit position relative to the mounting face is repeatable after remounting.
- Home Z away from the screw-crash end and X away from the work.
- Always finish a move from the same direction.
- Keep steppers energised while cutting, or add a clamp, since vibration can walk a screw.

### Loads to design for

- Router weight hangs to the right of the sleeve and tries to tip the Z stage.
- Plunge force acts along the bit axis: straight down the X screw, and as a tipping load on the Z stage.
- Feed force acts front to back: side load on both sliding pairs. Put the reference faces on the side the normal feed direction pushes toward.
- Priority is rigidity over print time or material.

### Materials

- Printed parts: PETG or ASA, not PLA (router heat plus steady spring load causes creep).
- If any box is wood, use plywood, not solid wood (seasonal movement changes the fit).

## 5. Open questions (not yet decided)

1. **Router model and its dimensions.** A Bosch Colt PR20EVS is on hand for the separate router-lift project; confirm whether this module uses it. Body diameter, length, vent and cord positions drive the quill size.
2. **Travel** on X and Z. Not specified. Sets quill length, column lengths and engagement lengths.
3. **Screw type:** M8 threaded rod with coupling nuts, or T8 lead screw.
4. **Motor size** (NEMA 17 assumed, not confirmed) and controller.
5. **Box material and construction:** fully printed, plywood with printed inserts, or mixed. Print-bed size limits.
6. **How the router is clamped** inside the quill, and whether it can be removed without disassembly.
7. **Z screw collision check:** confirm the central Z screw clears the X sleeve across the whole travel in the chosen proportions.
8. **Which faces are the reference faces** for each sliding pair, given the feed direction.
9. **Dust:** whether to add extraction at the bit slot.

## 6. Suggested first tasks for the CAD session

1. Ask for or look up the router's dimensions and the desired X and Z travel (questions 1 and 2).
2. Build a parametric model with top-level parameters: router envelope, X travel, Z travel, wall thickness, sliding clearance, screw diameter and pitch, motor frame size.
3. Derive quill length, sleeve length, column lengths and outer box size from those, enforcing the engagement-length rule.
4. Model the spider and nut boss, the gib pockets, bearing strips, thrust-bearing seats and motor end caps.
5. Check interferences at all four travel extremes, especially screw tip to router back cap, Z screw to X sleeve, and X column to outer box.
6. Report the overall envelope and total length behind the router so the trade-off can be reviewed before detailing.

## 7. Background: ideas considered and set aside

- **Ramp/wedge drive.** A ramp at angle θ gives output travel = input x tan θ and force gain of 1/tan θ, and self-locks at shallow angles. Capturing the follower between two parallel faces (a parallelogram in a parallelogram window) makes it push and pull. Rejected here: extra friction, side loads and reversal backlash, and its packaging advantage disappears when the router is sideways.
- **Rods and bushings.** Workable, but replaced by the nested-box guides.
- **Off-centre screw under the quill.** Acceptable with long engagement, but the on-axis screw was preferred.
- **Motor riding on the moving box, or non-captive stepper.** Both work; the fixed-motor travelling-nut layout was chosen to avoid moving cables and exposed screw.
- **Side preload principle (kept).** A constant one-directional side load seats a slide against a reference face and makes it repeatable. This is what the gibs do.
