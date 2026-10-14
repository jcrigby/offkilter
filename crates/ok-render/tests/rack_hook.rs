//! The wire rack coat hook (examples/rack-hook): one printed band
//! regenerates closed at its sizes, a U of the slot's radii about the
//! rail's axis, the two hooks of their radii down the front leg, every
//! bend a cylinder along the rail; on the rail it clears the top rail
//! all round and hangs in front of the lower one; and the STL is
//! written on its side for the bed.

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::Vec3;
use ok_model::{Body, Document, TabId};
use std::path::PathBuf;

const RAIL: f64 = 0.25 * 25.4;
const SLOT: f64 = 7.5;
const T: f64 = 4.0;
const W: f64 = 18.0;
const BACK: f64 = 25.0;
const LEG: f64 = 110.0;
const HOOK_R: f64 = 10.0;
const TIP: f64 = 20.0;
const LOWER_RAIL: f64 = 25.0;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/rack-hook")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/rack_hook.okpart")).unwrap();
    Document::from_json(&json).unwrap()
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

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Cylinders of about radius `r` (mm): a point on the axis and the
/// unit axis.
fn cylinders(s: &Solid, r: f64) -> Vec<(Vec3, Vec3)> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder {
                origin,
                axis,
                radius,
            } if (radius - r).abs() < 1e-6 => Some((*origin, axis.normalized().unwrap())),
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
fn the_band_is_one_body_with_the_slot_the_legs_and_two_hooks() {
    let mut doc = load();
    let r = doc
        .regenerate_studio(tab_named(&doc, "hook"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one band");
    let g = &r.bodies[0];
    g.solid.validate().unwrap();
    assert_eq!(g.material.as_ref().map(|m| m.name.as_str()), Some("PETG"));
    // The envelope: the band's width along the rail, the U's outer
    // radius behind the rail and above it, the hooks' tips out in
    // front, the bottom hook's outside below the leg's end.
    let (lo, hi) = bounds(&g.solid);
    let (ri, ro) = (SLOT / 2.0, SLOT / 2.0 + T);
    assert!(near(lo.x, 0.0) && near(hi.x, W), "x {}..{}", lo.x, hi.x);
    assert!(
        near(lo.y, -ro) && near(hi.y, ro + 2.0 * HOOK_R + T),
        "y {}..{}",
        lo.y,
        hi.y
    );
    assert!(
        near(hi.z, ro) && near(lo.z, -LEG - HOOK_R - T),
        "z {}..{}",
        lo.z,
        hi.z
    );
    // The U about the rail's axis: an inner cylinder of the slot's
    // radius and an outer one a band thickness bigger, both on the x
    // axis through the origin.
    for (radius, what) in [(ri, "the slot"), (ro, "the U's outside")] {
        let cs = cylinders(&g.solid, radius);
        assert_eq!(cs.len(), 1, "{what}: {} cylinders of r {radius}", cs.len());
        let (o, a) = cs[0];
        assert!(
            a.x.abs() > 0.999 && near(o.y, 0.0) && near(o.z, 0.0),
            "{what} on the rail's axis: {o:?}"
        );
    }
    // The hooks: inner and outer cylinders of their radii, centred a
    // hook radius out from the front leg's face, at half the leg and
    // at its end.
    let y_hook = ro + HOOK_R;
    for (radius, what) in [(HOOK_R, "inside"), (HOOK_R + T, "outside")] {
        let mut cs = cylinders(&g.solid, radius);
        assert_eq!(cs.len(), 2, "hooks {what}: {}", cs.len());
        cs.sort_by(|a, b| b.0.z.total_cmp(&a.0.z));
        for ((o, a), zc) in cs.iter().zip([LEG / 2.0, LEG]) {
            assert!(
                a.x.abs() > 0.999 && near(o.y, y_hook) && near(o.z, -zc),
                "hook {what} at {o:?}"
            );
        }
    }
    // Every bend and rounded end is a cylinder along the rail, and the
    // rest planes: nothing to overhang when the profile lies on the bed.
    assert!(
        g.solid.surfaces.iter().all(|s| match s {
            Surface::Plane { .. } => true,
            Surface::Cylinder { axis, .. } => axis.normalized().unwrap().x.abs() > 0.999,
            _ => false,
        }),
        "planes and cylinders along the rail only"
    );
    assert_eq!(cylinders(&g.solid, T / 2.0).len(), 3, "three rounded ends");
    // The volume: the band's profile by its width; the bends are
    // faceted so a few percent under the arcs' area is right.
    let pi = std::f64::consts::PI;
    let area = pi * (ro * ro - ri * ri) / 2.0
        + BACK * T
        + LEG * T
        + 2.0 * (pi * ((HOOK_R + T).powi(2) - HOOK_R * HOOK_R) / 2.0 + TIP * T)
        + 3.0 * pi * (T / 2.0).powi(2) / 2.0;
    let v = g.solid.volume();
    println!("band: {v:.0} mm3, {:.0} by the profile", area * W);
    assert!(v < area * W && v > 0.96 * area * W, "{v}");
}

#[test]
fn on_the_rail_the_u_clears_the_top_rail_and_the_leg_hangs_before_the_lower_one() {
    let mut doc = load();
    let r = doc
        .regenerate_assembly(tab_named(&doc, "on the rail"))
        .unwrap();
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(r.bodies.len(), 3, "two rails and the hook");
    let top = body(&r.bodies, "top rail");
    let lower = body(&r.bodies, "lower rail");
    let hook = body(&r.bodies, "hook");
    let (tlo, thi) = bounds(&top.solid);
    assert!(
        near(thi.y - tlo.y, RAIL) && near((thi.z + tlo.z) / 2.0, 0.0),
        "the top rail on the x axis"
    );
    let (llo, lhi) = bounds(&lower.solid);
    assert!(near((lhi.z + llo.z) / 2.0, -LOWER_RAIL) && near((lhi.y + llo.y) / 2.0, 0.0));
    // The rail sits in the slot with clearance all round, and the hook
    // touches neither rail: the booleans find nothing in common.
    for rail in [top, lower] {
        let common = ok_brep::boolean(&hook.solid, &rail.solid, BoolOp::Intersection)
            .map(|s| s.volume().abs())
            .unwrap_or(0.0);
        assert!(
            common < 1e-6,
            "{}: {common} mm3 in common with the hook",
            rail.name
        );
    }
    let gap = (SLOT - RAIL) / 2.0;
    println!("slot {SLOT} over a {RAIL:.2} rail: {gap:.2} mm each side");
    assert!(gap > 0.5, "room to drop on");
    // The front leg hangs just in front of the lower rail, so a load
    // on the hooks leans it there.
    let (hlo, _) = bounds(&hook.solid);
    let front_face = SLOT / 2.0;
    assert!(
        front_face > lhi.y - 1e-6 && front_face - lhi.y < 1.0,
        "the leg {} in front of the lower rail at {}",
        front_face,
        lhi.y
    );
    assert!(hlo.z < -LOWER_RAIL, "the leg reaches past the lower rail");
    // The STL: the band on its side, its width standing 0 to 18.
    let bytes = std::fs::read(dir().join("out/hook.stl")).unwrap();
    let tris = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(
        bytes.len(),
        84 + 50 * tris,
        "a binary STL of {tris} triangles"
    );
    let (mut zlo, mut zhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in 0..tris {
        let rec = &bytes[84 + 50 * t..84 + 50 * t + 48];
        for v in 0..3 {
            let at = 12 + 12 * v + 8;
            let z = f32::from_le_bytes(rec[at..at + 4].try_into().unwrap()) as f64;
            zlo = zlo.min(z);
            zhi = zhi.max(z);
        }
    }
    println!("stl: {tris} triangles, z {zlo:.3}..{zhi:.3}");
    assert!(
        zlo.abs() < 1e-3 && (zhi - W).abs() < 1e-3,
        "on its side: z {zlo}..{zhi}"
    );
}
