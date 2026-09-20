//! Edge blends: fillets and chamfers.
//!
//! Each selected edge segment gets a prism whose cross-section is the
//! corner between its two faces: a triangle for a chamfer, or the corner
//! minus the tangent arc for a fillet. Convex edges have the prism
//! subtracted; concave edges have it added. The arc is tagged as a cylinder
//! about the edge, so fillets shade smoothly and are one selectable face.

#[cfg(test)]
use crate::Surface;
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
///
/// Segments between the same two surfaces that join end to end (the rim
/// of a faceted cylinder, say) are blended as one chain: the cross-section
/// of the first segment is swept along the chain with mitred joints, so
/// the blend is continuous around the rim. The section is that of the
/// first segment, which is exact when the dihedral angle is constant along
/// the chain (rims on planar faces) and an approximation otherwise.
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
    // Segments, each oriented as its face A traverses it, where A is the
    // face on the lower-numbered surface so a chain is oriented consistently.
    struct Seg {
        a: u32,
        b: u32,
        geom: EdgeSegment,
        surfaces: (usize, usize),
        faces: (usize, usize),
    }
    let mut segments: Vec<Seg> = Vec::new();
    for (key, faces) in solid.edge_faces() {
        if faces.len() != 2 || !wanted(faces[0], faces[1]) {
            continue;
        }
        let (mut ia, mut ib) = (faces[0], faces[1]);
        if solid.faces[ia].surface > solid.faces[ib].surface {
            std::mem::swap(&mut ia, &mut ib);
        }
        let (fa, fb) = (&solid.faces[ia], &solid.faces[ib]);
        if fa.surface == fb.surface {
            continue; // a seam inside one surface is not an edge to blend
        }
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
        segments.push(Seg {
            a,
            b,
            geom: EdgeSegment {
                p,
                q,
                inward_a,
                inward_b,
                convex,
            },
            surfaces: (fa.surface, fb.surface),
            faces: (ia, ib),
        });
    }
    if segments.is_empty() {
        return Err(BrepError::Degenerate("no matching edges to blend".into()));
    }

    // Where three convex fillets on planar faces meet at a vertex, the
    // corner gets a spherical patch; those edges and corners become one
    // cutter each group (see `corner.rs`) and leave the chains below.
    let mut used = vec![false; segments.len()];
    let mut cutters = Solid::default();
    if kind == BlendKind::Fillet {
        let mut candidates: Vec<(usize, crate::corner::FilletEdge)> = Vec::new();
        for (i, seg) in segments.iter().enumerate() {
            let planar =
                |s: usize| matches!(solid.surfaces.get(s), Some(crate::Surface::Plane { .. }));
            if !seg.geom.convex || !planar(seg.surfaces.0) || !planar(seg.surfaces.1) {
                continue;
            }
            let g = &seg.geom;
            let e = (g.q - g.p).normalized().unwrap();
            let x = g.inward_a;
            let y = e.cross(x);
            let Ok(section) = cross_section(g, x, y, size, kind, segment_angle) else {
                continue;
            };
            let (fa, fb) = seg.faces;
            candidates.push((
                i,
                crate::corner::FilletEdge {
                    a: seg.a,
                    b: seg.b,
                    faces: (fa, fb),
                    frame: Plane {
                        origin: g.p,
                        x_axis: x,
                        y_axis: y,
                        normal: e,
                    },
                    section,
                },
            ));
        }
        let edges: Vec<crate::corner::FilletEdge> =
            candidates.iter().map(|(_, e)| e.clone()).collect();
        let (patched, taken) =
            crate::corner::patched_cutters(solid, &edges, size, segment_angle, feature);
        for (k, &(i, _)) in candidates.iter().enumerate() {
            if taken[k] {
                used[i] = true;
            }
        }
        for cutter in patched {
            cutters = boolean(&cutters, &cutter, BoolOp::Union)?;
        }
    }

    // Chain segments head to tail within one surface pair and convexity.
    let mut chains: Vec<(Vec<usize>, bool)> = Vec::new();
    let same_kind = |i: usize, j: usize| {
        segments[i].surfaces == segments[j].surfaces
            && segments[i].geom.convex == segments[j].geom.convex
    };
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = vec![start];
        // Extend forward (q -> p of the next) and backward (p <- q of the previous).
        loop {
            let last = *chain.last().unwrap();
            let next = (0..segments.len())
                .find(|&j| !used[j] && same_kind(last, j) && segments[j].a == segments[last].b);
            match next {
                Some(j) => {
                    used[j] = true;
                    chain.push(j);
                }
                None => break,
            }
        }
        let closed = chain.len() > 1 && segments[*chain.last().unwrap()].b == segments[start].a;
        if !closed {
            loop {
                let first = chain[0];
                let prev = (0..segments.len()).find(|&j| {
                    !used[j] && same_kind(first, j) && segments[j].b == segments[first].a
                });
                match prev {
                    Some(j) => {
                        used[j] = true;
                        chain.insert(0, j);
                    }
                    None => break,
                }
            }
        }
        chains.push((chain, closed));
    }

    let mut fillers = Solid::default();
    for (k, (chain, closed)) in chains.iter().enumerate() {
        let first = &segments[chain[0]].geom;
        let e = (first.q - first.p).normalized().unwrap();
        let x = first.inward_a;
        let y = e.cross(x);
        let section = cross_section(first, x, y, size, kind, segment_angle)?;
        let frame = Plane {
            origin: first.p,
            x_axis: x,
            y_axis: y,
            normal: e,
        };
        let profile = Profile {
            outer: section,
            holes: vec![],
        };
        let mut prism = if chain.len() == 1 {
            extrude(&profile, &frame, 0.0, first.q.distance(first.p), feature)?
        } else {
            let mut path: Vec<Vec3> = chain.iter().map(|&i| segments[i].geom.p).collect();
            if !closed {
                path.push(segments[*chain.last().unwrap()].geom.q);
            }
            if *closed {
                crate::sweep_closed(&profile, &frame, &path, feature)?
            } else {
                crate::sweep(&profile, &frame, &path, feature)?
            }
        };
        for f in &mut prism.faces {
            f.origin.local += (k as u32) * 1000;
        }
        if first.convex {
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
    fn three_fillets_meeting_at_a_corner_get_a_spherical_patch() {
        let b = block(10.0, 10.0, 10.0);
        let r = 2.0;
        let mut pairs = edges_between(&b, Vec3::Z, Vec3::X);
        pairs.extend(edges_between(&b, Vec3::Z, Vec3::Y));
        pairs.extend(edges_between(&b, Vec3::X, Vec3::Y));
        let f = blend_edges(&b, &pairs, r, BlendKind::Fillet, SEG, 5).unwrap();
        f.validate().unwrap();
        // Along each edge the fillet removes (1 - pi/4) r^2 per unit length
        // outside the corner cell; in the cell (a cube of side r) only the
        // ball's eighth stays.
        let removed = r * r * r * (1.0 - PI / 6.0) + (1.0 - PI / 4.0) * r * r * 3.0 * (10.0 - r);
        let expected = 1000.0 - removed;
        assert!(
            ((f.volume() - expected) / expected).abs() < 3e-3,
            "vol {} expected {expected}",
            f.volume()
        );
        // The patch is one smooth surface of its own besides the three cylinders.
        let cylinders = f
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Cylinder { .. }))
            .count();
        let revolved = f
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Revolved { .. }))
            .count();
        assert_eq!((cylinders, revolved), (3, 1));
        // Every patch vertex lies on the ball about the centre 2 mm inside the corner.
        let centre = Vec3::new(10.0 - r, 10.0 - r, 10.0 - r);
        let patch = f
            .surfaces
            .iter()
            .position(|s| matches!(s, Surface::Revolved { .. }))
            .unwrap();
        for face in f.faces.iter().filter(|face| face.surface == patch) {
            for &v in &face.loops[0] {
                let d = f.vertices[v as usize].distance(centre);
                assert!((d - r).abs() < 1e-6, "patch vertex {d} from the centre");
            }
        }
    }

    #[test]
    fn filleting_every_edge_of_a_block_rounds_all_eight_corners() {
        let b = block(10.0, 10.0, 10.0);
        let r = 2.0;
        let pairs: Vec<(usize, usize)> = b
            .edge_faces()
            .into_values()
            .filter(|f| f.len() == 2)
            .map(|f| (f[0], f[1]))
            .collect();
        let f = blend_edges(&b, &pairs, r, BlendKind::Fillet, SEG, 5).unwrap();
        f.validate().unwrap();
        let s = 10.0 - 2.0 * r;
        let expected =
            s * s * s + 2.0 * r * 3.0 * s * s + PI * r * r * 3.0 * s + 4.0 / 3.0 * PI * r * r * r;
        assert!(
            ((f.volume() - expected) / expected).abs() < 3e-3,
            "vol {} expected {expected}",
            f.volume()
        );
        let revolved = f
            .surfaces
            .iter()
            .filter(|s| matches!(s, Surface::Revolved { .. }))
            .count();
        assert_eq!(revolved, 8);
        // Nothing sticks out past the rounded shape: every vertex is within
        // the rounded box (the inner box grown by r).
        for v in &f.vertices {
            let inside = |x: f64| x.clamp(r, 10.0 - r);
            let nearest = Vec3::new(inside(v.x), inside(v.y), inside(v.z));
            assert!(
                v.distance(nearest) <= r + 1e-6,
                "vertex {v:?} outside the rounded block"
            );
        }
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

    fn cylinder(r: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, r);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    /// Face pairs between the top cap and every wall facet of a cylinder.
    fn rim_pairs(solid: &Solid) -> Vec<(usize, usize)> {
        let top = solid
            .faces
            .iter()
            .position(|f| f.plane.normal.approx_eq(Vec3::Z))
            .unwrap();
        (0..solid.faces.len())
            .filter(|&i| {
                matches!(
                    solid.surfaces[solid.faces[i].surface],
                    Surface::Cylinder { .. }
                )
            })
            .map(|i| (top, i))
            .collect()
    }

    #[test]
    fn fillet_the_rim_of_a_cylinder() {
        // Removed material is the corner square minus the tangent quarter
        // disc, revolved about the axis (Pappus): 2π (r²(R − r/2) − (πr²/4)(R − r) − r³/3).
        let (big_r, h, r) = (10.0, 5.0, 2.0);
        let solid = cylinder(big_r, h);
        let before = solid.volume();
        let out = blend_edges(&solid, &rim_pairs(&solid), r, BlendKind::Fillet, SEG, 7).unwrap();
        out.validate().unwrap();
        let removed = std::f64::consts::TAU
            * (r * r * (big_r - r / 2.0)
                - std::f64::consts::PI * r * r / 4.0 * (big_r - r)
                - r * r * r / 3.0);
        let got = before - out.volume();
        assert!(
            (got - removed).abs() / removed < 0.02,
            "removed {got}, expected {removed}"
        );
        // One continuous blend: the top cap's outline is one circle again,
        // so the top face keeps a single loop.
        let top = out
            .faces
            .iter()
            .find(|f| f.plane.normal.approx_eq(Vec3::Z))
            .unwrap();
        assert_eq!(top.loops.len(), 1);
    }

    #[test]
    fn chamfer_the_rim_of_a_cylinder() {
        // Removed: a triangle r²/2 with centroid at R − r/3, revolved.
        let (big_r, h, r) = (10.0, 5.0, 2.0);
        let solid = cylinder(big_r, h);
        let before = solid.volume();
        let out = blend_edges(&solid, &rim_pairs(&solid), r, BlendKind::Chamfer, SEG, 7).unwrap();
        out.validate().unwrap();
        let removed = std::f64::consts::PI * r * r * (big_r - r / 3.0);
        let got = before - out.volume();
        assert!(
            (got - removed).abs() / removed < 0.02,
            "removed {got}, expected {removed}"
        );
    }
}
