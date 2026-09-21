# Driving offkilter with ops

Every edit to an offkilter document is one JSON op. The browser sends
them over its WebSocket; scripts and agents send the same ops to
`POST /api/docs/<id>/ops` (or through the `ok-mcp` server, which wraps
them in tools). This is the reference for writing them.

Units are millimetres and degrees. Ids are integers the kernel hands out
in op results; a `report` lists every feature, entity, constraint, face
and instance with the id or reference that names it.

## Workflow

1. Create a document (`POST /api/docs`, or the MCP `create_document`
   tool). It has one part studio tab with id 1.
2. Add a sketch on a plane, draw in it, then extrude it.
3. Read the report: it lists the bodies with every face's `reference`
   (`{feature, local, part}`), so later features (a sketch on a face,
   a hole, a fillet on an edge between two faces, a shell) can name
   what they act on. Faces are named by the feature that made them, so
   the references survive edits upstream.
4. Keep going: holes, fillets, patterns, more sketches on faces. Check
   `errors` in the report after each step; a failing op names its index
   and leaves the earlier ops applied.
5. Look at it: `GET /api/docs/<id>/screenshot?view=iso` (or the MCP
   `screenshot` tool) renders the tab to a PNG; `view=top|front|right|iso`
   or `x,y,z`, `section=z:10` cuts it open.
6. Export STL or STEP, or open the document in the browser (the server
   relays every op live to whoever has it open).

## Op envelopes

An op on a part studio tab is wrapped as

```json
{ "type": "studio", "tab": 1, "op": { ...studio op... } }
```

an op on an assembly tab as `{ "type": "assembly", "tab": 2, "op": {...} }`,
and a sketch op as a studio op:

```json
{ "type": "studio", "tab": 1,
  "op": { "type": "sketch", "id": 3, "op": { "type": "add_circle", "center": { "x": 0, "y": 0 }, "radius": 5 } } }
```

The MCP `apply` tool wraps bare studio, sketch and assembly ops for you.
Document-level ops need no envelope: `add_part_studio {name}`,
`add_assembly {name}`, `rename_tab {tab, name}`, `delete_tab {tab}`,
`rename_document {name}`.

Op results carry the ids of what was made: a studio op's result has
`studio.feature` (the new feature's id), `studio.entities` (new sketch
entity ids, in the order the op names them) and `studio.constraint`; an
assembly op's result has `instance` or `mate`.

## Common shapes

- `Vec2`: `{ "x": 0, "y": 0 }`; `Vec3` adds `z`.
- `PlaneRef`: `{ "type": "standard", "base": "top" | "front" | "right", "offset": 0 }`,
  `{ "type": "face", "face": FaceRef, "offset": 0 }` (a planar face of a body), or
  `{ "type": "rotated", "base": "top", "axis": "x" | "y" | "z", "angle": 30, "offset": 0 }`.
  Top is the XY plane (normal +Z), Front is XZ (normal −Y), Right is YZ (normal +X).
- `FaceRef`: `{ "feature": 2, "local": 1, "part": 0, "near": [...] }`, copied from a
  report. `local` 0 and 1 of an extrude are its start and end caps; the sides follow.
  `part` numbers the pieces when a later feature splits a face, and `near` (hashes of
  the neighbouring pieces' origins, filled in by the report) lets the reference find
  its piece again if an edit reorders them; both may be left out when writing a
  reference by hand, meaning the first piece.
- `EdgeRef`: `{ "a": FaceRef, "b": FaceRef }`, the edge where two faces meet. One facet
  of a cylinder names the whole cylinder, so `{a: top, b: cylinder}` is the whole rim.
- `ExtrudeDirection`: `"normal"`, `"reverse"`, `"symmetric"`.
- `ExtrudeEnd`: `{ "type": "blind" }`, `{ "type": "through_all" }`, `{ "type": "up_to_face", "face": FaceRef }`.
- `ProfileSelection`: `{ "type": "all" }`, `{ "type": "largest" }`, `{ "type": "indices", "indices": [0] }`
  (regions of the sketch, largest first, as the report's `regions` counts them).
- `BodyOp`: `"new"` (a new body), `"add"` (union into existing bodies), `"remove"` (cut), `"intersect"`.

## Studio ops

Feature ops (each returns the new feature's id; `name` may be null):

| op | fields |
|----|--------|
| `add_sketch` | `plane: PlaneRef, name` |
| `add_extrude` | `sketch, depth, direction?, end?, profiles?, op?, name` |
| `add_revolve` | `sketch, axis: {type: "x_axis"|"y_axis"} or {type: "line", line}, angle? (360), profiles?, op?, name` |
| `add_hole` | `sketch` (its points are the hole centres), `diameter, depth?, through_all?, direction?, counterbore?: {diameter, depth}, countersink?: {diameter, angle?} (90° by default), name` |
| `add_blend` | `kind: "fillet"|"chamfer", edges: EdgeRef[], size, name` |
| `add_shell` | `thickness, faces?: FaceRef[]` (faces to leave open), `name` |
| `add_sweep` | `sketch` (profile), `path` (a sketch holding the path), `profiles?, op?, name` |
| `add_loft` | `sketch, sketch_b, op?, name` |
| `add_boolean` | `op: "union"|"subtract"|"intersect", targets: [feature ids], tools: [feature ids], keep_tools?, name` |
| `add_mirror` | `plane: PlaneRef, op?: "add"|"new", features?: [ids], bodies?: [indices], name` |
| `add_pattern` | `kind: {type: "linear", axis, spacing} or {type: "circular", axis, angle}, count, op?, features?, bodies?, name` |
| `add_move_face` | `faces: FaceRef[], distance` (negative pushes in), `name` |
| `add_draft` | `faces: FaceRef[], neutral: PlaneRef, angle, name` |
| `add_split` | `plane: PlaneRef, bodies?, name` |
| `add_mesh` | `vertices: Vec3[], triangles: [[i,j,k]...], name` |
| `add_puzzle` | `plane: PlaneRef, cols, rows, pitch, thickness, gap? (0), bit? (6.35, the pin router bit), lock? (20°, how far tab necks lean in), grain?: "x"|"y", web? (0: height of the alignment web filling the gaps), seed?, jitter? (mm the interior corners wander), name`. A jigsaw puzzle: one body per piece, `Piece col,row light|dark` by checkerboard parity, plus `Alignment web` when `web` and `gap` are set. Every outline is lines and tangent arcs; the feature refuses (with every rule named) what the bit cannot cut: socket openings and concave radii under the bit, necks too thin (thinner still across the grain), tabs too near a corner or too tall, cells not convex. |
| `add_variable` | `name, expression` (then `#name` works in any expression) |

Every `add_*` has a `set_*` twin taking `id` plus the fields to change
(`set_extrude {id, depth}`, `set_blend {id, size}`, `set_hole {id, diameter}`, …).
A puzzle also takes `set_puzzle_tab {id, edge, out?, size?, width?, shift?}` for one
tab (edges are numbered horizontal first, `(row - 1) * cols + col` for the edge
above piece `col,row` counting from 0, then vertical, `h + (col - 1) * rows + row`
for the edge right of it; `out` bulges towards the higher piece) and
`set_puzzle_corner {id, node, offset: Vec2}` for one interior corner (row by
row, `(row - 1) * (cols - 1) + (col - 1)`). Changing `cols`, `rows`, `seed` or
`jitter` through `set_puzzle` reseeds every tab and corner unless `tabs` and
`corners` are passed too.
Other studio ops: `rename_feature {id, name}`, `set_suppressed {id, suppressed}`,
`delete_feature {id}`, `move_feature {id, index}`, `set_binding {id, field, expression}`
(bind an expression such as `"#width / 2"` to a numeric field: `"depth"`, `"size"`,
`"diameter"`, `"constraint.<id>"` for a sketch dimension), `rename_part {source, name}`,
`set_part_material {source, material: {name, density}}` (g/cm³), `set_settings {facet_angle}`.

## Sketch ops

Wrapped as `{ "type": "sketch", "id": <sketch feature id>, "op": ... }`.
Sketch coordinates are in the sketch plane (x right, y up). Drawing ops
return the new entity ids in `studio.entities`:

| op | fields | entities returned |
|----|--------|-------------------|
| `add_point` | `pos` | point |
| `add_line` | `a, b` | line, start point, end point |
| `add_rectangle` | `a, b` (opposite corners) | the 4 lines (bottom, right, top, left); their endpoints are listed in the report |
| `add_circle` | `center, radius` | circle, centre point |
| `add_arc` | `center, start, end` (counter-clockwise) | arc, centre, start, end |
| `add_polygon` | `center, vertex, sides` | lines and points |
| `add_slot` | `a, b, width` | lines, arcs and points |
| `add_spline` | `points: Vec2[]` | spline, its points |

Editing: `remove_entity {id}`, `move_point {id, pos}`, `set_construction {id, construction}`,
`trim {entity, at}`, `offset {entities, distance}`, `mirror {entities, axis: <line id>}`,
`fillet {a, b, radius}` and `chamfer {a, b, distance}` (two lines meeting at a point), `pattern_linear {entities, count, step}`,
`pattern_circular {entities, count, center, angle}`, `project {source: {type: "face", face} | {type: "edge", edge}}`
(body geometry into the sketch).

Constraints: `add_constraint {constraint}` where the constraint is one of
`coincident {a, b}` (points), `fixed {point}`, `horizontal {line}`, `vertical {line}`,
`length {line, value}`, `distance {a, b, value}`, `horizontal_distance {a, b, value}`,
`vertical_distance {a, b, value}`, `radius {entity, value}`, `diameter {entity, value}`,
`equal {a, b}`, `parallel {a, b}`, `perpendicular {a, b}`, `angle {a, b, value}`,
`point_on_line {point, line}`, `point_on_circle {point, entity}`, `midpoint {point, line}`,
`tangent {line, entity}`, `symmetric {a, b, line}`. Then `set_constraint_value {id, value}`
and `remove_constraint {id}`. A rectangle's corners are already tied together; a
shape drawn from lines needs `coincident` constraints between the shared endpoints
to close (the report's `solve.dof` says how many degrees of freedom remain and
`regions` how many closed areas the sketch has).

## Assembly ops

Wrapped as `{ "type": "assembly", "tab": <assembly tab id>, "op": ... }`:
`add_instance {studio: <tab id>, body: <index>, name, fixed?, placement?: {position, rotation}}`,
`set_instance {id, name?, fixed?, placement?}`, `remove_instance {id}`,
`add_mate {kind: "fastened"|"revolute"|"slider"|"cylindrical"|"planar"|"ball", a: Connector, b: Connector, offset?, angle?, flip?, name}`,
`set_mate {...}`, `remove_mate {id}`. A `Connector` is `{ "instance": <id>, "face": FaceRef }`, optionally with
`"anchor": {type: "edge", other: FaceRef}` or `{type: "vertex", others: [FaceRef, FaceRef]}`.
Instances of a studio's bodies are placed by their mates from fixed instances.

A connector is a frame on its face: on a planar face the origin is the face's
centroid and z its normal; on a cylindrical face the origin is the middle of the
face projected onto the axis and z the axis (the report's `cylinders` give the
axis and a reference, its `faces` the centroid). x and y are canonical for z:
with `hint` = X unless |z.x| ≥ 0.9, then Y, `y = z × hint` normalised and
`x = y × z`. A mate puts the moving side's frame on the placed side's: z axes
opposed (aligned with `flip`), the moving x turned by `angle` degrees about z
from the placed x, and the origin `offset` along the placed side's z. To
reproduce a known pose, place both instances there first (their `placement`
stays as the initial guess), take both frames in world coordinates, and read
`flip` from the sign of the z dot product, `offset` from the origin difference
along z and `angle` from the x axes. The assembly report gives every resolved
instance's pose as `placed: {position, rotation}` (the same shape as a placement).

## Example: a plate with a boss, a hole and rounded corners

```json
[
  { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": "Base" },
  { "type": "sketch", "id": 1, "op": { "type": "add_rectangle", "a": { "x": 0, "y": 0 }, "b": { "x": 60, "y": 40 } } },
  { "type": "add_extrude", "sketch": 1, "depth": 8, "name": "Plate" },
  { "type": "add_sketch", "plane": { "type": "face", "face": { "feature": 2, "local": 1, "part": 0 }, "offset": 0 }, "name": "Boss sketch" },
  { "type": "sketch", "id": 3, "op": { "type": "add_circle", "center": { "x": 30, "y": 20 }, "radius": 10 } },
  { "type": "add_extrude", "sketch": 3, "depth": 6, "op": "add", "name": "Boss" },
  { "type": "add_sketch", "plane": { "type": "face", "face": { "feature": 2, "local": 1, "part": 0 }, "offset": 0 }, "name": "Hole centres" },
  { "type": "sketch", "id": 5, "op": { "type": "add_point", "pos": { "x": 30, "y": 20 } } },
  { "type": "add_hole", "sketch": 5, "diameter": 6, "through_all": true, "name": "Hole" }
]
```

After these, read the report: the plate's vertical edges are the pairs of
side faces of feature 2 (`local` 2 to 5), so a fillet of the corners is
`add_blend {kind: "fillet", size: 4, edges: [{a: {feature: 2, local: 2}, b: {feature: 2, local: 3}}, ...]}`.
