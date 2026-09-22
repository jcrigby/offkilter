# offkilter — notes for AI assistants

Open source parametric CAD (Onshape-like). Rust kernel compiled to wasm,
TypeScript web client. Read `docs/ARCHITECTURE.md` before changing the
kernel, `docs/ROADMAP.md` to see what is planned and what is next, and
`docs/DECISIONS.md` before changing something that looks odd: it records
why, and which alternative lost. Add to it when you make such a choice.
`docs/HISTORY.md` is the project's timeline in wall-clock hours.

## Commands

- Kernel tests: `cargo test --workspace`
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

`rust-toolchain.toml` pins the compiler and the wasm target; rustup
installs both on the first `cargo` command. Everything else:

```sh
cargo install wasm-bindgen-cli --version 0.2.128 --locked   # must match crates/ok-wasm/Cargo.toml
cd apps/web && npm ci && npx playwright install --with-deps chromium && cd ../..
cargo build --release -p ok-mcp        # the MCP server .mcp.json points at
```

Or skip the installs: `Dockerfile.dev` builds an image with all of the
above pinned, and its header comment has the `docker run` line that
mounts the checkout, the cargo cache and `~/.claude`.

`.claude/settings.json` (checked in) allows the build, test and git
commands above without prompting and installs a Stop hook that refuses
to end a turn while the checkout has uncommitted or unpushed work.

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
- Before pushing, run the whole chain, not just the tests that changed:
  `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `./scripts/build-wasm.sh`, then in `apps/web`
  `npm run typecheck && npm run build`, and the browser suite against a
  debug server (`cargo build -p ok-server`, start it as above on port
  8080, `PW_CHROMIUM=<chromium> npx playwright test`).
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
