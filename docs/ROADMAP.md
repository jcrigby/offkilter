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
- [x] Hole feature (through / blind, optional counterbore or countersink) at sketch points
- [x] Puzzle feature: a jigsaw grid of pieces with interlocking tabs (lines and tangent arcs), per-tab direction, size, neck and position, movable corners, a gap for a resin fill, an alignment web in the gaps, and pin router design rules that name what the bit cannot cut
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
- [x] Planar and ball mates; sub-assemblies (an assembly tab inserted as one rigid group, cycles refused; a connector names the member its face is on); mate frames drawn in the viewer
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
- [x] Shop drawing sheets from the kernel (`ok-sheet`): third-angle views at a standard scale, overall dimensions, diameter callouts, balloons and a parts list (a sub-assembly instance one item, with its own sheet from its own tab; hidden lines off on multi-part sheets), title block, written as vector PDF; from the MCP `export` tool, the server's `/export/pdf` and the drawing dialog
- [x] Language-model access: `ok-mcp` (MCP over stdio) with apply / report / screenshot / export (STL, STEP, DXF views, PDF sheets) tools against a running server or a local file; `POST /api/docs/:id/ops` and `GET /api/docs/:id/report` for scripts
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

- [x] Mechanism-at-limit checks of the router lift example: the carriage's travel and the rev D pin arm's pivot swept with interference, clearance and alignment measured (`router_lift_limits.rs`), the BOM checked against the shop drawings
- [x] `ok-sheet`: a 1:2.5 scale between 1:2 and 1:5 (the carriage assembly's sheet went from 1:5 to 1:2.5 on A4), and the views beyond the elevations go in a row under them when that fits a larger scale than a row to their right
- [x] A second whole project, `examples/ego-cart/`: a powered two-wheel shopping cart driven by an EGO power head through a 50:1 gearbox of printed gears, laid out from the speeds and the hill with every bought part a named assumption, and its regression test (closed parts, the gear train meshing, the load on the platform, no body inside another); the examples share one MCP client, `examples/okmcp.py`
- [x] Range of motion: `range_of_motion` draws an assembly at several positions of its mates (any number of mates per position) side by side, as a PNG strip fitted to one box or a captioned PDF sheet; the kernel previews an assembly with several mates set at once (`preview_assembly_at`)
- [x] Section views on PDF sheets: `section` (parallel to the front view) and `section-side` (parallel to the right view), each placed with `@<mm>`, cut faces hatched even-odd, captioned, the cutting plane traced with arrows on the view it is edge-on in
- [x] Measuring from a photograph: `measuring_sheet` prints a grid with four bullseye marks, `measure_photo` squares a picture of parts on it up and reports their sizes, outlines and holes in millimetres, and can put them in a sketch (`crates/ok-photo`, `examples/measuring-sheet/`)
- [x] The sheet says which size it is (a row of dots by the origin mark), and a reference of known size on it (a forensic photo scale's 10 mm bars, or a coin) gives the print scale, so a sheet that printed at 97 % still measures true; Tabloid joins the sheet sizes
- [x] Assembly sheets: an exploded isometric (`explode` on the PDF sheet, the MCP `export` tool and the server route; the drawing dialog's "Exploded iso" takes the viewport's slider), each part slid from the parts' common centre with its balloon
- [x] Kernel: `extrude` refuses a left-handed plane frame (x × y against the normal) instead of building inside-out faces from it; the "split through two corners fails" finding was a test passing such a frame, and the split works, so that test runs
- [x] Gear teeth: a root fillet (an arc tangent to the flank and the root circle at every corner, carrying on up the involute past a short radial piece; 0.38 modules is the rack standard, a ring's narrow spaces take about 0.15) and a profile shift for external gears (+0.3 takes a 12-tooth pinion at 20°, the undercut rule and its message know it, `working_centre_distance` says where a shifted pair mates); the cart's pinion and ring get fillets
- [x] Kernel: two solids touching along an edge of a facet (a facet next to a split, with the other piece wholly on its inner side) classified the facet as inside the other solid, because the section a hair inside was a zero-area sliver of collinear points whose rounding-noise area read as a hole; section loops thinner than the tolerance are dropped, and two halves of a hexagonal prism now intersect to nothing and union to the prism (the cart's ring sectors were the case)
- [x] EGO cart: the ring gear printed as six sectors, a dowel hole drilled across every joint before the radial cuts so each sector carries half; the test checks each sector fits a 220 mm bed, the six are alike and each has a hole at both ends, and the sectors together mesh with the pinion
- [x] Puzzle: a `spread` for the fabrication layouts' rows (zero keeps one bit diameter) and a wood per colour (`light`, `dark`, name and density) that every piece carries as its material, so the parts list names the species and the masses follow; the puzzle-top example sets maple and walnut
- [x] Drawing dialog: a hidden-lines choice (auto, on, off) that strips the dashed lines from the SVG and DXF and sets the PDF's `hidden`, auto being the kernel's rule
- [x] `ok-sheet`: the PDF content stream is deflated (the router lift's assembly sheet went from 148 KB to 32 KB); `ok_sheet::pdf::inflated` gives a sheet back as text for the tests and anything else that greps one
- [x] EGO cart rev B: a powered tricycle, one 20 inch bicycle wheel driven through a printed internal ring gear on its disc mount by a bought worm box's pinion, a flex shaft from the EGO stub, two casters on a wide track; `build.py` and the regression test compute the stability from the parts' masses (drive wheel share, side tip angle with two bags, one bag and none, the mass centre on the hill) and that set the load and caster positions
- [x] Kernel: a cut that leaves an enclosed cavity keeps the body as one lump with its void (`Solid::shells` groups a void with the lump round it), so a hole into a hollow box drills both walls; before, the cavity became an inside-out body of its own and the next cut failed (`a_hole_into_a_hollow_box` ran ignored as the record of it)
- [x] Gear teeth: `add_gear` draws an involute spur gear, external with a bore or internal with the teeth inside a ring, from module, tooth count, pressure angle, face width and backlash; each flank is one face and the circles are cylinders, so bores take mates; the design rules it refuses (undercut, an internal gear's tips inside its base circle, thin walls) come back as the feature's error, and the tests mesh a pinion with a gear and with a ring by boolean intersection
- [x] A steel rule as the reference: its millimetre ticks are found as thin local-dark marks, grouped by edge and fitted, an inch edge told apart by pitch; and with a rule a flatbed scan needs no sheet at all

## Next

Follow-ups nobody has asked for yet, in no order; each came up while
building something else. The reasoning behind the current choices is
in `docs/DECISIONS.md`.

- [x] Gear teeth, continued: straight bevel gears, as `add_gear` with a `cone` angle (Tredgold's virtual spur gear wrapped on the back cone, the teeth ruled to the apex, flat back and front faces, a bore); a 16:32 pair on shafts at 90° meshes by boolean intersection
- [ ] Kernel: a section through a solid with very fine facets can fail to close (the cart's ring sectors with their fillets cut into 0.04 mm facets made adjacent sectors' intersection fail with "cross-section of solid is not closed"; the gear now samples a fillet by chord sag, about four facets, and the pairs pass, but the sensitivity is still there). `crates/ok-brep/tests/fine_facets.rs` is the harness: a cylinder faceted at 0.05° split through its axis, and ring gears with their fillets resampled at 0.04 mm chords cut by three radial planes, every sector pair intersected; the 36-tooth ring and (ignored, three minutes) the cart's 80-tooth ring both pass, so the failure needs more of the cart's ring than its teeth, likely the dowel holes drilled across the cut planes or the web and bolt holes; the next try is the cart document with the fillet sampled at the gear's facet angle
- [x] Puzzle: a printable pattern plate (`groove`, the `pattern` layout): the gap lattice as grooves in a slab, so a pilot the size of the gap rides in them while a bit that size cuts every piece from one board, the kerf being the resin gap; `examples/puzzle-top/out/pattern_plate.stl`
- [ ] The carving duplicator (Woodsmith SN12918) rebuilt on the lift's 20 mm shafts and SC20UU blocks as a third example: X and Y as sliders, Z as a vertical slide with a stop screw for the per-pass depth and the pilot bottoming in the pattern's groove for the last (the Z slide was drawn and rendered in the chat: 52 mm of travel on 250 mm shafts, the tool support on four blocks in front of the shafts, the router and pilot side by side on one plate), with a limits test for reach over a puzzle board and the depth sequence; and a duplicator check on a body meant to be printed as a master (a height field, concave radii and depths against a bit)
- [x] Range of motion in the web app: a positions list (or a stepped range) of a mate's value on the drawing dialog, a captioned strip of a standard view under the sheet in the preview, SVG and DXF, the interference check run at each position with overlapping frames captioned in red, and the kernel's range-of-motion PDF downloadable from the dialog
- [x] Drawing dialog and SVG drawings: the two PDF section views (`section`, `section-side`, each with `@<mm>`), so the client's preview matches the sheet the MCP tool writes
- [ ] `Dockerfile.dev`: build it once on a machine with a Docker daemon; the sandbox it was written in had only the client
- [x] Photo measuring in the web app: a Measure photo button whose dialog shows the squared-up picture and the parts, takes the sheet size and a reference, downloads the printable sheet and adds the outlines sketch; measured in the wasm (`ok-photo` builds for wasm32), not behind a server route, so it works without a server too
- [x] Photo measuring: outlines fitted into straight runs and arcs (least-squares circles, greedy over the simplified corners, within 0.3 mm of the traced boundary), or one circle; the MCP and web sketches draw those instead of the polygon
- [x] Photo measuring: a disc reference wider than tall (by its outline's second moments) warns that the print was stretched one way; a scanner's dpi is read from the PNG or JPEG file and warns when it disagrees with the rule or the marks (the MCP caption and the web dialog show the warnings)

## Backlog

Parked, not planned: the need they answered has gone or moved on.
The models stay as examples and regression tests.

- [ ] EGO cart: a pawl or a freewheel hub against rolling back, the swing arm that lifts the pinion out of the ring for pushing by hand (then its range-of-motion sheet and an interference sweep), the clamps on the mast, and the EP7500 stub once it is measured. Parked: two full bags carry home by hand for now; a two-wheeled cart pulled behind would be the next step, and a motor only well after that. Rev B stays as the gear, stability and sector tests in `crates/ok-render/tests/ego_cart.rs`.
- [ ] Router lift: measure the Colt (clampable housing length, collet nut, bit reach) and put the numbers in `build.py`; the limits test then says whether the collet clears the table at max rise Parked with the lift and the pin arm: the carving duplicator on the same shafts and blocks supplants the router table for now (the pattern plate cuts the puzzle on it, and a printed master copies to wood on it); the lift stays as the rev C and rev D models, their limits test and the measured numbers in its README.
