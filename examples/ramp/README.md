# Folding torsion-box truck ramp — design brief

A two-piece folding loading ramp for a Hyundai Santa Cruz: two identical
4 ft torsion-box panels, hinged on a length of 3/4" pipe that doubles
as the carrying handle and pulls out to separate the halves. This file
is the state of the design as it left a chat on 2026-10-04. Nothing is
built yet, in wood or in offkilter. The job of the next session is to
turn it into an offkilter document the way `router-lift/` and
`puzzle-top/` were: a `build.py` driving `ok-mcp`, a committed
`out/ramp.okpart`, screenshots, shop drawings, a range-of-motion sheet
and a regression test. Read `docs/MCP.md` and `docs/OPS.md` first, and
`examples/router-lift/build.py` for the pattern.

Units: the shop works in inches; the document is millimetres like the
other examples. Define `IN = 25.4` at the top of `build.py` and write
every dimension once, in inches, as a named constant. Everything below
that says "tune" should be a variable in the document so it can be
dragged in the browser.

## 1. What it is and why

- Replaces two loose 8 ft 2x8 planks with end plates. Those work but
  are too long to leave in the bed.
- Two **identical** panels, 21" wide × 48" long × 2-3/8" thick, each a
  torsion box of 15/32" CDX plywood skins on 2x2 ribs.
- Joined at the middle by a **pinned hinge**: lugs on each panel, one
  3/4" Schedule 40 galvanized pipe through all of them. Pipe centre
  sits below the bottom skin, so under load the deck butts in
  compression and the pipe carries the tension at the longest lever
  arm available.
- **Folds bottom-to-bottom** (hinge pin below the panel, like a
  drop-leaf table). Decks and curbs end up on the outside of the
  folded package.
- Pull the pipe (unscrew one pipe cap, slide it out) and the halves
  separate: two 48" × 21" × 3-3/8" panels at roughly 30 lb each.
- Lugs are arranged so the exposed pipe in the middle third is a
  handle for dragging the folded ramp out of the bed and along the
  tailgate.
- Tailgate end keeps the existing aluminium ramp end plates, which
  clamp a bare 1-1/2" × 7-1/4" board end, so that end of each panel
  has 2x8 stubs that run out past the skins.
- Lives under a bed cover or in the garage, painted with sand in the
  top coat for grip. Not a weather-exposed design.

Load case used throughout: 300 lb at mid-span of the open 96" ramp.
M = 300 × 96 / 4 = 7,200 in·lb. With the pipe centre 1" below the
bottom skin the lever arm from pipe to the compression at the top of
the deck is about 3.2", so pipe tension is roughly 2,250 lb total,
shared by six lugs (two in the rails, four interior, see §4). A 1/2"
pin in a single 3/4" ply lug was marginal; this is why the lugs are
1-1/2" laminated birch and the pin is a 1.05" OD pipe.

## 2. Stock

| item | spec | qty | notes |
|------|------|-----|-------|
| plywood, skins | 15/32" (0.4375") CDX sheathing, 4×8 | 1 sheet | rip two 21" strips, crosscut each at 48" → all four skins; C face up on top skins |
| ribs | 2x2 (actual 1.5" × 1.5") SPF, or 2x4 ripped in half | ~5 × 8 ft | pick straight, dry |
| cross blocks | same 2x2 | from above | cut to fit between ribs |
| joint blocking | 2x4 flat (1.5" thick × 3.5" wide) | 2 × 21" | one per panel, across the hinge end, inside the skins |
| tailgate stubs | 2x8 (1.5" × 7.25") | 2 × 12" per panel | outer rib positions at the tailgate end; run 3" past the skins, bare, for the end plates |
| rails and lugs | 3/4" birch plywood | ~1 sheet half | rails 4-3/8" tall; lug cheeks and interior lugs laminated to 1-1/2" |
| rail spacer | 3/4" birch | 2 × 48" × 2-3/8" | under the "spaced" rail on each panel (§4) |
| hinge pipe | 3/4" Sch 40 galvanized pipe, 1.05" OD, threaded both ends | 1 × ~26" | plus two 3/4" pipe caps |
| ground edge | 1/8" × 1" aluminium angle | 2 × 21" | over the 30° bevel |
| end plates | existing Erickson-type 2x8 ramp end plates | 2 pr | tailgate end only; model as a ghost body |
| adhesive | PL Premium (polyurethane construction adhesive) | | every rib and block face |
| screws | 1" and 1-1/4" | | 1" through skins into 2x2s; longer only into 2x8 / 2x4 |
| finish | exterior latex, sand broadcast into second top coat | | |

## 3. Panel geometry

All of this is one panel. The second panel is the same part rotated
180° about the vertical axis, see §4.

Coordinates for the build: x across the width (0 to 21), y along the
length (0 at the **hinge end**, 48 at the tailgate end), z up (0 at
the bottom face of the bottom skin).

- Overall box: 21.0 × 48.0 × 2.375 (0.4375 + 1.5 + 0.4375).
- Skins: 21 × 48 × 0.4375, except at the tailgate end where both skins
  stop at y = 45 so the 2x8 stubs run bare from y = 45 to 48. Bottom
  skin also has two notches at the hinge end for the interior lugs
  (§4), 1.5" wide × 3" long.
- Long ribs: 2x2 on edge, five of them, centred at x = 1.5 (edge),
  6.0, 10.5, 15.0, 19.5 (edge), running y = 3.5 to 45 between the
  joint blocking and the stubs. Outer two positions are replaced by
  the 2x8 stubs from y = 33 to 48 (stub 12" long, 3" of it bare).
  Tune the rib pitch; the skin only has to span the gap, and 15/32"
  is fine to 6".
- Cross blocks: 2x2 between ribs at y = 16 and y = 32.
- Joint blocking: 2x4 flat across the full width at y = 0 to 3.5,
  between the skins. This is what the deck compresses against when
  the ramp is loaded, and what the lugs glue to.
- Free ends: each panel has a hinge end (y = 0) and a free end
  (y = 48). In use, one panel's free end is at the tailgate and the
  other's is on the ground. Because the panels are identical, **both**
  free ends carry the 2x8 stubs and the bare 3". The tailgate panel's
  stubs take the end plates; the ground panel's stubs get the 30°
  bevel and the aluminium angle in the shop. In the model: stubs on
  both, with the bevel and angle as suppressible features so the
  drawing shows both ends.

## 4. Hinge: rails, lugs, pipe

The hinge axis is parallel to x, at y = 0 (the joint plane, where the
two panels' skins and joint blocking butt), at z = −1.0 (pipe centre
1" below the bottom skin). Tune `PIN_DROP`; it trades lever arm against
how far the lugs hang below the panel and how big the gap is when
folded.

**Pipe**: 1.05" OD, length ≈ 21 + 2 × 0.75 (rails) + 2 × 0.75 (spaced
rail offset) + caps ≈ 26". Lug bores 1.125".

**Rails** (3/4" birch, on edge, outside the box on both long sides):

- Height: 1.0 curb above the deck + 2.375 box = 3.375 along the
  length. At the hinge end the rail drops to enclose the pipe: lug
  region is a half-round of radius 1.0 about the pin centre, so the
  rail bottom reaches z = −2.0 there, tapering back up to z = 0 over
  ~6" in y so the ramp lies flat on the tailgate.
- Above the pin, the rail ends flat on the joint plane y = 0, flush
  with the skins. Below the deck it runs 1.0" past y = 0 (the
  half-round around the pin).
- Lug cheek: a second 3/4" birch piece, 6" long, glued to the inside
  of the rail at the hinge end so the pipe bears on 1-1/2" of birch,
  not 3/4". Fender washer outboard.
- **Offset**: on each panel, the rail at x = 0 is glued flush to the
  box side (occupies x = −0.75 to 0); the rail at x = 21 sits on a
  3/4" spacer (occupies x = 21.75 to 22.5). Rotate the second panel
  180° in plan and its flush rail lands at x = 21 to 21.75, its
  spaced rail at x = −1.5 to −0.75: each side has the two panels'
  rail lugs side by side on the pipe. Overall width with rails 24".

**Interior lugs** (laminated 3/4" birch, 1.5" thick × 6" long, same
half-round end about the pin): glued to the side of a long rib and to
the joint blocking, passing down through the 1.5 × 3 notches in the
bottom skin to the pipe. Two per panel, centred at **x = 4.5 and
x = 15.0**. Non-collision rule for identical panels: if a panel's lugs
are at a and b, the rotated panel's are at 21 − a and 21 − b, and they
nest 1.5" apart only when a + b = 19.5. With 4.5 and 15.0 the four
interior lugs occupy x = 3.75–5.25, 5.25–6.75, 14.25–15.75, 15.75–17.25
and the bare pipe in the middle runs x = 6.75 to 14.25: **7.5" of
handle**. Make a and b variables and assert the rule in the test.

(The lug positions do not coincide with the long-rib positions above;
either move two ribs to 4.5 and 15.0 — simplest, do that — or sister
the lugs onto blocks. Prefer moving the ribs: five ribs at 1.5, 4.5,
10.5, 15.0, 19.5 is fine.)

**Folding**: revolute mate on the pipe axis between the two panel
instances. 0° is open and flat (decks coplanar, joint faces touching).
180° is folded bottom-to-bottom; bottoms are then 2 × PIN_DROP = 2"
apart, lugs interleaved. Check interference at 0, 45, 90, 135, 180.
The bottom edge of each box at y = 0 is 1" from the pin and sweeps a
1" radius; chamfer or round that edge 3/8" in the model and confirm
it clears.

**Takedown**: pipe is its own part with caps; the assembly should show
it, and the drawing's parts list should carry it as one item.

## 5. What to build in offkilter

Part studios (one tab each, names as the shop will use them):

1. `skin_top`, `skin_bottom` (bottom has the two lug notches)
2. `rib` (2x2 × 41.5), `cross_block`, `joint_block` (2x4 × 21)
3. `stub` (2x8 × 12), with a suppressible 30° bevel feature
4. `rail` with its lug profile, `lug_cheek`, `rail_spacer`
5. `interior_lug`
6. `pipe` (1.05 OD × 26, bore 0.824) and `pipe_cap`
7. `ground_angle` (aluminium angle, 21")
8. `end_plate` as a ghost: a block 7.25 × 1.5 pocket, 2" long, to
   show the stubs are bare where it clamps; no need to model the real
   casting

Assemblies:

- `panel`: everything above except the pipe, placed with fastened
  mates. This is the sub-assembly used twice.
- `ramp`: two `panel` instances, the second rotated 180° about z and
  positioned so its lugs are coaxial with the first's; the pipe
  through all six lugs; one revolute mate (panel 2 about panel 1 on
  the pipe axis). End plates on panel 1's stubs, bevel + angle on
  panel 2's.

Outputs the session should produce and commit, like the other
examples:

- `out/ramp.okpart`
- `out/*.png`: iso of the open ramp, folded ramp, one panel upside
  down (lugs and notches), a section through a lug on the pipe axis
- `out/panel.pdf`: shop drawing with a section across the width
  (ribs, skins, rails) and one along the length through a lug
- `out/ramp.pdf`: assembly sheet with balloons and parts list
- `out/fold.pdf` and `out/fold.png`: `range_of_motion` at 0, 45, 90,
  135, 180° from the side
- `out/cutlist.csv`: BOM; the test should also derive sheet yield
  (all four skins from one 4×8, rails and lugs from how much birch)

Regression test (`crates/ok-render/tests/ramp.rs`, following
`router_lift.rs`): every part a closed solid; the assembly places every
instance; six lug bores coaxial within tolerance; interior lugs satisfy
a + b = 19.5 and do not overlap; bare pipe between the inner lugs ≥ 7";
no interference at 0/45/90/135/180°; folded package extents ≈
24 × 48 × (2 × 3.375 + 2) with the lugs interleaved; open ramp overall
96 × 24.

## 6. Shop sequence (for the drawing notes, not the model)

1. Cut skins, ribs, blocks, stubs. Notch bottom skins for the lugs.
2. On a flat surface: bottom skin, PL on every rib face, ribs and
   blocking and stubs down, screws, then PL and the top skin. Weight
   it. 24 h.
3. Rails: rip 3/4" birch, cut the lug profile, glue on the cheek,
   bore 1-1/8" for the pipe. Glue and screw rails on (one flush, one
   on the spacer). Interior lugs through the notches, glued to rib and
   blocking.
4. Bore the interior lugs in place using the rail bores as the guide
   so all six are coaxial. A long 1-1/8" bit or a bored jig block.
5. Dry-fit both panels on the pipe, check the fold. Bevel the ground
   end, fit the angle, fit the end plates.
6. Paint; sand into the second top coat; third thin coat over it.

## 7. Open questions for John

- `PIN_DROP` 1" assumed. Smaller means less lug hanging below and a
  tighter fold; larger means less pipe tension.
- Curb height 1" assumed. Drop it if the folded package needs to be
  thinner.
- Rail taper length behind the lug (6" assumed) vs. a full-length
  skid that lifts the bottom skin off the ground.
- Whether the end plates clamp cleanly with the rail ending 3" short
  of the tailgate end, or need it shorter.
- Rib pitch, and whether the cross blocks at 16 and 32 are enough.
- Whether to keep the panels strictly identical (one part, two
  instances) or let the ground panel lose its stubs and gain a
  one-piece beveled edge. Identical is the current intent.

## 8. What was built (2026-10-05)

`build.py` makes the document the brief asked for; `cargo test -p
ok-render --test ramp` checks it. Where the build departs from the
brief, this is why.

- **Parts.** One studio per part as listed in §5, in inches through
  `IN = 25.4`, every part built in place in the panel's frame. The two
  rails, the two cheeks and the two interior lugs are separate studios
  rather than two bodies in one, because a kernel `add` joins every
  body in the studio and each of those parts needs a half-round added
  to its profile. The ribs are five bodies of their real lengths (the
  lug lines start behind the lugs, the outer lines stop at the stubs),
  which is what the cut list wants anyway.
- **Panel.** A `panel` assembly of fixed instances, used twice in
  `ramp`: panel 2 is the same tab placed turned 180 degrees about z
  and moved 21 in x. Fixed placement rather than fastened mates, since
  the panel is one glued box and a mate per part would say nothing.
- **Fold.** One revolute, `fold`, between the pipe and panel 2's flush
  rail bore, its parameters read off the drawn pose. The fold goes
  under: panel 2's free end drops, and at 180 degrees it lies under
  panel 1, bottoms 3 in apart (twice `PIN_DROP`), lugs interleaved.
  The bottom skin's hinge-end edge has the 3/8 chamfer and the test
  finds no interference at 0, 5, 10, 20, 45, 90, 135, 170 and 180
  degrees.
- **Tunables.** `skin`, `rib`, `curb`, `pin_drop`, `lug_a` and `lug_b`
  are document variables; the skin and rib extrude depths are bound to
  the first two. The rest live in sketched profiles, which this script
  redraws from the constants at the top.
- **Bevel and angle.** The stub's 30 degree bevel is a suppressed
  feature: on in `out/stub.pdf`, off in the ramp, whose two panels are
  identical. The ground angle is a ramp-level part on panel 2's end,
  the end plates ghosts on panel 1's stubs.
- **Outputs.** `out/ramp.okpart`; `ramp_iso`, `ramp_front`,
  `ramp_side`, `ramp_folded`, `ramp_folded_side`, `panel_below`
  (lugs, notches, cheeks) and `ramp_lug_section` (through a lug on the
  pipe axis); `panel.pdf` with a section across the width at
  mid-length and one along the length through a lug; `ramp.pdf` with
  the panel as one item twice and the pipe as one item; `stub.pdf`;
  `fold.pdf` and `fold.png` at 0, 45, 90, 135 and 180 degrees from
  the side; `cutlist.csv`.
- **The test** regenerates every part closed and places every instance;
  finds the eight bores (two rails and two lugs per panel) on the
  pipe's axis within a hundredth of a millimetre; checks a + b = 19.5,
  the four interior lugs side by side without overlap, the handle 7.5
  in bare, and the rails and cheeks paired at each end of the pipe;
  sweeps the fold; measures the open ramp at 96 x 24 over the rails
  and the folded package at 24 x 9.75 with the bottoms 3 in apart and
  the lugs filling the gap; and reads the sheet yield off the parts: the four skins
  are 82 % of one 4 x 8 (two 21 in rips crosscut at 45), the birch
  34 % of another.

Pipe length came out at 26 in with the caps outside its ends, 28 in
over the caps.

`PIN_DROP` and the lugs' radius are 1-1/2, not the 1 the brief
assumed, which answers the first open question in §7. The 7/16 wall a
1 in radius leaves round the 1-1/8 bore holds the 375 lb per lug on
paper (about 290 psi in tension and shear-out, 240 psi bearing, in two
plies of birch), but it is one pin diameter of end distance, and
shear-out at the end is the failure a shock load finds. The radius
cannot exceed the pin drop without the half-round meeting the other
panel's bottom, so both went to 1-1/2: a 15/16 wall, the lever arm
3.875 and the pipe tension about 1,860 lb, the folded bottoms 3 in
apart and the package 9.75 thick, the lugs 3 in below the deck at the
joint, and the rail taper 8 in long for the longer climb. The other
open questions stand.
