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
