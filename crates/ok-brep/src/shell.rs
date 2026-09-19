//! Shell: hollows a solid to a uniform wall thickness, optionally leaving
//! chosen faces open.
//!
//! The cavity is an offset polyhedron: every kept face's plane moves
//! inward by the thickness (open faces move outward so the cavity breaks
//! through them), each vertex is re-solved from its incident planes, and
//! the faces keep their loops. One boolean then subtracts the cavity from
//! the solid. This is what an offset-surface shell does on exact geometry,
//! restricted to the planar facets we have; corners where four or more
//! faces meet with no common offset point, and walls thicker than the
//! feature they hollow, are reported as errors rather than guessed.

use crate::{BrepError, Face, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec3};

/// Hollows `solid` leaving walls `thickness` thick. `open` lists faces (by
/// index) to remove so the cavity is reachable; empty makes a closed void.
pub fn shell(
    solid: &Solid,
    thickness: f64,
    open: &[usize],
    feature: u32,
) -> Result<Solid, BrepError> {
    if thickness <= 0.0 || !thickness.is_finite() {
        return Err(BrepError::Degenerate(
            "shell thickness must be positive".into(),
        ));
    }
    if solid.is_empty() {
        return Err(BrepError::Degenerate("nothing to shell".into()));
    }
    if let Some(&bad) = open.iter().find(|&&i| i >= solid.faces.len()) {
        return Err(BrepError::Degenerate(format!(
            "open face {bad} does not exist"
        )));
    }
    if open.len() >= solid.faces.len() {
        return Err(BrepError::Degenerate(
            "every face is open; nothing would remain".into(),
        ));
    }
    let cavity = offset_polyhedron(solid, thickness, open, feature).map_err(|e| match e {
        BrepError::Degenerate(msg) if msg.contains("too large") || msg.contains("no cavity") => {
            BrepError::Degenerate(msg)
        }
        other => BrepError::Degenerate(format!(
            "the shell could not close the cavity: the wall thickness changes the shape near a corner where a face is swallowed (try a thinner wall or coarser facets): {other}"
        )),
    })?;
    let out = crate::boolean(solid, &cavity, crate::BoolOp::Difference)
        .map_err(|e| BrepError::Degenerate(format!("shell failed: {e}")))?;
    if out.is_empty() {
        return Err(BrepError::Degenerate(
            "the shell removed the whole body".into(),
        ));
    }
    Ok(out)
}

/// The solid with every face plane moved inward by `t` (outward for open
/// faces): the offset planes and surfaces, reshaped.
fn offset_polyhedron(
    solid: &Solid,
    t: f64,
    open: &[usize],
    feature: u32,
) -> Result<Solid, BrepError> {
    let dist = |i: usize| if open.contains(&i) { -t } else { t };
    let planes: Vec<Option<Plane>> = solid
        .faces
        .iter()
        .enumerate()
        .map(|(i, f)| {
            Some(Plane {
                origin: f.plane.origin - f.plane.normal * dist(i),
                ..f.plane
            })
        })
        .collect();
    let surfaces: Vec<Surface> = solid
        .surfaces
        .iter()
        .enumerate()
        .map(
            |(si, s)| match solid.faces.iter().find(|f| f.surface == si) {
                Some(f) => offset_surface(*s, f, solid, t),
                None => *s,
            },
        )
        .collect();
    reshape(
        solid,
        &planes,
        surfaces,
        4.0 * t,
        "the wall thickness",
        Some(feature),
    )
}

/// Moves the given planar faces along their normals by `distance`
/// (negative moves them into the body), re-solving the corners of every
/// face they touch. Faces on curved surfaces are refused.
pub fn move_faces(solid: &Solid, faces: &[usize], distance: f64) -> Result<Solid, BrepError> {
    if !distance.is_finite() || distance.abs() <= 1e-12 {
        return Err(BrepError::Degenerate(
            "move distance must be non-zero".into(),
        ));
    }
    let mut planes: Vec<Option<Plane>> = vec![None; solid.faces.len()];
    let mut surfaces = solid.surfaces.clone();
    for &i in faces {
        let f = solid
            .faces
            .get(i)
            .ok_or_else(|| BrepError::Degenerate(format!("face {i} does not exist")))?;
        let Surface::Plane { normal, offset } = solid.surfaces[f.surface] else {
            return Err(BrepError::Degenerate(
                "only planar faces can be moved".into(),
            ));
        };
        planes[i] = Some(Plane {
            origin: f.plane.origin + f.plane.normal * distance,
            ..f.plane
        });
        surfaces[f.surface] = Surface::Plane {
            normal,
            offset: offset + distance,
        };
    }
    reshape(
        solid,
        &planes,
        surfaces,
        4.0 * distance.abs(),
        "the move",
        None,
    )
}

/// Tilts the given planar faces by `angle_deg` about the line where each
/// meets the neutral plane, so the body tapers towards the neutral
/// plane's normal (the pull direction); a negative angle tapers the
/// other way. Faces parallel to the neutral plane or on curved surfaces
/// are refused.
pub fn draft_faces(
    solid: &Solid,
    faces: &[usize],
    neutral: &Plane,
    angle_deg: f64,
) -> Result<Solid, BrepError> {
    if !angle_deg.is_finite() || angle_deg.abs() <= 1e-9 || angle_deg.abs() >= 89.0 {
        return Err(BrepError::Degenerate(
            "draft angle must be between -89 and 89 degrees and non-zero".into(),
        ));
    }
    let pull = neutral
        .normal
        .normalized()
        .ok_or_else(|| BrepError::Degenerate("neutral plane has no normal".into()))?;
    let theta = angle_deg.to_radians();
    let mut planes: Vec<Option<Plane>> = vec![None; solid.faces.len()];
    let mut surfaces = solid.surfaces.clone();
    let mut moved = 0.0f64;
    for &i in faces {
        let f = solid
            .faces
            .get(i)
            .ok_or_else(|| BrepError::Degenerate(format!("face {i} does not exist")))?;
        if !matches!(solid.surfaces[f.surface], Surface::Plane { .. }) {
            return Err(BrepError::Degenerate(
                "only planar faces can be drafted".into(),
            ));
        }
        let n = f.plane.normal;
        // Hinge: the line where the face plane meets the neutral plane.
        let axis = n.cross(pull);
        let Some(axis_dir) = axis.normalized() else {
            return Err(BrepError::Degenerate(format!(
                "face {} of feature {} is parallel to the neutral plane",
                f.origin.local, f.origin.feature
            )));
        };
        // A point on both planes nearest the face's first vertex.
        let p0 = solid.vertices[f.loops[0][0] as usize];
        let hinge = solve_point(
            &[(n, n.dot(f.plane.origin)), (pull, pull.dot(neutral.origin))],
            p0,
        );
        let side = axis_dir.cross(n); // in-plane, perpendicular to the hinge
        let new_normal = (n * theta.cos() + side * theta.sin())
            .normalized()
            .unwrap_or(n);
        let plane = Plane::from_origin_normal(hinge, new_normal)
            .ok_or_else(|| BrepError::Degenerate("degenerate draft plane".into()))?;
        for l in &f.loops {
            for &v in l {
                let p = solid.vertices[v as usize];
                moved = moved.max((new_normal.dot(p - hinge)).abs());
            }
        }
        planes[i] = Some(plane);
        surfaces[f.surface] = Surface::Plane {
            normal: new_normal,
            offset: new_normal.dot(hinge),
        };
    }
    reshape(
        solid,
        &planes,
        surfaces,
        4.0 * moved.max(1e-6),
        "the draft",
        None,
    )
}

/// The solid with some face planes replaced (`planes[i]` is the new plane
/// of face `i`; `None` keeps it). Each face keeps its outline, edge by
/// edge: every edge moves to the line where the face's new plane meets
/// the new plane of the face across that edge, and corners are where
/// consecutive edge lines meet. An edge whose new position would run
/// backwards has been swallowed by its neighbours (a short facet next to
/// a sharp corner) and is dropped, its neighbours meeting directly; a
/// loop left with fewer than three edges vanishes. Faces built this way
/// agree exactly along shared edges at corners where three faces meet,
/// and disagree by a little where more meet or where edges were dropped;
/// the assembly closes rings of open edges up to `gap` across. `what`
/// names the change in error messages. Faces keep their origins unless
/// `new_feature` is given (a shell's cavity is new geometry of that feature).
fn reshape(
    solid: &Solid,
    planes: &[Option<Plane>],
    surfaces: Vec<Surface>,
    gap: f64,
    what: &str,
    new_feature: Option<u32>,
) -> Result<Solid, BrepError> {
    let (min, max) = solid.bounds().unwrap();
    let scale = (max - min).length().max(1.0);
    let new_plane = |i: usize| planes[i].unwrap_or(solid.faces[i].plane);
    let plane_of = |i: usize| -> (Vec3, f64) {
        let p = new_plane(i);
        (p.normal, p.normal.dot(p.origin))
    };
    let edge_faces = solid.edge_faces();
    let across = |a: u32, b: u32, me: usize| -> Option<usize> {
        edge_faces
            .get(&crate::edge_key(a, b))
            .and_then(|fs| fs.iter().copied().find(|&f| f != me))
    };

    let mut polys = Vec::with_capacity(solid.faces.len());
    for (i, f) in solid.faces.iter().enumerate() {
        let n = new_plane(i).normal;
        let mut loops = Vec::with_capacity(f.loops.len());
        for (li, l) in f.loops.iter().enumerate() {
            // One entry per edge: the face across it, a point near it and
            // its direction. Where other faces touch a corner only at the
            // vertex (the ring of faces around it holds more than the two
            // across this loop's edges), their offset planes bound this
            // face too, so they are inserted as extra edges between the
            // two real ones, in ring order.
            let mut edges: Vec<(Option<usize>, Vec3, Vec3)> = Vec::with_capacity(l.len());
            for k in 0..l.len() {
                let (a, b) = (l[k], l[(k + 1) % l.len()]);
                let (pa, pb) = (solid.vertices[a as usize], solid.vertices[b as usize]);
                let prev_across = across(l[(k + l.len() - 1) % l.len()], a, i);
                let this_across = across(a, b, i);
                if let (Some(p), Some(q)) = (prev_across, this_across) {
                    if p != q {
                        if let Some(mut ring) = fan_order(solid, a, &edge_faces) {
                            // Rotate to start at this face: ring = [F, Q, X.., P].
                            if let Some(at) = ring.iter().position(|&r| r == i) {
                                ring.rotate_left(at);
                            }
                            let m = ring.len();
                            if m > 3 && ring[0] == i && ring[1] == q && ring[m - 1] == p {
                                let prev_dir =
                                    pa - solid.vertices[l[(k + l.len() - 1) % l.len()] as usize];
                                let hint = prev_dir + (pb - pa);
                                for &r in ring[2..m - 1].iter().rev() {
                                    let mut d = n.cross(solid.faces[r].plane.normal);
                                    if d.dot(hint) < 0.0 {
                                        d = -d;
                                    }
                                    edges.push((Some(r), pa, d));
                                }
                            }
                        }
                    }
                }
                edges.push((this_across, pa, pb - pa));
            }
            let corner = |edges: &[(Option<usize>, Vec3, Vec3)], k: usize| -> Vec3 {
                let prev = &edges[(k + edges.len() - 1) % edges.len()];
                let this = &edges[k];
                let mut planes = vec![plane_of(i)];
                if let Some(p) = prev.0 {
                    planes.push(plane_of(p));
                }
                if let Some(q) = this.0 {
                    if Some(q) != prev.0 {
                        planes.push(plane_of(q));
                    }
                }
                solve_point(&planes, this.1)
            };
            // A point on an edge's offset line, nearest its original start.
            let line_point = |e: &(Option<usize>, Vec3, Vec3)| -> Vec3 {
                let mut planes = vec![plane_of(i)];
                if let Some(g) = e.0 {
                    planes.push(plane_of(g));
                }
                solve_point(&planes, e.1)
            };
            let original: Vec<Vec3> = l.iter().map(|&v| solid.vertices[v as usize]).collect();
            let line_distance = |e: &(Option<usize>, Vec3, Vec3)| -> f64 {
                let (p, d) = (line_point(e), e.2.normalized().unwrap_or(Vec3::ZERO));
                original
                    .iter()
                    .map(|&q| ((q - p) - d * (q - p).dot(d)).length())
                    .fold(f64::INFINITY, f64::min)
            };
            let mut pts: Vec<Vec3> = (0..edges.len()).map(|k| corner(&edges, k)).collect();
            // Drop edges until every edge keeps its direction. Consecutive
            // edges on parallel lines cannot meet at a corner: with the same
            // direction the less restrictive one (further from the face
            // interior) is dropped; with opposite directions the one whose
            // line lies further from the original outline is a constraint
            // from a face that does not reach this far, and is dropped.
            loop {
                let m = edges.len();
                if m < 3 {
                    break;
                }
                let parallel = (0..m).find(|&k| {
                    let (dp, dq) = (edges[(k + m - 1) % m].2, edges[k].2);
                    dp.cross(dq).length() <= 1e-9 * dp.length() * dq.length()
                });
                if let Some(k) = parallel {
                    let (kp, kq) = ((k + m - 1) % m, k);
                    let (p, q) = (edges[kp], edges[kq]);
                    let drop = if p.2.dot(q.2) > 0.0 {
                        let left = n.cross(q.2);
                        if (line_point(&q) - line_point(&p)).dot(left) > 0.0 {
                            kp
                        } else {
                            kq
                        }
                    } else if line_distance(&p) > line_distance(&q) {
                        kp
                    } else {
                        kq
                    };
                    edges.remove(drop);
                    pts = (0..edges.len()).map(|k| corner(&edges, k)).collect();
                    continue;
                }
                let reversed = (0..m).find(|&k| {
                    let d = pts[(k + 1) % m] - pts[k];
                    d.dot(edges[k].2) <= 0.0
                });
                let Some(k) = reversed else { break };
                edges.remove(k);
                pts = (0..edges.len()).map(|k| corner(&edges, k)).collect();
            }
            if edges.len() < 3 {
                if li == 0 {
                    loops.clear();
                    break; // the whole face vanished
                }
                continue; // a hole closed up
            }
            loops.push(pts);
        }
        if loops.is_empty() {
            continue;
        }
        // A loop that still turned inside out means the wall is thicker
        // than the feature (its parts collided from afar).
        let area = crate::revolve::newell_normal(&loops[0]);
        if area.dot(n) <= 0.0 || area.length() > 4.0 * face_area(solid, f) {
            return Err(BrepError::Degenerate(format!(
                "{what} is too large for the feature at face {} of feature {}",
                f.origin.local, f.origin.feature
            )));
        }
        polys.push(Polygon {
            plane: new_plane(i),
            loops,
            surface: f.surface,
            origin: match new_feature {
                Some(feature) => FaceOrigin {
                    feature,
                    local: f.origin.local,
                },
                None => f.origin,
            },
        });
    }
    if polys.len() < 4 {
        return Err(BrepError::Degenerate(format!("{what} leaves no cavity")));
    }
    let tol = crate::merge_tolerance(scale);
    Solid::from_polygons_closing_gaps(polys, surfaces, tol, gap.max(tol * 10.0))
}

/// Twice the outer-loop area of a face (the length of its Newell normal).
fn face_area(solid: &Solid, f: &Face) -> f64 {
    let pts: Vec<Vec3> = f.loops[0]
        .iter()
        .map(|&v| solid.vertices[v as usize])
        .collect();
    crate::revolve::newell_normal(&pts).length()
}

/// Faces around vertex `v` in cyclic order: each face's outgoing edge at
/// `v` leads to the face across it. `None` when the faces around `v` do
/// not form one simple ring.
fn fan_order(
    solid: &Solid,
    v: u32,
    edge_faces: &std::collections::HashMap<crate::EdgeKey, Vec<usize>>,
) -> Option<Vec<usize>> {
    let mut next_of: Vec<(usize, u32)> = Vec::new();
    for (i, f) in solid.faces.iter().enumerate() {
        for l in &f.loops {
            if let Some(k) = l.iter().position(|&x| x == v) {
                next_of.push((i, l[(k + 1) % l.len()]));
            }
        }
    }
    let start = next_of.first()?.0;
    let mut ring = vec![start];
    let mut cur = start;
    for _ in 0..next_of.len() {
        let (_, nv) = *next_of.iter().find(|(f, _)| *f == cur)?;
        let across = edge_faces
            .get(&crate::edge_key(v, nv))?
            .iter()
            .copied()
            .find(|&f| f != cur)?;
        if across == start {
            return (ring.len() == next_of.len()).then_some(ring);
        }
        if ring.contains(&across) {
            return None;
        }
        ring.push(across);
        cur = across;
    }
    None
}

/// Least-squares point on the given planes (`n · x = c`), regularised
/// towards `near` so an under-determined set (one or two planes) yields
/// the closest point to `near` on them.
fn solve_point(planes: &[(Vec3, f64)], near: Vec3) -> Vec3 {
    let eps = 1e-9;
    let mut a = [[eps, 0.0, 0.0], [0.0, eps, 0.0], [0.0, 0.0, eps]];
    let mut b = [eps * near.x, eps * near.y, eps * near.z];
    for (n, c) in planes {
        let nv = [n.x, n.y, n.z];
        for r in 0..3 {
            for col in 0..3 {
                a[r][col] += nv[r] * nv[col];
            }
            b[r] += nv[r] * c;
        }
    }
    match solve3(a, b) {
        Some(x) => Vec3::new(x[0], x[1], x[2]),
        None => near,
    }
}

/// Solves the 3x3 system `a x = b` by Gaussian elimination with pivoting.
fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot =
            (col..3).max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap())?;
        if a[pivot][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for r in 0..3 {
            if r == col {
                continue;
            }
            let k = a[r][col] / a[col][col];
            let pivot_row = a[col];
            for (c, value) in a[r].iter_mut().enumerate().skip(col) {
                *value -= k * pivot_row[c];
            }
            b[r] -= k * b[col];
        }
    }
    Some([b[0] / a[0][0], b[1] / a[1][1], b[2] / a[2][2]])
}

/// The face's surface moved inward by `t`.
fn offset_surface(s: Surface, f: &Face, solid: &Solid, t: f64) -> Surface {
    match s {
        Surface::Plane { normal, offset } => Surface::Plane {
            normal,
            offset: offset - t,
        },
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => {
            // A face whose normal points away from the axis is the outside
            // of a cylinder (offset shrinks it); otherwise it is a hole.
            let p = solid.vertices[f.loops[0][0] as usize];
            let radial = (p - origin) - axis * (p - origin).dot(axis);
            let outward = radial.dot(f.plane.normal) > 0.0;
            Surface::Cylinder {
                origin,
                axis,
                radius: if outward { radius - t } else { radius + t },
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn block(w: f64, d: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    fn face_with_normal(s: &Solid, n: Vec3) -> usize {
        s.faces
            .iter()
            .position(|f| f.plane.normal.approx_eq(n))
            .unwrap()
    }

    #[test]
    fn closed_shell_of_a_block_is_a_hollow_box() {
        let b = block(20.0, 20.0, 20.0);
        let s = shell(&b, 2.0, &[], 9).unwrap();
        s.validate().unwrap();
        let expected = 20f64.powi(3) - 16f64.powi(3);
        assert!((s.volume() - expected).abs() < 1e-6, "{}", s.volume());
        // Two shells: the outside and the void.
        assert_eq!(s.shells().len(), 2);
    }

    #[test]
    fn open_top_shell_is_a_tray() {
        let b = block(30.0, 20.0, 10.0);
        let top = face_with_normal(&b, Vec3::Z);
        let s = shell(&b, 2.0, &[top], 9).unwrap();
        let expected = 30.0 * 20.0 * 10.0 - 26.0 * 16.0 * 8.0;
        assert!((s.volume() - expected).abs() < 1e-6, "{}", s.volume());
        assert_eq!(s.shells().len(), 1);
        // The tray's floor is a face at z = 2 with normal +Z.
        assert!(s.faces.iter().any(|f| {
            f.plane.normal.approx_eq(Vec3::Z) && (f.plane.origin.z - 2.0).abs() < 1e-9
        }));
    }

    #[test]
    fn two_open_faces_make_a_tube() {
        let b = block(10.0, 10.0, 40.0);
        let top = face_with_normal(&b, Vec3::Z);
        let bottom = face_with_normal(&b, -Vec3::Z);
        let s = shell(&b, 1.0, &[top, bottom], 9).unwrap();
        let expected = 40.0 * (100.0 - 64.0);
        assert!((s.volume() - expected).abs() < 1e-6, "{}", s.volume());
    }

    #[test]
    fn shelled_cylinder_keeps_one_inner_cylinder_surface() {
        let mut sk = Sketch::new();
        sk.add_circle(Vec2::ZERO, 10.0);
        let p = sk.profiles(&ProfileOptions::default()).remove(0);
        let cyl = extrude(&p, &Plane::XY, 0.0, 20.0, 1).unwrap();
        let top = face_with_normal(&cyl, Vec3::Z);
        let s = shell(&cyl, 2.0, &[top], 9).unwrap();
        let inner: Vec<&Surface> = s
            .surfaces
            .iter()
            .filter(
                |su| matches!(su, Surface::Cylinder { radius, .. } if (radius - 8.0).abs() < 1e-9),
            )
            .collect();
        assert_eq!(
            inner.len(),
            1,
            "one inner cylinder surface, found {inner:?}"
        );
        // Volume: faceted, so compare against the faceted outer and inner areas.
        let outer_area = p.area();
        let ratio = (8.0f64 / 10.0).powi(2);
        let expected = outer_area * 20.0 - outer_area * ratio * 18.0;
        assert!(
            (s.volume() - expected).abs() / expected < 2e-3,
            "{} vs {expected}",
            s.volume()
        );
    }

    #[test]
    fn l_shaped_block_keeps_full_wall_at_the_inner_corner() {
        // An L: 30x30 block minus a 15x15 corner, shelled 3 mm with the top open.
        let mut sk = Sketch::new();
        let corners = [
            Vec2::new(0.0, 0.0),
            Vec2::new(30.0, 0.0),
            Vec2::new(30.0, 15.0),
            Vec2::new(15.0, 15.0),
            Vec2::new(15.0, 30.0),
            Vec2::new(0.0, 30.0),
        ];
        for i in 0..corners.len() {
            sk.add_line(corners[i], corners[(i + 1) % corners.len()]);
        }
        let p = sk.profiles(&ProfileOptions::default()).remove(0);
        let l = extrude(&p, &Plane::XY, 0.0, 10.0, 1).unwrap();
        let top = face_with_normal(&l, Vec3::Z);
        let s = shell(&l, 3.0, &[top], 9).unwrap();
        // Cavity: the L offset inward by 3 (the 24x24 square minus the
        // notch grown to 15x15), 7 tall. A concave-corner wedge missing at
        // (15, 15) would show up as a larger cavity.
        let cavity = (24.0 * 24.0 - 15.0 * 15.0) * 7.0;
        let expected = (30.0 * 30.0 - 15.0 * 15.0) * 10.0 - cavity;
        assert!(
            (s.volume() - expected).abs() < 1e-6,
            "{} vs {expected}",
            s.volume()
        );
    }

    #[test]
    fn move_face_pushes_and_pulls_a_box_face() {
        let b = block(30.0, 20.0, 10.0);
        let top = face_with_normal(&b, Vec3::Z);
        let taller = move_faces(&b, &[top], 5.0).unwrap();
        assert!((taller.volume() - 30.0 * 20.0 * 15.0).abs() < 1e-6);
        let right = face_with_normal(&b, Vec3::X);
        let narrower = move_faces(&b, &[right], -12.0).unwrap();
        assert!((narrower.volume() - 18.0 * 20.0 * 10.0).abs() < 1e-6);
        assert_eq!(narrower.faces.len(), 6);
        // Moving a face past the opposite one collapses the box.
        assert!(move_faces(&b, &[right], -31.0).is_err());
    }

    #[test]
    fn draft_tapers_a_box_towards_the_pull_direction() {
        let b = block(30.0, 20.0, 10.0);
        let sides: Vec<usize> = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y]
            .into_iter()
            .map(|n| face_with_normal(&b, n))
            .collect();
        let neutral = Plane::XY; // pull direction +Z: the top shrinks
        let d = draft_faces(&b, &sides, &neutral, 10.0).unwrap();
        d.validate().unwrap();
        let k = 10f64.to_radians().tan();
        // Width and depth shrink linearly with height.
        let expected: f64 = (0..1000)
            .map(|i| {
                let z = (i as f64 + 0.5) / 100.0;
                (30.0 - 2.0 * k * z) * (20.0 - 2.0 * k * z) * 0.01
            })
            .sum();
        assert!(
            (d.volume() - expected).abs() < 1e-3,
            "{} vs {expected}",
            d.volume()
        );
        // The top face is smaller than the bottom by the taper.
        let top = d
            .faces
            .iter()
            .find(|f| f.plane.normal.approx_eq(Vec3::Z))
            .unwrap();
        let xs: Vec<f64> = top.loops[0]
            .iter()
            .map(|&v| d.vertices[v as usize].x)
            .collect();
        let w = xs.iter().cloned().fold(f64::MIN, f64::max)
            - xs.iter().cloned().fold(f64::MAX, f64::min);
        assert!((w - (30.0 - 2.0 * 10.0 * k)).abs() < 1e-6, "{w}");
        // A face parallel to the neutral plane cannot be drafted.
        let top_i = face_with_normal(&b, Vec3::Z);
        assert!(draft_faces(&b, &[top_i], &neutral, 10.0).is_err());
    }

    #[test]
    fn too_thick_is_an_error() {
        let b = block(10.0, 10.0, 10.0);
        assert!(shell(&b, 6.0, &[], 9).is_err());
        assert!(shell(&b, 0.0, &[], 9).is_err());
    }
}
