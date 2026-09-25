//! Makes and checks `examples/measuring-sheet/`: a synthetic photograph of
//! the A4 sheet with parts on it (`scan.png`, what a phone would take,
//! askew), and the measurement of it (`out/measured.png`). Both are
//! committed; this test regenerates them so they never go stale.

use ok_photo::{measure_image, photograph, sheet_image, SheetSize};

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
    // 25 mm disc with a 10 mm bore at (170, 110), and an 18 mm disc at
    // (110, 120), on the A4 sheet at 8 px/mm.
    let sheet = sheet_image(
        SheetSize::A4,
        8.0,
        &[[30.0, 30.0, 94.0, 68.0]],
        &[[170.0, 110.0, 12.5], [110.0, 120.0, 9.0]],
        &[[37.0, 49.0, 3.25], [87.0, 49.0, 3.25], [170.0, 110.0, 5.0]],
    );
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
    let m = measure_image(&scan, SheetSize::A4).unwrap();
    std::fs::create_dir_all(dir.join("out")).unwrap();
    std::fs::write(dir.join("out/measured.png"), &m.picture).unwrap();
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
}
