# The examples: whole projects built through the MCP server

Each directory here is a thing someone wanted, built the way a language
model builds it: a `build.py` that drives the release `ok-mcp` binary
over stdio through `okmcp.py`, writes a document and its outputs into
`out/`, and a regression test in `crates/ok-render/tests/` that reads
them back and checks the numbers. The README at the root lists them;
this file is how to add one.

```sh
cargo build --release -p ok-mcp              # or scripts/setup.sh
python3 examples/<name>/build.py             # the document, STLs, pictures, sheets, numbers
cargo test -p ok-render --test <name>        # what CI runs
```

## Checklist for a new example

- [ ] `examples/<name>/build.py`: the constants at the top, one line
  each with a comment saying where the number came from, grouped by
  what they describe, then the derived ones; the parts as functions
  taking a `Part`; `main()` making the document, printing each tab's
  bodies and volumes, taking the pictures, exporting, and writing a
  numbers JSON for the test. Starts with a docstring saying what the
  thing is, the three commands above, and what it writes.
- [ ] `examples/<name>/README.md`: what it is and why, the three
  commands, a picture, a **Measure these first** table (what, the
  number in the model, how to check it on the real thing) when any
  number is a guess, a parts table (part, size, what it does), and where
  the numbers came from.
- [ ] `examples/<name>/out/`, committed: `<name>.okpart` (the
  document), the STLs to print (or the DXF to cut, the PDF sheet), the
  PNGs the README shows, and `<name>.json` (or a name that says what it
  holds) with every number the test needs. Big or regenerable files
  (STEP, section sweeps) go in `.gitignore` as the router lift's do.
- [ ] `crates/ok-render/tests/<name>.rs`: the regression test, shape
  below. It reads the committed document and JSON, so a kernel change
  that alters what the script produces means running the script again
  and committing `out/`.
- [ ] A line in `docs/ROADMAP.md` (under "Next", `- [x]`, one sentence
  on what it is and what the test checks) and the mention in the root
  README's examples list.
- [ ] If the example needed something the kernel or `ok-mcp` did not
  have, that change comes first, with its own numeric test; the
  example's test then asserts the new behaviour too.

## `okmcp.py` in ten lines

```python
sys.path.insert(0, os.path.dirname(HERE)); from okmcp import Mcp, Part, new_tab, v2
mcp = Mcp(path)                                   # ok-mcp --file path, initialised
mcp.call("create_document", {"name": "thing"})     # tab 1 is a part studio
p = Part(mcp, 1, "body")                           # one tab: ops go through p.apply, ids come back
s = p.sketch("top", 10.0, "base")                  # standard plane + offset (docs/OPS.md has the frames)
p.rect(s, (0, 0), (60, 40)); p.circle(s, (30, 20), 8); p.polygon(s, pts); p.slot(s, a, b, w); p.point(s, c)
p.extrude(s, 20.0, op="new", name="block")        # or op="add" | "remove" | "intersect"; direction, profiles, through_all
p.cut(s, 5.0); p.hole(s, 6.5, depth=0.0); p.revolve_cut(s)   # a cut is an extrude that removes; a hole drills the sketch's points
p.feature({"type": "add_loft", "sketch": a, "sketch_b": b, "op": "add"})   # any other op, as docs/OPS.md writes it
t = new_tab(mcp, {"type": "add_part_studio", "name": "gauge"}); g = Part(mcp, t, "gauge")   # another tab
p.bodies(); p.report(); p.screenshot(path, view="iso", section="y:50"); p.export(path, "stl")   # read back, picture, write
```

Also on `Part`: `sketch_on(face, name)` for a sketch on a body's face
(the face reference from a report), `hexagon`, `gear`, `split`,
`plane_point(sketch, world_point)` for a world point in a sketch's
coordinates, and `export(path, "dxf", view="top")` or `"pdf"`. For
assemblies, the module-level `apply(mcp, ops, tab)`, `full(mcp, tab)`,
`cylinder(body, r)` and `mate_parameters(target, moving)` do what the
router lift's assembly needs; `screenshot` and `export` take any
argument `docs/MCP.md` lists through `mcp.call` directly, as the
scripts do for sheets with sections and notes.

`Part.apply` raises on the first `ERROR:` in the reply with the part's
name, so a script that fails says which op; the ids of features and
sketch entities come back parsed from the text (`new_tab` parses
`tab N` the same way).

## The shape of a regression test

```rust
//! What the test checks, in a sentence per claim.
use ok_brep::{BoolOp, Solid};
use ok_model::{Document, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/<name>") }
fn load() -> Document { Document::from_json(&std::fs::read_to_string(dir().join("out/<name>.okpart")).unwrap()).unwrap() }
fn numbers() -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(dir().join("out/<name>.json")).unwrap()).unwrap() }
fn tab_named(doc: &Document, name: &str) -> TabId { doc.tabs.iter().find(|t| t.name() == name).unwrap().id }

fn body_of(doc: &mut Document, tab: &str) -> Solid {
    let r = doc.regenerate_studio(tab_named(doc, tab), None).unwrap();   // regenerate, do not trust the file's bodies
    assert!(r.statuses.iter().all(|f| f.error.is_none()), "{:?}", r.statuses);
    assert_eq!(r.bodies.len(), 1, "one body in {tab}");
    let body = r.bodies[0].solid.clone();
    body.validate().unwrap();                                            // closed, consistent
    body
}

#[test]
fn the_thing_is_the_size_it_says() {
    let (mut doc, n) = (load(), numbers());
    let body = body_of(&mut doc, "adapter");
    let (lo, hi) = body.bounds().unwrap();
    assert!((hi.x - lo.x - n["adapter"]["stub_l"].as_f64().unwrap()).abs() < 0.01);
    // sections: intersect with a slab or a post built from ok_brep::extrude and compare volumes
    let probe = ok_brep::boolean(&body, &slab(..), BoolOp::Intersection).map(|s| s.volume()).unwrap_or(0.0);
    assert!(probe > 0.0, "the wall is there");
}
```

The tests check numbers: bounds, volumes, the volume left by a
boolean against a probe solid (a slab through a wall, a post down a
bore), cylinders found by their radius, clearances between bodies of
an assembly. They print what they measure (`cargo test ... --
--nocapture`), so a failure says the number, not just that it differed.
`lint_blower.rs` and `rack_hook.rs` are the small ones to copy;
`router_lift.rs` and `duplicator.rs` check mechanisms through their
mates.
