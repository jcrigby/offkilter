# Fresh checkout: what to fix so the first session is not spent on setup

A to-do list for a session, this one or a fresh one, written after
building `examples/lint-blower` in a cloud container that started from
a bare clone with no chat history. Each item says what went wrong,
what to change, where, and how to know it is done. Do them in order;
the first three are the ones that cost the most. Tick a box in the
same change that does the work, and delete this file when the list is
empty.

## 1. Build the MCP server before the session needs it

- [x] **What went wrong.** The session opened with "offkilter failed
  to connect": `.mcp.json` points at `target/release/ok-mcp`, which a
  fresh clone does not have. The `import` tool (an STL as mesh bodies,
  then `report` and `screenshot` with a section) would have read the
  reference STL inside offkilter; instead the model installed
  trimesh, shapely, scipy, networkx and rtree and did it in Python.
- **Change.** A `scripts/setup.sh` that does, idempotently and in
  this order: `rustup component add rustfmt clippy`, `cargo install
  wasm-bindgen-cli --version 0.2.128 --locked` (skip when the version
  already matches), `cd apps/web && npm ci && npx playwright install
  --with-deps chromium`, `cargo build --release -p ok-mcp`. Print
  what it is doing and how long each step took. `CLAUDE.md`'s "Setting
  up a fresh machine" becomes "run `scripts/setup.sh`", `Dockerfile.dev`
  calls it, and the cloud environment's setup script is that one
  line too.
- **Also.** A `SessionStart` hook in `.claude/settings.json` that runs
  `cargo build --release -p ok-mcp` when the binary is missing or older
  than `crates/ok-mcp` and `crates/ok-model`, so the server connects on
  the first prompt; the `session-start-hook` skill knows the shape.
- **Done when.** A new cloud session's first prompt can call
  `offkilter_reference` without building anything.

## 2. Install rustfmt and clippy with the toolchain

- [x] **What went wrong.** The lint command in `CLAUDE.md` failed
  twice: `cargo fmt` and `cargo clippy` were not installed, because
  `rust-toolchain.toml` pins the channel and the wasm target but not
  the components.
- **Change.** `components = ["rustfmt", "clippy"]` in
  `rust-toolchain.toml`. rustup installs them with the toolchain on the
  first `cargo` command.
- **Done when.** `cargo fmt --version` works in a container that has
  run nothing but `cargo build`.

## 3. Say how long the pre-push chain takes and what a small change may skip

- [x] **What went wrong.** `cargo test --workspace` on a cold target
  directory ran for over half an hour, and the Stop hook refuses to end
  a turn until the push lands, so the session pushed before clippy and
  the tests finished (a fix commit was the fallback). Nothing in
  `CLAUDE.md` says the chain is slow cold or that a change confined to
  `examples/` and `crates/ok-render/tests/` cannot affect the web app.
- **Change.** In `CLAUDE.md`, under "How work lands": the chain's
  duration warm and cold (measure it: `time` each step in
  `scripts/setup.sh`'s log or a `scripts/check.sh` that runs the whole
  chain and prints a table); and a rule for the small case: an example
  and its test may push after fmt, clippy and `cargo test -p
  ok-render`, with the web and browser steps left to CI. If the
  maintainer wants the whole chain every time, say so and say why, so
  a session does not have to guess.
- **Also.** `scripts/check.sh` as the one command for the chain, so the
  instruction is "run `scripts/check.sh`" and the list of six commands
  lives in one place.
- **Done when.** `CLAUDE.md` names the command, the time it takes and
  the case that may stop early.

## 4. Document the sketch planes' frames

- [ ] **What went wrong.** `docs/OPS.md` names the three standard
  planes and their normals but not their sketch axes or which way
  `offset` goes. The session wrote a probe script to learn that a
  front sketch at offset `o` lies at world y = −o with sketch x along
  world X and sketch y along world Z, and that a right sketch at
  offset `o` lies at x = o with sketch x along Y; and that a rotated
  top plane turned about x by θ has sketch y at (0, cos θ, sin θ).
- **Change.** A table in `docs/OPS.md` under `PlaneRef`: plane,
  world position for `offset`, sketch x axis, sketch y axis; and one
  line for `rotated` giving the sign convention. `examples/okmcp.py`'s
  `Part.plane_point` already converts a world point; say so there.
- **Done when.** A revolve-cut about a vertical axis at a known y can
  be written from the docs without a probe.

## 5. A checklist for adding an example

- [ ] **What went wrong.** The conventions are consistent but learned
  by reading three examples: the constants at the top of `build.py`
  with a "measure these first" table in the README, `out/` committed
  (STL, PNG, PDF, `.okpart`, a numbers JSON), the Rust test in
  `crates/ok-render/tests/` reading that JSON, the line in
  `docs/ROADMAP.md`, the mention in the README's list, and
  `okmcp.py`'s `Part` methods (`sketch`, `rect`, `polygon`, `extrude`,
  `cut`, `revolve_cut`, `feature` for anything else, `new_tab`,
  `screenshot` with `section`, `export` with `views`).
- **Change.** `examples/README.md`: the checklist above, a ten-line
  summary of the `Part` API with one example per method, and the
  shape of a regression test (load the document, regenerate the tab,
  one body, `validate`, then numbers by section and boolean against
  the JSON). Point `CLAUDE.md` at it.
- **Done when.** A new example can be written from that file and
  `docs/OPS.md` alone.

## 6. A recipe for reading a reference STL

- [ ] **What went wrong.** The photo tooling measures only the
  project's own sheet, and nothing points at `import` for a foreign
  mesh. The Samsung adapter's socket, wedge and end were read with
  trimesh sections and matplotlib plots.
- **Change.** A section in `docs/MCP.md`, "Reading a reference
  mesh": `import` the STL into its own part studio, `report` for the
  bounds and volume, `screenshot` with `section` at a few offsets
  along each axis to see the walls, and `export` of a DXF of a view
  for the outline. If `report` cannot give a section's area or the
  wall thicknesses, add that to the roadmap: a `section` tool that
  returns the loops at a plane as numbers is what a model wants.
- **Done when.** The lint-blower README's "Where the numbers came
  from" table could have been filled from offkilter alone.

## 7. Smaller things noticed on the way

- [ ] `okmcp.Part.apply` parses `tab N)` out of `add_part_studio`'s
  text; `new_tab` exists for it but the build scripts' docstrings do
  not mention it. Say so in `okmcp.py`'s header.
- [ ] The `screenshot` tool's `section` keeps the side at or above
  the offset and `:flip` the other; the first guess was the other way.
  One example in `docs/MCP.md` with a picture would settle it.
- [ ] Python packages for the examples that measure from photos
  (`examples/finch-plug/trace/`, `examples/measuring-sheet/`) are not
  listed anywhere; a `requirements.txt` next to them, or a line in
  `scripts/setup.sh`.
