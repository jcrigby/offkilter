//! The finch feeder plug (examples/finch-plug) as a regression suite:
//! the plug is one closed body through the ring modelled from the
//! photographs with nothing outside the tube but the flange hidden under
//! the ring, its skirt the clearance off the bore, its floor level with
//! the ring's top, its three tabs a hair inside the ring's top edge with
//! catches that reach over it by the engagement and can bend past it at
//! a strain PETG takes, its drain holes smaller than a seed; and the
//! plug and the ring do not meet once the plug is home. Every number
//! prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Surface};
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

/// The farthest any vertex of the body lies from the z axis, over the
/// heights `keep` picks.
fn radius_where(s: &Solid, keep: impl Fn(f64) -> bool) -> f64 {
    vertices(s)
        .iter()
        .filter(|v| keep(v.z))
        .map(|v| (v.x * v.x + v.y * v.y).sqrt())
        .fold(0.0, f64::max)
}

#[test]
fn the_plug_fits_the_ring_with_its_tabs_over_it_and_nothing_outside_the_tube() {
    let mut doc = load();
    let n = numbers();
    let (collar, plug) = (&n["collar"], &n["plug"]);
    let r = doc
        .regenerate_studio(tab_named(&doc, "plug"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one plug");
    let body = &r.bodies[0].solid;
    body.validate().unwrap();
    let (lo, hi) = bounds(body);
    let (d_mouth, d_top, ring_t, od) = (
        num(collar, "d_mouth"),
        num(collar, "d_top"),
        num(collar, "ring_t"),
        num(collar, "od"),
    );
    let (clear, ext, engage, tab_clear) = (
        num(plug, "clear"),
        num(plug, "ext"),
        num(plug, "engage"),
        num(plug, "tab_clear"),
    );
    let (flange_w, flange_t) = (num(plug, "flange_w"), num(plug, "flange_t"));
    println!(
        "plug {:.2} across, z {:.1} to {:.1}, {:.1} cm3",
        hi.x - lo.x,
        lo.z,
        hi.z,
        body.volume() / 1000.0
    );
    // Nothing outside the tube but the flange, which is under the ring
    // and well inside its outline; below the mouth there is the skirt
    // alone, and above the flange nothing is wider than the bore.
    assert!(
        (hi.x - lo.x) <= d_mouth + 2.0 * flange_w + 1e-6,
        "wider than the flange"
    );
    assert!((hi.x - lo.x) < od - 10.0, "the flange shows past the ring");
    assert!(
        (lo.z + ext).abs() < 1e-6,
        "the skirt hangs {} below the mouth",
        -lo.z
    );
    let flange = radius_where(body, |z| (z + flange_t).abs() < 0.05);
    assert!(
        (flange - (d_mouth / 2.0 + flange_w)).abs() < 0.3,
        "flange {flange:.3}"
    );
    let flange_top = vertices(body)
        .iter()
        .filter(|v| (v.x * v.x + v.y * v.y).sqrt() > d_mouth / 2.0 + flange_w - 0.6)
        .map(|v| v.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        flange_top.abs() < 1e-6,
        "the flange's top at {flange_top:.3}, not the ring's underside"
    );
    // The skirt is the clearance off the bore at the mouth (read at its
    // bottom edge, which is the same radius), and nothing of the plug
    // above the ring's top reaches past the ring's top edge but the
    // catches, which reach over it by the engagement.
    // (the revolve's facets stand a hair outside the circle they stand for)
    let at_mouth = radius_where(body, |z| (z - lo.z).abs() < 0.05);
    assert!(
        (at_mouth - (d_mouth / 2.0 - clear)).abs() < 0.3,
        "skirt {at_mouth:.3} at the mouth"
    );
    let z_catch = num(plug, "z_catch");
    let above_ring = radius_where(body, |z| z > ring_t + 0.05 && z < z_catch - 0.05);
    assert!(
        above_ring <= d_top / 2.0 - tab_clear + 0.05,
        "{above_ring:.3} past the ring's top edge"
    );
    let catches = radius_where(body, |z| z > z_catch - 1e-6);
    assert!(
        (catches - (d_top / 2.0 + engage)).abs() < 0.05,
        "catches reach {catches:.3}"
    );
    assert!(z_catch > ring_t, "the catch sits above the ring's top");
    println!(
        "skirt {:.2} at the mouth, flange {:.2}; catches to {:.2}, {:.2} over the ring's top edge at {:.1}, {:.2} above its top",
        at_mouth * 2.0,
        flange * 2.0,
        catches * 2.0,
        engage,
        d_top,
        z_catch - ring_t
    );
    // The tabs bend past the ring at a strain PETG takes more than once.
    let strain = num(plug, "strain");
    assert!(strain < 0.02, "{} strain", strain);
    assert!(num(plug, "length") > 8.0, "tabs long enough to bend");
    // Drain holes: six, smaller than a nyjer seed, through the floor.
    let drains = cylinders(body, num(plug, "drain_d") / 2.0);
    assert_eq!(
        drains.len(),
        num(plug, "drains") as usize,
        "{} drains",
        drains.len()
    );
    assert!(num(plug, "drain_d") <= 1.0);
    // Home in the ring, nothing meets: the flange under it, the catches
    // above it, the skirt off the bore, the tabs inside its top edge.
    let c = doc
        .regenerate_studio(tab_named(&doc, "collar"), None)
        .unwrap();
    assert_eq!(c.bodies.len(), 1);
    let hit = overlap(body, &c.bodies[0].solid);
    println!("plug in the collar: {hit:.3} mm3 overlap");
    assert!(hit < 1e-3, "the plug meets the collar: {hit} mm3");
    // The floor's top edge is level with the ring's top, so the lowest
    // seed lies at the first row of mesh: the highest vertex at the
    // floor's edge between two tabs, where the skirt's top meets the cone.
    let top_edge = vertices(body)
        .iter()
        .filter(|v| {
            let r = (v.x * v.x + v.y * v.y).sqrt();
            let a = v.y.atan2(v.x).to_degrees();
            v.z > 0.0
                && r > d_top / 2.0 - clear - 0.5
                && r < d_top / 2.0 + 0.3
                && (a - 60.0).abs() < 25.0
        })
        .map(|v| v.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (top_edge - ring_t).abs() < 1e-6,
        "floor edge at {top_edge:.3} vs the ring's top at {ring_t}"
    );
    let asm = doc
        .regenerate_assembly(tab_named(&doc, "plug in collar"))
        .unwrap();
    assert_eq!(asm.bodies.len(), 2);
    assert!(asm.instance_errors.is_empty());
    // The gauge: a stepped tube, the smallest step at the top, every step
    // a quarter millimetre bigger than the one above it, so the ring's
    // bore can be found by feel before the plug is printed.
    let gauge = &n["gauge"];
    let gr = doc
        .regenerate_studio(tab_named(&doc, "gauge"), None)
        .unwrap();
    assert_eq!(gr.bodies.len(), 1);
    let gsolid = &gr.bodies[0].solid;
    gsolid.validate().unwrap();
    let steps: Vec<f64> = gauge["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let (step_h, gmin, gmax) = (num(gauge, "h"), num(gauge, "min"), num(gauge, "max"));
    let (glo, ghi) = bounds(gsolid);
    assert!((ghi.z - glo.z - step_h * steps.len() as f64).abs() < 1e-6);
    for (i, d) in steps.iter().rev().enumerate() {
        let z = (i as f64 + 0.5) * step_h;
        let r = radius_where(gsolid, |zz| (zz - z).abs() < step_h / 2.0 - 0.8);
        // The grooves cut into the whole-mm steps, so read a step's
        // radius from its ungrooved band; the revolve's facets add a hair.
        let got = if r > 0.0 {
            r
        } else {
            radius_where(gsolid, |zz| (zz - (z - step_h / 2.0)).abs() < 0.05)
        };
        assert!(
            (got - d / 2.0).abs() < 0.3,
            "step {i} reads {:.3} for {d}",
            got * 2.0
        );
    }
    assert!((steps[0] - gmin).abs() < 1e-9 && (steps[steps.len() - 1] - gmax).abs() < 1e-9);
    assert!(
        gmin < d_top && gmax > d_mouth,
        "the gauge brackets the bore as modelled"
    );
    println!(
        "gauge: {} steps, {:.1} tall, {} to {}",
        steps.len(),
        ghi.z - glo.z,
        gmin,
        gmax
    );
}
