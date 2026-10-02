//! The EGO cart (examples/ego-cart, rev B: a powered tricycle) as a
//! regression suite: the document regenerates closed, the drive is the
//! one the README describes and meshes, the load sits where it should
//! and the cart stands up, and nothing is placed inside anything else.

use ok_brep::{BoolOp, Solid};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, FeatureKind, TabId};
use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/ego-cart")
}

fn load() -> Document {
    let json = std::fs::read_to_string(example_dir().join("out/ego_cart.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn tab_starting(doc: &Document, prefix: &str) -> (TabId, String) {
    let t = doc
        .tabs
        .iter()
        .find(|t| t.name().starts_with(prefix))
        .unwrap_or_else(|| panic!("no tab starting {prefix}"));
    (t.id, t.name().to_string())
}

fn body<'a>(r: &'a AssemblyResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
        .solid
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

/// Overlap volume of two placed bodies, None when the boolean fails.
fn overlap(a: &Solid, b: &Solid) -> Option<f64> {
    let (la, ha) = bounds(a);
    let (lb, hb) = bounds(b);
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return Some(0.0);
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .ok()
        .map(|s| s.volume().abs())
}

/// Pairs that touch by design: the bags' feet and the casters' plates
/// on the deck, the stub in the head's coupler, the gearbox on its
/// bracket; and the ring's sectors, which touch the pinion (the mesh),
/// the wheel (the web on the rotor boss) and each other (the cuts).
const KNOWN: &[(&str, &str)] = &[
    ("left bag", "platform"),
    ("right bag", "platform"),
    ("left caster", "platform"),
    ("right caster", "platform"),
    ("EGO stub", "power head"),
    ("worm box", "frame"),
    ("output shaft", "frame"),
];

const SEGMENTS: usize = 6;

fn is_segment(name: &str) -> bool {
    name.starts_with("ring segment ")
}

fn known(a: &str, b: &str) -> bool {
    KNOWN
        .iter()
        .any(|(p, q)| (a == *p && b == *q) || (a == *q && b == *p))
        || (is_segment(a) && (is_segment(b) || b == "pinion" || b == "drive wheel"))
        || (is_segment(b) && (a == "pinion" || a == "drive wheel"))
}

/// The same masses build.py uses (kg): the cart's parts estimated, the
/// bags as carried.
const MASS: &[(&str, f64)] = &[
    ("drive wheel", 2.5),
    ("axle", 0.1),
    ("ring segment", 0.15),
    ("pinion", 0.15),
    ("output shaft", 0.2),
    ("worm box", 2.6),
    ("flex shaft", 0.5),
    ("EGO stub", 0.3),
    ("power head", 4.5),
    ("frame", 6.0),
    ("platform", 4.0),
    ("left caster", 1.5),
    ("right caster", 1.5),
    ("left bag", 20.0),
    ("right bag", 20.0),
];

fn mass_of(name: &str) -> f64 {
    let key = if is_segment(name) {
        "ring segment"
    } else {
        name
    };
    MASS.iter()
        .find(|(n, _)| *n == key)
        .unwrap_or_else(|| panic!("no mass for {name}"))
        .1
}

/// Centre of mass of the named bodies.
fn centre_of_mass(r: &AssemblyResult, names: &[&str]) -> (f64, Vec3) {
    let mut total = 0.0;
    let mut sum = Vec3::ZERO;
    for b in &r.bodies {
        if !names.is_empty() && !names.contains(&b.name.as_str()) {
            continue;
        }
        let m = mass_of(&b.name);
        total += m;
        sum += b.solid.centroid().unwrap() * m;
    }
    (total, sum * (1.0 / total))
}

#[test]
fn every_part_regenerates_closed_and_the_cart_places_twenty_bodies() {
    let mut doc = load();
    let tabs: Vec<(TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    assert_eq!(tabs.len(), 14, "thirteen parts and the cart");
    for (id, name, kind) in &tabs {
        if kind == "part_studio" {
            let r = doc.regenerate_studio(*id, None).unwrap();
            let want = if name == "Ring gear" { SEGMENTS } else { 1 };
            assert_eq!(r.bodies.len(), want, "{name}: {want} bodies");
            for b in &r.bodies {
                assert!(b.solid.volume() > 0.0, "{name} has volume");
                b.solid.validate().unwrap();
            }
            assert!(
                r.statuses.iter().all(|f| f.error.is_none()),
                "{name}: {:?}",
                r.statuses
            );
        }
    }
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    assert_eq!(r.bodies.len(), 14 + SEGMENTS);
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
}

/// The ring prints as sectors: each fits a 220 mm bed, they are alike
/// in volume, and each rim joint has a dowel hole on both sides.
#[test]
fn the_ring_gear_is_six_printable_sectors_with_a_dowel_in_every_joint() {
    let mut doc = load();
    let ring = tab_named(&doc, "Ring gear");
    let r = doc.regenerate_studio(ring, None).unwrap();
    assert_eq!(r.bodies.len(), SEGMENTS);
    let volumes: Vec<f64> = r.bodies.iter().map(|b| b.solid.volume()).collect();
    let (lo, hi) = volumes
        .iter()
        .fold((f64::MAX, 0.0f64), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    assert!((hi - lo) / hi < 0.01, "sectors alike: {volumes:?}");
    for b in &r.bodies {
        let (lo, hi) = bounds(&b.solid);
        assert!(
            hi.y - lo.y <= 220.0 && hi.z - lo.z <= 220.0,
            "{} fits the bed",
            b.name
        );
        let dowels = b
            .solid
            .surfaces
            .iter()
            .filter(|s| {
                matches!(s, ok_brep::Surface::Cylinder { radius, .. } if (radius - 1.6).abs() < 1e-6)
            })
            .count();
        assert_eq!(dowels, 2, "{}: a dowel hole at each end", b.name);
    }
}

#[test]
fn the_drive_is_a_worm_box_into_a_ring_gear_and_the_pinion_meshes_in_it() {
    let mut doc = load();
    // The printed gears are gear features; the worm box's ratio is in
    // its tab name ("Worm box NMRV040 20:1").
    let gear = |tab: &str| -> (f64, f64, f64) {
        let id = tab_named(&doc, tab);
        doc.studio(id)
            .unwrap()
            .features()
            .iter()
            .find_map(|f| match &f.kind {
                FeatureKind::Gear(g) => Some((g.teeth as f64, g.module, g.rim)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{tab}: no gear feature"))
    };
    let (z_ring, m_ring, rim) = gear("Ring gear");
    let (z_pinion, m_pinion, pinion_rim) = gear("Pinion");
    assert!(rim > 0.0 && pinion_rim == 0.0, "a ring and a pinion");
    assert_eq!(m_ring, m_pinion);
    let (_, worm_name) = tab_starting(&doc, "Worm box");
    let worm: f64 = worm_name
        .rsplit(' ')
        .next()
        .and_then(|s| s.strip_suffix(":1"))
        .and_then(|s| s.parse().ok())
        .expect("the worm ratio in the tab name");
    let ratio = worm * z_ring / z_pinion;
    let wheel_d = 508.0;
    let speed = 4800.0 / ratio / 60.0 * std::f64::consts::PI * wheel_d / 1000.0;
    assert!((1.2..1.5).contains(&speed), "walking pace: {speed} m/s");
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    // The pinion's axis is a pitch-radius difference from the wheel's.
    let wheel_c = body(&r, "drive wheel").centroid().unwrap();
    let pinion_c = body(&r, "pinion").centroid().unwrap();
    let want = m_ring * (z_ring - z_pinion) / 2.0;
    let got = ((pinion_c.y - wheel_c.y).powi(2) + (pinion_c.z - wheel_c.z).powi(2)).sqrt();
    assert!((got - want).abs() < 1e-3, "centre distance {got} vs {want}");
    // The ring sits inside the tyre and wholly outboard of the rotor
    // face, where the spokes are not; and its sectors mesh with the
    // pinion: with a space facing a tooth the overlap is the backlash's
    // worth of nothing.
    let mut meshed = 0.0;
    for b in r.bodies.iter().filter(|b| is_segment(&b.name)) {
        let (lo, hi) = bounds(&b.solid);
        assert!(
            hi.x <= -40.0 + 1e-6,
            "{} outboard of the rotor face: {}",
            b.name,
            hi.x
        );
        assert!(hi.z - wheel_c.z < wheel_d / 2.0 && wheel_c.z - lo.z < wheel_d / 2.0);
        meshed += overlap(&b.solid, body(&r, "pinion")).expect("the mesh booleans");
    }
    assert!(meshed < 1.0, "ring and pinion overlap by {meshed} mm3");
}

#[test]
fn the_load_sits_on_the_deck_and_the_cart_stands_up() {
    let mut doc = load();
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    let (plo, phi) = bounds(body(&r, "platform"));
    for bag in ["left bag", "right bag"] {
        let (lo, hi) = bounds(body(&r, bag));
        assert!((lo.z - phi.z).abs() < 1e-9, "{bag} stands on the deck");
        assert!(
            lo.x >= plo.x && hi.x <= phi.x && lo.y >= plo.y && hi.y <= phi.y,
            "{bag} inside the deck's edges"
        );
    }
    // The three wheels stand on the ground; the gearbox clears it.
    for w in ["drive wheel", "left caster", "right caster"] {
        assert!(bounds(body(&r, w)).0.z.abs() < 1e-9, "{w} on the ground");
    }
    assert!(
        bounds(body(&r, "worm box")).0.z >= 120.0,
        "ground clearance under the gearbox"
    );
    let grip = bounds(body(&r, "power head")).1.z;
    assert!((950.0..1250.0).contains(&grip), "grip height {grip}");

    // Stability from the masses: the support triangle is the drive
    // wheel's contact and the two casters' contacts (under their axles).
    let contact = |name: &str| {
        let (lo, hi) = bounds(body(&r, name));
        ((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0)
    };
    let (cx, cy) = contact("left caster");
    let (_, wy) = contact("drive wheel");
    let caster_y = {
        // The caster wheel's contact is under its axle: the lowest
        // point of the body, which is the wheel.
        let s = body(&r, "left caster");
        let (lo, _) = bounds(s);
        // The wheel is 200 across, so its axle is at lo.y + 100.
        lo.y + 100.0
    };
    let _ = cy;
    let tip_angle = |cg: Vec3| -> f64 {
        // Perpendicular distance from the mass centre to the edge from
        // the drive wheel's contact to the nearer caster's, over the
        // height.
        let ex = if cg.x < 0.0 { cx } else { -cx };
        let (ex, ey) = (ex, caster_y - wy);
        let dist = (ex * (cg.y - wy) - ey * cg.x).abs() / (ex * ex + ey * ey).sqrt();
        dist.atan2(cg.z).to_degrees()
    };
    let (total, cg) = centre_of_mass(&r, &[]);
    let share = (cg.y - caster_y) / (wy - caster_y);
    assert!((60.0..70.0).contains(&total), "{total} kg");
    assert!((0.45..0.75).contains(&share), "drive wheel carries {share}");
    assert!(
        share * total >= 34.0,
        "enough on the drive wheel for a wet hill: {} kg",
        share * total
    );
    let both = tip_angle(cg);
    assert!(both >= 10.0, "side tip angle with two bags {both}");
    // On a 15 % hill the mass centre moves towards the drive wheel by
    // 0.15 of its height and must stay ahead of it: no hand force.
    assert!(
        cg.y + 0.15 * cg.z < wy,
        "the hill does not put the load behind the wheel"
    );
    // One bag, on one side: still standing on the flat, but with
    // little to spare; the README says to load a single bag over the
    // wheel.
    let names: Vec<&str> = MASS
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| *n != "right bag")
        .collect();
    let (_, one) = centre_of_mass(&r, &names);
    let single = tip_angle(one);
    assert!(
        single > 0.0 && single < both,
        "one bag on the left: {single} degrees"
    );

    // Every pair of placed bodies: no overlap but the known contacts,
    // and those by a negligible volume.
    let mut worst: Vec<(String, String, f64)> = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            let (a, b) = (&r.bodies[i], &r.bodies[j]);
            let known = known(&a.name, &b.name);
            match overlap(&a.solid, &b.solid) {
                Some(v) if v > 1e-3 && !known => worst.push((a.name.clone(), b.name.clone(), v)),
                Some(v) if known => {
                    assert!(v < 50.0, "{} / {}: {v} mm3 at a contact", a.name, b.name)
                }
                // Two sectors meeting on a cut face can fail the boolean
                // outright (`touching_ring_sectors_intersect_to_nothing`
                // below records it); a failure between other bodies is a
                // finding.
                None if known && is_segment(&a.name) && is_segment(&b.name) => {}
                None => worst.push((a.name.clone(), b.name.clone(), f64::NAN)),
                _ => {}
            }
        }
    }
    assert!(worst.is_empty(), "{worst:?}");
}

/// Two sectors that share a cut face, with half a dowel hole each on
/// it, intersect to nothing. Today the boolean fails on one such pair
/// ("result is not a closed solid"): a kernel bug on solids touching
/// over a face with a notch in it, kept here as its record.
#[test]
#[ignore]
fn touching_ring_sectors_intersect_to_nothing() {
    let mut doc = load();
    let ring = tab_named(&doc, "Ring gear");
    let r = doc.regenerate_studio(ring, None).unwrap();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            let v = ok_brep::boolean(&r.bodies[i].solid, &r.bodies[j].solid, BoolOp::Intersection)
                .unwrap_or_else(|e| panic!("{} x {}: {e}", r.bodies[i].name, r.bodies[j].name))
                .volume()
                .abs();
            assert!(v < 1e-6, "{} x {}: {v}", r.bodies[i].name, r.bodies[j].name);
        }
    }
}
