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

Claude Code, from the repository (or anywhere, with an absolute path):

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
| `apply {ops, doc?, tab?}` | Applies ops in order. Bare studio, sketch and assembly ops are wrapped for the tab. Stops at the first failure (earlier ops stay), returns the ids each op made, then the tab's report so mistakes show at once. |
| `report {doc?, tab?, detail?}` | Features with ids, kinds and errors; sketches with solver status, degrees of freedom, closed regions and entity ids; bodies with volume, bounds and every face's reference (`{feature, local, part}`), plus cylinders (holes and bosses) with their axes. `detail: "full"` returns the raw JSON. |
| `screenshot {view?, section?, width?, height?, path?, doc?, tab?}` | A PNG of the tab's bodies, rendered without a browser: `view` is `top`, `front`, `right`, `iso` (default) or an `x,y,z` eye direction; `section` is `axis:offset[:flip]` (`z:10` keeps z ≥ 10, cut faces hatched); 640×480 unless sized; `path` also writes the file. Returned as MCP image content, so a model that can see images checks its work. |
| `import {path, name?, doc?, tab?}` | Adds the bodies of an STL, OBJ or STEP file as mesh bodies (STEP faceted: planes and cylinders, millimetres). |
| `export {format, path, doc?, tab?}` | Writes STL or STEP. |
| `document_url {doc?}` | Where to look. |

The same functions are plain HTTP for scripts that are not models:
`POST /api/docs/:id/ops {ops: [...]}`, `GET /api/docs/:id/report?tab=N` and
`GET /api/docs/:id/screenshot?tab=N&view=iso&section=z:10&width=800&height=600`.

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
