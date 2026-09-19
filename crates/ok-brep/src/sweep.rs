//! Sweeps: a profile carried along a path polyline.
//!
//! The profile is placed at the start of the path with its normal along
//! the first segment, then transported along the path with a
//! rotation-minimising frame. At each interior joint the ring is projected
//! along the incoming direction onto the bisector plane, which mitres the
//! corner so the cross-section stays constant. Facets between rings are
//! triangles (the quads are not planar in general) tagged per profile
//! segment as a `Surface::Ruled` group.

use crate::revolve::newell_normal;
use crate::{flip_plane, BrepError, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{Loop, Profile, SegmentCurve};

/// Minimal rotation taking unit `a` to unit `b`, applied to `v`.
fn rotate_between(a: Vec3, b: Vec3, v: Vec3) -> Vec3 {
    let c = a.dot(b);
    if c > 1.0 - 1e-12 {
        return v;
    }
    if c < -1.0 + 1e-12 {
        // 180°: rotate about any axis perpendicular to a.
        let axis = a
            .cross(if a.x.abs() < 0.9 { Vec3::X } else { Vec3::Y })
            .normalized()
            .unwrap();
        return axis * (2.0 * axis.dot(v)) - v;
    }
    let axis = a.cross(b);
    let s = axis.length();
    let k = axis / s;
    // Rodrigues with sin = s, cos = c.
    v * c + k.cross(v) * s + k * (k.dot(v) * (1.0 - c))
}

/// Sweeps `profile` (in `plane`) along `path`. The profile's plane origin
/// is carried to the first path point.
pub fn sweep(
    profile: &Profile,
    plane: &Plane,
    path: &[Vec3],
    feature: u32,
) -> Result<Solid, BrepError> {
    sweep_path(profile, plane, path, false, feature)
}

/// Sweeps `profile` around a closed `path` (the last point joins back to
/// the first): a ring with mitred joints and no caps. Frames are
/// transported around the loop; a non-planar loop may leave a small twist
/// where the ends meet.
pub fn sweep_closed(
    profile: &Profile,
    plane: &Plane,
    path: &[Vec3],
    feature: u32,
) -> Result<Solid, BrepError> {
    sweep_path(profile, plane, path, true, feature)
}

fn sweep_path(
    profile: &Profile,
    plane: &Plane,
    path: &[Vec3],
    closed: bool,
    feature: u32,
) -> Result<Solid, BrepError> {
    if profile.outer.len() < 3 {
        return Err(BrepError::Degenerate(
            "profile has fewer than three points".into(),
        ));
    }
    // Drop repeated points (and, for a loop, a repeated first point).
    let mut pts: Vec<Vec3> = Vec::new();
    for &p in path {
        if pts.last().is_none_or(|q: &Vec3| q.distance(p) > 1e-9) {
            pts.push(p);
        }
    }
    if closed && pts.len() > 1 && pts[0].distance(*pts.last().unwrap()) <= 1e-9 {
        pts.pop();
    }
    if pts.len() < if closed { 3 } else { 2 } {
        return Err(BrepError::Degenerate(
            "path needs at least two distinct points".into(),
        ));
    }
    let n = pts.len();
    // Segment tangents; a loop has one more segment, back to the start.
    let seg_count = if closed { n } else { n - 1 };
    let tangents: Vec<Vec3> = (0..seg_count)
        .map(|i| (pts[(i + 1) % n] - pts[i]).normalized().unwrap())
        .collect();
    // Tangents into and out of each ring.
    let t_in = |i: usize| -> Vec3 {
        if i == 0 {
            if closed {
                tangents[n - 1]
            } else {
                tangents[0]
            }
        } else {
            tangents[i - 1]
        }
    };
    let t_out = |i: usize| -> Vec3 {
        if i < seg_count {
            tangents[i]
        } else {
            tangents[i - 1]
        }
    };

    // Frame at each ring, perpendicular to its outgoing tangent: start
    // aligned with the first segment, then transported segment to segment.
    let mut frames: Vec<Plane> = Vec::with_capacity(n);
    let x0 = rotate_between(plane.normal, tangents[0], plane.x_axis);
    let y0 = rotate_between(plane.normal, tangents[0], plane.y_axis);
    frames.push(Plane {
        origin: pts[0],
        x_axis: x0,
        y_axis: y0,
        normal: tangents[0],
    });
    for i in 1..n {
        let prev = frames[i - 1];
        let x = rotate_between(t_in(i), t_out(i), prev.x_axis);
        let y = rotate_between(t_in(i), t_out(i), prev.y_axis);
        frames.push(Plane {
            origin: pts[i],
            x_axis: x,
            y_axis: y,
            normal: t_out(i),
        });
    }

    // Ring points: place the profile in the incoming orientation (the ring
    // frame rotated back to the incoming tangent) and project it along the
    // incoming direction onto the bisector plane, which mitres the joint.
    let ring = |i: usize, p: Vec2| -> Vec3 {
        let f = frames[i];
        let (ti, to) = (t_in(i), t_out(i));
        if ti.distance(to) <= 1e-12 {
            return f.origin + f.x_axis * p.x + f.y_axis * p.y;
        }
        let ix = rotate_between(to, ti, f.x_axis);
        let iy = rotate_between(to, ti, f.y_axis);
        let r = f.origin + ix * p.x + iy * p.y;
        let nb = (ti + to).normalized().unwrap_or(to);
        let denom = ti.dot(nb);
        if denom.abs() < 1e-9 {
            return r;
        }
        let s = -(r - f.origin).dot(nb) / denom;
        r + ti * s
    };

    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();

    // Caps (open sweeps only).
    let loops_of = |i: usize, reversed: bool| -> Vec<Vec<Vec3>> {
        std::iter::once(&profile.outer)
            .chain(profile.holes.iter())
            .map(|l| {
                let l = if reversed { l.reversed() } else { l.clone() };
                l.points.iter().map(|&p| ring(i, p)).collect()
            })
            .collect()
    };
    if !closed {
        let start_plane = flip_plane(&frames[0]);
        surfaces.push(Surface::Plane {
            normal: start_plane.normal,
            offset: start_plane.normal.dot(start_plane.origin),
        });
        polys.push(Polygon {
            plane: start_plane,
            loops: loops_of(0, true),
            surface: 0,
            origin: FaceOrigin { feature, local: 0 },
        });
        let end_plane = frames[n - 1];
        surfaces.push(Surface::Plane {
            normal: end_plane.normal,
            offset: end_plane.normal.dot(end_plane.origin),
        });
        polys.push(Polygon {
            plane: end_plane,
            loops: loops_of(n - 1, false),
            surface: 1,
            origin: FaceOrigin { feature, local: 1 },
        });
    }

    let mut local = 2u32;
    let mut walls = |ring_loop: &Loop| {
        let count = ring_loop.len();
        let mut shared: Option<(Vec2, f64, usize)> = None;
        for i in 0..count {
            let a = ring_loop.points[i];
            let b = ring_loop.points[(i + 1) % count];
            let surface = match ring_loop.curves[i] {
                SegmentCurve::Line => {
                    surfaces.push(Surface::Ruled);
                    shared = None;
                    surfaces.len() - 1
                }
                SegmentCurve::Arc { center, radius } => match shared.filter(|(c, r, _)| {
                    c.approx_eq(center) && (r - radius).abs() <= ok_math::tol::LINEAR
                }) {
                    Some((_, _, id)) => id,
                    None => {
                        surfaces.push(Surface::Ruled);
                        let id = surfaces.len() - 1;
                        shared = Some((center, radius, id));
                        id
                    }
                },
                SegmentCurve::Spline { id } => {
                    let key = ok_math::Vec2::new(id as f64, f64::NAN);
                    match shared.filter(|(c, _, _)| c.x == key.x && c.y.is_nan()) {
                        Some((_, _, s)) => s,
                        None => {
                            surfaces.push(Surface::Ruled);
                            let s = surfaces.len() - 1;
                            shared = Some((key, 0.0, s));
                            s
                        }
                    }
                }
            };
            for k in 0..seg_count {
                let k1 = (k + 1) % n;
                let quad = [ring(k, a), ring(k, b), ring(k1, b), ring(k1, a)];
                for tri in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                    let Some(normal) = newell_normal(&tri).normalized() else {
                        continue;
                    };
                    let x_axis = (tri[1] - tri[0]).normalized().unwrap();
                    let face_plane = Plane {
                        origin: tri[0],
                        x_axis,
                        y_axis: normal.cross(x_axis),
                        normal,
                    };
                    polys.push(Polygon {
                        plane: face_plane,
                        loops: vec![tri.to_vec()],
                        surface,
                        origin: FaceOrigin { feature, local },
                    });
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ok_sketch::{ProfileOptions, Sketch};

    fn square(side: f64) -> Profile {
        let mut s = Sketch::new();
        s.add_rectangle(
            Vec2::new(-side / 2.0, -side / 2.0),
            Vec2::new(side / 2.0, side / 2.0),
        );
        s.profiles(&ProfileOptions::default()).remove(0)
    }

    #[test]
    fn straight_sweep_is_a_box() {
        // Profile on the Right plane (normal +X), path along +X.
        let solid = sweep(
            &square(2.0),
            &Plane::YZ,
            &[Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)],
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        assert!(
            (solid.volume() - 20.0).abs() < 1e-9,
            "vol {}",
            solid.volume()
        );
        let (min, max) = solid.bounds().unwrap();
        assert!(
            min.approx_eq(Vec3::new(0.0, -1.0, -1.0)) && max.approx_eq(Vec3::new(5.0, 1.0, 1.0)),
            "{min:?} {max:?}"
        );
    }

    #[test]
    fn l_shaped_sweep_keeps_cross_section() {
        let path = [
            Vec3::ZERO,
            Vec3::new(6.0, 0.0, 0.0),
            Vec3::new(6.0, 4.0, 0.0),
        ];
        let solid = sweep(&square(2.0), &Plane::YZ, &path, 1).unwrap();
        solid.validate().unwrap();
        // Mitred corner: volume is area times path length for a centred profile.
        assert!(
            (solid.volume() - 4.0 * 10.0).abs() < 1e-9,
            "vol {}",
            solid.volume()
        );
    }

    #[test]
    fn sweep_around_an_arc_path() {
        // Quarter circle of radius 10 in the XY plane, sampled every 5°.
        let path: Vec<Vec3> = (0..=18)
            .map(|i| {
                let t = (i as f64) * 5f64.to_radians();
                Vec3::new(10.0 * t.sin(), 10.0 - 10.0 * t.cos(), 0.0)
            })
            .collect();
        let solid = sweep(&square(2.0), &Plane::YZ, &path, 1).unwrap();
        solid.validate().unwrap();
        // Pappus: area × path length of the centroid (radius 10 quarter turn).
        let expected = 4.0 * (std::f64::consts::PI / 2.0) * 10.0;
        assert!(
            ((solid.volume() - expected) / expected).abs() < 2e-3,
            "vol {} expected {expected}",
            solid.volume()
        );
    }

    #[test]
    fn swept_tube_with_a_hole() {
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, 3.0);
        s.add_circle(Vec2::ZERO, 2.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let solid = sweep(
            &p[0],
            &Plane::XY,
            &[Vec3::ZERO, Vec3::new(0.0, 0.0, 8.0)],
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        let expected = std::f64::consts::PI * (9.0 - 4.0) * 8.0;
        assert!(((solid.volume() - expected) / expected).abs() < 3e-3);
    }

    #[test]
    fn closed_sweep_around_a_square_ring() {
        // A 1x1 profile carried around a 10x10 square loop: a picture-frame
        // ring with mitred corners, no caps. Volume = perimeter along the
        // centreline (4 * 10) times the section area (1).
        let path = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
        ];
        let solid = sweep_closed(&square(1.0), &Plane::YZ, &path, 1).unwrap();
        solid.validate().unwrap();
        assert!(
            (solid.volume() - 40.0).abs() < 1e-9,
            "vol {}",
            solid.volume()
        );
        // Every face is a wall: no planar caps.
        assert!(solid.surfaces.iter().all(|s| matches!(s, Surface::Ruled)));
    }

    #[test]
    fn closed_sweep_around_a_circle_is_a_torus() {
        let r = 10.0;
        let n = 72;
        let path: Vec<Vec3> = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                Vec3::new(r * a.cos(), r * a.sin(), 0.0)
            })
            .collect();
        // Profile on the ZX plane at (r, 0, 0) (right-handed: Z x X = Y).
        let plane = Plane {
            origin: Vec3::new(r, 0.0, 0.0),
            x_axis: Vec3::Z,
            y_axis: Vec3::X,
            normal: Vec3::Y,
        };
        let solid = sweep_closed(&square(2.0), &plane, &path, 1).unwrap();
        solid.validate().unwrap();
        // Pappus with the polygonal centreline: 72 * chord * 4.
        let chord = 2.0 * r * (std::f64::consts::PI / n as f64).sin();
        let expected = n as f64 * chord * 4.0;
        assert!(
            (solid.volume() - expected).abs() / expected < 1e-3,
            "vol {} expected {expected}",
            solid.volume()
        );
    }
}
