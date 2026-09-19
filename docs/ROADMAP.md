# Roadmap

Rough order of work toward a usable parametric modeller. Items near the
top are concrete and self-contained; items lower down are directions.

## Sketching

- [x] Points, lines, circles, arcs
- [x] Geometric and dimensional constraints with DOF reporting
- [x] Closed-region extraction with holes
- [ ] Split edges at crossings so intersecting lines form regions
- [ ] Interactive sketch mode in the client: draw on the plane, drag points, click to pick entities for constraints, inferred constraints while drawing
- [ ] Dimension display in the viewport
- [ ] Analytic Jacobians for the common constraints (performance)
- [ ] Splines (B-spline entity + point-on-curve)
- [ ] Sketch on planar faces of bodies; project/use edges from bodies
- [ ] Construction geometry, mirror, patterns

## Solids

- [x] Extrude regions (normal / reverse / symmetric)
- [ ] Boundary representation (B-rep): vertices, edges, faces with planar and cylindrical surfaces first, then general
- [ ] Booleans (union / subtract / intersect) — the enabler for cut extrudes, holes, and "Add" that actually merges
- [ ] Extrude up-to-face / through-all
- [ ] Revolve, sweep, loft
- [ ] Fillet and chamfer
- [ ] Shell, draft
- [ ] Feature patterns and mirror
- [ ] Persistent naming of topology across regeneration (so downstream features survive edits)

## Document and client

- [x] Ordered feature list with suppression, reorder, delete
- [x] JSON document format and browser persistence
- [ ] Undo/redo from the op log
- [ ] Variables and expressions in dimensions (`width / 2`)
- [ ] Face/edge/body selection in the viewport with picking
- [ ] Multiple part studios per document; assemblies with mates
- [ ] Drawings (2D projections) and export: STL/3MF now, STEP once there is a B-rep
- [ ] Import: STEP via a B-rep reader

## Platform

- [ ] Server-side regeneration using the same crates
- [ ] Version history: branches, merges and diffs over the op log
- [ ] Real-time multi-user editing (op-based sync)
- [ ] Accounts, sharing, teams

## Engineering

- [x] CI: fmt, clippy, tests, wasm build, web build
- [ ] Property/fuzz tests for the solver and region extraction
- [ ] Benchmarks for regeneration time on realistic parts
- [ ] `wasm-opt` in the release pipeline
