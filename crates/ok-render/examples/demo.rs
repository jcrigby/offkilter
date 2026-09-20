//! Renders a plate with a boss, a hole and filleted corners to PNG files
//! in the directory given, one per standard view plus a section.
use ok_render::{Options, Section, View};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut doc = ok_model::Document::new("demo");
    let tab = doc.tabs[0].id;
    let ops = serde_json::json!([
        { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": "Base" },
        { "type": "sketch", "id": 1, "op": { "type": "add_rectangle", "a": { "x": 0, "y": 0 }, "b": { "x": 60, "y": 40 } } },
        { "type": "add_extrude", "sketch": 1, "depth": 8, "name": "Plate" },
        { "type": "add_sketch", "plane": { "type": "face", "face": { "feature": 2, "local": 1, "part": 0 }, "offset": 0 }, "name": "Boss sketch" },
        { "type": "sketch", "id": 3, "op": { "type": "add_circle", "center": { "x": 30, "y": 20 }, "radius": 10 } },
        { "type": "add_extrude", "sketch": 3, "depth": 6, "op": "add", "name": "Boss" },
        { "type": "add_sketch", "plane": { "type": "face", "face": { "feature": 2, "local": 1, "part": 0 }, "offset": 0 }, "name": "Hole centres" },
        { "type": "sketch", "id": 5, "op": { "type": "add_point", "pos": { "x": 30, "y": 20 } } },
        { "type": "add_hole", "sketch": 5, "diameter": 6, "through_all": true, "name": "Hole" },
        { "type": "add_blend", "kind": "fillet", "size": 4, "edges": [
            { "a": { "feature": 2, "local": 2 }, "b": { "feature": 2, "local": 3 } },
            { "a": { "feature": 2, "local": 3 }, "b": { "feature": 2, "local": 4 } },
            { "a": { "feature": 2, "local": 4 }, "b": { "feature": 2, "local": 5 } },
            { "a": { "feature": 2, "local": 5 }, "b": { "feature": 2, "local": 2 } }
        ], "name": "Corners" }
    ]);
    let ops: Vec<serde_json::Value> = serde_json::from_value(ops).unwrap();
    let ops: Vec<ok_model::DocOp> = ops
        .into_iter()
        .map(|op| {
            serde_json::from_value(serde_json::json!({ "type": "studio", "tab": tab.0, "op": op }))
                .unwrap()
        })
        .collect();
    doc.apply_all(ops).unwrap();
    for (name, view, section) in [
        ("iso", View::Iso, None),
        ("top", View::Top, None),
        ("front", View::Front, None),
        ("section", View::Iso, Some(Section::parse("y:20").unwrap())),
    ] {
        let png = ok_render::screenshot(
            &mut doc,
            tab,
            &Options {
                view,
                section,
                ..Options::default()
            },
        )
        .unwrap();
        let path = format!("{dir}/{name}.png");
        std::fs::write(&path, &png).unwrap();
        println!("{path}: {} bytes", png.len());
    }
}
