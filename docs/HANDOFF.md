# Handoff: where the Spectre stick machines stand

Written on 2026-11-19 at the end of a session that had run for two
weeks, for whichever session picks the work up, and for the owner to
correct. It holds what was only in that conversation: what the owner
has bought and measured, what is waiting on whom, and things said in
chat that are written nowhere else. The designs themselves are in
their own READMEs and in `docs/DECISIONS.md`; this does not repeat
them. When something here goes stale, fix it or delete it.

## What is being built

A machine to mill the leaning half-ellipse Spectre tile round one
solid stick, 200 mm long, to be sliced into tiles for a box lid. Three
examples, each with a regression test in `crates/ok-render/tests/`:

| | what | state |
|---|---|---|
| `examples/router-positioner` | two-axis plywood positioner holding the owner's Bosch Colt | drawn to the owner's calliper readings; cut list written |
| `examples/stick-sled` | the stick on a sled, a wheel on its own spindle, a screw feed, a Pico controller | automatic since 2026-11-19; a precheck table before any real stick |
| `examples/stick-finish` | two printed sanding forms, two scraper blade templates, a captioned animation of the whole job | drawn; nothing printed |

**Nothing has been built, and nothing is queued to print.** Every
holding and timing figure in the sled is an estimate until the
precheck has been run on a real machine.

## What the owner has, has ordered and has measured

| | |
|---|---|
| router | Bosch Colt, North American, 1/4 in collet. By the owner's callipers: 90 mm from the dial's housing to the chin; body 69 mm on the plastic, the aluminium ring (70.71) slightly proud; spindle flats 9.49 ("maybe it's not metric": 3/8 in is 9.525); 30 mm from the rim to the nut's face with a bit tightened; a shank bottoms 32 mm in from the tightened nut's face; 51 mm of bare round from the rim to the rubber grip |
| bits | Freud 04-110 (1/4 in straight, 2-7/8 in long) and Freud 72-405 (1/8 in tapered ball tip, 2-1/2 in long): ordered, the owner confirmed the specifications |
| abrasive | 3M 268L aluminium oxide microfinishing film, PSA, 8-1/2 x 11 in sheets: one each of 100, 80 and 60 micron |
| scraper | card scraper stock, to be scored with a grinder and shaped without overheating |
| plywood | **not bought yet.** The models take 1/2 in as 12.7 and 3/4 in as 19.05 |
| printer | Bambu Lab P1S; prints are asked for by a GitHub issue, `docs/PRINTING.md` |
| photographs | three of the router (side on the measuring sheet, rear, nose). They are **not in the repository**; the positioner's README records what was read off them |

The owner looked at buying a four-axis CNC instead (see below) and on
2026-11-19 decided to stay with this machine.

## Waiting on the owner

- **Plate clear, every time.** No print is asked for without the
  owner's yes for that print. Also standing since mid-November:
  "don't print anything yet because I have a few more ideas."
- **Plywood thicknesses**, when the sheets are bought: `T12` and `T34`
  in `router-positioner/build.py`, `T12` and `PLY` in
  `stick-sled/build.py`. Then regenerate both. The positioner writes
  a cut list; the sled does not yet, and a cutting plan per sheet has
  not been written (the owner spoke of 30 x 30 in sheets: confirm).
- **The bits, when they arrive:** overall lengths (73.0 and 63.5 mm)
  and the 72-405's tip (3.175). Constants `ROUGH_LONG`, `BIT_LONG`,
  `BIT`, `TAPER_L` in `stick-sled/plan.py`.
- **The film's thickness** by callipers: `FILM = 0.20` in
  `stick-finish/build.py` is assumed.
- **The scraper stock's thickness and width.** The blade holder, the
  owner's "hybrid micro scraper finger plane", is not drawn for want
  of them. The two blade templates are.
- **Whether parts for the earlier sled were already bought:** an 852 mm
  belt, a 20-tooth pulley, two KFL001 flange bearings, a toggle clamp.
  The automatic sled needs none of them (the clamp only if the lock is
  fitted). Its shopping list is under "To buy" in its README.

## Open, and not asked for yet

- **The sled's printed parts are drawn as envelopes.** The wheel is a
  plain cylinder with no belt teeth; the disc, the wheel and the
  socket have no bolt holes, inserts or screw holes; and the build
  exports no STL for any of them. Before one can be printed it has to
  be drawn for printing. The files that are ready to print:
  `router-positioner/out/liner.stl`,
  `stick-finish/out/form_bump.stl`, `form_flute.stl`,
  `template_bump.stl`, `template_flute.stl`.
- **Fewer passes, finished by the forms** (roadmap, unchecked):
  `stick-finish/whole.py` plans and draws it, 204 detail rows for the
  table's 323; the sled's own table is not made that way yet. The
  0.4 mm ridge is a guess until a form has been along a stick.
- **A lock that works itself.** The lock is drawn and left off; fitted
  as drawn it is a hand toggle clamp again. No actuator is drawn.
- **Not drawn:** the router's mains on a relay (its switch is a
  hand's), the cable that follows the sled, the belt as a body.
- **Not written:** how to measure whether the positioner's two travels
  are square, which the sled's README says is the largest error left.
- **The fourteen corners of the tile.** The two forms cover 169 of the
  outline's 205 mm; the corners are the other 36. By a scratch
  calculation, not tested and not in the repository, they fall into
  five kinds with 5, 4, 2, 2 and 1 members. `shapes()` in
  `stick-finish/build.py` finds the stretches the edges share, which
  is where to start again.
- **"Show me the segments on the spectre shape itself"** was asked,
  interrupted and replaced by the captioned animation. The coloured
  line inside the tile's edge in `whole.gif` is the nearest thing
  drawn.
- `examples/stick-sled-hinged` and `examples/stick-mill` are parked
  predecessors. Their tests still run. Nobody has asked for them to go.

## Said in chat and written nowhere else

**The four-axis CNC that would do this job**, should the machine fail.
Every pass is a straight line along the stick with the other axes
still, so an indexed rotary axis on a small three-axis router is
enough. Worked back from the plan's numbers:

| | minimum |
|---|---|
| X, along the stick | 200 mm clear of chuck and tailstock |
| Y, across | 50 mm: the bit works up to 19 mm either side of the axis |
| Z | 60 mm of travel, and the collet nut able to rise 80 mm above the rotary axis (72 if retracts are made sideways) |
| rotary | 75 mm swing, 220 mm from chuck face to tail centre, a tailstock; 0.02° steps; 0.05° accuracy and backlash (0.1° would match the plan's own ridge); about 2 N m holding |
| spindle | 1/4 in collet, 10 000 to 25 000 rpm; 300 W was a round figure, and the estimate in the sled's README puts the roughing cut at tens of watts |
| the rest | 0.05 mm repeatability; the rotary axis parallel to X within 0.05 mm in 200; a controller with four axes, indexed moves only |

The pass table converts row for row (turn to A, depth to Z, height to
Y); the converter is not written. The 49 x 42 blank with its axis
1.5 mm off centre will not sit in a self-centring chuck: the printed
socket as an adapter, or a re-plan for a square blank.

Machines looked at on 2026-11-19, from search summaries and one
maker's posts, none confirmed to meet all of that: Makera Carvera Air
with its fourth axis (harmonic drive, 0.01°, 92 x 200 mm, the only
one that publishes an accuracy; unknown: how high the nut rises over
the rotary axis, and whether it takes a 73 mm bit); Sienci LongMill
MK3 with the Vortex (only the closed-loop Vortex in 4th-axis mode on
the SuperLongBoard keeps Y, and the Colt's 70.7 mm body does not fit
a 65 mm mount); Onefinity Elite with the Revolution (an 80 mm
four-jaw chuck of unknown kind, 160 mm of Z).

**Pass counts.** With the table as it is: 573 passes, 2 h 37 min by
the controller's own pauses. With the reduced plan drawn in
`whole.py`: 250 coarse and 204 detail, 454.

**Mistakes made and corrected in that session**, in case they come
round again:

- The tile's edges are never mirrored: every edge is the same leaning
  half-ellipse, a whole number of twelfths of a turn apart. A form
  fits one way round. "Turn the tool end for end" was wrong.
- How far a bit must stand out of the collet nut is set by the nut
  passing the uncut blank, 39.8 mm, not by the cut itself (37 was
  wrong).
- An animation made from the sled's own table shows a finished tile
  and nothing to sand: its ridges are 0.05 mm.
- A 20-tooth pulley 148 mm from the wheel has 4.8 teeth in the belt.
- Amana 46282 is a 1/16 in ball, not 1/8.
- A section of the sled's README was written and not applied before a
  push. Grep the README for the old design's words before committing.

## Working on these examples

Regenerate in this order after a change; each reads the one before:

```sh
cargo build --release -p ok-mcp
python3 examples/router-positioner/build.py
python3 examples/stick-sled/build.py            # 11 s
python3 examples/stick-sled/calibrate.py study  # out/calibration.json, which the test reads
python3 examples/stick-sled/animate.py          # 19 s; only if the cutting rows changed
python3 examples/stick-finish/build.py          # only if the tile's outline changed
python3 examples/stick-finish/whole.py          # 16 s; only if the cutting rows changed
scripts/check.sh --quick                        # three minutes: fmt, clippy, the ok-render suite
```

- `scripts/check.sh --quick` prints FAILED for fmt whenever it has
  reformatted a file. Run `cargo fmt --all -- --check`; if that is
  clean, the reformatted file only wants committing.
- In `okmcp.Part`, an `op="add"` is united with every body it touches,
  not only the last one drawn. Bodies that must stay apart are each
  drawn `new`, and anything added to is drawn before its neighbours
  (the sled's travel drive and travel nut are ordered for this).
- The sled's numbers are in `out/sled.json` under `sled`, `turn`,
  `travel`, `hold`, `lock` and `precheck`. There is no `index` any
  more.
- The controller's program runs on a computer with `--sim`. Long moves
  are counted and not stepped unless something sets `STEPS` to listen
  to the pins; `HOME` and `CONTACT` stand in for switches and the
  probe. `build.py` has examples of each.
- sienci.com refuses automated page reads, and Makera's product pages
  carry their specifications in script: search results were the only
  way in.

In that session the owner had each increment pushed to `main` as well
as to the session's branch, once `scripts/check.sh` had passed
(`--quick` for changes under `examples/` and `crates/ok-render/tests/`
only), with no pull request. A new session should ask once whether
that still stands before pushing to `main`.

## For `docs/HISTORY.md`

That file is measured from a transcript, and this session's will not
outlive it. From its timestamps: first message 2026-11-05 23:54 UTC,
last 2026-11-19 21:41 UTC; 334 hours of wall clock, 41 hours active
by the same rule (pauses over 25 minutes removed); about 245 messages
from the owner; 94 commits. `HISTORY.md` itself has not been brought
up to date and still describes the first session only.
