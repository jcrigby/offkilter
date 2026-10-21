//! The 4 in Spectre box top (examples/spectre-box) as a regression
//! suite: the document its build script wrote regenerates into one body
//! per piece, every piece a closed solid inside the board at its
//! thickness with its wood, the pieces together filling the board (no
//! gap, and what a fillet adds to one piece its mate's rounding takes
//! off), no inside corner sharper than the bit's fillet, no piece under
//! the scrap area, the edge curves arcs that clear the bit; and the
//! feature refuses a curve the bit cannot follow.

use ok_model::{Body, Document, FeatureKind, Op, SpectreCurve, SpectreFeature, TabId, TabKind};
use std::path::PathBuf;

const IN: f64 = 25.4;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-box")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_box.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn feature(doc: &Document, tab: TabId) -> (ok_model::FeatureId, SpectreFeature) {
    match &doc.tabs.iter().find(|t| t.id == tab).unwrap().kind {
        TabKind::PartStudio(ps) => match &ps.features()[0].kind {
            FeatureKind::Spectre(s) => (ps.features()[0].id, s.clone()),
            other => panic!("{other:?}"),
        },
        _ => unreachable!(),
    }
}

fn area(b: &Body, thickness: f64) -> f64 {
    b.solid.volume() / thickness
}

#[test]
fn the_box_top_is_pieces_the_bit_can_route_that_still_nest() {
    let mut doc = load();
    let tab = doc.tabs[0].id;
    let (_, sf) = feature(&doc, tab);
    assert_eq!(
        (sf.width, sf.height, sf.thickness),
        (4.0 * IN, 4.0 * IN, 12.0)
    );
    assert_eq!((sf.edge, sf.gap, sf.scrap), (9.0, 0.0, 40.0));
    assert_eq!((sf.curve, sf.bulge), (SpectreCurve::Arc, 0.3));
    assert!((sf.bit - 3.0 / 16.0 * IN).abs() < 1e-9);
    assert_eq!(sf.light.as_ref().map(|m| m.name.as_str()), Some("Maple"));
    assert_eq!(sf.dark.as_ref().map(|m| m.name.as_str()), Some("Walnut"));
    let start = std::time::Instant::now();
    let r = doc.regenerate_studio(tab, None).unwrap();
    let elapsed = start.elapsed();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    let board = sf.width * sf.height;
    let tile = ok_sketch::spectre::TILE_AREA * sf.edge * sf.edge;
    assert!(
        r.bodies.len() as f64 > board / tile,
        "{} pieces",
        r.bodies.len()
    );
    let (mut light, mut dark, mut piece_area) = (0, 0, 0.0);
    let facet = 10f64.to_radians().to_degrees();
    for (k, b) in r.bodies.iter().enumerate() {
        b.solid
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", b.name));
        let colour = if b.name.ends_with(" dark") {
            "dark"
        } else {
            "light"
        };
        assert_eq!(b.name, format!("Piece {} {colour}", k + 1));
        let (lo, hi) = b.solid.bounds().unwrap();
        assert!(
            lo.x >= -1e-6 && lo.y >= -1e-6 && lo.z.abs() < 1e-9,
            "{}: {lo:?}",
            b.name
        );
        assert!(
            hi.x <= sf.width + 1e-6
                && hi.y <= sf.height + 1e-6
                && (hi.z - sf.thickness).abs() < 1e-9,
            "{}: {hi:?}",
            b.name
        );
        let a = area(b, sf.thickness);
        // A whole tile with its fillets may be a hair over the ideal tile.
        assert!(a >= sf.scrap && a < 1.02 * tile, "{}: {a} mm2", b.name);
        piece_area += a;
        let wood = b.material.as_ref().map(|m| m.name.as_str());
        if colour == "dark" {
            dark += 1;
            assert_eq!(wood, Some("Walnut"), "{}", b.name);
        } else {
            light += 1;
            assert_eq!(wood, Some("Maple"), "{}", b.name);
        }
    }
    assert!(dark > 0 && dark < light, "{dark} dark, {light} light");
    // No gap: the pieces fill the board but for the slivers scrapped at
    // its edge, and the bit's fillets and rounds cancel.
    assert!(
        piece_area > 0.99 * board && piece_area <= board + 1e-6,
        "{piece_area} of {board}"
    );
    // The same geometry straight from the tiling: no inside corner is
    // sharper than the sampled fillet arcs, so a bit of this radius
    // follows every outline.
    let patch = ok_sketch::spectre::build(&sf.params(), 10f64.to_radians()).unwrap();
    assert_eq!(patch.pieces.len(), r.bodies.len());
    for piece in &patch.pieces {
        let n = piece.outline.len();
        for i in 0..n {
            let (a, b, c) = (
                piece.outline[(i + n - 1) % n],
                piece.outline[i],
                piece.outline[(i + 1) % n],
            );
            let (u, v) = (b - a, c - b);
            let turn = u.cross(v).atan2(u.dot(v)).to_degrees();
            assert!(
                turn > -(facet + 3.0),
                "tile {}: turn {turn:.1} at {b:?}",
                piece.tile
            );
        }
    }
    // The arcs' radius is twice the bit's; Kaplan's cubic at an 8 mm
    // edge would be under it, and is refused with both numbers.
    let radius = ok_sketch::spectre::tightest_radius(sf.curve, sf.bulge) * sf.edge;
    assert!(radius > 2.0 * sf.bit / 2.0, "radius {radius}");
    let err = ok_sketch::spectre::build(
        &ok_sketch::spectre::Params {
            curve: SpectreCurve::Cubic,
            bulge: ok_sketch::spectre::KAPLAN_BULGE,
            edge: 8.0,
            ..sf.params()
        },
        10f64.to_radians(),
    )
    .unwrap_err();
    assert!(
        err.contains("tightest radius") && err.contains("2.38"),
        "{err}"
    );
    eprintln!(
        "{} pieces ({light} maple, {dark} walnut) regenerated in {elapsed:?}",
        r.bodies.len()
    );
    assert!(elapsed.as_secs() < 60, "{elapsed:?}");
}

/// Without the bit the same top has sharp inside corners, and the two
/// tops have the same pieces in the same places.
#[test]
fn the_bit_only_changes_the_corners() {
    let mut doc = load();
    let tab = doc.tabs[0].id;
    let (id, sf) = feature(&doc, tab);
    let with = doc.regenerate_studio(tab, None).unwrap();
    if let TabKind::PartStudio(ps) = &mut doc.tabs.iter_mut().find(|t| t.id == tab).unwrap().kind {
        ps.apply(Op::SetSpectre {
            id,
            plane: None,
            width: None,
            height: None,
            edge: None,
            thickness: None,
            gap: None,
            curve: None,
            bulge: None,
            bit: Some(0.0),
            shift: None,
            angle: None,
            scrap: None,
            web: None,
            groove: None,
            show: None,
            scheme: None,
            seed: None,
            light: None,
            dark: None,
            third: None,
            fourth: None,
            rips: None,
        })
        .unwrap();
    }
    let without = doc.regenerate_studio(tab, None).unwrap();
    assert_eq!(with.bodies.len(), without.bodies.len());
    let mut moved = 0.0f64;
    for (a, b) in with.bodies.iter().zip(&without.bodies) {
        assert_eq!(a.name, b.name);
        // A fillet or a round moves a piece's area by a few square
        // millimetres at most; the pieces are otherwise the same.
        let da = (area(a, sf.thickness) - area(b, sf.thickness)).abs();
        assert!(da < 20.0, "{}: {da} mm2", a.name);
        moved = moved.max(da);
    }
    assert!(moved > 0.5, "the bit changed nothing");
    let sharpest = ok_sketch::spectre::build(
        &ok_sketch::spectre::Params {
            bit: 0.0,
            ..sf.params()
        },
        10f64.to_radians(),
    )
    .unwrap()
    .pieces
    .iter()
    .flat_map(|p| {
        let n = p.outline.len();
        (0..n).map(move |i| {
            let (a, b, c) = (
                p.outline[(i + n - 1) % n],
                p.outline[i],
                p.outline[(i + 1) % n],
            );
            (b - a).cross(c - b).atan2((b - a).dot(c - b)).to_degrees()
        })
    })
    .fold(0.0, f64::min);
    assert!(sharpest < -55.0, "{sharpest}");
}
