# History

How the project came to be, in wall-clock time. Measured from the
session transcript (a timestamp on every message) and the git log, not
from memory. Times are the maintainer's local clock, UTC−6. A "block"
is a stretch of activity with no pause longer than 25 minutes; every
pause longer than that ended with a message from the maintainer, so
the gaps are waiting on a person, not on a build.

The whole project so far is one Claude Code session, compacted several
times, started on Saturday 19 September 2026 at 05:16 with an empty
repository and the goal of an open-source Onshape-like CAD.

## Totals

| | |
|---|---|
| Wall clock, first message to last | 78.8 h |
| Active, pauses over 25 min removed | 26.7 h |
| Commits, excluding merges | 125 |
| Pull requests merged | 19 |
| Messages from the maintainer | 48 |

The tool was building a real project (the router lift, through the
MCP server) about 28 hours after the first message.

## Working blocks

| Block | Length | Commits | What landed |
|---|---|---|---|
| Sat 05:16–06:00 | 0.7 h | 2 | Kernel foundation, B-rep and booleans |
| Sat 06:32–07:56 | 1.4 h | 14 | Face references, sketch mode, undo, revolve, fillets, patterns, variables, document server, browser tests, holes |
| Sat 10:13–11:39 | 1.4 h | 8 | Convergent multi-user editing, sweep, loft, accounts, curved blends, sketch trim and offset |
| Sat 13:16–22:27 | 9.2 h | 67 | Assemblies and mates, robustness corpus, shell, drawings, direct edits, benchmarks, splines, sections, STEP, materials, teams, branches and merges, the MCP server ("keep going until you can't think of anything remaining") |
| Sun 03:35–10:53 | 7.3 h | 20 | Screenshot tool, incremental booleans, topology naming, STEP import, exact surfaces (PR #1), the router lift example (PR #2), countersink, mates, DXF, sketch chamfer (PRs #3–#6) |
| Sun 11:29–12:09 | 0.7 h | 1 | Remote Control and `.mcp.json` for driving it from a phone (PR #7) |
| Mon 07:28–10:10 | 2.7 h | 5 | Puzzle feature, plan editor, row gap and fixture, veneer note, strip tabs (PRs #8–#12) |
| Mon 12:24–12:50 | 0.4 h | 1 | Fabrication layouts per colour (PR #13) |
| Tue 06:18–07:13 | 0.9 h | 1 | PDF shop sheets (PR #14) |
| Tue 08:36–08:44 | 0.1 h | 1 | Assembly sheet checked in (PR #15) |
| Tue 09:34–11:03 | 1.5 h | 3 | Sub-assemblies and member connectors, workflow in `CLAUDE.md`, dev image and settings (PRs #16–#18) |
| Tue 11:49–12:04 | 0.3 h | 1 | Decisions log (PR #19) |
| **Total** | **26.7 h** | **125** | |

## Pauses

| Pause | Length | Ended with |
|---|---|---|
| Sat 07:56–10:13 | 2.3 h | "Keep going in your suggested order" |
| Sat 11:39–13:16 | 1.6 h | "Keep going with assemblies" |
| Sat 22:27–Sun 03:35 | 5.1 h | "Tell me about the remaining items in the roadmap" |
| Sun 12:09–16:15 | 4.1 h | the Remote Control setup |
| Sun 16:15–Mon 07:28 | 15.2 h | the puzzle feature request |
| Mon 10:10–12:24 | 2.2 h | "The trough is still wrong" |
| Mon 12:50–Tue 06:18 | 17.5 h | "Does ok support export of pdf shop assembly drawings?" |
| Tue 07:13–08:36 | 1.4 h | "I don't see assembly.pdf in the out directory" |

## Shape

Saturday was one long unattended run: nine hours and 67 commits on a
standing instruction to keep going. Sunday added the pieces that made
the tool usable from a chat (screenshots, the MCP server driving a real
project, the phone setup) and switched to a branch and a pull request
per increment. Monday was slower, three hours across the day with a
usage limit approaching, and was the first feature driven by the
maintainer's own project rather than a roadmap. Tuesday's blocks are
short and each one is a pull request that came from looking at real
output: a sheet that was too busy became sub-assemblies and, with them,
a kernel fix. `docs/DECISIONS.md` has the reasoning from those days.
