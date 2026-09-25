# offkilter

An experiment in building an open source, browser-based parametric CAD
system in the spirit of Onshape: a real geometry kernel, a feature-based
part studio, and a web client, all under the MIT license.

**Status: usable for simple parts, early for everything else.** What
works today, all in the browser:

- Sketching: lines, rectangles, circles, arcs and splines drawn on standard or angled planes
  or on faces of bodies, with a constraint solver (20 constraint types,
  degrees-of-freedom reporting), inferred coincident / horizontal /
  vertical constraints while drawing, point dragging, construction
  geometry, trim, offset, fillet and mirror (with symmetric constraints),
  dimension labels you click to edit, constraint glyphs you click to
  remove, and a Use tool that
  projects body edges and face outlines into the sketch as geometry that
  follows the model.
- Solids: extrude (blind, through all, up to face), revolve, sweep along a
  sketched path, loft between two sketches, holes with counterbores, fillet and
  chamfer on straight and curved edges (three fillets meeting at a corner
  get the exact rolling-ball patch), shell, move face, draft, split by a
  plane, mirror and linear / circular patterns, with
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
  between faces, edges or corners (closed loops solved numerically), sub-assemblies,
  with offset, angle and flip, resolved as chains from fixed instances,
  plus an interference check between instances and a display-only
  animation of any spinning or sliding mate.
- Workflow: per-user undo/redo (inverse ops, so undoing in a shared
  document only reverts your own edit), `.okpart` JSON documents, STL and OBJ import, DXF import into sketches, STL, 3MF, STEP (faceted B-rep) and DXF export, PNG snapshots, drawing
  sheets (SVG/DXF, A4 to A2 or Letter, chosen views with a live preview)
  with hidden-line removal, overall dimensions, diameter callouts for
  holes and bosses, dimensions you place
  between corners in the preview, a hatched section view, a detail
  view of the selected face and a parts list with item balloons for
  assemblies, bills of materials as CSV, mass properties with per-part
  materials, and a measure tool for corners, faces, edges and cylinders.

- Cloud: a small document server with real-time multi-user editing;
  concurrent edits converge without conflicts, documents keep named
  versions, and optional accounts own documents and share them with
  other accounts as editors or read-only viewers, by name, by team or by
  invitation link; documents can be branched and merged back, and the
  list shows a preview of each.
- Import: STL, OBJ and STEP (faceted: planar and cylindrical faces)
  files become mesh bodies; DXF into sketches.
- Measuring from a photo: print the measuring sheet
  (`examples/measuring-sheet/`), lay parts on it, photograph it, and
  the sizes, outlines and holes come back in millimetres, or as a
  sketch ready to extrude.
- Agents: `ok-mcp` is a Model Context Protocol server that lets a
  language model build and edit documents through the same ops the
  client uses, live in your browser or on a local file, with a report
  of features, faces and errors after every step and a rendered
  screenshot of any view (sectioned if wanted) to check its work; see
  [docs/MCP.md](docs/MCP.md) and the op reference in
  [docs/OPS.md](docs/OPS.md).

Not yet: exact curved surfaces (curved faces are facets at an adjustable
resolution, 5° by default, and STEP files carry those facets as planar
faces) and STEP import. See
[docs/ROADMAP.md](docs/ROADMAP.md).

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
`L` line, `R` rectangle, `C` circle, `A` arc, `B` spline, `I` fillet, `S` select, `Q` construction,
`Esc` finishes; right-drag orbits. Click a face to select it, then
"+ Sketch" sketches on it. Documents are saved as `.okpart` JSON files and
also kept in the browser's local storage.

## Running the document server

```sh
cd apps/web && npm run build && cd ../..
cargo run -p ok-server -- --static apps/web/dist --data ./data --port 8080
# add --secure-cookies when serving over HTTPS (directly or behind a TLS proxy)
```

Open http://localhost:8080, click **Docs**, create a document, and share
its URL (`?doc=<id>`): everyone with it edits the same feature list live.
Click **Sign in** to create an account: documents you create while signed
in are yours, listed only for you and the accounts you share them with
(**Share…** by account name, or **Invite link…** for a URL that lets any
signed-in account join, in the Docs dialog). Documents created without signing in stay
open to everyone on the server. Passwords are stored as argon2id hashes in
`data/users.json`; sessions are cookies. Put the server behind HTTPS
before exposing it beyond a trusted network. Scripts can validate and
export without a browser: `GET /api/docs/<id>/check` regenerates every
tab and lists bodies and errors as JSON, and
`GET /api/docs/<id>/export/stl?tab=<n>` returns a tab's bodies as STL
(`export/step` as STEP, `export/dxf?view=top` a view's edges as DXF,
`export/pdf?views=front,top,right,iso&sheet=A4` a shop drawing sheet
as PDF).
During development run `npm run dev` in `apps/web`; it proxies `/api` to
the server on port 8080. The Docs dialog also saves and restores named
versions of a document and compares any of them with the document as it
is now (features added, changed or removed, per tab), and branches a
document or a version into a new document of your own. A branch can be
merged back into its origin, or pull the origin's later work, as a
three-way merge feature by feature; anything changed on both sides is
reported and left alone. Press `?` for the keyboard shortcuts.

Or with Docker: `docker build -t offkilter . && docker run -p 8080:8080 -v offkilter-data:/data offkilter`.
For development, `Dockerfile.dev` is the pinned toolchain (Rust, wasm-bindgen,
Node, Playwright's Chromium, Claude Code) with the checkout mounted; its
header has the run line.

## Driving it from your phone

A model can build and edit documents through `ok-mcp` (see
`docs/MCP.md`), and Claude Code's Remote Control lets the Claude mobile
app drive a Claude Code session running on a machine of yours. Put the
two together and you can ask for a part from the sofa and watch it
appear in the browser. Nothing is exposed to the internet: the phone
reaches the session through Anthropic's relay, and the model reaches the
document server on localhost.

On a machine that stays on (the repository checked out, `tmux` and
Claude Code installed, signed in with a claude.ai account):

```sh
cargo build --release -p ok-server -p ok-mcp
cd apps/web && npm install && npm run build && cd ../..
tmux new -d -s offkilter './target/release/ok-server --static apps/web/dist --data ./data --port 8080'
tmux new -d -s claude 'claude remote-control'
```

The repository's `.mcp.json` registers `ok-mcp` for any Claude Code
session started in it, pointed at the server on port 8080 (set
`OFFKILTER_URL` to point it elsewhere). Claude Code asks once whether to
trust the project's MCP servers; say yes. Then open the session from the
Claude app and ask for something: the model reads the op reference,
applies ops through the server, and can hand back a `screenshot` so you
see the result in the chat. To watch live instead, open the document's
URL in the phone's browser over your LAN or a tailnet (`tailscale serve
8080` gives it an HTTPS name; add `--secure-cookies` to the server then).

If the server has accounts, add `"--login", "NAME:PASSWORD"` to the args
in `.mcp.json` (or register it with `claude mcp add` instead) so the
model edits as you; documents created signed out are open to everyone
on the server. The session goes offline seconds after the process ends,
which is what the `tmux` sessions are for; `claude remote-control
--continue` picks a stopped one back up.

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
