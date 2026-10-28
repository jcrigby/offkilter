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
