# offkilter

An experiment in building an open source, browser-based parametric CAD
system in the spirit of Onshape: a real geometry kernel, a feature-based
part studio, and a web client, all under the MIT license.

**Status: early foundation.** The kernel can solve 2D sketches with
geometric and dimensional constraints, find their closed regions, and
extrude them into solid meshes. The web client edits the feature list and
regenerates the part live. There are no booleans, fillets, assemblies or
collaboration yet. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Layout

| Path | What |
| --- | --- |
| `crates/ok-math` | Vectors, planes, tolerances. |
| `crates/ok-sketch` | Sketch entities, constraints, Levenberg–Marquardt solver, closed-region extraction. |
| `crates/ok-mesh` | Triangle meshes and extrusion. |
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
Select a feature to edit it; press `f` to fit the view. Documents are saved
as `.okpart` JSON files and also kept in the browser's local storage.

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
