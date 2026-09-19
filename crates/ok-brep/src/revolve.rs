use crate::{flip_plane, BrepError, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{Loop, Profile, SegmentCurve};

/// Rotates `p` about the line through `origin` along unit `axis` by `angle`.
fn rotate(p: Vec3, origin: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let d = p - origin;
    let (s, c) = angle.sin_cos();
    // Rodrigues' rotation formula.
    origin + d * c + axis.cross(d) * s + axis * (axis.dot(d) * (1.0 - c))
}

/// Revolves a planar profile about an axis lying in the sketch plane, given
/// by a point and a direction in sketch coordinates. `angle` is in radians;
/// a full turn (within tolerance) produces a closed ring with no caps. The
/// profile must lie entirely on one side of the axis.
pub fn revolve(
    profile: &Profile,
    plane: &Plane,
    axis_point: Vec2,
    axis_dir: Vec2,
    angle: f64,
    segment_angle: f64,
    feature: u32,
) -> Result<Solid, BrepError> {
    if profile.outer.len() < 3 {
        return Err(BrepError::Degenerate(
            "profile has fewer than three points".into(),
        ));
    }
    let Some(axis_dir) = axis_dir.normalized() else {
        return Err(BrepError::Degenerate("revolve axis has zero length".into()));
    };
    if angle.abs() <= ok_math::tol::ANGULAR {
        return Err(BrepError::Degenerate("revolve angle is zero".into()));
    }
    let full =
        (angle.abs() - std::f64::consts::TAU).abs() <= 1e-9 || angle.abs() > std::f64::consts::TAU;
    let angle = if full { std::f64::consts::TAU } else { angle };

    // Every profile point must be on one side of the axis (or on it).
    let side = |p: Vec2| axis_dir.cross(p - axis_point);
    let mut sign = 0.0f64;
    for l in std::iter::once(&profile.outer).chain(profile.holes.iter()) {
        for &p in &l.points {
            let s = side(p);
            if s.abs() > 1e-9 {
                if sign != 0.0 && s.signum() != sign {
                    return Err(BrepError::Degenerate(
                        "profile crosses the revolve axis".into(),
                    ));
                }
                sign = s.signum();
            }
        }
    }
    if sign == 0.0 {
        return Err(BrepError::Degenerate(
            "profile lies on the revolve axis".into(),
        ));
    }

    let origin3 = plane.to_world(axis_point);
    let axis3 = (plane.x_axis * axis_dir.x + plane.y_axis * axis_dir.y)
        .normalized()
        .unwrap();
    // A positive angle is a right-hand rotation about the axis direction.
    let signed_angle = angle;
    let steps = ((angle.abs() / segment_angle).ceil() as usize).max(3);
    let step = signed_angle / steps as f64;
    let at = |p: Vec2, k: usize| rotate(plane.to_world(p), origin3, axis3, step * k as f64);
    let ring_index = |k: usize| if full { k % steps } else { k };

    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();
    let mut local = 0u32;

    // Direction the rotation carries the profile at its start position:
    // v = sign(angle) · axis × r, evaluated at an off-axis profile point.
    // Wall quads [a_k, b_k, b_k+1, a_k+1] face outward exactly when the
    // sweep moves along the sketch normal; otherwise they are reversed.
    let sample = profile
        .outer
        .points
        .iter()
        .copied()
        .find(|p| side(*p).abs() > 1e-9)
        .unwrap();
    let v = axis3.cross(plane.to_world(sample) - origin3) * signed_angle.signum();
    let velocity_along_normal = v.dot(plane.normal) > 0.0;

    if !full {
        // Start cap at angle 0 and end cap at the final angle. The cap that
        // faces against the rotation velocity is the reversed profile.
        let start_plane = if velocity_along_normal {
            flip_plane(plane)
        } else {
            *plane
        };
        surfaces.push(Surface::Plane {
            normal: start_plane.normal,
            offset: start_plane.normal.dot(start_plane.origin),
        });
        polys.push(Polygon {
            plane: start_plane,
            loops: std::iter::once(&profile.outer)
                .chain(profile.holes.iter())
                .map(|l| {
                    let l = if velocity_along_normal {
                        l.reversed()
                    } else {
                        l.clone()
                    };
                    l.points.iter().map(|&p| at(p, 0)).collect()
                })
                .collect(),
            surface: 0,
            origin: FaceOrigin { feature, local: 0 },
        });
        let end_normal = rotate(plane.origin + plane.normal, origin3, axis3, signed_angle)
            - rotate(plane.origin, origin3, axis3, signed_angle);
        let end_origin = at(Vec2::ZERO, steps);
        let mut end_plane = Plane::from_origin_normal(end_origin, end_normal).unwrap();
        if velocity_along_normal {
            // Outward normal is +rotated normal; loops keep profile orientation.
        } else {
            end_plane = flip_plane(&end_plane);
        }
        surfaces.push(Surface::Plane {
            normal: end_plane.normal,
            offset: end_plane.normal.dot(end_plane.origin),
        });
        polys.push(Polygon {
            plane: end_plane,
            loops: std::iter::once(&profile.outer)
                .chain(profile.holes.iter())
                .map(|l| {
                    let l = if velocity_along_normal {
                        l.clone()
                    } else {
                        l.reversed()
                    };
                    l.points.iter().map(|&p| at(p, steps)).collect()
                })
                .collect(),
            surface: 1,
            origin: FaceOrigin { feature, local: 1 },
        });
        local = 2;
    }

    let mut walls = |ring: &Loop| {
        let count = ring.len();
        let mut shared: Option<(Vec2, f64, usize)> = None;
        for i in 0..count {
            let a = ring.points[i];
            let b = ring.points[(i + 1) % count];
            // Segments on the axis sweep nothing.
            if side(a).abs() <= 1e-9 && side(b).abs() <= 1e-9 {
                shared = None;
                continue;
            }
            // A line segment perpendicular to the axis sweeps a plane, one
            // parallel to it a cylinder; anything else is a cone, kept as a
            // generic revolved surface.
            let line_kind = match ring.curves[i] {
                SegmentCurve::Line => {
                    let d = b - a;
                    let len = d.length();
                    if len <= ok_math::tol::LINEAR {
                        None
                    } else if d.dot(axis_dir).abs() <= 1e-9 * len {
                        Some("plane")
                    } else if d.cross(axis_dir).abs() <= 1e-9 * len {
                        Some("cylinder")
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let surface = match ring.curves[i] {
                SegmentCurve::Line => {
                    surfaces.push(match line_kind {
                        Some("cylinder") => Surface::Cylinder {
                            origin: origin3,
                            axis: axis3,
                            radius: side(a).abs(),
                        },
                        // Planar: the normal is fixed up from the first facet below.
                        _ => Surface::Revolved {
                            origin: origin3,
                            axis: axis3,
                        },
                    });
                    shared = None;
                    surfaces.len() - 1
                }
                SegmentCurve::Arc { center, radius } => {
                    let reuse = shared.filter(|(c, r, _)| {
                        c.approx_eq(center) && (r - radius).abs() <= ok_math::tol::LINEAR
                    });
                    match reuse {
                        Some((_, _, id)) => id,
                        None => {
                            surfaces.push(Surface::Revolved {
                                origin: origin3,
                                axis: axis3,
                            });
                            let id = surfaces.len() - 1;
                            shared = Some((center, radius, id));
                            id
                        }
                    }
                }
                SegmentCurve::Spline { id } => {
                    // Pieces of one spline share a surface; the spline id
                    // stands in for the centre/radius key.
                    let key = Vec2::new(id as f64, f64::NAN);
                    match shared.filter(|(c, _, _)| c.x == key.x && c.y.is_nan()) {
                        Some((_, _, s)) => s,
                        None => {
                            surfaces.push(Surface::Revolved {
                                origin: origin3,
                                axis: axis3,
                            });
                            let s = surfaces.len() - 1;
                            shared = Some((key, 0.0, s));
                            s
                        }
                    }
                }
            };
            for k in 0..steps {
                let k1 = ring_index(k + 1);
                let mut quad = [at(a, k), at(b, k), at(b, k1), at(a, k1)];
                if !velocity_along_normal {
                    quad.reverse();
                }
                // Drop duplicate corners (points on the axis) before building the plane.
                let mut pts: Vec<Vec3> = Vec::new();
                for p in quad {
                    if pts.last().is_none_or(|q: &Vec3| !q.approx_eq(p)) {
                        pts.push(p);
                    }
                }
                if pts.len() > 1 && pts[0].approx_eq(*pts.last().unwrap()) {
                    pts.pop();
                }
                if pts.len() < 3 {
                    continue;
                }
                let Some(normal) = newell_normal(&pts).normalized() else {
                    continue;
                };
                if line_kind == Some("plane") {
                    if let Surface::Revolved { .. } = surfaces[surface] {
                        surfaces[surface] = Surface::Plane {
                            normal,
                            offset: normal.dot(pts[0]),
                        };
                    }
                }
                let x_axis = (pts[1] - pts[0]).normalized().unwrap();
                let face_plane = Plane {
                    origin: pts[0],
                    x_axis,
                    y_axis: normal.cross(x_axis),
                    normal,
                };
                polys.push(Polygon {
                    plane: face_plane,
                    loops: vec![pts],
                    surface,
                    origin: FaceOrigin { feature, local },
                });
            }
            local += 1;
        }
    };
    walls(&profile.outer);
    for h in &profile.holes {
        walls(h);
    }
    Solid::from_polygons(polys, surfaces)
}

pub(crate) fn newell_normal(pts: &[Vec3]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        n += a.cross(b);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_sketch::{ProfileOptions, Sketch};
    use std::f64::consts::{PI, TAU};

    fn profile(build: impl FnOnce(&mut Sketch)) -> Profile {
        let mut s = Sketch::new();
        build(&mut s);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        p.remove(0)
    }

    const SEG: f64 = 5.0 * PI / 180.0;

    #[test]
    fn full_ring_matches_pappus() {
        // Rectangle 2 wide (x 3..5), 1 tall, revolved about the sketch y axis.
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(3.0, 0.0), Vec2::new(5.0, 1.0));
        });
        let solid = revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).unwrap();
        solid.validate().unwrap();
        let expected = TAU * 4.0 * 2.0; // 2π · centroid radius · area
        assert!(
            ((solid.volume() - expected) / expected).abs() < 3e-3,
            "vol {}",
            solid.volume()
        );
        assert_eq!(solid.faces.len(), 4 * 72);
    }

    #[test]
    fn profile_on_either_side_gives_positive_volume() {
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(-5.0, 0.0), Vec2::new(-3.0, 1.0));
        });
        let solid = revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).unwrap();
        solid.validate().unwrap();
        assert!(solid.volume() > 0.0);
    }

    #[test]
    fn quarter_turn_has_caps() {
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(3.0, 0.0), Vec2::new(5.0, 1.0));
        });
        let solid = revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, PI / 2.0, SEG, 1).unwrap();
        solid.validate().unwrap();
        let expected = (PI / 2.0) * 4.0 * 2.0;
        assert!(
            ((solid.volume() - expected) / expected).abs() < 3e-3,
            "vol {}",
            solid.volume()
        );
        let reversed = revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, -PI / 2.0, SEG, 1).unwrap();
        reversed.validate().unwrap();
        assert!((reversed.volume() - solid.volume()).abs() < 1e-6);
    }

    #[test]
    fn shaft_with_edge_on_axis() {
        // Rectangle touching the axis: a plain cylinder of radius 2, height 5.
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(2.0, 5.0));
        });
        let solid = revolve(&p, &Plane::XZ, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).unwrap();
        solid.validate().unwrap();
        let expected = PI * 4.0 * 5.0;
        assert!(
            ((solid.volume() - expected) / expected).abs() < 3e-3,
            "vol {}",
            solid.volume()
        );
    }

    #[test]
    fn sphere_from_semicircle() {
        // Semicircle of radius 3 closed by a line on the axis.
        let p = profile(|s| {
            let (l, a, b) = s.add_line(Vec2::new(0.0, -3.0), Vec2::new(0.0, 3.0));
            let (_, _, s0, s1) = s.add_arc(Vec2::ZERO, Vec2::new(0.0, 3.0), Vec2::new(0.0, -3.0));
            s.add_constraint(ok_sketch::Constraint::Coincident { a: b, b: s0 });
            s.add_constraint(ok_sketch::Constraint::Coincident { a, b: s1 });
            let _ = l;
        });
        let solid = revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).unwrap();
        solid.validate().unwrap();
        let expected = 4.0 / 3.0 * PI * 27.0;
        assert!(
            ((solid.volume() - expected) / expected).abs() < 5e-3,
            "vol {}",
            solid.volume()
        );
        let revolved = solid
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Revolved { .. }))
            .count();
        assert_eq!(revolved, 1, "one shared surface for the arc");
    }

    #[test]
    fn crossing_the_axis_is_an_error() {
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(-1.0, 0.0), Vec2::new(1.0, 1.0));
        });
        assert!(revolve(&p, &Plane::XY, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).is_err());
    }

    #[test]
    fn revolved_solid_booleans_with_a_box() {
        let p = profile(|s| {
            s.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(2.0, 5.0));
        });
        let shaft = revolve(&p, &Plane::XZ, Vec2::ZERO, Vec2::Y, TAU, SEG, 1).unwrap();
        let block = crate::extrude(
            &profile(|s| {
                s.add_rectangle(Vec2::new(-3.0, -3.0), Vec2::new(3.0, 3.0));
            }),
            &Plane::XY.offset(2.0),
            0.0,
            1.0,
            2,
        )
        .unwrap();
        let cut = crate::boolean(&block, &shaft, crate::BoolOp::Difference).unwrap();
        cut.validate().unwrap();
        let expected = 36.0 - PI * 4.0;
        assert!(
            ((cut.volume() - expected) / expected).abs() < 5e-3,
            "vol {}",
            cut.volume()
        );
    }
}
