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
| `export {format, path, view?, hidden?, body?, views?, sheet?, parts?, note?, doc?, tab?}` | Writes STL or STEP of a tab's bodies (or of the one `body`, by name or index, for a part to print); or (`format: "dxf"`) a DXF of their visible edges seen from `view` (`top` by default, the names `screenshot` takes) at 1:1 in millimetres, hidden lines dashed on their own layer with `hidden: true`: a template to print or a profile to cut; or (`format: "pdf"`) a shop drawing sheet: the `views` (front, top, right, iso by default; `section` cuts parallel to the front view and `section-side` parallel to the right view, each with `@<mm>` for where the cut goes, through the middle otherwise; cut faces hatched, captioned SECTION A-A, the cutting plane traced with arrows on the top view) laid out third angle on `sheet` (A4, A3, A2, Letter, Tabloid) at the largest standard scale that fits, with overall dimensions; a sheet of one part has hidden lines dashed and diameter callouts for holes seen end-on; an assembly's has a balloon per item and a parts list (`parts: false` to leave them off), a sub-assembly instance being one item that gets its own sheet from its own tab; `hidden` forces hidden lines on or off; `explode` (0 to about 1.5) slides the isometric's parts away from their common centre by that times their own offset, so a stacked assembly shows its parts, the balloons following; `note` is the title block's second line. |
| `document_url {doc?}` | Where to look. |
| `range_of_motion {positions, mate?, view?, format?, path?, width?, height?, sheet?, note?, doc?, tab?}` | A mechanism drawn at several positions of its mates: the assembly resolved with the mates set to each position and drawn side by side, a lift at lowest, working and highest, an arm level, half up and fully up. `positions` is a list: a bare number sets `mate` (degrees for a revolute, cylindrical or ball mate, millimetres for a slider or planar one), or an object of mate name to value (a number, or `{angle, offset}`) sets several mates at once, with `label` for its caption. `view` front (default), top, right, iso or `x,y,z`. `format` `png` (default) returns one strip with every frame fitted to the same box so the fixed parts stay put; `pdf` writes a sheet with the view at each position captioned. The positions are absolute values of the mates, which the tab's report lists. |
| `duplicator_check {bit, reach?, shape?, body?, pitch?, path?, doc?, tab?}` | Whether a body can be copied on a duplicator (a pilot tracing it from above, a bit of the same shape cutting beside it), for a master meant to be printed and copied in wood: the body's top surface sampled from above as a height field `pitch` apart (a quarter of the bit unless given), the bit rolled over it (a morphological closing with its floor, a disc for `shape` flat or a hemisphere for ball) to get the surface the bit can leave, and the findings: material under overhangs the pilot never sees (the copy's volume less the master's), concave corners tighter than the bit (where the bit's surface stands above the master's, with the area and the height left), and depths beyond `reach` (how far below the body's highest point the bit can cut, three diameters unless given). `body` names one body of a tab that has several, or its index. Returns a picture of the master from above (light where high, red where the bit cannot reach in, blue where too deep) with the findings as its caption; `path` writes it. See `examples/duplicator/`. |
| `measuring_sheet {path, sheet?}` | Writes the printable measuring sheet as a PDF: a light 10 mm grid, four bullseye marks in the corners (the origin's double-ringed, with a row of dots beside it that encodes the size) and a 100 mm bar to check the print. `sheet` is Letter (default), A4, A3, A2 or Tabloid. |
| `measure_photo {path, sheet?, reference?, out?, sketch?, doc?, tab?}` | Measures a photograph (JPEG or PNG) of parts lying on the printed sheet: finds the four marks, reads the sheet size from the dots by the origin (`sheet` is the fallback when they are not seen), squares the picture up onto the sheet's millimetres and reports every dark shape on the grid as a part with bounding box, area, centroid, outline, roundness and holes (with diameters), from the origin mark, x right, y up, the outline also fitted as edges: straight runs and arcs within 0.3 mm of the traced boundary, meeting at shared vertices, or the one circle a round part is. `reference` names a thing of known size lying on the sheet, from which the print scale is read and every size corrected: `rule` for a steel rule with millimetre graduations (its ticks are read and fitted; an inch edge is told apart by its pitch), `bars 10` for a photo scale with alternating 10 mm black and white bars (an ABFO No. 2), `disc 24.26` or a US coin by name (`quarter`, `nickel`, `dime`, `penny`). With `rule` the sheet is optional: a flatbed scan of parts and a rule on the glass, no sheet, is measured in the picture's own frame (origin at its bottom-left, x right, y up) at the scale the rule gives, which is the sharper way for flat parts. The resolution the file claims (a scanner's dpi) is read as a cross-check, and the reading warns when it disagrees with the rule or the marks, and when a disc reference comes out wider than tall, which says the print was stretched one way. Returns the squared-up picture with the grid, the parts and the reference drawn on it (`out` writes it too); `sketch: true` adds a sketch of the fitted edges (lines, arcs, circles) and holes on the tab, ready to extrude. See `examples/measuring-sheet/`. |

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
