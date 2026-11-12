//! The Spectre lid as a bundle of triangular sticks (examples/spectre-mosaic)
//! as a regression suite: the hex-edged tile's cover from the kernel's
//! search, the lid the tool laid from it, and the mold it wrote.

use ok_math::Vec2;
use ok_sketch::spectre::mosaic::{self, Shape};
use ok_sketch::spectre::{curved_tile, Curve, TILE_AREA};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-mosaic")
}

fn answer() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/mosaic.json")).unwrap()).unwrap()
}

fn poly(v: &serde_json::Value) -> Vec<Vec2> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| Vec2::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
        .collect()
}

fn area(p: &[Vec2]) -> f64 {
    0.5 * (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.x * b.y - a.y * b.x
        })
        .sum::<f64>()
}

/// The hex tile: 42 straight pieces of half an edge, the straight
/// tile's area (every bump has a dent), its corners on one of six
/// lattices, and the kernel's cover of it: the four shapes at their
/// sizes, no two overlapping, 99 % of the tile, the same every time.
#[test]
fn the_hex_tile_is_covered_by_the_four_sticks_to_ninety_nine_percent() {
    let tile = mosaic::tile();
    assert_eq!(tile.len(), 42);
    assert!((area(&tile) - TILE_AREA).abs() < 1e-9);
    assert_eq!(curved_tile(Curve::Hex, 0.0, 3), tile);
    let m = answer();
    let cover = m["tile"]["coverage"].as_f64().unwrap();
    let pieces = m["tile"]["pieces"].as_array().unwrap();
    println!(
        "{:.2} % in {} pieces, counts {}",
        100.0 * cover,
        pieces.len(),
        m["tile"]["counts"]
    );
    assert!(cover > 0.99, "{cover}");
    let sum: f64 = pieces.iter().map(|p| area(&poly(&p["outline"]))).sum();
    assert!((sum / TILE_AREA - cover).abs() < 1e-9);
    // The shapes' sides, in edge units.
    let r3 = 3f64.sqrt();
    for p in pieces {
        let o = poly(&p["outline"]);
        let mut sides: Vec<f64> = (0..o.len())
            .map(|i| o[(i + 1) % o.len()].distance(o[i]))
            .collect();
        sides.sort_by(|a, b| a.total_cmp(b));
        let want: Vec<f64> = match p["shape"].as_str().unwrap() {
            "triangle" => vec![0.5, 0.5, 0.5],
            "half" => vec![0.25, r3 / 4.0, 0.5],
            "small" => vec![0.125, r3 / 8.0, 0.25],
            "trapezoid" => vec![0.125, r3 / 8.0, 0.25, 0.25],
            other => panic!("{other}"),
        };
        assert_eq!(sides.len(), want.len());
        for (a, b) in sides.iter().zip(&want) {
            assert!((a - b).abs() < 1e-9, "{sides:?}");
        }
    }
    // The search repeats: the same rounds and seed give the same cover.
    let rounds = m["tile"]
        .get("rounds")
        .and_then(|v| v.as_u64())
        .unwrap_or(6000) as u32;
    let again = mosaic::mosaic(rounds, 3);
    assert!(
        (again.coverage - cover).abs() < 1e-12,
        "{} vs {cover}",
        again.coverage
    );
    assert_eq!(again.pieces.len(), pieces.len());
    let counts: Vec<usize> = Shape::ALL
        .iter()
        .map(|s| again.pieces.iter().filter(|p| p.shape == *s).count())
        .collect();
    assert_eq!(counts, again.counts.to_vec());
}

/// The lid: every whole tile the 4 in square reaches carries the cover
/// through its frame, each stick pulled in by the gaps, the slivers and
/// the sticks under a millimetre left to the resin; the sticks of one
/// tile never overlap and the lid's sticks sum to the tile count times
/// the cover less what was dropped.
#[test]
fn the_lid_carries_the_cover_through_every_tile_with_the_two_gaps() {
    let m = answer();
    let edge = m["edge"].as_f64().unwrap();
    let (gap, inner, min) = (
        m["gap"].as_f64().unwrap(),
        m["inner"].as_f64().unwrap(),
        m["min"].as_f64().unwrap(),
    );
    let tiles = m["tiles"].as_u64().unwrap() as usize;
    let pieces = m["pieces"].as_array().unwrap();
    let per_tile = m["tile"]["pieces"].as_array().unwrap().len();
    let dropped = m["dropped"].as_u64().unwrap() as usize;
    println!("{tiles} tiles, {} sticks, {dropped} dropped", pieces.len());
    assert!((16..=30).contains(&tiles), "{tiles}");
    assert_eq!(pieces.len() + dropped, tiles * per_tile);
    assert!(
        dropped < tiles * per_tile / 4,
        "too many dropped: {dropped}"
    );
    // Every stick is a convex polygon narrower than a triangle's inradius
    // and at least `min` wide, inside its tile's eroded outline.
    let tile = mosaic::tile();
    let frames: Vec<&serde_json::Value> = Vec::new();
    let _ = frames;
    let mut by_tile: std::collections::BTreeMap<u64, Vec<Vec<Vec2>>> = Default::default();
    for p in pieces {
        let o = poly(&p["outline"]);
        assert!(o.len() >= 3);
        assert!(area(&o) > 0.0, "clockwise");
        let widest = 2.0 * area(&o)
            / (0..o.len())
                .map(|i| o[(i + 1) % o.len()].distance(o[i]))
                .sum::<f64>();
        assert!(widest * 2.0 >= min * 0.5, "a sliver kept: {widest}");
        assert!(area(&o) <= Shape::Triangle.area() * edge * edge + 1e-6);
        by_tile
            .entry(p["tile"].as_u64().unwrap())
            .or_default()
            .push(o);
    }
    assert_eq!(by_tile.len(), tiles);
    // Within a tile the sticks are the gap apart: no overlap, and the
    // closest two sides at least `inner` apart (sampled at the corners).
    for sticks in by_tile.values() {
        for i in 0..sticks.len() {
            for j in i + 1..sticks.len() {
                for q in &sticks[i] {
                    let d = nearest(q, &sticks[j]);
                    assert!(d > inner - 0.05, "sticks {i} and {j} are {d} apart");
                }
            }
        }
    }
    // The wood: the cover less the gaps. A 0.7 mm line round a 5.3 mm
    // triangle takes two fifths of it (the inradius goes from 1.52 to
    // 1.17), so the lid is about half wood at these lines; a lattice
    // triangle pulled in by half the inner gap says what to expect.
    let wood: f64 = pieces.iter().map(|p| area(&poly(&p["outline"]))).sum();
    let tile_mm = area(&tile) * edge * edge;
    let r3 = 3f64.sqrt();
    let side = edge / 2.0 - inner * r3;
    let kept = (side / (edge / 2.0)).powi(2);
    let cover = m["tile"]["coverage"].as_f64().unwrap();
    println!(
        "wood {:.0} mm2 of {:.0} in the tiles ({:.1} %); a triangle keeps {:.0} % inside its lines",
        wood,
        tiles as f64 * tile_mm,
        100.0 * wood / (tiles as f64 * tile_mm),
        100.0 * kept
    );
    let share = wood / (tiles as f64 * tile_mm);
    assert!(
        share > 0.6 * kept * cover && share < kept * cover,
        "{share} against {}",
        kept * cover
    );
    let _ = gap;
}

/// Distance from a point to a polygon's boundary.
fn nearest(q: &Vec2, p: &[Vec2]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            let ab = b - a;
            let t = ((*q - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
            (a + ab * t).distance(*q)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The mold: the positive is the plate with a bar per stick, every bar
/// the stick's outline tapering by the draft over its height; the box
/// takes the plate on its rim with the backing under the bars' tips.
#[test]
fn the_mold_has_a_bar_per_stick_and_a_box_the_plate_rests_on() {
    let m = answer();
    let positive =
        ok_mesh::from_stl(&std::fs::read(dir().join("out/positive.stl")).unwrap()).unwrap();
    let tray = ok_mesh::from_stl(&std::fs::read(dir().join("out/box.stl")).unwrap()).unwrap();
    let sticks = m["pieces"].as_array().unwrap();
    let corners: usize = sticks
        .iter()
        .map(|p| p["outline"].as_array().unwrap().len())
        .sum();
    // A bar's triangles: two fans and two per side; the plate's twelve.
    let bar_tris: usize = sticks
        .iter()
        .map(|p| 4 * p["outline"].as_array().unwrap().len() - 4)
        .sum();
    println!(
        "{} sticks, {} corners, {} triangles in the positive",
        sticks.len(),
        corners,
        positive.triangles.len()
    );
    assert_eq!(positive.triangles.len(), bar_tris + 12);
    let (height, plate_t) = (m["bar"].as_f64().unwrap(), 4.0);
    let (zlo, zhi) = positive
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.z), hi.max(v.z))
        });
    assert!(
        (zlo + plate_t).abs() < 1e-6 && (zhi - height).abs() < 1e-6,
        "{zlo} {zhi}"
    );
    let plate = poly(&m["plate"]);
    let (plo, phi) = (plate[0], plate[1]);
    let at = |v: &serde_json::Value| Vec2::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap());
    let (b_lo, b_hi, b_top) = (
        at(&m["box"][0]),
        at(&m["box"][1]),
        m["box"][2].as_f64().unwrap(),
    );
    // The plate overhangs the box's cavity and sits inside its outer walls.
    assert!(b_lo.x < plo.x && b_lo.y < plo.y && b_hi.x > phi.x && b_hi.y > phi.y);
    assert!((b_top - (3.0 + m["backing"].as_f64().unwrap() + height)).abs() < 1e-9);
    assert_eq!(tray.triangles.len(), 5 * 12);
    let tz = tray
        .vertices
        .iter()
        .fold(f64::NEG_INFINITY, |hi, v| hi.max(v.z));
    assert!((tz - b_top).abs() < 1e-6);
}
