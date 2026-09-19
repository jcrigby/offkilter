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
    if profile.outer.len() < 3 {
        return Err(BrepError::Degenerate(
            "profile has fewer than three points".into(),
        ));
    }
    // Drop repeated points.
    let mut pts: Vec<Vec3> = Vec::new();
    for &p in path {
        if pts.last().is_none_or(|q: &Vec3| q.distance(p) > 1e-9) {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return Err(BrepError::Degenerate(
            "path needs at least two distinct points".into(),
        ));
    }
    let n = pts.len();
    let tangents: Vec<Vec3> = (0..n - 1)
        .map(|i| (pts[i + 1] - pts[i]).normalized().unwrap())
        .collect();

    // Frame at each ring: (origin, x, y, normal). Start aligned with the
    // first segment, then transported segment to segment.
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
        let t_in = tangents[i - 1];
        let t_out = if i < n - 1 {
            tangents[i]
        } else {
            tangents[i - 1]
        };
        // Transported frame keeps the incoming normal; the ring is then
        // projected onto the bisector plane along the incoming direction.
        let x = rotate_between(t_in, t_out, prev.x_axis);
        let y = rotate_between(t_in, t_out, prev.y_axis);
        frames.push(Plane {
            origin: pts[i],
            x_axis: x,
            y_axis: y,
            normal: t_out,
        });
    }

    // Ring points: for interior joints, place the profile in the incoming
    // frame and project along t_in onto the bisector plane.
    let ring = |i: usize, p: Vec2| -> Vec3 {
        let f = frames[i];
        if i == 0 || i == n - 1 {
            return f.origin + f.x_axis * p.x + f.y_axis * p.y;
        }
        let t_in = tangents[i - 1];
        let t_out = tangents[i];
        let incoming = frames[i - 1];
        // Profile in the incoming orientation, positioned at this joint.
        let ix = rotate_between(tangents[i - 1], tangents[i - 1], incoming.x_axis);
        let iy = incoming.y_axis;
        let r = f.origin + ix * p.x + iy * p.y;
        let nb = (t_in + t_out).normalized().unwrap_or(t_out);
        let denom = t_in.dot(nb);
        if denom.abs() < 1e-9 {
            return r;
        }
        let s = -(r - f.origin).dot(nb) / denom;
        r + t_in * s
    };

    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();

    // Caps.
    let loops_of = |i: usize, reversed: bool| -> Vec<Vec<Vec3>> {
        std::iter::once(&profile.outer)
            .chain(profile.holes.iter())
            .map(|l| {
                let l = if reversed { l.reversed() } else { l.clone() };
                l.points.iter().map(|&p| ring(i, p)).collect()
            })
            .collect()
    };
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
            };
            for k in 0..n - 1 {
                let quad = [ring(k, a), ring(k, b), ring(k + 1, b), ring(k + 1, a)];
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
}
