# Router lift and pin router, built through MCP

A bench-top trim-router table with an integral lift, plus a hinged
overarm pin attachment: a real project designed in a chat, originally as
OpenSCAD models, rebuilt here as an offkilter document by a script that
drives the `ok-mcp` server exactly as a language model would.

- `build.py`: the build. Speaks JSON-RPC to `ok-mcp --file` over stdio,
  makes one part studio per part (sketches, extrudes, holes, revolves,
  slots) and an assembly of 32 instances, 25 fixed and 7 hung off one
  slider mate (the carriage on its shafts), reads each part back through
  `report`, pictures it through `screenshot` and exports the printed
  parts through `export`.
- `out/router_lift.okpart`: the document the script writes; open it in
  the web app (Docs, Open file) or point `ok-mcp --file` at it.
- `out/*.png`: the script's screenshots (the assembly, a section along
  the leadscrew at mid travel and with the carriage raised, every part).
- `reference/`: the OpenSCAD sources (rev C), the reference STLs they
  produced for the printed parts, and the arm template DXF.
- `build-instructions.md`: the shop instructions the project came with.

```sh
cargo build --release -p ok-mcp
python3 examples/router-lift/build.py
cargo test -p ok-render --test router_lift     # what CI runs
```

The test regenerates every tab of the committed document, requires every
part to be a closed solid, the assembly to place every instance and its
mates to resolve to exactly the poses the parts were drawn at, and
compares each printed part with its reference mesh: volume within half a
percent and extents within 0.2 mm. On the branch this landed on, the
printed parts match to 0.02 percent or better; what remains is the
facet count (the kernel's 5° facets against OpenSCAD's `$fn = 96`).

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
  socket entry use; the chamfered dowel is still a revolved profile,
  where a chamfer on the profile would say it in one op.
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
