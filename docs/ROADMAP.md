# Roadmap

Rough order of work toward a usable parametric modeller. Items near the
top are concrete and self-contained; items lower down are directions.

## Sketching

- [x] Points, lines, circles, arcs
- [x] Geometric and dimensional constraints with DOF reporting
- [x] Closed-region extraction with holes
- [x] Split edges at crossings and T-junctions so intersecting lines and arcs form regions
- [x] Interactive sketch mode in the client: draw on the plane, drag points, click to pick entities for constraints, inferred coincident / horizontal / vertical constraints while drawing
- [x] Arc tool (centre, start, end) and construction geometry
- [x] Regular polygon and slot tools (constrained to stay regular / tangent)
- [x] Trim, offset, mirror within a sketch (symmetric constraints on mirrored points)
- [x] Dimension labels in the viewport with click-to-edit (values or expressions)
- [ ] Analytic Jacobians for the common constraints (performance)
- [ ] Splines (B-spline entity + point-on-curve)
- [x] Sketch on planar faces of bodies (face references)
- [x] Project / use edges and face outlines from bodies in sketches (fixed entities that follow the model)
- [x] Construction geometry, mirror
- [x] Sketch patterns: linear copies held by distance constraints, circular copies by a rotated-point constraint about a construction centre

## Solids

- [x] Extrude regions (normal / reverse / symmetric)
- [x] Boundary representation: shared-vertex planar faces with analytic surface tags (plane, cylinder) and face origins
- [x] Booleans (union / subtract / intersect) on polyhedral solids, with exact coplanar handling
- [x] Adjustable facet resolution per document (0.5° to 30° per facet) flowing through profiles, revolves, holes and blends
- [ ] Exact curved faces (cylinder, later general surfaces) instead of facets, with curve/surface intersection. This is a multi-month kernel program: ellipse and quartic intersection curves, trimmed parametric faces, and tangent/coincident degeneracies; the surface tags on faces are the seed for it.
- [x] Merge coplanar faces of the same body after a boolean
- [x] Extrude up-to-face / through-all
- [x] Revolve (about a sketch axis or sketch line, full or partial)
- [x] Hole feature (through / blind, optional counterbore) at sketch points
- [x] Sweep (profile along a sketched polyline / arc path) and loft (between two sketch regions)
- [x] Fillet and chamfer on straight edges (blend by boolean with a cutter prism per edge)
- [x] Fillets and chamfers on curved edges (a rim is one chain, blended with one mitred sweep)
- [ ] Proper corner patches where blends meet; variable-dihedral chains
- [x] Shell (uniform wall, chosen faces open; an offset polyhedron subtracted in one boolean; cavities whose offset changes the topology are reported, not guessed)
- [x] Draft (tilt planar faces about a neutral plane) and Move face (push / pull planar faces): direct edits that re-solve the surrounding corners
- [x] Mirror and linear / circular patterns of bodies
- [x] Boolean feature between existing bodies (union / subtract / intersect, tools optionally kept)
- [x] Patterns and mirrors of features (the named features' tool volumes are replayed with their own add / remove operation)
- [x] Mirror / pattern selected bodies only (tick bodies in the panel)
- [ ] Patterns of faces
- [~] Persistent naming: face origins survive booleans; needs disambiguation for split faces and edge/vertex references

## Document and client

- [x] Ordered feature list with suppression, reorder, delete
- [x] JSON document format and browser persistence
- [x] Undo/redo (document snapshots in the client; op-log based undo later)
- [x] Variables and expressions in dimensions (`#width / 2`) via per-field bindings
- [x] Face selection in the viewport with picking
- [x] Edge selection (edges are named by their two faces)
- [x] Rollback bar in the feature list
- [x] Part hide/show and rename from the parts list; standard views (top/front/right/iso); measure tool (point to point, corner snap); section view (axis-aligned clipping)
- [x] Multiple part studios per document; assemblies with fastened / revolute / slider / cylindrical mates resolved as chains from fixed instances
- [x] Interference check between placed instances (boolean intersection, on demand)
- [x] Numeric mate solver: closed loops and redundant mates solved over the free degrees of freedom of revolute, slider and cylindrical mates
- [x] Planar and ball mates; sub-assemblies (an assembly tab inserted as one rigid group, cycles refused); mate frames drawn in the viewer
- [x] Explode view slider (display only) and dragging free instances in the viewport (one undo step per drag)
- [ ] Mate connectors on edges and vertices; mate animation
- [x] STL and 3MF export of bodies (3MF keeps part names); DXF export of a sketch
- [x] Drawings: front / top / right / isometric views with exact hidden-line removal for the faceted geometry, laid out third-angle on an A4 sheet (SVG) or as DXF lines
- [ ] Drawing dimensions, section views and detail views; STEP once faces are exact
- [ ] Import: STEP via a B-rep reader

## Platform

- [x] Document server (`ok-server`): storage, static app, REST
- [ ] Server-side regeneration / validation beyond op checking
- [x] Named versions of a document (save / restore on the server)
- [ ] Branches, merges and diffs over the op log
- [x] Real-time multi-user editing (ops relayed in server order)
- [x] Conflict-free concurrent editing: per-client id ranges make ops commute; structural hashes detect and repair any divergence
- [x] Per-user undo in shared documents (inverse ops instead of document replacement)
- [x] Accounts (argon2id passwords, cookie sessions) and per-document sharing
- [x] Server hardening: sign-in rate limiting per address, request and WebSocket size limits, security headers, `--secure-cookies`
- [x] Read-only collaborators: share as viewer; the server refuses their edits and the client shows a read-only badge
- [ ] Teams and invitations by link

## Engineering

- [x] CI: fmt, clippy, tests, wasm build, web build, browser end-to-end suite
- [x] Dockerfile for the server + web app
- [x] Randomised boolean robustness tests: grid-aligned box/cylinder sequences, and general-position sequences with rotated tools and near-coincident nudges (`--ignored` long runs; `OK_FUZZ_SEED` replays one seed)
- [x] Realistic-parts corpus (`crates/ok-model/tests/parts.rs`): brackets, revolved flanges, pockets with counterbores, bosses on oblique faces, pulleys, grazing cuts, sweeps and lofts, patterns then fillets
- [x] Randomised property tests for the solver and region extraction (`crates/ok-sketch/tests/property.rs`)
- [x] Regeneration cache: unchanged feature prefixes are reused, so editing late features is cheap
- [x] Benchmarks for regeneration time on realistic parts (`scripts/bench.sh`); the boolean's T-junction grid and section prefilters came out of the first run (a 4000-face cover shells in 0.65 s, from 3.9 s)
- [x] `wasm-opt` in the release pipeline (from the binaryen npm package when present)
