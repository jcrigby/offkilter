//! Lofts: a ruled solid between two planar profiles.
//!
//! The two outer loops (and each pair of holes) are resampled by arc
//! length to a common vertex count, aligned to minimise twist, and joined
//! by triangle facets tagged per segment of the denser loop as
//! `Surface::Ruled` groups.

use crate::revolve::newell_normal;
use crate::{flip_plane, BrepError, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec3};
use ok_sketch::{Loop, Profile, SegmentCurve};

/// Points of a loop in 3D.
fn lift(l: &Loop, plane: &Plane) -> Vec<Vec3> {
    l.points.iter().map(|&p| plane.to_world(p)).collect()
}

/// Resamples a closed polyline to `count` points by arc length, keeping
/// the first point.
fn resample(ring: &[Vec3], count: usize) -> Vec<Vec3> {
    let n = ring.len();
    if n == count {
        return ring.to_vec();
    }
    let mut cum = vec![0.0; n + 1];
    for i in 0..n {
        cum[i + 1] = cum[i] + ring[i].distance(ring[(i + 1) % n]);
    }
    let total = cum[n];
    let mut out = Vec::with_capacity(count);
    let mut seg = 0;
    for k in 0..count {
        let target = total * k as f64 / count as f64;
        while seg + 1 < n && cum[seg + 1] < target {
            seg += 1;
        }
        let a = ring[seg];
        let b = ring[(seg + 1) % n];
        let len = cum[seg + 1] - cum[seg];
        let t = if len > 0.0 {
            (target - cum[seg]) / len
        } else {
            0.0
        };
        out.push(a + (b - a) * t);
    }
    out
}

/// Rotation offset of `b` that best matches `a` point by point.
fn best_offset(a: &[Vec3], b: &[Vec3]) -> usize {
    let n = a.len();
    (0..n)
        .min_by(|&x, &y| {
            let dx: f64 = (0..n).map(|i| a[i].distance(b[(i + x) % n])).sum();
            let dy: f64 = (0..n).map(|i| a[i].distance(b[(i + y) % n])).sum();
            dx.partial_cmp(&dy).unwrap()
        })
        .unwrap_or(0)
}

/// Lofts from `a` (in `plane_a`) to `b` (in `plane_b`).
pub fn loft(
    a: &Profile,
    plane_a: &Plane,
    b: &Profile,
    plane_b: &Plane,
    feature: u32,
) -> Result<Solid, BrepError> {
    if a.outer.len() < 3 || b.outer.len() < 3 {
        return Err(BrepError::Degenerate(
            "profiles need at least three points".into(),
        ));
    }
    if a.holes.len() != b.holes.len() {
        return Err(BrepError::Degenerate(
            "profiles must have the same number of holes".into(),
        ));
    }
    let centroid = |ring: &[Vec3]| ring.iter().fold(Vec3::ZERO, |s, p| s + *p) / ring.len() as f64;
    let ring_a0 = lift(&a.outer, plane_a);
    let ring_b0 = lift(&b.outer, plane_b);
    let dir = centroid(&ring_b0) - centroid(&ring_a0);
    if dir.length() <= ok_math::tol::LINEAR {
        return Err(BrepError::Degenerate("profiles coincide".into()));
    }
    // Both loops counter-clockwise about the loft direction.
    let flip_a = plane_a.normal.dot(dir) < 0.0;
    let flip_b = plane_b.normal.dot(dir) < 0.0;
    let orient = |l: &Loop, flip: bool| if flip { l.reversed() } else { l.clone() };

    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();

    // Caps: A faces backwards, B forwards.
    let cap_a_plane = if flip_a {
        *plane_a
    } else {
        flip_plane(plane_a)
    };
    surfaces.push(Surface::Plane {
        normal: cap_a_plane.normal,
        offset: cap_a_plane.normal.dot(cap_a_plane.origin),
    });
    polys.push(Polygon {
        plane: cap_a_plane,
        loops: std::iter::once(&a.outer)
            .chain(a.holes.iter())
            .map(|l| lift(&orient(l, !flip_a), plane_a))
            .collect(),
        surface: 0,
        origin: FaceOrigin { feature, local: 0 },
    });
    let cap_b_plane = if flip_b {
        flip_plane(plane_b)
    } else {
        *plane_b
    };
    surfaces.push(Surface::Plane {
        normal: cap_b_plane.normal,
        offset: cap_b_plane.normal.dot(cap_b_plane.origin),
    });
    polys.push(Polygon {
        plane: cap_b_plane,
        loops: std::iter::once(&b.outer)
            .chain(b.holes.iter())
            .map(|l| lift(&orient(l, flip_b), plane_b))
            .collect(),
        surface: 1,
        origin: FaceOrigin { feature, local: 1 },
    });

    let mut local = 2u32;
    let pairs: Vec<(Loop, Loop)> =
        std::iter::once((orient(&a.outer, flip_a), orient(&b.outer, flip_b)))
            .chain(
                a.holes
                    .iter()
                    .zip(b.holes.iter())
                    .map(|(ha, hb)| (orient(ha, flip_a), orient(hb, flip_b))),
            )
            .collect();
    for (la, lb) in &pairs {
        let ra0 = lift(la, plane_a);
        let rb0 = lift(lb, plane_b);
        let count = ra0.len().max(rb0.len());
        let ra = resample(&ra0, count);
        let mut rb = resample(&rb0, count);
        let off = best_offset(&ra, &rb);
        rb.rotate_left(off);
        // Surface groups follow the denser loop's segment tags.
        let dense = if ra0.len() >= rb0.len() { la } else { lb };
        let mut shared: Option<(ok_math::Vec2, f64, usize)> = None;
        for i in 0..count {
            let surface = match dense.curves.get(i).copied().unwrap_or(SegmentCurve::Line) {
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
            let j = (i + 1) % count;
            let quad = [ra[i], ra[j], rb[j], rb[i]];
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
            local += 1;
        }
    }
    Solid::from_polygons(polys, surfaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn square(side: f64) -> Profile {
        let mut s = Sketch::new();
        s.add_rectangle(
            Vec2::new(-side / 2.0, -side / 2.0),
            Vec2::new(side / 2.0, side / 2.0),
        );
        s.profiles(&ProfileOptions::default()).remove(0)
    }

    fn circle(r: f64) -> Profile {
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, r);
        s.profiles(&ProfileOptions::default()).remove(0)
    }

    #[test]
    fn loft_between_equal_squares_is_a_box() {
        let solid = loft(
            &square(2.0),
            &Plane::XY,
            &square(2.0),
            &Plane::XY.offset(5.0),
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        assert!(
            (solid.volume() - 20.0).abs() < 1e-9,
            "vol {}",
            solid.volume()
        );
    }

    #[test]
    fn loft_to_a_smaller_square_is_a_frustum() {
        let solid = loft(
            &square(4.0),
            &Plane::XY,
            &square(2.0),
            &Plane::XY.offset(3.0),
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        let (a1, a2): (f64, f64) = (16.0, 4.0);
        let expected = 3.0 / 3.0 * (a1 + a2 + (a1 * a2).sqrt());
        assert!(
            (solid.volume() - expected).abs() < 1e-9,
            "vol {} expected {expected}",
            solid.volume()
        );
    }

    #[test]
    fn loft_circle_to_circle_is_a_cone_frustum() {
        let solid = loft(
            &circle(4.0),
            &Plane::XY,
            &circle(2.0),
            &Plane::XY.offset(6.0),
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        let pi = std::f64::consts::PI;
        let expected = 6.0 / 3.0 * pi * (16.0 + 4.0 + 8.0);
        assert!(
            ((solid.volume() - expected) / expected).abs() < 3e-3,
            "vol {} expected {expected}",
            solid.volume()
        );
        assert_eq!(
            solid
                .surfaces
                .iter()
                .filter(|s| matches!(s, Surface::Ruled))
                .count(),
            1,
            "one smooth group"
        );
    }

    #[test]
    fn loft_square_to_circle_and_reversed_planes() {
        let solid = loft(
            &square(4.0),
            &Plane::XY,
            &circle(2.0),
            &Plane::XY.offset(4.0),
            1,
        )
        .unwrap();
        solid.validate().unwrap();
        assert!(solid.volume() > 4.0 * std::f64::consts::PI * 4.0 && solid.volume() < 64.0);
        // Second profile on a plane facing the other way still lofts outward.
        let flipped = Plane {
            normal: -Plane::XY.normal,
            x_axis: Plane::XY.y_axis,
            y_axis: Plane::XY.x_axis,
            ..Plane::XY
        }
        .offset(-4.0);
        let s2 = loft(&square(4.0), &Plane::XY, &circle(2.0), &flipped, 1).unwrap();
        s2.validate().unwrap();
        assert!((s2.volume() - solid.volume()).abs() < 1e-6);
    }
}
