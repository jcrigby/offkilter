//! The finch feeder cap (examples/finch-plug) as a regression suite:
//! the cap is one closed body whose plug sits in the ring's bore the
//! clearance off it with its top level with the ring's, whose plate
//! reaches the ring's edge and no further but for the hook lobes, whose
//! three hooks stand outside the ears' tips with lips that come down
//! along the arc from the entry clearance to touching, and which meets
//! the ring nowhere when locked or when offered up before the turn; the
//! drain holes are smaller than a seed; and the gauge's three rings read
//! their diameters and bracket the bore. Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Surface, Transform};
use ok_math::Vec3;
use ok_model::{Document, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/finch-plug")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/finch_plug.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/plug.json")).unwrap()).unwrap()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no {k}"))
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// Cylinders of about radius `r` on a body: their axis origins.
fn cylinders(s: &Solid, r: f64) -> Vec<Vec3> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder { origin, radius, .. } if (radius - r).abs() < 1e-6 => Some(*origin),
            _ => None,
        })
        .collect()
}

/// Every vertex of the body's faces.
fn vertices(s: &Solid) -> Vec<Vec3> {
    s.polygons()
        .iter()
        .flat_map(|p| p.loops.iter().flatten().copied().collect::<Vec<_>>())
        .collect()
}

/// The farthest any vertex lies from the z axis, over the vertices
/// `keep` picks.
fn radius_where(s: &Solid, keep: impl Fn(&Vec3) -> bool) -> f64 {
    vertices(s)
        .iter()
        .filter(|v| keep(v))
        .map(|v| (v.x * v.x + v.y * v.y).sqrt())
        .fold(0.0, f64::max)
}

fn angle_of(v: &Vec3) -> f64 {
    v.y.atan2(v.x).to_degrees().rem_euclid(360.0)
}

#[test]
fn the_cap_plugs_the_ring_and_its_hooks_take_the_ears_with_room_to_offer_up() {
    let mut doc = load();
    let n = numbers();
    let (collar, cap) = (&n["collar"], &n["cap"]);
    let r = doc.regenerate_studio(tab_named(&doc, "cap"), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one cap");
    let body = &r.bodies[0].solid;
    body.validate().unwrap();
    let (lo, hi) = bounds(body);
    let (d_mouth, d_top, ring_t, od, ear_at) = (
        num(collar, "d_mouth"),
        num(collar, "d_top"),
        num(collar, "ring_t"),
        num(collar, "od"),
        num(collar, "ear_at"),
    );
    let ear_t = num(collar, "ear_t");
    let (clear, plate_t, plate_r) = (num(cap, "clear"), num(cap, "plate_t"), num(cap, "plate_r"));
    let (r_ear, r_wall0, r_wall1, r_lip, hook_h, hook_arc) = (
        num(cap, "r_ear"),
        num(cap, "r_wall0"),
        num(cap, "r_wall1"),
        num(cap, "r_lip"),
        num(cap, "hook_h"),
        num(cap, "hook_arc"),
    );
    let ramp: Vec<f64> = cap["ramp"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    println!(
        "cap {:.1} across, z {:.1} to {:.1}, {:.1} cm3",
        hi.x - lo.x,
        lo.z,
        hi.z,
        body.volume() / 1000.0
    );
    // The plate: under the ring from its mouth down, to the ring's edge
    // and no further but for the three lobes that carry the hooks.
    assert!((lo.z + plate_t).abs() < 1e-6);
    assert!(
        (plate_r - od / 2.0).abs() < 1e-9,
        "the plate reaches the ring's edge"
    );
    let hooks: Vec<f64> = cap["hook_at"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(hooks.len(), 3);
    // Locked, each ear's leading edge rests a hair short of its hook's
    // stop; offered up, the ears sit mid-gap.
    let (ear_half, shift) = (num(cap, "ear_half"), num(cap, "hook_shift"));
    assert!((hooks[0] - ear_at - shift).abs() < 1e-9);
    assert!(
        (ear_at + ear_half + 0.5 - (hooks[0] + hook_arc / 2.0)).abs() < 1e-9,
        "the ear's edge against the stop"
    );
    assert!(
        120.0 - hook_arc > 2.0 * ear_half + 10.0,
        "the gap takes an ear with room"
    );
    let near_hook = |v: &Vec3| {
        hooks.iter().any(|h| {
            ((angle_of(v) - h + 180.0).rem_euclid(360.0) - 180.0).abs() <= hook_arc / 2.0 + 2.5
        })
    };
    let plate_between = radius_where(body, |v| {
        v.z < -1e-6 && v.z > -plate_t + 1e-6 && !near_hook(v)
    });
    assert!(
        plate_between <= plate_r + 0.3,
        "plate {plate_between:.2} between the hooks"
    );
    assert!(
        (hi.x - lo.x) <= 2.0 * r_wall1 + 1e-6,
        "wider than the hook walls"
    );
    // The plug: the clearance off the bore at the mouth and its top level
    // with the ring's top at the edge, rising inside.
    let at_mouth = radius_where(body, |v| {
        v.z.abs() < 1e-6 && (v.x * v.x + v.y * v.y).sqrt() < od / 2.0 - 2.0
    });
    assert!(
        (at_mouth - (d_mouth / 2.0 - clear)).abs() < 0.3,
        "plug {at_mouth:.3} at the mouth"
    );
    let top_edge = vertices(body)
        .iter()
        .filter(|v| {
            let rr = (v.x * v.x + v.y * v.y).sqrt();
            v.z > 0.0 && rr > d_top / 2.0 - clear - 0.6 && rr < d_top / 2.0 + 0.3
        })
        .map(|v| v.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (top_edge - ring_t).abs() < 1e-6,
        "plug's edge at {top_edge:.3} vs the ring's top at {ring_t}"
    );
    // The hooks: walls outside the ears' tips, lips reaching in over them,
    // the underside of each step where the ramp says, down to touching.
    for h in &hooks {
        let in_hook = |v: &Vec3| {
            ((angle_of(v) - h + 180.0).rem_euclid(360.0) - 180.0).abs() <= hook_arc / 2.0 + 0.1
        };
        let wall = radius_where(body, |v| in_hook(v) && v.z > ear_t + 0.1);
        assert!(
            (wall - r_wall1).abs() < 0.3,
            "hook at {h}: wall to {wall:.2}"
        );
        let lip_in = vertices(body)
            .iter()
            .filter(|v| {
                in_hook(v)
                    && v.z > ear_t - 1e-6
                    && v.z < hook_h - 1e-6
                    && (v.x * v.x + v.y * v.y).sqrt() > od / 2.0 - 1.0
            })
            .map(|v| (v.x * v.x + v.y * v.y).sqrt())
            .fold(f64::INFINITY, f64::min);
        assert!(
            (lip_in - r_lip).abs() < 0.3,
            "hook at {h}: lip in to {lip_in:.2}, over the ear tip at {r_ear}"
        );
        assert!(r_wall0 > r_ear + 0.3, "the wall clears the ear's tip");
        // The steps' undersides, lowest first toward the stop.
        let mut undersides: Vec<f64> = vertices(body)
            .iter()
            .filter(|v| {
                let rr = (v.x * v.x + v.y * v.y).sqrt();
                in_hook(v)
                    && rr > od / 2.0 - 1.0
                    && rr < r_wall0 - 0.5
                    && v.z > ear_t - 1e-6
                    && v.z < ear_t + ramp[0] + 1e-6
            })
            .map(|v| (v.z * 1000.0).round() / 1000.0)
            .collect();
        undersides.sort_by(f64::total_cmp);
        undersides.dedup();
        let want: Vec<f64> = ramp
            .iter()
            .map(|c| ((ear_t + c) * 1000.0).round() / 1000.0)
            .collect();
        let mut want_sorted = want.clone();
        want_sorted.sort_by(f64::total_cmp);
        assert_eq!(undersides, want_sorted, "hook at {h}: lip steps");
    }
    assert!(
        ramp.windows(2).all(|w| w[1] < w[0]) && ramp[ramp.len() - 1].abs() < 1e-9,
        "the ramp comes down to touching"
    );
    // Drain holes, smaller than a nyjer seed.
    let drains = cylinders(body, num(cap, "drain_d") / 2.0);
    assert_eq!(drains.len(), num(cap, "drains") as usize);
    assert!(num(cap, "drain_d") <= 1.0);
    // On the ring, locked, nothing meets: the ears sit under the lips
    // with the last step touching. Offered up, turned by the gap, the
    // ears pass between the hooks.
    let c = doc
        .regenerate_studio(tab_named(&doc, "collar"), None)
        .unwrap();
    assert_eq!(c.bodies.len(), 1);
    let ring = &c.bodies[0].solid;
    let hit = overlap(body, ring);
    println!("locked: {hit:.3} mm3 overlap");
    assert!(hit < 1e-3, "the cap meets the ring locked: {hit} mm3");
    let offered = body.transformed(&Transform::rotation(
        Vec3::ZERO,
        Vec3::Z,
        num(cap, "offer").to_radians(),
    ));
    let hit = overlap(&offered, ring);
    println!("offered up: {hit:.3} mm3 overlap");
    assert!(
        hit < 1e-3,
        "the ears do not pass between the hooks: {hit} mm3"
    );
    for name in ["cap on collar", "cap offered up"] {
        let asm = doc.regenerate_assembly(tab_named(&doc, name)).unwrap();
        assert_eq!(asm.bodies.len(), 2);
        assert!(asm.instance_errors.is_empty());
    }
    // The gauge: three rings in one print, one under the bore by rule, one
    // at it, one over, each a thin open ring of its diameter, notched to
    // tell apart.
    let gauge = &n["gauge"];
    let gr = doc
        .regenerate_studio(tab_named(&doc, "gauge"), None)
        .unwrap();
    let sizes: Vec<f64> = gauge["sizes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(gr.bodies.len(), sizes.len());
    let (gh, gwall) = (num(gauge, "h"), num(gauge, "wall"));
    let mut rings: Vec<(f64, f64, f64)> = gr
        .bodies
        .iter()
        .map(|b| {
            b.solid.validate().unwrap();
            let (lo, hi) = bounds(&b.solid);
            (hi.x - lo.x, hi.z - lo.z, b.solid.volume())
        })
        .collect();
    rings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for ((d, h, vol), size) in rings.iter().zip(&sizes) {
        assert!((d - size).abs() < 0.3, "ring reads {d:.2} for {size}");
        assert!((h - gh).abs() < 1e-6);
        let full =
            std::f64::consts::PI * (size / 2.0 * size / 2.0 - (size / 2.0 - gwall).powi(2)) * gh;
        assert!(
            *vol < full && *vol > 0.9 * full,
            "ring {size} volume {vol:.1} of {full:.1}: a thin open ring"
        );
    }
    let (gmin, gmax) = (sizes[0], sizes[sizes.len() - 1]);
    assert!(
        gmin <= d_top && gmax > d_mouth,
        "the gauge brackets the bore as modelled"
    );
    println!("gauge: rings of {:?}, {gh} tall, {gwall} wall", sizes);
}
