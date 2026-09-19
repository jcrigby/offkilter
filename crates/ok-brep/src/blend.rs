//! Edge blends: fillets and chamfers.
//!
//! Each selected edge segment gets a prism whose cross-section is the
//! corner between its two faces: a triangle for a chamfer, or the corner
//! minus the tangent arc for a fillet. Convex edges have the prism
//! subtracted; concave edges have it added. The arc is tagged as a cylinder
//! about the edge, so fillets shade smoothly and are one selectable face.

use crate::{boolean, extrude, BoolOp, BrepError, Solid};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{Loop, Profile, SegmentCurve};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendKind {
    Fillet,
    Chamfer,
}

struct EdgeSegment {
    p: Vec3,
    q: Vec3,
    inward_a: Vec3,
    inward_b: Vec3,
    convex: bool,
}

/// Cross-section of the blend in the frame (origin p, x = inward_a).
fn cross_section(
    seg: &EdgeSegment,
    x: Vec3,
    y: Vec3,
    size: f64,
    kind: BlendKind,
    segment_angle: f64,
) -> Result<Loop, BrepError> {
    let ib = Vec2::new(seg.inward_b.dot(x), seg.inward_b.dot(y));
    let phi = ib.y.atan2(ib.x).abs(); // angle between the faces' inward directions
    if !(1f64.to_radians()..=179f64.to_radians()).contains(&phi) {
        return Err(BrepError::Degenerate(
            "edge faces are nearly parallel".into(),
        ));
    }
    let mut points = vec![Vec2::ZERO];
    let mut curves = vec![SegmentCurve::Line];
    match kind {
        BlendKind::Chamfer => {
            points.push(Vec2::new(size, 0.0));
            curves.push(SegmentCurve::Line);
            points.push(ib * size);
            curves.push(SegmentCurve::Line);
        }
        BlendKind::Fillet => {
            let t = size / (phi / 2.0).tan();
            let ta = Vec2::new(t, 0.0);
            let tb = ib * t;
            let bisector = (Vec2::X + ib).normalized().unwrap();
            let center = bisector * (size / (phi / 2.0).sin());
            let a0 = (ta - center).angle();
            let a1 = (tb - center).angle();
            let mut sweep = a1 - a0;
            while sweep > std::f64::consts::PI {
                sweep -= std::f64::consts::TAU;
            }
            while sweep <= -std::f64::consts::PI {
                sweep += std::f64::consts::TAU;
            }
            let n = ((sweep.abs() / segment_angle).ceil() as usize).max(2);
            for i in 0..n {
                let ang = a0 + sweep * i as f64 / n as f64;
                points.push(center + Vec2::from_angle(ang) * size);
                curves.push(SegmentCurve::Arc {
                    center,
                    radius: size,
                });
            }
            points.push(tb);
            curves.push(SegmentCurve::Line);
        }
    }
    let l = Loop { points, curves };
    Ok(if l.signed_area() < 0.0 {
        l.reversed()
    } else {
        l
    })
}

/// Fillets or chamfers every edge between the given face-index pairs.
pub fn blend_edges(
    solid: &Solid,
    pairs: &[(usize, usize)],
    size: f64,
    kind: BlendKind,
    segment_angle: f64,
    feature: u32,
) -> Result<Solid, BrepError> {
    if !(size.is_finite() && size > ok_math::tol::LINEAR) {
        return Err(BrepError::Degenerate("blend size must be positive".into()));
    }
    let wanted = |fa: usize, fb: usize| {
        pairs
            .iter()
            .any(|&(x, y)| (x == fa && y == fb) || (x == fb && y == fa))
    };
    let mut segments: Vec<EdgeSegment> = Vec::new();
    for (key, faces) in solid.edge_faces() {
        if faces.len() != 2 || !wanted(faces[0], faces[1]) {
            continue;
        }
        let (fa, fb) = (&solid.faces[faces[0]], &solid.faces[faces[1]]);
        // Direction of the edge as face A traverses it.
        let mut dir: Option<(u32, u32)> = None;
        for l in &fa.loops {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if crate::edge_key(a, b) == key {
                    dir = Some((a, b));
                }
            }
        }
        let Some((a, b)) = dir else { continue };
        let (p, q) = (solid.vertices[a as usize], solid.vertices[b as usize]);
        let Some(e) = (q - p).normalized() else {
            continue;
        };
        let na = fa.plane.normal;
        let nb = fb.plane.normal;
        let inward_a = na.cross(e);
        let inward_b = -(nb.cross(e));
        let convex = inward_a.dot(nb) < 0.0;
        segments.push(EdgeSegment {
            p,
            q,
            inward_a,
            inward_b,
            convex,
        });
    }
    if segments.is_empty() {
        return Err(BrepError::Degenerate("no matching edges to blend".into()));
    }

    let mut cutters = Solid::default();
    let mut fillers = Solid::default();
    for (k, seg) in segments.iter().enumerate() {
        let e = (seg.q - seg.p).normalized().unwrap();
        let x = seg.inward_a;
        let y = e.cross(x);
        let section = cross_section(seg, x, y, size, kind, segment_angle)?;
        let frame = Plane {
            origin: seg.p,
            x_axis: x,
            y_axis: y,
            normal: e,
        };
        let mut prism = extrude(
            &Profile {
                outer: section,
                holes: vec![],
            },
            &frame,
            0.0,
            seg.q.distance(seg.p),
            feature,
        )?;
        for f in &mut prism.faces {
            f.origin.local += (k as u32) * 1000;
        }
        if seg.convex {
            cutters = boolean(&cutters, &prism, BoolOp::Union)?;
        } else {
            fillers = boolean(&fillers, &prism, BoolOp::Union)?;
        }
    }
    let mut out = solid.clone();
    if !cutters.is_empty() {
        out = boolean(&out, &cutters, BoolOp::Difference)?;
    }
    if !fillers.is_empty() {
        out = boolean(&out, &fillers, BoolOp::Union)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_sketch::{ProfileOptions, Sketch};
    use std::f64::consts::PI;

    const SEG: f64 = 5.0 * PI / 180.0;

    fn block(w: f64, d: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    /// Face index pairs for all edges whose faces have the given normals.
    fn edges_between(solid: &Solid, na: Vec3, nb: Vec3) -> Vec<(usize, usize)> {
        let fa = solid
            .faces
            .iter()
            .position(|f| f.plane.normal.approx_eq(na))
            .unwrap();
        let fb = solid
            .faces
            .iter()
            .position(|f| f.plane.normal.approx_eq(nb))
            .unwrap();
        vec![(fa, fb)]
    }

    #[test]
    fn chamfer_one_edge_of_a_block() {
        let b = block(10.0, 10.0, 10.0);
        let pairs = edges_between(&b, Vec3::Z, -Vec3::Y);
        let c = blend_edges(&b, &pairs, 2.0, BlendKind::Chamfer, SEG, 5).unwrap();
        c.validate().unwrap();
        let expected = 1000.0 - 0.5 * 2.0 * 2.0 * 10.0;
        assert!((c.volume() - expected).abs() < 1e-6, "vol {}", c.volume());
        assert_eq!(c.faces.len(), 7);
    }

    #[test]
    fn fillet_one_edge_of_a_block() {
        let b = block(10.0, 10.0, 10.0);
        let pairs = edges_between(&b, Vec3::Z, -Vec3::Y);
        let f = blend_edges(&b, &pairs, 2.0, BlendKind::Fillet, SEG, 5).unwrap();
        f.validate().unwrap();
        let expected = 1000.0 - (4.0 - PI) * 10.0;
        assert!(
            ((f.volume() - expected) / expected).abs() < 2e-3,
            "vol {}",
            f.volume()
        );
        let cylinders = f
            .surfaces
            .iter()
            .filter(|s| matches!(s, crate::Surface::Cylinder { .. }))
            .count();
        assert_eq!(cylinders, 1);
    }

    #[test]
    fn fillet_all_vertical_edges() {
        let b = block(10.0, 10.0, 4.0);
        let sides: Vec<usize> = b
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.plane.normal.z.abs() < 1e-9)
            .map(|(i, _)| i)
            .collect();
        let mut pairs = Vec::new();
        for i in 0..sides.len() {
            for j in i + 1..sides.len() {
                pairs.push((sides[i], sides[j]));
            }
        }
        let f = blend_edges(&b, &pairs, 3.0, BlendKind::Fillet, SEG, 5).unwrap();
        f.validate().unwrap();
        let expected = (100.0 - (4.0 - PI) * 9.0) * 4.0;
        assert!(
            ((f.volume() - expected) / expected).abs() < 3e-3,
            "vol {}",
            f.volume()
        );
    }

    #[test]
    fn concave_edge_fillet_adds_material() {
        // An L: a 10x10x10 block with a 5x10x5 notch removed from the top back.
        let b = block(10.0, 10.0, 10.0);
        let notch = {
            let mut s = Sketch::new();
            s.add_rectangle(Vec2::new(5.0, -1.0), Vec2::new(11.0, 11.0));
            let p = s.profiles(&ProfileOptions::default()).remove(0);
            extrude(&p, &Plane::XY, 5.0, 11.0, 2).unwrap()
        };
        let l = boolean(&b, &notch, BoolOp::Difference).unwrap();
        // The inside corner edge is between the notch floor (normal +Z, at z=5)
        // and the notch wall (normal +X, at x=5).
        let floor = l
            .faces
            .iter()
            .position(|f| {
                f.plane.normal.approx_eq(Vec3::Z)
                    && (f.plane.normal.dot(f.plane.origin) - 5.0).abs() < 1e-9
            })
            .unwrap();
        let wall = l
            .faces
            .iter()
            .position(|f| {
                f.plane.normal.approx_eq(Vec3::X)
                    && (f.plane.normal.dot(f.plane.origin) - 5.0).abs() < 1e-9
            })
            .unwrap();
        let f = blend_edges(&l, &[(floor, wall)], 2.0, BlendKind::Fillet, SEG, 5).unwrap();
        f.validate().unwrap();
        let expected = l.volume() + (4.0 - PI) * 10.0;
        assert!(
            ((f.volume() - expected) / expected).abs() < 2e-3,
            "vol {} expected {expected}",
            f.volume()
        );
    }

    #[test]
    fn chamfer_three_edges_meeting_at_a_corner() {
        let b = block(10.0, 10.0, 10.0);
        let idx = |n: Vec3| {
            b.faces
                .iter()
                .position(|f| f.plane.normal.approx_eq(n))
                .unwrap()
        };
        let (top, front, right) = (idx(Vec3::Z), idx(-Vec3::Y), idx(Vec3::X));
        let c = blend_edges(
            &b,
            &[(top, front), (top, right), (front, right)],
            2.0,
            BlendKind::Chamfer,
            SEG,
            5,
        )
        .unwrap();
        c.validate().unwrap();
        assert!(
            c.volume() < 1000.0 - 3.0 * 20.0 + 10.0 && c.volume() > 1000.0 - 3.0 * 20.0 - 10.0,
            "vol {}",
            c.volume()
        );
    }
}
