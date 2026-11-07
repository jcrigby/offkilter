# offkilter and SolidCraft

Written 2026-11-07 after rebuilding the carving duplicator on
[SolidCraft](https://solidcraft.io) (`examples/duplicator/solidcraft/`),
to answer one question: is it time to move. The short answer is at the
end. The premise, stated by the maintainer: multi-user editing does not
matter, and the cloud side of offkilter (`ok-server`, accounts, teams,
branches, merges, the real-time sync) came from the first prompt's
"Onshape-like" and has become a burden.

## The two projects in numbers

| | offkilter | SolidCraft |
|---|---|---|
| Began | 19 Sep 2026, one Claude Code session, empty repository | 2025 or earlier (the oldest commit was not reachable); pull requests number past 12,900 |
| Code | 75k lines of Rust in 12 crates, 8k of TypeScript | 1,014k lines of Rust in 17 crates (`ui-egui` 381k, `engine` 227k, `doc` 83k, `kernel` 70k), plus the kernel itself in a second repository |
| Who writes it | the maintainer's agent, 175 commits, 139 pull requests | Tangency Labs and contributors, humans with `claude` and `codex` on about half the recent commits; `AGENTS.md` sets the rules (clean room, no GPL, no panics, every command typed) |
| Kernel | polyhedral B-rep with analytic tags: curved faces are facets at 5°, booleans on polyhedra, an `exact.rs` for some curves | exact B-rep: SolidCore, a Rust port of NVIDIA's SMLib (NURBS surfaces, pcurves, tolerances per edge and vertex). It replaced the `truck` kernel between 21 Oct and now; SMLib booleans measured 1.8 ms median against truck's 8.8 |
| Verification | 377 tests; a regression test per example reading the document and regenerating it in-process, checking volumes, clearances and mechanisms | 90 of 90 reference parts match analytic or FreeCAD values (volume, area, centre of mass, bounding box); 33 MB of test corpora; a naming corpus of 154 designs; fuzzing of every command |
| Features | sketch solver (20 constraints), extrude, revolve, sweep, loft, holes, fillet, chamfer, shell, draft, split, mirror, patterns, variables, gears, puzzle and Spectre tops, assemblies with six mate kinds and sub-assemblies, PDF shop sheets, range-of-motion sheets, DXF/SVG, STL/3MF/STEP (faceted), photo measuring, turntable scanning, the duplicator check | all of the above except the last six, plus surfaces, sheet metal, plastic features, frames, threads, coil, configurations, unit-aware parameters, joints with limits, motion studies, contact sets, cams, gears and screws as motion links, exploded views, render workspace, STEP AP242 in and out with assemblies, IGES, glTF, URDF, FreeCAD and Rhino import |
| Not there | exact curved surfaces, real STEP import, drawings beyond what `ok-sheet` draws | 2D drawings (deferred; 3 of 21 pieces, on a branch), CAM, simulation |
| User interface | a web client (three.js) and a document server | a desktop app (egui + wgpu), the same app in the browser as wasm, a CLI, an MCP server |
| Agents | `ok-mcp`: apply ops, report, screenshot (sectioned), export, range of motion, duplicator check; `docs/MCP.md` | `solidcraft-cli mcp`: `batch` with rollback and `expect` (volume, body count, bbox), `measure`, `body_topology`, `inspect_design`, `screenshot`, `compare_picture`, `check_parameters`, bundled skills for modelling, mechanisms, checking; file access sandboxed to named folders |
| Cloud | a document server with real-time multi-user editing, accounts, teams, versions, branches | none. Local files; the Docker web image "keeps nothing on the server" |
| Build | `cargo build --release -p ok-mcp` about 4 min cold; 9 MB binary; the full pre-push chain 10 min warm | `cargo build --release -p solidcraft-cli` 16.5 min cold on 4 cores, 183 crates, 3.4 GB of `target`, 637 MB binary (line tables in); or the AppImage, deb, snap, Flatpak, which carry the CLI |
| Pace | one maintainer's evenings | daily releases (v0.26 to v0.30.1 in a week, v0.40 release candidates in the tree); 14.7k stars, 2k forks, 5,220 open issues, 890 open pull requests |
| Licence | MIT | MIT or Apache-2.0 |

## The experiment

`examples/duplicator/solidcraft/build_sc.py` builds the duplicator
from `build.py`'s own constants through SolidCraft's MCP server: 28
parts as components, the three moving groups as nested components, four
slider joints, the depth sequence and the reach driven and
photographed, a 72-pose interference sweep over the work, a bill of
materials, a hidden-line DXF, STEP and the `.sc` design. Its README has
the table of what came out and what could not.

What it took: an afternoon. Reading SolidCraft's docs and skills, a
450-line client and port, and four parameter corrections found by
running it (`document.metadata.set` wants `values`, the DXF export wants
a camera in radians, a drive outside a joint's limits is clamped rather
than refused, a drive to the value a joint already has reports
`moved: false`). Nothing in the geometry failed: 300-odd booleans
(boxes and cylinders cut by cylinders and boxes, two sketched prisms)
all closed, the same total volume as offkilter's model to 0.005 %
(43,033 against 43,031 cm³: offkilter's 2 mm cylinders are 72-gons),
every joint created without moving a part, every drive landing where
the numbers said.

What was better than offkilter:

- The kernel is exact and fast. A `solid.box` cut through a part with
  a dozen bodies takes milliseconds; the whole machine builds in 14 s.
- `batch` with rollback and `expect` is the right shape for an agent:
  one call per part, the design unchanged on failure, the check in the
  same call.
- `inspect.interference` over 38 bodies is one call and 1.5 s, with
  the overlap volumes; the equivalent in offkilter is a pairwise
  boolean loop in the Rust test.
- Joints by a world point and axis, limits, drives, motion studies,
  `joint.sweep` until contact and `joint.find_limits` exist; offkilter
  has the mates and the preview and nothing above them.
- It exports a real STEP: the shafts are cylinders, not 72 planes.
- It has a desktop and a browser app someone else maintains, with no
  server.

What was worse, or missing:

- No drawings. Every example in `examples/` ends in PDF sheets with
  views, hidden lines, dimensions, callouts, sections, balloons and a
  parts list, and the mechanisms in range-of-motion sheets; those are
  what gets taken to the shop. SolidCraft has the current view as a
  DXF with hidden lines removed and a deferred milestone.
- No duplicator check, no puzzle or Spectre features, no gear feature
  (SolidCraft's gears are motion links, not involutes), no photo
  measuring, no scanner. These are offkilter's own and would have to
  be rewritten as Python that generates sketches, or stay in
  offkilter.
- The regression tests. offkilter's read the document and regenerate
  it in-process in Rust, numbers against numbers, in CI. SolidCraft's
  counterpart is a Python script over MCP (or `solidcraft-cli run
  --json-out`), which is what `build_sc.py` is; or Rust tests against
  `solidcraft-engine` as a library, which means the 16-minute build in
  CI.
- Churn. The homepage says v0.22, GitHub's latest is v0.30.1, the tree
  is v0.40.0-rc.2; the kernel was swapped three weeks ago; the file
  format number moves and an older version refuses a newer design;
  the docs still describe the old kernel in places. Five thousand open
  issues, two of today's are "REVOLVE: operation failed". A design
  saved today should be kept as a script, which the MCP workflow does
  anyway.
- Size. A million lines, mostly agent-written, in a codebase that
  cannot be read whole; a fix the maintainer needs waits on upstream
  (890 open pull requests) or lives in a fork. offkilter's 75k lines
  are fixable in a session.
- Small things: headless screenshots fit the model loosely and give
  every body the same grey (colours were painted per part); the `dof`
  list reports 6 for every nested occurrence although the groups move
  rigidly; a sub-assembly cannot be photographed alone without hiding
  the rest.

## Is it time to move

Not wholesale, and not to stay put either.

The CAD core is decided. SolidCraft's kernel, assemblies, exchange
formats, apps and agent interface are ahead of offkilter's by an
amount offkilter will not close: exact surfaces and real STEP import
are the two biggest items on offkilter's roadmap and SolidCraft has
them, with a company and a crowd behind it. Writing more kernel, web
client or server code in offkilter is now effort spent on a worse copy
of something free. That includes the cloud layer, which the premise
already retires.

What is not decided is the workshop layer, which is the part the
maintainer actually uses: the sheets, the range-of-motion pages, the
duplicator check, the puzzle and Spectre generators, the photo and
scan tools, and the examples with their tests. None of it exists in
SolidCraft and the one that matters most, drawings, is deferred there.

So the recommendation is a split, in this order:

1. **Stop building offkilter's kernel, client and server.** Freeze
   them; keep them building. Delete nothing yet.
2. **Model new rail-and-ply machines in SolidCraft** through its MCP
   server, with `scmcp.py`'s `Part` as the shim so a build script
   reads like the existing ones, and the checks as `expect`,
   `measure`, `occurrence.properties` and `inspect.interference` in
   the same script. The duplicator shows it takes an afternoon per
   project of that kind; the router lift, chop saw, cutoff grinder,
   jack table and the cart's frame are the same kind.
3. **Bridge the sheets.** `ok-sheet` draws from `ok_brep` solids, and
   `ok-step` already imports planar and cylindrical faces, which is
   everything these machines are made of. A `sheet` command that
   takes SolidCraft's STEP (names and assembly structure included) and
   writes the same PDFs is the one piece of new offkilter code worth
   writing; it keeps the shop output and the sheet tests, and it is
   what a move depends on.
4. **Leave the generators where they are** for now: the puzzle,
   Spectre and gear features, the duplicator check, the photo and scan
   tools stay in offkilter's kernel and MCP server, which `--file` runs
   without the server. If one is wanted in SolidCraft, it becomes a
   Python generator of sketches, not a port of the feature.
5. **Decide the rest when SolidCraft's drawings land** (milestone 8,
   the `drawings` branch). If they come with parts lists, sections and
   a way to put a mechanism at several positions on one sheet, the
   bridge in step 3 retires and offkilter becomes the generators and
   the examples' history.

The one reason to move everything now would be wanting the desktop app
day to day. The one reason to stay entirely would be not trusting a
kernel swapped three weeks ago in a project with five thousand open
issues; the duplicator did not find a reason for that distrust, but it
is all planes and cylinders.
