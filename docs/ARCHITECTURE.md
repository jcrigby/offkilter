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
   │                              └─ Extrude feature → ok-mesh   (extrude_profile)
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

All mutation goes through `Op` / `SketchOp` (`ops.rs`). Ops are plain serde
types. This is deliberate: an op log gives undo/redo for free, is the unit
of a future version history, and is what real-time collaboration will
synchronise. Do not add mutating methods outside `apply`.

Bodies are currently `TriMesh`es. "Add" merges by concatenating meshes;
there is no boolean yet. The `Body` type is where a B-rep solid will go.

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

## Meshes (`ok-mesh`)

`TriMesh` is flat shaded (vertices duplicated per face) with `f32`
buffers, ready for GPU upload. `extrude_profile` triangulates the caps with
`earcutr` and builds side walls per loop edge. `signed_volume` is used by
tests to check orientation and correctness.

## Web client (`apps/web`)

`kernel.ts` is the typed wrapper around the wasm `Studio`. `viewer.ts` is
the three.js scene (Z up, orbit controls, flat shaded bodies with edge
overlay, sketch curves drawn on their planes). `main.ts` renders the
feature list and detail panels and sends ops. State lives in the kernel;
the UI re-renders from the regen summary after every op.

## Conventions

- Model units are millimetres; angles in the document are degrees.
- Planes: Top = XY (normal +Z), Front = XZ (normal −Y), Right = YZ
  (normal +X). All are right-handed frames.
- Ids (`FeatureId`, `EntityId`, `ConstraintId`) are stable for the lifetime
  of the document and never reused.
- Documents serialise to JSON (`.okpart`). The format is not yet stable.
