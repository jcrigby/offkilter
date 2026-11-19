//! The bucket parts tumbler (examples/bucket-tumbler). The tests check
//! the brief's numbers against the regenerated model:
//! - the bucket's axis is 7.5 in above the deck and its rim clears the
//!   deck by about 1.3 in;
//! - the two casters' wheels touch the wall about 35 deg either side of
//!   bottom centre, about 7.7 in apart, their axles 2 in above the deck,
//!   touching it without cutting into it;
//! - the three battens lie on the inside wall at 120 deg without going
//!   into it;
//! - the motor's shaft passes through the pocketed plate far enough
//!   into the coupling's hub;
//! - propped at 25 deg, the deck rests on the prop leg and nothing goes
//!   below the base.

use ok_brep::{BoolOp, Solid, Surface, Transform};
use ok_math::Vec3;
use ok_model::{Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bucket-tumbler")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/bucket_tumbler.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    let json = std::fs::read_to_string(dir().join("out/bucket_tumbler.json")).unwrap();
    serde_json::from_str(&json).unwrap()
}

/// A number from the JSON, inches to mm.
fn mm(n: &serde_json::Value, key: &str) -> f64 {
    n[key].as_f64().unwrap_or_else(|| panic!("no {key}")) * IN
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn body<'a>(bodies: &'a [Body], name: &str) -> &'a Body {
    bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
}

fn deck_bodies(doc: &mut Document) -> Vec<Body> {
    let r = doc
        .regenerate_studio(tab_named(doc, "on the deck"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    for b in &r.bodies {
        b.solid
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", b.name));
    }
    r.bodies
}

fn common(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

fn moved(s: &Solid, by: Vec3) -> Solid {
    s.transformed(&Transform::translation(by))
}

#[test]
fn the_bucket_turns_on_its_axis_clear_of_the_deck() {
    let (mut doc, n) = (load(), numbers());
    let bodies = deck_bodies(&mut doc);
    let names: Vec<&str> = bodies.iter().map(|b| b.name.as_str()).collect();
    println!("{names:?}");
    assert_eq!(bodies.len(), 17, "the deck's parts");
    let bucket = body(&bodies, "bucket, 5 gal");
    let (lo, hi) = bucket.solid.bounds().unwrap();
    // Revolved about the world x axis: symmetric in y and z.
    assert!((hi.y + lo.y).abs() < 0.01 && (hi.z + lo.z).abs() < 0.01);
    let rim = hi.z;
    println!(
        "bucket {:.2} in long, rim radius {:.2} in",
        (hi.x - lo.x) / IN,
        rim / IN
    );
    assert!(((hi.x - lo.x) - 14.5 * IN).abs() < 0.01);
    assert!((lo.x - mm(&n, "floor_x")).abs() < 0.01);
    // The deck's top face is the axis height below the axis.
    let (_, dhi) = body(&bodies, "deck").solid.bounds().unwrap();
    let axis_h = -dhi.z;
    let clear = axis_h - rim;
    println!(
        "axis {:.2} in above the deck, rim clears it by {:.2} in",
        axis_h / IN,
        clear / IN
    );
    assert!((axis_h - 7.5 * IN).abs() < 0.01);
    assert!((clear - 1.3 * IN).abs() < 0.01);
    // A wall that thin cannot hold much: a hollow shell, most of the
    // revolved envelope empty.
    let v = bucket.solid.volume();
    println!("bucket {:.0} cm3 of HDPE", v / 1000.0);
    assert!(v > 900_000.0 && v < 1_500_000.0, "{v}");
}

#[test]
fn the_wheels_touch_the_wall_at_35_degrees_without_cutting_it() {
    let (mut doc, n) = (load(), numbers());
    let bodies = deck_bodies(&mut doc);
    let bucket = &body(&bodies, "bucket, 5 gal").solid;
    let reach = mm(&n, "wheel_reach");
    let deck_top = -7.5 * IN;
    let mut ys = vec![];
    for side in ["left", "right"] {
        let wheel = &body(&bodies, &format!("caster {side} wheel")).solid;
        // The wheel's tread: a 1 in radius cylinder along x.
        let (origin, axis) = wheel
            .surfaces
            .iter()
            .find_map(|s| match s {
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } if (radius - IN).abs() < 1e-6 => Some((*origin, axis.normalized().unwrap())),
                _ => None,
            })
            .expect("a 2 in wheel");
        assert!(
            axis.x.abs() > 1.0 - 1e-9,
            "the axle parallel to the bucket's axis"
        );
        let (y, z) = (origin.y, origin.z);
        let r = (y * y + z * z).sqrt();
        let angle = y.abs().atan2(-z).to_degrees();
        println!(
            "{side}: axle {:.3} in from the axis at {angle:.1} deg, {:.2} in above the deck",
            r / IN,
            (z - deck_top) / IN
        );
        assert!((r - reach).abs() < 0.01);
        assert!((angle - 35.0).abs() < 0.01);
        assert!(
            (z - deck_top - 2.0 * IN).abs() < 0.05 * IN,
            "the brief's 2 in"
        );
        ys.push(y);
        // Touching: nothing in common, but half a millimetre toward the
        // axis and the wheel is in the wall.
        let c = common(bucket, wheel);
        assert!(c < 1.0, "{side}: {c} mm3 of wheel in the wall");
        let inward = Vec3::new(0.0, -y, -z) * (0.5 / r);
        let pressed = common(bucket, &moved(wheel, inward));
        println!("{side}: pressed 0.5 mm in, {pressed:.1} mm3 in the wall");
        assert!(pressed > 1.0, "the wheel reaches the wall");
        // The caster's bracket stays clear of the wall.
        let bracket = &body(&bodies, &format!("caster {side}")).solid;
        assert!(common(bucket, bracket) < 1e-6);
    }
    let spacing = (ys[1] - ys[0]).abs();
    println!("wheels {:.2} in apart", spacing / IN);
    assert!((spacing - 7.7 * IN).abs() < 0.05 * IN);
    assert!((spacing - mm(&n, "wheel_spacing")).abs() < 0.01);
    // Under the smooth wall, about 5 in below the rim.
    let below_rim = mm(&n, "rim_to_casters");
    assert!(below_rim > 4.5 * IN && below_rim < 5.5 * IN, "{below_rim}");
}

#[test]
fn three_battens_on_the_inside_wall_at_120_degrees() {
    let mut doc = load();
    let bodies = deck_bodies(&mut doc);
    let bucket = &body(&bodies, "bucket, 5 gal").solid;
    let mut angles = vec![];
    for k in 1..=3 {
        let b = &body(&bodies, &format!("batten {k}")).solid;
        let v = b.volume();
        println!("batten {k}: {:.1} cm3", v / 1000.0);
        // 3/4 x 3/4 x 12, its ends cut square to the axis.
        assert!((v - 0.75 * 0.75 * 12.0 * IN.powi(3)).abs() < 0.01 * v);
        let inside = common(bucket, b);
        println!("batten {k}: {inside:.2} mm3 in the wall");
        assert!(inside < 1.0, "batten {k} in the wall");
        let (lo, hi) = b.bounds().unwrap();
        let c = (lo + hi) * 0.5;
        angles.push(c.y.atan2(c.z).to_degrees().rem_euclid(360.0));
    }
    angles.sort_by(f64::total_cmp);
    println!("battens at {angles:?} deg");
    assert!((angles[1] - angles[0] - 120.0).abs() < 0.5);
    assert!((angles[2] - angles[1] - 120.0).abs() < 0.5);
}

#[test]
fn the_shaft_reaches_through_the_plate_into_the_hub() {
    let (mut doc, n) = (load(), numbers());
    let bodies = deck_bodies(&mut doc);
    let motor = &body(&bodies, "gearmotor").solid;
    let plate = &body(&bodies, "motor plate").solid;
    let coupling = &body(&bodies, "flange coupling").solid;
    // The motor sits in its pocket and goes through the plate's hole
    // without touching the plate; the shaft runs inside the hub's bore.
    assert!(common(motor, plate) < 1e-6, "the gearbox clears its pocket");
    assert!(common(motor, coupling) < 1e-6, "the shaft clears the bore");
    let (_, mhi) = motor.bounds().unwrap();
    let (clo, _) = coupling.bounds().unwrap();
    let into = mhi.x - clo.x;
    println!(
        "shaft {:.1} mm into a {:.1} mm hub",
        into,
        mm(&n, "hub_len")
    );
    assert!(into > 6.0, "enough shaft under the set screws");
    assert!((into - mm(&n, "shaft_into_hub")).abs() < 0.01);
    // The pocket leaves a web of 1/4 in plywood around the shaft hole.
    let full = 14.0 * 10.0 * 0.75 * IN.powi(3);
    let pocket = full - plate.volume();
    println!(
        "pocket and hole take {:.0} cm3 from the plate",
        pocket / 1000.0
    );
    assert!(pocket > 0.5 * 58.0 * 40.0 * IN && pocket < 0.5 * 62.0 * 44.0 * IN + 1500.0);
}

#[test]
fn propped_the_deck_rests_on_the_leg_and_nothing_goes_below_the_base() {
    let (mut doc, n) = (load(), numbers());
    let r = doc
        .regenerate_assembly(tab_named(&doc, "propped at 25 deg"))
        .unwrap();
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(r.bodies.len(), 20, "the deck's 17 and the base's 3");
    let deck = &body(&r.bodies, "deck").solid;
    let leg = &body(&r.bodies, "prop leg").solid;
    let (lo, hi) = deck.bounds().unwrap();
    // The hinge end stays on the base; the far end rises 20 sin 25.
    println!(
        "deck x {:.2}..{:.2}, z {:.3}..{:.2} in",
        lo.x / IN,
        hi.x / IN,
        lo.z / IN,
        hi.z / IN
    );
    assert!(lo.z.abs() < 0.01, "the hinge end on the base");
    let rise = mm(&n, "far_end_rise");
    assert!((rise - 20.0 * IN * 25f64.to_radians().sin()).abs() < 0.01);
    assert!(common(deck, leg) < 1.0, "the leg does not go into the deck");
    assert!(
        common(deck, &moved(leg, Vec3::new(0.0, 0.0, 0.5))) > 1.0,
        "the deck rests on it"
    );
    for b in &r.bodies {
        if b.name == "base" || b.name == "hinge" {
            continue;
        }
        let (blo, _) = b.solid.bounds().unwrap();
        assert!(blo.z > -0.01, "{} below the base's top: {}", b.name, blo.z);
    }
}
