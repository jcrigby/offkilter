//! A straight bevel gear as a solid: the one-pitch outline `ok_sketch`
//! draws on the developed back cone is wrapped onto that cone at the
//! outer end, every point is pulled towards the pitch apex for the inner
//! end, and the teeth are ruled between the two, so each flank facet is
//! a planar trapezoid on a line through the apex. Behind the teeth the
//! back cone runs down to a flat back face at the root, the same (scaled)
//! in front, and a bore goes through both.

use crate::{BrepError, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec3};
use ok_sketch::gear::{BevelCurve, BevelPitch};
use std::f64::consts::PI;

/// The gear of `pitch` with `teeth` teeth, its pitch apex at `apex`
/// and its axis along `axis` (unit; the teeth lie `cone_distance`
/// along it), the outer end `width` further from the apex than the
/// inner one, a bore of diameter `bore` (0 for none), and the first
/// tooth's centreline `angle` degrees from `x_axis` (unit, square to
/// the axis). `seg` is the bore's facet angle in radians.
#[allow(clippy::too_many_arguments)]
pub fn bevel_gear(
    pitch: &BevelPitch,
    teeth: u32,
    width: f64,
    bore: f64,
    angle: f64,
    apex: Vec3,
    axis: Vec3,
    x_axis: Vec3,
    seg: f64,
    feature: u32,
) -> Result<Solid, BrepError> {
    let n = teeth as usize;
    let d = pitch.cone;
    let big_r = pitch.cone_distance;
    if !(width.is_finite() && width > ok_math::tol::LINEAR) {
        return Err(BrepError::Degenerate("face width must be positive".into()));
    }
    let s = (big_r - width) / big_r;
    if s < 0.2 {
        return Err(BrepError::Degenerate(format!(
            "face width {width} is too much of the cone distance {big_r:.2}: a bevel gear's face is a third of it at most"
        )));
    }
    let r_root = pitch.virtual_root_radius * d.cos();
    if bore < 0.0 {
        return Err(BrepError::Degenerate("bore cannot be negative".into()));
    }
    let module = pitch.virtual_tip_radius - pitch.virtual_pitch_radius;
    if bore > 0.0 && bore / 2.0 + module > s * r_root {
        return Err(BrepError::Degenerate(format!(
            "bore {bore} leaves less than a module under the teeth at the inner end (root diameter there {:.2})",
            2.0 * s * r_root
        )));
    }
    let y_axis = axis.cross(x_axis);
    let world = |x: f64, y: f64, z: f64| apex + x_axis * x + y_axis * y + axis * z;
    // The back cone: its apex on the axis, the virtual gear wrapped on it.
    let z_b = big_r * d.cos() + pitch.virtual_pitch_radius * d.sin();
    let on_cone = |rho: f64, psi: f64| {
        world(
            rho * d.cos() * psi.cos(),
            rho * d.cos() * psi.sin(),
            z_b - rho * d.sin(),
        )
    };
    let phase = angle.to_radians();
    let per = pitch.points.len();
    // The outer loop, each point with its curve, and the back cone's
    // foot of each point at the root radius.
    let mut outer: Vec<(Vec3, BevelCurve, usize)> = Vec::with_capacity(n * per);
    let mut feet: Vec<Vec3> = Vec::new();
    let mut last_psi = f64::NAN;
    for k in 0..n {
        for &(rho, phi, curve) in &pitch.points {
            let psi = phi / d.cos() + k as f64 * 2.0 * PI / n as f64 + phase;
            if last_psi.is_nan() || (psi - last_psi).abs() > 1e-12 {
                feet.push(on_cone(pitch.virtual_root_radius, psi));
                last_psi = psi;
            }
            outer.push((on_cone(rho, psi), curve, feet.len() - 1));
        }
    }
    let total = outer.len();
    let scale = |p: Vec3| apex + (p - apex) * s;
    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();
    let mut local = 0u32;
    let push = |pts: Vec<Vec3>,
                surface: usize,
                polys: &mut Vec<Polygon>,
                local: &mut u32|
     -> Result<(), BrepError> {
        // Drop repeated points; a facet needs three distinct ones.
        let mut v: Vec<Vec3> = Vec::with_capacity(pts.len());
        for p in pts {
            if v.last().is_none_or(|q| (*q - p).length() > 1e-9) {
                v.push(p);
            }
        }
        if v.len() > 1 && (v[0] - v[v.len() - 1]).length() <= 1e-9 {
            v.pop();
        }
        if v.len() < 3 {
            return Ok(());
        }
        let Some(normal) = (v[1] - v[0])
            .cross(v[2] - v[0])
            .normalized()
            .or_else(|| (v[1] - v[0]).cross(v[v.len() - 1] - v[0]).normalized())
        else {
            return Ok(());
        };
        let x = (v[1] - v[0]).normalized().unwrap();
        let plane = Plane {
            origin: v[0],
            x_axis: x,
            y_axis: normal.cross(x),
            normal,
        };
        polys.push(Polygon {
            plane,
            loops: vec![v],
            surface,
            origin: FaceOrigin {
                feature,
                local: *local,
            },
        });
        *local += 1;
        Ok(())
    };
    // Surfaces: a ruled one per flank, the face cone for the tips, the
    // root cone for the lands, the back and front cones, the flats, the bore.
    let z_tip = z_b - pitch.virtual_tip_radius * d.sin();
    let r_tip = pitch.virtual_tip_radius * d.cos();
    let z_root = z_b - pitch.virtual_root_radius * d.sin();
    let face_cone = surfaces.len();
    surfaces.push(Surface::Cone {
        apex,
        axis,
        half_angle: (r_tip / z_tip).atan(),
    });
    let root_cone = surfaces.len();
    surfaces.push(Surface::Cone {
        apex,
        axis,
        half_angle: (r_root / z_root).atan(),
    });
    let back_cone = surfaces.len();
    surfaces.push(Surface::Cone {
        apex: world(0.0, 0.0, z_b),
        axis: -axis,
        half_angle: PI / 2.0 - d,
    });
    let front_cone = surfaces.len();
    surfaces.push(Surface::Cone {
        apex: world(0.0, 0.0, s * z_b),
        axis: -axis,
        half_angle: PI / 2.0 - d,
    });
    let mut flank_surface: Vec<Option<usize>> = vec![None; 2 * n];
    // The teeth: ruled between the outer loop and its scaled copy.
    for j in 0..total {
        let (o0, curve, _) = outer[j];
        let (o1, _, _) = outer[(j + 1) % total];
        let surface = match curve {
            BevelCurve::Flank(f) => {
                let k = j / per;
                let slot = 2 * k + f as usize;
                *flank_surface[slot].get_or_insert_with(|| {
                    surfaces.push(Surface::Ruled);
                    surfaces.len() - 1
                })
            }
            BevelCurve::Tip => face_cone,
            BevelCurve::Root => root_cone,
        };
        push(
            vec![o0, scale(o0), scale(o1), o1],
            surface,
            &mut polys,
            &mut local,
        )?;
    }
    // The back cone from the outline down to the root circle, and the
    // same scaled in front.
    for j in 0..total {
        let (o0, _, f0) = outer[j];
        let (o1, _, f1) = outer[(j + 1) % total];
        let (c0, c1) = (feet[f0], feet[f1]);
        push(vec![o0, o1, c1, c0], back_cone, &mut polys, &mut local)?;
        push(
            vec![scale(o1), scale(o0), scale(c0), scale(c1)],
            front_cone,
            &mut polys,
            &mut local,
        )?;
    }
    // The flat back and front faces at the root circles, bored.
    let bore_points = |z: f64| -> Vec<Vec3> {
        let count = ((2.0 * PI / seg).round() as usize).max(12);
        (0..count)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / count as f64;
                world(bore / 2.0 * a.cos(), bore / 2.0 * a.sin(), z)
            })
            .collect()
    };
    let back_ring: Vec<Vec3> = feet.clone();
    let front_ring: Vec<Vec3> = feet.iter().rev().map(|&p| scale(p)).collect();
    let back = surfaces.len();
    surfaces.push(Surface::Plane {
        normal: axis,
        offset: axis.dot(world(0.0, 0.0, z_root)),
    });
    let front = surfaces.len();
    surfaces.push(Surface::Plane {
        normal: -axis,
        offset: (-axis).dot(world(0.0, 0.0, s * z_root)),
    });
    let flat = |ring: Vec<Vec3>,
                hole: Option<Vec<Vec3>>,
                normal: Vec3,
                surface: usize,
                polys: &mut Vec<Polygon>,
                local: &mut u32| {
        let x = (ring[1] - ring[0]).normalized().unwrap();
        let plane = Plane {
            origin: ring[0],
            x_axis: x,
            y_axis: normal.cross(x),
            normal,
        };
        let mut loops = vec![ring];
        if let Some(h) = hole {
            loops.push(h);
        }
        polys.push(Polygon {
            plane,
            loops,
            surface,
            origin: FaceOrigin {
                feature,
                local: *local,
            },
        });
        *local += 1;
    };
    if bore > 0.0 {
        let back_bore = bore_points(z_root);
        let front_bore = bore_points(s * z_root);
        let hole_back: Vec<Vec3> = back_bore.iter().rev().copied().collect();
        flat(
            back_ring,
            Some(hole_back),
            axis,
            back,
            &mut polys,
            &mut local,
        );
        flat(
            front_ring,
            Some(front_bore.clone()),
            -axis,
            front,
            &mut polys,
            &mut local,
        );
        let cyl = surfaces.len();
        surfaces.push(Surface::Cylinder {
            origin: apex,
            axis,
            radius: bore / 2.0,
        });
        let count = back_bore.len();
        for i in 0..count {
            let (a, b) = (back_bore[i], back_bore[(i + 1) % count]);
            let (fa, fb) = (front_bore[i], front_bore[(i + 1) % count]);
            push(vec![a, b, fb, fa], cyl, &mut polys, &mut local)?;
        }
    } else {
        flat(back_ring, None, axis, back, &mut polys, &mut local);
        flat(front_ring, None, -axis, front, &mut polys, &mut local);
    }
    Solid::from_polygons(polys, surfaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_math::Vec2;
    use ok_sketch::gear::{bevel_pitch, Params};

    fn params(teeth: u32, bore: f64) -> Params {
        Params {
            module: 2.0,
            teeth,
            pressure_angle: 20.0,
            center: Vec2::new(0.0, 0.0),
            angle: 0.0,
            bore,
            rim: 0.0,
            backlash: 0.0,
            shift: 0.0,
            fillet: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn gear(
        teeth: u32,
        cone: f64,
        width: f64,
        bore: f64,
        angle: f64,
        apex: Vec3,
        axis: Vec3,
        x: Vec3,
    ) -> Solid {
        let p = bevel_pitch(&params(teeth, bore), cone, 10f64.to_radians()).unwrap();
        bevel_gear(
            &p,
            teeth,
            width,
            bore,
            angle,
            apex,
            axis,
            x,
            10f64.to_radians(),
            7,
        )
        .unwrap()
    }

    #[test]
    fn a_bevel_gear_is_closed_and_sized_like_its_cones() {
        // 20 teeth, module 2, 45° cone: cone distance 28.28, face 9.
        let g = gear(20, 45.0, 9.0, 8.0, 0.0, Vec3::ZERO, Vec3::Z, Vec3::X);
        let v = g.volume();
        // Between the root cone frustum and the tip cone frustum, less the bore.
        let p = bevel_pitch(&params(20, 8.0), 45.0, 0.2).unwrap();
        let d = p.cone;
        let s = (p.cone_distance - 9.0) / p.cone_distance;
        let z_b = p.cone_distance * d.cos() + p.virtual_pitch_radius * d.sin();
        let frustum = |r: f64, z: f64| {
            // A cone frustum between z and s z with radii r and s r about the axis.
            let h = z * (1.0 - s);
            PI * h / 3.0 * (r * r + r * s * r + s * s * r * r)
        };
        let z_root = z_b - p.virtual_root_radius * d.sin();
        let z_tip = z_b - p.virtual_tip_radius * d.sin();
        let lo = frustum(p.virtual_root_radius * d.cos(), z_root) - PI * 16.0 * z_root * (1.0 - s);
        let hi = frustum(p.virtual_tip_radius * d.cos(), z_tip)
            + PI * (p.virtual_root_radius * d.cos()).powi(2) * (z_root - z_tip);
        assert!(v > lo && v < hi, "volume {v} not in {lo}..{hi}");
        // 40 flanks, each one ruled surface; one bore cylinder.
        let flanks = g
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Ruled))
            .count();
        assert_eq!(flanks, 40);
        let cylinders = g
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Cylinder { .. }))
            .count();
        assert_eq!(cylinders, 1);
        // The teeth sit between the inner and outer cone distances along z.
        let (lo_b, hi_b) = g.bounds().unwrap();
        assert!(
            hi_b.z <= z_root + 1e-6 && lo_b.z >= s * z_tip - 1e-6,
            "{lo_b:?} {hi_b:?}"
        );
    }

    #[test]
    fn a_mitre_pair_at_ninety_degrees_meshes() {
        // Two 20-tooth gears at 45°, apexes together, axes z and y: the
        // mesh line is along (0, sin 45, cos 45). A's tooth points at it;
        // B, on the y axis with x and -z as its plane, must show a space
        // there, at its azimuth -90°.
        let a = gear(20, 45.0, 8.0, 0.0, 90.0, Vec3::ZERO, Vec3::Z, Vec3::X);
        let b_angle = -90.0 + 180.0 / 20.0;
        let b = gear(20, 45.0, 8.0, 0.0, b_angle, Vec3::ZERO, Vec3::Y, Vec3::X);
        let clash = crate::boolean(&a, &b, crate::BoolOp::Intersection)
            .map(|s| s.volume().abs())
            .unwrap_or(f64::NAN);
        assert!(clash < 0.5, "meshed pair overlaps by {clash} mm³");
        // Turned half a pitch, tooth meets tooth.
        let b2 = gear(20, 45.0, 8.0, 0.0, -90.0, Vec3::ZERO, Vec3::Y, Vec3::X);
        let clash2 = crate::boolean(&a, &b2, crate::BoolOp::Intersection)
            .map(|s| s.volume().abs())
            .unwrap_or(f64::NAN);
        assert!(
            clash2 > 20.0,
            "tooth on tooth overlaps by only {clash2} mm³"
        );
    }

    #[test]
    fn bevel_solid_rules() {
        let p = bevel_pitch(&params(20, 0.0), 45.0, 0.2).unwrap();
        let too_wide = bevel_gear(&p, 20, 25.0, 0.0, 0.0, Vec3::ZERO, Vec3::Z, Vec3::X, 0.2, 1)
            .unwrap_err()
            .to_string();
        assert!(
            too_wide.contains("too much of the cone distance"),
            "{too_wide}"
        );
        let big_bore = bevel_gear(&p, 20, 8.0, 30.0, 0.0, Vec3::ZERO, Vec3::Z, Vec3::X, 0.2, 1)
            .unwrap_err()
            .to_string();
        assert!(big_bore.contains("less than a module"), "{big_bore}");
    }
}
