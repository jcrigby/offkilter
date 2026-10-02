//! Involute spur gears as closed profiles ready to extrude: external (a
//! disc with the teeth round it and a bore through it) or internal (a
//! ring with the teeth on its inside).
//!
//! Standard full-depth teeth: addendum one module, dedendum 1.25, no
//! profile shift, no backlash (a tooth is as thick as its space at the
//! pitch circle). Each flank is the involute of the base circle from the
//! root (or from the base circle, with a radial flank below it, as a hob
//! leaves) to the tip, sampled into short lines tagged as one smooth
//! surface per flank; tips, roots, bores and rims are arcs, so they
//! become cylinders. [`check`] refuses the designs that would not be what
//! the drawing shows: too few teeth for the pressure angle (undercut),
//! an internal gear whose tips fall inside its base circle, a bore or a
//! rim that leaves less than a module of material.

use crate::{Loop, Profile, SegmentCurve};
use ok_math::Vec2;
use std::f64::consts::PI;

/// Dedendum in modules (addendum is one module).
pub const DEDENDUM: f64 = 1.25;

/// Everything that shapes a gear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub module: f64,
    pub teeth: u32,
    /// Degrees.
    pub pressure_angle: f64,
    pub center: Vec2,
    /// Degrees counter-clockwise from +x to the centreline of the first
    /// tooth; a mating gear with an even tooth count wants half a pitch.
    pub angle: f64,
    /// Bore diameter of an external gear; zero for none.
    pub bore: f64,
    /// Outer diameter of an internal gear; zero makes an external gear.
    pub rim: f64,
    /// How much thinner than its space the tooth is at the pitch circle,
    /// mm: the pair's circular backlash is the sum of both gears'. Zero
    /// draws the theoretical tooth; a printed pair wants 0.1 to 0.3.
    pub backlash: f64,
}

/// The circles of a gear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dims {
    pub internal: bool,
    pub pitch_radius: f64,
    pub base_radius: f64,
    /// Where the teeth end: outside the pitch circle on an external
    /// gear, inside it on an internal one.
    pub tip_radius: f64,
    /// Where the spaces bottom out.
    pub root_radius: f64,
}

fn inv(a: f64) -> f64 {
    a.tan() - a
}

/// Fewest teeth an external gear can have at this pressure angle
/// (degrees) without undercut: 17 at 20°, 12 at 25°, 32 at 14.5°.
pub fn min_teeth(pressure_angle: f64) -> u32 {
    let s = pressure_angle.to_radians().sin();
    (2.0 / (s * s)).round() as u32
}

/// Fewest teeth an internal gear can have at this pressure angle before
/// its tips fall inside the base circle (34 at 20°).
pub fn min_internal_teeth(pressure_angle: f64) -> u32 {
    (2.0 / (1.0 - pressure_angle.to_radians().cos())).ceil() as u32
}

pub fn dims(p: &Params) -> Dims {
    let m = p.module;
    let rp = m * p.teeth as f64 / 2.0;
    let internal = p.rim > 0.0;
    Dims {
        internal,
        pitch_radius: rp,
        base_radius: rp * p.pressure_angle.to_radians().cos(),
        tip_radius: if internal { rp - m } else { rp + m },
        root_radius: if internal {
            rp + DEDENDUM * m
        } else {
            rp - DEDENDUM * m
        },
    }
}

/// The design rules; `Ok` carries the circles.
pub fn check(p: &Params) -> Result<Dims, String> {
    if !(p.module.is_finite() && p.module > 0.0) {
        return Err("module must be positive".into());
    }
    if !(p.pressure_angle.is_finite() && (10.0..=35.0).contains(&p.pressure_angle)) {
        return Err("pressure angle must be between 10 and 35 degrees".into());
    }
    if !(p.bore.is_finite() && p.bore >= 0.0 && p.rim.is_finite() && p.rim >= 0.0) {
        return Err("bore and rim must be zero or positive".into());
    }
    if !(p.backlash.is_finite() && p.backlash >= 0.0) {
        return Err("backlash must be zero or positive".into());
    }
    if !p.angle.is_finite() || !p.center.x.is_finite() || !p.center.y.is_finite() {
        return Err("angle and centre must be finite".into());
    }
    let d = dims(p);
    let m = p.module;
    if d.internal {
        if p.bore > 0.0 {
            return Err("an internal gear takes a rim diameter, not a bore".into());
        }
        let least = min_internal_teeth(p.pressure_angle);
        if p.teeth < least {
            return Err(format!(
                "an internal gear with {} teeth at {}° has its tips inside the base circle: at least {least} teeth",
                p.teeth, p.pressure_angle
            ));
        }
        if p.rim / 2.0 < d.root_radius + m {
            return Err(format!(
                "a rim of {} mm leaves less than one module outside the roots (root diameter {:.2})",
                p.rim,
                2.0 * d.root_radius
            ));
        }
    } else {
        let least = min_teeth(p.pressure_angle);
        if p.teeth < least {
            return Err(format!(
                "{} teeth at {}° would be undercut: at least {least} teeth, or a larger pressure angle (25° takes {})",
                p.teeth,
                p.pressure_angle,
                min_teeth(25.0)
            ));
        }
        if p.bore > 0.0 && p.bore / 2.0 > d.root_radius - m {
            return Err(format!(
                "a bore of {} mm leaves less than one module under the teeth (root diameter {:.2})",
                p.bore,
                2.0 * d.root_radius
            ));
        }
    }
    let half = half_angle(p, &d);
    if half(d.tip_radius) <= 0.0 {
        return Err("the teeth come to a point".into());
    }
    Ok(d)
}

/// Half the tooth's angular thickness at a radius, measured from the
/// tooth's centreline. Narrows towards the tip on both kinds of gear
/// (the tip of an internal tooth being the inner end). The backlash
/// turns the whole flank about the centre, which is how a cutter fed
/// deeper thins a tooth.
fn half_angle(p: &Params, d: &Dims) -> impl Fn(f64) -> f64 {
    let n = p.teeth as f64;
    let half_pitch = PI / (2.0 * n) - p.backlash / (2.0 * d.pitch_radius);
    let inv_a = inv(p.pressure_angle.to_radians());
    let rb = d.base_radius;
    let internal = d.internal;
    move |r: f64| {
        let inv_r = inv((rb / r).min(1.0).acos());
        if internal {
            half_pitch - inv_a + inv_r
        } else {
            half_pitch + inv_a - inv_r
        }
    }
}

/// Points of the gear's outline, counter-clockwise, each tagged with the
/// curve of the segment leaving it. The outline of an internal gear is
/// the toothed hole, also counter-clockwise.
fn outline(p: &Params, d: &Dims, seg: f64) -> Loop {
    const FLANK_STEPS: usize = 12;
    let n = p.teeth as usize;
    let tau = 2.0 * PI / n as f64;
    let rb = d.base_radius;
    let half = half_angle(p, d);
    let center = p.center;
    let polar = |r: f64, a: f64| center + Vec2::from_angle(a) * r;
    // The involute's roll parameter at a radius: r = rb * sqrt(1 + t²).
    let roll = |r: f64| ((r / rb).powi(2) - 1.0).max(0.0).sqrt();
    let radius = |t: f64| rb * (1.0 + t * t).sqrt();
    let mut pts: Vec<(Vec2, SegmentCurve)> = Vec::new();
    // Interior points of an arc from a0 to a1 (both excluded).
    let arc = |r: f64, a0: f64, a1: f64, pts: &mut Vec<(Vec2, SegmentCurve)>| {
        let tag = SegmentCurve::Arc { center, radius: r };
        let steps = ((a1 - a0) / seg).ceil().max(1.0) as usize;
        for s in 1..steps {
            let a = a0 + (a1 - a0) * s as f64 / steps as f64;
            pts.push((polar(r, a), tag));
        }
    };
    let tip = SegmentCurve::Arc {
        center,
        radius: d.tip_radius,
    };
    let root = SegmentCurve::Arc {
        center,
        radius: d.root_radius,
    };
    // Where the involute starts: the root, or the base circle with a
    // radial flank below it.
    let radial = !d.internal && d.root_radius < rb - ok_math::tol::LINEAR;
    let r_lo = if d.internal {
        d.tip_radius
    } else {
        d.root_radius.max(rb)
    };
    let r_hi = if d.internal {
        d.root_radius
    } else {
        d.tip_radius
    };
    let (t_lo, t_hi) = (roll(r_lo), roll(r_hi));
    let half_lo = half(r_lo);
    let half_tip = half(d.tip_radius);
    let half_root = if radial { half_lo } else { half(d.root_radius) };
    for i in 0..n {
        let phi = p.angle.to_radians() + i as f64 * tau;
        let flank_right = SegmentCurve::Spline { id: 2 * i as u32 };
        let flank_left = SegmentCurve::Spline {
            id: 2 * i as u32 + 1,
        };
        if radial {
            pts.push((polar(d.root_radius, phi - half_lo), SegmentCurve::Line));
        }
        // Right flank (negative side of the centreline) from the root
        // towards the tip, then the tip arc, then the left flank back.
        // On an internal gear the tip is the inner end, so the flank
        // runs inwards first.
        let flank: Vec<f64> = if d.internal {
            (0..=FLANK_STEPS)
                .map(|j| radius(t_hi + (t_lo - t_hi) * j as f64 / FLANK_STEPS as f64))
                .collect()
        } else {
            (0..=FLANK_STEPS)
                .map(|j| radius(t_lo + (t_hi - t_lo) * j as f64 / FLANK_STEPS as f64))
                .collect()
        };
        for (j, &r) in flank.iter().enumerate() {
            let tag = if j < FLANK_STEPS { flank_right } else { tip };
            pts.push((polar(r, phi - half(r)), tag));
        }
        arc(d.tip_radius, phi - half_tip, phi + half_tip, &mut pts);
        for (j, &r) in flank.iter().enumerate().rev() {
            let tag = if j > 0 {
                flank_left
            } else if radial {
                SegmentCurve::Line
            } else {
                root
            };
            pts.push((polar(r, phi + half(r)), tag));
        }
        if radial {
            pts.push((polar(d.root_radius, phi + half_lo), root));
        }
        arc(
            d.root_radius,
            phi + half_root,
            phi + tau - half_root,
            &mut pts,
        );
    }
    let (points, curves) = pts.into_iter().unzip();
    Loop { points, curves }
}

/// A full circle, counter-clockwise, tagged as one arc.
fn circle(center: Vec2, radius: f64, seg: f64) -> Loop {
    let steps = ((2.0 * PI / seg).ceil() as usize).max(8);
    let points = (0..steps)
        .map(|i| center + Vec2::from_angle(2.0 * PI * i as f64 / steps as f64) * radius)
        .collect();
    Loop {
        points,
        curves: vec![SegmentCurve::Arc { center, radius }; steps],
    }
}

/// The gear as a region: the toothed outline with the bore as a hole,
/// or the rim with the toothed outline as the hole. `seg` is the most
/// angle one segment of an arc may span, radians.
pub fn profile(p: &Params, seg: f64) -> Result<Profile, String> {
    let d = check(p)?;
    let seg = if seg.is_finite() && seg > 0.0 {
        seg
    } else {
        5f64.to_radians()
    };
    let teeth = outline(p, &d, seg);
    Ok(if d.internal {
        Profile {
            outer: circle(p.center, p.rim / 2.0, seg),
            holes: vec![teeth.reversed()],
        }
    } else {
        Profile {
            outer: teeth,
            holes: if p.bore > 0.0 {
                vec![circle(p.center, p.bore / 2.0, seg).reversed()]
            } else {
                Vec::new()
            },
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point_in_polygon;

    const SEG: f64 = 5.0 * PI / 180.0;

    fn gear(module: f64, teeth: u32, pressure_angle: f64) -> Params {
        Params {
            module,
            teeth,
            pressure_angle,
            center: Vec2::new(0.0, 0.0),
            angle: 0.0,
            bore: 0.0,
            rim: 0.0,
            backlash: 0.0,
        }
    }

    fn radii(l: &Loop, c: Vec2) -> (f64, f64) {
        l.points.iter().fold((f64::MAX, 0.0), |(lo, hi), p| {
            let r = p.distance(c);
            (lo.min(r), hi.max(r))
        })
    }

    #[test]
    fn the_minimum_tooth_counts_are_the_textbook_ones() {
        assert_eq!(min_teeth(20.0), 17);
        assert_eq!(min_teeth(25.0), 11);
        assert_eq!(min_teeth(14.5), 32);
        assert_eq!(min_internal_teeth(20.0), 34);
    }

    #[test]
    fn an_external_gear_fills_its_circles_with_a_tooth_as_thick_as_its_space() {
        let p = Params {
            bore: 10.0,
            ..gear(2.0, 20, 20.0)
        };
        let pr = profile(&p, SEG).unwrap();
        let d = dims(&p);
        assert!((d.pitch_radius - 20.0).abs() < 1e-12);
        let (lo, hi) = radii(&pr.outer, p.center);
        assert!((hi - d.tip_radius).abs() < 1e-9, "{hi}");
        assert!((lo - d.root_radius).abs() < 1e-9, "{lo}");
        // The tooth is as thick as the space at the pitch circle: the
        // points there at ±π/(2N) are on the outline.
        let a = PI / 40.0;
        for s in [-1.0, 1.0] {
            let q = Vec2::from_angle(s * a) * d.pitch_radius;
            let inside = point_in_polygon(q * (1.0 - 1e-3), &pr.outer.points);
            let outside = point_in_polygon(q * (1.0 + 1e-3), &pr.outer.points);
            assert!(inside && !outside, "pitch point {s} is not on the outline");
        }
        // Area: a little under the pitch circle (the tooth narrows above
        // the pitch line faster than it widens below), minus the bore.
        let area = pr.area();
        let pitch = PI * d.pitch_radius.powi(2);
        let bore = PI * 25.0;
        assert!(area < pitch - bore && area > 0.9 * pitch - bore, "{area}");
        assert_eq!(pr.holes.len(), 1);
        assert!((pr.holes[0].signed_area() + bore).abs() < bore * 2e-3);
        // Twenty teeth at 20° sit below the base circle, so each tooth
        // has its two radial flank pieces.
        assert!(d.root_radius < d.base_radius);
        let lines = pr
            .outer
            .curves
            .iter()
            .filter(|c| matches!(c, SegmentCurve::Line))
            .count();
        assert_eq!(lines, 40);
    }

    #[test]
    fn a_big_gear_has_no_radial_flank_and_each_flank_is_one_surface() {
        let p = gear(2.5, 80, 20.0);
        let pr = profile(&p, SEG).unwrap();
        let d = dims(&p);
        assert!(d.root_radius > d.base_radius);
        assert!(!pr
            .outer
            .curves
            .iter()
            .any(|c| matches!(c, SegmentCurve::Line)));
        let mut ids = std::collections::BTreeSet::new();
        for c in &pr.outer.curves {
            if let SegmentCurve::Spline { id } = c {
                ids.insert(*id);
            }
        }
        assert_eq!(ids.len(), 160);
    }

    #[test]
    fn an_internal_gear_is_a_ring_with_the_teeth_pointing_in() {
        let p = Params {
            rim: 220.0,
            ..gear(2.5, 80, 20.0)
        };
        let pr = profile(&p, SEG).unwrap();
        let d = dims(&p);
        assert!(d.internal);
        assert!((d.tip_radius - 97.5).abs() < 1e-12);
        assert!((d.root_radius - 103.125).abs() < 1e-12);
        assert_eq!(pr.holes.len(), 1);
        let (lo, hi) = radii(&pr.holes[0], p.center);
        assert!((lo - d.tip_radius).abs() < 1e-9 && (hi - d.root_radius).abs() < 1e-9);
        assert!(pr.holes[0].signed_area() < 0.0);
        let rim = PI * 110f64.powi(2);
        let area = pr.area();
        assert!(
            area < rim - PI * d.tip_radius.powi(2) && area > rim - PI * d.root_radius.powi(2),
            "{area}"
        );
        // The pitch points at ±π/(2N) are on the hole's outline too.
        let a = PI / 160.0;
        let q = Vec2::from_angle(a) * d.pitch_radius;
        let hole: Vec<Vec2> = pr.holes[0].points.clone();
        assert!(
            point_in_polygon(q * (1.0 - 1e-3), &hole) && !point_in_polygon(q * (1.0 + 1e-3), &hole)
        );
    }

    /// A pinion and a gear at their centre distance, the gear turned by
    /// half a pitch so a space faces a tooth, do not overlap; turned
    /// tooth to tooth they do.
    #[test]
    fn mating_gears_clear_each_other_only_when_a_space_faces_a_tooth() {
        let a = gear(2.0, 20, 20.0);
        let b = Params {
            center: Vec2::new(60.0, 0.0),
            angle: 180.0 / 40.0,
            ..gear(2.0, 40, 20.0)
        };
        let pa = profile(&a, SEG).unwrap().outer;
        let pb = profile(&b, SEG).unwrap().outer;
        let overlap = |x: &Loop, y: &Loop| {
            x.points
                .iter()
                .filter(|p| point_in_polygon(**p, &y.points))
                .count()
        };
        assert_eq!(overlap(&pa, &pb) + overlap(&pb, &pa), 0);
        let clash = Params { angle: 0.0, ..b };
        let pc = profile(&clash, SEG).unwrap().outer;
        assert!(overlap(&pa, &pc) + overlap(&pc, &pa) > 0);
    }

    /// 20 in 80 (module 2.5): the pinion's centre is at the difference
    /// of the pitch radii; its even count wants half a pitch too. At the
    /// contact point the pinion's tip corner lies exactly on the ring's
    /// flank, whose chords cut a hair into the hole, so the pinion is
    /// shrunk by a hundredth of a millimetre for the check; with a
    /// little backlash it clears as drawn.
    #[test]
    fn a_pinion_in_a_ring_gear_clears_it() {
        let ring = Params {
            rim: 220.0,
            ..gear(2.5, 80, 20.0)
        };
        let pinion = Params {
            center: Vec2::new(75.0, 0.0),
            angle: 180.0 / 20.0,
            ..gear(2.5, 20, 20.0)
        };
        let hole = profile(&ring, SEG).unwrap().holes.remove(0).reversed();
        let inside = |p: &Params, shrink: f64| {
            let pin = profile(p, SEG).unwrap().outer;
            pin.points
                .iter()
                .map(|q| p.center + (*q - p.center) * (1.0 - shrink))
                .all(|q| point_in_polygon(q, &hole.points))
        };
        assert!(inside(&pinion, 1e-2 / 27.5));
        assert!(!inside(
            &Params {
                angle: 0.0,
                ..pinion
            },
            1e-2 / 27.5
        ));
        assert!(inside(
            &Params {
                backlash: 0.1,
                ..pinion
            },
            0.0
        ));
    }

    #[test]
    fn backlash_thins_the_tooth_at_the_pitch_circle() {
        let p = gear(2.0, 20, 20.0);
        let thin = Params { backlash: 0.2, ..p };
        let d = dims(&p);
        // The theoretical pitch point is now outside the thinner tooth,
        // and a point 0.1 mm (0.2 / 2 per flank) inside it is still in.
        let a = PI / 40.0;
        let q = Vec2::from_angle(a) * d.pitch_radius;
        let outline = profile(&thin, SEG).unwrap().outer;
        assert!(!point_in_polygon(q, &outline.points));
        let q2 = Vec2::from_angle(a - 0.1 / d.pitch_radius * 1.05) * d.pitch_radius;
        assert!(point_in_polygon(q2, &outline.points));
    }

    #[test]
    fn the_rules_refuse_undercut_pointed_and_thin_designs() {
        let e = check(&gear(2.0, 12, 20.0)).unwrap_err();
        assert!(e.contains("undercut") && e.contains("17"), "{e}");
        check(&gear(2.0, 12, 25.0)).unwrap();
        let e = check(&Params {
            rim: 100.0,
            ..gear(2.0, 20, 20.0)
        })
        .unwrap_err();
        assert!(e.contains("inside the base circle"), "{e}");
        let e = check(&Params {
            bore: 33.0,
            ..gear(2.0, 20, 20.0)
        })
        .unwrap_err();
        assert!(e.contains("bore"), "{e}");
        let e = check(&Params {
            rim: 206.0,
            ..gear(2.5, 80, 20.0)
        })
        .unwrap_err();
        assert!(e.contains("rim"), "{e}");
        let e = check(&Params {
            rim: 220.0,
            bore: 5.0,
            ..gear(2.5, 80, 20.0)
        })
        .unwrap_err();
        assert!(e.contains("not a bore"), "{e}");
        assert!(check(&gear(0.0, 20, 20.0)).is_err());
        assert!(check(&gear(2.0, 20, 40.0)).is_err());
    }
}
