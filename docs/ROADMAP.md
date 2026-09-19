# Roadmap

Rough order of work toward a usable parametric modeller. Items near the
top are concrete and self-contained; items lower down are directions.

## Sketching

- [x] Points, lines, circles, arcs
- [x] Geometric and dimensional constraints with DOF reporting
- [x] Closed-region extraction with holes
- [ ] Split edges at crossings so intersecting lines form regions
- [x] Interactive sketch mode in the client: draw on the plane, drag points, click to pick entities for constraints, inferred coincident / horizontal / vertical constraints while drawing
- [ ] Arc and polyline-with-arcs tools; trim; construction lines
- [ ] Dimension display in the viewport
- [ ] Analytic Jacobians for the common constraints (performance)
- [ ] Splines (B-spline entity + point-on-curve)
- [x] Sketch on planar faces of bodies (face references)
- [ ] Project / use edges from bodies in sketches
- [ ] Construction geometry, mirror, patterns

## Solids

- [x] Extrude regions (normal / reverse / symmetric)
- [x] Boundary representation: shared-vertex planar faces with analytic surface tags (plane, cylinder) and face origins
- [x] Booleans (union / subtract / intersect) on polyhedral solids, with exact coplanar handling
- [ ] Exact curved faces (cylinder, later general surfaces) instead of facets, with curve/surface intersection
- [x] Merge coplanar faces of the same body after a boolean
- [x] Extrude up-to-face / through-all
- [x] Revolve (about a sketch axis or sketch line, full or partial)
- [ ] Sweep, loft
- [ ] Fillet and chamfer
- [ ] Shell, draft
- [ ] Feature patterns and mirror
- [~] Persistent naming: face origins survive booleans; needs disambiguation for split faces and edge/vertex references

## Document and client

- [x] Ordered feature list with suppression, reorder, delete
- [x] JSON document format and browser persistence
- [x] Undo/redo (document snapshots in the client; op-log based undo later)
- [ ] Variables and expressions in dimensions (`width / 2`)
- [x] Face selection in the viewport with picking
- [ ] Edge and body selection
- [ ] Multiple part studios per document; assemblies with mates
- [x] STL export
- [ ] Drawings (2D projections); 3MF export; STEP once faces are exact
- [ ] Import: STEP via a B-rep reader

## Platform

- [ ] Server-side regeneration using the same crates
- [ ] Version history: branches, merges and diffs over the op log
- [ ] Real-time multi-user editing (op-based sync)
- [ ] Accounts, sharing, teams

## Engineering

- [x] CI: fmt, clippy, tests, wasm build, web build
- [ ] Property/fuzz tests for the solver, region extraction and booleans (random box/cylinder stacks compared against analytic volumes)
- [x] Regeneration cache: unchanged feature prefixes are reused, so editing late features is cheap
- [ ] Benchmarks for regeneration time on realistic parts
- [ ] `wasm-opt` in the release pipeline
