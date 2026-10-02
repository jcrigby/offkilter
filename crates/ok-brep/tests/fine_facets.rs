//! Sections through solids with very fine facets: a cylinder faceted at
//! 0.05° (chords under a tenth of a millimetre) split through its axis
//! and the halves intersected, as the cart's ring sectors were.

use ok_brep::{boolean, extrude, split, BoolOp};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{ProfileOptions, Sketch};

fn fine_cylinder(radius: f64, height: f64, step_deg: f64) -> ok_brep::Solid {
    let mut sk = Sketch::new();
    sk.add_circle(Vec2::new(0.0, 0.0), radius);
    let opts = ProfileOptions {
        arc_segment_angle: step_deg.to_radians(),
        ..ProfileOptions::default()
    };
    let profile = sk.profiles(&opts).into_iter().next().unwrap();
    let plane = Plane::from_origin_normal(Vec3::ZERO, Vec3::Z).unwrap();
    extrude(&profile, &plane, 0.0, height, 1).unwrap()
}

fn halves_intersect_to_nothing(step_deg: f64, plane_angle_deg: f64) {
    let cyl = fine_cylinder(100.0, 20.0, step_deg);
    let a = plane_angle_deg.to_radians();
    let plane = Plane::from_origin_normal(Vec3::ZERO, Vec3::new(a.cos(), a.sin(), 0.0)).unwrap();
    let (lo, hi) = split(&cyl, &plane)
        .unwrap()
        .expect("the plane cuts the cylinder");
    lo.validate().unwrap();
    hi.validate().unwrap();
    let both = boolean(&lo, &hi, BoolOp::Intersection)
        .unwrap_or_else(|e| panic!("{step_deg}° facets, plane at {plane_angle_deg}°: {e}"));
    assert!(
        both.volume().abs() < 1e-3,
        "{step_deg}° facets, plane at {plane_angle_deg}°: overlap {}",
        both.volume()
    );
    let all = boolean(&lo, &hi, BoolOp::Union).unwrap();
    assert!(
        (all.volume() - cyl.volume()).abs() < 1e-3 * cyl.volume(),
        "{step_deg}°: union {} vs {}",
        all.volume(),
        cyl.volume()
    );
}

#[test]
fn coarse_halves() {
    halves_intersect_to_nothing(5.0, 0.0);
    halves_intersect_to_nothing(5.0, 37.0);
}

#[test]
fn fine_halves_through_a_vertex() {
    halves_intersect_to_nothing(0.05, 0.0);
}

#[test]
fn fine_halves_through_facets() {
    halves_intersect_to_nothing(0.05, 37.0);
    halves_intersect_to_nothing(0.02, 11.3);
}

use ok_sketch::gear::{profile, Params};
use ok_sketch::{Loop, Profile, SegmentCurve};

/// Every arc of the loop sampled at chords of `chord` mm, as the cart's
/// ring fillets once were (0.04 mm facets on a 265 mm ring).
fn resample_arcs(l: &Loop, chord: f64) -> Loop {
    let n = l.points.len();
    let mut points = Vec::new();
    let mut curves = Vec::new();
    for i in 0..n {
        let a = l.points[i];
        let b = l.points[(i + 1) % n];
        points.push(a);
        curves.push(l.curves[i]);
        if let SegmentCurve::Arc { center, radius } = l.curves[i] {
            let a0 = (a - center).angle();
            let mut sweep = (b - center).angle() - a0;
            while sweep > std::f64::consts::PI {
                sweep -= 2.0 * std::f64::consts::PI;
            }
            while sweep < -std::f64::consts::PI {
                sweep += 2.0 * std::f64::consts::PI;
            }
            let steps = ((sweep.abs() * radius) / chord).ceil().max(1.0) as usize;
            for k in 1..steps {
                let t = a0 + sweep * k as f64 / steps as f64;
                points.push(center + Vec2::from_angle(t) * radius);
                curves.push(l.curves[i]);
            }
        }
    }
    Loop { points, curves }
}

fn ring_sectors(teeth: u32, rim: f64, chord: f64, cuts: &[f64]) -> Vec<ok_brep::Solid> {
    let p = Params {
        module: 3.0,
        teeth,
        pressure_angle: 20.0,
        center: Vec2::new(0.0, 0.0),
        angle: 0.0,
        bore: 0.0,
        rim,
        backlash: 0.15,
        shift: 0.0,
        fillet: 0.45,
    };
    let prof = profile(&p, 5f64.to_radians()).unwrap();
    let fine = Profile {
        outer: resample_arcs(&prof.outer, chord),
        holes: prof.holes.iter().map(|h| resample_arcs(h, chord)).collect(),
    };
    let plane = Plane::from_origin_normal(Vec3::ZERO, Vec3::Z).unwrap();
    let ring = extrude(&fine, &plane, 0.0, 20.0, 1).unwrap();
    let mut pieces = vec![ring];
    for &deg in cuts {
        let a = deg.to_radians();
        let cut = Plane::from_origin_normal(Vec3::ZERO, Vec3::new(a.cos(), a.sin(), 0.0)).unwrap();
        let mut next = Vec::new();
        for piece in &pieces {
            match split(piece, &cut).unwrap() {
                Some((lo, hi)) => {
                    next.push(lo);
                    next.push(hi);
                }
                None => next.push(piece.clone()),
            }
        }
        pieces = next;
    }
    pieces
}

/// Adjacent sectors of a finely faceted ring gear intersect to nothing.
fn sectors_touch_cleanly(teeth: u32, rim: f64, chord: f64, cuts: &[f64]) {
    let pieces = ring_sectors(teeth, rim, chord, cuts);
    assert_eq!(pieces.len(), 2 * cuts.len());
    let facets: usize = pieces.iter().map(|s| s.faces.len()).sum();
    for (i, a) in pieces.iter().enumerate() {
        for b in pieces.iter().skip(i + 1) {
            let both = boolean(a, b, BoolOp::Intersection).unwrap_or_else(|e| {
                panic!("{teeth} teeth, {chord} mm chords ({facets} facets), cuts {cuts:?}: {e}")
            });
            assert!(
                both.volume().abs() < 1e-3,
                "{teeth} teeth, {chord} mm chords: sectors overlap by {}",
                both.volume()
            );
        }
    }
}

#[test]
fn ring_sectors_with_fine_fillets() {
    sectors_touch_cleanly(36, 130.0, 0.04, &[0.0, 60.0, 120.0]);
}

#[test]
#[ignore = "the cart's ring at the old facet size: minutes"]
fn the_carts_ring_sectors_with_fine_fillets() {
    sectors_touch_cleanly(80, 265.0, 0.04, &[0.0, 60.0, 120.0]);
}
