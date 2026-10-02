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
