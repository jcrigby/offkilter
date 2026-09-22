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
crates/ok-model  Document ─ tabs: PartStudio (Feature list ─ Op) | Assembly (instances, mates)
crates/ok-sheet  Shop drawing sheets of a tab (views, dimensions, callouts, balloons, parts list) as PDF
   │                              │
   │                              ├─ Sketch feature  → ok-sketch (solve, profiles)
   │                              └─ Extrude feature → ok-brep   (extrude, boolean)
   │                                                      └→ ok-mesh (tessellation for display)
   ▼
crates/ok-math   Vec2 / Vec3 / Plane / tolerances
```

## Document model (`ok-model`)

A `Document` (`document.rs`) is a list of tabs, each a `PartStudio` or an
`Assembly`, plus a document-level id counter for tabs, instances and
mates. Every edit is a `DocOp`: a studio `Op` addressed to a tab, an
`AssemblyOp`, or a tab change (add, rename, delete, insert-back). Older
`.okpart` files holding a bare part studio load as a one-tab document.
The wasm facade regenerates one tab at a time: a studio tab yields its
features, sketches and bodies; an assembly tab yields placed instances
and mates, with the bodies of its instances as the displayed bodies.

### Assemblies (`assembly.rs`)

An `Instance` names a body of a part studio tab in the same document
(by tab and body index), or an assembly tab, whose placed bodies then
move as one rigid group (an assembly that would contain itself gets no
bodies and an error on that instance). It carries a `Placement`
(position and Euler rotation) used when nothing mates it. A `Mate` joins two instances
through `Connector`s, each a face reference on an instance's body (for a
sub-assembly instance, the first of its bodies that has the face) plus an
`Anchor` saying where on that face the connector sits. On the face
itself the frame has its origin at the face's area-weighted centroid
(over every facet of the surface for a curved face, projected onto the
axis for a cylindrical one, with z along the axis; over the referenced
piece for a planar face), z along the normal, x and y canonical for that
normal (`Plane::from_origin_normal`). The report gives the same
centroids, and each instance's resolved pose as `placed`. On an edge (the face plus the other face
across the edge) the origin is the middle of the edge and z runs along
it, with x along the face normal; a circular rim takes the centre and
axis of its cylinder instead. On a vertex (the face plus two more faces
meeting there) the origin is the vertex, z the face normal and x the edge
shared with the first other face. Mates are resolved as directed chains:
fixed instances and unmated instances sit at their own placement, then
every mate whose one side is placed moves the other side so that the
connector frames meet, z axes opposed (or aligned with `flip`), rotated
by `angle` about z and separated by `offset` along it. Revolute, slider,
cylindrical, planar and ball mates use the same placement as the initial
guess; the kind decides what the numeric solve pins. The chain result is then refined
numerically: a Levenberg–Marquardt solve over the pose (rotation vector
and translation) of every movable mated instance, with residuals per
mate that pin what its kind pins (fastened: everything; revolute: all but
spin about z; slider: all but travel along z; cylindrical: both free;
planar: parallel faces at the offset; ball: coincident origins), so
closed loops and redundant mates are solved through the free degrees of
freedom. A mate that still has a residual afterwards (inconsistent with
the others) is reported on that mate; a chain with no fixed instance
falls back to placements with an error on each instance.

A mate animation never touches the document: `Document::preview_assembly`
resolves the tab with one mate's angle and offset overridden, and the
wasm `mate_preview` call turns that into a rigid delta transform per
shown body (new placement composed with the inverse of the current one),
which the client applies to the meshes each frame.

### Part studios

A `PartStudio` is an ordered `Vec<Feature>`. A feature has a stable
`FeatureId`, a name, a `suppressed` flag and a `FeatureKind`:

- `Sketch { plane: PlaneRef, sketch: ok_sketch::Sketch, projections }`
- `Extrude { sketch: FeatureId, profiles, depth, direction, end, op }`
- `Revolve { sketch: FeatureId, profiles, axis, angle, op }`
- `Sweep { sketch, profiles, path: FeatureId, op }`: the path sketch's
  non-construction lines and arcs are chained into one open polyline.
- `Loft { sketch, sketch_b, op }`: joins the largest region of each sketch.

- `Hole { sketch, diameter, depth, through_all, direction, counterbore, countersink }`:
  drills at every standalone point of the sketch.
- `Puzzle { plane, cols, rows, pitch, thickness, gap, bit, lock, grain, web, seed, jitter, fixture, show, tabs, corners }`:
  a jigsaw from `ok_sketch::jigsaw`, one body per piece, an alignment
  web in the gaps and a printable fixture tray, each piece's faces in
  their own `local` range; or, by `show`, one colour's fabrication
  layout (rows spread a bit apart).

Solid features share one path: select regions (or points for holes),
build a tool solid (the union of one solid per region), then apply the
body operation.

Regeneration (`PartStudio::regenerate`) walks the list in order. Sketches
are solved in place so the document always stores solved geometry, as
Onshape does. Each feature produces a `FeatureStatus` with an optional
error; later features that depend on a failed feature report their own
error rather than aborting the whole regen.

`PartStudio.settings` holds document-wide regeneration settings; today
that is `facet_angle`, the maximum angle per facet for arcs, circles,
revolves and blends (default 5°). It is persisted, edited through
`Op::SetSettings` so it syncs between clients, and seeds the cache hash
chain so changing it regenerates everything.

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

### Variables and expressions

A `Variable` feature defines `#name = expression`, evaluated in feature
order into a table the later features see. Any feature can carry
`bindings`: a map from a numeric field name (`depth`, `size`, `angle`,
`spacing`, `count`, `plane.offset`, `constraint.<id>`) to an expression.
At regeneration the bindings are evaluated and written into the fields
before the feature runs (and before it is hashed for the cache), so the
document always stores the last evaluated numbers as well as the
expressions. The expression language (`expr.rs`) has arithmetic, `^`,
parentheses, `#name` or bare-name references, `pi`, and trigonometric /
rounding / min-max functions with angles in degrees. A failing expression
is a feature error.

### Face references

A `FaceRef` names a face by the feature that created it and that
feature's local face index (extrude: 0 = start cap, 1 = end cap, 2+ =
walls in loop order). Every face carries this as its `FaceOrigin`, and
boolean fragments keep the origin of the face they came from, so the
reference survives later cuts and unions as long as some part of the
original face remains. A sketch `PlaneRef::Face` resolves to that face's
plane at regeneration time (through `canonical_frame`, which derives a
stable sketch frame from the plane alone); `PlaneRef::Rotated` is a
standard plane turned about a world axis through the origin by an angle
(bindable on sketches as `plane.angle`) and then offset, the angled datum
for tilted features; and `ExtrudeEnd::UpToFace`
extrudes to that face's plane. `ExtrudeEnd::ThroughAll` extends past the
bounding boxes of all existing bodies. A reference whose face no longer
exists is a feature error.

This is a deliberately simple form of persistent naming. When a later
operation divides an originating face into several pieces (a slot across
a top face leaves two with one origin), `Solid::face_parts` numbers the
pieces: faces of one origin that share an edge are one piece (so the
facets of a cylinder stay one), the pieces are ordered by their
area-weighted centroids and numbered from 0. A `FaceRef` carries the
piece number as `part` (the client copies it from the summary when a
face is picked), and every lookup, including edge references, honours
it; a reference without one, as in older documents, means the first
piece. The numbering is deterministic for a given shape but can change
when an edit moves pieces past each other (a pocket added to one piece
shifts its centroid), which is the usual limit of naming by position.
So a reference also carries `near`: up to eight hashes (the smallest,
sorted) of the origins of the pieces across the piece's edges, filled
in by `piece_near` wherever references are handed out (the wasm
summary the client picks from, the report a script reads). Every lookup
goes through `faces_of_ref`: with `near` present and several pieces of
the origin, the piece whose neighbours contain the most of the recorded
hashes wins, ties go to the piece number, and a single piece left after
a split heals is taken as the one meant; without `near` (older documents,
hand-written ops) the piece number decides as before. Hashes keep the
reference `Copy` and small; a neighbour list is a sample of the
topology, not a proof, and a reference whose neighbours all changed
falls back to position like before.

### Edge references and blends

An `EdgeRef` is the unordered pair of `FaceRef`s that meet at the edge.
It resolves to every edge segment between the *surfaces* of those two
faces, so a straight edge split by a T-junction is still one edge and one
picked facet of a cylinder's rim stands for the whole rim (the client
highlights by surface the same way). `Blend` features (fillet or chamfer)
chain the segments head to tail within each surface pair and build one
cutter per chain: a single segment gets a prism whose cross-section is
the corner between the two faces (a triangle for a chamfer, or the
corner minus the tangent arc for a fillet, tagged as a cylinder about
the edge so it shades smoothly and selects as one face). A chain of
several segments (a rim) gets `chain_cutter`: at every vertex of the
chain the section is rebuilt in the plane perpendicular to the local
tangent (the bisector of the two segments there) from the average
normals of each face's facets on both sides, so a rim whose dihedral
angle changes along it (a cylinder cut obliquely) gets a section that
fits everywhere; the sections, all cut into the same number of facets,
are joined by quads into one polyhedron, its blend facets sharing one
surface: a torus when the chain is a circle about an axis and both
faces turn about it (`rim_axis`: the rim of a boss or a turned part),
its tube the fillet's arc swept about the axis, else a ruled surface.
Those sections do not run back along the faces to the
edge, as a prism's does: from the tangent points they leave each face
perpendicularly and close a tenth of the size outside the wedge, so the
cutter meets the body only along the tangent lines, squarely, where a
wall lying almost but not quite in the tilting facets of a curved face
would leave the boolean slivers to resolve. Convex edges have the
cutter subtracted, concave edges have it added. This is "blend by
boolean": exact between planar facets, and along a curved rim exact to
the facets' own approximation. Where three convex fillets of one feature meet at a vertex
between three planar faces, the corner is the rolling ball exactly
(`corner.rs`): the ball centre `c` lies `r` inside all three faces, each
fillet's axis passes through it, and the ball is tangent to the three
cylinders along the circles in the planes through `c` perpendicular to
the edges. Those edges and corners become one closed cutter polyhedron
per connected group, built from polygons rather than by union: the edge
prisms cut back to the cell planes, the cell's three face quads, and a
spherical patch (rings shrinking from the fillet arcs, which it shares
vertex for vertex with the prisms, towards the middle direction) tagged
as a sphere so it shades smoothly and goes out exactly. Chamfers, corners with any
other number of blended edges and non-planar faces keep the union of
cutters: three chamfers meeting at a corner leave their three planes
meeting at a point, which is what other CAD systems draw, and a corner
with four or more fillets, where no one ball is tangent to every face,
gets a plausible faceted corner.

While a blend feature is collecting edges, the client asks for a
"rollback" regeneration (`regenerate_to`), which returns the cached state
after the previous feature so the original edges are visible to pick.

### Transforms, mirror and patterns

`Transform` is an orthogonal 3x3 matrix plus translation. `Solid::transformed`
maps vertices, planes and surfaces; a reflection reverses every loop and
swaps the frame axes so faces stay outward. `Mirror` and `Pattern`
features apply transforms to every body and either union the copies into
their originals or keep them as new bodies.

## Sketching (`ok-sketch`)

Entities: `Point`, `Line`, `Circle`, `Arc`, `Spline`. Curves reference
point entities for their defining positions, so constraints only ever act
on points and radii. Arcs carry an implicit "start and end equidistant
from centre" equation. A tangent constraint whose line starts or ends on
its circle (a slot's line, a fillet) uses the perpendicular form, radius
⟂ line at that endpoint, for the whole solve: the distance form has no
gradient there, and the choice is made once from the initial geometry so
it cannot flip while a coincident endpoint converges onto the arc. A
spline is a Catmull–Rom curve through its points
(`spline.rs`): each span is a cubic Hermite segment whose end tangents are
half the chord between the neighbouring points, so it interpolates every
point and moving one only reshapes the spans beside it. Region extraction
samples it into straight graph edges (a number of pieces per span set by
the facet angle) whose polygon segments are tagged `SegmentCurve::Spline`
with the entity id, so extrude, revolve, sweep and loft give all of a
spline's facets one smooth surface. Trim and offset refuse splines; mirror
and patterns copy them.

Constraints (`entity.rs`): coincident, fixed, horizontal, vertical,
distance, horizontal/vertical distance, length, radius, diameter, equal,
parallel, perpendicular, angle, point-on-line, point-on-circle,
point-on-spline (distance to the nearest piece of the curve sampled at a
fixed density), midpoint, tangent (to a circle or arc, or to a spline at
the end nearest the line, where the line follows the end chord),
symmetric, rotated. Angles are stored in degrees, lengths in model units
(mm).

### Editing (`edit.rs`)

Trim, offset and mirror produce ordinary entities and constraints, so
their results stay editable. `trim` removes the piece of a curve nearest
the click between its intersections with any other entity (construction
included) or points lying on it; the original endpoints are kept, new
cut points get point-on-line / point-on-circle (or coincident)
constraints to what cut them, and a circle becomes an arc. `offset`
orders the selected lines and arcs into one chain, offsets each to the
left of the chain's direction (negative distances go right), mitres
line corners, trims overlapping offsets at their intersection, rounds
gaps with an arc about the original corner, keeps arcs concentric by
reusing their centre point, and adds parallel constraints for lines.
`mirror` reflects entities across a line and adds a `Symmetric`
constraint per point pair (and `Equal` for circle radii), so the copy
follows the original under the solver. `fillet` rounds the corner where
two lines meet: each line keeps its own endpoint, moved to the tangent
point `r / tan(θ/2)` from the corner, and a tangent arc about the centre
on the bisector joins them, held by coincident, tangent and radius
constraints so the radius dimension drives it afterwards.

### Solver (`solver.rs`)

Levenberg–Marquardt over a parameter vector of free point coordinates and
circle radii. Fixed points are removed from the parameter set rather than
expressed as residuals. The Jacobian is sparse: a constraint only
depends on the parameters of the entities it names, so each row holds a
handful of entries. Coincident, horizontal/vertical, the distances,
length, radius, diameter, equal, point-on-line, point-on-circle,
midpoint and the implicit arc equation have analytic rows; the
trigonometric ones (parallel, perpendicular, angle, symmetric, rotated,
tangent) are differenced over just their own parameters, so adding a
constraint stays trivial (a test checks the sparse Jacobian against dense
central differences over every parameter). The normal matrix `JᵀJ` is
accumulated from those rows. Damping keeps under-constrained sketches
close to their starting geometry, which is what a user dragging a point
expects.

After solving, the rank of the Jacobian gives the remaining degrees of
freedom: `dof = parameters − rank`. It is read off the normal matrix by
elimination with diagonal pivots (no row swaps on a positive
semidefinite matrix, so it stays as sparse as the sketch), treating a
pivot that has shrunk to noise relative to its original diagonal entry
as a dependent parameter. The result is reported as fully constrained,
under-constrained (with DOF count) or inconsistent (the residual did not
converge). A conflict is reported rather than "fixed". A grid of 128
dimensioned rectangles (about 1000 parameters) solves in under 0.1 s
(`solve_time_for_a_grid_of_dimensioned_rectangles`, ignored by default).

### Projected geometry (`ok-model/src/project.rs`)

A sketch can carry projections of body geometry ("Use"): an edge, named
by the two faces meeting there, or the outline of a face. Faces are
matched by origin and widened to every face on the same surface, so one
segment of a faceted rim stands for the whole rim. On each regeneration the
matching display segments of the bodies that exist before the sketch are
projected onto the sketch plane, chained through shared endpoints, and
turned into a circle or arc when the source lies on a cylinder whose axis
is normal to the plane (or when a long chain fits one), otherwise into
lines sharing their endpoints. The entities are marked projected: the
solver holds them fixed and they cannot be moved or deleted on their own.

Projected entities need deterministic ids so collaborating replicas agree:
each projection reserves a block of entity ids when it is added (an op),
and regeneration fills the block in order. When the model changes but the
projected shape keeps its kinds, positions are updated in place so
constraints attached to projected points survive.

### Regions (`loops.rs`)

Profiles for extrusion come from the planar graph of lines and arcs.
Points are merged via coincident constraints and positional coincidence
(union-find). Dangling edges are pruned. Faces are traced with a half-edge
walk that turns clockwise from the twin at each vertex, keeping the face
interior on the left; bounded faces are the positive-area ones. Circles
become loops directly. Loops are nested by containment so a region that
encloses another gets it as a hole, while the inner region is still a
profile in its own right.

Before tracing, edges are split where they cross (line-line, line-arc,
arc-arc) and where another edge's endpoint lies on them, so the graph only
meets at vertices and crossing lines form regions. Construction entities
are skipped when building the graph but still take part in constraints.

Known gap: two curves leaving a vertex with the same tangent are ordered
by direction only (no curvature tie-break).

## Solids (`ok-brep`)

A `Solid` is a closed set of planar polygonal faces (loops of shared
vertex indices; outer loop counter-clockwise about the outward normal,
holes clockwise). Every face references an analytic `Surface`: its plane
or, for facets produced from a sketch arc or circle, the cylinder they
approximate; a revolve tags the cone, torus or sphere its oblique lines
and arcs sweep, and the rolling-ball corner patch its sphere. This is a polyhedral B-rep with surface tags: facets on
one cylinder shade smoothly, hide their internal edges, and can later be
replaced by exact curved faces without changing the topology model. Each
face also carries a `FaceOrigin` (feature id + local index) as the seed of
persistent naming.

`extrude` builds a solid from a profile: two caps and one wall facet per
polygon segment, with arc segments sharing a cylinder surface. `revolve`
sweeps a profile about an axis in its plane in 5° steps; each profile
segment becomes a `Surface::Revolved` group (arc segments share one), and
the tessellator shades those with area-weighted averaged normals since
there is no single analytic form. Profile edges lying on the axis sweep
nothing, so a half-profile touching the axis yields a plain solid.
`sweep` carries a profile along a 3D polyline using rotation-minimising
frames, mitring the profile onto the bisector plane at each corner; `loft`
resamples two loops by arc length, aligns their start points, and joins
them with triangles. Both tag their walls `Surface::Ruled`, which shades
with averaged normals like `Revolved`.
`Solid::from_polygons` is the single assembly path: it merges vertices
within a size-relative tolerance, inserts vertices that lie on other
polygons' edges (T-junctions), strips zero-width spikes, and validates
that every edge is used an even number of times with balanced orientation.
Two repairs run between the T-junction pass and validation, and the three
alternate until nothing changes: edges shorter than ten tolerances are
collapsed, and vertices on still-open edges that lie within ten
tolerances of each other are merged. Both target the same defect, a
grazing intersection that leaves the faces around one corner disagreeing
on it by slightly more than the merge tolerance (a sliver triangle or two
near-coincident corner vertices). Anything that still fails validation is
an error, never a displayed body.

### Exact curves (`exact.rs`)

The surface tags make curved faces exact without a second representation
(`docs/EXACT.md` is the plan and record). Where both faces at an edge are
analytic, the edge's exact curve follows from the pair (`edge_curve`:
plane ∩ plane a line, plane ∩ cylinder a circle, ellipse or line pair,
cylinder ∩ cylinder a quartic, plane ∩ sphere a circle, a plane across a
cone's axis a circle; `run_curve` adds the circles a plane across a
torus or two coaxial surfaces of revolution make, told apart by a point
of the run), a vertex's exact position is the point on all of its
surfaces (`vertex_position`, alternating projection; a vertex of a run
where two facets of one cylinder or cone meet along a ruling is held to
that ruling, solved directly against the other surfaces), an edge run
(`edge_runs`, the chain of facet edges between one pair of surfaces) is a
piece of the pair's curve and can be resampled anywhere on it
(`run_points`, at a cylinder's rulings), and a cylinder's region is its
facets grouped by seams with loops of runs (`surface_regions`). The STEP
writer uses all of these. `refit` moves every vertex of a solid to its
exact position, welds vertices that then coincide, puts a run vertex that
would change places with a neighbour onto it, and splits the facets that
bend into planar triangles (Delaunay-flipped in `split_nonplanar_faces`
so slivers along a nearly straight boundary become long triangles fanned
to a far vertex). Every boolean ends with it, on the region the operation
touched, and keeps its own result if the refit fails to close.

### Shell (`shell.rs`)

`shell` hollows a solid by subtracting an offset polyhedron: every kept
face's plane moves inward by the wall thickness (open faces move outward
so the cavity breaks through them), and each face keeps its outline edge
by edge. An edge moves to the line where the face's offset plane meets
the offset plane of the face across it, and corners are where consecutive
edge lines meet; faces that only touch a corner at a vertex (the ring of
faces around it holds more than the two across the loop's edges) are
inserted as extra edges so the corner is consistent all round. Edges whose
offset would run backwards were swallowed by their neighbours (a short
facet next to a sharp corner) and are dropped; consecutive parallel edges
are resolved by keeping the more restrictive one, or dropping the one
whose line lies far from the outline. Faces then meet exactly along
offset edges at three-face corners and disagree by a little where more
faces meet, and the assembly (`from_polygons_closing_gaps`) fills those
small rings of open edges with fan triangles. Slab bottoms carry the
face's surface moved inward (planes shifted, cylinders shrunk or grown),
so a shelled cylinder shades smoothly inside. A wall thicker than the
feature it hollows (a rim narrower than twice the thickness) is an error,
as is a cavity the face offsets cannot make consistent, which happens
when the thickness swallows features in ways that change the topology
(a slot through a faceted boss at a sharp angle); that case needs a true
offset with topology changes, not attempted here.

The same machinery (`reshape`, private to `shell.rs`) takes any set of
new face planes, so `move_faces` translates planar faces along their
normals and `draft_faces` tilts them about the line where each meets a
neutral plane; the faces around them re-solve their corners as above.

### DXF import (`apps/web/src/dxf.ts`)

`parseDxf` reads the ENTITIES section of an ASCII DXF: LINE, CIRCLE, ARC
(counter-clockwise from the start to the end angle, as the kernel's arcs
are) and LWPOLYLINE / POLYLINE with bulges (a bulge is tan(θ/4) of the
segment's included angle; positive turns counter-clockwise, so a negative
bulge becomes an arc from the next vertex back to this one). The client
adds the geometry to the active sketch as one undo step through
`applyRaw`, tying endpoints that land on each other, or on an existing
sketch point, with coincident constraints so imported outlines close
into regions that extrude.

### STEP export (`ok-step`)

`write_step` writes AP214 Part 21: the unit and context entities once,
then per body a `MANIFOLD_SOLID_BREP` in a `CLOSED_SHELL` with an
`ADVANCED_BREP_SHAPE_REPRESENTATION`, product and definition entities
so readers see named parts. Every vertex is written at its exact position
(`exact::vertex_position`: the point on all of its analytic surfaces).
Planar faces go out as they are, but each loop's facet edges are grouped
into runs (`exact::edge_runs`: chains of edges between one pair of
surfaces) and a run on an exact curve is one `EDGE_CURVE`: a `LINE`
between planes, a `CIRCLE` or `ELLIPSE` where a plane meets a cylinder
(the curve turned so the run's direction is its parametric direction,
the reference direction through the run's first vertex), or a degree-one
`B_SPLINE_CURVE_WITH_KNOTS` through the run's points sampled at every
degree of the first cylinder's rulings where two cylinders meet; a run
between two exact surfaces on no such curve goes segment by segment
through its exact vertices). Planar facets in one plane are merged
first. The facets of a cylinder, cone, torus or sphere become one
`ADVANCED_FACE` per connected region (`exact::surface_regions`,
components by facet seams) on a `CYLINDRICAL_SURFACE`,
`CONICAL_SURFACE` (placed at the apex), `TOROIDAL_SURFACE` or
`SPHERICAL_SURFACE` (placed with its axis across the patch and the
seam meridian at the patch's antipode, so its loops never wrap); a
region whose loops sweep a full turn gets a seam along a meridian that
both rims have a vertex on and no hole spans (a ruling, or a circle
round a torus's tube through the facet vertices between), its outer
bound then one rim round, the seam, the other rim round and the seam
back (the rims' closed runs rotated to start at the seam; surfaces
sharing a rim seam it at the same vertex, since a closed run starts
once). A surface no seam can be placed on, and facets on revolved
splines or ruled surfaces, stay facets with line edges. Entity numbers
are sequential; reals carry a decimal point; strings escape apostrophes
and non-ASCII.

### STEP import (`ok-step/src/read.rs`)

`read_step` parses a Part 21 file (strings with doubled quotes,
comments, complex instances as lists of typed parts) into an entity map
and walks every `MANIFOLD_SOLID_BREP`: shell → faces → bounds → edge
loops → oriented edges → edge curves → vertex points. Each edge is
sampled once (a line stays straight; a circle or ellipse is sampled at
5° from its start vertex around its axis, the whole way round for a
closed edge; a B-spline, rational or not, is evaluated by de Boor at
sixteen points per span, a degree-one one at its poles alone, and
reversed if the edge runs against the curve) so the two faces on an
edge share the same mesh vertices and the result welds closed. A planar
face is triangulated by earcut in its plane, outer bound first, with
the loop's own winding deciding the normal when it disagrees with the
flags. A face on a surface of revolution (cylinder, cone, torus or
sphere) is mapped to (angle about the axis, profile parameter: height,
tube angle or latitude), each scaled to a length, the angle unwrapped
along each loop so a loop around the seam stays continuous (a torus's
tube angle likewise) and holes shifted into the outer loop's turn;
because a triangle spanning more than a facet cuts a chord through the
surface (earcut fans from a seam corner would turn a wall into two
cones), the parameter polygon is cut into a grid of facet-wide columns
and, where the profile curves, rows, and each cell triangulated on its
own. That takes two passes: first every such face decides its grid and
puts a mesh vertex where each grid line crosses an edge of the face (on
the surface, at the interpolated other coordinate), into the edge's
shared samples, so the face on the other side uses the same point; a
sample within a hundredth of a cell of a grid line moves onto it
instead, and samples closer than that to their neighbours are dropped,
since a triangle thinner than the mesh importer's tolerance would be
lost and open the mesh. Then each face clips its loops to every cell
exactly at the grid lines (`ok_brep::clip2d`, the same clipper the
boolean uses to close cut loops in a cell; a cell no loop crosses is
kept or dropped by where its corner lies) and triangulates the pieces,
reusing the loop vertices, so no new vertex ever lies on a shared edge.
Other surfaces are refused by name. The length
unit comes from the header's `LENGTH_UNIT` (`SI_UNIT` prefixes,
`CONVERSION_BASED_UNIT` inch, foot, yard). The bodies come back as
vertices and triangles for `add_mesh`, whose coplanar merge restores
the planar faces with their hole loops; the wasm build exposes
`parse_step`, the client's Import button accepts `.step`/`.stp`, and
`ok-mcp`'s `import` tool reads STL and OBJ (`ok_mesh::from_stl`,
`from_obj`) as well.

### Drawing views (`drawing.rs`)

`project_view` projects solids orthographically along a view direction
and removes hidden lines exactly for the faceted geometry: every display
edge, plus every silhouette seam of a curved surface (a facet facing the
viewer next to one facing away), is split into the parts covered by a
nearer face that faces the viewer and the parts that are not. Coverage is
decided per face by cutting the projected edge at the face outline's
crossings and testing each piece's midpoint (even-odd, outline counts as
covered), and depth is compared linearly along the piece, splitting where
the edge passes through the face's plane. Collinear overlaps are merged
with visible lines winning, so the back edges of a box seen square on
draw once. Edges on an exact circle or ellipse (`exact::edge_runs` and
`edge_curve`) are not returned as their chords: the chords go through
the same hidden-line work, and their visible and hidden pieces, measured
as parameter intervals of the ellipse the curve projects to, are joined
into arcs (`visible_arcs`, `hidden_arcs`; two runs projecting onto one
ellipse, the rims of a hole seen along it, are one, and hidden arcs are
cut back where visible ones cover them). A rim seen edge-on becomes a
single line the same way. The client lays front, top, right and
isometric views out in third angle and writes an SVG sheet (arcs as
path arcs) or a DXF (R2000: `CIRCLE`, `ARC` and `ELLIPSE` entities
beside the lines). The measure tool's edge length comes from
`exact::run_length` through the wasm `edge_length`: along the circle or
ellipse for a rim, so a Ø20 hole measures 62.832 rather than its 72
chords.

A section view (`section_view`) cuts each solid with a boolean
difference against a box covering the removed side of the cutting plane,
draws what remains with the same hidden-line removal, and returns the
outlines of the faces lying in the cut plane. The client hatches those at
45° (clipped by the even-odd rule, so holes stay clear), captions the view
"SECTION A-A" and draws the cutting-plane trace, lettered at both ends,
across the view where the plane shows edge-on. The plane is the
viewport's section plane when one is shown, else a cut through the middle
of the model parallel to the front view. A detail view (client side)
takes the standard view that faces the selected face best, clips its
lines to a circle around the face and enlarges them 2:1, drawing the
lettered marker circle on the source view. The Drawing dialog in the client
renders the same SVG inline as a preview and lets the user pick the views
and the sheet size (A4, A3, A2, Letter) before downloading. Dimensions
can be placed on the preview: `drawingFrame` reports where each view
sits on the sheet, a click is mapped back into view coordinates and
snapped to the nearest line endpoint, and two picks on one view become
a `DrawingDimension` (aligned, its text along the span, the dimension
line on the side away from the view's middle) that both the SVG and the
DXF draw beside the automatic overall dimensions. The body summary also
lists each body's cylindrical surfaces (axis, radius, extent, hole or
boss); a standard view whose direction runs along such an axis sees it
end-on and gets a diameter callout with a leader ("Ø12", or "4× Ø6" when
several of one size are visible). An assembly sheet (or a part studio
with several bodies) carries a parts list above the title block, one
item per distinct part with its quantity and material, and item
balloons on the isometric view anchored at each part's centroid; the
views fit in the room above the list. Placed dimensions are
document state: each `Tab` carries its `drawing` list, replaced through
`DocOp::SetDrawingDimensions` (one undo step, relayed to collaborators
like any edit) and reported per tab in the summary. The Export
menu also writes a bill of materials as CSV (bodies of a part studio, or
instances with their source tab in an assembly).

### Part names and materials

A studio keeps `part_names` and `part_materials` keyed by the feature
that created each body, applied after regeneration so they survive
edits and undo per user (`Op::RenamePart`, `Op::SetPartMaterial`, each
with an inverse). A material is a name and a density in g/cm³; the wasm
summary reports the mass (volume × density / 1000) of every body that
has one, which the parts list and the bill of materials show.

### Split (`FeatureKind::Split`)

`ok_brep::split` cuts a solid by a plane with two booleans against a box
covering the plane's normal side (difference and intersection), so it
inherits the boolean's handling of coincident faces; the cut faces are
tagged with the split feature. The feature splits every body the plane
crosses (or the chosen ones): the part against the normal keeps the
body's slot and name, the other is appended as a new part, and a plane
that misses every body is reported.

### Mesh import (`FeatureKind::Mesh`)

An imported STL or OBJ becomes a `Mesh` feature holding welded vertices and
triangle indices (the client welds corners closer than a millionth of
the mesh size and drops degenerate triangles). Regeneration turns every
triangle into a planar polygon and lets `Solid::from_polygons` decide
whether the mesh closes a volume; an open mesh or one wound inside out
is reported on the feature. Coplanar neighbours are merged
(`merge_coplanar_faces`), so a boxy mesh has boxy faces that later
features can reference by origin like any other body.

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
Solid vertices within tolerance of the section plane are snapped onto it
first; a face parallel to the plane is skipped (it has no crossing line),
so when any of its vertices sits on the plane all of them are pulled onto
it, otherwise a vertex a rounding error past the snap band leaves the
neighbouring faces producing crossings that nothing closes (the fuzz
found this with a nudge equal to the tolerance).

After assembly, planar faces in the same plane that share an edge are
merged into one face (`merge_coplanar_faces`), keeping the surface and
origin of the largest member, so flush unions produce single faces.
Last, the result is refitted to its exact surfaces (`exact::refit_within`
over the overlap of the operands' boxes; see "Exact curves" below), so a
vertex the sectioning put where two facet planes met sits on the curve
the surfaces meet on.

Known limits: results are polyhedral (arcs are 5° facets, with their
vertices on the exact surfaces after the refit), and the merge scope is
"every body whose bounding box touches the tool".

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

## Server and collaboration (`ok-server`)

`ok-server` is an Axum binary that serves the built web app, stores
documents as `.okpart` JSON files with a small metadata file each, and
relays edits between clients:

- Headless: `GET /api/docs/:id/check` regenerates every tab on the
  server (the live copy when one is open) and reports each tab's bodies
  (name, volume, area, faces, bounds, material and mass) and feature,
  instance or mate errors, with an overall `ok`; `GET
  /api/docs/:id/export/stl?tab=N` returns the tab's bodies as binary
  STL. Both run the kernel natively on a blocking thread, so scripts and
  CI can validate and export documents without a browser.
- REST: `GET/POST /api/docs`, `GET/PUT/DELETE /api/docs/:id`,
  `GET/PUT /api/docs/:id/thumbnail` (a PNG preview, at most 512 KB, that
  the client captures from the viewport when the Docs dialog opens and
  the list shows beside each document),
  `POST/DELETE /api/docs/:id/share`, versions under `/api/docs/:id/versions`
  (list, save, fetch one, restore); `POST /api/docs/:id/branch` copies
  the document, as it is or at a saved version, into a new document
  owned by the caller whose metadata names the origin; the state it
  started from is kept beside it as the merge base. `POST
  /api/docs/:id/merge {from}` is a three-way merge between a branch and
  its origin in either direction (`ok_model::Document::merge_from`):
  features are matched by id per part studio, instances and mates by id
  per assembly, tabs by id, plus the document, tab, part and material
  names. An item changed only on the far side comes over as ordinary ops
  (insert / delete / restore) applied through the live document, so open
  clients see the merge like any other edit; an item changed on both
  sides is a conflict, reported and left as it is. Branches take their
  client id-range prefixes from their root origin so ids never collide
  across a branch family. Comparing a version with the current
  document happens in the client: both JSON documents are loaded into
  their own wasm `Doc`, every tab is regenerated in each, and features
  are matched by id (added, removed, or changed when their name, kind,
  suppression or bindings differ).
- Accounts (`auth.rs`): `POST /api/auth/register|login|logout`,
  `GET /api/auth/me`. Users live in `users.json` with argon2id password
  hashes; a session is a random token in an `HttpOnly` cookie, persisted
  in `sessions.json` and expiring after 30 days idle. The `CurrentUser`
  extractor resolves the cookie on every request. Signing in is optional:
  a document created anonymously has no owner and is open to everyone on
  the server; one created while signed in belongs to that account, is
  listed only for the owner and the accounts it is shared with (404 for
  anyone else), and only the owner can delete or share it. Sharing is by
  account name (`POST /api/docs/:id/share`, editor or viewer role) or by
  invitation link: the owner mints a token (`POST /api/docs/:id/invites`,
  32 random bytes as hex, stored in the document's metadata with its
  role) and hands out `?doc=<id>&invite=<token>`; any signed-in account
  that presents it (`POST .../invites/:token/accept`) joins in that role
  until the owner withdraws the link (`DELETE .../invites/:token`),
  which keeps the accounts that already joined. Tokens are stripped from
  metadata served to anyone but the owner, so a viewer cannot use an
  editor link it was never given. Teams (`teams.rs`, `teams.json`) are
  named groups of accounts: the creator owns one and adds or removes
  members (`/api/teams`, `/api/teams/:id/members`), and an owner shares a
  document with a team they belong to in a role (`POST
  /api/docs/:id/share-team`). The `CurrentUser` extractor fills in the
  signed-in user's team ids on every request, so the access checks are
  the same functions as for direct shares. The WebSocket applies the same
  access check and shows a signed-in client under its account name.
- WebSocket `/api/docs/:id/ws`: a client sends `hello`, then `op`
  messages carrying `ok_model::Op` JSON. The server applies each op to
  its own copy of the document (rejecting invalid ones with an `error`
  to the sender only), assigns a sequence number, persists, and
  broadcasts the op to every client. `snapshot` returns the current
  document; `presence` announces who is connected.

The client applies its own ops optimistically and sends them; ops from
others are applied on arrival. Ops commute because every replica
allocates the same ids for the same op: on `welcome` the server hands the
client a persistent 12-bit id-range prefix (stored in the document's
metadata and never reused), and each op carries an id base
`(prefix << 20) | counter` from which `PartStudio::apply_with_base`
allocates any new feature, entity and constraint ids. Concurrent adds
therefore never collide, and applying two clients' ops in different
orders yields the same document. Default feature names are chosen by the
client for the same reason.

Every broadcast op carries the server's structural hash of the document
(feature order, ids, kinds and references, but no floating-point values,
which can differ in the last bits between native and wasm). The hash is
computed with fixed-width writes only, because the standard `Hash` impls
for slices, strings and enums write `usize`/`isize` values whose width
differs between the 32-bit wasm client and the 64-bit server. A client
with nothing in flight compares its own hash and resyncs from a snapshot
on a mismatch, which covers the remaining order-dependent edits such as
two simultaneous adds (same ids, different list order until the server
order wins). Semantic conflicts (editing a feature someone just deleted)
are rejected by the server and trigger a resync on that client.

### Undo (`ok-model/src/invert.rs`)

Undo is op-based and per user. `PartStudio::apply_with_inverse` applies
an op and returns, in `OpResult::inverse`, the ops that undo it, computed
from the state before and after: creations are undone by `DeleteFeature`,
a deletion by `InsertFeature` carrying the saved feature at its old index,
`Set*` ops by the same op with the old values of just the fields they
changed, a binding change by restoring the binding and the field's old
value, and any sketch op by `SketchOp::Restore`, a patch obtained by
diffing the sketch: entities and constraints the op created are removed,
those it removed or changed are put back by id, flags and projections
follow. The client keeps a stack of these inverse lists (one entry per
user-level edit, so a drag with many `move_point`s is one step) and undoes
by applying them as ordinary synced ops, which reverts only that user's
edit and leaves everyone else's in place; the inverses of the inverses
form the redo stack. An inverse that no longer applies (someone deleted
the feature meanwhile) is skipped with a status message.

## Scripts and agents (`ok-mcp`)

`ok_model::Document::describe` builds a `TabReport`: features with ids,
kinds, errors and parameters; bodies with volume, bounds, centroid and
one entry per face reference (`{feature, local, part}`, with the facets
of a curved surface folded into one line) plus cylinders with axis,
radius and hole-or-boss; sketches with solver status, degrees of freedom,
region count, entities and constraints; instances and mates for
assemblies. The server serves it at `GET /api/docs/:id/report` and
applies op batches at `POST /api/docs/:id/ops` through the live document
(`LiveDoc::apply_many`), so scripted edits reach open browsers like any
other. `crates/ok-mcp` is a Model Context Protocol server over stdio
(hand-written JSON-RPC: `initialize`, `tools/list`, `tools/call`,
`resources/read`) whose tools wrap those two endpoints, or the kernel
directly on a local file; its `apply` tool wraps bare ops in their tab
envelope and follows every batch with the report, and its reference
resource is docs/OPS.md.

### Headless renders (`ok-render`)

`ok_render::screenshot(doc, tab, options)` regenerates a tab and
rasterises its bodies to a PNG without a GPU or browser: an orthographic
camera fitted to the bodies' bounds (the client's `top`, `front`,
`right` and `iso` directions, or any eye vector), a depth buffer over the
display tessellation with Lambert shading from a light over the viewer's
shoulder, the `display_edges` drawn on top with a small depth bias, an
outline pass that darkens pixels where the nearest body changes or the
depth jumps (so cylinder rims and overlaps read without an edge
between facets), and an axis triad. A section (`axis:offset[:flip]`,
the client's clip) is applied by splitting each solid with
`ok_brep::split` and drawing the kept half, so the cut faces come back
as real faces and are painted with diagonal hatching rather than showing
a hollow interior; the camera still fits the uncut bounds so the picture
does not jump as the plane moves. The PNG encoder is forty lines over
`miniz_oxide` (8-bit RGB, filter 0), with a matching decoder for tests.
The server serves it at `GET /api/docs/:id/screenshot` and `ok-mcp`
returns it as MCP image content from its `screenshot` tool (a base64
encoder is inlined rather than pulled in). The crate is not part of the
wasm build; the browser has its own renderer.

## Performance

`scripts/bench.sh` prints regeneration timings (release build) for the demo
plate, the same plate with bosses and 24 holes (2000 faces), and that
part shelled (a 4000-face cover), cold and with only the last feature
edited: about 9 ms, 70 ms and 250 ms cold at the time of writing (from
29 ms, 145 ms and 1.5 s before the two profiling passes). Booleans are
the cost that scales: every face of one solid is classified against a
cross-section of the other. The other solid's faces sit in a
bounding-volume tree built once per boolean, so a section visits only
the subtrees the plane cuts, the sections just above and just below a
face share that work, vertex distances are computed on demand rather
than for every vertex per section, loops are chained without rescanning
the segment map, the overlay leaves out section loops clear of the
face, and the kernel's integer-keyed maps use a fast multiply-rotate
hasher.

The second pass made the boolean incremental, so its cost follows the
faces the operation touches rather than the size of the operands:

- Faces whose box stays clear of the other solid's box pass into the
  result with their vertex ids; only the fragments of touched faces are
  welded, against a merger seeded with the touched faces' vertices (and
  any vertex near the other solid's box), and vertices nothing uses are
  dropped at the end. Two solids with no face box near the other are
  apart or nested, decided by a winding-number test, and skip the
  boolean entirely (a hole pattern's drills union in microseconds).
- The classification sections the other solid *locally*: only the faces
  whose box reaches a rectangle drawn around the face (5% margin) are
  cut, the loops of a large face that stay clear of that box are skipped
  (a closed loop crosses the plane an even number of times, so parity
  holds), crossing segments outside the box along the crossing line are
  left out in pairs, and the resulting open chains are closed along the
  rectangle's boundary: material lies to the left of a chain, so from
  where a chain exits, the boundary is followed counter-clockwise to the
  next entry. A rectangle no chain touches is filled or left empty by a
  ray probe from its corner along the face normal, with the same
  vertex-pulling rules the section uses: a face through the corner
  decides by its orientation, and a face within a few tolerances of the
  plane is too close to call against the section's infinitesimal hair,
  in which case (as for any other ambiguity: a chain vertex on the
  rectangle's edge, endpoints that do not alternate) the whole solid is
  sectioned as before. A full or empty rectangle, or loops the overlay
  leaves the face whole under, return the face itself rather than a
  re-welded copy; when the sections above and below coincide the inside
  rule intersects once, since a second intersection against identical
  edges can leave grid-unit slivers.
- The snapping of nearly coincident geometry looks up vertices through
  the point grid and faces through the bounding-volume tree instead of
  scanning, and the 2D snap of section loops onto a face outline buckets
  the outline's edges on a grid (a plate face with 24 holes has two
  thousand edges). The offset polyhedron of a shell orders the faces
  around a corner from a vertex-to-faces table rather than by scanning
  every face per corner.

Assembling the result was dominated by T-junction
repair, which asks for the vertices near every edge; its point grid is a
dense array of cells sized to the model (about 1/32 of the extent, never
below 64 tolerances), so a query is index arithmetic along the edge
rather than hashing, and consecutive samples skip the cells the previous
one gathered. A finer grid is slower, not faster: the cost is cells
visited, not candidates tested. A boolean also limits the repair to
edges reaching into the overlap of the operands' boxes, which is sound
because fragments restore their face's own collinear vertices. The wasm
build runs `wasm-opt -O3` when binaryen is available (the web app's dev
dependencies provide it).

## Testing

`examples/router-lift/` is a whole project (a router table lift and a
pin-router attachment, 25 parts and an assembly) built through the MCP
server by `build.py`, the way a language model would build it; the
committed document is regenerated by `crates/ok-render/tests/router_lift.rs`,
which requires every part to close and the printed parts to match their
OpenSCAD reference meshes in volume and extent. Its README lists what the
port found wanting.

Three layers guard the kernel. Unit tests in each crate check numbers
(areas, volumes, DOF counts). Two randomised boolean tests in
`crates/ok-brep/tests/fuzz.rs` run sequences of unions, differences and
intersections and check closure and volume bounds: one on an integer grid
so coplanar and touching cases are common, one in general position with
rotated tools and tiny nudges (`OK_FUZZ_EPS` picks the nudge sizes, `OK_FUZZ_SEED` replays one seed) so
nearly coincident geometry is common; `OK_FUZZ_DUMP=<dir>` writes the
operands of a failing step as JSON and the ignored `replay_dumped_case`
test in `boolean.rs` reruns them). The long general-position run passes at
every nudge size from 1e-8 to 1e-3. Operands that once failed are kept
under `crates/ok-brep/tests/cases` and replayed by `tests/cases.rs`: a
vertex a hair off its face's plane (within the planarity the solid
allows) used to be lifted onto the plane when the face's fragments were
rebuilt while its other faces kept it in place, so the fragments
disagreed by more than the merge tolerance; and a face nearly coplanar
with a face of the other solid used to get only the vertices inside that
face's box snapped onto its plane, leaving it tilted by a hair and its
sections empty. Fragments now keep their own vertices' positions, and a
nearly coplanar face is snapped whole. Stitching open vertices and
collapsing short edges may move a vertex by up to ten times the merge
tolerance, which can leave a face non-planar by that much; assembly then
splits such a face into triangles (same surface tag and origin, so it
still shades and selects as one face) so every face a boolean sections
is planar within tolerance. A third ignored run, `random_blends_stay_closed`,
fillets and chamfers random edges of bodies built by a few booleans and
requires every result that is not an error to be closed (`OK_FUZZ_BLENDS`
sets the seed count). `crates/ok-model/tests/parts.rs`
is a corpus of realistic parts built through ops, each regenerated
without errors, validated closed and checked against hand-calculated
volumes. Booleans merge their fragments at the same tolerance the
classification used, so seams between fragments of the two inputs meet.

## Conventions

- Model units are millimetres; angles in the document are degrees.
- Planes: Top = XY (normal +Z), Front = XZ (normal −Y), Right = YZ
  (normal +X). All are right-handed frames.
- Ids (`FeatureId`, `EntityId`, `ConstraintId`) are stable for the lifetime
  of the document and never reused.
- Documents serialise to JSON (`.okpart`). The format is not yet stable.
