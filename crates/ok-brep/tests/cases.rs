//! Boolean operands that once produced open solids, kept as regression
//! cases. Each `<name>-<op>-body.json` / `-tool.json` pair in `tests/cases`
//! is replayed and the result must be closed with a plausible volume.
//! (`OK_FUZZ_DUMP` in `fuzz.rs` writes new ones in this format.)

use ok_brep::{boolean, BoolOp, Solid};

#[test]
fn recorded_cases_stay_closed() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut stems: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix("-body.json").map(str::to_string)
        })
        .collect();
    stems.sort();
    assert!(!stems.is_empty());
    for stem in stems {
        let read = |suffix: &str| -> Solid {
            let path = dir.join(format!("{stem}-{suffix}.json"));
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
        };
        let (a, b) = (read("body"), read("tool"));
        let op = match stem.rsplit('-').next().unwrap() {
            "union" => BoolOp::Union,
            "difference" => BoolOp::Difference,
            "intersection" => BoolOp::Intersection,
            other => panic!("{stem}: unknown op {other}"),
        };
        let r = boolean(&a, &b, op).unwrap_or_else(|e| panic!("{stem}: {e}"));
        r.validate().unwrap_or_else(|e| panic!("{stem}: {e}"));
        let (va, vb, v) = (a.volume(), b.volume(), r.volume());
        let (lo, hi) = match op {
            BoolOp::Union => (va.max(vb), va + vb),
            BoolOp::Difference => ((va - vb).max(0.0), va),
            BoolOp::Intersection => (0.0, va.min(vb)),
        };
        let tol = 0.01 * (va + vb) + 1e-9;
        assert!(
            v >= lo - tol && v <= hi + tol,
            "{stem}: volume {v} outside [{lo}, {hi}]"
        );
    }
}
