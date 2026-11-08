//! The Spectre lid from glued-up prisms (examples/spectre-lid) as a
//! regression suite: the document its build script wrote through the MCP
//! server regenerates into whole tiles, each split into the five convex
//! prisms the search found, every piece a closed solid at the lid's
//! thickness with its wood; the whole tiles cover the 4 in square the
//! glue-up is trimmed to; the prisms satisfy the shoulder rule for a
//! five-sixteenths arc; and the sample layout is one tile to print.

use ok_model::{
    Body, Document, FeatureKind, Op, SpectreCurve, SpectreFeature, SpectreLayout, TabId, TabKind,
};
use ok_sketch::spectre::prisms;
use std::path::PathBuf;

const IN: f64 = 25.4;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-lid")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_lid.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/lid.json")).unwrap()).unwrap()
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

fn point_in(p: ok_math::Vec2, poly: &[ok_math::Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[test]
fn the_lid_regenerates_into_whole_tiles_of_five_prisms_that_cover_the_square() {
    let mut doc = load();
    let n = numbers();
    let tab = doc.tabs[0].id;
    let (_, sf) = feature(&doc, tab);
    assert_eq!(
        (sf.width, sf.height, sf.thickness),
        (4.0 * IN, 4.0 * IN, 12.0)
    );
    assert_eq!((sf.edge, sf.gap, sf.bit), (10.56, 0.7, 0.0));
    assert_eq!((sf.curve, sf.bulge), (SpectreCurve::Arc, 0.2673));
    assert!(sf.whole);
    assert_eq!(sf.prisms.len(), n["cuts"].as_array().unwrap().len());
    let facet = 10f64.to_radians();
    // The prisms: five, convex, within the shoulder rule of a 112.6 degree
    // arc (a 1/4 in bullnose at this edge), no bump split by a glue line.
    let dec = prisms::decompose(sf.curve, sf.bulge, &sf.prisms, facet).unwrap();
    assert_eq!(dec.prisms.len(), n["prisms"].as_u64().unwrap() as usize);
    assert_eq!(dec.prisms.len(), 5);
    assert!(dec.clean);
    assert!((dec.theta - 56.3).abs() < 0.1, "{}", dec.theta);
    assert!(
        (dec.radius * sf.edge - IN / 4.0).abs() < 0.02,
        "{}",
        dec.radius * sf.edge
    );
    let mut total = 0.0;
    for p in &dec.prisms {
        assert!(p.convex && p.shoulders && !p.bump_split, "{p:?}");
        total += p.area;
    }
    assert!(
        (total - ok_sketch::spectre::TILE_AREA).abs() < 1e-6,
        "{total}"
    );
    // The patch: every tile whole, split into the five prisms, the board
    // inside the pieces' box.
    let patch = ok_sketch::spectre::build(&sf.params(), facet).unwrap();
    assert_eq!(patch.pieces.len(), n["pieces"].as_u64().unwrap() as usize);
    assert_eq!(patch.tiles, n["tiles"].as_u64().unwrap() as usize);
    let (lo, hi) = patch.board;
    assert!(
        lo.x > 0.0 && lo.y > 0.0 && hi.x < patch.width && hi.y < patch.height,
        "{lo:?} {hi:?} in {} x {}",
        patch.width,
        patch.height
    );
    assert!((hi.x - lo.x - sf.width).abs() < 1e-9 && (hi.y - lo.y - sf.height).abs() < 1e-9);
    let mut per_tile = std::collections::BTreeMap::<u32, Vec<&ok_sketch::spectre::Piece>>::new();
    for piece in &patch.pieces {
        per_tile.entry(piece.tile).or_default().push(piece);
    }
    assert_eq!(per_tile.len(), patch.tiles);
    for (tile, pieces) in &per_tile {
        let mut prisms_seen: Vec<u8> = pieces.iter().map(|p| p.prism.unwrap()).collect();
        prisms_seen.sort();
        assert_eq!(
            prisms_seen,
            vec![0, 1, 2, 3, 4],
            "tile {tile}: {:?}",
            pieces
                .iter()
                .map(|p| (p.prism, ok_sketch::signed_area(&p.outline)))
                .collect::<Vec<_>>()
        );
    }
    // Coverage: with no gap the whole tiles cover every point of the
    // board, since nothing was clipped at its edge.
    let solid = ok_sketch::spectre::build(
        &ok_sketch::spectre::Params {
            gap: 0.0,
            ..sf.params()
        },
        facet,
    )
    .unwrap();
    let steps = 40;
    for i in 0..=steps {
        for j in 0..=steps {
            let p = ok_math::Vec2::new(
                lo.x + (hi.x - lo.x) * i as f64 / steps as f64,
                lo.y + (hi.y - lo.y) * j as f64 / steps as f64,
            );
            let covered = solid.pieces.iter().any(|piece| point_in(p, &piece.outline));
            assert!(covered, "{p:?} is not under a tile");
        }
    }
    // The bodies: one per piece, closed, at the thickness, with its wood.
    let start = std::time::Instant::now();
    let r = doc.regenerate_studio(tab, None).unwrap();
    let elapsed = start.elapsed();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(r.bodies.len(), patch.pieces.len());
    let (mut light, mut dark) = (0, 0);
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
        let (blo, bhi) = b.solid.bounds().unwrap();
        assert!(
            blo.z.abs() < 1e-9 && (bhi.z - sf.thickness).abs() < 1e-9,
            "{}: {blo:?} {bhi:?}",
            b.name
        );
        let a = area(b, sf.thickness);
        assert!(a > 50.0 && a < 250.0, "{}: {a} mm2", b.name);
        if colour == "dark" {
            dark += 1;
            assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("Walnut"));
        } else {
            light += 1;
            assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("Maple"));
        }
    }
    assert_eq!(
        (light, dark),
        (
            n["light"].as_u64().unwrap() as usize,
            n["dark"].as_u64().unwrap() as usize
        )
    );
    assert!(dark > 0 && dark < light);
    eprintln!(
        "{} pieces in {} tiles regenerated in {elapsed:?}",
        r.bodies.len(),
        patch.tiles
    );
    assert!(elapsed.as_secs() < 120, "{elapsed:?}");
}

/// The sample layout is one whole tile, eroded by half the gap, at the
/// board's centre, in the light wood.
#[test]
fn the_sample_is_one_tile_to_print() {
    let mut doc = load();
    let n = numbers();
    let tab = doc.tabs[0].id;
    let (id, sf) = feature(&doc, tab);
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
            bit: None,
            shift: None,
            angle: None,
            scrap: None,
            web: None,
            groove: None,
            show: Some(SpectreLayout::Sample),
            scheme: None,
            seed: None,
            light: None,
            dark: None,
            third: None,
            fourth: None,
            rips: None,
            whole: None,
            prisms: None,
        })
        .unwrap();
    }
    let r = doc.regenerate_studio(tab, None).unwrap();
    assert!(
        r.errors().next().is_none(),
        "{:?}",
        r.errors().collect::<Vec<_>>()
    );
    assert_eq!(r.bodies.len(), 1);
    let b = &r.bodies[0];
    assert_eq!(b.name, "Sample piece");
    b.solid.validate().unwrap();
    // Three millimetres thick whatever the lid is, with the glue lines
    // engraved in the top, so it is less than the eroded tile's prism.
    let (lo, hi) = b.solid.bounds().unwrap();
    assert!(
        lo.z.abs() < 1e-9 && (hi.z - ok_model::SAMPLE_THICKNESS).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    let volume = b.solid.volume();
    assert!(
        (volume - n["sample_volume"].as_f64().unwrap()).abs() < 1.0,
        "{volume}"
    );
    let master = ok_sketch::spectre::master(&sf.params(), 0.0, 10f64.to_radians()).unwrap();
    let full = master.area * ok_model::SAMPLE_THICKNESS;
    let tile = ok_sketch::spectre::TILE_AREA * sf.edge * sf.edge;
    assert!(
        master.area < tile && master.area > tile - 14.0 * sf.edge * sf.gap,
        "{} of {tile}",
        master.area
    );
    // Four grooves 0.6 wide and 0.5 deep across a 41 mm tile take tens
    // of cubic millimetres, not hundreds.
    let (w, d) = ok_model::SAMPLE_GROOVE;
    let cut = full - volume;
    assert!(
        cut > 4.0 * 8.0 * w * d && cut < 4.0 * 45.0 * w * d,
        "grooves took {cut} of {full}"
    );
    // The top has more faces than a plain extrusion's one: the grooves' floors.
    let tops = b
        .solid
        .faces
        .iter()
        .filter(|f| f.plane.normal.z > 0.99)
        .count();
    assert!(tops > sf.prisms.len(), "{tops} upward faces");
    let centre = ok_math::Vec3::new(sf.width / 2.0, sf.height / 2.0, 0.0);
    assert!(
        lo.x < centre.x && centre.x < hi.x && lo.y < centre.y && centre.y < hi.y,
        "{lo:?} {hi:?}"
    );
    assert!(
        hi.x - lo.x < 5.0 * sf.edge && hi.y - lo.y < 4.0 * sf.edge,
        "{lo:?} {hi:?}"
    );
    assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("Maple"));
}

/// The sheet at 1:1 draws the trim line and fills every piece in its
/// tile's gray with its prism's white stripes; the pieces of one tile
/// share a colour in the renderer and differ by their stripes' angle.
#[test]
fn the_sheet_fills_the_prisms_and_the_render_colours_by_tile() {
    let mut doc = load();
    let tab = doc.tabs[0].id;
    let r = doc.regenerate_studio(tab, None).unwrap();
    let mut by_tile = std::collections::BTreeMap::<u32, Vec<Option<f64>>>::new();
    for b in &r.bodies {
        let s = b.shade.expect("a piece has a shade");
        by_tile.entry(s.tile).or_default().push(s.stripes);
    }
    for (tile, stripes) in &by_tile {
        let mut angles: Vec<i64> = stripes.iter().map(|a| a.unwrap().round() as i64).collect();
        angles.sort();
        assert_eq!(angles, vec![0, 36, 72, 108, 144], "tile {tile}");
    }
    let colours: std::collections::BTreeSet<_> = r
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (
                b.shade.unwrap().tile,
                format!("{:?}", ok_render::colour_of(b, i)),
            )
        })
        .collect();
    assert_eq!(colours.len(), by_tile.len(), "one colour per tile");
    let opts = ok_sheet::Options {
        views: vec!["top".into()],
        sheet: ok_sheet::SheetSize::parse("A3").unwrap(),
        parts: false,
        scale: Some(1.0),
        ..ok_sheet::Options::default()
    };
    let pdf = ok_sheet::drawing_pdf(&mut doc, tab, &opts).unwrap();
    let text = ok_sheet::pdf::inflated(&pdf);
    assert!(text.contains("TRIM"));
    let fills = text.matches(" h f").count();
    assert!(
        fills >= r.bodies.len(),
        "{fills} filled regions for {} pieces",
        r.bodies.len()
    );
    assert!(text.contains("1 G 1 g"), "white stripes");
}
