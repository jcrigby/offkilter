# Decisions

Why things are the way they are, with the alternative that lost. One
entry per decision, newest last. Add to this when a design choice is
not obvious from the code, and especially when an approach was tried
and rejected: the next session will otherwise re-derive or re-argue it.
Dates are when the decision landed on `main`.

## Who this is for

One person building real shop projects (a router lift, a series of
puzzle boxes with jigsaw tops) drives the tool, mostly through the MCP
server from a chat, with the web app as a viewer and editor. Features
come from those projects, not from a feature list; a feature is worth
having if one person will use it. The kernel, by contrast, has to be
general: geometry does not care whose project it is, and every
robustness fix so far came from a real part. Build the kernel as if
everyone will use it, the features as if only you will, and let the
conversation carry the generality a menu system used to.

## 2026-09 · Puzzle tops: fabrication layouts, not a row gap

A jigsaw checkerboard top is pin-routed from one board per colour.
Same-colour pieces in adjacent rows meet at corners, and a pin router
bites a quarter disc of two bit radii out of each corner it turns, so
routing each colour from one board leaves the corners wrong on a tight
design. The first fix tried was a gap between rows in the design
itself; it was rejected because the strip sides then carried no tabs
and the design view showed a trough that is not part of the puzzle.
The design view now shows the plain jigsaw with the design gap, and
`show: light | dark` fabrication layouts regenerate one colour's pieces
with rows spread one bit diameter apart so same-colour corners route
clean. The alignment web (half-height connectors between pieces) stays
for the design; a printed fixture (a tray with pockets grown 0.15 mm)
replaces it where the layouts make it impossible. Noted for later:
veneer stock would let both colours come from near-identical sheets for
a one-species look.

## 2026-09 · Shop sheets are written by the kernel, as PDF, with no dependencies

Drawings were client-side SVG only. Sheets now come from `ok-sheet`, a
kernel crate, so the MCP tool, the server route and the client share
one layout, and the PDF writer is a few hundred lines rather than a
dependency (Helvetica, WinAnsi, dashed hidden lines, an xref table).
Content streams are left uncompressed: every test greps the stream
text, and deflating would mean inflating in each test. The router
lift's assembly sheet is 65 KB without hidden lines; it was 930 KB with
them, which is what prompted the next two decisions.

## 2026-09 · The sheet fits around the parts list and lifts the iso view

The first fit subtracted the whole parts list height from the sheet,
which put the router lift at 1:10 with a list of 21 rows. The fit now
uses the full free height, checks that no view's box lands on the list
in the bottom right corner, and when the iso view alone is in the way
lifts it (its height on the sheet is free; the three orthographic views
are not). Balloons count toward the fit, spread apart around the iso
view so none overlap, and skip the arc of directions that would land
one on the list. The alternative of shrinking the drawing until
everything fits was rejected because 1:10 on A3 was unreadable.

## 2026-09 · Sub-assemblies are one item, and a connector names its member

An assembly sheet was still busy at 21 items. A sub-assembly instance
is now one item with one balloon (anchored at the volume-weighted
centroid of its bodies) and one parts-list row, and gets its own sheet
from its own tab; hidden lines are off on any sheet of more than one
part unless forced. Making the router lift into three sub-assemblies
exposed a kernel gap: a mate onto a sub-assembly took the first body of
the group that had the face, which two studios with alike feature
numbering can get wrong silently. A `Connector` now carries `sub`, the
member instance that holds the face, the resolver narrows to it, and an
unknown member is an error rather than a guess. The client sets `sub`
from the picked body. The scripts had to learn that the kernel builds a
member's connector frame on the body as placed inside the sub-assembly
(canonical x from the placed axis), not on the body-local frame rotated
afterwards: the two differ by the member's rotation about the axis.

## 2026-09 · Working agreements live in the repo, not the session

A Claude Code session on another machine starts from the files, not
from the conversation that shaped them. `CLAUDE.md` carries the setup,
the branch-and-PR-per-increment workflow, the verification chain and
the example regeneration rule; `.claude/settings.json` carries the
permission allowlist and the Stop hook that refuses to end a turn with
uncommitted or unpushed work; `Dockerfile.dev` pins the toolchain. This
file carries the reasoning. A raw transcript was not kept: it is mostly
build output and CI waits.

## 2026-09 · The router lift is verified at its limits, and the SCAD is not the last word

A design brief asked for the lift and the pin arm to be checked as
mechanisms: the carriage swept over its travel, the arm over its hinge,
clearances and alignment measured, every pair of placed bodies checked
for interference at each position. That lives in a test
(`crates/ok-render/tests/router_lift_limits.rs`) rather than the build
script, so the measurements run in CI and print with `--nocapture`.
Overlaps the SCAD draws deliberately (a knuckle let into a corner, press
fits) are listed with their reason and reported, not hidden; a genuine
flaw is reported as a finding and left in the model, because the SCAD
is the design of record and the point of the check is to show what it
missed. The first run, against the rev C hinged arm, found that the
hinge binds at 5 degrees; the rev D arm on a shaft pivot swings clear
to 80 and meets the table at 85 as intended, but its leveling bolt
stops the lift from the first degree, the guide pin runs into the
nose, and the crank nut stands in the stock envelope. The lift's SK20
and SC20UU studios serve the pivot too, rotated into new poses, rather
than duplicate parts.

## 2026-09 · The model may depart from the SCAD when the shop asks

The SCAD is the design of record and the checks report what it missed
rather than fix it silently. When the maintainer decides a finding is a
change to make, the model makes it and the example README lists it
under "Where the model departs from the SCAD", so the two stay
reconcilable. First case: the crank nut, 31 mm proud of the table in
the SCAD and in the stock envelope, is recessed into the top the way
commercial lifts do it, with the ply left above the bearing pocket
checked (11 mm) rather than assumed.

## 2026-09 · Measuring from a photograph is a printed sheet and four marks, not a ruler in the frame

Some parts of a shop project exist before the model does: a router
clamp, a bearing, an odd bracket. The first idea was to photograph them
next to a ruler; it was rejected because a ruler gives one scale along
one line and nothing about the camera's tilt. A printed sheet with four
bullseye marks at known spacing gives a homography instead, so a phone
photo taken roughly from above is squared up onto the sheet's
millimetres, and the grid on the sheet is only for people. The marks are
concentric rings rather than squares or QR codes because nested
components are found with a threshold and a connected-components pass,
which keeps `ok-photo` dependency-free apart from a JPEG decoder, and
the origin mark carries one extra ring so the corners come back in a
known order. The result is deliberately modest: top-face silhouettes,
good to a fraction of a millimetre on flat things, with a stated
parallax caveat. Its job is to give a first sketch and a picture to
point at, so the next question is "what is the hole at (65, 55) with
calipers", not "describe the part". The crate has no server route or
client yet: the tool exists for the MCP session that was asking for
measurements.

## 2026-09 · The sheet names its size with dots, and a photo scale's bars name the print scale

The first sheet needed the caller to say A4 or Letter, and a Letter
print read with the A4 spacing comes out 8 % wrong one way and 4 % the
other with no warning. A QR code was considered and rejected: decoding
one is a Reed-Solomon library and a mask search for five possible
answers. A row of one to five dots beside the origin mark, read in the
origin's own frame (its ring for scale, the x mark for direction) so
the count is known before the size is, does the same job in twenty
lines with the component pass the marks already use. Printing at
exactly 100 % is the other assumption nobody can check from the
picture, since the sheet's own bar scales with it. A calibrated thing
on the sheet fixes that, and the forensic photo scale (ABFO No. 2,
accurate to 0.1 mm, alternating 10 mm black and white bars) is what
the tool reads: an even run of at least three alike dark blocks whose
pitch says how far the print is from 100 %. Reading the scale's
millimetre graduations was rejected for now because 0.2 mm strokes do
not survive a phone photograph at 0.2 mm per pixel; the bars do. A
coin does the same job less accurately (its edge shadow adds to its
diameter) for anyone without a scale. Once the factor is known the
picture is rectified again in true millimetres rather than corrected
afterwards, so the outlines, the sketch and the drawn grid all agree.

## 2026-09 · A steel rule is read by fitting its ticks, and a scan needs no sheet

The maintainer's reference is a steel rule, and the parts are scanned
on a flatbed rather than photographed. A scan changes two things: the
sheet cannot lie under parts that lie on the glass, and the picture is
sharp enough (0.085 mm per pixel at 300 dpi) to read millimetre
graduations, which a phone photo is not. So the rule is read directly:
its ticks are the thin marks darker than their surroundings (local
mean, in proportion, so a light body against white paper does not
read as a mark at its own edge), the rule's direction is the one most
ticks share, the ticks whose bases line up are one edge, and a
straight-line fit of position against index over 100 or more ticks
gives pixels per millimetre to a tenth of a percent. Reading the
numerals was not attempted: an inch edge is told apart from the metric
one by the ratio of the two edges' pitches, which is all the numerals
would add. With no sheet marks in the picture the scan is measured in
its own frame from the rule alone, through the same rectification with
the picture's corners standing in for the marks, rather than a second
code path. Two lessons from the synthetic tests are in the code: index
ticks from their neighbours, not from a rough pitch that drifts a whole
tick over a long rule, and cluster ticks by their ends rather than
sliding a window over them, since a window over tied ends takes an
arbitrary subset and the gaps look uneven. A third came from the
phone-photo test: a tilted picture is foreshortened one way, so the
rule's pitch, read along the rule, is compared with the marks' scale
along the rule's direction at that spot, not with an average of the x
and y scales, which was 3 % out on a picture squashed 5 % one way.
The rule does read in a phone picture that fills the frame with the
sheet; the assumption that only a scan resolves the ticks was wrong.

## 2026-09 · Sections on the sheet are two fixed cuts with a position, and the extra views may go in a second row

The top of the router lift got features on its underside that no
standard view shows, and the maintainer asked for two sections at
right angles. A real package lets the author place any cutting plane;
here the sheet offers exactly two, `section` parallel to the front
view and `section-side` parallel to the right view, each with `@<mm>`
for where the cut goes and through the middle of the bodies without
it, because those two cover every part in the project and a free plane
would need a way to draw and dimension its trace in a general view.
The cut faces are hatched by the even-odd rule over every loop, so a
face with a hole in it stays open, the section is captioned, and the
plane is traced with arrows for the direction of sight on the top view
(or the elevation the plane is edge-on in when there is no top view),
which is the convention the client's SVG drawing already used. Two
sections made five views, and five views in one row put a 400 mm top
at 1:5 on A3. The layout now tries the extra views in a row under the
elevations as well as to their right and keeps whichever fits the
larger scale, and 1:2.5 joined the standard scales, which the roadmap
had wanted since the carriage assembly dropped to 1:5 for want of a
few millimetres. The client's drawing dialog still has its own single
section; matching it to the sheet's two is on the roadmap.

## 2026-09 · Range of motion is positions drawn side by side, not an animation

Showing a mechanism at its limits and in use is the first thing every
mechanical design needs, and the next project (puzzle box mechanisms)
will need it more than the router lift. The client has a mate
animation, which is right for a screen and useless on paper or in a
chat. `range_of_motion` takes a list of positions, each setting one
or more mates to absolute values, and draws the assembly at each, side
by side: a PNG strip for the chat, every frame fitted to the same box
so the fixed parts stay put and only the moving ones move, or a PDF
sheet with the view at each position captioned, which the sheet
layout already knew how to place. Positions are given, not derived:
the tool does not know which end of a slider is "lowest" or what
"typical" means, and the limits test, which does know, names them.
Overlaying the positions in phantom lines on one view, the drafting
convention, was set aside for now because three overlaid assemblies of
twenty parts are unreadable; it would suit a single moving part on an
otherwise still drawing and can come when a drawing asks for it. The
kernel gained the one thing this needed, a preview with several mates
set at once, which the bit-change check had worked around by editing
the document.

## 2026-10 · The cart starts from its speeds and loads, and size is strength only where the torque is

The EGO cart is laid out from three numbers before any geometry: the
head's 4800 rpm, a walking pace of 1.3 m/s on 254 mm wheels, and
80 kg up a 15 % hill. Those give 50:1 and 17 Nm at the axle, and the
head has ten times the power the hill needs, so the gearing is set by
pace, not torque. The advice that printed parts get strength from size
was taken for the two spur stages, which carry the torque, and refused
for the first stage, which carries 4800 rpm and almost nothing: a
24 mm pinion there already runs 6 m/s at its pitch line, the limit for
printed plastic, so that stage is kept small and may be bought. Every
shaft is parallel to the axle and at its height, in a row behind it,
so the gearbox is long and low and the platform sits over the tyres
rather than over the gears. The rear wall of the housing is cut square
to the input axis so the drive shaft's bore and the tube's seat are one
hole drilled normal to a face, which the kernel can do, rather than an
angled hole, which it cannot. The gears are discs at pitch diameter and
the housing a hollow box: the increment is the layout and the numbers,
and teeth, bearings and the clutch each get their own. Hollowing the
housing turned up a kernel bug, any hole into a box with an enclosed
cavity, recorded as an ignored corpus test; the build drills before it
hollows.

## 2026-10 · Gears are a feature, not a sketch

A gear could have been a sketch op that draws its teeth as lines and
arcs, the way `add_polygon` and `add_slot` work, and it would then
extrude, pattern and mate like any sketch. It is a feature instead
(`add_gear`), with the profile built straight from module, tooth count
and pressure angle at regeneration. Two reasons. A gear with eighty
teeth is three thousand sketch points, which the constraint solver
would carry on every edit for nothing, since no one constrains a tooth.
And the numbers are the design: changing the tooth count of a feature
regenerates the gear, where a sketch would have to be redrawn, and the
rules a gear should refuse (undercut, a ring's tips inside its base
circle, a bore under the roots) are checks on numbers, not on drawn
geometry. The flanks are sampled involutes tagged as one surface each,
so a gear's report has four to six faces per tooth, and its bore is a
cylinder a mate can use. The tooth form is the theoretical one with no
root fillet and no profile shift; the first pinion that needs either
will add it.

## 2026-10 · The cart becomes a tricycle, and its stability is a number in the test

Rev A of the EGO cart drove a live axle through a printed gearbox,
which needed wheels fixed to the axle, a differential or scrub in
turns, and a printed first stage at 4800 rpm. Looking at what can be
bought changed all three: standard wheels come on bearings for a dead
axle, a worm gearbox turns the drive 90° and reduces 20:1 in one
casting, and a bicycle hub's six-bolt disc mount is a machined bolt
pattern for a printed ring gear. So rev B is one driven bicycle
wheel with the ring on its hub, the worm box's pinion inside the
ring, and two casters: no axle, no differential, the big printed gear
where the torque is. The flex shaft between the EGO stub and the worm
box is what lets the head sit where the hands want it.

The tricycle's price is stability, and the first layout, casters just
ahead of the drive wheel on a wide track with the load near the
wheel, looked stable and was not: the support triangle is a point at
the drive wheel, and the mass centre 37 mm ahead of it tipped at 5°
of side slope. The model now computes the mass centre from the
placed bodies and a mass table, and the test asserts the drive
wheel's share, the two-bag and one-bag tip angles and the mass
centre's position on the hill; those numbers moved the bags 250 mm
ahead of the wheel and the casters 300 mm ahead, widened the track,
and put "one bag goes over the wheel" in the README. The lesson is
general: a layout question like this one is a few lines of statics
on the model's own centroids, and it belongs in the regression test
next to the interference check, which found five collisions in the
same revision.

## 2026-10 · Drill before you cut

The cart's 265 mm ring gear prints as six sectors with a dowel across
every joint. The first attempt cut the ring and then drilled each cut
face, finding the faces in the report by their angle: every hole
changed the body, the references taken before the first hole went
stale, and sectors came out with one hole or none. The holes are now
drilled through the whole ring at the six joint angles, 16 mm long and
centred on the cut, and the cuts come after, leaving half a hole on
each side. Placing by angle on a known body is dependable; chasing
faces across a sequence of edits is not, and a build script should
prefer the former whenever the geometry lets it.

## Photo measuring runs in the wasm, not behind a server route

The roadmap had a server route and an upload for measuring a phone photo
from the web app. `ok-photo` is a kernel crate (pure Rust, no I/O) and
already built for wasm32, so the client measures in the browser: the
photograph never leaves the phone, and the app works the same from a
static host or a local file as against `ok-server`. The cost is about
200 KB of wasm for the JPEG decoder and the rasteriser that draws the
squared-up picture, and a 12-megapixel photo takes a second or two in
one thread. A server route would have kept the wasm small and let the
server cache sheets, but every other reading tool (DXF, STEP, STL) is
client side too, and a route would have been one more thing that only
works signed in.

## The puzzle's resin gap is the kerf

The gapped top was going to be pin-routed piece by piece from a
template layout with the rows spread apart. The pattern plate replaces
that: the gap lattice printed as grooves, a pilot the size of the gap
riding in them and a bit of the same size beside it on one slide, so
the kerf is the gap and all 48 pieces come off one board in one sitting,
in their places, with no offset between template and cut. It needs a
gap, which the resin top has and the tight top does not, and a bit as
thin as the gap, which is why the stock should stay at 10 to 12 mm and
the gap can go to 2 mm. The tight top keeps the template route. The
plate is its own layout (`show: "pattern"`) rather than a body in the
design, so the design's drawings and template DXF stay what they were.
The plate carries a reference hole the size of the gap 5 mm inside its
front-left corner, outside the board: with the pilot in it the bit sits
5 mm outside the blank's corner, which is how the blank's fence on the
duplicator is set, so the hole lives in the plate and not in a jig.

## 2026-10 · The Spectre top is a kernel feature, and its mystics are the dark wood

The rounded Spectre's edges alternate in and out like a jigsaw's, so a
patch of it under the puzzle box's board is a puzzle top. Building it
as sketches through the MCP server was tried first and rejected: each
piece is a polyline of some four hundred points, so a top is thirty
thousand sketch lines in the document, slow to draw and to find
regions in, where the checkerboard stores a dozen numbers. It is now
`add_spectre`, holding the board, the edge, the gap, the curve and a
shift, and `ok_sketch::spectre` lays the shallowest patch that covers
the board, erodes every tile the board reaches by half the gap and
clips it to the board with the region finder, so a tile the edge cuts
in two is two pieces. Polylines rather than arcs for the outlines: the
curve is a cubic, a biarc with the same end tangents is a flatter
curve, and the pattern plate and the DXF do not mind. Two woods cannot
alternate on this tiling (three tiles meet at a vertex) and three
cannot colour it properly (a tile with seven neighbours round it needs
four), so the pieces follow Kaplan's mystics scheme instead, the pairs
that stand in for a ninth supertile dark and the field light; on a
gapped top the resin outlines every piece anyway. The web is the one
connected lattice: a gap the board's edge closes off inside a piece's
concavity, where the neighbour beyond the edge had a lobe, is left to
the resin, while the pattern plate grooves it like the rest. The
region finder's point merging and crossing checks were every pair;
they now go through a grid, which the lattice needed and every sketch
gains from.

## 2026-10 · A piece routed to its template is shaped by the bit, and its mate agrees

The 4 in box's top is cut piece by piece with a 3/16 in flush trim
bit, each piece the template for its own blank. A flush trim bit
follows a template exactly except where the template turns inside a
circle the bit's size, where it leaves a fillet of its own radius; so
the design has to draw what the bit will do, or the pieces will not
nest. Two rules came out of it. The edge curve must clear the bit's
radius on its concave side everywhere, which Kaplan's cubic does only
above a 10 mm edge or so (its apex is 0.28 of the edge), so the feature
grew a circular-arc edge, one radius all along, 0.57 of the edge at the
sagitta that meets the corners at Kaplan's angle; the cubic stays the
default and a curve tighter than the bit is refused rather than drawn.
And the tile's four inside corners, which sit only ever at vertices of
two tiles, are filleted to the bit and the convex corner across the
vertex is rounded to the same circle, found by bisection on the two
shared curves, so what the fillet adds to one piece the rounding takes
off its mate and the pieces still fill the board. Convex corners at
vertices of three or four tiles stay sharp, since the bit cuts those as
drawn. Rounding every convex corner was considered and rejected: it
leaves a small void at every three-tile vertex for no gain.

## 2026-10 · Spectre woods are a scheme over the whole patch

The mystic scheme was the only one, because two woods cannot alternate
on this tiling. Asked for one wood, two or three at random and a mix,
the feature grew a `scheme` with a seed and two more woods. The
colouring is computed over every tile of the patch, not over the
pieces the board keeps, so sliding or turning the board changes which
tiles it gets but not what colour each tile is: the plan stays
readable while the board is moved. The four-wood schemes are a DSatur
map colouring (most-constrained tile first, ties by degree then by the
seed), which four woods always manage on these patches in practice,
with a few re-draws against the order's bad luck rather than a proof.
Weighted DSatur (8:4:2:1) gives the "mix" the look of a mostly-light
top with the fourth wood rare, which a plain random draw cannot do
while keeping neighbours apart. The pieces are named by their slot
(light, dark, third, fourth), not by the wood's name, so a document
whose woods are unset still names its pieces, and the two-wood
examples keep their names.

## 2026-10 · The Spectre's rips are a feature parameter, and the saw's rules are the kernel's

The plan for a Spectre top in two woods is to rip strip profiles on the
table saw, glue them into straight Spectre extrusions and cut the tiles
from those, so what matters is which few profiles a tile divides into.
The rip planner started as a page beside the app; it moved into the
kernel as `ok_sketch::spectre::rips` so a chosen set of cuts is part of
the document (`rips` on the feature, in tile units) and every tile of a
top splits by it, with a wood per profile, instead of living in a
browser tab. Cuts run only along the 30° lines through the tile's
vertices: every edge of the tile lies on one, so pieces cut that way
have faces at 60°, 90° or 120° to each other, which is what a saw tilted
to 30° at most can make; a free-angle cut would make a profile nothing
rests square for. The saw's reading of a profile (through rip, rabbet,
dado, nothing) is the kernel's so the search and the panel agree, and
so a decomposition can be tested. The search is bounded at four cuts
from the inside corners plus one from any corner: the straight tile
has four inside corners, none sharing a line, so through rips need all
four cuts and five pieces, and one more cut is enough to bring the kit
to two profiles (two of one, four of the other); a wider search was not
worth its seconds in the panel. Rabbets are allowed as a ranking, not a
default, since every rabbet is a second setup per face. Mirror images
are one profile because stock just flips over.

## 2026-10 · The stack cage has no rails, twelve indices and a ball-nose bit

The dry stack for the Spectre pieces (`examples/spectre-cage`) began
as a hex tube with corner rails carrying the panels in grooves. Rails
with any foot along the blank's faces shade the blank's corners from
every window, and the bit then cannot reach the profile's extreme
points nor remove the corner material around them: with 4 mm feet a
third of the outline was out of reach. So the panels are mitred and
meet at the corners, the rods lie in the V between adjacent panels'
outer faces and hold them in, and the window is the whole face. Six
flat indices still miss the tile's deep bays, which open toward the
hex's corners wherever the profile is turned; a V cradle the hex sits
in on a corner adds the six corner directions through the same
panels, and with twelve the clocking search reaches the whole outline
at a 48 mm hex. The bit for the curved tile is a ball nose, not the flush trim bit
the spectre-box pieces were drawn for: a flat bit on a duplicator
leaves only vertical walls and flat floors, and `duplicator_check` on
the master from above shows the profile's curves as corners it cannot
make, where a ball of the same radius follows them, the fillets being
its own. The reach rule is the ball's: the ball on the edge, its
shank's column swept to the window clear of everything. The straight
tile is the other way round: every edge of it lies on the 30° grid,
so with twelve indices each edge can be presented parallel to a
square-end bit or flat to its floor, and that bit makes the tile
exactly, sharp inside corners and all, where a ball would fillet
them. The example carries both masters for one cage and stack, each
with its own finned panels, and the reach check has a rule per bit. The master is
one shape rather than the tiling's pieces, which have the convex
corners at a vertex of two tiles rounded to seat in a mate's fillet
and the rest sharp: a stack routed as one prism can only be one
shape, so `round` rounds all ten or none. All, to the bit's radius,
is the softer knob the template-routed pieces show and costs a sliver
of resin at each vertex of three tiles, which a top with a gap has
anyway; none is the Spectre's own outline. The cage example rounds. Fins are kept to the hex's six sectors, so they never
meet whatever the profile, and stop short of it sideways as well as
down, since a sampled envelope with clearance taken only along the
column cut into a vertical wall between samples, which the fin test
found by boolean.

## 2026-10 · The saw station's blank is a rectangle, its rabbets overshoot, and its offcuts are captured

The table saw version of the straight Spectre stick
(`examples/spectre-saw`) was going to start from the cage's hex blank
and index it in a cradle at every 30°. Reading the tile's faces as a
saw does changed both: six faces are through rips, eight end at an
inside corner, none lies between two corners, and of the through
faces two are parallel with a third square to them. A blank planed to
that thickness and ripped to that width carries three faces from the
start and the rest is ten rips, and since every cut is a printed
cassette that holds the stick at its own angle, no cradle or index is
needed at all; the hex, the rods and the twelve indices belong to the
duplicator, where the bit has to come at the blank from outside it.
The sequence is simulated with a polygon clipper rather than reasoned
about, because the inside corners turned out to need it: a rabbet is
two stopped cuts meeting at the corner, each kerf ending square to its
own face, and a blade that stops short of the corner by any amount
leaves a sliver of waste of that size in the corner, at the 90° ones
as a square and at the 120° ones as a wedge, which a first guess at a
kerf's worth of tolerance missed. A blade past the corner only nicks
the far face, a kerf wide and the overshoot deep, under the gap the
top has anyway, so the program aims past by the setup's error (`bias`)
and the test holds the section to the tile less exactly those nicks.
The kicker wedge behind the blade went the same way: the cassette's
lid comes down to the stick's upper envelope on both sides of the
blade, with a groove over the blade's sweep, so an offcut is held
where it was cut until the lid comes off, which is safer than
directing it anywhere. The cassette is the negative of the stick with
its open kerfs filled in, since a rib into a kerf would be a kerf
wide and hold nothing, and the stock after the first cut is already
chiral, so from the second cassette a stack put in end for end, which
would mirror every piece, does not fit; the first cassette cannot tell
and the leading plug is marked instead.

## The measuring field erases a mark's ink only where the paper round it shows

A field's marks sit where parts will lie, so the measuring pass cannot
keep clear of them the way it keeps clear of a sheet's four corners: a
keep-out circle round every mark would punch holes in any part across
one. The marks' ink is erased from the picture instead, ring by ring
and dot by dot where the lattice says they are. Erasing blindly notches
a part wherever a dot sits on its edge, and under a part there is no
ink, only part, so each element is judged by a halo sampled just
outside it: paper all round, erase; part all round, leave it, there is
nothing to erase; a mixture, the element straddles an edge, so erase
and close the notch locally. What is left is a part lying across a
mark reading its true size to a few tenths, with at worst a rounded
corner or a flat of a fraction of a millimetre where a dot sat on its
very edge, and that is the price taken over the alternatives: a lighter
ink the camera might not see (the marks must read at arm's length on a
phone), or marks far enough apart to be avoided (then too few are in
view when a part covers the middle of the picture). The lattice and the
codes were chosen together: four bits each way is sixteen columns and
rows, three Letter tiles across and five up, more than a workbench.

## The laser's plane comes from the line on the paper and the turntable's axis

The scanner's one unknown beyond the camera is the laser's plane, and
the obvious way to find it, fitting a plane to where the line's pixels
land on the field, fails: on the paper every pixel lands on the table
by construction, so the fit returns the table, and on the object the
landing points are what the plane is needed to find. What the picture
does know is which of the line's pixels are on the paper: there the
line is straight in the picture, so the straight line most pixels lie
on, found by RANSAC, separates them from the object's. Those pixels
meet the field's plane at known points, which in the camera's frame
(the rig is fixed; only the table turns) are the same line every
frame, so they give the plane one direction. The other is the
turntable's axis: the camera's centres over the turning field lie on a
circle about it, and the laser is set up to be aimed through it, so
the axis lies in the plane. The plane is fitted to the paper points
and samples along the axis together, and the scan reports how far the
line really passes from the axis, which is the one thing the setup can
get wrong; a few millimetres off bends the whole profile, and a
reported number beats a silent one. The alternative, a calibration
object of known shape scanned first, would work without the aiming
rule, but it is one more thing to print and keep, and the gourd
scanned in the closed loop stands in for it in the tests.

The scanner was written against its own simulation before any video
existed. A rendered gourd of known shape costs a depth buffer, a shadow
map and a shade of green, and in return every stage has a true value to
be held against: the focal length, the axis, the laser's angle, each
point's radius and the volume. The frames also go through ffmpeg and
back as the example, because a phone's video is chroma subsampled and
compressed, and the line that was crisp in the PNGs comes back blurred
and dimmer on dark surfaces; that is what set the line detector's
adaptive threshold and its continuity filter, not the clean frames.

## A second scan is registered by what was done to the object, not by search

A turntable scan sees nothing the camera cannot, and that is less
than it looks: a camera 22 degrees above the table still sees a face
leaning out over it by nearly 70 degrees, since a face at the side
of the object faces the camera in azimuth even while it faces down,
so a dodecahedron or a bucky ball on a face scans whole in one pass
and so does the gourd's waist. What it never sees is a surface facing
down, the underside of a cap or a lip, and there one scan bridges
from the stem to the rim with a cone (the mushroom in the tests comes
out 12 per cent over). The remedy is a second scan with the object
turned over or rolled onto a side, and the two clouds registered. Registration is iterative closest point,
point to plane (point to point snaps the two scans' sampling lattices
together and stalls a few degrees short on anything with flat faces),
and it is seeded by what the person did: a flip, a roll, a move, or
nothing but the camera. A blind search over every rotation was the
alternative, and it would find the wrong answer as happily as the
right one on anything with symmetry, which is most of what gets
scanned; the person always knows which way up they put the thing. The
seeds are every 30 degrees of turn about the vertical (and of the
axis rolled about), scored coarsely and the best few refined, since
the ICP basin from a 15 degree miss is reliable and from 90 it is not.
The fit and the overlap are reported so a wrong placement shows as a
loose number rather than a quietly wrong model.

The mesh is built about the cloud's own centre line rather than the
turntable's axis. The single scan's profiles were first stitched
frame by frame about the axis, which is right only for an object
centred on the turntable: a tetrahedron set down by eye has its apex
17 mm off the axis, and above the height where the section no longer
holds the axis the profiles end. Sectors of azimuth about the mean of
the points cost nothing on a centred object and hold the off-centre
one, and they take a merged cloud the same way. What stays is the
representation's limit, one radius per direction and height, so a
torus or a cup with a handle is still not a thing this scanner
returns whole.

## A scanned polyhedron is its planes, not a mesh

The mesh the scanner stitches is a lattice of small triangles through
points resampled on a cylinder, and on anything with flat faces it
reads as what it is: every corner chamfered by the chord between two
frames, every face a quilt of slightly jittered facets. The points are
not the problem, they lie within a tenth of a millimetre of the face;
the representation is. So the scan is also read for its planes, one
at a time: from the unclaimed point whose neighbourhood lies most
fully on its own plane, that plane claims every point lying on it
whose normal agrees, is refitted and claims again, and keeps its
points if there are enough and their normals do not fan out as a
curve's would; each plane is least squares through hundreds of points
(good to a few hundredths of a millimetre), and the planes bound a
convex solid whose edges and corners are their intersections, sharper
than any point.
That solid goes into the document through the mesh op as a fan of
triangles per face, and regeneration, which already merges coplanar
facets, gives it back as one planar face per plane.

Two things in that pipeline are there because of how a laser scan
samples a surface. The points lie on lines, dense along each and a
centimetre apart across, and one line is perfectly planar on its own:
it lies in the laser's plane, bent or not. A normal estimated from a
neighbourhood that holds one line crossing an edge is the laser
plane's normal, and a patch grown from it runs down the line instead
of across the face. So a point's normal is never taken from one line:
it is the line's own tangent crossed with the step to the nearest
point of a neighbouring line, one that runs parallel (the same line
bent over an edge also offers points across the tangent, but their
tangent has turned with the fold), refined on the neighbours within
the tolerance of that first plane, the search widening until such a
line appears, since lines lie further apart out at the rim and on a
face struck at a slant. That keeps a point near an edge on its own
face's normal instead of a blend, and lets a face holding two lines
still be found. Region growing was tried first and dropped: it hangs
on where it starts and how far it reaches, and a merged cloud, with
a second scan's lines laid between or over the first's, broke every
setting of it; claiming by plane is indifferent to both. The
half-space intersection, rather than a general
polyhedron from faces, is the honest choice for what the scanner can
see: a turntable scan of a concave object is bridged across the
concavity anyway, and a convex solid is what the planes of one
determine. Corners where four or more planes meet fall out as a
cluster with a sliver edge between, since fitted planes never meet at
exactly one point, and are clustered to the point nearest all of them.

## The dove-proof perch is a gap, not a thin wire

`examples/finch-perch` adds a perch ring to the finch feeder cap for
the birds that cannot cling to the mesh, and has to keep the Eurasian
collared doves off it. The obvious lever, a perch too thin for a dove,
does not work: doves sit on telephone wires. The ring is still a
pencil's thickness (6 mm, what finch feeders use for pegs), but what
excludes the dove is where the ring stands: its inner edge 15.75 mm
off the mesh tube, so a bird on it facing in has its chest at the
tube. A finch is 25 to 30 mm through the body and feeds with its
chest on the mesh anyway; a collared dove is 65 to 70 and cannot lean
back to make the room. The cap above the ring stays the ring's own
size with 12.5 mm of air to the perch, so there is no tray to land on
instead. Rejected: a wider ring with a cage of bars over it (works,
but it is a different feeder); pegs instead of a ring (fewer birds at
a time, and a peg is a lever on a printed plate where a ring is a
hoop); a ring hung on wires below the cap (nothing to print flat).

It is a separate example rather than a flag on `finch-plug`: the plain
cap is what is on the feeder, its outputs and its test stay as they
are, and the perch builds through the original script's functions
(`build_cap`, `build_collar`, `add_assembly`), so the two cannot
drift apart. The spokes stand a millimetre proud of the plate because
a spoke flush with it shares its top face with the hook wall's
underside, overlapping it in part, and the kernel's union leaves open
edges on a partial coplanar overlap of opposite faces; a spoke that
ends inside the wall has no such face.

## The stylus adjusts to the bit, the router stays put

The duplicator's pilot was a 2 mm rod in a keyless chuck clamped by
its shank in a bore drilled beside the router's in one setup, and the
puzzle got by on that: a stylus-to-bit error there shifts the whole
copy and the slotted fence absorbs it. The Spectre cage does not: it
turns the master and the stack through twelve indices about their own
axes, so an error across the axis comes out doubled as a step between
faces cut at opposite indices, against a 0.7 gap and fins 0.3 off the
profile. So the pilot is now a stylus in a straight-shank ER11 collet
chuck in a block that moves in a pocket through the tool plate on
screws against springs, with a height screw over the shank, and the
router is the reference. Rejected: adjusting the router instead (its
clamp is a 66 mm bore in 38 mm of ply, and nothing about a trim router
wants to move in tenths); bottoming the cutting bit in the router's
collet to match the stylus (router makers say not to: the shank's
fillet ends up in the collet's grip and the bit creeps, and the height
screw makes the bit's length irrelevant anyway); opposed screws on
every axis (the far side of the x axis is 150 mm of plate from the
router's bore, so a spring there and one screw, then the head bolts
for stiffness); a dial indicator (the cones do the job, and the cut on
the spare blanks confirms it). The registration itself is two
identical printed cones rather than a measured spacing: the master's
cradles and the cage's are the same print, so a cone in each rim is
the same off each axis to the printer's accuracy, and a ball dropped
into one and the stylus into the other either sit the same way or do
not.

## 2026-11 · A fresh checkout sets itself up, and the pre-push chain says what it costs

A cloud session that started from a bare clone to build the lint
blower spent its first hour on setup: the MCP server failed to connect
because nothing had built `target/release/ok-mcp`, `cargo fmt` and
`cargo clippy` were not installed, the pre-push chain ran for half an
hour cold with no warning, and three conventions (the sketch planes'
frames, how an example is laid out, how to read a reference STL) had
to be rediscovered by probing. It wrote the list `docs/FRESH-CHECKOUT.md`
and the next session worked through it, one pull request per item
(#124, #122, #127, #123, #125, #126, #128, in the list's order), then
deleted the list; this entry is its record.

What changed and which alternative lost:

- **One setup script**, `scripts/setup.sh`, installs rustfmt and
  clippy, `wasm-bindgen-cli` at the version read from
  `crates/ok-wasm/Cargo.toml` (one pin, not two), the web app's npm
  packages and Playwright's Chromium, and the release `ok-mcp`, each
  step skipped when done and timed (219 s cold, 1 s warm). Rejected:
  leaving the three commands in `CLAUDE.md` (a session reads them
  after the server has already failed to connect) and relying on
  `Dockerfile.dev` alone (the cloud container is not that image; the
  image now runs the same steps and the script finds them done).
- **`rust-toolchain.toml` names the components** as well as the
  channel and the wasm target, so rustup installs them on the first
  `cargo` command, whether or not the script ran.
- **A synchronous SessionStart hook** builds `ok-mcp` when the binary
  is missing or older than `crates/ok-mcp`, `crates/ok-model` or
  `Cargo.lock`. Rejected: an asynchronous hook (the first prompt could
  still find no binary, which is the failure being fixed) and no hook
  (the environment's setup script is the right place, but a hook costs
  a stat when the binary is current and two minutes once when it is
  not). If a session still opens disconnected, `/mcp` reconnects.
- **`scripts/check.sh` is the chain**, in CI's order, printing seconds
  per step and running on after a failure so one run reports
  everything. Measured: 22 minutes cold, 18 warm, the tests themselves
  13 of that and the browser suite 3. Rejected: failing fast (on a
  20-minute chain a second run to find the next failure is the
  expensive part) and `cargo fmt --check` (CLAUDE.md's chain formats in
  place; the step fails when it changed a file, so the push carries it).
- **An example and its regression test may push after `--quick`**
  (fmt, clippy, `cargo test -p ok-render`, 4 minutes), the web and
  browser jobs left to CI, because files under `examples/` and
  `crates/ok-render/tests/` cannot reach the kernel, the wasm or the
  web app. Everything else runs the whole chain. Rejected: the whole
  chain every time (18 minutes to confirm what the file list already
  says; the session that could not wait pushed first and fixed up
  after, which is worse than a stated shortcut). The maintainer can
  reverse this by replacing one bullet in `CLAUDE.md`.
- **The sketch planes' frames are a table in `docs/OPS.md`** (plane,
  position at `offset`, sketch x, sketch y, normal; the sign of
  `rotated`; the canonical frame of a face plane). Rejected: a tool
  that answers the question (the report already carries each sketch's
  frame, and `Part.plane_point` reads it; what was missing was the
  convention written down).
- **`examples/README.md` is the checklist** for a new example, the
  `okmcp.py` client in ten lines and the shape of a regression test.
  Rejected: leaving it to be learned from three examples, which is
  what every fresh session had done.
- **A reference mesh is read with the tools that exist**: `import`
  into its own part studio, `report` for the planar faces (normal,
  centre, area, which give walls and thicknesses), section screenshots,
  a DXF of a view. The report gives no cylinders and no section as
  numbers for a mesh, so a `section` tool returning the loops a plane
  cuts went on the roadmap rather than into this work; the list was
  about setup, and the recipe covers the lint blower's numbers without
  it.
- **No `requirements.txt`**: the photo tooling is standard-library
  Python; the trace scripts want Node's Playwright from
  `apps/web/node_modules` and the turntable scan wants `ffmpeg`, which
  `okmcp.py`'s header now says. Rejected: an empty requirements file,
  which answers the wrong question.
- **The `screenshot` tool's description** said the iso eye looks from
  +x, +y, +z; `ok_render::View::Iso` is +X, −Y, +Z, which is why a `y`
  section shows its cut faces and `x` and `z` sections need `:flip`.
  `docs/MCP.md` has the picture.

## 2026-11 · The test binaries run in parallel; the slow tests stay

The chain's test step took 13 minutes warm with `cargo test --workspace`,
and the question was what to cache or prebuild to make a session start
faster. Nothing: the compile is 4 of the 22 cold minutes and the tests
themselves 13, because cargo runs the 54 test binaries one after
another and the two long ones (ok-photo's turntable merge and solids,
three and five minutes with two and six tests each) hold one core while
three idle. `cargo-nextest` schedules every test across the cores: 381
tests in 283 s, the whole chain 10 minutes warm instead of 18, and
`--quick` 90 s instead of 4 minutes. The floor is now the longest single
test, the mushroom merged from two scans, at 169 s.

Rejected: a Docker image or prebuilt binaries on a registry (they cache
the compile, which is the small part, and a cloud session runs a fixed
base container plus `scripts/setup.sh`, not an image; `Dockerfile.dev`
stays for local machines); Flatpak (packaging for end users, not a dev
loop where the kernel changes every pull request); marking the scan and
robustness corpora `#[ignore]` for a nightly job (they are the tests
that catch the kernel's real failures, and in parallel they cost what
the longest one costs, not their sum). Splitting the two long tests
into more, smaller ones would lower the floor further and is open.
Doc tests are not nextest's, so `cargo test --workspace --doc` runs
beside it, 4 s.

## 2026-11 · The rounding circle rides a blended normal, and a straight run is not a corner

Looking again at `spectre_tile` with the cage's numbers and others
turned up three faults in the kernel's Spectre module, none of them
in the tool itself. The straight tile, eroded by the gap, came back
with ten doubled points and four 180° spikes: `offset` decided mitre
or arc from the sign of a cross product, and the points along a
straight edge (four per edge, from the tile sampler) turn by a
rounding error either way, so a slightly negative one got an arc of
no angle, two points a hair apart and sometimes a hair backwards.
The clip tolerance hid it in the top's pieces; the master's outline
goes out raw. Now a turn under 1e-9 is a straight run and the output
is deduplicated. An arc of sagitta exactly half the edge, which the
check allows, is a semicircle, and the short way round between its
ends is either way, so some edges went the wrong way and the tile
crossed itself; the sweep is now taken through the apex, which is
never ambiguous. The third was the one that matters for a routed
master: `round_corner` carries the rounding circle back along the
incoming curve on the normal of whichever facet the carried point is
on, so the circle's centre jumped by `r` times the facet angle at
every facet vertex, and a bisection that ended on one of those jumps
rather than a root left the arc's end up to 0.3 mm off the curve
with a 25° kink on the knob. Which radii hit a jump was luck: the
cage's 2.38 mm did not, a 2 mm round at the same tile did, and a
rounding-error change elsewhere moved one corner's answer by 0.27 mm.
The direction is now blended from the facet's end bisectors, so the
centre moves continuously and every sign change is a root; the arc
starts at the circle's foot on the carried facet or a neighbour, and
a cut landing on a facet vertex is the start of the next facet, not
the end of this one, so the vertex is not emitted twice. A test
rounds the tile at every radius from 0.5 to 4 mm, with and without
the bit's fillets, and allows no turn beyond the facet plus the half
facet a join can add. Rejected: a finer facet to shrink the jumps
(they shrink with the facet but never go, and the chord test on the
output would still flap); solving the tangency analytically between
circular arcs (right for `arc` edges, not for the cubic, and the
polyline is what every later step sees anyway).

## 2026-11 · The duplicator's portal: feet under the tools, cheeks in front of the wall

Looking at the duplicator from above with the carriage forward, the
back half of the 855 mm base is bare, and at full rearward travel the
deck's back edge is 33 mm from the base's. The depth is the gantry's
body: the tools hang 210 mm ahead of the front Y blocks and 325 ahead
of the back ones (200 of deck behind the blocks' centre line, 35 to
the wall, 245 of X and Z stack in front of it), and
all of that has to park behind the work with base under its blocks.
The owner's proposal was the rails to the front, the gantry an
inverted U on them, and the gussets smaller and in front. Drawn as a
variant (`examples/duplicator/build.py --portal`), with the blocks 50
behind the tools: the base is 980 x 640, a quarter less area. Three
things decided its shape. A plain U connects the legs only through the
wall, 300 mm up, which resists one side running ahead of the other by
bending out of its plane, hundreds of times softer than the deck's
in-plane shear; so a deck strip 150 deep stays behind the wall, on the
back blocks, and the gussets become a full cheek each side standing on
the foot in front of the wall, the triangle the owner asked for with
its hypotenuse filled in. The legs cannot stand in the X slide's
sweep: the carriage plate reaches ±387 at full mechanical travel and
the tool plate ±517, so the cheeks stand behind the tool plate's
110 mm of Y and outboard of everything else, and the feet, which are
in that Y band, are outboard of the plate's working reach; the stylus
block's x knob, not the plate, sets it, at 408 with the bit at the
blank's right edge, which is why the rails went out 25 a side and the
base gained 80 of width. The tools are not between the blocks in Y:
the front support stands at the base's front edge, and at full
forward travel the front block has to stop short of it, which puts
the block's centre 50 behind the tool at the pose drawn; 210 was the
number being cut, and the remaining 50 is the price of a support at
the edge rather than a base extending past the work. Rejected: rails
out to ±560 so the legs could stand anywhere in Y (the width given
back would be a third of the depth saved); a narrower tool plate (its
width is the two tools' spacing plus the clamps); a fixed bridge with
the platforms on the Y rails instead (it also saves the depth, with
no racking question, but the pilot's reference and both fences move
with the table, and the owner's sketch was the moving gantry). The
original gantry stays as the default: both build from one script, one
test file sweeps both, and the README has the comparison.

## 2026-11 · The curvy tile is a glue-up of prisms, and the lid is whole tiles trimmed after

Thirty boxes want thirty lids of a hundred curvy pieces each, and
routing every piece to a template, or a dry stack of them on the
duplicator, is a piece at a time. The owner's idea was to make the
curve with a pair of matched bits instead: a symmetric arc on every
edge, out on the bumps and in on the flutes, is a bullnose and a core
box of one diameter (a quarter circle is also a roundover on a 45°
gable), so a straight stick with that section run past the bits is a
long prism with the curve already on it, and a glued-up stick of such
prisms, sliced like a loaf, is pieces by the dozen. The tile is split
into prisms along the 30° grid through its vertices, with three rules
the shop sets: every prism convex, or the stick cannot be ripped; no
bump split by a glue line, since a bit cuts a bump whole or not at
all (a split flute is two core box passes that have to register, so
it is penalised, not refused); and at every corner where a convex arc
ends, the prism no wider than 180° less the arc's end angle, because
the bullnose's shoulder leaves the arc tangent to the stick's next
face, which must therefore lie at or outside that tangent. The search
is over vertex diagonals and grid rays from the vertices that need a
cut, fewest cuts first, then no shared flutes, no glue face under 0.3
edge (a ray that stops a hair short of a vertex leaves a flat two
millimetres wide that no one rips), no corner under 45° (a fragile
edge on the stick, and a cassette nest that pinches it; the
semicircle cannot avoid its 30° points), the biggest smallest prism
and the fewest shapes: four prisms at the quarter circle, five
at five sixteenths of a circle, nine at the semicircle. The owner
settled on five sixteenths at a 10.56 mm edge, which puts the pair at
1/4 in bits, over the quarter circle's 8.98 mm, which is a smaller
tile for the same bits and one prism fewer, because the five looked
better. Rejected: decomposing into right triangles or squares (the
tile's corners are multiples of 30°, so no such tiling exists);
Kaplan's cubic (no one radius, so no bit); ripping only (the saw
station cuts the straight tile, and the curves then still want
routing); a die or molding planes (a cutter per profile, sharpened
by hand, for thirty boxes).

The lid is laid out with every tile the board reaches kept whole and
the board drawn as a trim line, rather than clipped at the board's
edge as the earlier tops were, because a glue-up of sticks cannot
produce a part tile: the pieces come off the stick whole, the
glue-up is made a tile larger all round, and the square is cut from
it after. That is also why the PDF has a forced 1:1 scale and the
trim rectangle, and why the sample layout exists: one tile at the
gap, printed, is the fit check before any stick is glued. The
explorer page the decomposition was found on stays as a scratch tool;
the same planner is in the web panel, where the cuts land on the
feature.

The prisms also retire the carving duplicator for the puzzle boxes:
the first box goes ahead with the table saw and the router table the
owner has, and the duplicator's other jobs (a printed master copied
to wood) wait for a CNC, which would do everything the duplicator was
built for. Its models, the stylus block, the portal variant and their
tests stay in the tree as built; nothing further is planned on it.

Cutting the sticks: the blade stands at 90° for good and the
cradles do the angling, because a convex section has every face as a
through rip, the offcut falling away on the far side, so a one-sided
sled with a printed cassette per cut (the saw station's idea again)
needs nothing set at the saw but which cassette is in. The sled
holds nothing past the blade, so the offcut is loose the moment it
is free, and each cut turns the stick as it is or end for end,
whichever puts the offcut above the kept stock, where it falls clear
instead of wedging under the stock against the blade. The bumps are
not cut with a bullnose after all. A half-round bit of the arc's
radius has a body that continues the semicircle past the arc's ends,
and where a bump ends at one of the tile's 120° corners that body
cuts 1.7 mm into the face next door: the corner would have to be
under 107°, which the decomposition does not give. A roundover of
the same radius does it in two passes on the gable the saw leaves:
with a gable flat against the fence and the bit's arc starting at the
flat's foot, where the arc is tangent to it, the roundover sweeps a
quarter circle, 90° − θ past the apex onto the other half of the same
circle, its shoulder flat slits the ridge off, and the pass from the
other flat finishes the arc. That works for any arc from the quarter
circle (one pass) to the semicircle; under 45° the quarter circle
would run past the arc's far end, and the cutter would have to be
ground to the arc. The shoulder rule stands as it was, since the
face next door still has to lean away from the bearing. Rejected: a
bullnose on sticks whose bump corners are under 107° (five more
prisms); straddling the blade (nothing needs it); setting the blade
to each face's angle (a cassette's nest is the angle, and a printed
one does not drift).

Drawing the prisms: on the sheet every prism of a tile is the same
gray and the prisms are told apart by thin white stripes at the
prism's own angle (the half turn shared out among them), and the
renderer does the same with one colour per tile and lighter stripes,
because colouring prisms differently made every tile look like five
pieces of five woods, when the tile is one piece of one wood with
glue lines in it. Rejected: drawing the glue lines heavier (the
pieces' outlines are already the gap, and a second line weight
clutters a 1:1 sheet); a gray per prism shape (a shape repeats in
every tile, so it says nothing the stripes do not, and makes the
tiles vanish).

## 2026-11 · The duplicator's Woodsmith variant: the plan's machine, part for part, and what got pared off

The owner may unretire the duplicator, and read the Woodsmith plan
again: the long carriage rolls about its round main rail, the arms
reach forward to the tools, and depth is the arc, not a slide. The
first ask was that machine on the rail hardware on hand with an extra
pivot at the head, free with a lock, so the tools could be set plumb
at the working depth or leaned for an undercut. It was drawn as a
variant beside the gantry and the portal (`build.py --arm`, in the
history at `e5fc922`): a ply channel, then a ladder, riding one X
shaft on two SC20UU blocks, the head on a 300 mm offcut across the
arm's front with a split clamp for its lock, a stop screw through the
arm landing first on a deck and then on a second shaft tied across
the feet, and a sand box at the tail; every part given a density so
the sand could be sized from the document and the balance read back
by the test. Three times the owner looked at the picture and saw more
plywood than the original, and three times that was right: a full-
width bed where two arms on edge do; a deck where the plan's short
carriages are tied by the main rail alone; a second rail the plan
does not have. The last word was to start from zero and build the
Woodsmith with the bearings and rails and no extra tilt, and that is
the variant kept (`--woodsmith`): side rails, short carriages, main
rail, long carriage, arms, tool support, counterweight, each the
plan's part in the nearest piece of hardware on hand, and nothing
else. The arm-variant lessons that survive in it: a ball bushing is
as free about its shaft as along it, so two blocks are the X slide and
the depth pivot together; the tips swing back on the arc by R (1 −
cos) and h sin, both backward, which is harmless because the bit and
the pilot are one body; the router clamp's knob swings 5 lower on an
arc than the Z slide took it, so the rails sit 10 further out; and a
shaft turned in a bore is two faceted cylinders that overlap by a
cubic millimetre, which the sweep allows for the bearing pair only.
The passes come from the stylus, as the plan's do: set 8.5 mm longer
than the bit it bottoms in the groove with the bit 4 mm in, matched it
bottoms with the bit through; so there is no stop screw and nothing
for one to land on. What the plan has that this does not is conduit:
the arms are ply on edge, 19 x 80, because that is what is on hand,
and they are twice the plan's weight for it.

## 2026-11 · The Spectre lid from triangular sticks: a hex edge, a lattice search, a silicone comb

The owner looked at the sticks planner's cassettes, then at a
programmable jig to replace them, and then set both aside for a
simpler idea: build the tile from a few primitive sticks on the
triangular lattice, hold a bundle of them end-on in a comb at the
pattern, and pour resin through. The curved edge is what made the
prisms and their router passes necessary, so the first move is the
edge: half a regular hexagon on every edge, out then in as the arcs
alternate, which puts every corner of the tile on the lattice of
half-edge triangles. The tile still tiles (a bump meets a dent as the
arcs do) and has the straight tile's area, and the router goes with
the curve: a 60° corner is sawn. What does not come for free is the
cover. The tile's fourteen edges run eight on the 0-60-120 lattice
and six on the 30-90-150 one, in three runs each, and a 30° edge
moves the 60° lattice by a fraction of a step, so the runs disagree
on where the lattice sits and no single lattice covers the tile. The
owner's own figure had four shapes covering 97.9 % with slivers
inside; the kernel takes the same four (the lattice triangle, its
half, the quarter's half, the trapezoid) on the six lattices the runs
define and searches: greedy, biggest first and boundary first, then
ruin-and-recreate from a fixed seed, which reaches 99.2 % in a few
seconds and repeats exactly, and a merge after it for the pairs of
halves that make a triangle, which the owner spotted in the first
picture. The slivers are the resin's. Two lines show by design, 0.7
between a tile's sticks for the texture and 1.5 between tiles for
the pieces, and the test puts a number on what they cost: a 0.7 line
round a 5.3 mm triangle takes two fifths of it, so the lid is about
half wood at this edge, which is the owner's to weigh against a
thinner line or a longer edge. The comb is silicone, because resin
does not bond to it and one comb serves every bundle: cast in a
printed box over a printed positive of the lid's sticks as bars, the
plate resting on the box's rim so the bars stop a backing's depth
above its floor, and the box backing the comb when it is turned over
for a pour. Both print on the owner's FDM machine, since the channels
between the bars are air and only the bars' sides carry layer lines,
on faces nothing sees; the comb holds only the sticks' ends, and that
end of the bundle is sawn off with the first slice. Rejected: a comb
printed as the negative (resin bonds to it; it could be sacrificial,
but it is an hour of printing per bundle), a single lattice with the
boundary pieces cut to arbitrary shapes (four sticks is the point),
and the search's result baked in as constants (a few seconds, and the
test proves it repeats).

The comb was for a bundle poured through with resin; the owner's
next step was to glue the tile sticks tight in printed clamping
cauls and keep the resin for the lines between tiles only, which is
the pattern plate the puzzle top already has, now with the hex curve.
A caul pair is a cradle whose pocket is the outline and a plunger
that is the negative of what stands above a split, and the plunger
only comes down if that part does nothing but narrow going up; the
Spectre has a dent close under its top at every turn, so the search
over every turn and split ends with the lid taking one bump, 4 % of
the tile. That was accepted rather than fought with side cauls: the
pocket enforces the other 96 % exactly, since the sticks go in one at
a time from above and a bay under an overhang is reached by sliding
the stick in before the ones above it, and the plunger's force
spreads through the lattice at 60°. The plunger is the slot's full
width for the clamps, with the bump's negative in its middle.

The owner asked what the cradle's material under the plunger's flat
was for, expecting a flexure; it is not one, and measuring it found
the first cut's flaw: the slot above the split was the outline's full
width, so where a bay reaches up to just under the split the cradle's
lip over it was 0.04 mm thick. Now the cradle rises 4 mm above the
split in two shoulders outside the chord, the bump's sides stand in
the slot between them, and the plunger's flat stops a millimetre
above the shoulders when its notch seats on the bump's roof. Rejected:
a tongue on the plunger the chord wide going down between the
shoulders, because the bump's sides are vertical for 5.4 mm above the
chord, so the negative cut the tongue away to a fin. Nothing in the
cauls flexes; the squeeze is the pocket's clearance and the glue lines.

The owner then settled the lid's construction: the tiles stay
separate units, so a later version can push them, laid out on a
raised template at their spacing, taped, and glued down to a
substrate with a dark glue or a coat of dark resin showing in the
lines; the sticks within a tile get the thinnest glue line. Two
consequences went into the kernel and the example. A tile that stands
1.5 mm from its neighbours is the tile eroded by 0.75, which is what
every layout has drawn as a piece since the puzzle top, so the cauls'
pocket is the eroded outline and the boundary sticks are the lattice
shapes with a side moved in: a triangle stays a triangle and a half a
half, only smaller, which keeps the cut list at six shapes. And the
template is a layout of its own (`template`), the pattern plate's
inverse: a pocket per piece and the lines standing between them, so
the tiles drop in at their exact places and tape lifts the whole
layout off. Rejected: tiles at full size with the tiling spread to
make the line (the Spectre's tiling is not a dilation of its tile, so
the lines would vary round each one), and the comb or the pattern
plate for the layout (the first holds sticks, the second is for a
pilot; neither holds a finished tile at its place).

The line between tiles is 0.7 mm, chosen from a printed strip of
three samples of the lid at 1.5, 1.0 and 0.7, and a second print of
the 0.7 sample with the tiles' top edges rounded 0.6, which the owner
found made the line read right; the roundover will be tumbling or a
sanding block on the wood. Choosing the line changed the sticks, not
just the template: a tile that stands 0.7 from its neighbours is the
tile eroded by 0.35, so the boundary sticks grew from 4.30 to 4.76 on
a side and the lid went from 48 % wood to 54 %. The examples carry
0.7 now; the strip stays as the record of the comparison.

The owner asked for a drawing of eroded triangles at a tile edge,
side, point, side, and drawing the point showed what the kernel was
not doing. A stick that meets the tile's outline with a point rather
than a side was pulled in by its sides alone, the inner line's worth,
so its point stood 0.25 mm into the line between tiles and would not
have gone into the cradle's pocket; a third of the sticks at a reflex
corner of the outline do that, 36 per tile, which the first cut list
listed as whole inner triangles. The same went for the end of a side
on the outline at a convex corner, into the tile's other segment.
Now every stick keeps half the gap off the outline: beside a segment
by that segment's offset line, exact there; out in the fan of a
reflex corner, where the eroded tile is an arc about the corner, by a
flat tangent to the arc and square to the stick's own corner, since a
stick stays convex and nobody saws a concave bite into a 5 mm point.
The cut list reports the stick as the whole shape with the point
taken off, the depth of the flat, because that is how it is made: a
pass over a sanding block along the stick. Finding it also fixed the
outline test, which took a side ending exactly at a segment's end as
off it when the parameter rounded past 1; with that the search's
tie-break counts differently and the cover moved from 99.4 % in 107
sticks to 99.2 % in 104, as good a cover, so the examples carry it.

## 2026-11 · A curvy Spectre from the same sticks: scraped arcs, rails for depth, a master printed on end

The owner saw that if the outer sticks of the hex-edged tile had
curves instead of flats the tile would be a true curvy Spectre, and
proposed small moulding planes in a fixture, then printed plane
bodies with the curve built in, then card scraper steel for the
blades. The geometry is kind: half a hexagon is inscribed in the
circle of half an edge about the edge's middle, so the arcs are that
circle's, every edge a semicircle, and the kernel's cover already has
a triangle with its apex at that middle on every bump chord, which
makes a bump stick a plain 60° sector whose arc is centred, to the
glue line, on its own apex. The eroded tile makes the two radii
differ, 4.93 on a bump and 5.63 in a dent, so the blades are not a
matched hollow and round; a matched pair would leave no line.

What carries the curve is the blade and a master, not the plane's
sole. A sole with the curve in it registers on the work, a 5 mm
triangular stick, and that is the fidgety part; here the sole is flat
and lands on rails on the fixture, the skirts down the block's sides
are the fence, and the blade stops cutting when the sole is down.
Three consequences. The blade's lean, which a scraper needs, is in
the slot (10°), because a sole that must end flat cannot be tipped by
hand; the 1.5 % the lean takes off a filed curve's height is 0.01 mm
over a face and is left. The top between the rails is lower than
they are, so no blade shoulder wears them. And the depth is not filed
into the blade: it is set on a master, the finished face standing
over a pair of rail edges, so only the curve has to be filed right.
The master is printed on end, face and rail edges one outline; lying
down, the 0.93 mm between them would round to a layer. Each rail
height is chosen so its blade can be seen to be right: the hollow's
arc where the bump's top is, the cove's tip on the rail plane, the
straight blade flush.

Rejected: a fence on the stick itself (the beading tool's own, which
tips on a triangle); shoulders on the blade as the depth stop (they
would ride and wear the printed rails, and put the filing's accuracy
into the stick); one master with a face on each of two opposite edges
(it rocks on whichever is down, so there are two, each with a flat
back); a chamfer plane for the points (it registers on a square
corner and a stick's is 60°); an edger blade for the steel (hard
enough, but 3 mm thick where a scratch stock wants under 1). Not done
yet, and said in the example's README: the kernel does not draw the
arcs, so the cradle, template and cut list are still the flat tile's;
nine dent chords per tile lie on halves and odd sticks with no seat;
and with arcs the points at a dent's inner corners come off square to
one face, not to the stick.


## 2026-11 · The leaning half ellipse: a tile for one solid stick, and one-handed by its edges

The owner turned from bundles of sticks to milling the tile's whole
outline round one solid stick on an indexed router, set by the bit
against an end pattern. Checked against the hex tile, twelve stops
reach 40 of its 42 faces with straight bits and two faces only two
thirds of the way, tucked under a neighbouring bump. A curved edge
has no such pockets: a 3 mm ball-nose at the same twelve stops
reaches everything but the four inside corners, which the kernel's
bit rule already fillets. The half circle was the owner's favourite
shape but leaves a 2.5 mm neck; half an ellipse as wide as the edge
and 0.8 as deep keeps the look, since it still leaves each corner
square to the edge where a shallower circular arc does not, and the
neck is 4.3. Leaning the ellipse, its crest off the middle, is what
makes the tile strictly one-handed: a bump and the dent it meets are
one curve only if bumps lean back along the outline and dents
forward, which is what the alternating sign already gives, and a
mirrored tile's lean the other way, so no edge of it fits any edge
(the test's best pairing is 2.4 mm off). Whether the symmetric
bump-and-dent tile is one-handed already was not settled; the lean
removes the question. Of the two directions the lean can take, one
narrows the neck and the other widens it to 4.5; the owner chose
that one at 1.2 mm from drawings at 0.8, 1.2 and 1.6.

The lean is a constant of the curve, not a parameter: `Params` is
built by hand in forty-six places and one more field for a number
nobody has asked to vary is not worth that. Circles in resin, half
rounds with ring segments, and drilling a blank on the edge middles
were drawn on the way and not built; the ball-nose's cost, a few
hundred passes a stick, is why the end pattern has to set position
and depth together.

## 2026-11 · The stick mill: the bit sets itself on a pattern at the stick's end, and the shafts are held at both ends

The owner wanted a duplicator with most of it taken out: a 12-stop
fourth axis for the stock and a pattern that only sets the router,
since the section does not change along the stick, and pointed at the
two-shaft carriages of mitre saws, held at both ends so nothing sags.
So the carriage is four of the owner's SC20UU on two 20 mm shafts
either side of the stick, an SK20 at each end of each; the router
stays in its own plunge base, whose lock is the depth lock, on a
plate that crosses by an M8 screw; and the pattern is the tile's
section at full size on the stick's own spindle, touched by the bit
itself with the router off. No stylus, no second tool, no scale.
The index plate is outboard over a riser, clear of the carriage, so
it can be 120 across: at the 60 that fits under the carriage a
tenth of slop at the pin would be a tenth at the tile.

The pass plan comes from the kernel's tile, not a rule of thumb. A
first plan stepped evenly across the stick and kept the passes that
faced their stop; it left the steep walls of the dents with ridges
several times the allowance, because a step across is a longer step
along a wall. Now each point of the outline goes to the stop that
faces it best among those that can reach it, and a stop's passes are
spaced by the ridge they leave on the surface. Honest limits, said in
the README: a twentieth of the outline is out of a 3 mm ball's reach
from twelve stops by up to a third of a millimetre; the deepest
passes need the bit 50 mm out of the collet, which means a tapered
bit whose taper the plan does not model; and the stick's stiffness
at 200 mm was not checked.

The first drawing had the shafts either side of the stick with a pin
in a 120 mm index plate. The owner preferred two things, and both are
better. The shafts are stacked behind the stick on a back wall, as on
a sliding saw but held at both ends, with a ply square upright on a
bearing block at each corner and the router on a shelf out from its
face: the lower shaft is at the stick's height, so the bit is not far
below the rails as it would be hung from rails overhead. And the
twelve stops are a hexagon in two cradles, one holding it corner down
and one flat down, 30° apart: a hexagon seats on its faces with no
slop where a pin has a hole's clearance, and a stick needs one change
of cradle. The spindle lies in a slot so it can lift the 8.7 mm a
corner needs to pass. Not drawn: the hold-down over the hexagon.

## 2026-11 · The stick mill's index: a twelve-pointed star in one cradle, and the axis as low as the star allows

The mill indexed with a hexagon in two cradles, corner down and flat
down, swapped in a tray half way round a stick. The owner asked
whether the two hexagons could be one shape, then pointed out the
cradle is that shape too. A hexagon and itself turned 30° is a
twelve-pointed star, and the negative of its lower three points is a
pocket it drops straight into at every stop: six flanks, two of them
level, none upright. One cradle screwed to the base replaces two
cradles and the tray, and the change of cradle mid-stick goes.

- A single V under one point lost: it bears on two 15.5 mm flanks
  close together, about 2.5 times less accurate than the hexagon's
  seat. Across three points the outer flanks are 73 mm apart.
- Wrapping more than three points lost: the next flank round is
  upright, so the star could not drop in.
- The axis was 119 over the base, a figure left from the first
  layout. With the cradle on the base the star's point, a relief and
  6.8 mm of floor put it at 66, and the shafts, square, shelf and
  uprights came down 53 with it. Lower would need a smaller star,
  which costs accuracy, or a hole in the base.

## 2026-11 · The stick mill: journals on posts, and the axis where the tile is fattest

A stick milled to the tile has 4.5 mm necks and 200 mm between
centres. The owner proposed steadies: cut round places and put
supports under them, at a third and two thirds.

- The journals are cut first, on the whole blank, which needs no
  support; the owner's first step, a temporary round at the middle,
  is dropped for that reason.
- Twelve flats rather than a round: the mill indexes, it does not
  turn, and a twelve-sided journal meets its post alike at every stop.
- A journal has to lie inside the tile or the profile cuts it away.
  About the tile's own origin a 7.0 mm circle fits; the owner asked
  for the largest, so the build finds the tile's largest inscribed
  circle (9.8) and puts the axis there. The plan is the same plan
  moved; the swing grows from 25.4 to 28.9, which took a millimetre
  from the gap under the cross slide to keep the bit's reach at 50.
- A cap over the journal lost: the passes come within a hair of the
  circle the journal is cut inside.
- The journal's depth is set as every other depth is, by touching the
  bit on a printed master, here on the pattern's outer face.

## 2026-11 · The stick mill's roughing: the pattern sets every depth, a grid counts the bites

The plan was 262 finishing passes and nothing before them, so the
first pass at a stop was a 3 mm ball-nose at full depth in solid
wood. The owner asked for roughing.

- Roughing depths are found as finishing depths are: the bit, off,
  on the pattern. A flat 6 mm bit resting on the pattern is exactly
  as deep as it can go there without cutting the tile; a feeler under
  the depth stop leaves the allowance. A table of depths to dial lost:
  the machine has no vertical scale and the point of it is that
  nothing is measured.
- The bites are counted by following the waste on a 0.15 mm grid in
  the build script, in the order the cuts are made. It is a count for
  the plan, not geometry the kernel owns, so it stays in the example.
- The blank is taken as the tile's convex hull and 2 mm: what a saw
  can leave, and it swings under the cross slide.
- Three slots between lobes are too narrow for the straight bit and
  stay with the ball-nose, up to 16.7 mm deep in twelve bites. A
  second, narrower roughing bit lost for now: it is three places.

## 2026-11 · The stick mill: the cradle slides, the spindle stays, the journals are turned

The owner asked whether a journal could be a circle, cut by turning
the stick while the bit comes down. It could not on the mill as it
was: the star sat in a fixed cradle, and the spindle lay loose in a
slot so the star could be lifted out to index. The owner proposed
sliding the spindle assembly off the cradle, then agreed that moving
only the cradle is fewer moving parts.

- The spindle runs in two bearings and never moves. The cradle slides
  along the axis in a guide, 26 mm, off the star's 20. Indexing is
  slide off, turn a point, slide on; nothing is lifted, so the stick
  stays level and on its posts.
- With height set by the bearings, a pocket with sliding clearance
  would leave the angle loose. So star and pocket taper 10° a side
  and the cradle is pushed home by a screw: it seats as a cone does.
  10° is past the angle at which printed tapers stick. A fork coming
  down on the star from above lost: its guides or hinge would have
  had the play the taper removes.
- Journals are round, 19.4 mm, turned with a crank in 24 turns each
  where twelve flats took 48 plunges in bites; their posts are plain
  right-angled Vs.
- The star's reliefs went with the lift: its points are blunted and
  the pocket's crests cut back, so the two sections differ only away
  from the flanks.

## 2026-11 · The stick mill's cradle moves on a push-pull toggle clamp

The cradle was pushed home by a knob screw. The owner pointed out it
is moved at every stop and wanted a lever. A push-pull toggle clamp's
plunger carries it: one throw is the whole 30 mm off the star, the
other brings it home and locks over centre.

- A toggle locks at a place, not a force, and the taper needs a push:
  a urethane pad between plunger and cradle, squeezed 2 mm, supplies
  it. Draw latches either side lost: two levers and the cradle still
  slid by hand.
- The plunger must push the cradle's middle, where the pocket is, so
  the cradle has a back closing the pocket's small end, a gap inside
  it so the taper seats first.
- The clamp lies on its side: upright, its lever would swing through
  the pattern. On its side it fits under the pattern's swing by 2 mm,
  with the size drawn; the clamp bought has to be measured.

## 2026-11 · The stick sled: the cutter stays put, three motors place the stick, a person pushes

The owner started over: a sled on a router table, the stick held at
both ends over a fixed cutter, the two ends lifting together. The
first thought was pattern wheels, a notch a pass setting height and
angle; then, since the wheels would have been computed here anyway,
motors and a controller, with the sled still pushed by hand.

- **Wheels lost to motors.** A wheel for 262 passes came out 270 mm
  across with notches 2 mm apart and cliffs of 17 mm between
  neighbours, and it has to hang off the table's edge, which wants the
  cutter within centimetres of that edge. A table of rows has none of
  that, and roughing bites are more rows, not a stepped rest.
- **Nothing moves under the cut.** The motors only place the stick
  between passes; the sled's back switch gates every move. That is
  what keeps this short of a CNC machine: no powered feed, no motion
  planning, a hundred lines of program.
- **Two hinges, not slides.** Height is a plate hinged at one side
  and propped by a pusher at the other, the stick between them. Left
  and right is a frame hung from a second hinge above the stick,
  because below it is the cutter. The hinges are the owner's 20 mm
  shafts and blocks. Each swing also shifts and tilts the stick; the
  planner works that out exactly, so the hardware need not be square
  to anything, only measured.
- **Pusher 2 is on the sled, not on the plate.** On the plate it had
  to undo the plate's own tilt, 42 mm of travel for 37 of use, and it
  hung a motor from the moving part. Level on the sled it bears on the
  tail hanger's edge near the stick's own height.
- **Every pass has its own angle.** Twelve stops were the star's
  limit, not the tile's. Free, each pass meets its face as squarely as
  the bit can get in, and the 5 % left for a file goes.
- **Steadies from above.** Under the stick is the cutter. Caps over
  the tops of the journals sit where no cut comes, and take the push a
  cutter from below gives.
- **A Pico, steppers and no encoders.** The G2 Nano the owner found
  has a closed-loop motion chip but no documentation for it in its
  repository. Screws of 2 mm lead give 0.00125 mm a microstep, more
  than a cheap sensor resolves; the plate's weight and a spring keep
  each screw loaded one way; a test stick finds the trims.
- **Checked by other code than made it.** The hinges are written
  twice, Python for the table and Rust for the test, and the Pico's
  program is run on dead pins over the whole table. The first run of
  that test found the safe height wrong: the frame is lowest hanging
  straight down, in the middle of its swing, not at either end.

## 2026-11 · The stick sled's screws hold both ways, through links

As first drawn each pusher's ball end rested on a flat: the plate lay
on one by its weight, and a spring held the hanging frame against the
other. The owner looked at it and said it just hangs there. A cut
that lifted the stick, or pushed it off the swing pusher, met a few
kilograms and a spring.

- Each nose is now tied to what it moves by a link pinned at both
  ends, 30 mm to a clevis under the plate and 60 mm to the tail
  hanger. A lead screw does not back-drive, so the screw is the stop
  in both directions.
- A link, not a pin in a slot: the far pin moves across the screw's
  line as the plate tilts and the stick rises (the hanger's pin 23 mm
  either way), and a link takes that up by leaning, at most 22
  degrees. The planner solves the link as it did the ball on a flat.
- A brake on each hinge lost for now: stiffer, but two more actuators
  and a step in every pass.
- Springs stay, light, only to keep the slack of nut and pins to one
  side.

## 2026-11 · The stick sled, checked: what calculation found that drawing had not

The owner asked for a sanity check of the sled. It was done by
computing, not by reading the design over, and it found things the
tests had passed.

- **The bit was a fiction.** The plan took the finishing bit as 3 mm
  all the way up. The bit that exists tapers to a 6 mm shank, and on
  that table it cut into the tile on 120 rows, up to 0.33 mm. The
  planner now knows the bit's outline, shuts that much more either
  side of every direction, and still reaches everything; a straight
  3 mm bit fits inside the tapered one, so one table serves both.
- **Bites were counted in one order and made in another.** Sorting
  the rows by spindle angle after counting the bites left a roughing
  row meeting 11.7 mm at once. The order is now settled first. The
  test that should have caught it compared the heights of rows, not
  the wood; it now runs the rows over the waste itself, on its own
  grid, and judges by the area standing over a bite, since a fin a
  hair wide is as tall as a full cut.
- **After a journal the cutter was left in its groove.** The
  controller lifted to the height that clears the spindle, which is
  lower than the journal. Pulled back, the sled would have ploughed
  the blank from there to its end. It now lifts back to where the
  sled came in.
- **One pose is not a collision check.** The model was the sled at a
  single row. Tried at 150 poses, the swing link was in its guide at
  the home position. The build now tries them every time.
- **Two back stops 17 mm apart, with different things over the
  cutter, was an accident waiting.** The master is now a sleeve the
  length of the exposed spindle, smaller than anything a pass comes
  near, and the safe height is reckoned from it: one stop, anywhere
  under 29 mm.
- **The stick is not limp.** The claim that started the journals and
  steadies was never calculated. The section's second moments are
  56,000 and 110,000 mm4; at the forces a 6 mm bit makes in wood,
  estimated at 2 N, it bends under a micrometre between centres. The
  journals are its weakest places. They stay until the owner decides,
  but the numbers do not ask for them.
- **Home switches fail safe, and homing gives up.** Wired normally
  closed; a travel limit on the search.
- **Left open, and said so:** how to calibrate. Half a millimetre of
  error in every hinge and link dimension is 0.06 mm at the cut after
  six trims and 0.2 without the two scales; the scales are in the
  controller, the procedure to find them is not written. Play in the
  swing screw's nose is 0.1 mm at the stick per 0.2 mm, because its
  link leans 22 degrees; that nose needs linear bearings, or the
  screw should ride on the plate, where its link would lean 5.

## 2026-11 · The stick sled: no journals, no steadies

The stick was to have two round journals turned in it at a third and
two thirds, on steadies. That came from the mill, and there from a
risk listed as the first thing to go wrong: that a stick cut to the
tile, with necks of 4.5 mm in its section, would be limp. It was never
calculated. Calculated, the section's second moments are 56,000 and
110,000 mm4, and between centres a roughing cut of about 2 N bends it
under a micrometre and twists its edge 14. The owner dropped them.

- What goes with them: 55 rows of the table, 28 mm of every stick, the
  one mode in which a motor moved with the stick over the cutter (and
  its dead-man switch), the pause to fit the steadies, two printed
  parts, and a second kind of stop for the sled.
- What they would have been for is the case the estimate leaves out,
  a bit that grabs. The answer to that is lighter bites, which are
  rows in a table.
- The axis stays where the tile is fattest, though no journal needs
  it now: that is what lets a 16 mm spur and master sit inside the
  circle every pass stays clear of.

## 2026-11 · The stick sled: the swing screw's nose rides a linear rail

The sanity check found that play in the swing screw's nose came
through to the stick at half of itself: 0.2 mm of rattle in a printed
bore, 0.1 mm at the cut, differently at every pass. The cause is the
link's lean, up to 21 degrees, and the lean is there because the
frame rises and falls with the lift past a screw fixed to the sled.

- **The nose is now a block on an MGN12H carriage** on a 100 mm rail:
  about a hundredth of a millimetre of play, 0.005 mm at the stick.
  It also stops the nose turning with the screw, which the printed
  guide left to "a flat or a key", and it takes the side push of a
  leaning link on four rows of balls where a bore took it on 24 mm of
  plastic.
- **Moving the screw onto the lift plate lost.** There the link would
  lean 5 degrees and the play would matter a fifth as much, but the
  screw would hang on a bracket 156 mm under the plate to reach the
  pin where its leverage is good, and travel 54 mm for 32 of use
  undoing the plate's own tilt. A rail removes the play; the plate
  only made it matter less.
- **A longer link lost** for want of room: 120 mm halves the lean and
  puts the motor through the riser.
- The geometry the table is worked from did not change: same pin,
  same link, same screw line. The table is the same rows.
- Anti-backlash nuts on both screws, since slack along a screw comes
  through whole and no geometry helps that.

## 2026-11 · The stick sled's passes keep off the ball's dead centre

The planner gave every finishing pass the direction that met its face
most squarely, which is the direction that cuts with the very tip of
the ball: the one point of a ball-nose that does not turn. 162 of 244
passes were within 5 degrees of it. Each pass now takes the open
direction nearest 15 degrees round the ball from the tip, where the
edge passes at about a metre a second at 25,000 rpm. Two passes
cannot. Passes, reach, bit height and travel are unchanged; at 25
degrees the bit would have to stand 1.4 mm higher, so 15.

## 2026-11 · The stick sled: most moves between passes go straight

With the sled back the master is over the cutter, so the controller
went up to a safe height before every swing and turn and came down
after. That was most of the lift screw's 8.5 m a stick. The build now
follows each move along the path the controller takes (swing at the
old lift, turn there, then lift) and marks it straight if the master
stays 1.5 mm from the cutter and the blank 1 mm from the table at
every angle it turns through; the controller goes by the safe height
unless a row is so marked. 346 of 438 are, and the screw travels
5.5 m.

- The table carries the mark, not the controller the reasoning: the
  Pico has no geometry, and the test can follow every marked move
  with its own.
- The blank turning into the table was the part nearly missed: at
  the safe height nothing reaches it, so the old way never had to
  ask.
- A setting of the bit is never gone to or left straight: there the
  master touches the cutter by design.

## 2026-11 · The stick sled's controller keeps its place and says what it wants

It spoke only over USB and began every stick at row one. A stick is
two hours and more of rows, so:

- A row is counted done when the sled has left its back stop and come
  back, not when the foot switch is next pressed: the switch that
  already gates the motors also knows a cut was made. The number goes
  to a file on the Pico; switched on again it offers to go on from
  there. Its first move after homing is from home, so it goes by the
  safe height whatever the table says about that row.
- A display is four lines on an SSD1306 if the driver and the display
  are there, and the same four lines over USB always. Not required:
  a controller that will not start for want of a display is worse
  than one without.

## 2026-11 · The stick sled is measured by touch and planned for as built

The table came from the drawing, and the README said calibration was
not designed. Run from the drawing's table, a sled with every
dimension up to half a millimetre out and its home switches 2 mm out
cuts 1.6 mm from where it should (median of 300), so this was the gap
between a drawing and a machine.

What lost:

- **Trims and scales from a test stick.** Six numbers in `config.py`
  got a sled 0.5 mm out to 0.06 mm at best, and nobody could say how
  to get the six with callipers from a cut tile. The scales are gone
  from the controller; the trims stay as a nudge.
- **Cutting forks and measuring them.** A prototype that cut slots in
  a scrap stick and fitted the sled to calliper readings of them left
  0.38 mm (median). Callipers on wood are the limit.
- **Measuring the built sled directly.** A hinge's centre cannot be
  reached with a rule to better than the half millimetre in question.

What is there: the sled measures itself. The master on the spindle is
a ground steel sleeve; a clip lead on it and another on what it
touches make a contact the Pico reads. The controller brings it onto
a 6 mm pin in the collet and a 1-2-3 block in three attitudes, 40
places, and `calibrate.py` fits the sled those touches describe.
`build.py` plans for that sled, so the controller stays what it was:
three numbers a row, no geometry.

Things the numbers decided, not taste:

- **Coverage before precision.** The first probing set (pin, block
  flat, block on edge) left 0.035 mm with perfect touches: nothing
  was touched above 59 mm and finishing cuts went to 68 (71 since the
  blank became a rectangle). The block on
  end is 76 tall, too tall to cross, so only one face of it can be
  touched from each side, with its place unknown. A straight face at
  an unknown place is still a straight line, and four heights on it
  each side brought perfect touches to 0.009 and real ones to 0.021.
- **The pin twice, half a turn apart.** A sleeve on studding runs
  out. Probed at one angle, 0.1 mm of runout reads as a cutter 0.1 mm
  out of place: 0.11 mm median. The second half turn separates them.
- **Three touches a place.** Scatter of 0.01 mm a touch is the next
  limit: 0.029 with one touch, 0.021 with three.
- **The sled puts the block where it wants it.** A block placed by
  eye is 3 mm from where the table expects it and the master came
  down on it in the simulation. Held where the face should be, the
  master is the gauge: the block is slid up until a lamp lights.
- **Down the pin's side a step at a time.** Coming down from the top
  of the lift beside the pin's foot, 1 sled in 300 brought the master
  within half a millimetre of the pin on the way. The first touch on each side is the shallowest and
  the rest step down from it.

What it cost the sled: the master is bought, not printed; the spur is
14 mm so a block lying under master and spur touches only the master;
the swing screw's travel went from 32 to 51 mm for the block's two
faces, and its rail from 100 to 150, the lift's size.

Whether the stick's axis runs parallel to the sled's travel is not
something a touch on one master can say, so that is a second table:
round bands turned near each end of a scrap stick, under the cutter
and beside it, and four calliper readings that say how far to move
the tail centre.

All of it is simulation. The study assumes a flat table, a square
block, a clean touch and a stiff sled.

## 2026-11 · The stick sled's blank is a rectangle

The cuts were planned from "the tile's hull and 2 mm over", and the
README promised a card template of it. Drawing the template showed
what that shape is: six flats of 34, 17, 17, 17, 12 and 12 mm at odd
angles with curves between them. Nothing in a shop makes it. A polygon
sawn round it stands proud at its corners by an amount no bite was
counted for, so the test's promise that no row meets more than its
bite was a promise about a blank that would not exist.

The blank is now the least rectangle about the tile with 2 mm over on
every side, rounded up to the millimetre: 49 x 42. A saw and a planer
make that, and it needs no template: the centre is 19.5 from one wide
face and 24.8 from one narrow one. The table's first row turns the
spindle to where the blank goes on with that wide face level and
underneath.

What it cost:

- Waste goes from 767 to 1214 mm2 of section and roughing from 145
  rows to 200: 63 more pushes, a quarter of an hour a stick.
- The cutter stands 42 mm over the table for 39. The table's clearance
  is tested with the uncut blank at every row, and a rectangle's
  corner reaches 33.5 mm from the axis for the hull's 30.9. By the
  deep rows the corners are long gone, so following what still stands
  would give the 3 mm back; that is not done, and the test stays
  simple and on the safe side.
- The lift's travel grows from 79 to 86 mm.

What lost: a six-sided blank, the hull's flats run out to their
corners. Fewer pushes, but four bevel rips each good to a millimetre,
against none.

The stops are blocks clamped to the fence, in the right runner's way,
because a fence is what a clamp reaches; the sled-back switch is on
that runner's end, and the riser's gusset there moved 40 mm in to
leave it room.

## 2026-11 · The stick sled's spindle is a ground rod, not studding

The spindle was M12 studding with everything nutted onto it. Asked
what was still open, the pulley with a 12 mm bore came up as a part
hard to find, and the question behind it had not been asked: what
does that pulley do if it does not run true?

The belt's radius on a pulley that is off the spindle's turning
centre by e comes and goes by e once a turn, so the spindle is e
over the pitch radius, times the sine of where it is, from where the
motor sent it. On 19.1 mm of radius, 0.1 mm is 0.3 deg, and the
finishing cuts are 0.15 mm out. Studding is 11.70 to 11.97 mm over
its crests, so a 12 mm bore sits up to 0.15 mm off centre on it, and
so do the bearings. That is several times what the probing was built
to take out, and the probing cannot see it: the master is round, so
touching it says where the axis is and nothing about how far the
spindle has turned.

So the spindle is a 12 mm ground rod with M12 cut on its ends, on
which a 12 mm bore runs true to a couple of hundredths: 0.02 mm off
centre is 0.03 mm of cut. The README says to check the pulley with a
dial indicator. The test turns every finishing row's spindle by the
error and works the cuts again, so the figure is not only the
formula's.

What lost, for now: measuring the once-a-turn error and taking it out
in the table. It is two numbers and the table could carry them, but
it needs something on the spindle that is not round to measure
against, and a rod that fits makes the error small without that.

The same blind spot holds for the home switches. The fit finds where
they are; how well they repeat from one day to the next is not in
the study, and the swing's goes to the stick whole. That is in the
README as a thing to measure before trusting a table.

## 2026-11 · The router positioner: what the first pass chose, and what it found

The owner's brief (`examples/router-positioner/BRIEF.md`) settles the
layout: boxes in boxes, a screw on the centre of each, motors on
column end caps. It leaves nine questions open and asks for a
parametric model and an envelope before any detailing. Where the
brief decided, the model follows; these are the model's own choices.

- **The Z stage is wider than the X column, by a rail each side.**
  The column goes out through the outer box's right wall and travels
  80 mm up and down in it, so its slot takes that wall from 116 mm
  up. The stage's tipping load (the router's weight out to the right,
  and the plunge) puts its top against exactly that wall. A stage the
  column's own width would have nothing there to bear on. Widening
  the stage 16 mm front and back leaves the wall a rail either side
  of the slot. What lost: a slim stage held by gibs alone, and an
  outer box long enough to swallow the whole column, which moves the
  wall away rather than giving it back.
- **Reference faces follow the loads.** The stage bears on the rails
  and the back wall, the quill down and back; the gibs are opposite.
  The brief's own rule, applied; its eighth question is still the
  owner's to confirm.
- **The column guides the quill too.** The brief calls the column a
  fixed extension of the sleeve, and taken at its word the quill's
  back end runs in it. That makes the quill's guide 2.7 of its widths
  long without a longer sleeve, and keeps the stage short.
- **One drive housing for both axes.** Collar and thrust bearing in a
  recess on the nut's side of a bulkhead, coupling in a chamber
  behind, motor on the cap. It is why the Z column is as wide as the
  X one.
- **The collision check has a control.** Nothing through anything at
  four poses is only worth saying if the check can fail, so the test
  also runs each axis past each end and requires a hit.

The router's size is a guess three ways (65 across the housing, 80
across the cap, 190 long): Bosch publishes none of it. The model is
parametric so that the owner's callipers replace the guesses, and the
README says which box grows with which number.

## 2026-11 · The stick sled holds its router in the positioner; the hinges go

Asked whether the router might come in from the side, or the work stay
put under a router above, the answer given was a fourth layout: the
hinged frame fixed, the router sliding beneath it. The owner's answer
was a design brief for a two-axis router positioner, and then: that is
the stick sled's router.

It is a better answer than the one given. The hinged sled moved the
stick on two arcs and worked the arcs backwards for every cut; a
sled built half a millimetre out cut 1.6 mm out, and it took 40
touches and a fit in 22 unknowns to plan for the sled as built. With
the router on two straight screws the stick has only to turn. A row is
a depth, a height and an angle, and none moves another.

What followed from it:

- **The hinged sled is parked, not deleted** (`examples/stick-sled-hinged`,
  its tests with it). The pass planner came across unchanged; the
  probing and the fit stay there as the worked example of measuring a
  machine with its own motors.
- **The sled slides, the positioner stays.** The positioner is the
  heavy, wired thing with a datum face; the sled is now a plate, a
  spindle and a centre. It rides the duplicator's two shafts, which
  the hinged sled had as hinges.
- **The positioner lies on its lid.** Upright, its bit is 185 to 265 mm
  over what it stands on, with its Z column wanting a hole below.
  Turned over, the bit is 70 to 150 up and the column is out of the
  way on top. The brief says not to count on gravity for Z, so
  turning it over is allowed; it puts the stage's weight on the side
  of the bulkhead with no thrust bearing yet.
- **Its face is a wall beside the stick.** The blank's corners swing
  33.5 mm, so the wall is 36 from the axis and the bit has to reach
  48 past it: 45.5 mm out of the collet nut. That is no shorter than
  the bit over the router table was. The earlier finding stands: the
  bit's length was never what limited anything.
- **A bit is set by the controller, by touch.** The router is inside
  three boxes; raising it by hand to a master is gone. The probe's
  clip goes on the bit and the quill is run out until it touches the
  master. Depth then needs no calibration at all: it is measured at
  every bit change.
- **The clip has to be parked before a cut.** A clip left on a bit is
  the one new way to get hurt that setting by touch brings. Its
  parking post is wired to ground, so the probe reads closed only
  when the clip is there, and the controller waits for that.
- **The wheel the owner first proposed is in**: 160 teeth, hanging off
  the sled's head end where neither plate nor positioner is.

What it costs: the positioner is 472 mm long and unbuilt, its router
unmeasured, and its boxes slide on strips under gib pressure where
the hinged sled had ball bushings and rails. Whether that holds a
few hundredths is the open question, and it is the positioner's to
answer.

## 2026-11 · The stick sled is set up by six touches, and what they leave

The hinged sled needed 40 touches and a fit in 22 unknowns. With two
straight screws the list is short: depth is measured whenever a bit is
set; height needs its middle and its screw's true travel; the master
has its runout. A pin against the master from the front, from below
and from above, at two half turns, gives all of that by arithmetic.

Tried on 300 machines built wrong it brings the worst cut from 1.6 mm
to 0.044 (median), which is twice what the hinged sled's probing left.
The study says why: the same machines with their travels square come
to 0.013. The six touches level the bit with the stick's axis at one
depth, and a quill that does not travel square to the stage carries
the bit off level as it runs in and out.

So squareness is the next thing to measure, and how is known: the
ball-nose's tip on the top and the side of a square block stood on the
sled's plate, two touches on each. A pin cannot do it, because where
a pin's side meets an edge moves along the pin, and the pin's own lean
in the collet reads as travel. That is not written; the README says so
and says what 2 mm in a metre costs. The depth screw's pitch, which
looked like the worry with a threaded rod, turned out to cost under a
hundredth.

## 2026-11 · The positioner's router: one measurement and a photograph

The owner took the Colt out of its base, measured its body at 70.71 mm
and sent two photographs of it, from the side and end on. The first
pass had guessed 65, and guessed the rest too.

- **The photographs are used, with their limits said.** Scaled by the
  body, they give lengths along the router and widths across one way
  to 2 or 3 mm. That is worse than callipers and far better than the guesses
  it replaces, two of which were wrong by 30 mm: the router is 159
  long behind its body's front face, not 190, and it stays round for
  58 mm, not 150. `build.py` marks each number measured or photo, and
  the README lists what to measure, most important first.
- **The cord goes down, through the floors.** It leaves the cap's
  side, where the first pass had it leave the back. Out of the front
  or the back of the column it meets the rails the stage bears on,
  just where finding 1 put them; out of the top it meets the lid, and
  in the stick sled the plinth the positioner lies on. Down, it wants
  a slot in two floors that miss the bearing strips and a notch in a
  part of the right wall nothing bears on. In the sled, turned over,
  down is up, into free air.
- **The bit slot is as wide as the body.** The front face has three
  screw heads, the spindle lock and the rack's end at its rim, the
  rack 12 mm proud, and at full plunge that face is 1.5 mm behind the
  box's wall. A slot for the boss alone would want the router set
  back in the quill by the rack's 12 mm, which gives away reach and
  leaves the lock behind the wall. A slot 77 wide passes all of it
  and puts the lock under a thumb.
- **The cord's boot is part of the router in the collision check.**
  The notch exists because the check found the boot in the wall, and
  taking the notch out makes the check fail.
- **In the stick sled the collet nut places the positioner.** The
  real nut is 34 mm in front of the quill where 16 was drawn, so at
  the deepest cut it is 20.5 mm out of the positioner's face. The
  rule that the nut stays outside the uncut blank's swing then puts
  the face 54 mm from the stick, not 36. The alternative, keeping 36
  and using less of the quill's travel, leaves 41 mm of depth where
  the cuts need 45. The bit's length out of the nut is the same
  either way: it is the blank's swing and the 12 mm past the axis.

- **The collet is a quarter inch, and the sled is planned for it.**
  The owner's Colt is the North American one. The plan had 6 mm
  shanks because the tapered ball-noses it was drawn from are metric;
  the same bits are sold on 1/4 in shanks, 3 in long. The shank is
  6.35, the flank leans 3.1° a side where it leant 2.8, the straight
  bit and the probe's pin are 6.35. The table comes to 485 rows from
  503: the wider bit roughs in 185 runs where it took 211, and the
  steeper flank costs 8 more finishing runs. The outline is still
  reached everywhere, 0.05 mm at the worst.
- **Points nothing reaches are counted at the end of planning.** The
  planner counted a point as unreached when its own pass could not
  better it, even if a neighbour's pass then took it. The steeper
  flank made four such points; all four end within 0.05 mm. The
  count is now of what still stands when the passes are all made.

- **Two readings stay estimates, with room for being wrong.** Asked
  for the cap's width the other way across and for where the cord's
  boot leaves it, the owner said to go with what there is. So the
  design is made not to care much: the quill's bore is 86 for a cap
  the photograph has at 81, and the cord's slots run 12 mm further
  each way than the reading needs, forward into the edge of the
  stage's floor and back under the quill's back strip, which bears
  either side of a slot a quarter its width. That costs 2 mm on
  every box. The README says which numbers are measured.

Not settled by this: whether a thumb through the slot works the lock.

## 2026-11 · The stick's drive, joint by joint

The owner asked for the chain from the motor to the far end of the
stick, connections included. Writing it down showed four joints that
were a bore on a rod and nothing more: wheel to spindle, the spindle
endways, the master, and spur to stick; and a home mark the controller
read that was not drawn at all.

- **Bought flange units, not bearings in bored plywood.** A 6201 in a
  32 mm hole in ply sits wherever the hole lets it, and nothing held
  the rod endways. Two KFL001 units bolt to the uprights' faces, line
  themselves up, and hold the rod with their own set screws. The
  uprights are drilled slack.
- **The master is the rod.** A sleeve needs fixing and can run out;
  the ground rod between the bearing and the spur is already a
  cylinder of known size on the turning axis. One part and one joint
  fewer.
- **The spur is a toothed ring, because everything at the stick's end
  has to be inside 8 mm of the axis.** The bit comes onto the stick
  from its end at cutting depth, so nothing bigger than what the tile
  leaves can be there: no faceplate, no screws into the blank. On a
  12 mm rod a blade or pins act at 4 mm; a ring of 16 x 2 tube acts
  at 7 and has twelve teeth. Lost with reasons: pins in drilled holes
  (any clearance at 5 mm of radius is degrees), a tang in a slot (the
  same), a faceplate and a plunge past it (the controller does not
  know where the sled is).
- **The tail centre slides.** The M10 pointed screw would have put
  its point somewhere new each time it was screwed up to a stick,
  unless its point were dead on its thread. A pointed length of the
  spindle's rod in two flange clamps is pushed up, tapped and
  clamped, and is not turned.
- **Twenty teeth on the motor.** Sixteen had 5.6 in the belt. Twenty
  has 7.1, is the commonest pulley there is, and keeps the belt. The
  ratio falls from ten to eight; the runout arithmetic does not
  change, since a pulley's error is divided by the ratio it makes.
- **The wheel's runout goes in the study, trued and not.** The six
  touches cannot see it, so the figure after them was short of the
  truth. Trued to 0.02 mm off centre it costs 0.006 mm on the median;
  left at 0.1 it costs 0.02. Measuring it and taking it out in the
  table is still not done: truing makes it small.
- **Homing backs off first.** A fin 10 degrees wide says home over
  10 degrees. An axis found at home is run away until the sensor lets
  go and then brought back, so home is one edge from one side.

What the spur holds is an estimate, not a measurement, and so is what
the cut asks of it. A pencil line across the ring and the stick is
the check until someone cuts a stick.

## 2026-11 · The owner's socket, and the sled turned end for end

The ring spur was a part to file and an estimate to trust. The owner
proposed a printed part instead: a socket for the rod on one side,
epoxied onto its roughened end, and a socket for the blank on the
other.

- **It is the better spur.** It drives the blank by its flats, so
  there is nothing to slip and nothing to estimate; it puts the blank
  on the axis with no centre mark at that end; and it fixes which way
  round the blank is, which had been done by eye.
- **It cannot be where the bit comes onto the stick,** which is why
  the last entry kept everything there inside 8 mm of the axis. So
  the sled is turned end for end: the bit waits at the tail, where
  there is only a rod, and a pass stops short of the socket. The cost
  is the stick's last 21 mm, uncut.
- **The tail becomes a spindle.** The bit is set against the rod it
  waits beside, and that rod has to turn with the stick for its
  runout to be found and allowed for. So it is the drive's rod over
  again in two more flange bearings, with three flats for a point so
  that the stick turns it. Lost: the sliding centre in two clamps,
  which did not turn; and keeping the master on the drive's rod
  behind the socket, which wants a third place for the sled to stop
  and a controller that knows which it is at.
- **Neither spindle slides.** The pocket is 8 mm deeper than the
  blank stands in it. The blank goes in, comes back onto the tail's
  point on a jack screw through the socket's floor, and is locked by
  a screw through a wall, which also takes up the pocket's fit.
- **The plate was in the shafts' supports.** Checking the new layout
  at one pose found the plate through a support, and it was not the
  new layout's doing: the supports are 2 mm higher than the bearing
  blocks, and the plate had always passed over them at the ends of
  its travel. The test only ever looked at the pose that was drawn.
  The plate is packed up 6 mm, and the test now moves the sled to
  both ends of its travel.

The socket's angle on the rod is set when it is glued, on the table's
first row with a wide wall of the pocket flat on a block. If that is
a degree out the tile is a degree round in its blank, which the 2 mm
all round allows.

## 2026-11 · A sled between two fences, not on shafts

Shown the sled on its two shafts, the owner saw the hinged design's
hardware still in the picture, which it was: the same shafts, supports
and blocks, as rails. The instruction that followed was a sled between
two fences, the work support anchored to it, and a lip on the fences
to stop it lifting.

- **It is built as asked.** A plate on the base, a fence each side,
  a lip over each edge. No shafts, supports, bearing blocks, packing
  or plinth.
- **One fence is the datum and the sled is sprung onto it.** Two
  fences a sliding fit apart is the usual way, and the fit is then
  the accuracy: across the sled is the way the bit reaches, so slack
  there is depth of cut. Spring plungers in the sled's near edge keep
  its far edge on one fence, the near fence stands a millimetre off,
  and the cut pushes the same way as the springs.
- **The lips are clear of the sled.** A lip that rubs is a second
  guide fighting the base. At 0.3 mm they only catch a sled that
  lifts or tips; weight and a hand hold it down.
- **The plungers are in the sled, not the fence.** The near fence is
  14 mm wide and stands against the positioner's face, where nothing
  behind it can be reached.
- **What it costs is said, not hidden.** Ground shafts in ball
  bushings were taken to be true and the study has no term for the
  slide. A fence is as straight as it is made and a ply base as flat
  as it is: both go into the stick one for one. The README asks for a
  metal datum and a straightedge on it.

The sled is low now, so the stick's axis is 89.5 mm over it and the
uprights are 130 tall where they were 84. They are stiff the way the
cut pushes, across their 50 mm, and limber along the stick, which
does not matter to a prism.

## 2026-11 · The positioner in plywood

The owner asked for everything that can be plywood to be, 1/2 in or
3/4 in, with 3/4 already on hand (a combination-core sheet, not
Baltic birch). The first pass had 8 and 10 mm walls and was sized
against a printer's bed.

- **Which thickness where.** 3/4 where a part is a wall or a cheek
  with screws into its edge and where there is plenty of it: the
  outer box, the stage's cheeks, the nut's bar, the router's collars.
  1/2 where a part slides inside another and its thickness is paid for
  twice in every box outside it: the quill, the tube, the motor
  housings, the cages. All 3/4 would be 25 mm deeper again.
- **The outer box's face is 1/2.** At full plunge the quill's front is
  at the sleeve's mouth, and the collet nut has to come out of the
  face for a wrench: 4.8 mm clear in 1/2, 1.5 mm inside it in 3/4. In
  the stick sled a 3/4 face would bring the positioner 7 mm nearer the
  stick, where there is no room left for the near fence.
- **The sleeve, the column and the X motor's housing are one tube.**
  The first pass left "how the column is fixed to the stage" as not
  drawn. In panels the answer is not to have a joint: four boards run
  from the mouth to the motor's cap, and the stage is glued round
  their first 140 mm.
- **The stage's cheeks are its bearing faces.** The rail the stage
  needed either side of the tube is a 3/4 cheek, and the cheek's two
  edges are where it bears: right on what the slot leaves of the right
  wall, left under two gibs. The rule of a width and a half then asks
  for a stage 267 tall, 3 mm more than its parts need.
- **Nuts are caged, not pocketed.** A hex pocket is a printer's
  feature. Two strips two layers thick either side of the nut's
  flats, and a keeper each end, is a saw's.
- **A tube's four panels are one width,** each lapping the next, so a
  tube is one rip setting and four crosscuts.
- **The cut list is tested.** Every body of the plywood parts has to
  be a piece on the list at its size, or two layers of one, with the
  list used up.
- **The sled became a box.** The plywood positioner's bit is 130 mm
  over the base where the printed one's was 111. On a flat sled that
  is uprights 148 mm tall. A bottom, two rails on edge and a deck,
  all 3/4, bring the deck up instead, and the uprights are the 84 mm
  they were.

Not settled: screws, glue and joints are not drawn, and plywood's
movement with the weather is left to the gibs. The sheet's real
thickness goes in two numbers at the top of the build.

## 2026-11 · The positioner with no joinery

The owner's method is glue and an air brad nailer, and no joinery. The
plywood model's panels were already butted, an edge on a face. Three
things in it were not a saw's work.

- **The gibs' grooves go, and the gap grows to take the gibs.** Seven
  stopped grooves 3 mm deep held the gib strips. With the boxes 1/8 in
  apart all round, where they were 1.5 mm, a gib is a length of
  1/8 x 3/4 in bar lying in the gap, kept in place by its set screws'
  points in dimples in its back, and the bearing strips are 1/8 in
  stock glued on. It costs 3.4 mm on each box outside another. Lost:
  keeping the 1.5 mm gap and making the tube 3 mm off centre round the
  quill to leave room on the gib sides only, which puts the screw off
  the middle of everything it passes through.
- **The switch's pocket goes.** The Z home switch sat in a recess in
  the lid with the stage coming within a millimetre of it. It is
  screwed under the lid and the box is 6 mm taller inside.
- **Cutouts stay.** The bit slot, the tube's way and the cord's slots
  are holes through one panel, a jigsaw's or a drill's work, and
  piecing a wall out of four rectangles to avoid one is weaker. The
  air windows became drilled holes because they could.
- **Some things are screwed.** The lid and the two motors' caps have
  to come off, and the router's collars have to come out. That is in
  the README as the exception it is.

The README's assembly section is where the method is written down:
nail from the outside in, because the inside faces are where things
slide.

## 2026-11 · Clips that hold a joint until it is nailed

The owner asked for L and T shaped things that slide onto the pieces
by friction and hold them together until they are nailed.

- **A clip is a cap over the ends of the two panels.** Two panels at a
  corner have nothing between them to join an inside wall to an
  outside one, so the walls stand on a plate that lies across the
  panels' ends. It pushes on along the joint, which is the one way
  open, and it prints flat with no supports.
- **The outer walls are the reference and the ribs are inside.** The
  ribs push each sheet out against a wall that is square to the other
  one. Ribs on the outside would push the sheets against the inner
  elbow, which is shorter.
- **Friction is a tenth of a millimetre of rib,** not a tight slot. A
  slot printed to the sheet's size either falls off or will not go on,
  depending on the day's print. A slot 0.4 over with ribs 0.5 high
  that stop 3 mm short of the mouth starts easily and then nips.
- **One clip per pairing of the two sheets,** three L and three T,
  because a slot fits one thickness. That makes each a gauge for the
  number the build was given, which was worth saying in the README.
- **In the picture the clips are drawn before the panels.** A wall
  added after a panel it touches is joined to it by the kernel, and
  the first picture was one body.

Nothing is printed and nothing is queued: the owner said to hold
prints.

## 2026-11 · An index wheel that stays behind, a star and a lock

The owner's idea: the tapered self-centring star from the stick mill
on the sled's spindle, its negative on a much larger wheel that does
not go with the sled, and a lock on the shaft. Unlock, engage, the
motor turns to the angle, lock, make the passes at that angle.

- **The index head floats; nothing is lined up to a fit.** A star on
  one shaft seated in a pocket on another joins two axes that are
  never quite one. Fixed, the head would hold the star off its seat by
  whatever the two axes differ, and a seat that is not home has play:
  0.05 mm is 0.05° at the star's radius. So the head stands on three
  feet and hangs back on two bolts through slack holes, sprung toward
  the sled, and the star puts it where it wants it. Lost: a spring
  post, and a head that has to be set on its feet once.
- **The springs push level with the axis.** Pushed at the plate, with
  the star pushing back 130 mm higher, the head tips: about 1.3 N m
  against the 1.4 its weight resists. Two bolts in a post at the
  axis's height cost one more piece of plywood.
- **Home, the head stands on the star and two feet, not three.** On
  three feet the base says how high the wheel is and the star says so
  too. With the near foot set short, the star carries that end; a
  star off its centre then lifts it as it turns, and the two feet
  that bear are far enough apart that 0.03 mm of lift is 0.005° of
  roll, which is the only part that turns the stick.
- **The lock is a pinch on a disc, not a brake on the rod.** At 80 mm
  it holds eight times what a cut asks, by estimate, and the pinch is
  between a plunger and an anvil on the same post, so the spindle's
  bearings are not loaded. A plunger that moves along the spindle does
  not drag the disc round as a swinging clamp arm would.
- **The clamp has a switch, and opening it is the go-ahead.** The
  controller cannot see the star, only the sled-back switch; turning
  the wheel with the clamp closed would slip the star's angle against
  the motor's count for the rest of the stick. A row at a new angle is
  begun by opening the clamp rather than by the foot switch: it is as
  deliberate, the hand is 300 mm from the bit, and it saves 200 presses.
- **A star with twelve ways in, and a mark.** One point cut off would
  make it go in one way only, and then a spindle turned by hand could
  not come home at all. Twelve ways and a painted mark: a blank put on
  a point out is 30° round and plainly not level.
- **Three hundred and sixty teeth, not more.** It prints on a 256 mm
  bed, clears the base by 22 mm and gives a whole number of steps to a
  degree. The owner's slot is in the head's own plate, 3 mm deep in
  use; a larger wheel would need it through the base as well. Larger
  buys little: the wheel's runout is already the smallest term.
- **A worm was asked about earlier and is still not used.** The star
  takes the worm's place as the thing that makes the wheel's angle the
  stick's, and it has no backlash while it is seated.
- **The bands are turned by hand.** They were turned by the motor with
  the sled held at a stop, which is where the motor now cannot reach.
  One whole turn of the disc back to its mark keeps the star's angle.
- **Kept: arriving forwards, homing off one edge, the same table.**
  The 485 rows did not change; only how the third number gets to the
  stick.

Not measured, and the largest guess in it: how nearly a printed star
comes home in a printed pocket twice running. The study takes 0.01 mm
at its flanks.

## 2026-11 · Pillow blocks on the sled, flange bearings on the index head

The owner asked to be taken through what carries the work piece and
everything else on its axis, and to use bearing blocks where they
suit. Six bearings carry three rods. All six were flange units on the
faces of plywood uprights.

- **On the sled, pillow blocks on risers.** An upright there was
  50 x 19 at its foot, stood on its end and butt-glued, with no
  joinery by the owner's rule. The socket's jack screw pushes along
  the rod, and that bent two such feet: about 1.4 MPa for 200 N by my
  estimate, on a glue line that is half end grain. A pillow block
  bolts down on a flat, so the pair stands on one solid block glued
  face to face, and the push is 0.04 MPa of shear. Lost: a flange
  unit could be slid up its upright to set the height, and a block
  wants shims. That is the better trade: shims do not creep.
- **The riser is two layers of 1/2 in,** and the axis is whatever
  that and the block's 19 mm come to, 44.4 over the deck. The other
  choice was to keep 43.5 and plane a riser to 24.5. Nothing needs
  43.5: the positioner's height is found by touch.
- **The deck was widened, not the block turned.** A KP001's foot is
  71 mm long across the shaft and the deck ended 26 mm from the axis,
  inside the near fence's lip. The deck is 46 mm above the fences, so
  it now overhangs its rail to 10 mm short of the positioner's face.
- **The tail's inner block moved 5 mm away from the stick.** The
  collet nut comes lower than a riser's top and nearer the axis than
  a riser's edge, so with the sled back it has to be past the riser's
  end. The master is 26 mm long for it. The build stops if the nut is
  ever over a riser, and the test works the same sum from the rows.
- **On the index head, the flange units stay.** The wheel's shaft is
  113 mm over the head's plate: blocks would stand on a pedestal of
  five layers, and the sensor and the two spring bolts would still
  want faces at the axis's height. The uprights there are 110 wide,
  and what loads them is the belt, in their own plane, and 15 N.
- **The lock's post is glued to the clamp's riser.** Found on the way
  through. The clamp pushes the disc onto an anvil that hangs from the
  post, so the post is pulled along the spindle 44 mm over the deck
  with whatever the clamp makes, 500 N if it is a good one. Alone on
  its 75 x 19 edge that is about 5 MPa at the glue. The clamp's own
  riser, which is pushed the other way, was drawn 4 mm short of the
  post; butted and glued to it, the pinch closes through the two of
  them and the deck only holds them down.

## 2026-11 · Flip books: an animation is frames of flat shapes, written as a GIF

The owner asked for an animation of every pass of the stick sled's
table: white behind, the cut being made solid, the blank another
colour, the cuts before it outlines fading to white. And for it to be
done in offkilter, so the code is there to use again.

- **In the kernel, not in a script with a picture library.** The
  examples run on Python's standard library and `ok-mcp` and nothing
  else, and that is worth keeping. So the drawing and the file format
  are a module of `ok-render` and a tool of `ok-mcp`; a script only
  says what is in each frame.
- **A GIF.** It plays wherever a picture is shown: a README on GitHub,
  a phone, a chat. A video wants a player and an encoder nobody should
  have to install; an animated PNG is smaller and truer in colour but
  is a still picture in too many places. GIF's 256 colours a frame are
  enough for flat shapes, and its LZW is a page of code.
- **`keep` and `draw`.** A cut animation is the same thing again and
  again: something is done, and then it has been done. Shapes a frame
  keeps are painted onto the page and stay; shapes it draws are over
  the page for that frame only. With that there is no boolean to
  work out: what is left of the blank is the blank with every cut so
  far painted over in the background's colour. The other way, a list
  of everything to draw in every frame, is 482 times as much to send
  by the end.
- **Only what changed is written.** Each frame is the box of pixels
  that differ from the frame before, the unchanged ones inside it
  see-through. That is what keeps 492 frames under 3 MB, and it is
  also why the fading is in eight shades: an outline that changes
  shade every frame is rewritten every frame, and eight steps over 48
  passes look the same as 48 and are less than half the file.
- **The module reads its own files.** A written file that only its
  writer has looked at is not tested. Reading the sled's animation
  back found a real fault at once: when a frame's last code lands
  where the codes grow a bit wider, the end marker has to be written
  wider too. Forgiving readers never notice; a strict one stops.
- **The picture is fixed to the stick.** On the machine the stick
  turns and the bit always comes from one side. Drawn that way the
  whole picture would turn at every new angle and the fading outlines
  with it. Fixed to the stick, the tile stays still and comes out of
  the blank where the eye can follow it. The README says which way
  round the machine is.
- **An old cut's outline is the bit's, cut off at the blank.** The
  true outline of what one pass removed is the bit less everything
  before it, which is the boolean again. The bit's own outline inside
  the blank's rectangle is near enough for a line that is fading.

## 2026-11 · A flip book frame can be turned

The owner, having seen every pass drawn fixed to the stick, asked for
the other picture: the bit staying level, its height and depth
changing, and the piece turning. That is what the machine does.

- **The frame turns, not the shapes.** The script still gives the
  blank and every cut in the stick's own frame, as for the first
  animation, and says how far each frame is turned. A cut that comes
  from angle t, shown turned by minus t, is a level bit at its row's
  height and depth without a line of arithmetic in the script; the
  test measures that it is.
- **What is kept is painted again, not turned as a picture.** The
  first book kept only pixels. Turning pixels blurs them, and 200
  turns would leave a smudge. A book with a turn in it remembers its
  kept shapes and repaints them at each new angle; one without keeps
  only pixels and costs what it did.
- **`still` shapes.** Nothing in the sled's animation uses one. A
  fence, a table or a scale that does not turn with the work is the
  next thing anyone will want, and it is a flag and one branch.
- **A long turn is shown on the way.** Roughing comes from 24
  directions 15 degrees apart, and a blank that jumps 15 degrees reads
  as a different blank. Turns over 6 degrees get frames in between
  with no cutter in them, which is also true to the machine: the bit
  is drawn back while the wheel turns the stick.
- **A shorter trail than the first.** With the stick turning, every
  outline moves at every new angle and is written again: at 48 passes
  of trail the file was 6 MB. At 24 it is 4.1 MB, and the picture is
  less crowded.
- **Both are kept.** Fixed to the stick is the one to judge a plan by;
  as the machine has it is the one to learn the machine from. The
  decision above this one, that the picture is fixed to the stick,
  still describes the first.

## 2026-11 · The router as measured, and a mount that comes out of the front

The owner photographed the Colt three times so that the positioner's
mount could be settled: the nose from the side, the router on the
measuring sheet, and the rear end on. What they showed changed three
things the two earlier photographs had wrong, and decided how the
router is held.

- **The rear is not a cylinder.** It was drawn as a cap 81 across with
  84 allowed. It is a 76 mm circle with two things standing out of it
  on opposite sides: the speed dial's housing, 45 mm from the axis,
  and the chin the cord leaves, 46. The quill's bore went from 86 to
  98. A window in the quill for each was the alternative and keeps
  the boxes smaller, but the router goes in rear first and both would
  have to pass the whole length of the bore in front of them: a
  window would be a slot the length of the quill. A bore that is tall
  and narrow would do it too, and every tube here is square by the
  same `tube()`; 12 mm on each box is cheaper than a second kind of
  tube.
- **The allowance is the reading's error, said as such.** The bore is
  twice (46 + 2) + 2: the side photograph is good to 2 mm, and a
  millimetre a side past that. The dial's housing and the chin are
  drawn as boxes of the sizes read, so the model's router is as wide
  as the photograph's and the interference test sees it.
- **One block, not two collars.** Two separate 3/4 in collars on
  54 mm of body are two bores to get in line. Three squares glued
  face to face and bored once are in line by construction, and the
  thin layer between them, relieved, leaves the same two lands. It
  also gives the four holding screws solid wood all the way through.
- **Tightened on the bench.** Anything across a kerf is tangent to
  the body, and nothing tangent to the body can be reached once the
  block is in a tube in a tube in a box. So the block is not part of
  the quill: it is clamped on the router outside, and the two go in
  together against stop plates, held by screws along the bit's axis,
  which is the one direction a driver can come from. The stop plates
  are on the side walls because the dial's housing and the chin pass
  above and below.
- **Ordinary bolts in notches, not inserts in an edge.** A bolt across
  the kerf needs a nut. A threaded insert in the edge of a 3/4 in
  layer splits the plies; a cross dowel wants a 10 mm hole 3 mm from
  the top edge. Notching the block's top corners gives a head and a
  nut a face each and room inside the quill, with hardware from any
  bin. The holding screws do go into inserts, but in the stop plates'
  faces, which is where an insert holds.
- **Out of the front.** The brief asked whether the router comes out
  without disassembly. It did not: the bit slot was 77 mm wide and
  the router went in before the stage went into the outer box. Three
  changes make it come out: the slot is 100 mm, the block's width;
  the cord's slots in the quill's floor and the tube's are open at
  their front ends, so the boot rides out along them; and the quill's
  front bearing strip is in two pieces either side of its slot. The
  rear's two widest things are up and down, the way the bit slot is
  long, which is luck.
- **What it costs.** The tube's floor slot is now over the Z screw,
  whose tip stops 14 mm under it; the test's post had to be made
  wider than the slot to find the floor. The spindle lock's button is
  under the front land, so a bit is changed with two wrenches. And
  the stage has to be in the top 49 mm of its travel for the boot to
  pass the bit slot's end: the test draws the router out with the
  stage down and sees it meet the face.
- **80 mm of plunge.** The owner's suggestion, for bits that sit
  deeper or shallower in the collet and for later jobs. The sled's
  plan uses the far end of it, 32.6 to 78, so at home the bit's tip
  is 32 mm clear of the blank's corners where it was 12.
- **The sled followed without being touched.** It reads the
  positioner's sizes, so its axis rose 6 mm with the bigger box and
  its box's rails grew by as much. The index wheel's rim, which
  dipped 3 mm into the slot in the head's plate, now passes 3 mm over
  it; the slot stays, and the test's wording changed from "dips into"
  to "in it or just over it".

## 2026-11 · Callipers after photographs: a liner, and a shorter block

The mount drawn from the photographs hung on six sizes. They were
marked on the owner's own photographs, lettered, and the owner put
callipers on each. Three were not what the photographs said.

- **Photographs first, callipers on what matters.** Three photographs
  gave nearly thirty dimensions good to 2 mm, enough to draw the
  whole mount and see which few it was sensitive to. Six calliper
  readings then settled those. Asking for thirty at the start would
  not have said which six mattered.
- **What a photograph cannot see.** The plastic of the body is
  1.7 mm smaller across than the aluminium ring in front of it: under
  the photograph's error, and the one that mattered most. A nut
  tightened on a bit sits 4.5 mm nearer the rim than the slack nut
  photographed; the note had guessed a millimetre or two.
- **A liner, not a second bore.** A land bored for the plastic
  cannot pass the ring to get there, and one that has passed the ring
  cannot close 1.7 mm on a 3 mm kerf. The alternatives were a block
  in two halves, each land bored to its own size, or bringing the
  plastic up to the ring's size. Two halves is four bolts, two bores
  that have to be concentric, and a relief filed for the slotted
  metal plate if that plate is proud. A printed band 0.8 mm thick
  leaves the block one piece with one bore, and its gap takes the
  plate whether it is proud or not. Its thickness is the thing to
  get right, and callipers over it on the router say whether it is.
- **Two layers, not three.** With 51 mm of bare round a block of
  3/4, 1/2 and 3/4 in is 50.8 mm and fits with nothing to spare. Two
  3/4 in layers are 38.1. The relieved layer between two lands is
  gone; the step from ring to liner is its own relief, half a
  millimetre long.
- **The front land is on the ring.** The ring is the turned metal
  the router's own base locates on; the back land, on a printed band
  over plastic, steadies it. The block's front face is 1 mm behind
  the rim so that 17 of the ring's 18 mm are in the land.
- **The nut's face moves the positioner, not the bit.** The sled
  stands the positioner off by what the nut stands out, so that at
  the deepest cut the nut is just outside the blank's swing. A nut
  4.5 mm nearer the rim brings the positioner 5 mm nearer the stick
  and leaves the bit where it was, 45.9 mm out. The deck's edge came
  to 6 mm from the positioner's face, from 10, so that the risers
  under the pillow blocks stay on it.
- **The collet's depth is a limit on the bit, said as one.** 32 mm
  of shank, 19 at the least and 1.5 off the bottom: a bit 65 to 76 mm
  long. The 2-7/8 in straight bit is inside that. The 2-1/2 in
  tapered one is not, by 1.4 mm, which is the case for planning its
  passes with less of it out; that is the next piece of work and is
  not in this one.
- **The wrench.** 9.49 mm across the flats is 3/8 in, not 10 mm.
  With the nut where it is on a bit, its back is 1.6 mm out of the
  box's face at full plunge and the flats are in the slot's mouth:
  the rule that wanted the nut 2 mm clear now wants 1, and the
  README says where each wrench goes.

