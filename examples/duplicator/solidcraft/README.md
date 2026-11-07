# The duplicator, rebuilt on SolidCraft

The carving duplicator of the parent directory, built through
[SolidCraft](https://github.com/tangency-labs/solidcraft)'s MCP server
instead of offkilter's, as the experiment behind
`docs/SOLIDCRAFT.md`. Every number comes from `../build.py` (the script
imports its constants), so the two models are the same machine: 28
parts in place at mid travel with Z raised, the three moving groups as
sub-assemblies, four sliders, the depth sequence and the reach, and a
sweep over the work in which only the tools may meet it.

```sh
git clone --depth 1 https://github.com/tangency-labs/solidcraft
cd solidcraft && cargo build --release -p solidcraft-cli     # 16.5 min cold on 4 cores, 3.4 GB of target
SOLIDCRAFT_CLI=.../target/release/solidcraft-cli python3 examples/duplicator/solidcraft/build_sc.py
```

`scmcp.py` is the client, the counterpart of `examples/okmcp.py`: a
stdio JSON-RPC loop over `solidcraft-cli mcp --headless`, and a `Part`
that collects one part's primitives (`solid.box`, `solid.cylinder`, a
sketch and extrude for the triangular webs) and sends them as one
`batch`. SolidCraft has no part studios: every body is in one design,
so a cut names its `targets`, which `Part` fills in with the part's own
bodies, and the bodies are then moved into a component named after the
part. The three moving groups are components with the parts' components
nested inside them, and a slider joint between a group and what it
rides on (`joint.create`, `flip: true`, both origins at a point on the
shaft's axis) moves the whole group.

## What it produces (`out/`)

| | |
|---|---|
| `duplicator.sc` | the design (1.7 MB; offkilter's `.okpart` of the same machine is 0.55 MB) |
| `duplicator.step` | STEP AP242 with the assembly structure (not committed, 1.3 MB) |
| `iso.png`, `front.png`, `right.png`, `top.png` | the drawn pose, rendered headless by SolidCraft's CPU rasterizer (the container has no GPU) |
| `section_tools.png`, `section_carriage.png` | `inspect.section` at y = -279.5 and x = 0, then a screenshot |
| `motion_z_1..3.png` | the depth sequence: raised, first pass on the stop screw, last pass with the screw backed off |
| `motion_xy_1..3.png` | the reach: the bit at the blank's front-left corner (pilot in the reference hole), the centre, the back-right corner |
| `bom.csv`, `bom.json` | `assembly.bom` at the top level: the 8 fixed parts, the 3 groups and the stop screw |
| `front.dxf` | `file.export_view_dxf`, the front view with hidden lines removed (613 lines) |
| `hardware.csv` | the fastener schedule, plain Python as in `build.py` |
| `build.json` | timings, the sweep's result, file sizes |

## What the run checks

The same claims as `crates/ok-render/tests/duplicator.rs`, through
`occurrence.properties` and `joint.drive`:

- joining moved nothing (every bounding box the same before and after
  each `joint.create`), no conflicts, nothing floating;
- each slider moves its group 10 mm along its axis and nothing else;
- travel X ±277.5, Y ±192.5, Z 62.5 down, read off the supports and
  blocks; the bit over the blank and the pilot over the lattice;
- tips 40 above the board raised, 4 mm deep on the screw, 12.5 at the
  last pass, 52.5 of the 62.5 mm of travel;
- the sweep: 72 poses (3 in X, 13 in Y every 25 mm, first and last
  pass, the stop screw backed off for the last), `inspect.interference`
  at each: 3790 pairs intersected, 88 overlaps of a tool in the work
  (the bit and pilot at the last pass, every one under 100 mm³), no
  other pair touching.

Total volume 43,033 cm³ in SolidCraft against 43,031 in offkilter (the
same 2 mm cylinders faceted at 5° there, exact here).

## What it cannot do

- **Shop sheets.** No `duplicator.pdf`, `gantry.pdf`, `x_slide.pdf`,
  `z_slide.pdf`: SolidCraft's drawings milestone is deferred (3 of 21
  pieces built, on a branch). What exists is the current view as a DXF
  with hidden lines removed, so a sheet with views, dimensions,
  balloons and a parts list would be offkilter's `ok-sheet` reading
  SolidCraft's DXF or STEP, or a drawing package written on top.
- **Range-of-motion sheets.** The poses are driven and photographed one
  by one; there is no side-by-side sheet with captions. A motion study
  and `motion.export_animation` would give a GIF instead.
- **Sub-assembly sheets.** A group alone cannot be photographed without
  hiding everything else; not done.
- **The duplicator check** (`duplicator_check`, `ok_brep::duplicate_check`):
  the height-field sample of a master and the bit rolled over it is
  offkilter's own and has no counterpart.
- **The per-body colours** offkilter's client gives every body were
  painted by hand (`appearance.assign`, one colour per part).

## Timings (4-core cloud container, no GPU)

| | offkilter | SolidCraft |
|---|---|---|
| Build the tool | 0 s (the release `ok-mcp` was built by the session hook; CLAUDE.md says about 4 min cold) | 16.5 min cold, 183 crates, 637 MB binary with line tables |
| Build the 28 parts | about 10 s of the 21 s run | 14 s (363 calls in all; the biggest part, the tool support with its pocket, slits and holes, 2.7 s) |
| Pictures, sheets, poses | the rest of the 21 s, including 6 PDFs and the two duplicator checks | 4 s for 14 PNGs |
| The sweep over the work | inside `cargo test -p ok-render --test duplicator`: the four tests, sweep included, 45 s warm | 110 s for 72 poses: 1.5 s per `inspect.interference` over 38 bodies |
| Whole run | 21 s | 130 s |
