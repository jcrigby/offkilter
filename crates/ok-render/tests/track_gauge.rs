//! The track saw edge gauge (examples/track-gauge): the fence and base
//! regenerate closed at their sizes, the base 2 in plus the kerf wide
//! from the fence's inner face with its dust groove and pilot holes,
//! the fence with its two countersunk holes; and in use the sheet
//! edge, the gauge, the track's edge and the kerf line up so the strip
//! the blade leaves is exactly 2 in.

use ok_brep::{Solid, Surface};
use ok_math::Vec3;
use ok_model::{Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;
const WIDTH: f64 = 2.0;
const KERF: f64 = 2.2 / IN;
const T: f64 = 0.75;
const LEN: f64 = 4.0;

fn load() -> Document {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/track-gauge");
    let json = std::fs::read_to_string(dir.join("out/track_gauge.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

/// Bounds in inches.
fn bounds(s: &Solid) -> (Vec3, Vec3) {
    let (lo, hi) = s.bounds().unwrap();
    (lo * (1.0 / IN), hi * (1.0 / IN))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Cylinders of about radius `r` (inches) and their unit axes.
fn cylinders(s: &Solid, r: f64) -> Vec<Vec3> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder { axis, radius, .. } if (radius - r * IN).abs() < 1e-6 => {
                Some(axis.normalized().unwrap_or(Vec3::X))
            }
            _ => None,
        })
        .collect()
}

fn body<'a>(bodies: &'a [Body], name: &str) -> &'a Body {
    bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
}

#[test]
fn the_fence_and_base_regenerate_at_their_sizes_with_the_groove_and_holes() {
    let mut doc = load();
    let r = doc
        .regenerate_studio(tab_named(&doc, "gauge"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 2, "fence and base");
    for b in &r.bodies {
        b.solid.validate().unwrap();
        assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("maple"));
    }
    let fence = &r.bodies[0];
    let base = &r.bodies[1];
    assert!(fence.name.starts_with("fence") && base.name.starts_with("base"));
    // The fence: 3/4 thick outside the sheet edge, 1-1/2 tall, hanging
    // 3/4 below the base's underside; two #8 clearance holes along x
    // with their countersinks (cones, not cylinders) on its outer face.
    let (lo, hi) = bounds(&fence.solid);
    assert!(
        near(lo.x, -T) && near(hi.x, 0.0),
        "fence x {}..{}",
        lo.x,
        hi.x
    );
    assert!(near(lo.y, 0.0) && near(hi.y, LEN));
    assert!(
        near(lo.z, -T) && near(hi.z, T),
        "fence z {}..{}",
        lo.z,
        hi.z
    );
    let holes = cylinders(&fence.solid, 0.17 / 2.0);
    assert_eq!(holes.len(), 2, "two clearance holes");
    assert!(holes.iter().all(|a| a.x.abs() > 1.0 - 1e-9), "along x");
    assert!(
        fence
            .solid
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Cone { .. }))
            .count()
            >= 2,
        "countersunk"
    );
    let v = fence.solid.volume() / IN.powi(3);
    println!("fence {v:.3} in3");
    assert!(
        v > 4.44 && v < 4.47,
        "{v}: 4.5 less the two countersunk holes"
    );
    // The base: from the fence's inner face out 2 in plus the kerf,
    // 3/4 thick on the sheet, with the 1/8 dust groove along its inside
    // bottom edge and two 7/64 pilots in its edge.
    let (lo, hi) = bounds(&base.solid);
    assert!(
        near(lo.x, 0.0) && near(hi.x, WIDTH + KERF),
        "base x {}..{}",
        lo.x,
        hi.x
    );
    assert!(near(hi.x * IN, 53.0), "53.0 mm: 50.8 and a 2.2 kerf");
    assert!(near(lo.z, 0.0) && near(hi.z, T) && near(lo.y, 0.0) && near(hi.y, LEN));
    let pilots = cylinders(&base.solid, 0.109 / 2.0);
    assert_eq!(pilots.len(), 2, "two pilots");
    assert!(pilots.iter().all(|a| a.x.abs() > 1.0 - 1e-9));
    let want = (WIDTH + KERF) * T * LEN
        - 0.125 * 0.125 * LEN
        - 2.0 * std::f64::consts::PI * (0.109_f64 / 2.0).powi(2) * T;
    let v = base.solid.volume() / IN.powi(3);
    println!("base {v:.4} in3, {want:.4} wanted");
    assert!((v - want).abs() < 1e-3, "{v}: the groove and the pilots");
    // The groove is at the inside bottom edge: the base's underside
    // starts 1/8 out from the fence, and nothing of it lies in the
    // groove's corner.
    let in_groove = base.solid.surfaces.iter().any(|s| match s {
        Surface::Plane { normal, offset } => {
            normal.z.abs() > 0.999 && near((offset * normal.z).abs(), 0.125 * IN)
        }
        _ => false,
    });
    assert!(in_groove, "the groove's ceiling at z = 1/8");
}

#[test]
fn in_use_the_track_sits_on_the_gauge_and_the_kerf_leaves_two_inches() {
    let mut doc = load();
    let r = doc.regenerate_assembly(tab_named(&doc, "in use")).unwrap();
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(r.bodies.len(), 7, "sheet, track, kerf, two gauges");
    let sheet = bounds(&body(&r.bodies, "sheet").solid);
    let track = bounds(&body(&r.bodies, "track").solid);
    let kerf = bounds(&body(&r.bodies, "kerf").solid);
    // The sheet's edge is x = 0 and its top z = 0; the kerf is a 2.2 mm
    // slot through it starting exactly 2 in from the edge; the track's
    // edge is at the kerf's far side.
    assert!(near(sheet.0.x, 0.0) && near(sheet.1.z, 0.0));
    assert!(
        near(kerf.0.x, WIDTH) && near(kerf.1.x, WIDTH + KERF),
        "kerf {:?}",
        kerf
    );
    assert!(
        near(kerf.0.z, sheet.0.z) && near(kerf.1.z, sheet.1.z),
        "through the sheet"
    );
    assert!(
        near(track.0.x, kerf.1.x),
        "the track's edge on the kerf's far side"
    );
    for n in 1..=2 {
        let fence = bounds(&body(&r.bodies, &format!("gauge {n} / fence")).solid);
        let base = bounds(&body(&r.bodies, &format!("gauge {n} / base")).solid);
        // Hooked on: the fence's inner face on the sheet's edge, the
        // base on the sheet's top, its face where the track's edge is.
        assert!(near(fence.1.x, sheet.0.x), "gauge {n}: fence on the edge");
        assert!(near(base.0.z, sheet.1.z), "gauge {n}: base on the sheet");
        assert!(
            near(base.1.x, track.0.x),
            "gauge {n}: the track against its face"
        );
        assert!(
            base.0.y >= track.0.y && base.1.y <= track.1.y,
            "gauge {n}: within the track's length"
        );
    }
    println!(
        "strip: {:.4} in, kerf {:.2} mm, track edge at {:.4} in",
        kerf.0.x - sheet.0.x,
        (kerf.1.x - kerf.0.x) * IN,
        track.0.x
    );
}
