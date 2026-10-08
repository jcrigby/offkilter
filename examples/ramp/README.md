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
| stubs | 2x8 (1.5" × 7.25") | 2 × 12" (tailgate panel), 2 × 9" (ground panel) | outer rib positions at the free end; the tailgate panel's run 3" past the skins, bare, for the end plates; the ground panel's stop flush with the skins (§8) |
| rails and lugs | 3/4" birch plywood | ~1 sheet half | rails 4-3/8" tall; lug cheeks and interior lugs laminated to 1-1/2" |
| rail spacer | 3/4" birch | 2 × 48" × 2-3/8" | under the "spaced" rail on each panel (§4) |
| hinge pipe | 3/4" Sch 40 galvanized pipe, 1.05" OD, threaded both ends | 1 × ~26" | plus two 3/4" pipe caps |
| bushings | SAE 841 oil-impregnated bronze sleeve, 1-1/16" ID × 1-5/16" OD × 3/4" | 8 | one pressed into every lug, flush both faces; a slip fit on the pipe (§8) |
| ground edge | 1/8" × 1-1/2" aluminium angle | 1 × 21" | one leg screwed to the ground panel's flush end, the other out past it as the lip (§8) |
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

- **Two panels, not one.** The brief's identical panels needed a 3/4
  spacer under one rail of each so the rail lugs could sit side by
  side on the pipe, which put a 3/4 step in each rail at the joint.
  Instead the two halves differ only at the hinge. Panel A's rails
  are flush on both sides and carry their lugs in one piece. Panel B's
  rails are plain and flush, with a lug stub (an 8 x 5-1/2 plate with
  the half-round, lapping the rail's lower 2-1/2 in) glued and
  brad-nailed to the outside of each. On the pipe each end has A's rail lug with B's
  stub beside it, 24 over the lugs, 22.5 over the rails, no step. The
  interior lugs sit against different ribs on the two panels (A's
  against the ribs at 6 and 13.5, B's against those at 1.5 and 19.5),
  so B's land outboard of A's and the handle between the inner pair
  is 8-1/4 in of bare pipe. No cheeks, no spacers; the pipe is 24-1/2.
- **Lugs one ply.** With the 1-1/2 radius the wall round the bushing
  seat is 27/32, and at the 465 lb a lug carries (the 1,860 lb pipe
  tension over four lugs per panel) a single 3/4 birch lug sees about
  370 psi in tension and shear-out and 470 psi of bearing from the
  bushing on its seat, the bearing the governing number at under half
  of what birch ply takes. The doubling was buying back the thin wall
  the radius already fixed.
- **Bushings.** Every lug bore carries a bronze sleeve, SAE 841
  oil-impregnated, 1-1/16 ID x 1-5/16 OD x 3/4: a slip fit on the
  pipe's 1.050 and a press fit in the lug, flush with both faces, so
  the pipe turns in bronze rather than wearing the ply's end grain and
  the bearing on the wood is spread over the sleeve's larger diameter.
  The lug bores are 1-5/16 for the seat, bored in place through the
  rail bores as before so all eight are coaxial; the sleeves are
  pressed in afterwards. In the model each panel has a `bushing`
  studio of four bodies at its lugs' spans, placed with the rest, and
  the hinge section cuts one with its lug.
- **Ribs from the birch, and brads.** The ribs and cross blocks are
  3/4 birch strips on edge, ripped from the sheet the rails and lugs
  come from, not 2x2s: straight, stable, exactly 3/4 wide, and about
  1-1/2 lb lighter per panel; as shear webs they are good for about
  the same as a 2x2 of spruce, and the skins carry the bending. A
  screw into the edge of 3/4 ply holds poorly, so the box is glued
  and brad-nailed throughout (skins to ribs, lugs to ribs and from
  the skin into the feet, stubs to rails) and weighted while it
  cures; the only screws left are the pipe caps. The joint block
  stays a 2x4 (the lugs bear on it) and the stubs 2x8 (the end plates
  clamp 1-1/2 x 7-1/4).
- **Lug feet against ribs.** Each interior lug's foot lies against the
  side of a rib, brad-nailed to it through the faces and from the
  bottom skin, the top skin solid. The pull on a lug bears on the
  joint block (about 210 psi on 1.1 in²), so the fasteners hold the
  lug square during glue-up and are the insurance if a glue line lets
  go. The ribs are at 1.5, 6, 10.5, 13.5 and 19.5 on both panels.
- **Pin drop and lug radius 1-1/2**, not the 1 the brief assumed,
  which answers the first open question in §7: the 7/16 wall a 1 in
  radius leaves is one pin diameter of end distance, and the radius
  cannot exceed the pin drop without the half-round meeting the other
  panel's bottom. The lever arm is 3.875, the folded bottoms 3 apart,
  the package 9.75 thick, the lugs 3 below the deck at the joint, the
  rail taper 8 in long.
- **Parts.** One studio per part, in inches through `IN = 25.4`, every
  part built in place in the panel's frame; the skins, ribs, cross
  blocks and joint block are shared by both panels, the bottom skin's
  notches excepted. The stubs differ: panel A's run 3 in bare past
  the skins for the end plates, panel B's stop flush with the skins'
  end at 45 and carry the bevel feature. The rails with lugs, the lug
  stubs and the interior lugs are a studio each because a kernel
  `add` joins every body in the studio and each needs a half-round
  added to its profile.
- **Panels and ramp.** `panel A` and `panel B` are assemblies of fixed
  instances; the ramp places B turned 180 degrees about z and moved
  21 in x. Fixed placement rather than fastened mates, since a panel
  is one glued box and a mate per part would say nothing.
- **Fold.** One revolute, `fold`, between the pipe and panel B's left
  lug stub bore, its parameters read off the drawn pose. The fold goes
  under: B's free end drops, and at 180 degrees it lies under A,
  bottoms 3 in apart, lugs interleaved. The bottom skin's hinge-end
  edge has the 3/8 chamfer and the test finds no interference at 0,
  5, 10, 20, 45, 90, 135, 170 and 180 degrees.
- **Tunables.** `skin`, `rib`, `curb`, `pin_drop` and `lug_r` are
  document variables; the skin and rib extrude depths are bound to the
  first two. The rest live in sketched profiles, which this script
  redraws from the constants at the top.
- **Hinge section.** The section is cut through the middle of panel
  B's first interior lug from the right wall (at 18-3/4), so the
  hatched profile is that lug, pipe and all, with panel A's lug seen
  behind it. The sheet draws the three lugs at the cut (B's interior
  lug, A's behind it, B's lug stub) by themselves beneath the section,
  in the same folded pose.
- **Cut sheets.** Two assemblies lay the real bodies flat on a 4 x 8
  sheet, 96 along x: the CDX in two 21 in rips, a top and a bottom
  skin each; the birch in rips along the sheet, the two rails with
  lugs (6-3/8 wide, 46-1/2 long) end to end, the plain rails, then the
  four interior lugs and two lug stubs in a 5-1/2 rip, then 1-1/2 in
  strips for the ribs and cross blocks packed first-fit by length,
  23-3/4 in of the sheet's 48 in all. Each piece sits a kerf from the
  next; the script checks every piece lies flat on and inside its
  sheet before drawing it, and the test that no two overlap. The ply
  bodies carry their names and material (CDX 0.55, birch 0.68 g/cm3),
  so the parts lists read "rib · 1.5 x 32.5" rather than "Part 1".
- **Bevel and angle.** The ground stub's 30 degree bevel is a
  suppressed feature: on in `out/stub.pdf`, off in the ramp. The
  ground angle, 1/8 x 1-1/2, is a ramp-level part on panel B's flush
  end, the end plates ghosts on panel A's bare stubs. The angle's one
  leg lies flat against the end face (the skins' edges and the stubs'
  ends) and is screwed into the stubs' end grain, four #10 x 2, two
  per stub, one high and one low, both rows below the top skin, which
  is why the leg is 1-1/2 rather than the 1 the brief had; the other
  leg runs out past the end flush with the top skin, the lip the
  wheel rolls off. The ramp shows it on the square end; with the bevel
  cut across the whole end, that face stands plumb on the ground and
  the lip lies flat on it.
- **Outputs.** `out/ramp.okpart`; `ramp_iso`, `ramp_front`,
  `ramp_side`, `ramp_below`, `ramp_angle` (the ground angle on panel
  B's end), `ramp_folded`, `ramp_folded_side`,
  `panel_a_below` and `panel_b_below` (lugs, notches, stubs),
  `panel_a_cutaway` and `panel_b_cutaway` (from below with the bottom
  skin cut away: the lug feet against their ribs, the joint block, the
  top skin intact), `ramp_lug_section` (through a lug on the pipe
  axis) and `ramp_hinge_section` (folded to 90 degrees, cut between
  the right wall and the first interior lug and fitted to the hinge:
  the two panels' lugs passing each other on the pipe, each its full
  length) with `hinge_section.pdf` the same cut as a sheet, hidden
  lines dashed, so the cut lug's foot shows inside its box and where
  panel A's lug passes behind it, and the three lugs at the cut drawn
  by themselves below it, each with its bushing; `cut_sheet_cdx.pdf` and `cut_sheet_birch.pdf`,
  the plywood pieces laid flat on their 4 x 8 sheets with a 1/8 kerf
  between, numbered against a list; `panel_a.pdf` and `panel_b.pdf` with a section across the
  width at mid-length and one along the length through a lug;
  `ramp.pdf` with each panel as one item and the pipe as one;
  `stub.pdf`; `fold.pdf` and `fold.png` at 0, 45, 90, 135 and 180
  degrees from the side; `cutlist.csv`.
- **The test** regenerates every part closed and places every
  instance; finds the eight bores (a rail lug or stub and an interior
  lug on each side of each panel) and the eight bushings in them on
  the pipe's axis within a hundredth of a millimetre; checks the interior lugs one ply, B's
  outboard of A's, none overlapping, each against a rib of its own
  panel, the handle 8-1/4 in bare, and A's rail lug beside B's stub at
  each end of the pipe; sweeps the fold; measures the open ramp at 96
  long and 22.5 over the rails, the folded package 24 over the lugs
  and 9.75 thick with the bottoms 3 in apart and the lugs filling the
  gap; and reads the sheet yield off the parts: the four skins are
  82 % of one 4 x 8 (two 21 in rips crosscut at 45), the birch, ribs
  included, about 40 % of another.

The remaining open questions in §7 stand: the curb height, the rail
taper against a full skid, the end plates with the rail ending 3 in
short, and the rib pitch.
