//! Renders blend corner cases to PNG files in the directory given: a
//! block with three chamfers at one corner and three fillets at
//! another, and a cylinder cut by an inclined plane with its rim
//! filleted (a variable-dihedral chain).
use ok_render::{Options, View};

fn apply(doc: &mut ok_model::Document, tab: u32, ops: serde_json::Value) {
    let ops: Vec<serde_json::Value> = serde_json::from_value(ops).unwrap();
    let ops: Vec<ok_model::DocOp> = ops
        .into_iter()
        .map(|op| {
            serde_json::from_value(serde_json::json!({ "type": "studio", "tab": tab, "op": op }))
                .unwrap()
        })
        .collect();
    let results = doc.apply_all(ops);
    if let Err(e) = &results {
        eprintln!("apply: {e}");
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut doc = ok_model::Document::new("corners");
    let tab = doc.tabs[0].id;
    apply(
        &mut doc,
        tab.0,
        serde_json::json!([
            { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": null },
            { "type": "sketch", "id": 1, "op": { "type": "add_rectangle", "a": { "x": 0, "y": 0 }, "b": { "x": 30, "y": 20 } } },
            { "type": "add_extrude", "sketch": 1, "depth": 15, "name": null },
            { "type": "add_blend", "kind": "chamfer", "size": 4, "edges": [
                { "a": { "feature": 2, "local": 1 }, "b": { "feature": 2, "local": 2 } },
                { "a": { "feature": 2, "local": 1 }, "b": { "feature": 2, "local": 5 } },
                { "a": { "feature": 2, "local": 2 }, "b": { "feature": 2, "local": 5 } }
            ], "name": "Chamfer corner" },
            { "type": "add_blend", "kind": "fillet", "size": 4, "edges": [
                { "a": { "feature": 2, "local": 1 }, "b": { "feature": 2, "local": 3 } },
                { "a": { "feature": 2, "local": 1 }, "b": { "feature": 2, "local": 4 } },
                { "a": { "feature": 2, "local": 3 }, "b": { "feature": 2, "local": 4 } }
            ], "name": "Fillet corner" }
        ]),
    );
    let report = doc.describe(tab).unwrap();
    for e in &report.errors {
        eprintln!("block: {e}");
    }
    let png = ok_render::screenshot(
        &mut doc,
        tab,
        &Options {
            view: View::Direction(ok_math::Vec3::new(-0.5, -0.6, 0.6)),
            ..Options::default()
        },
    )
    .unwrap();
    std::fs::write(format!("{dir}/block-corners.png"), png).unwrap();

    let mut doc = ok_model::Document::new("rim");
    let tab = doc.tabs[0].id;
    apply(
        &mut doc,
        tab.0,
        serde_json::json!([
            { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": null },
            { "type": "sketch", "id": 1, "op": { "type": "add_circle", "center": { "x": 0, "y": 0 }, "radius": 10 } },
            { "type": "add_extrude", "sketch": 1, "depth": 30, "name": null },
            { "type": "add_sketch", "plane": { "type": "rotated", "base": "top", "axis": "x", "angle": 30, "offset": 20 }, "name": null },
            { "type": "sketch", "id": 3, "op": { "type": "add_rectangle", "a": { "x": -40, "y": -40 }, "b": { "x": 40, "y": 40 } } },
            { "type": "add_extrude", "sketch": 3, "depth": 40, "op": "remove", "name": null }
        ]),
    );
    let report = doc.describe(tab).unwrap();
    for e in &report.errors {
        eprintln!("rim: {e}");
    }
    // The rim: the edge between the cylinder wall (feature 2 local 2, any facet) and the cut face.
    let cut_face = report.bodies[0]
        .faces
        .iter()
        .find(|f| f.reference.feature.0 == 4 && f.surface == "plane")
        .map(|f| f.reference)
        .unwrap();
    let wall = report.bodies[0]
        .faces
        .iter()
        .find(|f| f.surface == "cylinder")
        .map(|f| f.reference)
        .unwrap();
    apply(
        &mut doc,
        tab.0,
        serde_json::json!([
            { "type": "add_blend", "kind": "fillet", "size": 3, "edges": [ { "a": wall, "b": cut_face } ], "name": "Rim" }
        ]),
    );
    let report = doc.describe(tab).unwrap();
    for e in &report.errors {
        eprintln!("rim fillet: {e}");
    }
    let png = ok_render::screenshot(
        &mut doc,
        tab,
        &Options {
            view: View::Direction(ok_math::Vec3::new(0.3, -0.8, 0.5)),
            ..Options::default()
        },
    )
    .unwrap();
    std::fs::write(format!("{dir}/rim.png"), png).unwrap();
}
