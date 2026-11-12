//! The clamping cauls for a hex Spectre tile stick
//! (examples/spectre-cauls) as a regression suite: the cradle and the
//! plunger regenerate to the volumes their profiles give, the pocket
//! and the negative together are the tile, the part above the split
//! only narrows going up so the plunger comes down, and the pattern
//! plate for the sliced tiles regenerates.

use ok_model::{Document, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-cauls")
}

fn load(name: &str) -> Document {
    let json = std::fs::read_to_string(dir().join("out").join(name)).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/cauls.json")).unwrap()).unwrap()
}

fn tab(doc: &Document, name: &str) -> TabId {
    doc.tabs.iter().find(|t| t.name() == name).unwrap().id
}

fn pts(v: &serde_json::Value) -> Vec<(f64, f64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
        .collect()
}

fn area(p: &[(f64, f64)]) -> f64 {
    0.5 * (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - a.1 * b.0
        })
        .sum::<f64>()
}

/// Where the outline crosses height y, sorted.
fn crossings(p: &[(f64, f64)], y: f64) -> Vec<f64> {
    let mut xs: Vec<f64> = (0..p.len())
        .filter_map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            ((a.1 > y) != (b.1 > y)).then(|| a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1))
        })
        .collect();
    xs.sort_by(|a, b| a.total_cmp(b));
    xs
}

#[test]
fn the_cradle_and_the_plunger_regenerate_to_their_profiles() {
    let n = numbers();
    let mut doc = load("cauls.okpart");
    let length = n["length"].as_f64().unwrap();
    let block = pts(&n["block"]);
    let block_area = (block[1].0 - block[0].0) * (block[1].1 - block[0].1);
    let pocket = n["pocket_area"].as_f64().unwrap();
    let plunger = pts(&n["plunger"]);
    let plunger_area = (plunger[1].0 - plunger[0].0) * (plunger[1].1 - plunger[0].1);
    let negative = n["negative_area"].as_f64().unwrap();
    for (name, want) in [
        ("Base", (block_area - pocket) * length),
        ("Lid", (plunger_area - negative) * length),
        ("Tile stick", n["tile_area"].as_f64().unwrap() * length),
    ] {
        let r = doc.regenerate_studio(tab(&doc, name), None).unwrap();
        assert_eq!(r.bodies.len(), 1, "{name}");
        r.bodies[0].solid.validate().unwrap();
        let v = r.bodies[0].solid.volume();
        println!("{name}: {v:.0} mm3, expected {want:.0}");
        assert!((v - want).abs() < 1e-3 * want, "{name}: {v} vs {want}");
    }
    let asm = doc.regenerate_assembly(tab(&doc, "Cauls")).unwrap();
    assert_eq!(asm.bodies.len(), 3);
}

/// The pocket's lower part and the lid's negative together are the
/// loosened outline; above the split the outline only narrows going
/// up, within the slot, so the plunger comes down onto the bundle, and
/// the plunger fits the slot with its play.
#[test]
fn the_lid_takes_the_part_of_the_tile_that_narrows_upward() {
    let n = numbers();
    let loose = pts(&n["loose"]);
    let outline = pts(&n["outline"]);
    let s = n["split"].as_f64().unwrap();
    let (clear, play) = (n["clear"].as_f64().unwrap(), n["play"].as_f64().unwrap());
    let tile = n["tile_area"].as_f64().unwrap();
    assert!((area(&outline) - tile).abs() < 1e-6);
    // The loosened outline: the tile plus the clearance round it.
    let perimeter: f64 = (0..outline.len())
        .map(|i| {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
        })
        .sum();
    let loose_area = area(&loose);
    assert!(
        (loose_area - tile - clear * perimeter).abs() < 0.5,
        "{loose_area} vs {}",
        tile + clear * perimeter
    );
    // The split's chord is one span, and above it the spans only narrow.
    let top = loose.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let chord = crossings(&loose, s + 1e-6);
    assert_eq!(chord.len(), 2);
    let (mut left, mut right) = (chord[0], chord[1]);
    let mut share = 0.0;
    for k in 1..400 {
        let y = s + (top - s) * k as f64 / 400.0;
        let c = crossings(&loose, y);
        assert_eq!(c.len(), 2, "two pieces above the split at {y}");
        assert!(c[0] >= left - 1e-6 && c[1] <= right + 1e-6, "widens at {y}");
        left = c[0];
        right = c[1];
        share += (c[1] - c[0]) * (top - s) / 400.0;
    }
    let lid_share = n["lid_share"].as_f64().unwrap();
    println!("the lid takes {:.1} % of the tile", 100.0 * lid_share);
    assert!((share / loose_area - lid_share).abs() < 0.01);
    assert!(lid_share > 0.03 && lid_share < 0.1, "{lid_share}");
    // The plunger in the slot.
    let plunger = pts(&n["plunger"]);
    let xlo = loose.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let xhi = loose.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    assert!((plunger[0].0 - xlo - play).abs() < 1e-9 && (xhi - plunger[1].0 - play).abs() < 1e-9);
    assert!(plunger[0].1 > s);
}

#[test]
fn the_pattern_plate_for_the_sliced_tiles_regenerates() {
    let mut doc = load("pattern.okpart");
    let r = doc
        .regenerate_studio(tab(&doc, "Pattern plate"), None)
        .unwrap();
    assert_eq!(r.bodies.len(), 1);
    let plate = &r.bodies[0].solid;
    plate.validate().unwrap();
    let (lo, hi) = plate.bounds().unwrap();
    println!(
        "plate {:.0} x {:.0} x {:.0}, {} faces",
        hi.x - lo.x,
        hi.y - lo.y,
        hi.z - lo.z,
        plate.faces.len()
    );
    // The grooves are the hex outlines' lines: hundreds of faces.
    assert!(plate.faces.len() > 500, "{}", plate.faces.len());
    assert!(hi.x - lo.x > 101.6 && hi.y - lo.y > 101.6);
}
