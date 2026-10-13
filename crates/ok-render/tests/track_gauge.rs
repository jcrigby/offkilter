//! The track saw edge gauge (examples/track-gauge): one printed L,
//! extruded along the edge, regenerates closed at its sizes, the depth
//! leg 2 in plus the kerf long from the fence's inner face with its
//! dust groove, its corners rounded, its labels engraved in the top,
//! every other face square to the axes so it prints flat, and the STL
//! is written; in use the sheet edge, the gauge, the track's
//! edge and the kerf line up so the strip the blade leaves is exactly
//! 2 in.

use ok_brep::{Solid, Surface};
use ok_math::Vec3;
use ok_model::{Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;
const WIDTH: f64 = 2.0;
const KERF: f64 = 2.2 / IN;
const T: f64 = 6.0 / IN;
const FENCE_T: f64 = 6.0 / IN;
const LEN: f64 = 60.0 / IN;
const DROP: f64 = 12.0 / IN;
const GROOVE: f64 = 2.0 / IN;
const ROUND: f64 = 4.0 / IN;
const ENGRAVE: f64 = 0.6 / IN;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/track-gauge")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/track_gauge.okpart")).unwrap();
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

fn body<'a>(bodies: &'a [Body], name: &str) -> &'a Body {
    bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
}

#[test]
fn the_gauge_is_one_printed_l_at_its_sizes_with_the_groove() {
    let mut doc = load();
    let r = doc
        .regenerate_studio(tab_named(&doc, "gauge"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one piece to print");
    let g = &r.bodies[0];
    g.solid.validate().unwrap();
    assert_eq!(g.material.as_ref().map(|m| m.name.as_str()), Some("PETG"));
    // The envelope: the fence 6 mm outside the sheet edge, hanging 12
    // below the depth leg's underside and flush with its top; the
    // depth leg 6 thick from the fence's inner face out 2 in plus the
    // kerf, 53.0 mm; 60 along the edge, the same section throughout.
    let (lo, hi) = bounds(&g.solid);
    assert!(
        near(lo.x, -FENCE_T) && near(hi.x, WIDTH + KERF),
        "x {}..{}",
        lo.x,
        hi.x
    );
    assert!(near(hi.x * IN, 53.0), "53.0 mm: 50.8 and a 2.2 kerf");
    assert!(
        near(lo.y, -LEN / 2.0) && near(hi.y, LEN / 2.0),
        "y {}..{}",
        lo.y,
        hi.y
    );
    assert!(near(lo.z, -DROP) && near(hi.z, T), "z {}..{}", lo.z, hi.z);
    // An extrusion: its two ends are the faces normal to y at the ends
    // (the lettering's bars have walls facing y too, inside).
    let ends = g
        .solid
        .surfaces
        .iter()
        .filter(|s| {
            matches!(s, Surface::Plane { normal, offset }
            if normal.y.abs() > 0.999 && near(offset.abs(), LEN / 2.0 * IN))
        })
        .count();
    assert_eq!(ends, 2, "an L extruded along the edge");
    // The four corners in plan are rounded R4: four vertical cylinders
    // of that radius at the corners, the fence's full height and the
    // depth leg's.
    let rounds: Vec<&Surface> = g
        .solid
        .surfaces
        .iter()
        .filter(|s| matches!(s, Surface::Cylinder { radius, .. } if near(*radius, ROUND * IN)))
        .collect();
    assert_eq!(rounds.len(), 4, "four rounded corners");
    for r in &rounds {
        if let Surface::Cylinder { origin, axis, .. } = r {
            let a = axis.normalized().unwrap();
            assert!(a.z.abs() > 0.999, "a vertical corner");
            let (x, y) = (origin.x / IN, origin.y / IN);
            assert!(
                (near(x, -FENCE_T + ROUND) || near(x, WIDTH + KERF - ROUND))
                    && near(y.abs(), LEN / 2.0 - ROUND),
                "corner at {x}, {y}"
            );
        }
    }
    // The labels: bars engraved 0.6 into the top face, their floors
    // planes at T - 0.6; the two lines together take 20 or more bars.
    let floors = g
        .solid
        .surfaces
        .iter()
        .filter(|s| {
            matches!(s, Surface::Plane { normal, offset }
            if normal.z.abs() > 0.999 && near((offset * normal.z).abs(), (T - ENGRAVE) * IN))
        })
        .count();
    assert!(floors >= 20, "{floors} label bars");
    // The volume: the L's section less the groove, by the length, less
    // the four corners, less the lettering (about 150 mm2 of bars, 0.6
    // deep).
    let section = FENCE_T * (DROP + T) + (WIDTH + KERF) * T - GROOVE * GROOVE;
    let corner = (1.0 - std::f64::consts::PI / 4.0) * ROUND * ROUND;
    let solid = section * LEN - 2.0 * corner * (DROP + T) - 2.0 * corner * T;
    let v = g.solid.volume() / IN.powi(3);
    let lettering = (solid - v) * IN.powi(3);
    println!(
        "gauge {v:.4} in3 ({:.0} mm3), {:.0} mm3 of lettering",
        v * IN.powi(3),
        lettering
    );
    assert!(
        lettering > 60.0 && lettering < 110.0,
        "{lettering} mm3 of lettering: expected 150 mm2 by 0.6"
    );
    let in_groove = g.solid.surfaces.iter().any(|s| match s {
        Surface::Plane { normal, offset } => {
            normal.z.abs() > 0.999 && near((offset * normal.z).abs(), GROOVE * IN)
        }
        _ => false,
    });
    assert!(in_groove, "the groove's ceiling at z = 2 mm");
    // Nothing overhangs when it prints on its top face: every face is
    // a plane square to the axes or one of the vertical corner rounds,
    // and the top is one plane.
    assert!(
        g.solid.surfaces.iter().all(|s| match s {
            Surface::Plane { normal, .. } =>
                normal.x.abs() > 0.999 || normal.y.abs() > 0.999 || normal.z.abs() > 0.999,
            Surface::Cylinder { axis, .. } => axis.normalized().unwrap().z.abs() > 0.999,
            _ => false,
        }),
        "all faces square to the axes or vertical rounds"
    );
    // The STL is written: a binary STL of the L's facets.
    let bytes = std::fs::read(dir().join("out/gauge.stl")).unwrap();
    let tris = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(
        bytes.len(),
        84 + 50 * tris,
        "a binary STL of {tris} triangles"
    );
    assert!(
        tris >= 200,
        "{tris} triangles: the L, four rounds and the lettering"
    );
}

#[test]
fn in_use_the_track_sits_on_the_gauge_and_the_kerf_leaves_two_inches() {
    let mut doc = load();
    let r = doc.regenerate_assembly(tab_named(&doc, "in use")).unwrap();
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(r.bodies.len(), 5, "sheet, track, kerf, two gauges");
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
        let g = bounds(&body(&r.bodies, &format!("gauge {n}")).solid);
        // Hooked on: the fence's inner face on the sheet's edge (the
        // body starts a fence thickness outside it), the stem on the
        // sheet's top, its end where the track's edge is, within the
        // track's length.
        assert!(
            near(g.0.x, sheet.0.x - FENCE_T),
            "gauge {n}: fence on the edge"
        );
        assert!(
            near(g.0.z, sheet.1.z - DROP),
            "gauge {n}: hanging over the edge"
        );
        assert!(
            near(g.1.x, track.0.x),
            "gauge {n}: the track against its end"
        );
        assert!(
            g.0.y >= track.0.y && g.1.y <= track.1.y && g.0.y >= sheet.0.y && g.1.y <= sheet.1.y,
            "gauge {n}: within the track and the sheet"
        );
    }
    println!(
        "strip: {:.4} in, kerf {:.2} mm, track edge at {:.4} in",
        kerf.0.x - sheet.0.x,
        (kerf.1.x - kerf.0.x) * IN,
        track.0.x
    );
}
