# offkilter — notes for AI assistants

Open source parametric CAD (Onshape-like). Rust kernel compiled to wasm,
TypeScript web client. Read `docs/ARCHITECTURE.md` before changing the
kernel, `docs/ROADMAP.md` to see what is planned and what is next, and
`docs/DECISIONS.md` before changing something that looks odd: it records
why, and which alternative lost. Add to it when you make such a choice.
`docs/HISTORY.md` is the project's timeline in wall-clock hours.
`examples/README.md` is the checklist for adding an example and a
summary of the `okmcp.py` client the build scripts use.

To print a part on the owner's 3D printer, open a GitHub issue as
`docs/PRINTING.md` describes; ask the owner whether the plate is clear
first, every time.

## Commands

- Kernel tests: `cargo nextest run --workspace` (the test binaries in
  parallel; `cargo test --workspace` gives the same answer in twice the
  time, one binary after another), plus `cargo test --workspace --doc`
  for the doc tests, which nextest does not run
- Lint: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
- Build wasm into the web app: `./scripts/build-wasm.sh` (needs
  `wasm-bindgen-cli` 0.2.128 and the `wasm32-unknown-unknown` target)
- Web: `cd apps/web && npm install && npm run build` (or `npm run dev`)

## Rules

- Kernel crates (`crates/*`) are pure Rust: no I/O, no browser, must build
  for wasm32. Geometry is `f64`; compare with `ok_math::tol`, never `==`.
- All document mutation goes through `ok_model::Op` and `PartStudio::apply`.
  Add new edits as new `Op` variants, not new mutating methods.
- Keep the wasm API narrow (JSON in/out + typed arrays). Add TypeScript
  types in `apps/web/src/kernel.ts` when `Op`/summary shapes change.
- Geometry changes need numeric tests (areas, volumes, DOF counts).
- Solids are only ever built through `ok_brep::Solid::from_polygons`, which
  validates closure. Never construct a `Solid` by hand or skip validation.
- `apps/web/src/wasm/` is generated and git-ignored; never edit it.

## Setting up a fresh machine

```sh
scripts/setup.sh
```

It installs, in order and skipping what is already there: rustfmt and
clippy for the pinned toolchain (`rust-toolchain.toml` pins the
compiler and the wasm target, which rustup installs on the first
`cargo` command), `wasm-bindgen-cli` at the version
`crates/ok-wasm/Cargo.toml` pins, the web app's npm packages and
Playwright's Chromium, and the release `ok-mcp` binary that `.mcp.json`
points at. It prints each step and how long it took. A cloud
environment's setup script is that one line too, and `Dockerfile.dev`
builds an image that has run it (its header comment has the
`docker run` line that mounts the checkout, the cargo cache and
`~/.claude`).

`.claude/settings.json` (checked in) allows the build, test and git
commands above without prompting and installs two hooks: a SessionStart
hook that builds `target/release/ok-mcp` when it is missing or older
than `crates/ok-mcp` and `crates/ok-model`, so the MCP server connects
on the first prompt (if a session still opens with "offkilter failed to
connect", `/mcp` reconnects it once the build is done), and a Stop hook
that refuses to end a turn while the checkout has uncommitted or
unpushed work.

Node 22 and Python 3 are assumed. The MCP server in `.mcp.json` talks to
a document server at `$OFFKILTER_URL` (default `http://localhost:8080`);
start one with `cargo run -p ok-server -- --static apps/web/dist --data
./data --port 8080` after `npm run build`, or run `ok-mcp --file
<doc.okpart>` to work on a file without a server. The example builds
(`python3 examples/*/build.py`) drive the release `ok-mcp` binary.

## How work lands

- One increment per branch off `main`, one pull request per branch,
  merged once the three CI jobs (Kernel, Web, End-to-end) are green.
  Commit messages say what changed and why, not which model wrote them.
- Before pushing, run `scripts/check.sh`: the whole chain the three CI
  jobs run, in one command with a table of seconds per step. It is
  `cargo fmt --all` (a file it changes fails the step: commit it),
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run --workspace` and `cargo test --workspace --doc`,
  `./scripts/build-wasm.sh`, `npm run typecheck` and `npm run build` in
  `apps/web`, then `cargo build -p ok-server`, a debug server it starts
  itself on port 8080, and the browser suite (`PW_CHROMIUM=<chromium>`
  to use an installed browser). Measured on a 4-core cloud container,
  warm: 10 minutes, of which the tests 4.7 (381 tests over 54 binaries,
  run in parallel; the floor is the longest single test, the mushroom
  merge scan at nearly 3), the browser suite 3 and the wasm build 1.6;
  cold adds the compile, about 4 minutes, after `scripts/setup.sh`'s
  own 3.5. Start it before writing the commit message, not after. The
  Stop hook holds the turn until the push lands, which is a reason to
  start the chain early, not to push first and fix up after.
- The one change that may stop early: an example and its regression
  test (files under `examples/` and `crates/ok-render/tests/` only)
  cannot affect the kernel, the wasm, the web app or the browser suite,
  so it may push after `scripts/check.sh --quick` (fmt, clippy and
  `cargo nextest run -p ok-render`; 90 seconds warm), leaving the web and
  browser jobs to CI. The measuring sheet and the turntable scan are
  tested from `ok-photo` and the cart's parts list from `ok-model`:
  `--quick=ok-photo`. Anything else under `crates/` or `apps/web` runs
  the whole chain.
- Kernel, wasm or MCP changes that alter what the examples produce mean
  regenerating them: `cargo build --release -p ok-mcp`, then
  `python3 examples/router-lift/build.py` and
  `python3 examples/puzzle-top/build.py`. Their outputs under `out/` are
  committed and the tests in `crates/ok-render/tests/` read them, so a
  stale document fails CI. Look at the regenerated screenshots and PDFs;
  the tests check numbers, not whether a drawing reads well.
- New kernel behaviour gets a numeric test in its crate and, when an
  example exercises it, an assertion in that example's regression test.
- When an `Op`, a summary shape or the MCP tool surface changes, update
  `apps/web/src/kernel.ts`, `docs/OPS.md` and `docs/MCP.md` in the same
  change, and tick or add the line in `docs/ROADMAP.md`.
- Sub-assemblies: a connector on a sub-assembly instance names its member
  (`sub`); the client sets it from the picked body and scripts read the
  member ids from the sub-assembly's own report.
