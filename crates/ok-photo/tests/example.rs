//! Makes and checks `examples/measuring-sheet/`: a synthetic photograph of
//! the Letter sheet with parts and a photo scale on it (`scan.png`, what a
//! phone would take, askew, of a print that came out at 97 %), and the
//! measurement of it (`out/measured.png`). Both are committed; this test
//! regenerates them so they never go stale.

use ok_photo::{measure_image, photograph, scan_image, sheet_image, Reference, Scene, SheetSize};

fn example_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/measuring-sheet")
        .canonicalize()
        .unwrap()
}

#[test]
fn the_example_scan_measures_its_plate_and_disc() {
    let dir = example_dir();
    // A 64 x 38 plate at (30, 30) with two 6.5 mm holes 50 mm apart, a
    // 25 mm washer with a 10 mm bore at (170, 110), an 18 mm disc at
    // (110, 120), and a photo scale with five 10 mm bars along the top,
    // on a Letter sheet printed at 97 %, at 8 px/mm.
    let scene = Scene {
        rects: vec![[30.0, 30.0, 94.0, 68.0]],
        discs: vec![[170.0, 110.0, 12.5], [110.0, 120.0, 9.0]],
        holes: vec![[37.0, 49.0, 3.25], [87.0, 49.0, 3.25], [170.0, 110.0, 5.0]],
        print: 0.97,
        ..Scene::default()
    }
    .bars(40.0, 140.0, 10.0, 5);
    let sheet = sheet_image(SheetSize::Letter, 8.0, &scene);
    let scan = photograph(
        &sheet,
        [
            [210.0, 140.0],
            [1830.0, 190.0],
            [1760.0, 1330.0],
            [260.0, 1270.0],
        ],
        2000,
        1500,
    );
    std::fs::write(dir.join("scan.png"), ok_render::to_png(&scan)).unwrap();
    let m = measure_image(&scan, None, Some(&Reference::Bars(10.0))).unwrap();
    std::fs::create_dir_all(dir.join("out")).unwrap();
    std::fs::write(dir.join("out/measured.png"), &m.picture).unwrap();
    assert!(
        m.sheet == Some(SheetSize::Letter) && m.sheet_read,
        "{:?}",
        m.sheet
    );
    let cal = m.calibration.as_ref().expect("the bars read");
    assert!((cal.factor - 0.97).abs() < 0.01, "{cal:?}");
    assert!(m.residual < 1.5, "fit {}", m.residual);
    assert_eq!(m.parts.len(), 3, "{:?}", m.parts);
    let plate = &m.parts[0];
    for (got, want) in plate.bbox.iter().zip([30.0, 30.0, 94.0, 68.0]) {
        assert!((got - want).abs() < 0.6, "plate {:?}", plate.bbox);
    }
    assert!(plate.circularity < 0.85);
    assert_eq!(plate.holes.len(), 2, "{:?}", plate.holes);
    let mut xs: Vec<f64> = plate.holes.iter().map(|h| h.centre[0]).collect();
    xs.sort_by(f64::total_cmp);
    assert!((xs[1] - xs[0] - 50.0).abs() < 0.5, "hole spacing {xs:?}");
    for h in &plate.holes {
        assert!((h.diameter - 6.5).abs() < 0.4, "hole {h:?}");
        assert!((h.centre[1] - 49.0).abs() < 0.5, "hole {h:?}");
    }
    let bored = &m.parts[1];
    assert!(bored.circularity > 0.85, "bored disc {}", bored.circularity);
    assert!(
        (bored.diameter - 25.0).abs() < 0.5,
        "bored disc {}",
        bored.diameter
    );
    assert_eq!(bored.holes.len(), 1);
    assert!(
        (bored.holes[0].diameter - 10.0).abs() < 0.4,
        "{:?}",
        bored.holes
    );
    let disc = &m.parts[2];
    assert!(
        disc.circularity > 0.85 && (disc.diameter - 18.0).abs() < 0.5,
        "{disc:?}"
    );
    assert!((disc.centroid[0] - 110.0).abs() < 0.5 && (disc.centroid[1] - 120.0).abs() < 0.5);
    // Without the reference the same picture reads 3 % big.
    let raw = measure_image(&scan, None, None).unwrap();
    assert!((raw.parts[0].bbox[2] - raw.parts[0].bbox[0] - 64.0 / 0.97).abs() < 0.7);
}

#[test]
fn the_example_rule_scan_measures_its_parts_from_the_rule() {
    // A 300 dpi flatbed scan, no sheet: the same plate and washer with a
    // 150 mm steel rule laid a little askew. `rule-scan.png` and its
    // measurement `out/rule-measured.png` are committed too.
    let dir = example_dir();
    let scene = Scene {
        rects: vec![[20.0, 20.0, 84.0, 58.0]],
        discs: vec![[120.0, 40.0, 12.5]],
        holes: vec![[27.0, 39.0, 3.25], [77.0, 39.0, 3.25], [120.0, 40.0, 5.0]],
        rule: Some((20.0, 75.0, 150.0, 3.0)),
        ..Scene::default()
    };
    let scan = scan_image(200.0, 120.0, 300.0 / 25.4, &scene);
    std::fs::write(dir.join("rule-scan.png"), ok_render::to_png(&scan)).unwrap();
    let m = measure_image(&scan, None, Some(&Reference::Rule)).unwrap();
    std::fs::write(dir.join("out/rule-measured.png"), &m.picture).unwrap();
    assert!(m.sheet.is_none());
    let rule = m.rule.as_ref().expect("rule read");
    assert!((rule.px_per_mm - 300.0 / 25.4).abs() < 0.02, "{rule:?}");
    assert_eq!(m.parts.len(), 2, "{:?}", m.parts);
    let plate = &m.parts[0];
    for (got, want) in plate.bbox.iter().zip([20.0, 20.0, 84.0, 58.0]) {
        assert!((got - want).abs() < 0.5, "plate {:?}", plate.bbox);
    }
    let mut xs: Vec<f64> = plate.holes.iter().map(|h| h.centre[0]).collect();
    xs.sort_by(f64::total_cmp);
    assert!((xs[1] - xs[0] - 50.0).abs() < 0.4, "hole spacing {xs:?}");
    let washer = &m.parts[1];
    assert!(
        washer.circularity > 0.85 && (washer.diameter - 25.0).abs() < 0.4,
        "{washer:?}"
    );
    assert!(
        (washer.holes[0].diameter - 10.0).abs() < 0.4,
        "{:?}",
        washer.holes
    );
}
