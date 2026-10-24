//! The finch feeder plug (examples/finch-plug) as a regression suite:
//! the plug is one closed body that sits inside the collar modelled
//! from the photographs with nothing of it outside the tube, its skirt
//! the clearance off the bore, its floor's edge at the lip's underside,
//! its three tabs a hair inside the lip's bore with catches that reach
//! over the lip by the engagement and can bend past it at a strain
//! PETG takes, its drain holes smaller than a seed; and the plug and
//! the collar do not meet once the plug is home. Every number prints
//! (`--nocapture`).

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
fn the_plug_fits_the_collar_with_its_tabs_over_the_lip_and_nothing_outside() {
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
    let (d_mouth, h_rim, t_rim, d_rim) = (
        num(collar, "d_mouth"),
        num(collar, "h_rim"),
        num(collar, "t_rim"),
        num(collar, "d_rim"),
    );
    let (clear, ext, engage, tab_clear) = (
        num(plug, "clear"),
        num(plug, "ext"),
        num(plug, "engage"),
        num(plug, "tab_clear"),
    );
    println!(
        "plug {:.2} across, z {:.1} to {:.1}, {:.1} cm3",
        hi.x - lo.x,
        lo.z,
        hi.z,
        body.volume() / 1000.0
    );
    // Nothing outside the tube: the plug's widest point is inside the
    // bore at the mouth, and it hangs below the mouth only by the skirt.
    assert!(
        (hi.x - lo.x) <= d_mouth - 2.0 * clear + 1e-6,
        "wider than the bore"
    );
    assert!(
        (lo.z + ext).abs() < 1e-6,
        "the skirt hangs {} below the mouth",
        -lo.z
    );
    // The skirt is the clearance off the bore at the mouth (its bottom
    // edge, since the bore does not draft below the mouth), and nothing
    // of the plug above the lip's underside reaches past the lip's bore
    // but the catches, which reach over it by the engagement.
    let at_mouth = radius_where(body, |z| (z - lo.z).abs() < 0.05);
    // (the revolve's facets stand a hair outside the circle they stand for)
    assert!(
        (at_mouth - (d_mouth / 2.0 - clear)).abs() < 0.3,
        "skirt {at_mouth:.3} at the mouth"
    );
    let z_catch = num(plug, "z_catch");
    let above_lip = radius_where(body, |z| z > h_rim + 0.05 && z < z_catch - 0.05);
    assert!(
        above_lip <= d_rim / 2.0 - tab_clear + 0.05,
        "{above_lip:.3} through the lip's bore"
    );
    let catches = radius_where(body, |z| z > z_catch - 1e-6);
    assert!(
        (catches - (d_rim / 2.0 + engage)).abs() < 0.05,
        "catches reach {catches:.3}"
    );
    assert!(
        z_catch > h_rim + t_rim,
        "the catch sits above the lip's top"
    );
    println!(
        "skirt {:.2} at the mouth; tabs {:.2} through a {:.1} lip; catches to {:.2}, {:.2} over it, {:.2} above the lip's top",
        at_mouth * 2.0,
        above_lip * 2.0,
        d_rim,
        catches * 2.0,
        engage,
        z_catch - h_rim - t_rim
    );
    // The tabs bend past the lip at a strain PETG takes more than once.
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
    // Home in the collar, nothing meets: the catches are above the lip,
    // the skirt off the bore, the tabs inside the lip.
    let c = doc
        .regenerate_studio(tab_named(&doc, "collar"), None)
        .unwrap();
    assert_eq!(c.bodies.len(), 1);
    let hit = overlap(body, &c.bodies[0].solid);
    println!("plug in the collar: {hit:.3} mm3 overlap");
    assert!(hit < 1e-3, "the plug meets the collar: {hit} mm3");
    // The floor's top edge sits at the lip's underside: the plug is held
    // between the floor's edge and the catches with the play.
    let top_edge = vertices(body)
        .iter()
        .filter(|v| (v.x * v.x + v.y * v.y).sqrt() > d_mouth / 2.0 - clear - 0.6)
        .map(|v| v.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (top_edge - h_rim).abs() < 1e-6,
        "floor edge at {top_edge:.3} vs the lip at {h_rim}"
    );
    let asm = doc
        .regenerate_assembly(tab_named(&doc, "plug in collar"))
        .unwrap();
    assert_eq!(asm.bodies.len(), 2);
    assert!(asm.instance_errors.is_empty());
}
