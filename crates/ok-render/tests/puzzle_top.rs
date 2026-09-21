//! The checkerboard puzzle example (examples/puzzle-top) as a regression
//! suite: the document its build script wrote through the MCP server is
//! regenerated, every piece must be a closed solid in its own local
//! range, the web must fill exactly the gaps, and the plan view must be
//! a DXF of arcs and lines.

use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/puzzle-top")
}

#[test]
fn the_puzzle_regenerates_into_pieces_and_a_web() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/puzzle_top.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let tab = doc.tabs[0].id;
    let start = std::time::Instant::now();
    let r = doc.regenerate_studio(tab, None).unwrap();
    let elapsed = start.elapsed();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(r.bodies.len(), 8 * 6 + 1, "{} bodies", r.bodies.len());
    let feature = match &doc.tabs[0].kind {
        ok_model::TabKind::PartStudio(ps) => match &ps.features()[0].kind {
            ok_model::FeatureKind::Puzzle(p) => p.clone(),
            other => panic!("{other:?}"),
        },
        _ => unreachable!(),
    };
    let (mut light, mut dark) = (0, 0);
    let mut piece_area = 0.0;
    for (k, b) in r.bodies[..48].iter().enumerate() {
        b.solid
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", b.name));
        let (col, row) = (k % 8 + 1, k / 8 + 1);
        let colour = if (col + row) % 2 == 0 {
            "light"
        } else {
            "dark"
        };
        assert_eq!(b.name, format!("Piece {col},{row} {colour}"));
        if colour == "light" {
            light += 1;
        } else {
            dark += 1;
        }
        piece_area += b.solid.volume() / feature.thickness;
        assert!(
            b.solid
                .faces
                .iter()
                .all(|f| f.origin.local >> 12 == k as u32 + 1),
            "{}: faces in another piece's range",
            b.name
        );
    }
    assert_eq!((light, dark), (24, 24));
    let web = &r.bodies[48];
    assert_eq!(web.name, "Alignment web");
    web.solid.validate().unwrap();
    let board = 8.0 * 6.0 * feature.pitch * feature.pitch;
    let web_area = web.solid.volume() / feature.web;
    assert!(
        (piece_area + web_area - board).abs() < board * 2e-3,
        "pieces {piece_area} + web {web_area} vs board {board}"
    );
    // The plan view as routing templates: arcs stay arcs.
    let dxf = ok_render::view_dxf(&mut doc, tab, ok_render::View::Top, false).unwrap();
    let arcs = dxf.matches("\nARC\n").count();
    let lines = dxf.matches("\nLINE\n").count();
    assert!(arcs >= 3 * 82 * 2, "{arcs} arcs for 82 tabs");
    assert!(lines >= 48 * 4, "{lines} lines");
    assert!(!dxf.contains("\nELLIPSE\n"), "no ellipses in a plan view");
    eprintln!("8x6 puzzle with web regenerated in {elapsed:?}");
    assert!(elapsed.as_secs() < 20, "{elapsed:?}");
}

/// The tight version: bands a bit apart, no web, a tray with a pocket
/// per piece, and the notes empty because nothing gets rounded.
#[test]
fn the_tight_top_has_bands_and_a_printable_fixture() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/puzzle_top.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let tab = doc
        .tabs
        .iter()
        .find(|t| t.name() == "Tight top")
        .unwrap()
        .id;
    let start = std::time::Instant::now();
    let r = doc.regenerate_studio(tab, None).unwrap();
    let elapsed = start.elapsed();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(r.bodies.len(), 8 * 6 + 1, "{} bodies", r.bodies.len());
    let feature = match &doc.tabs.iter().find(|t| t.id == tab).unwrap().kind {
        ok_model::TabKind::PartStudio(ps) => match &ps.features()[0].kind {
            ok_model::FeatureKind::Puzzle(p) => p.clone(),
            other => panic!("{other:?}"),
        },
        _ => unreachable!(),
    };
    assert!(feature.gap == 0.0 && feature.row_gap >= feature.bit);
    let plan = ok_sketch::jigsaw::plan(&feature.params()).unwrap();
    assert!(
        plan.problems.is_empty() && plan.notes.is_empty(),
        "{plan:?}"
    );
    // The pieces add up to the bands (the strips keep their width even
    // where the corners wander, so the strips are exactly the rest).
    let bands = 8.0 * 6.0 * feature.pitch * feature.pitch * feature.thickness;
    let v: f64 = r.bodies[..48].iter().map(|b| b.solid.volume()).sum();
    assert!((v - bands).abs() < bands * 2e-3, "{v} vs {bands}");
    // No piece owns a tab on its top or bottom: every one's top and
    // bottom edges are single lines.
    for piece in &plan.pieces {
        assert!(matches!(
            piece.outline[0],
            ok_sketch::jigsaw::Seg::Line { .. }
        ));
    }
    let tray = &r.bodies[48];
    assert_eq!(tray.name, "Printing fixture");
    tray.solid.validate().unwrap();
    let (lo, hi) = tray.solid.bounds().unwrap();
    assert!(
        lo.z < 0.0 && (hi.z - feature.fixture).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    assert!(lo.x < 0.0 && hi.x > 8.0 * feature.pitch, "{lo:?} {hi:?}");
    eprintln!("8x6 tight top with fixture regenerated in {elapsed:?}");
}
