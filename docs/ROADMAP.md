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
- [x] Sketch fillet: round the corner between two lines with a tangent, dimensioned arc
- [x] Dimension labels in the viewport with click-to-edit (values or expressions)
- [x] Sparse Jacobian: analytic rows for the common constraints, local differences for the rest; rank from the normal matrix
- [x] Splines (interpolating Catmull–Rom through sketch points; sampled into one smooth wall)
- [x] Point-on-spline constraint
- [x] Tangent constraint between a line and a spline end (the line follows the end chord)
- [x] Sketch on planar faces of bodies (face references)
- [x] Angled planes: a standard plane turned about a world axis (angle bindable), usable wherever a plane is chosen
- [x] Project / use edges and face outlines from bodies in sketches (fixed entities that follow the model)
- [x] Construction geometry, mirror
- [x] Sketch patterns: linear copies held by distance constraints, circular copies by a rotated-point constraint about a construction centre

## Solids

- [x] Extrude regions (normal / reverse / symmetric)
- [x] Boundary representation: shared-vertex planar faces with analytic surface tags (plane, cylinder) and face origins
- [x] Booleans (union / subtract / intersect) on polyhedral solids, with exact coplanar handling
- [x] Adjustable facet resolution per document (0.5° to 30° per facet) flowing through profiles, revolves, holes and blends
- [~] Exact curved faces (cylinder, later general surfaces) instead of facets, with curve/surface intersection. Under way on the `exact-surfaces` branch (`docs/EXACT.md`): exact curves are recovered from the surface tags, STEP goes out with exact cylinders, and every boolean refits its result so vertices lie on the exact curves; trimmed parametric faces for general surfaces are still ahead.
- [x] Merge coplanar faces of the same body after a boolean
- [x] Extrude up-to-face / through-all
- [x] Revolve (about a sketch axis or sketch line, full or partial)
- [x] Hole feature (through / blind, optional counterbore) at sketch points
- [x] Sweep (profile along a sketched polyline / arc path) and loft (between two sketch regions)
- [x] Fillet and chamfer on straight edges (blend by boolean with a cutter prism per edge)
- [x] Fillets and chamfers on curved edges (a rim is one chain, blended with one mitred sweep)
- [x] Spherical corner patches where three fillets meet on planar faces (exact rolling ball, one cutter polyhedron per group of corners)
- [x] Variable-dihedral chains: a blend along a chain (a rim) rebuilds its cross-section at every vertex from the faces there and lofts the sections into one cutter, so a fillet around an obliquely cut cylinder fits all the way round; the cutter leaves the faces squarely at the tangent lines instead of with walls lying almost in the facets
- [~] Corner patches for chamfers: three chamfers meeting at a corner already leave the three chamfer planes meeting at a point, as other CAD systems do, through the plain union of their cutters; corners where four or more fillets meet, where no single ball is tangent to every face, keep the union of cutters and are not planned
- [x] Booleans stay closed when a vertex sits a hair off its face plane or a face is nearly coplanar with the tool (the two former fuzz failures are regression cases)
- [x] Faces left non-planar by stitching are split into planar triangles at assembly, so every sectioned face is planar within tolerance
- [x] Shell (uniform wall, chosen faces open; an offset polyhedron subtracted in one boolean; cavities whose offset changes the topology are reported, not guessed)
- [x] Draft (tilt planar faces about a neutral plane) and Move face (push / pull planar faces): direct edits that re-solve the surrounding corners
- [x] Mirror and linear / circular patterns of bodies
- [x] Boolean feature between existing bodies (union / subtract / intersect, tools optionally kept)
- [x] Split bodies by a plane into two parts (the plane offset is bindable)
- [x] Part materials (name + density) with mass in the parts list and bill of materials; standard metric hole presets
- [x] Patterns and mirrors of features (the named features' tool volumes are replayed with their own add / remove operation)
- [x] Mirror / pattern selected bodies only (tick bodies in the panel)
- [~] Patterns of faces: covered by feature patterns here (every face comes from a feature whose tool volume is replayed); a face-only pattern would need face-bounded volumes, which the polyhedral kernel does not keep
- [x] Persistent naming: face origins survive booleans, and pieces of a face split by later features are numbered by position so references name the piece they mean
- [x] Naming that follows a piece when an edit reorders pieces: a face reference records hashes of the origins of the neighbouring pieces when it is made (from the summary or the report), and lookups pick the piece whose neighbours match best, falling back to the piece number; a healed split resolves to the one piece left

## Document and client

- [x] Ordered feature list with suppression, reorder, delete
- [x] JSON document format and browser persistence
- [x] Undo/redo (document snapshots in the client; op-log based undo later)
- [x] Variables and expressions in dimensions (`#width / 2`) via per-field bindings
- [x] Face selection in the viewport with picking
- [x] Edge selection (edges are named by their two faces)
- [x] Rollback bar in the feature list
- [x] Part hide/show and rename from the parts list; standard views (top/front/right/iso); measure tool (corners, faces, edges and cylinders: distances, angles, lengths, diameters); section view (axis-aligned clipping)
- [x] Constraint glyphs in the viewport while editing a sketch (click to remove with the select tool)
- [x] Multiple part studios per document; assemblies with fastened / revolute / slider / cylindrical mates resolved as chains from fixed instances
- [x] Interference check between placed instances (boolean intersection, on demand)
- [x] Numeric mate solver: closed loops and redundant mates solved over the free degrees of freedom of revolute, slider and cylindrical mates
- [x] Planar and ball mates; sub-assemblies (an assembly tab inserted as one rigid group, cycles refused); mate frames drawn in the viewer
- [x] Explode view slider (display only) and dragging free instances in the viewport (one undo step per drag)
- [x] Mate connectors on edges and vertices
- [x] Mate animation (display-only sweep of a revolute, cylindrical or slider mate)
- [x] STL and 3MF export of bodies (3MF keeps part names); DXF export of a sketch
- [x] Drawings: front / top / right / isometric views with exact hidden-line removal for the faceted geometry, laid out third-angle on an A4 sheet (SVG) or as DXF lines; on the `exact-surfaces` branch circle and ellipse edges are drawn as arcs (SVG arcs, DXF `ARC`/`ELLIPSE`)
- [x] Automatic overall dimensions (width, height, depth) on drawing sheets
- [x] Section views on drawings (hatched cut faces, lettered cutting-plane trace; follows the viewport section plane)
- [x] Drawing dialog: choose the views and sheet size with a live preview; bill of materials export (CSV) for part studios and assemblies
- [x] Detail views on drawings: the selected face's neighbourhood enlarged 2:1 from the view that faces it, with a lettered marker circle
- [x] Dimensions placed by the user on drawings (two corners of a view in the sheet preview; aligned, on the side away from the view); they are document state
- [x] Diameter callouts for holes and bosses seen end-on in the standard views, counted when several share a size
- [x] Parts list and item balloons on assembly sheets (and multi-body part studios)
- [x] STEP export (AP214): each body a manifold B-rep, named; planar faces with line edges, and on the `exact-surfaces` branch cylindrical faces as `CYLINDRICAL_SURFACE`s bounded by circles, ellipses and B-splines with vertices at their exact positions
- [x] STEP with exact cylindrical, conical, toroidal and spherical faces, written and read back, on the `exact-surfaces` branch (see `docs/EXACT.md`)
- [x] Import: STL (binary or ASCII) and OBJ meshes as bodies; coplanar triangles merge into faces
- [x] Import: DXF lines, circles, arcs and polylines (with bulges) into a sketch, endpoints tied by coincident constraints
- [x] Import: STEP via a B-rep reader (`ok-step::read_step`): solids of a Part 21 file walked from `MANIFOLD_SOLID_BREP` down to points, edges on lines, circles and B-splines sampled once and shared, faces on planes and cylinders triangulated in their own parameters (cylinders in facet-wide strips), lengths scaled to millimetres; other surfaces are refused by name. Imported as mesh bodies from the client's Import button, the `ok-mcp` `import` tool (which also reads STL and OBJ) and `parse_step` in the wasm API
- [x] Documents list with previews, a name filter and sort order
- [x] Language-model access: `ok-mcp` (MCP over stdio) with apply / report / export tools against a running server or a local file; `POST /api/docs/:id/ops` and `GET /api/docs/:id/report` for scripts
- [x] Screenshot tool for models: `ok-render` rasterises a tab's tessellation headlessly (orthographic standard views or any direction, optional section with hatched caps, edges and silhouettes, axis triad); served at `GET /api/docs/:id/screenshot` and returned as image content by the `ok-mcp` `screenshot` tool
- [x] Incremental boolean assembly: faces whose box stays clear of the other solid pass through with their vertex ids, only fragments are welded, the classification sections the other solid locally around each face (chains closed along a rectangle, a ray probe for uniform faces) instead of cutting the whole solid per face, and solids that share no face box skip the boolean (apart, or one inside the other); a 24-hole plate with bosses regenerates in about 70 ms and the shelled cover in about 250 ms, from 145 and 760 ms

## Platform

- [x] Document server (`ok-server`): storage, static app, REST
- [x] Server-side regeneration: `/check` reports bodies and errors per tab, `/export/stl` returns a tab as STL, for scripts and CI
- [x] Named versions of a document (save / restore on the server)
- [x] Diffs: compare a saved version with the document now (features added / changed / removed per tab)
- [x] Branches: copy a document, as it is or at a saved version, into a new document that remembers its origin
- [x] Merges between a branch and its origin, both ways: three-way at the feature / instance / mate level against the branch point; changes on both sides of one item are reported and left alone
- [x] Real-time multi-user editing (ops relayed in server order)
- [x] Conflict-free concurrent editing: per-client id ranges make ops commute; structural hashes detect and repair any divergence
- [x] Per-user undo in shared documents (inverse ops instead of document replacement)
- [x] Accounts (argon2id passwords, cookie sessions) and per-document sharing
- [x] Server hardening: sign-in rate limiting per address, request and WebSocket size limits, security headers, `--secure-cookies`
- [x] Read-only collaborators: share as viewer; the server refuses their edits and the client shows a read-only badge
- [x] Invitations by link (editor or viewer role, withdrawable)
- [x] Teams: named groups of accounts; documents shared with a team as editors or viewers

## Engineering

- [x] CI: fmt, clippy, tests, wasm build, web build, browser end-to-end suite
- [x] Dockerfile for the server + web app
- [x] Randomised boolean robustness tests: grid-aligned box/cylinder sequences, and general-position sequences with rotated tools and near-coincident nudges (`--ignored` long runs; `OK_FUZZ_SEED` replays one seed)
- [x] Realistic-parts corpus (`crates/ok-model/tests/parts.rs`): brackets, revolved flanges, pockets with counterbores, bosses on oblique faces, pulleys, grazing cuts, sweeps and lofts, patterns then fillets
- [x] Randomised property tests for the solver and region extraction (`crates/ok-sketch/tests/property.rs`)
- [x] Regeneration cache: unchanged feature prefixes are reused, so editing late features is cheap
- [x] Benchmarks for regeneration time on realistic parts (`scripts/bench.sh`); the boolean's T-junction grid and section prefilters came out of the first run (a 4000-face cover shells in 0.65 s, from 3.9 s), the incremental boolean out of the second (0.25 s)
- [x] `wasm-opt` in the release pipeline (from the binaryen npm package when present)
