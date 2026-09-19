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
  geometry, trim, offset and mirror (with symmetric constraints),
  dimension labels you click to edit, and a Use tool that
  projects body edges and face outlines into the sketch as geometry that
  follows the model.
- Solids: extrude (blind, through all, up to face), revolve, sweep along a
  sketched path, loft between two sketches, holes with counterbores, fillet and
  chamfer on straight and curved edges, mirror and linear / circular patterns, with
  boolean new / add / remove / intersect on a polyhedral
  boundary-representation kernel that tags curved faces with their
  analytic surface.
- Parametrics: an ordered feature list with suppression, reordering and a
  rollback bar; faces and edges referenced by origin so downstream features
  survive edits; variables and expressions (`#width / 2`) bound to any
  dimension; a regeneration cache so late edits are cheap.
- Assemblies: a document holds part studio and assembly tabs; an
  assembly inserts bodies from part studios as instances and joins them
  with fastened, revolute, slider, cylindrical, planar and ball mates
  between faces (closed loops solved numerically), sub-assemblies,
  with offset, angle and flip, resolved as chains from fixed instances,
  plus an interference check between instances.
- Workflow: per-user undo/redo (inverse ops, so undoing in a shared
  document only reverts your own edit), `.okpart` JSON documents, STL export, mass
  properties.

- Cloud: a small document server with real-time multi-user editing;
  concurrent edits converge without conflicts, documents keep named
  versions, and optional accounts own documents and share them with
  other accounts.

Not yet: exact curved surfaces (curved faces are facets at an adjustable
resolution, 5° by default),
shells, drawings, STEP, and teams. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Layout

| Path | What |
| --- | --- |
| `crates/ok-math` | Vectors, planes, tolerances. |
| `crates/ok-sketch` | Sketch entities, constraints, Levenberg–Marquardt solver, closed-region extraction. |
| `crates/ok-brep` | Boundary-representation solids: extrude, revolve, booleans, blends, transforms. |
| `crates/ok-mesh` | Triangle meshes for display. |
| `crates/ok-model` | Documents with part studio and assembly tabs, features, operations (`DocOp`, `Op`) and regeneration. |
| `crates/ok-wasm` | WebAssembly bindings used by the web client. |
| `crates/ok-server` | Document server: storage, static app, real-time op relay. |
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
cargo test --workspace          # kernel and server tests
./scripts/build-wasm.sh         # compile kernel to apps/web/src/wasm
cd apps/web && npm install && npm run dev
```

Open http://localhost:5173. The app loads an example plate on first run.
Select a feature to edit it; press `f` to fit the view. In sketch mode:
`L` line, `R` rectangle, `C` circle, `A` arc, `S` select, `Q` construction,
`Esc` finishes; right-drag orbits. Click a face to select it, then
"+ Sketch" sketches on it. Documents are saved as `.okpart` JSON files and
also kept in the browser's local storage.

## Running the document server

```sh
cd apps/web && npm run build && cd ../..
cargo run -p ok-server -- --static apps/web/dist --data ./data --port 8080
```

Open http://localhost:8080, click **Docs**, create a document, and share
its URL (`?doc=<id>`): everyone with it edits the same feature list live.
Click **Sign in** to create an account: documents you create while signed
in are yours, listed only for you and the accounts you share them with
(**Share…** in the Docs dialog). Documents created without signing in stay
open to everyone on the server. Passwords are stored as argon2id hashes in
`data/users.json`; sessions are cookies. Put the server behind HTTPS
before exposing it beyond a trusted network.
During development run `npm run dev` in `apps/web`; it proxies `/api` to
the server on port 8080. The Docs dialog also saves and restores named
versions of a document.

Or with Docker: `docker build -t offkilter . && docker run -p 8080:8080 -v offkilter-data:/data offkilter`.

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
