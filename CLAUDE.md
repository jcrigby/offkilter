# offkilter — notes for AI assistants

Open source parametric CAD (Onshape-like). Rust kernel compiled to wasm,
TypeScript web client. Read `docs/ARCHITECTURE.md` before changing the
kernel and `docs/ROADMAP.md` to see what is planned.

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
- `apps/web/src/wasm/` is generated and git-ignored; never edit it.
