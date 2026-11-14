//! The scratch-stock stations for the Spectre's sticks
//! (examples/spectre-planes) as a regression suite: every edge of the
//! hex tile is three chords of a semicircle, so the curvy faces are
//! arcs of the eroded tile's radii and the kernel's cover puts a
//! sector on every bump chord; the sticks, blocks, sled and masters
//! regenerate to the volumes their sections give; each block seats
//! its stick and stops it; each blade at its set lies on the finished
//! face and never reaches the block; the sled fits the block and the
//! block the printer's bed; and the masters carry the faces over
//! their rail edges.

use ok_brep::{BoolOp, Solid};
use ok_math::{Plane, Vec2, Vec3};
use ok_model::{Document, TabId};
use ok_sketch::spectre::{curved_tile, Curve};
use ok_sketch::{Loop, Profile};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-planes")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/planes.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/planes.json")).unwrap()).unwrap()
}

fn tab(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

/// A tab's one body, regenerated: the file's own bodies are not trusted.
fn body_of(doc: &mut Document, name: &str) -> Solid {
    let r = doc.regenerate_studio(tab(doc, name), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one body in {name}");
    let body = r.bodies[0].solid.clone();
    body.validate().unwrap();
    body
}

fn f(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap()
}

fn pts(v: &serde_json::Value) -> Vec<(f64, f64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| (f(&p[0]), f(&p[1])))
        .collect()
}

fn area(p: &[(f64, f64)]) -> f64 {
    (0.5 * (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - a.1 * b.0
        })
        .sum::<f64>())
    .abs()
}

/// The highest point of a closed section over x, or of a polyline.
fn top_at(p: &[(f64, f64)], x: f64, closed: bool) -> f64 {
    let n = if closed { p.len() } else { p.len() - 1 };
    (0..n)
        .filter_map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            ((a.0 > x) != (b.0 > x)).then(|| a.1 + (x - a.0) * (b.1 - a.1) / (b.0 - a.0))
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// A box from `lo` to `hi`.
fn slab(lo: Vec3, hi: Vec3) -> Solid {
    let p = Profile {
        outer: Loop::polygon(vec![
            Vec2::new(lo.x, lo.y),
            Vec2::new(hi.x, lo.y),
            Vec2::new(hi.x, hi.y),
            Vec2::new(lo.x, hi.y),
        ]),
        holes: Vec::new(),
    };
    ok_brep::extrude(&p, &Plane::XY, lo.z, hi.z, 1).unwrap()
}

fn size(s: &Solid) -> Vec3 {
    let (lo, hi) = s.bounds().unwrap();
    hi - lo
}

const T30: f64 = 0.577_350_269_189_625_8;

/// The premise: half a hexagon on an edge is inscribed in the circle
/// of half the edge about the edge's middle, so bowing its three
/// flats into that circle's arcs makes the edge a semicircle; seven
/// edges bump out and seven dent in; and the kernel's cover has a
/// triangle with its apex at the middle on every chord of every bump,
/// which is the sector the bump station is for.
#[test]
fn every_edge_of_the_hex_tile_is_three_chords_of_a_semicircle() {
    let t = curved_tile(Curve::Hex, 0.0, 3);
    assert_eq!(t.len(), 42);
    let at = |i: usize| t[i % 42];
    let phase = (0..3)
        .find(|&ph| {
            (0..14).all(|k| (at(ph + 3 * k).distance(at(ph + 3 * k + 3)) - 1.0).abs() < 1e-9)
        })
        .expect("an edge's length between every third corner");
    let ccw: f64 = (0..42).map(|i| at(i).cross(at(i + 1))).sum();
    let (mut bumps, mut dents) = (0, 0);
    for k in 0..14 {
        let (a, p1, p2, b) = (
            at(phase + 3 * k),
            at(phase + 3 * k + 1),
            at(phase + 3 * k + 2),
            at(phase + 3 * k + 3),
        );
        let m = (a + b) * 0.5;
        for q in [a, p1, p2, b] {
            assert!(
                (q.distance(m) - 0.5).abs() < 1e-9,
                "edge {k}: a corner off the semicircle"
            );
        }
        for (u, w) in [(a, p1), (p1, p2), (p2, b)] {
            assert!(
                (u.distance(w) - 0.5).abs() < 1e-9,
                "edge {k}: a chord that is not half an edge"
            );
        }
        // The outline turns away from the tile at a bump's corner.
        if (p1 - a).cross(p2 - p1) * ccw > 0.0 {
            bumps += 1;
        } else {
            dents += 1;
        }
    }
    assert_eq!((bumps, dents), (7, 7));
    let n = numbers();
    let (edge, gap) = (f(&n["edge"]), f(&n["gap"]));
    assert!((f(&n["r_bump"]) - (edge / 2.0 - gap / 2.0)).abs() < 1e-9);
    assert!((f(&n["r_dent"]) - (edge / 2.0 + gap / 2.0)).abs() < 1e-9);
    let per = &n["per_tile"];
    println!("per tile: {per}");
    assert_eq!(
        (per["bumps"].as_u64(), per["dents"].as_u64()),
        (Some(7), Some(7))
    );
    assert_eq!(per["bump_sectors"].as_u64(), Some(21));
    assert_eq!(per["bump_chords_otherwise"].as_u64(), Some(0));
    let tri = per["dent_triangles"].as_u64().unwrap();
    assert_eq!(tri + per["dent_chords_otherwise"].as_u64().unwrap(), 21);
    assert!(tri >= 10, "{tri} triangles in the dents");
}

/// The sticks: a sector of the bump's eroded arc about a centre the
/// glue line under its apex, a triangle hollowed to the dent's, and
/// the inner triangle with its point off; each regenerates to its
/// section's volume.
#[test]
fn the_sticks_are_the_eroded_arcs() {
    let (mut doc, n) = (load(), numbers());
    let (edge, gap, inner, len) = (f(&n["edge"]), f(&n["gap"]), f(&n["inner"]), f(&n["stick"]));
    let alt = edge / 2.0 * 3f64.sqrt() / 2.0;
    let (rb, rd) = (edge / 2.0 - gap / 2.0, edge / 2.0 + gap / 2.0);
    // The bump: top and corners.
    let b = &n["bump"];
    assert!((f(&b["top"]) - (rb - inner)).abs() < 1e-9);
    let (cx, cy) = (f(&b["corner"][0]), f(&b["corner"][1]));
    assert!((cx - cy * T30).abs() < 1e-9, "the corner is on the flank");
    assert!((cx.hypot(cy + inner) - rb).abs() < 1e-9, "and on the arc");
    let section = pts(&b["section"]);
    assert_eq!(section[0], (0.0, 0.0));
    for q in &section[1..] {
        assert!((q.0.hypot(q.1 + inner) - rb).abs() < 1e-9);
        assert!(
            q.0.abs() <= q.1 * T30 + 1e-9,
            "inside the 60° of its flanks"
        );
    }
    // The dent: the arc's centre is two altitudes less the glue line up.
    let d = &n["dent"];
    let c = 2.0 * alt - inner;
    assert!((f(&d["centre"][1]) - c).abs() < 1e-9);
    assert!((f(&d["low"]) - (c - rd)).abs() < 1e-9);
    let (cx, cy) = (f(&d["corner"][0]), f(&d["corner"][1]));
    assert!((cx - cy * T30).abs() < 1e-9 && (cx.hypot(cy - c) - rd).abs() < 1e-9);
    for q in &pts(&d["section"])[1..] {
        assert!((q.0.hypot(q.1 - c) - rd).abs() < 1e-9);
    }
    // The point: the cauls' cut list takes gap / 2 - inner off it.
    let p = &n["point"];
    let side = edge / 2.0 - inner * 3f64.sqrt();
    assert!((f(&n["inner_side"]) - side).abs() < 1e-9);
    assert!((f(&p["flat"]) - (side * 3f64.sqrt() / 2.0 - (gap / 2.0 - inner))).abs() < 1e-9);
    println!(
        "bump: top {:.3}, corner {:.3}; dent: bottom {:.3}, corner {:.3}; point flat {:.3}",
        f(&b["top"]),
        f(&b["corner"][1]),
        f(&d["low"]),
        cy,
        f(&p["flat"])
    );
    // A strip leaves the blade a hair: over the bump's top, over the dent's corners.
    assert!(f(&b["strip"]) > f(&b["top"]) && f(&b["strip"]) < f(&b["top"]) + 0.2);
    assert!(f(&d["strip"]) > f(&d["corner"][1]) && f(&d["strip"]) < f(&d["corner"][1]) + 0.2);
    for key in ["bump", "dent", "point"] {
        let body = body_of(&mut doc, &format!("Stick {key}"));
        let expect = area(&pts(&n[key]["section"])) * len;
        println!(
            "stick {key}: {:.1} mm3, its section gives {expect:.1}",
            body.volume()
        );
        assert!((body.volume() - expect).abs() < 0.05);
        assert!((size(&body).x - len).abs() < 1e-6);
    }
}

/// Each block is its section less the seat, the screws and its
/// notches; the seat is open along the stick and closed at the stop;
/// the rails stand over the land; and the finished stick sits in it.
#[test]
fn the_blocks_seat_their_sticks_and_stop_them() {
    let (mut doc, n) = (load(), numbers());
    let b = &n["block"];
    let (l, rail_z, stop, ft) = (
        f(&b["l"]),
        f(&b["rail_z"]),
        f(&b["stop"]),
        f(&b["flange_t"]),
    );
    let section = area(&pts(&b["outline"]));
    for (k, st) in n["stations"].as_array().unwrap().iter().enumerate() {
        let key = st["key"].as_str().unwrap();
        let body = body_of(&mut doc, &format!("Block {key}"));
        let holes = 4.0 * std::f64::consts::PI * (f(&b["hole_d"]) / 2.0).powi(2) * ft;
        let notches = (k + 1) as f64 * f(&b["notch_w"]).powi(2) * ft;
        let expect = section * l - area(&pts(&st["seat_cut"])) * (l - stop) - holes - notches;
        println!(
            "block {key}: {:.0} mm3, its section gives {expect:.0}",
            body.volume()
        );
        assert!(
            (body.volume() - expect).abs() < 1.0,
            "{}",
            body.volume() - expect
        );
        let s = size(&body);
        assert!(
            (s.x - l).abs() < 1e-6
                && (s.y - f(&b["flange_w"])).abs() < 1e-6
                && (s.z - rail_z).abs() < 1e-6
        );
        let seat = f(&st["seat"]);
        assert!((seat + f(&st["rail"]) - rail_z).abs() < 1e-9);
        // Air in the seat at mid length, block under it, and block where the seat stops.
        let probe = |x0: f64, x1: f64, z0: f64, z1: f64, y0: f64, y1: f64| {
            overlap(&body, &slab(Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1)))
        };
        assert!(
            probe(100.0, 110.0, seat + 1.0, seat + 2.0, -0.2, 0.2) < 1e-6,
            "{key}: the seat is open"
        );
        assert!(
            (probe(100.0, 110.0, seat - 3.0, seat - 1.5, -0.2, 0.2) - 6.0).abs() < 1e-3,
            "{key}: block under the seat"
        );
        assert!(
            (probe(2.0, stop - 2.0, seat + 1.0, seat + 2.0, -0.2, 0.2) - (stop - 4.0) * 0.4).abs()
                < 1e-3,
            "{key}: the stop"
        );
        // The rails, and air over the land between them.
        let drop = f(&b["land_drop"]);
        assert!((probe(100.0, 110.0, rail_z - 0.3, rail_z - 0.1, 10.0, 12.0) - 4.0).abs() < 1e-3);
        assert!(
            probe(100.0, 110.0, rail_z + 0.1, rail_z + 0.3, 10.0, 12.0) < 1e-6,
            "{key}: nothing over the rails"
        );
        assert!(
            probe(100.0, 110.0, rail_z - drop + 0.1, rail_z, 3.6, 7.0) < 1e-6,
            "{key}: the land is under the rails"
        );
        // The stick in its seat: touching, not through it.
        let stick = body_of(&mut doc, &format!("Stick {key}"));
        let through = overlap(&body, &stick);
        println!("  stick against block: {through:.4} mm3");
        assert!(
            through < 0.05,
            "{key}: the stick is in the block by {through}"
        );
        let (lo, _) = stick.bounds().unwrap();
        assert!((lo.x - stop).abs() < 1e-6 && (lo.z - seat).abs() < 1e-6);
    }
}

/// At its set the blade's edge is the stick's finished face, across
/// the face; the edge is over the land everywhere, so the blade never
/// reaches the block; and the hollow's mouth is wider than the stock.
#[test]
fn each_blade_lies_on_its_face_and_clear_of_the_block() {
    let (mut doc, n) = (load(), numbers());
    let b = &n["block"];
    let (rail_z, drop) = (f(&b["rail_z"]), f(&b["land_drop"]));
    assert!(
        f(&n["blade"]["w"]) < f(&b["land_w"]),
        "the blade is narrower than the land"
    );
    for st in n["stations"].as_array().unwrap() {
        let key = st["key"].as_str().unwrap();
        let name = st["blade"].as_str().unwrap();
        let (rail, seat) = (f(&st["rail"]), f(&st["seat"]));
        let edge = pts(&st["edge"]);
        let lowest = edge.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
        assert!(
            lowest > -drop + 0.2,
            "{name}: {lowest} over the rail plane, the land {drop} under it"
        );
        // Across the finished face, a tenth in from each corner.
        let section = pts(&n[key]["section"]);
        let half = match key {
            "bump" => f(&n["bump"]["corner"][0]),
            "dent" => f(&n["dent"]["corner"][0]),
            _ => f(&n["point"]["flat_w"]) / 2.0,
        };
        let mut worst = 0f64;
        for i in 0..=20 {
            let x = (half - 0.1).max(0.01) * (i as f64 / 10.0 - 1.0) + 1e-4;
            worst = worst.max((top_at(&edge, x, false) + rail - top_at(&section, x, true)).abs());
        }
        println!("{name} blade on the {key} stick: {worst:.4} mm off its face at worst");
        assert!(worst < 0.01, "{name}: {worst}");
        // The bodies, where they are used.
        let blade = body_of(&mut doc, &format!("Blade {name}"));
        let block = body_of(&mut doc, &format!("Block {key}"));
        let stick = body_of(&mut doc, &format!("Stick {key}"));
        assert!(
            overlap(&blade, &block) < 1e-9,
            "{name}: the blade is in the block"
        );
        let bite = overlap(&blade, &stick);
        assert!(
            bite < 0.02,
            "{name}: the blade is {bite} mm3 into the finished stick"
        );
        let (lo, hi) = blade.bounds().unwrap();
        assert!(
            (lo.z - (rail_z + lowest)).abs() < 1e-6,
            "{name}: its edge at {}",
            lo.z - seat
        );
        assert!((hi.y - lo.y - f(&n["blade"]["w"])).abs() < 1e-6);
    }
    // The hollow starts on the stock's two corners: its mouth takes the strip's width.
    let (rb, apex, notch) = (f(&n["r_bump"]), f(&n["apex"]), f(&n["blade"]["notch"]));
    let mouth = 2.0 * (rb * rb - (rb - notch).powi(2)).sqrt();
    let stock = 2.0 * f(&n["bump"]["strip"]) * T30;
    println!("hollow's mouth {mouth:.2}, the stock {stock:.2} wide; centre {apex} under the apex");
    assert!(mouth > stock);
}

/// The sled: its section less the slot, the screw's bore and the nut's
/// trap; it straddles the block with its play, hangs clear of the
/// foot, clears the stock when it starts, and the bore reaches the
/// slot without marking the wall the blade lies on; and the block is
/// no longer than the bed, with rail under the sled's heel at the
/// stick's near end.
#[test]
fn the_sled_fits_the_block_and_the_block_the_bed() {
    let (mut doc, n) = (load(), numbers());
    let (s, b) = (&n["sled"], &n["block"]);
    let body = body_of(&mut doc, "Sled");
    let (l, h, slot_x, slot_t, slot_w) = (
        f(&s["l"]),
        f(&s["body_h"]),
        f(&s["slot_x"]),
        f(&s["slot_t"]),
        f(&s["slot_w"]),
    );
    let (tw, th) = (f(&s["tunnel"][0]), f(&s["tunnel"][1]));
    let (skirt_t, skirt_d, play) = (f(&s["skirt"][0]), f(&s["skirt"][1]), f(&s["play"]));
    let tl = f(&n["blade"]["lean"]).to_radians().tan();
    let (sz, sd, sl) = (
        f(&s["screw"]["z"]),
        f(&s["screw"]["d"]),
        f(&s["screw"]["l"]),
    );
    let (nx, nt, nw, nd) = (
        f(&s["nut"]["x"]),
        f(&s["nut"]["t"]),
        f(&s["nut"]["w"]),
        f(&s["nut"]["deep"]),
    );
    // A leaning slot of one thickness along the cut is that thickness times its height.
    let slot = slot_t * ((h - th) * tw + h * (slot_w - tw));
    let bore = std::f64::consts::PI * (sd / 2.0).powi(2) * (nx + (slot_x - sz * tl) - (nx + nt));
    let expect = area(&pts(&s["outline"])) * l - slot - nt * nw * nd - bore;
    println!(
        "sled: {:.0} mm3, its section gives {expect:.0}",
        body.volume()
    );
    assert!(
        (body.volume() - expect).abs() < 1.0,
        "{}",
        body.volume() - expect
    );
    let size = size(&body);
    let width = f(&b["w"]) + 2.0 * (play + skirt_t);
    assert!(
        (size.x - l).abs() < 1e-6
            && (size.y - width).abs() < 1e-6
            && (size.z - (h + skirt_d)).abs() < 1e-6
    );
    // As printed, sole up: the slot at the screw's height, the tunnel, the trap, and the sole beside the tunnel.
    let probe = |lo: Vec3, hi: Vec3| overlap(&body, &slab(lo, hi));
    let front = slot_x - sz * tl;
    assert!(
        probe(
            Vec3::new(front + 0.25, -5.0, h - sz - 0.4),
            Vec3::new(front + slot_t - 0.25, 5.0, h - sz + 0.4)
        ) < 1e-6,
        "the slot"
    );
    assert!(
        probe(
            Vec3::new(1.0, -tw / 2.0 + 0.2, h - th + 0.2),
            Vec3::new(l - 1.0, tw / 2.0 - 0.2, h + 1.0)
        ) < 1e-6,
        "the tunnel"
    );
    assert!(
        probe(
            Vec3::new(nx + 0.2, -nw / 2.0 + 0.2, 0.2),
            Vec3::new(nx + nt - 0.2, nw / 2.0 - 0.2, nd - 0.2)
        ) < 1e-6,
        "the nut's trap"
    );
    assert!(
        (probe(Vec3::new(2.0, 9.0, h - 1.0), Vec3::new(10.0, 13.0, h + 1.0)) - 32.0).abs() < 1e-3,
        "the sole stops at its plane"
    );
    // The fit.
    assert!(slot_t > f(&n["blade"]["t"]) + 0.2 && slot_w > f(&n["blade"]["w"]) + 0.2);
    assert!(play > 0.05 && play < 0.3);
    assert!(
        skirt_d < f(&b["rail_z"]) - f(&b["flange_t"]) - 1.0,
        "the skirts clear the foot"
    );
    assert!(th <= 2.0, "the blade's free length");
    for st in n["stations"].as_array().unwrap() {
        let key = st["key"].as_str().unwrap();
        let over = f(&n[key]["strip"]) - f(&st["rail"]);
        let lift = f(&st["lift"]);
        println!("{key}: stock {over:.2} over the rails, the sole starts {lift:.2} up");
        assert!(
            lift > 0.1 && lift < 1.2 && over < th - 0.5,
            "{key}: the tunnel clears the stock"
        );
        // The stick's widest under the tunnel's roof.
        assert!(f(&n[key]["strip"]) * 2.0 * T30 < tw || key == "point");
    }
    assert!(f(&n["inner_side"]) < tw);
    // The bore: into the slot at its lowest edge, short of the back wall at its highest.
    let into = sl - (slot_x - (sz - sd / 2.0) * tl);
    let short = slot_x - (sz + sd / 2.0) * tl + slot_t - sl;
    println!("the bore is {into:.2} into the slot and {short:.2} short of its back wall");
    assert!(into > 0.0 && short > 0.0);
    assert!(
        slot_x - h * tl > nx + nt + 2.0,
        "plastic between the nut and the slot"
    );
    assert!(h - th - nd > 1.0, "and between the nut and the tunnel");
    // The bed, and the heel on the rails when the blade is at the stick's near end.
    assert!(f(&b["l"]) <= 250.0);
    assert!(f(&b["stop"]) + f(&n["stick"]) + (l - slot_x) <= f(&b["l"]));
}

/// A master is its foot and its outline on end; over its rail edges
/// the bump master carries the arc the hollow is set on, its top
/// where the stick's will be, and the dent master the hollow face in
/// a plateau, its bottom on the rail edges.
#[test]
fn the_masters_carry_the_faces_over_their_rail_edges() {
    let (mut doc, n) = (load(), numbers());
    let m = &n["master"];
    let (l, h, foot) = (f(&m["l"]), f(&m["h"]), f(&m["foot"]));
    let (rb, rd, apex) = (f(&n["r_bump"]), f(&n["r_dent"]), f(&n["apex"]));
    let rail = f(&n["stations"][0]["rail"]);
    for kind in ["bump", "dent"] {
        let body = body_of(&mut doc, &format!("Master {kind}"));
        let outline = pts(&m[kind]);
        let expect = f(&m["foot_area"]) * foot + area(&outline) * l;
        println!(
            "master {kind}: {:.0} mm3, its outline gives {expect:.0}",
            body.volume()
        );
        assert!((body.volume() - expect).abs() < 0.5);
        let s = size(&body);
        let high = outline
            .iter()
            .map(|q| q.1)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (s.x - f(&n["block"]["w"])).abs() < 1e-6
                && (s.y - high).abs() < 1e-6
                && (s.z - (l + foot)).abs() < 1e-6
        );
        if kind == "bump" {
            assert!(
                (high - h - (f(&n["bump"]["top"]) - rail)).abs() < 1e-9,
                "the arc's top over the rail edges"
            );
            let centre = h - apex - rail;
            let arc: Vec<_> = outline
                .iter()
                .filter(|q| q.0.abs() < 3.0 && q.1 > h - 0.4)
                .collect();
            assert!(arc.len() > 10);
            assert!(arc
                .iter()
                .all(|q| (q.0.hypot(q.1 - centre) - rb).abs() < 1e-9));
            // The hollow blade, set, lies along it.
            let edge = pts(&n["stations"][0]["edge"]);
            for i in -8..=8 {
                let x = i as f64 * 0.3 + 1e-4;
                assert!((top_at(&edge, x, false) + h - top_at(&outline, x, true)).abs() < 0.01);
            }
        } else {
            assert!((high - h - f(&m["plateau"])).abs() < 1e-9);
            let arc: Vec<_> = outline
                .iter()
                .filter(|q| q.0.abs() < 2.9 && q.1 > h - 1e-6)
                .collect();
            assert!(arc.len() > 10);
            assert!(arc
                .iter()
                .all(|q| (q.0.hypot(q.1 - h - rd) - rd).abs() < 1e-9));
            let low = arc.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
            assert!(
                (low - h).abs() < 1e-9,
                "the hollow's bottom on the rail edges"
            );
            assert!(
                f(&m["plateau"]) < f(&n["blade"]["notch"]) - 0.05,
                "the plateau is under the cove's shoulders"
            );
        }
    }
}
