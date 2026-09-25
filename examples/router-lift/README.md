# Router lift and pin router, built through MCP

A bench-top trim-router table with an integral lift, plus a hinged
overarm pin attachment: a real project designed in a chat, originally as
OpenSCAD models, rebuilt here as an offkilter document by a script that
drives the `ok-mcp` server exactly as a language model would.

- `build.py`: the build. Speaks JSON-RPC to `ok-mcp --file` over stdio,
  makes one part studio per part (sketches, extrudes, holes, revolves,
  slots), three sub-assemblies (the carriage with its blocks and nut,
  the leadscrew with its bearings and collars, the arm with its chuck,
  guide pin and dowels) and the lift assembly placing them and the
  loose parts, 33 bodies in all, with the carriage assembly and the
  router hung off one slider mate (a block's bore on its shaft) and the
  arm assembly on a revolute about the piano hinge's knuckle; reads
  each part back through `report`, pictures it through `screenshot`,
  exports the printed parts through `export`, and checks the bill of
  materials against the two shop drawings' BOM tables
  (`reference/bom_*.csv`).
- `out/router_lift.okpart`: the document the script writes; open it in
  the web app (Docs, Open file) or point `ok-mcp --file` at it.
- `out/*.png`: the script's screenshots (the assembly, a section along
  the leadscrew at mid travel and with the carriage raised, every part).
- `out/assembly.pdf`, `out/carriage_assembly.pdf`,
  `out/leadscrew_assembly.pdf`, `out/arm_assembly.pdf`,
  `out/carriage.pdf`: shop drawing sheets from `export {format: "pdf"}`:
  third-angle views at a standard scale with the overall sizes; the
  lift's sheet has a balloon per item and the parts list, where each
  sub-assembly is one item with its own sheet listing its parts, and
  the carriage's sheet has its holes called out.
- `reference/`: the OpenSCAD sources (rev C), the reference STLs they
  produced for the printed parts, and the arm template DXF.
- `build-instructions.md`: the shop instructions the project came with.

```sh
cargo build --release -p ok-mcp
python3 examples/router-lift/build.py
cargo test -p ok-render --test router_lift --test router_lift_limits   # what CI runs
cargo test -p ok-render --test router_lift_limits -- --nocapture       # with every number
```

The test regenerates every tab of the committed document, requires every
part to be a closed solid, the assembly to place every instance and its
mates to resolve to exactly the poses the parts were drawn at, and
compares each printed part with its reference mesh: volume within half a
percent and extents within 0.2 mm. On the branch this landed on, the
printed parts match to 0.02 percent or better; what remains is the
facet count (the kernel's 5° facets against OpenSCAD's `$fn = 96`).

## Mechanism at its limits

`router_lift_limits.rs` is the definition of done from the design
brief: the carriage swept to the ends of its travel and the arm on its
hinge, with every clearance and alignment measured and every pair of
placed bodies checked for interference at each position, plus the
carriage's own features (the bore stays whole, every bolt hole reaches
its nut trap). What it measures, at rev C:

| Check | Result |
|---|---|
| Interference, carriage at min / mid / max, 528 pairs | none, apart from the hinge knuckle let into the rail's corner (1226 mm³, as the SCAD draws it) |
| Carriage bottom to the lower SK20 at min | 3.00 mm |
| Carriage top to the top's underside at max | 23.00 mm |
| Leadscrew parallel to each shaft over the travel | 0.0000 mm |
| Router through the 74 mm opening at max rise | 4.50 mm radial clearance |
| Carriage volume against the SCAD mesh | +0.02 % |
| Nut traps and the router bore | 5.7 mm of wall, bore one clean piece |
| Block bolt holes reaching their traps | 16 of 16 |
| Guide pin on the bit axis, level | 0.0000 mm |
| Arm underside above the table at the nose | 75.0 mm |

And what it found that the SCAD did not:

- **The rev C hinge cannot swing.** The knuckle is on the rail's front
  top corner and the arm lies over the rail, so lifting the nose turns
  the tail down into the rail: 39 cm³ of overlap at 5°. The hinge line
  belongs at the rail's back corner, or the arm should end at the hinge.
  Rev D replaces the hinge with a shaft pivot.
- **The guide pin runs 5 mm into the nose** as drawn (75 mm long, tip 5
  mm above the table, nose underside at 75 mm). A clearance hole over
  the bit axis, or the pin set lower, fixes it.
- **The leadscrew's coupling nut stands 31 mm proud of the table**, 45
  to 62 mm behind the bit, inside the space a template would use. The
  registration posts stand there too (0 to 75 mm tall, 50 to 90 mm
  behind the bit).
- **The bit tip is 2 mm below the table at max rise** with the ghost
  router's guessed collet and bit lengths; measure the real router.
- **The drawings' BOM says two guide pins; the model places one.** Every
  other modelled line matches.

For a closer look at any part, `compare_stl` booleans the kernel's part
against the mesh both ways and lists the lumps of material each has that
the other lacks, with their extents:

```sh
cargo run -p ok-render --example compare_stl -- \
    examples/router-lift/out/router_lift.okpart examples/router-lift/reference/carriage.stl --tab Carriage
```

## What the port found

Things the project needed that were missing, awkward, or wrong, in the
order they came up. They are the roadmap this example feeds.

- The rev C carriage has its body 4 mm deeper, wider clamp ears and its
  leadscrew nut 2 mm further back than the hand-transcribed Rust example
  in `crates/ok-model/examples/router_lift.rs` (which is kept as it
  arrived). Every difference the mesh comparison reported was a number
  in the transcription, not the kernel.
- A cone-bottomed socket, a countersink and a chamfered dowel were all
  revolved profiles at first. The hole feature now takes a `countersink`
  (`{diameter, angle}`), which the chuck's screws and the round post's
  socket entry use, and the sketch has a `chamfer` op (a line across a
  corner, next to its `fillet`), which the dowel's profile uses.
- The slotted post's socket is a hull of two cones in OpenSCAD; here it
  is two revolved cones and the section extruded between them. A slot
  with a conical entry has no single feature.
- The posts' screw countersinks are on the bottom face in the SCAD (the
  cone opens downwards into the table), which looks like a slip in the
  original; they are modelled as drawn so the comparison holds.
- The mesh comparison's boolean fails for the posts and the chuck: the
  reference mesh's cones and countersinks coincide with the kernel's
  exact ones, and coincident curved faces with different facets are
  beyond the boolean. Volumes and extents carry the check there.
- A section keeps the half on the axis's positive side, which from the
  default isometric eye shows that half's outside; `x:0:flip` keeps the
  half whose cut faces face the camera. Worth a hint in the tool's
  description, since the first try looked like missing parts.
- The instances were first placed by fixed placements copied from the
  SCAD, so moving the carriage along its travel meant editing numbers.
  Now one slider mate between a pillow block's bore and its shaft
  carries the travel, and fastened mates through bolt holes, the router
  bore and the nut pocket hang the carriage, the other blocks, the
  router and the nut off it: `set_mate {offset}` on the slider raises
  the lot (`out/assembly_raised.png`). The script derives each mate's
  offset, angle and flip from the placements it already knew, which
  needed two things the tool did not give: the rule for a connector's
  frame (now in `docs/OPS.md`) and where the assembly actually put each
  instance (now `placed` on every instance in the report).
- The instructions say to print `arm_template.dxf` full size and cut the
  arm to it, and drawings are the client's: there was no way to ask the
  tool for one. `export {format: "dxf", view: "top"}` now writes a view's
  edges at 1:1; the build exports the arm's plan
  (`out/arm_template.dxf`) and checks that the template's six lines are
  in it, as does the regression test.
- The first run had the carriage 0.59 mm off along the bolts: a cylinder
  connector's origin was the average of its facets' vertices, which the
  carriage's nut traps had shifted by cutting some facets, while the
  report's centroid is area-weighted. The kernel now uses the same
  area-weighted centroid, so a connector no longer moves when a later
  feature splits a facet and the report says where it is.
