# Driving offkilter from a language model

`ok-mcp` is a Model Context Protocol server: a small binary that speaks
JSON-RPC over stdio and exposes an offkilter document as tools. Point a
model at it (Claude Code, Claude Desktop, or any MCP client) and it can
sketch, extrude, drill, fillet, read back what it made, and export,
while you watch in the browser and take over whenever you like.

## Two ways to run it

**With the document server** (what you want when you also have the
browser open):

```sh
cargo build --release -p ok-server -p ok-mcp
./target/release/ok-server --static apps/web/dist --data ./data --port 8080 &
./target/release/ok-mcp --server http://localhost:8080
```

Every op the model applies goes through the live document, so a
document open in the browser updates as the model works, and the model's
`report` tool sees your edits too. If the server has accounts, add
`--login NAME:PASSWORD` or `--cookie ok_session=...` so the model edits
as you. `--doc ID` starts with a document already current.

**On a local file** (no server, nothing to watch):

```sh
./target/release/ok-mcp --file bracket.okpart
```

The file is written after every `apply`; open it in the web app later
or export from the model side.

## Registering it

The repository's `.mcp.json` registers `ok-mcp` for Claude Code sessions
started in it: the release binary, against the server at
`OFFKILTER_URL` or `http://localhost:8080`. Build it first with
`cargo build --release -p ok-mcp`. The README's "Driving it from your
phone" section puts that together with Remote Control.

Claude Code elsewhere (or anywhere, with an absolute path):

```sh
claude mcp add offkilter -- /path/to/offkilter/target/release/ok-mcp --server http://localhost:8080
```

Claude Desktop and other clients take the same command in their MCP
server configuration:

```json
{ "mcpServers": { "offkilter": { "command": "/path/to/ok-mcp", "args": ["--server", "http://localhost:8080"] } } }
```

## The tools

| tool | what it does |
|------|--------------|
| `offkilter_reference` | The op reference (docs/OPS.md): envelopes, every feature and sketch op, how faces and edges are referenced. The model should read it first; the server's `instructions` say so. |
| `create_document {name}` | A new document with one part studio (tab 1); becomes current. Returns the browser URL on a server. |
| `open_document {doc}` / `list_documents` | Work on an existing document. |
| `apply {ops, doc?, tab?}` | Applies ops in order. Bare studio, sketch and assembly ops are wrapped for the tab, and face references written as `{feature, local, part}` get the piece's neighbours attached from the report so they follow the piece through later edits. Stops at the first failure (earlier ops stay), returns the ids each op made, then the tab's report so mistakes show at once. |
| `report {doc?, tab?, detail?}` | Features with ids, kinds and errors; sketches with solver status, degrees of freedom, closed regions and entity ids; bodies with volume, bounds and every face's reference (`{feature, local, part}`), plus cylinders (holes and bosses) with their axes; on an assembly tab, its instances and mates, each mate with its kind and current angle and offset. `detail: "full"` returns the raw JSON. |
| `screenshot {view?, section?, fit?, width?, height?, path?, doc?, tab?}` | A PNG of the tab's bodies, rendered without a browser: `view` is `top`, `front`, `right`, `iso` (default) or an `x,y,z` eye direction; `section` is `axis:offset[:flip]` (`z:10` keeps z ≥ 10, cut faces hatched); `fit` is `x0,y0,z0,x1,y1,z1` in millimetres, a box the view is fitted to instead of the bodies, so a detail of a large assembly fills the picture and the rest runs off the frame; 640×480 unless sized; `path` also writes the file. Returned as MCP image content, so a model that can see images checks its work. |
| `import {path, name?, doc?, tab?}` | Adds the bodies of an STL, OBJ or STEP file as mesh bodies (STEP faceted: planes and cylinders, millimetres). |
| `export {format, path, view?, hidden?, body?, views?, sheet?, parts?, note?, explode?, window?, doc?, tab?}` | Writes STL or STEP of a tab's bodies (or of the one `body`, by name or index, for a part to print); or (`format: "dxf"`) a DXF of their visible edges seen from `view` (`top` by default, the names `screenshot` takes) at 1:1 in millimetres, hidden lines dashed on their own layer with `hidden: true`: a template to print or a profile to cut; or (`format: "pdf"`) a shop drawing sheet: the `views` (front, top, right, iso by default; `section` cuts parallel to the front view and `section-side` parallel to the right view, each with `@<mm>` for where the cut goes, through the middle otherwise; cut faces hatched, captioned SECTION A-A, the cutting plane traced with arrows on the top view; and `<view> of <names>`, e.g. `right of lug` or `front of rib 1; rib 2`, a standard view of only the bodies whose names contain one of the `;`-separated texts, drawn alone and captioned) laid out third angle on `sheet` (A4, A3, A2, Letter, Tabloid) at the largest standard scale that fits, with overall dimensions; a sheet of one part has hidden lines dashed and diameter callouts for holes seen end-on; an assembly's has a balloon per item on the isometric view, or on the sheet's only standard view when it has no isometric, and a parts list (`parts: false` to leave them off), a sub-assembly instance being one item that gets its own sheet from its own tab; `hidden` forces hidden lines on or off (on, a section also shows what lies behind its cut, dashed); `window` is `x0,y0,z0,x1,y1,z1` in millimetres, a detail window every view is clipped to and framed by, so one corner of a large assembly comes out at a scale that reads; `explode` (0 to about 1.5) slides the isometric's parts away from their common centre by that times their own offset, so a stacked assembly shows its parts, the balloons following; `note` is the title block's second line. |
| `document_url {doc?}` | Where to look. |
| `range_of_motion {positions, mate?, view?, format?, path?, width?, height?, sheet?, note?, doc?, tab?}` | A mechanism drawn at several positions of its mates: the assembly resolved with the mates set to each position and drawn side by side, a lift at lowest, working and highest, an arm level, half up and fully up. `positions` is a list: a bare number sets `mate` (degrees for a revolute, cylindrical or ball mate, millimetres for a slider or planar one), or an object of mate name to value (a number, or `{angle, offset}`) sets several mates at once, with `label` for its caption. `view` front (default), top, right, iso or `x,y,z`. `format` `png` (default) returns one strip with every frame fitted to the same box so the fixed parts stay put; `pdf` writes a sheet with the view at each position captioned. The positions are absolute values of the mates, which the tab's report lists. |
| `duplicator_check {bit, reach?, shape?, body?, pitch?, path?, doc?, tab?}` | Whether a body can be copied on a duplicator (a pilot tracing it from above, a bit of the same shape cutting beside it), for a master meant to be printed and copied in wood: the body's top surface sampled from above as a height field `pitch` apart (a quarter of the bit unless given), the bit rolled over it (a morphological closing with its floor, a disc for `shape` flat or a hemisphere for ball) to get the surface the bit can leave, and the findings: material under overhangs the pilot never sees (the copy's volume less the master's), concave corners tighter than the bit (where the bit's surface stands above the master's, with the area and the height left), and depths beyond `reach` (how far below the body's highest point the bit can cut, three diameters unless given). `body` names one body of a tab that has several, or its index. Returns a picture of the master from above (light where high, red where the bit cannot reach in, blue where too deep) with the findings as its caption; `path` writes it. See `examples/duplicator/`. |
| `spectre_tile {edge, curve?, bulge?, bit?, gap?, round?}` | One Spectre tile as the pieces of a top are routed, for a master to print and copy on the duplicator: the curved tile at `edge` mm (`curve` and `bulge` as `add_spectre`'s), its four inside corners filleted to `bit` (a fillet is what a bit leaves in a notch), eroded by half of `gap`, the ten convex corners left sharp (a bit goes round them) or, with `round` (a radius in mm; half the bit gives the knobs the template-routed pieces have), all rounded to it. Returns JSON: `outline` (counter-clockwise points, mm, centred on the smallest circle round the tile), `radius` and `across` (that circle: a blank has to be at least `across`), `area` and `tile_area`. No document is needed. See `examples/spectre-cage/`. |
| `spectre_saw {edge, gap?, kerf?, margin?, floor?, bias?}` | The straight Spectre as a stick on a table saw, for a stack of blanks cut to the tile as one prism: the tile at `edge` mm eroded by half of `gap`, in a blank planed to the tile's thickness between its two parallel faces and ripped to its width (three faces made), and the ten rips that make the rest, two through and four pairs of rabbets at the inside corners, simulated in order with the blade's `kerf`. Each cut is a cassette on a sled with the blade at one height: the stick turned so the face stands vertical with the open end down (`turn`, `shift`), the blade's near face at x 0, the floor (`bed`) putting the corner at the blade's top (`top`, the same for every cut, `height`), the stock `before` and `after`, the `offcuts`, and `held`, the stock with open kerfs filled, with its `lower` and `upper` envelopes the cassette's base and lid are the negatives of. `margin` is the blank past the notched end, `floor` the thinnest bed, `bias` how far past the corner a rabbet is aimed (short leaves a sliver; past nicks the far face a kerf wide under the gap). Returns JSON with the blank (`width`, `thickness`), `cuts`, `height`, and `left` and `nicked`, the tile's area the cuts leave or take. No document is needed. See `examples/spectre-saw/`. |
| `measuring_sheet {path, sheet?, tiles?}` | Writes the printable measuring sheet as a PDF: a light 10 mm grid, four bullseye marks in the corners (the origin's double-ringed, with a row of dots beside it that encodes the size) and a 100 mm bar to check the print. `sheet` is Letter (default), A4, A3, A2 or Tabloid. With `tiles` ("2x2", "1x1"), a measuring field instead: a page per tile that cut and tape into one sheet carrying a lattice of marks 50 mm apart, each mark's dots saying where it is, so a photograph needs any four marks and parts may cover marks. |
| `measure_photo {path, sheet?, reference?, out?, sketch?, doc?, tab?}` | Measures a photograph (JPEG or PNG) of parts lying on the printed sheet: finds the four marks, reads the sheet size from the dots by the origin (`sheet` is the fallback when they are not seen), squares the picture up onto the sheet's millimetres and reports every dark shape on the grid as a part with bounding box, area, centroid, outline, roundness and holes (with diameters), from the origin mark, x right, y up, the outline also fitted as edges: straight runs and arcs within 0.3 mm of the traced boundary, meeting at shared vertices, or the one circle a round part is. `reference` names a thing of known size lying on the sheet, from which the print scale is read and every size corrected: `rule` for a steel rule with millimetre graduations (its ticks are read and fitted; an inch edge is told apart by its pitch), `bars 10` for a photo scale with alternating 10 mm black and white bars (an ABFO No. 2), `disc 24.26` or a US coin by name (`quarter`, `nickel`, `dime`, `penny`). With `rule` the sheet is optional: a flatbed scan of parts and a rule on the glass, no sheet, is measured in the picture's own frame (origin at its bottom-left, x right, y up) at the scale the rule gives, which is the sharper way for flat parts. The resolution the file claims (a scanner's dpi) is read as a cross-check, and the reading warns when it disagrees with the rule or the marks, and when a disc reference comes out wider than tall, which says the print was stretched one way. Returns the squared-up picture with the grid, the parts and the reference drawn on it (`out` writes it too); `sketch: true` adds a sketch of the fitted edges (lines, arcs, circles) and holes on the tab, ready to extrude. See `examples/measuring-sheet/`. A photograph of a field is read from whichever of its coded marks are in view, four or more in two columns and two rows, fitted together; the answer says how many read and the field millimetres covered. |
| `scan_turntable {video?, frames?, out, fps?, every?, laser_min?, bin?, min_height?}` | Scans an object standing on the measuring field on a lazy Susan, turned under a phone and a line laser fixed to each other: from a video (`video`, frames taken with ffmpeg at `fps` per second, 2 by default) or a directory of frames (`frames`, in order, every `every`th used). Each frame's marks give the camera's pose over the field, which turns with the object; the green laser line is the ridge of green over red and blue in each row, to a fraction of a pixel; on the paper the line is straight in the picture, and those pixels meet the field's plane at known points, which with the turntable's axis (the camera's centres circle about it, and the laser is aimed through it) fix the laser's plane; each pixel of the line on the object is a ray meeting that plane. Writes `scan.stl` (each frame's profile resampled into `bin` mm height bins, gaps bridged, consecutive frames stitched and capped, so it imports as a closed body), `scan.ply` (the points), `scan.json` (focal length, laser angle and plane fit, the axis, how far the line passes from it, the bounds, volume by slices, a line per frame, warnings) and `scan-frame.png` (the first frame as read) into `out`. `laser_min` (12) is the least green a pixel is ever taken as the line at, `min_height` (1.5 mm) what counts as the table. Shoot it dim with the exposure locked, the laser's vertical line through the turntable's centre and 30 to 45 degrees round from the camera, the camera a little above the object's middle, the marks in view all the way round. See `examples/turntable-scan/`. |
| `merge_scans {scans: [{dir, placed?}], out, sectors?, bin?}` | Merges turntable scans of one object taken in different poses into one cloud and mesh. A scan sees nothing under an overhang steeper than the camera's height above the table (the waist under a bulge, the underside of a lip), so the object is scanned again upside down or on its side, or with the camera higher, and the scans merged. Each `dir` holds a `scan.ply` from `scan_turntable`; `placed` says how the object was put down between that scan and the first: `same` (not touched; only the camera or laser moved), `moved` (set down again the same way up, anywhere, turned any amount), `upside down`, or `on its side` (rolled a quarter turn). Each scan after the first is registered onto the rest by point-to-plane iterative closest point from the seeds the placement gives (every 30 degrees of turn about the vertical, and for a roll every 30 degrees of the axis rolled about), the closest kept; the answer gives the fit (median mm) and the share of its points lying on the rest, and warns when either says the placement was not what was said. Writes `merged.stl` (the cloud meshed about its own centre line in `sectors` of azimuth, 36 by default, and `bin` mm height bins), `merged.ply` and `merged.json` (the registrations as transforms, bounds, volume) into `out`. The mesh takes the object to be one a horizontal ray from its centre line leaves once at every height: anything convex, most things that stand; not a torus. |
| `fit_faces {scan?, ply?, out?, tol?, angle?, min_points?, table?, doc?, tab?, name?}` | Fits flat faces to a scan's point cloud and, where they close, builds the solid they bound. The planes are taken one at a time: from the unclaimed point whose neighbourhood lies most fully on its own plane, that plane claims every unclaimed point lying on it whose normal agrees, is refitted and claims again, and keeps its points if there are enough and their normals do not fan out as a curve's would (a point's normal comes from its laser line's tangent and the step to a parallel neighbouring line, never from one line alone, since a line lies in the laser's own plane wherever it bends over an edge); then the planes' half-spaces are intersected into a convex solid: every point three planes meet at that no plane excludes is a corner, corners that fall within a millimetre (where four or more planes meet) are one corner at the point nearest all its planes, each plane's corners ordered round it are a face, and the faces must share every edge in pairs. The table's plane is supplied when the cloud reaches it and no scanned plane lies there. Reports every plane (normal, offset, points, rms) and the solid's faces, edges with dihedral angles and lengths, corners, volume and area; `out` writes `faces.json`; with `doc`, `tab` or `name` the solid goes into the tab as a body through `add_mesh`, whose regeneration merges the coplanar triangles back into one planar face per plane. A convex polyhedron scanned once closes with the table; scanned in two poses and merged it closes on its own; a curved thing fits few planes and does not close. |

The same functions are plain HTTP for scripts that are not models:
`POST /api/docs/:id/ops {ops: [...]}`, `GET /api/docs/:id/report?tab=N` and
`GET /api/docs/:id/screenshot?tab=N&view=iso&section=z:10&fit=0,0,0,50,50,50&width=800&height=600`.

## How a session goes

The model reads the reference, creates a document, adds a sketch on the
top plane, draws a rectangle, extrudes it, and reads the report. The
report names the top face as `{feature: 2, local: 1, part: 0}`, so the
next sketch goes on that face, a point in it becomes a hole, and the
side faces' pairs become fillet edges. When something fails (a sketch
that does not close, a fillet too large for its edge), the report says
which feature and why, and the model fixes that op rather than starting
over. You can open the document URL at any point, drag a dimension, add
a feature, or finish the last ten yards yourself; the model's next
`report` shows your changes, and its next `apply` builds on them.

## Limits

Ops are the whole surface, so anything the web client can do, a model
can do. Sight comes from `screenshot`: a server-side render of the
tessellation from a standard view, or sectioned to look inside. A model
that cannot take images still reasons from the report (volumes, bounds,
face normals and centroids, sketch solver state), which is usually
enough to notice a hole that landed in the wrong place. The render is
flat-shaded facets, not the browser's shaded viewport, and it does not
show sketches, dimensions or mate frames.
