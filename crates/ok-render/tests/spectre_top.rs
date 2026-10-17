//! The Spectre puzzle top (examples/spectre-top) as a regression suite:
//! the document its build script wrote through the MCP server
//! regenerates into one body per piece and the alignment web, every
//! piece a closed solid inside the board at the board's thickness with
//! its wood (the mystic pairs walnut), no piece a sliver, the web
//! filling the gaps between them; the pattern layout is one plate whose
//! grooves are every gap; and the STL of the plate is written.

use ok_model::{Body, Document, FeatureKind, Op, SpectreFeature, SpectreLayout, TabId, TabKind};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-top")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_top.okpart")).unwrap();
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

/// Area of a piece's top, from its volume at the board's thickness.
fn area(b: &Body, thickness: f64) -> f64 {
    b.solid.volume() / thickness
}

#[test]
fn the_top_regenerates_into_pieces_with_their_woods_and_a_web() {
    let mut doc = load();
    let tab = doc.tabs[0].id;
    let (_, sf) = feature(&doc, tab);
    // The checkerboard top's board, half-inch edges, the resin gap.
    assert_eq!((sf.width, sf.height, sf.thickness), (304.0, 228.0, 12.0));
    assert_eq!((sf.edge, sf.gap, sf.web, sf.groove), (12.7, 1.5, 5.0, 12.5));
    assert_eq!(sf.scrap, 150.0);
    assert_eq!(sf.bulge, ok_sketch::spectre::KAPLAN_BULGE);
    assert_eq!(sf.show, SpectreLayout::Design);
    assert_eq!(sf.light.as_ref().map(|m| m.name.as_str()), Some("Maple"));
    assert_eq!(sf.dark.as_ref().map(|m| m.name.as_str()), Some("Walnut"));
    let start = std::time::Instant::now();
    let r = doc.regenerate_studio(tab, None).unwrap();
    let elapsed = start.elapsed();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    let (pieces, rest) = r.bodies.split_at(r.bodies.len() - 1);
    let web = &rest[0];
    assert_eq!(web.name, "Alignment web");
    // The patch: the depth-3 supertile, 559 tiles, is the shallowest
    // with room for this board.
    let patch = ok_sketch::spectre::build(&sf.params(), 10f64.to_radians()).unwrap();
    assert_eq!(patch.depth, 3);
    assert_eq!(patch.pieces.len(), pieces.len());
    // A tile the edge cuts in two is two pieces, unless the smaller is
    // scrap; so at least as many pieces as tiles.
    assert!(
        patch.tiles <= patch.pieces.len(),
        "{} tiles, {} pieces",
        patch.tiles,
        patch.pieces.len()
    );
    // More pieces than whole tiles would fill the board, since the
    // edge cuts the outer ones; a tile is 8.2 square edges.
    let tile = ok_sketch::spectre::TILE_AREA * sf.edge * sf.edge;
    let board = sf.width * sf.height;
    assert!(
        pieces.len() as f64 > board / tile,
        "{} pieces",
        pieces.len()
    );
    let mut piece_area = 0.0;
    let (mut light, mut dark, mut whole) = (0, 0, 0);
    let mut smallest = f64::INFINITY;
    for (k, b) in pieces.iter().enumerate() {
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
        assert!(a < tile, "{}: {a} mm2 is more than a tile", b.name);
        smallest = smallest.min(a);
        piece_area += a;
        // A whole tile, eroded: the gap's half off its perimeter, less
        // than the tile by a few percent.
        if a > 0.85 * tile {
            whole += 1;
        }
        let wood = b.material.as_ref().map(|m| m.name.as_str());
        if colour == "dark" {
            dark += 1;
            assert_eq!(wood, Some("Walnut"), "{}", b.name);
        } else {
            light += 1;
            assert_eq!(wood, Some("Maple"), "{}", b.name);
        }
    }
    // Slivers under the scrap area are resin, not pieces.
    assert!(smallest >= sf.scrap, "smallest piece {smallest} mm2");
    assert!(
        whole > pieces.len() / 3,
        "{whole} whole tiles of {}",
        pieces.len()
    );
    // The mystics: two of every nine tiles, give or take the edge.
    assert!(
        dark > pieces.len() / 8 && dark < pieces.len() / 3,
        "{dark} dark, {light} light"
    );
    // The web is the gaps between the pieces, less any pocket the edge
    // closes off in one piece's concavity.
    web.solid.validate().unwrap();
    let (lo, hi) = web.solid.bounds().unwrap();
    assert!(lo.x >= -1e-6 && lo.y >= -1e-6 && hi.x <= sf.width + 1e-6 && hi.y <= sf.height + 1e-6);
    assert!(
        lo.z.abs() < 1e-9 && (hi.z - sf.web).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    let gaps = board - piece_area;
    let web_area = web.solid.volume() / sf.web;
    assert!(
        web_area <= gaps + 1e-6 && web_area > 0.9 * gaps,
        "web {web_area} of gaps {gaps}"
    );
    // The gap is a lattice a gap wide: a few percent of the board.
    assert!(
        gaps > 0.04 * board && gaps < 0.15 * board,
        "gaps {gaps} of {board}"
    );
    eprintln!(
        "{} pieces ({light} maple, {dark} walnut, {whole} whole) and the web regenerated in {elapsed:?}",
        pieces.len()
    );
    assert!(elapsed.as_secs() < 120, "{elapsed:?}");
}

#[test]
fn the_pattern_plate_grooves_every_gap() {
    let mut doc = load();
    let tab = doc.tabs[0].id;
    let (id, sf) = feature(&doc, tab);
    let r = doc.regenerate_studio(tab, None).unwrap();
    let piece_area: f64 = r.bodies[..r.bodies.len() - 1]
        .iter()
        .map(|b| area(b, sf.thickness))
        .sum();
    let gaps = sf.width * sf.height - piece_area;
    if let TabKind::PartStudio(ps) = &mut doc.tabs.iter_mut().find(|t| t.id == tab).unwrap().kind {
        ps.apply(Op::SetSpectre {
            id,
            plane: None,
            width: None,
            height: None,
            edge: None,
            thickness: None,
            gap: None,
            bulge: None,
            shift: None,
            scrap: None,
            web: None,
            groove: None,
            show: Some(SpectreLayout::Pattern),
            light: None,
            dark: None,
        })
        .unwrap();
    }
    let r = doc.regenerate_studio(tab, None).unwrap();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(r.bodies.len(), 1);
    let plate = &r.bodies[0];
    assert_eq!(plate.name, "Pattern plate");
    plate.solid.validate().unwrap();
    // A slab 10 mm wider than the board all round on a 3 mm floor, the
    // grooves as deep as the board and a half, and every gap a groove,
    // plus the reference hole.
    let (lo, hi) = plate.solid.bounds().unwrap();
    assert!(
        (lo.x + 10.0).abs() < 1e-6 && (lo.y + 10.0).abs() < 1e-6 && (lo.z + 3.0).abs() < 1e-9,
        "{lo:?}"
    );
    assert!(
        (hi.x - sf.width - 10.0).abs() < 1e-6
            && (hi.y - sf.height - 10.0).abs() < 1e-6
            && (hi.z - sf.groove).abs() < 1e-9,
        "{hi:?}"
    );
    let slab = (sf.width + 20.0) * (sf.height + 20.0) * (sf.groove + 3.0);
    let hole = std::f64::consts::PI * (sf.gap / 2.0).powi(2) * (sf.groove + 3.0);
    let expect = slab - gaps * sf.groove - hole;
    let v = plate.solid.volume();
    assert!((v - expect).abs() < 2e-3 * expect, "{v} vs {expect}");
    // The STL the build script wrote is the plate: binary, its
    // triangles spanning the plate's height.
    let stl = std::fs::read(dir().join("out/pattern_plate.stl")).unwrap();
    let tris = u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize;
    assert_eq!(stl.len(), 84 + 50 * tris, "binary STL");
    let (mut zlo, mut zhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in 0..tris {
        for v in 0..3 {
            let at = 84 + 50 * t + 12 + 12 * v + 8;
            let z = f32::from_le_bytes(stl[at..at + 4].try_into().unwrap()) as f64;
            zlo = zlo.min(z);
            zhi = zhi.max(z);
        }
    }
    assert!(
        (zlo + 3.0).abs() < 1e-3 && (zhi - sf.groove).abs() < 1e-3,
        "{zlo}..{zhi}"
    );
}
