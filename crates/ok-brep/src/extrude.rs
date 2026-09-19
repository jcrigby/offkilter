use crate::{flip_plane, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{Loop, Profile, SegmentCurve};

/// Extrudes a planar profile along the plane normal from height `start` to
/// height `end`, producing a closed solid. Arc segments of the profile
/// become facets sharing one cylinder surface each.
pub fn extrude(
    profile: &Profile,
    plane: &Plane,
    start: f64,
    end: f64,
    feature: u32,
) -> Result<Solid, crate::BrepError> {
    if profile.outer.len() < 3 {
        return Err(crate::BrepError::Degenerate(
            "profile has fewer than three points".into(),
        ));
    }
    if (end - start).abs() <= ok_math::tol::LINEAR {
        return Err(crate::BrepError::Degenerate("extrude depth is zero".into()));
    }
    let (lo, hi) = if start < end {
        (start, end)
    } else {
        (end, start)
    };
    let n = plane.normal;
    let mut surfaces: Vec<Surface> = Vec::new();
    let mut polys: Vec<Polygon> = Vec::new();

    let lift = |p: Vec2, h: f64| plane.to_world_at(p, h);

    // Caps.
    let top_plane = plane.offset(hi);
    let bottom_plane = flip_plane(&plane.offset(lo));
    surfaces.push(Surface::Plane {
        normal: -n,
        offset: (-n).dot(bottom_plane.origin),
    });
    polys.push(Polygon {
        plane: bottom_plane,
        loops: std::iter::once(&profile.outer)
            .chain(profile.holes.iter())
            .map(|l| l.reversed().points.iter().map(|&p| lift(p, lo)).collect())
            .collect(),
        surface: 0,
        origin: FaceOrigin { feature, local: 0 },
    });
    surfaces.push(Surface::Plane {
        normal: n,
        offset: n.dot(top_plane.origin),
    });
    polys.push(Polygon {
        plane: top_plane,
        loops: std::iter::once(&profile.outer)
            .chain(profile.holes.iter())
            .map(|l| l.points.iter().map(|&p| lift(p, hi)).collect())
            .collect(),
        surface: 1,
        origin: FaceOrigin { feature, local: 1 },
    });

    // Walls.
    let mut local = 2u32;
    let mut walls = |ring: &Loop| {
        let count = ring.len();
        let mut cyl: Option<(Vec2, f64, usize)> = None;
        let mut spline: Option<(u32, usize)> = None;
        for i in 0..count {
            let a = ring.points[i];
            let b = ring.points[(i + 1) % count];
            let quad = [lift(a, lo), lift(b, lo), lift(b, hi), lift(a, hi)];
            let Some(normal) = (quad[1] - quad[0]).cross(quad[3] - quad[0]).normalized() else {
                continue;
            };
            let x_axis = (quad[1] - quad[0]).normalized().unwrap();
            let face_plane = Plane {
                origin: quad[0],
                x_axis,
                y_axis: normal.cross(x_axis),
                normal,
            };
            let surface = match ring.curves[i] {
                SegmentCurve::Line => {
                    surfaces.push(Surface::Plane {
                        normal,
                        offset: normal.dot(quad[0]),
                    });
                    surfaces.len() - 1
                }
                SegmentCurve::Arc { center, radius } => {
                    let reuse = cyl.filter(|(c, r, _)| {
                        c.approx_eq(center) && (r - radius).abs() <= ok_math::tol::LINEAR
                    });
                    match reuse {
                        Some((_, _, id)) => id,
                        None => {
                            surfaces.push(Surface::Cylinder {
                                origin: plane.to_world(center),
                                axis: n,
                                radius,
                            });
                            let id = surfaces.len() - 1;
                            cyl = Some((center, radius, id));
                            id
                        }
                    }
                }
                SegmentCurve::Spline { id } => match spline.filter(|(s, _)| *s == id) {
                    Some((_, surface)) => surface,
                    None => {
                        surfaces.push(Surface::Ruled);
                        spline = Some((id, surfaces.len() - 1));
                        surfaces.len() - 1
                    }
                },
            };
            if !matches!(ring.curves[i], SegmentCurve::Arc { .. }) {
                cyl = None;
            }
            if !matches!(ring.curves[i], SegmentCurve::Spline { .. }) {
                spline = None;
            }
            polys.push(Polygon {
                plane: face_plane,
                loops: vec![quad.to_vec()],
                surface,
                origin: FaceOrigin { feature, local },
            });
            local += 1;
        }
    };
    walls(&profile.outer);
    for h in &profile.holes {
        walls(h);
    }
    let _ = Vec3::ZERO;
    Solid::from_polygons(polys, surfaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_sketch::{ProfileOptions, Sketch};

    #[test]
    fn spline_profile_extrudes_to_one_smooth_wall() {
        let mut s = Sketch::new();
        let (_, ids) = s
            .add_spline(&[
                Vec2::new(0.0, 0.0),
                Vec2::new(5.0, 4.0),
                Vec2::new(10.0, 0.0),
            ])
            .unwrap();
        let (_, a, b) = s.add_line(Vec2::new(10.0, 0.0), Vec2::new(0.0, 0.0));
        s.add_constraint(ok_sketch::Constraint::Coincident { a: ids[2], b: a });
        s.add_constraint(ok_sketch::Constraint::Coincident { a: ids[0], b });
        let opts = ProfileOptions::default();
        let p = s.profiles(&opts).remove(0);
        let solid = extrude(&p, &Plane::XY, 0.0, 3.0, 1).unwrap();
        assert!((solid.volume() - p.area() * 3.0).abs() < 1e-9);
        // Bottom, top, the spline wall (one shared ruled surface) and the flat wall.
        assert_eq!(solid.surfaces.len(), 4);
        let ruled = solid
            .surfaces
            .iter()
            .position(|s| matches!(s, Surface::Ruled))
            .unwrap();
        let facets = solid.faces.iter().filter(|f| f.surface == ruled).count();
        assert_eq!(facets, 2 * ok_sketch::spline_pieces(&opts));
    }

    fn rect_profile(w: f64, h: f64) -> Profile {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, h));
        s.profiles(&ProfileOptions::default()).remove(0)
    }

    #[test]
    fn box_is_valid_and_has_six_faces() {
        let solid = extrude(&rect_profile(4.0, 2.0), &Plane::XY, 0.0, 3.0, 1).unwrap();
        assert_eq!(solid.faces.len(), 6);
        assert_eq!(solid.vertices.len(), 8);
        assert!((solid.volume() - 24.0).abs() < 1e-9);
        solid.validate().unwrap();
    }

    #[test]
    fn area_and_centroid_of_a_box() {
        let solid = extrude(&rect_profile(4.0, 2.0), &Plane::XY, 0.0, 3.0, 1).unwrap();
        assert!((solid.surface_area() - 2.0 * (8.0 + 12.0 + 6.0)).abs() < 1e-9);
        let c = solid.centroid().unwrap();
        assert!(c.approx_eq(ok_math::Vec3::new(2.0, 1.0, 1.5)), "{c:?}");
    }

    #[test]
    fn reversed_extrude_is_outward() {
        let solid = extrude(&rect_profile(1.0, 1.0), &Plane::XZ, 0.0, -2.0, 1).unwrap();
        assert!((solid.volume() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn tube_has_cylinder_surfaces_and_hole() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 10.0));
        s.add_circle(Vec2::new(5.0, 5.0), 2.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let solid = extrude(&p[0], &Plane::XY, 0.0, 1.0, 1).unwrap();
        let cylinders = solid
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Cylinder { .. }))
            .count();
        assert_eq!(cylinders, 1);
        let expected = 100.0 - std::f64::consts::PI * 4.0;
        assert!(
            (solid.volume() - expected).abs() < 0.1,
            "vol {}",
            solid.volume()
        );
        assert_eq!(solid.faces[1].loops.len(), 2, "top cap has a hole");
    }
}
