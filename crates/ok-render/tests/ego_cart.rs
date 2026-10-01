//! The EGO cart (examples/ego-cart) as a regression suite: the document
//! regenerates closed, the gear train meshes, the load sits where it
//! should, and nothing is placed inside anything else.

use ok_brep::{BoolOp, Solid};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, TabId};
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

/// Pairs that touch by design: the bevel pinion's pitch circle on the
/// bevel gear's, and the bags' feet on the platform.
const KNOWN: &[(&str, &str)] = &[
    ("bevel pinion", "bevel gear"),
    ("left bag", "platform"),
    ("right bag", "platform"),
];

#[test]
fn every_part_regenerates_closed_and_the_cart_places_twenty_bodies() {
    let mut doc = load();
    let tabs: Vec<(TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    assert_eq!(tabs.len(), 16, "fifteen parts and the cart");
    for (id, name, kind) in &tabs {
        if kind == "part_studio" {
            let r = doc.regenerate_studio(*id, None).unwrap();
            assert_eq!(r.bodies.len(), 1, "{name}: one body");
            assert!(r.bodies[0].solid.volume() > 0.0, "{name} has volume");
            assert!(
                r.statuses.iter().all(|f| f.error.is_none()),
                "{name}: {:?}",
                r.statuses
            );
        }
    }
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    assert_eq!(r.bodies.len(), 20);
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
}

#[test]
fn the_gear_train_meshes_and_gears_down_fifty_to_one() {
    let mut doc = load();
    // Tooth counts from the gear features' names: "63t gear, module 2.5".
    let teeth = |name: &str| -> f64 {
        let tab = tab_named(&doc, name);
        doc.studio(tab)
            .unwrap()
            .features()
            .iter()
            .find_map(|f| {
                f.name
                    .strip_suffix(&f.name[f.name.find("t gear")?..])
                    .and_then(|n| n.parse::<f64>().ok())
            })
            .unwrap_or_else(|| panic!("{name}: no feature named like 63t gear"))
    };
    let ratio = teeth("Bevel gear") / teeth("Bevel pinion") * teeth("Gear 2") / teeth("Pinion")
        * teeth("Axle gear")
        / teeth("Pinion");
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    assert!((ratio - 50.4).abs() < 1e-9, "{ratio}");
    // Each spur pair's axes are a pitch radius sum apart: the discs are
    // the pitch cylinders, so their radii from the bounds, their axes
    // from the centroids.
    let radius = |s: &Solid| {
        let (lo, hi) = bounds(s);
        (hi.y - lo.y) / 2.0
    };
    let axis_y = |s: &Solid| s.centroid().unwrap().y;
    for (a, b) in [("pinion 3", "axle gear"), ("pinion 2", "gear 2")] {
        let (pa, pb) = (body(&r, a), body(&r, b));
        let want = radius(pa) + radius(pb);
        let got = (axis_y(pa) - axis_y(pb)).abs();
        assert!((got - want).abs() < 1e-6, "{a} to {b}: {got} vs {want}");
    }
    // The bevels' pitch circles touch: the gear's edge at the apex's
    // height meets the pinion's edge, each a pitch radius from the apex.
    let (bp, bg) = (body(&r, "bevel pinion"), body(&r, "bevel gear"));
    let apex = Vec3::new(0.0, axis_y(bg), bg.centroid().unwrap().z);
    let pinion_c = bp.centroid().unwrap();
    let along = (pinion_c - apex).length();
    assert!(
        (along - (radius(bg) + 6.0)).abs() < 1e-3,
        "the pinion's centre sits a gear radius plus half its face from the apex: {along}"
    );
}

#[test]
fn the_load_sits_on_the_platform_and_nothing_is_inside_anything_else() {
    let mut doc = load();
    let cart = tab_named(&doc, "Cart");
    let r = doc.regenerate_assembly(cart).unwrap();
    let (plo, phi) = bounds(body(&r, "platform"));
    for bag in ["left bag", "right bag"] {
        let (lo, hi) = bounds(body(&r, bag));
        assert!((lo.z - phi.z).abs() < 1e-9, "{bag} stands on the platform");
        assert!(
            lo.x >= plo.x && hi.x <= phi.x && lo.y >= plo.y && hi.y <= phi.y,
            "{bag} inside the platform's edges"
        );
    }
    // The platform clears the tyres; the gearbox clears the ground.
    let tyre_top = bounds(body(&r, "left wheel")).1.z;
    assert!(
        plo.z - tyre_top >= 30.0,
        "platform {} over the tyre {}",
        plo.z,
        tyre_top
    );
    assert!(
        bounds(body(&r, "gearbox")).0.z >= 40.0 - 1e-9,
        "ground clearance"
    );
    // The wheels stand on the ground; the head's motor block, where the
    // rear grip is, tops out about where a hand hangs.
    assert!(bounds(body(&r, "left wheel")).0.z.abs() < 1e-9);
    let grip = bounds(body(&r, "power head")).1.z;
    assert!((950.0..1250.0).contains(&grip), "grip height {grip}");
    // Every pair of placed bodies: no overlap but the known contacts,
    // and those by a negligible volume.
    let mut worst: Vec<(String, String, f64)> = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            let (a, b) = (&r.bodies[i], &r.bodies[j]);
            let known = KNOWN
                .iter()
                .any(|(p, q)| (a.name == *p && b.name == *q) || (a.name == *q && b.name == *p));
            match overlap(&a.solid, &b.solid) {
                Some(v) if v > 1e-3 && !known => worst.push((a.name.clone(), b.name.clone(), v)),
                Some(v) if known => {
                    assert!(v < 50.0, "{} / {}: {v} mm3 at a contact", a.name, b.name)
                }
                None => worst.push((a.name.clone(), b.name.clone(), f64::NAN)),
                _ => {}
            }
        }
    }
    assert!(worst.is_empty(), "{worst:?}");
}
