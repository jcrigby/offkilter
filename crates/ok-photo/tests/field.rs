//! The measuring field: tiles that tape into one sheet, marks that say
//! where they are, and a photograph that needs any four of them.

use ok_photo::{field_image, measure_image, photograph, Field, Reference, Scene, SheetSize};

#[test]
fn the_field_lays_out_its_tiles_on_the_lattice() {
    let f = Field::new(SheetSize::Letter, 2, 2).unwrap();
    assert_eq!(f.per_tile(), (5, 4));
    assert_eq!(f.tile_pitch(), (200.0, 150.0));
    assert_eq!((f.columns(), f.rows()), (9, 7));
    let b = f.bounds();
    assert!(
        (b[0] + 24.0).abs() < 1e-9 && (b[1] + 30.0).abs() < 1e-9,
        "{b:?}"
    );
    assert!(
        (b[2] - 443.4).abs() < 1e-9 && (b[3] - 323.9).abs() < 1e-9,
        "{b:?}"
    );
    // The next tile starts on this one's last mark column, so the
    // marks there print on both and the tiles tape mark over mark.
    let (t0, t1) = (f.tile_bounds(0, 0), f.tile_bounds(1, 0));
    assert!(
        t1[0] < 200.0 - 12.0 && t0[2] > 200.0 + 12.0,
        "{t0:?} {t1:?}"
    );
    assert_eq!(
        Field::parse(SheetSize::Letter, "3x2").unwrap().columns(),
        13
    );
    assert!(
        Field::parse(SheetSize::Letter, "4x1").is_err(),
        "17 columns do not code"
    );
    assert!(Field::parse(SheetSize::Letter, "two").is_err());
    let one = Field::new(SheetSize::A4, 1, 1).unwrap();
    assert_eq!((one.columns(), one.rows()), (6, 4));
    let pdf = ok_photo::field_pdf(&f);
    let text = ok_sheet::pdf::inflated(&pdf);
    assert!(text.contains("/Count 4") && text.contains("tile 4 of 4, column 2 row 2"));
    assert!(text.contains("cut along the dashed line"));
}

#[test]
fn a_photograph_of_part_of_the_field_with_marks_covered_still_measures() {
    // A 2x2 Letter field with a 64 x 38 plate lying right across the
    // mark at (100, 100), a washer and a disc, printed at 97 % and
    // photographed askew so that the field's corners fall outside the
    // picture: the marks that are seen carry the fit.
    let field = Field::new(SheetSize::Letter, 2, 2).unwrap();
    let scene = Scene {
        rects: vec![[70.0, 85.0, 134.0, 123.0]],
        discs: vec![[220.0, 160.0, 12.5], [160.0, 230.0, 9.0]],
        holes: vec![
            [77.0, 104.0, 3.25],
            [127.0, 104.0, 3.25],
            [220.0, 160.0, 5.0],
        ],
        print: 0.97,
        ..Scene::default()
    }
    .bars(40.0, 180.0, 10.0, 5);
    let sheet = field_image(&field, 5.0, &scene);
    // The raster's corners land outside a 2000 x 1500 picture: the
    // camera saw the middle of the field, tilted.
    let scan = photograph(
        &sheet,
        [
            [-300.0, -500.0],
            [2500.0, -300.0],
            [2300.0, 1900.0],
            [-100.0, 2100.0],
        ],
        2000,
        1500,
    );
    let m = measure_image(&scan, None, Some(&Reference::Bars(10.0))).unwrap();
    let fr = m.field.as_ref().expect("a field");
    assert!(m.sheet.is_none());
    assert!(
        fr.marks_used >= 20,
        "{} marks used of {}",
        fr.marks_used,
        fr.marks_read
    );
    assert_eq!(fr.marks_used, fr.marks_read, "every mark that read agreed");
    assert!(m.residual < 1.5, "fit {}", m.residual);
    // The mark under the plate did not read, and nothing else was invented.
    assert!(!fr.marks.iter().any(|k| (k.column, k.row) == (2, 2)));
    for k in &fr.marks {
        assert!(k.column < field.columns() && k.row < field.rows(), "{k:?}");
    }
    let cal = m.calibration.as_ref().expect("the bars read");
    assert!((cal.factor - 0.97).abs() < 0.01, "{cal:?}");
    assert_eq!(m.parts.len(), 3, "{:?}", m.parts);
    let plate = &m.parts[0];
    for (got, want) in plate.bbox.iter().zip([70.0, 85.0, 134.0, 123.0]) {
        assert!((got - want).abs() < 0.6, "plate {:?}", plate.bbox);
    }
    // The erased mark under it is filled back in: its two real holes
    // are all it has.
    assert_eq!(plate.holes.len(), 2, "{:?}", plate.holes);
    for h in &plate.holes {
        assert!((h.diameter - 6.5).abs() < 0.4, "hole {h:?}");
    }
    let washer = &m.parts[1];
    assert!(
        washer.circularity > 0.85 && (washer.diameter - 25.0).abs() < 0.5,
        "{washer:?}"
    );
    assert_eq!(washer.holes.len(), 1);
    let disc = &m.parts[2];
    assert!(
        (disc.diameter - 18.0).abs() < 0.5 && (disc.centroid[0] - 160.0).abs() < 0.5,
        "{disc:?}"
    );
    // The picture covers what the camera saw, clipped near the marks.
    assert!(
        fr.bounds[0] > -31.0 && fr.bounds[2] < 444.0,
        "{:?}",
        fr.bounds
    );
}

#[test]
fn too_few_marks_say_so() {
    let field = Field::new(SheetSize::Letter, 1, 1).unwrap();
    let sheet = field_image(&field, 5.0, &Scene::default());
    // Only the strip along the bottom row of marks is in the picture.
    let scan = photograph(
        &sheet,
        [
            [0.0, -1300.0],
            [2000.0, -1300.0],
            [2000.0, 250.0],
            [0.0, 250.0],
        ],
        2000,
        400,
    );
    let err = measure_image(&scan, None, None).unwrap_err();
    assert!(err.contains("two columns and two rows"), "{err}");
}
