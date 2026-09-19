# offkilter

An experiment in building an open source, browser-based parametric CAD
system in the spirit of Onshape: a real geometry kernel, a feature-based
part studio, and a web client, all under the MIT license.

**Status: usable for simple parts, early for everything else.** What
works today, all in the browser:

- Sketching: lines, rectangles, circles and arcs drawn on standard planes
  or on faces of bodies, with a constraint solver (18 constraint types,
  degrees-of-freedom reporting), inferred coincident / horizontal /
  vertical constraints while drawing, point dragging, construction
  geometry, and dimension labels you click to edit.
- Solids: extrude (blind, through all, up to face), revolve, fillet and
  chamfer on straight edges, mirror and linear / circular patterns, with
  boolean new / add / remove / intersect on a polyhedral
  boundary-representation kernel that tags curved faces with their
  analytic surface.
- Parametrics: an ordered feature list with suppression, reordering and a
  rollback bar; faces and edges referenced by origin so downstream features
  survive edits; variables and expressions (`#width / 2`) bound to any
  dimension; a regeneration cache so late edits are cheap.
- Workflow: undo/redo, `.okpart` JSON documents, STL export, mass
  properties.

Not yet: exact curved surfaces (arcs are 5° facets), sweeps and lofts,
shells, assemblies, drawings, STEP, server-side regeneration and
collaboration. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Layout

| Path | What |
| --- | --- |
| `crates/ok-math` | Vectors, planes, tolerances. |
| `crates/ok-sketch` | Sketch entities, constraints, Levenberg–Marquardt solver, closed-region extraction. |
| `crates/ok-brep` | Boundary-representation solids: extrude, revolve, booleans, blends, transforms. |
| `crates/ok-mesh` | Triangle meshes for display. |
| `crates/ok-model` | Part studio, features, operations (`Op`) and regeneration. |
| `crates/ok-wasm` | WebAssembly bindings used by the web client. |
| `apps/web` | Vite + TypeScript + three.js client. |
| `docs/` | Architecture and roadmap. |

Everything in the kernel is plain Rust with `f64` math and serde types, so
it runs natively (tests, future server) and in the browser (via wasm).

## Getting started

Prerequisites: Rust stable with the `wasm32-unknown-unknown` target
(`rust-toolchain.toml` adds it), `wasm-bindgen-cli` matching the version
in `crates/ok-wasm/Cargo.toml`, and Node 22.

```sh
cargo install wasm-bindgen-cli --version 0.2.128 --locked
cargo test --workspace          # kernel tests
./scripts/build-wasm.sh         # compile kernel to apps/web/src/wasm
cd apps/web && npm install && npm run dev
```

Open http://localhost:5173. The app loads an example plate on first run.
Select a feature to edit it; press `f` to fit the view. In sketch mode:
`L` line, `R` rectangle, `C` circle, `A` arc, `S` select, `Q` construction,
`Esc` finishes; right-drag orbits. Click a face to select it, then
"+ Sketch" sketches on it. Documents are saved as `.okpart` JSON files and
also kept in the browser's local storage.

## Using the kernel from Rust

```rust
use ok_model::{Op, PartStudio, PlaneSpec, SketchOp, StandardPlane};
use ok_math::Vec2;

let mut ps = PartStudio::new("bracket");
let s = ps.apply(Op::AddSketch { plane: PlaneSpec::standard(StandardPlane::Top), name: None })?.feature.unwrap();
ps.apply(Op::Sketch { id: s, op: SketchOp::AddRectangle { a: Vec2::ZERO, b: Vec2::new(40.0, 20.0) } })?;
ps.apply(Op::AddExtrude { sketch: s, depth: 10.0, direction: Default::default(), profiles: Default::default(), op: Default::default(), name: None })?;
let result = ps.regenerate();
assert_eq!(result.bodies.len(), 1);
```

Every edit is an `Op`, serialisable as JSON. That is also the wasm API:
`Studio.apply(json)` then `Studio.regenerate()`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Good first areas are listed in
the roadmap.

## License

MIT. See [LICENSE](LICENSE).
