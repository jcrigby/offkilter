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

