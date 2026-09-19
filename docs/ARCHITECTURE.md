# Architecture

offkilter is split into a geometry kernel written in Rust and a web client
written in TypeScript. The kernel is compiled to WebAssembly and runs in the
browser today; the same crates will run on a server later for headless
regeneration, version history and collaboration.

```
apps/web  (Vite + three.js)
   │  JSON ops / JSON summaries / typed arrays
   ▼
crates/ok-wasm  (wasm-bindgen facade)
   ▼
crates/ok-model  PartStudio ─ Feature list ─ Op ─ regenerate()
   │                              │
   │                              ├─ Sketch feature  → ok-sketch (solve, profiles)
   │                              └─ Extrude feature → ok-brep   (extrude, boolean)
   │                                                      └→ ok-mesh (tessellation for display)
   ▼
crates/ok-math   Vec2 / Vec3 / Plane / tolerances
```

## Document model (`ok-model`)

A `PartStudio` is an ordered `Vec<Feature>`. A feature has a stable
`FeatureId`, a name, a `suppressed` flag and a `FeatureKind`:

- `Sketch { plane: PlaneSpec, sketch: ok_sketch::Sketch }`
- `Extrude { sketch: FeatureId, profiles, depth, direction, op }`

Regeneration (`PartStudio::regenerate`) walks the list in order. Sketches
are solved in place so the document always stores solved geometry, as
Onshape does. Each feature produces a `FeatureStatus` with an optional
error; later features that depend on a failed feature report their own
error rather than aborting the whole regen.

Regeneration keeps a cache of the result state after each feature, keyed
by a hash chain of the solved feature definitions. A regeneration reuses
the longest unchanged prefix, so editing or dragging in the last feature
never re-runs the booleans before it. The cache is in-memory only.

All mutation goes through `Op` / `SketchOp` (`ops.rs`). Ops are plain serde
types. This is deliberate: an op log gives undo/redo for free, is the unit
of a future version history, and is what real-time collaboration will
synchronise. Do not add mutating methods outside `apply`.

A `Body` holds an `ok_brep::Solid` plus its display tessellation, a
triangle-to-face map for picking, and display edges. Extrude builds a tool solid from the selected regions and then, per
`BodyOp`, creates a new body, unions it with every body its bounding box
touches, subtracts it from them, or intersects with them. A cut that
splits a body yields separate bodies (one per shell). Boolean failures are
reported as feature errors and leave the existing bodies untouched.

### Face references

A `FaceRef` names a face by the feature that created it and that
feature's local face index (extrude: 0 = start cap, 1 = end cap, 2+ =
walls in loop order). Every face carries this as its `FaceOrigin`, and
boolean fragments keep the origin of the face they came from, so the
reference survives later cuts and unions as long as some part of the
original face remains. A sketch `PlaneRef::Face` resolves to that face's
plane at regeneration time (through `canonical_frame`, which derives a
stable sketch frame from the plane alone), and `ExtrudeEnd::UpToFace`
extrudes to that face's plane. `ExtrudeEnd::ThroughAll` extends past the
bounding boxes of all existing bodies. A reference whose face no longer
exists is a feature error.

This is a first, deliberately simple form of persistent naming. Faces
split into several fragments resolve to the first one found, and there is
no disambiguation when a feature's face is later divided by another
operation.

## Sketching (`ok-sketch`)

Entities: `Point`, `Line`, `Circle`, `Arc`. Curves reference point entities
for their defining positions, so constraints only ever act on points and
radii. Arcs carry an implicit "start and end equidistant from centre"
equation.

Constraints (`entity.rs`): coincident, fixed, horizontal, vertical,
distance, horizontal/vertical distance, length, radius, diameter, equal,
parallel, perpendicular, angle, point-on-line, point-on-circle, midpoint,
tangent. Angles are stored in degrees, lengths in model units (mm).

### Solver (`solver.rs`)

Levenberg–Marquardt over a parameter vector of free point coordinates and
circle radii. Fixed points are removed from the parameter set rather than
expressed as residuals. The Jacobian is numeric (central differences),
which keeps adding constraints trivial; analytic derivatives are a
performance item for later. Damping keeps under-constrained sketches close
to their starting geometry, which is what a user dragging a point expects.

After solving, the rank of the Jacobian gives the remaining degrees of
freedom: `dof = parameters − rank`. The result is reported as
fully constrained, under-constrained (with DOF count) or inconsistent (the
residual did not converge). A conflict is reported rather than "fixed".

### Regions (`loops.rs`)

Profiles for extrusion come from the planar graph of lines and arcs.
Points are merged via coincident constraints and positional coincidence
(union-find). Dangling edges are pruned. Faces are traced with a half-edge
walk that turns clockwise from the twin at each vertex, keeping the face
interior on the left; bounded faces are the positive-area ones. Circles
become loops directly. Loops are nested by containment so a region that
encloses another gets it as a hole, while the inner region is still a
profile in its own right.

Known gaps: edges that cross mid-span are not split (no intersection
insertion yet), and two curves leaving a vertex with the same tangent are
ordered by direction only (no curvature tie-break).

## Solids (`ok-brep`)

A `Solid` is a closed set of planar polygonal faces (loops of shared
vertex indices; outer loop counter-clockwise about the outward normal,
holes clockwise). Every face references an analytic `Surface`: either its
plane or, for facets produced from a sketch arc or circle, the cylinder
they approximate. This is a polyhedral B-rep with surface tags: facets on
one cylinder shade smoothly, hide their internal edges, and can later be
replaced by exact curved faces without changing the topology model. Each
face also carries a `FaceOrigin` (feature id + local index) as the seed of
persistent naming.

`extrude` builds a solid from a profile: two caps and one wall facet per
polygon segment, with arc segments sharing a cylinder surface.
`Solid::from_polygons` is the single assembly path: it merges vertices
within a size-relative tolerance, inserts vertices that lie on other
polygons' edges (T-junctions), strips zero-width spikes, and validates
that every edge is used an even number of times with balanced orientation.
Anything that fails validation is an error, never a displayed body.

### Booleans (`boolean.rs`)

Booleans work face by face. For a face `F` of `A` with plane `P`, take two
cross-sections of `B` in `P`: one infinitesimally above the plane (`X+`)
and one below (`X-`). "Infinitesimally" is implemented exactly by treating
vertices on the plane as belonging to one side or the other (simulation of
simplicity), so no coordinates are perturbed. In `P`'s 2D frame:

| region of `P`                          | in X+ | in X- |
|----------------------------------------|-------|-------|
| inside `B`                             | yes   | yes   |
| outside `B`                            | no    | no    |
| on a `B` face with the same normal     | no    | yes   |
| on a `B` face with the opposite normal | yes   | no    |

The part of `F` to keep is then a 2D polygon boolean (via `i_overlay`)
of `F`'s region against those sections. Union keeps `F − X+`; difference
keeps `F − X-`; intersection keeps `F ∩ X-`; the faces of `B` use the
symmetric rules (and are flipped for difference). This treats coplanar
faces, which are the common case in CAD (a boss on a plate, a cut from a
face), exactly and without special cases.

Cross-sections (`section.rs`) compute one crossing point per edge and
chain segments by edge identity, so section loops close exactly for a
valid solid. Section vertices within tolerance of the face boundary are
snapped onto it before clipping, and the clipper runs with a 64-bit grid,
so shared boundaries come out coincident rather than as hairline slivers.

Known limits: results are polyhedral (arcs are 5° facets), overlapping
coplanar faces from different features stay separate faces rather than
merging, and the merge scope is "every body whose bounding box touches
the tool".

## Meshes (`ok-mesh`)

`TriMesh` carries `f32` position, normal and index buffers ready for GPU
upload. `ok_brep::tessellate` triangulates each face with `earcutr` and
emits analytic normals on cylinder facets. `signed_volume` is used by tests
to check orientation and correctness.

## Web client (`apps/web`)

`kernel.ts` is the typed wrapper around the wasm `Studio`. `viewer.ts` is
the three.js scene (Z up, orbit controls, bodies with kernel-computed
normals and edges, sketch curves drawn on their planes, face picking by
raycast). `main.ts` renders the feature list and detail panels and sends
ops. `sketcher.ts` is sketch mode: it turns pointer input on the sketch
plane into ops (draw line / rectangle / circle, drag points via
`move_point`, select entities and apply constraints). Snapping to an
existing point adds a coincident constraint and nearly axis-aligned lines
get horizontal / vertical constraints. State lives in the kernel; the UI
re-renders from the regen summary after every op.

## Conventions

- Model units are millimetres; angles in the document are degrees.
- Planes: Top = XY (normal +Z), Front = XZ (normal −Y), Right = YZ
  (normal +X). All are right-handed frames.
- Ids (`FeatureId`, `EntityId`, `ConstraintId`) are stable for the lifetime
  of the document and never reused.
- Documents serialise to JSON (`.okpart`). The format is not yet stable.
