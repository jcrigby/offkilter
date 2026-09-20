//! Boolean operations on solids.
//!
//! Each face of one solid is classified against the other solid by taking
//! two cross-sections of that solid in the face's plane: one infinitesimally
//! above the plane (`X+`) and one below (`X-`). In the face's 2D frame:
//!
//! | region        | in X+ | in X- |
//! |---------------|-------|-------|
//! | inside        | yes   | yes   |
//! | outside       | no    | no    |
//! | coplanar face with the same normal     | no  | yes |
//! | coplanar face with the opposite normal | yes | no  |
//!
//! The kept part of the face is then a 2D polygon boolean of the face
//! region against those sections, which handles coplanar faces exactly
//! without any special casing. Fragments from both solids are reassembled
//! into a new solid by merging vertices, and the result is validated to be
//! closed; a non-closed result is reported as an error rather than shown.

use crate::section::{sections_above_below, FaceIndex, Section};
use crate::{
    bounds_of, bounds_overlap, flip_plane, merge_tolerance, BrepError, Face, Polygon, Solid,
};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use ok_math::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

type Contour = Vec<[f64; 2]>;

fn to_contours(loops: &[Vec<Vec2>]) -> Vec<Contour> {
    loops
        .iter()
        .map(|l| l.iter().map(|p| [p.x, p.y]).collect())
        .collect()
}

/// Keeps the contours whose bounding box overlaps the region's (grown by
/// `tol`); a contour entirely outside neither encloses nor crosses it.
fn near_region(contours: Vec<Contour>, region: &[Contour], tol: f64) -> Vec<Contour> {
    let bbox = |cs: &[Contour]| {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in cs.iter().flatten() {
            b = [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ];
        }
        b
    };
    let r = bbox(region);
    contours
        .into_iter()
        .filter(|c| {
            let b = bbox(std::slice::from_ref(c));
            b[2] >= r[0] - tol && b[0] <= r[2] + tol && b[3] >= r[1] - tol && b[1] <= r[3] + tol
        })
        .collect()
}

fn face_region(solid: &Solid, f: &Face) -> Vec<Contour> {
    f.loops
        .iter()
        .map(|l| {
            l.iter()
                .map(|&v| {
                    let q = f.plane.to_plane(solid.vertices[v as usize]);
                    [q.x, q.y]
                })
                .collect()
        })
        .collect()
}

/// Region ops on contour sets. `subject` is one polygon-with-holes; `clip`
/// is a set of consistently oriented loops combined with the non-zero rule.
/// The 64-bit engine keeps the internal grid far below model tolerance.
fn overlay(subject: &[Contour], clip: &[Contour], rule: OverlayRule) -> Vec<Vec<Contour>> {
    if clip.is_empty() {
        return match rule {
            OverlayRule::Difference | OverlayRule::Union | OverlayRule::Subject => {
                vec![subject.to_vec()]
            }
            _ => vec![],
        };
    }
    subject
        .to_vec()
        .overlay_as::<i64>(&clip.to_vec(), rule, FillRule::NonZero)
}

/// Moves clip vertices that lie within `tol` of a subject vertex or edge
/// exactly onto it, so shared boundaries are seen as identical by the
/// clipper instead of producing hairline slivers.
fn snap_to_region(clip: &mut [Contour], region: &[Contour], tol: f64) {
    let tol2 = tol * tol;
    // Only points within the region's box (grown by the tolerance) can snap.
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in region.iter().flatten() {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    for c in clip.iter_mut() {
        for p in c.iter_mut() {
            if p[0] < b[0] - tol || p[0] > b[2] + tol || p[1] < b[1] - tol || p[1] > b[3] + tol {
                continue;
            }
            let mut best: Option<(f64, [f64; 2])> = None;
            for r in region {
                let n = r.len();
                for i in 0..n {
                    let a = r[i];
                    let b = r[(i + 1) % n];
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 > 0.0 {
                        ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2
                    } else {
                        0.0
                    };
                    let t = t.clamp(0.0, 1.0);
                    let q = [a[0] + dx * t, a[1] + dy * t];
                    let d2 = (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2);
                    // Prefer vertices over edge interiors when both are in range.
                    let dv = (p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2);
                    let (d2, q) = if dv <= tol2 {
                        (dv - tol2 * 2.0, a)
                    } else {
                        (d2, q)
                    };
                    if d2 <= tol2 && best.is_none_or(|(bd, _)| d2 < bd) {
                        best = Some((d2, q));
                    }
                }
            }
            if let Some((_, q)) = best {
                *p = q;
            }
        }
    }
}

/// What to keep of a face of `subject_solid` given the other solid.
#[derive(Clone, Copy)]
enum Keep {
    /// Keep the part outside the other solid, plus coplanar same-normal (A side of union).
    NotAbove,
    /// Keep the part outside, excluding all coplanar (B side of union).
    NotAboveNorBelow,
    /// Keep the part outside plus coplanar opposite-normal (A side of difference).
    NotBelow,
    /// Keep the part strictly inside (B side of difference / intersection).
    Inside,
    /// Keep the part inside plus coplanar same-normal (A side of intersection).
    Below,
}

fn classify_face(
    solid: &Solid,
    f: &Face,
    other: &Solid,
    other_boxes: &FaceIndex,
    other_bounds: (ok_math::Vec3, ok_math::Vec3),
    keep: Keep,
    tol: f64,
) -> Result<Vec<Vec<Contour>>, BrepError> {
    let region = face_region(solid, f);
    let fb = bounds_of(
        f.loops
            .iter()
            .flatten()
            .map(|&v| solid.vertices[v as usize]),
    )
    .unwrap();
    if !bounds_overlap(fb, other_bounds, tol) {
        return Ok(match keep {
            Keep::NotAbove | Keep::NotAboveNorBelow | Keep::NotBelow => vec![region],
            Keep::Inside | Keep::Below => vec![],
        });
    }
    let (above, below): (Section, Section) =
        sections_above_below(other, other_boxes, &f.plane, tol)?;
    if above.is_empty() && below.is_empty() {
        return Ok(match keep {
            Keep::NotAbove | Keep::NotAboveNorBelow | Keep::NotBelow => vec![region],
            Keep::Inside | Keep::Below => vec![],
        });
    }
    // Loops of the section that stay clear of this face cannot enclose or
    // cut it, so they are left out of the overlay (a large part sectioned
    // by one of its facets otherwise drags every hole into every overlay).
    let mut above = near_region(to_contours(&above.loops), &region, tol);
    let mut below = near_region(to_contours(&below.loops), &region, tol);
    snap_to_region(&mut above, &region, tol);
    snap_to_region(&mut below, &region, tol);
    Ok(match keep {
        Keep::NotAbove => overlay(&region, &above, OverlayRule::Difference),
        Keep::NotBelow => overlay(&region, &below, OverlayRule::Difference),
        Keep::Below => overlay(&region, &below, OverlayRule::Intersect),
        Keep::NotAboveNorBelow => {
            let mut both = above.clone();
            both.extend(below);
            overlay(&region, &both, OverlayRule::Difference)
        }
        Keep::Inside => {
            let mut out = Vec::new();
            for shape in overlay(&region, &above, OverlayRule::Intersect) {
                out.extend(overlay(&shape, &below, OverlayRule::Intersect));
            }
            out
        }
    })
}

/// Lifts the kept 2D fragments of `f` back to 3D. A fragment corner that
/// is one of the face's own vertices takes that vertex's exact position
/// rather than its projection onto the plane: a vertex may sit a hair off
/// the plane (within the planarity the solid allows), and the other faces
/// at that vertex keep it where it is, so lifting it onto the plane would
/// leave the fragments of neighbouring faces disagreeing by more than the
/// merge tolerance.
fn fragments_to_polygons(
    solid: &Solid,
    f: &Face,
    shapes: Vec<Vec<Contour>>,
    flip: bool,
    surface_offset: usize,
    tol: f64,
) -> Vec<Polygon> {
    let own: Vec<(Vec2, ok_math::Vec3)> = f
        .loops
        .iter()
        .flatten()
        .map(|&v| {
            let p = solid.vertices[v as usize];
            (f.plane.to_plane(p), p)
        })
        .collect();
    let lift = |p: &[f64; 2]| {
        let q = Vec2::new(p[0], p[1]);
        own.iter()
            .find(|(o, _)| o.distance(q) <= tol)
            .map(|(_, w)| *w)
            .unwrap_or_else(|| f.plane.to_world(q))
    };
    // The overlay drops collinear corners, so a fragment edge that runs
    // along the face's original boundary may have lost the vertices the
    // neighbouring faces still share there; put the face's own vertices
    // back on any edge they lie on.
    let restore = |l: Vec<ok_math::Vec3>| -> Vec<ok_math::Vec3> {
        let mut out: Vec<ok_math::Vec3> = Vec::with_capacity(l.len());
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            out.push(a);
            let d = b - a;
            let len2 = d.length_squared();
            if len2 == 0.0 {
                continue;
            }
            let mut on_edge: Vec<(f64, ok_math::Vec3)> = Vec::new();
            for &(_, w) in &own {
                let t = (w - a).dot(d) / len2;
                if t <= 0.0 || t >= 1.0 {
                    continue;
                }
                if (w - (a + d * t)).length() <= tol && w.distance(a) > tol && w.distance(b) > tol {
                    on_edge.push((t, w));
                }
            }
            on_edge.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
            out.extend(on_edge.into_iter().map(|(_, w)| w));
        }
        out
    };
    shapes
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|shape| {
            let mut loops: Vec<Vec<ok_math::Vec3>> = shape
                .iter()
                .map(|c| restore(c.iter().map(lift).collect()))
                .collect();
            let plane = if flip {
                for l in &mut loops {
                    l.reverse();
                }
                flip_plane(&f.plane)
            } else {
                f.plane
            };
            Polygon {
                plane,
                loops,
                surface: f.surface + surface_offset,
                origin: f.origin,
            }
        })
        .collect()
}

/// Combines two solids. The result keeps the surface tags and face origins
/// of both inputs.
/// Snaps the vertices of `s` that lie within `tol` of a vertex or a face
/// plane of `other` exactly onto it, so that nearly coincident geometry
/// becomes exactly coincident, which the classification handles exactly.
/// Faces whose vertices moved get their plane recomputed.
fn snap_to(s: &mut Solid, other: &Solid, tol: f64) {
    if s.is_empty() || other.is_empty() {
        return;
    }
    // Face bounding boxes of `other`, expanded by tol, to limit plane snaps
    // to vertices that are actually near the face rather than its plane.
    let face_boxes: Vec<(Vec3, Vec3)> = other
        .faces
        .iter()
        .map(|f| {
            let (lo, hi) = bounds_of(
                f.loops
                    .iter()
                    .flatten()
                    .map(|&v| other.vertices[v as usize]),
            )
            .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            (lo - Vec3::new(tol, tol, tol), hi + Vec3::new(tol, tol, tol))
        })
        .collect();
    let inside = |p: Vec3, b: &(Vec3, Vec3)| {
        p.x >= b.0.x && p.x <= b.1.x && p.y >= b.0.y && p.y <= b.1.y && p.z >= b.0.z && p.z <= b.1.z
    };
    let mut moved = vec![false; s.vertices.len()];
    for (vi, v) in s.vertices.iter_mut().enumerate() {
        // Vertex-to-vertex snap wins outright.
        if let Some(q) = other
            .vertices
            .iter()
            .find(|q| q.distance(*v) <= tol && q.distance(*v) > 0.0)
        {
            *v = *q;
            moved[vi] = true;
            continue;
        }
        // Then onto up to three nearby face planes, one after another, so a
        // vertex near a corner of `other` lands on the corner.
        let mut p = *v;
        let mut hits = 0;
        for (f, b) in other.faces.iter().zip(&face_boxes) {
            if !inside(p, b) {
                continue;
            }
            let d = f.plane.normal.dot(p - f.plane.origin);
            if d != 0.0 && d.abs() <= tol {
                p -= f.plane.normal * d;
                hits += 1;
                if hits == 3 {
                    break;
                }
            }
        }
        if hits > 0 {
            *v = p;
            moved[vi] = true;
        }
    }
    if !moved.iter().any(|m| *m) {
        return;
    }
    // A face of `s` that is nearly coplanar with a face of `other` may have
    // had only some of its vertices snapped (the plane snap is limited to
    // the other face's box); the rest would leave it tilted by a hair and
    // no longer planar. Put every vertex of such a face onto that plane.
    let planes: Vec<ok_math::Plane> = other.faces.iter().map(|f| f.plane).collect();
    for fi in 0..s.faces.len() {
        let verts: Vec<u32> = s.faces[fi].loops.iter().flatten().copied().collect();
        if !verts.iter().any(|&v| moved[v as usize]) {
            continue;
        }
        let on = |plane: &ok_math::Plane, p: Vec3| plane.normal.dot(p - plane.origin).abs() <= tol;
        let Some(plane) = planes.iter().find(|pl| {
            pl.normal.dot(s.faces[fi].plane.normal).abs() > 0.999
                && verts.iter().all(|&v| on(pl, s.vertices[v as usize]))
        }) else {
            continue;
        };
        for &v in &verts {
            let p = s.vertices[v as usize];
            let d = plane.normal.dot(p - plane.origin);
            if d != 0.0 {
                s.vertices[v as usize] = p - plane.normal * d;
                moved[v as usize] = true;
            }
        }
    }
    for f in &mut s.faces {
        if !f.loops[0].iter().any(|&v| moved[v as usize]) {
            continue;
        }
        let pts: Vec<Vec3> = f.loops[0].iter().map(|&v| s.vertices[v as usize]).collect();
        let Some(n) = crate::revolve::newell_normal(&pts).normalized() else {
            continue;
        };
        let x = f.plane.x_axis - n * f.plane.x_axis.dot(n);
        let Some(x) = x.normalized() else { continue };
        f.plane = ok_math::Plane {
            origin: pts[0],
            x_axis: x,
            y_axis: n.cross(x),
            normal: n,
        };
    }
}

pub fn boolean(a: &Solid, b: &Solid, op: BoolOp) -> Result<Solid, BrepError> {
    if a.is_empty() || b.is_empty() {
        return Ok(match op {
            BoolOp::Union => a.merged(b),
            BoolOp::Difference => a.clone(),
            BoolOp::Intersection => Solid::default(),
        });
    }
    let (ab, bb) = (a.bounds().unwrap(), b.bounds().unwrap());
    let diag = ((ab.1 - ab.0).length()).max((bb.1 - bb.0).length());
    let tol = merge_tolerance(diag) * 10.0;
    if !bounds_overlap(ab, bb, tol) {
        return Ok(match op {
            BoolOp::Union => a.merged(b),
            BoolOp::Difference => a.clone(),
            BoolOp::Intersection => Solid::default(),
        });
    }

    // Nearly coincident geometry is made exactly coincident first (within
    // the tolerance the classification already treats as "on").
    let (mut a_snapped, mut b_snapped) = (a.clone(), b.clone());
    snap_to(&mut b_snapped, a, tol);
    snap_to(&mut a_snapped, &b_snapped, tol);
    let (a, b) = (&a_snapped, &b_snapped);

    let (keep_a, keep_b, flip_b) = match op {
        BoolOp::Union => (Keep::NotAbove, Keep::NotAboveNorBelow, false),
        BoolOp::Difference => (Keep::NotBelow, Keep::Inside, true),
        BoolOp::Intersection => (Keep::Below, Keep::Inside, false),
    };

    let (boxes_a, boxes_b) = (FaceIndex::new(a), FaceIndex::new(b));
    let mut polys: Vec<Polygon> = Vec::new();
    for f in &a.faces {
        let shapes = classify_face(a, f, b, &boxes_b, bb, keep_a, tol)?;
        polys.extend(fragments_to_polygons(a, f, shapes, false, 0, tol));
    }
    for f in &b.faces {
        let shapes = classify_face(b, f, a, &boxes_a, ab, keep_b, tol)?;
        polys.extend(fragments_to_polygons(
            b,
            f,
            shapes,
            flip_b,
            a.surfaces.len(),
            tol,
        ));
    }
    let mut surfaces = a.surfaces.clone();
    surfaces.extend_from_slice(&b.surfaces);
    // Fragments outside the overlap of the two boxes are whole faces of a
    // valid input, so only edges reaching into it can have T-junctions.
    let overlap = (
        Vec3::new(ab.0.x.max(bb.0.x), ab.0.y.max(bb.0.y), ab.0.z.max(bb.0.z)),
        Vec3::new(ab.1.x.min(bb.1.x), ab.1.y.min(bb.1.y), ab.1.z.min(bb.1.z)),
    );
    let mut solid = Solid::from_polygons_within(polys, surfaces, tol, overlap)?;
    solid.merge_coplanar_faces();
    solid.compact_surfaces();
    solid.validate()?;
    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude;
    use ok_math::{Plane, Vec2};
    use ok_sketch::{ProfileOptions, Sketch};
    use std::f64::consts::PI;

    fn rect(plane: &Plane, a: Vec2, b: Vec2, start: f64, end: f64, id: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(a, b);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, plane, start, end, id).unwrap()
    }

    /// Replays a case the fuzz dumped (`OK_FUZZ_DUMP`):
    /// `OK_BREP_CASE=<dir>/seed-59-step-3 OK_BREP_OP=union cargo test -p ok-brep --lib replay_dumped_case -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn replay_dumped_case() {
        let Ok(stem) = std::env::var("OK_BREP_CASE") else {
            return;
        };
        let load = |what: &str| -> Solid {
            serde_json::from_str(&std::fs::read_to_string(format!("{stem}-{what}.json")).unwrap())
                .unwrap()
        };
        let (a, b) = (load("body"), load("tool"));
        let op = match std::env::var("OK_BREP_OP").as_deref() {
            Ok("difference") => BoolOp::Difference,
            Ok("intersection") => BoolOp::Intersection,
            _ => BoolOp::Union,
        };
        eprintln!("a: {} faces, valid {:?}", a.faces.len(), a.validate());
        eprintln!("b: {} faces, valid {:?}", b.faces.len(), b.validate());
        let r = boolean(&a, &b, op).unwrap();
        eprintln!("result: {} faces, volume {}", r.faces.len(), r.volume());
        r.validate().unwrap();
    }

    /// A box whose bottom sits a hair above another box's bottom and whose
    /// side pokes out of it leaves a sliver strip of the first box's side
    /// face; every boolean must still close, whatever the gap size.
    #[test]
    fn slivers_of_every_size_stay_closed() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 0.0, 3.0, 1);
        for gap in [1e-2, 1e-3, 1e-4, 3e-5, 1.5e-5, 1e-5, 5e-6, 1e-6, 1e-7] {
            let b = rect(
                &Plane::XY,
                Vec2::new(2.0, 1.0),
                Vec2::new(4.0, 2.0),
                gap,
                1.0,
                2,
            );
            for (op, expect) in [
                (BoolOp::Union, 27.0 + (1.0 - gap)),
                (BoolOp::Difference, 27.0 - (1.0 - gap)),
                (BoolOp::Intersection, 1.0 - gap),
            ] {
                let r = boolean(&a, &b, op).unwrap_or_else(|e| panic!("gap {gap} {op:?}: {e}"));
                r.validate()
                    .unwrap_or_else(|e| panic!("gap {gap} {op:?}: {e}"));
                let v = r.volume();
                assert!(
                    (v - expect).abs() < 1e-3,
                    "gap {gap} {op:?}: volume {v} vs {expect}"
                );
            }
        }
    }

    fn cylinder(plane: &Plane, c: Vec2, r: f64, start: f64, end: f64, id: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_circle(c, r);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, plane, start, end, id).unwrap()
    }

    fn assert_vol(s: &Solid, expected: f64, rel: f64) {
        s.validate().unwrap();
        let v = s.volume();
        assert!(
            ((v - expected) / expected).abs() < rel,
            "volume {v}, expected {expected}"
        );
    }

    #[test]
    fn union_of_overlapping_boxes() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(6.0, 6.0),
            2.0,
            6.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 64.0 + 64.0 - 8.0, 1e-9);
        assert_eq!(
            u.vertices.len(),
            8 + 8 + 6 - 2,
            "six new corners, two buried corners gone"
        );
    }

    #[test]
    fn difference_and_intersection_of_overlapping_boxes() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(6.0, 6.0),
            2.0,
            6.0,
            2,
        );
        assert_vol(
            &boolean(&a, &b, BoolOp::Difference).unwrap(),
            64.0 - 8.0,
            1e-9,
        );
        assert_vol(&boolean(&a, &b, BoolOp::Intersection).unwrap(), 8.0, 1e-9);
    }

    #[test]
    fn union_of_stacked_boxes_removes_touching_faces() {
        // Boss sitting exactly on a plate: coplanar opposite-normal faces.
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 2.0, 1);
        let boss = rect(
            &Plane::XY,
            Vec2::new(3.0, 3.0),
            Vec2::new(7.0, 7.0),
            2.0,
            5.0,
            2,
        );
        let u = boolean(&plate, &boss, BoolOp::Union).unwrap();
        assert_vol(&u, 200.0 + 48.0, 1e-9);
        // Plate top became a face with a hole; boss bottom vanished: 6 + 5 faces.
        assert_eq!(u.faces.len(), 11);
    }

    #[test]
    fn union_of_flush_boxes_merges_side_by_side() {
        // Same-normal coplanar faces (tops flush) and a shared internal wall.
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(4.0, 0.0),
            Vec2::new(8.0, 4.0),
            0.0,
            4.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 128.0, 1e-9);
        assert_eq!(u.faces.len(), 6, "internal wall gone; flush faces merged");
        assert_eq!(
            u.vertices.len(),
            12,
            "merged faces keep the mid-edge vertices"
        );
    }

    #[test]
    fn through_hole_cut() {
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(20.0, 20.0), 0.0, 5.0, 1);
        let drill = cylinder(&Plane::XY, Vec2::new(10.0, 10.0), 3.0, -1.0, 6.0, 2);
        let cut = boolean(&plate, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 2000.0 - PI * 9.0 * 5.0, 2e-3);
        // Top and bottom each have one hole loop now.
        let holed = cut.faces.iter().filter(|f| f.loops.len() == 2).count();
        assert_eq!(holed, 2);
    }

    #[test]
    fn flush_hole_cut_from_top_face() {
        // Drill starts exactly on the top face (coplanar, same normal).
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(20.0, 20.0), 0.0, 5.0, 1);
        let drill = cylinder(&Plane::XY, Vec2::new(10.0, 10.0), 3.0, 5.0, 2.0, 2);
        let cut = boolean(&plate, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 2000.0 - PI * 9.0 * 3.0, 2e-3);
        assert_eq!(
            cut.faces.iter().filter(|f| f.loops.len() == 2).count(),
            1,
            "only the top has a hole"
        );
    }

    #[test]
    fn cut_that_splits_a_body_in_two() {
        let bar = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 2.0), 0.0, 2.0, 1);
        let saw = rect(
            &Plane::XY,
            Vec2::new(4.0, -1.0),
            Vec2::new(6.0, 3.0),
            -1.0,
            3.0,
            2,
        );
        let cut = boolean(&bar, &saw, BoolOp::Difference).unwrap();
        assert_vol(&cut, 32.0, 1e-9);
        assert_eq!(cut.shells().len(), 2);
    }

    #[test]
    fn boss_with_hole_on_plate_then_drill_through() {
        // The demo part: plate with a hole, boss with the same hole on top,
        // union, then a side cut through the boss.
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(60.0, 40.0));
        s.add_circle(Vec2::new(30.0, 20.0), 6.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let plate = extrude(&p[0], &Plane::XY, 0.0, 8.0, 1).unwrap();

        let mut s2 = Sketch::new();
        s2.add_circle(Vec2::new(30.0, 20.0), 10.0);
        s2.add_circle(Vec2::new(30.0, 20.0), 6.0);
        let mut p2 = s2.profiles(&ProfileOptions::default());
        p2.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let boss = extrude(&p2[0], &Plane::XY.offset(8.0), 0.0, 6.0, 2).unwrap();

        let u = boolean(&plate, &boss, BoolOp::Union).unwrap();
        let expected = 60.0 * 40.0 * 8.0 - PI * 36.0 * 8.0 + PI * (100.0 - 36.0) * 6.0;
        assert_vol(&u, expected, 2e-3);

        let slot = rect(
            &Plane::XZ,
            Vec2::new(25.0, 10.0),
            Vec2::new(35.0, 20.0),
            -50.0,
            50.0,
            3,
        );
        let cut = boolean(&u, &slot, BoolOp::Difference).unwrap();
        assert!(cut.volume() < u.volume());
        cut.validate().unwrap();
    }

    #[test]
    fn cylinder_union_cylinder_crosswise() {
        let a = cylinder(&Plane::XY, Vec2::ZERO, 2.0, -5.0, 5.0, 1);
        let b = cylinder(&Plane::XZ, Vec2::ZERO, 2.0, -5.0, 5.0, 2);
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        // Steinmetz solid volume for the intersection: 16 r^3 / 3.
        let expected = 2.0 * PI * 4.0 * 10.0 - 16.0 * 8.0 / 3.0;
        assert_vol(&u, expected, 5e-3);
        assert_vol(
            &boolean(&a, &b, BoolOp::Intersection).unwrap(),
            16.0 * 8.0 / 3.0,
            5e-3,
        );
    }

    #[test]
    fn disjoint_union_keeps_both_lumps() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(1.0, 1.0), 0.0, 1.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(5.0, 5.0),
            Vec2::new(6.0, 6.0),
            0.0,
            1.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 2.0, 1e-9);
        assert!(boolean(&a, &b, BoolOp::Intersection).unwrap().is_empty());
    }

    #[test]
    fn identical_solids() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 2.0), 0.0, 1.0, 1);
        let b = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 2.0), 0.0, 1.0, 2);
        assert_vol(&boolean(&a, &b, BoolOp::Union).unwrap(), 6.0, 1e-9);
        assert_vol(&boolean(&a, &b, BoolOp::Intersection).unwrap(), 6.0, 1e-9);
        assert!(boolean(&a, &b, BoolOp::Difference).unwrap().is_empty());
    }

    #[test]
    fn contained_solids() {
        let big = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 10.0, 1);
        let small = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(4.0, 4.0),
            2.0,
            4.0,
            2,
        );
        assert_vol(&boolean(&big, &small, BoolOp::Union).unwrap(), 1000.0, 1e-9);
        assert_vol(
            &boolean(&small, &big, BoolOp::Intersection).unwrap(),
            8.0,
            1e-9,
        );
        assert!(boolean(&small, &big, BoolOp::Difference)
            .unwrap()
            .is_empty());
        // A fully buried void: outer shell plus inverted inner shell.
        let hollow = boolean(&big, &small, BoolOp::Difference).unwrap();
        assert_vol(&hollow, 992.0, 1e-9);
        assert_eq!(hollow.shells().len(), 2);
    }

    #[test]
    fn stacked_boxes_with_identical_footprint() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 0.0, 1.0, 1);
        let b = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 1.0, 2.0, 2);
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 18.0, 1e-9);
        assert_eq!(u.faces.len(), 6, "touching caps vanish, walls merge");
    }

    #[test]
    fn blind_pocket_cut_from_top() {
        let block = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 5.0, 1);
        let pocket = rect(
            &Plane::XY.offset(5.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(8.0, 8.0),
            0.0,
            -3.0,
            2,
        );
        let cut = boolean(&block, &pocket, BoolOp::Difference).unwrap();
        assert_vol(&cut, 500.0 - 108.0, 1e-9);
        assert_eq!(cut.faces.len(), 6 + 5);
    }

    #[test]
    fn edge_to_edge_touching_boxes_do_not_merge_volume() {
        // Boxes sharing only an edge: union is a valid two-lump solid.
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(2.0, 2.0), 0.0, 2.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(4.0, 4.0),
            0.0,
            2.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 16.0, 1e-9);
    }

    #[test]
    fn merged_faces_survive_further_booleans() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(4.0, 0.0),
            Vec2::new(8.0, 4.0),
            0.0,
            4.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        let drill = cylinder(&Plane::XY, Vec2::new(4.0, 2.0), 1.0, -1.0, 5.0, 3);
        let cut = boolean(&u, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 128.0 - PI * 4.0, 2e-3);
        assert_eq!(
            cut.faces.iter().filter(|f| f.loops.len() == 2).count(),
            2,
            "one hole in the merged top and bottom"
        );
    }

    #[test]
    fn repeated_operations_stay_valid() {
        let mut body = rect(&Plane::XY, Vec2::ZERO, Vec2::new(30.0, 30.0), 0.0, 10.0, 1);
        for i in 0..4 {
            let x = 6.0 + 6.0 * i as f64;
            let drill = cylinder(&Plane::XY, Vec2::new(x, 15.0), 2.0, -1.0, 11.0, 10 + i);
            body = boolean(&body, &drill, BoolOp::Difference).unwrap();
        }
        assert_vol(&body, 9000.0 - 4.0 * PI * 4.0 * 10.0, 2e-3);
    }
}
