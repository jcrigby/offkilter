# Contributing

Thanks for looking. This is an early-stage project; the best contributions
right now are ones that push the kernel forward (see `docs/ROADMAP.md`) or
make the client more usable.

## Ground rules

- **Kernel code is pure.** Crates under `crates/` must not depend on the
  browser, the filesystem or the network. Everything must compile for
  `wasm32-unknown-unknown` and run natively for tests.
- **`f64` everywhere in geometry.** Only convert to `f32` when producing
  display meshes.
- **Never compare floats exactly.** Use `ok_math::tol`.
- **Every edit is an `Op`.** Do not add methods that mutate a `PartStudio`
  outside `ok_model::ops`. The op log is the future basis for undo, history
  and collaboration.
- **Tests for geometry.** Solver and topology changes need unit tests with
  numeric expectations (areas, volumes, positions), not just "doesn't panic".

## Before you push

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/build-wasm.sh
cd apps/web && npm run build
npm run e2e        # browser tests against the build (needs Playwright's Chromium: npx playwright install chromium)
```

CI runs the same steps, plus the end-to-end suite against a running
`ok-server`. Set `PW_CHROMIUM=/path/to/chromium` to use a pre-installed
browser.

## Style

Rust: default `rustfmt`, clippy clean. TypeScript: strict mode, no `any`
outside the wasm boundary. Keep modules small and documented at the top.
