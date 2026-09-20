//! Spherical corner patches where three fillets meet.
//!
//! Three convex fillets of one radius that meet at a vertex between three
//! planar faces are exactly a rolling ball: each fillet's axis passes
//! through the centre `c` that lies `r` inside all three faces, and the
//! ball there is tangent to all three cylinders along the circles in the
//! planes through `c` perpendicular to the edges. So the cutter for such a
//! corner is one closed polyhedron: the three edge prisms cut back to
//! those planes, the corner cell between the planes and the faces, and a
//! spherical patch spanning the three fillet arcs. Building it as one
//! polygon set (rather than a union of solids) keeps the seams exact: the
//! patch's boundary uses the prisms' own arc vertices.
//!
//! Anything else (chamfers, rims, unequal counts of edges at a vertex,
//! faces that are not planes) keeps the plain union of cutters.

use crate::{extrude, BrepError, FaceOrigin, Polygon, Solid, Surface};
use ok_math::{Plane, Vec3};
use ok_sketch::{Loop, Profile};
use std::collections::HashMap;

/// A straight convex edge to fillet: its vertices, the two faces (the
/// first on the frame's x side), and the cross-section in the frame with
/// origin `p`, x along the first face's inward direction and normal along
/// the edge.
#[derive(Clone)]
pub(crate) struct FilletEdge {
    pub a: u32,
    pub b: u32,
    pub faces: (usize, usize),
    pub frame: Plane,
    pub section: Loop,
}

struct Corner {
    vertex: u32,
    /// Indices into the edge list, with each edge's direction away from the vertex.
    edges: [(usize, Vec3); 3],
    /// The three faces around the corner and their outward normals.
    faces: [(usize, Vec3); 3],
    centre: Vec3,
}

/// Builds one cutter per connected group of patched corners and the edges
/// between them. Returns the cutters and, per edge, whether one of them
/// took it (those edges need no prism of their own).
pub(crate) fn patched_cutters(
    solid: &Solid,
    edges: &[FilletEdge],
    radius: f64,
    segment_angle: f64,
    feature: u32,
) -> (Vec<Solid>, Vec<bool>) {
    let mut taken = vec![false; edges.len()];
    let corners = find_corners(solid, edges, radius);
    if corners.is_empty() {
        return (Vec::new(), taken);
    }
    // Where each edge is cut back at either end (distance from p / from q).
    let mut cut_at: Vec<(f64, f64)> = vec![(0.0, 0.0); edges.len()];
    for c in &corners {
        for &(ei, dir) in &c.edges {
            let t = (c.centre - solid.vertices[c.vertex as usize]).dot(dir);
            if edges[ei].a == c.vertex {
                cut_at[ei].0 = t;
            } else {
                cut_at[ei].1 = t;
            }
        }
    }
    // Corners whose cells would overlap along an edge are left alone.
    let corners: Vec<Corner> = corners
        .into_iter()
        .filter(|c| {
            c.edges.iter().all(|&(ei, _)| {
                let len = edges[ei]
                    .frame
                    .origin
                    .distance(solid.vertices[edges[ei].b as usize]);
                cut_at[ei].0 + cut_at[ei].1 < len - 1e-6 * len
            })
        })
        .collect();
    // Group corners and edges into connected components.
    let mut parent: Vec<usize> = (0..edges.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for c in &corners {
        let r0 = find(&mut parent, c.edges[0].0);
        for &(ei, _) in &c.edges[1..] {
            let r = find(&mut parent, ei);
            parent[r] = r0;
        }
    }
    let mut groups: HashMap<usize, (Vec<usize>, Vec<usize>)> = HashMap::new();
    for (ci, c) in corners.iter().enumerate() {
        let root = find(&mut parent, c.edges[0].0);
        let g = groups.entry(root).or_default();
        g.0.push(ci);
        for &(ei, _) in &c.edges {
            if !g.1.contains(&ei) {
                g.1.push(ei);
            }
        }
    }
    let mut roots: Vec<usize> = groups.keys().copied().collect();
    roots.sort_unstable();
    let mut out = Vec::new();
    for (gi, root) in roots.into_iter().enumerate() {
        let (corner_ids, edge_ids) = &groups[&root];
        let patched_ends = |ei: usize| -> (bool, bool) {
            let at_p = corner_ids.iter().any(|&ci| {
                corners[ci].vertex == edges[ei].a && corners[ci].edges.iter().any(|e| e.0 == ei)
            });
            let at_q = corner_ids.iter().any(|&ci| {
                corners[ci].vertex == edges[ei].b && corners[ci].edges.iter().any(|e| e.0 == ei)
            });
            (at_p, at_q)
        };
        let built = build_group(
            solid,
            edges,
            &corners,
            corner_ids,
            edge_ids,
            &cut_at,
            &patched_ends,
            radius,
            segment_angle,
            feature,
            gi as u32,
        );
        if let Ok(s) = built {
            for &ei in edge_ids {
                taken[ei] = true;
            }
            out.push(s);
        }
    }
    (out, taken)
}

/// Vertices where exactly three of the edges meet across exactly three
/// planar faces, with the ball centre that touches all three.
fn find_corners(solid: &Solid, edges: &[FilletEdge], radius: f64) -> Vec<Corner> {
    let mut at: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        at.entry(e.a).or_default().push(i);
        at.entry(e.b).or_default().push(i);
    }
    let mut keys: Vec<u32> = at.keys().copied().collect();
    keys.sort_unstable();
    let mut out = Vec::new();
    for v in keys {
        let list = &at[&v];
        if list.len() != 3 {
            continue;
        }
        let mut faces: Vec<usize> = list
            .iter()
            .flat_map(|&i| [edges[i].faces.0, edges[i].faces.1])
            .collect();
        faces.sort_unstable();
        faces.dedup();
        if faces.len() != 3 {
            continue;
        }
        if !faces.iter().all(|&f| {
            matches!(
                solid.surfaces.get(solid.faces[f].surface),
                Some(Surface::Plane { .. })
            )
        }) {
            continue;
        }
        let n: Vec<Vec3> = faces.iter().map(|&f| solid.faces[f].plane.normal).collect();
        let det = n[0].dot(n[1].cross(n[2]));
        if det.abs() < 1e-6 {
            continue;
        }
        // Solve n_i · d = -r for d = c - v (Cramer's rule).
        let rhs = Vec3::new(-radius, -radius, -radius);
        let comp = |v: Vec3, k: usize| match k {
            0 => v.x,
            1 => v.y,
            _ => v.z,
        };
        let col = |k: usize| Vec3::new(comp(n[0], k), comp(n[1], k), comp(n[2], k));
        let (c0, c1, c2) = (col(0), col(1), col(2));
        let d = Vec3::new(
            rhs.dot(c1.cross(c2)) / det,
            c0.dot(rhs.cross(c2)) / det,
            c0.dot(c1.cross(rhs)) / det,
        );
        let vertex = solid.vertices[v as usize];
        let centre = vertex + d;
        let mut dirs = [(0usize, Vec3::ZERO); 3];
        let mut ok = true;
        for (k, &ei) in list.iter().enumerate() {
            let e = &edges[ei];
            let (p, q) = (solid.vertices[e.a as usize], solid.vertices[e.b as usize]);
            let away = if e.a == v { q - p } else { p - q };
            let len = away.length();
            let Some(dir) = away.normalized() else {
                ok = false;
                break;
            };
            let t = d.dot(dir);
            if t <= 1e-9 || t >= len {
                ok = false;
                break;
            }
            dirs[k] = (ei, dir);
        }
        if !ok {
            continue;
        }
        out.push(Corner {
            vertex: v,
            edges: dirs,
            faces: [(faces[0], n[0]), (faces[1], n[1]), (faces[2], n[2])],
            centre,
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn build_group(
    solid: &Solid,
    edges: &[FilletEdge],
    corners: &[Corner],
    corner_ids: &[usize],
    edge_ids: &[usize],
    cut_at: &[(f64, f64)],
    patched_ends: &dyn Fn(usize) -> (bool, bool),
    radius: f64,
    segment_angle: f64,
    feature: u32,
    group: u32,
) -> Result<Solid, BrepError> {
    let mut polys: Vec<Polygon> = Vec::new();
    let mut surfaces: Vec<Surface> = Vec::new();
    let local_base = 500_000 + group * 10_000;
    // The prisms, cut back at patched ends, without the caps there.
    for (k, &ei) in edge_ids.iter().enumerate() {
        let e = &edges[ei];
        let len = e.frame.origin.distance(solid.vertices[e.b as usize]);
        let (at_p, at_q) = patched_ends(ei);
        let t0 = if at_p { cut_at[ei].0 } else { 0.0 };
        let t1 = if at_q { len - cut_at[ei].1 } else { len };
        let profile = Profile {
            outer: e.section.clone(),
            holes: vec![],
        };
        let prism = extrude(&profile, &e.frame, t0, t1, feature)?;
        let axis = e.frame.normal;
        let offset = surfaces.len();
        surfaces.extend_from_slice(&prism.surfaces);
        for f in &prism.faces {
            let along = axis.dot(f.plane.normal);
            let height = axis.dot(f.plane.origin - e.frame.origin);
            let is_cap = along.abs() > 0.999;
            if is_cap
                && ((at_p && (height - t0).abs() < 1e-7 * len)
                    || (at_q && (height - t1).abs() < 1e-7 * len))
            {
                continue;
            }
            polys.push(Polygon {
                plane: f.plane,
                loops: f
                    .loops
                    .iter()
                    .map(|l| l.iter().map(|&v| prism.vertices[v as usize]).collect())
                    .collect(),
                surface: offset + f.surface,
                origin: FaceOrigin {
                    feature,
                    local: local_base + k as u32 * 100 + f.origin.local,
                },
            });
        }
    }
    // Each corner: the cell's three face quads and the spherical patch.
    for (ci, &cid) in corner_ids.iter().enumerate() {
        let c = &corners[cid];
        let v = solid.vertices[c.vertex as usize];
        // Edge points where the cells' planes cross the edges.
        let edge_point = |k: usize| v + c.edges[k].1 * (c.centre - v).dot(c.edges[k].1);
        for &(face, normal) in &c.faces {
            // The two edges on this face.
            let on: Vec<usize> = (0..3)
                .filter(|&k| {
                    let e = &edges[c.edges[k].0];
                    e.faces.0 == face || e.faces.1 == face
                })
                .collect();
            if on.len() != 2 {
                return Err(BrepError::Degenerate(
                    "corner face without two edges".into(),
                ));
            }
            let q = c.centre + normal * radius;
            let mut quad = vec![v, edge_point(on[0]), q, edge_point(on[1])];
            let n = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
            if n.dot(normal) < 0.0 {
                quad.reverse();
            }
            let x = (quad[1] - quad[0]).normalized().unwrap_or(Vec3::X);
            surfaces.push(Surface::Plane {
                normal,
                offset: normal.dot(v),
            });
            polys.push(Polygon {
                plane: Plane {
                    origin: quad[0],
                    x_axis: x,
                    y_axis: normal.cross(x),
                    normal,
                },
                loops: vec![quad],
                surface: surfaces.len() - 1,
                origin: FaceOrigin {
                    feature,
                    local: local_base + 5_000 + ci as u32 * 10 + face as u32 % 10,
                },
            });
        }
        // The patch boundary: the three fillet arcs at the cell planes, as
        // the prisms have them, chained end to end.
        let mut arcs: Vec<Vec<Vec3>> = Vec::new();
        for &(ei, _) in &c.edges {
            let e = &edges[ei];
            let len = e.frame.origin.distance(solid.vertices[e.b as usize]);
            let height = if e.a == c.vertex {
                cut_at[ei].0
            } else {
                len - cut_at[ei].1
            };
            let pts = &e.section.points;
            let zero = pts
                .iter()
                .position(|p| p.length() < 1e-12)
                .ok_or_else(|| BrepError::Degenerate("section without its corner".into()))?;
            let n = pts.len();
            let arc: Vec<Vec3> = (1..n)
                .map(|i| e.frame.to_world_at(pts[(zero + i) % n], height))
                .collect();
            arcs.push(arc);
        }
        let boundary = chain_arcs(arcs, radius * 1e-6)?;
        let patch_local = local_base + 8_000 + ci as u32;
        sphere_patch(
            &boundary,
            c.centre,
            radius,
            segment_angle,
            feature,
            patch_local,
            &mut polys,
            &mut surfaces,
        )?;
    }
    Solid::from_polygons(polys, surfaces)
}

/// Joins arcs end to end (reversing as needed) into one closed loop of
/// points, without repeating the shared endpoints.
fn chain_arcs(mut arcs: Vec<Vec<Vec3>>, tol: f64) -> Result<Vec<Vec3>, BrepError> {
    let mut chain = arcs.remove(0);
    while !arcs.is_empty() {
        let end = *chain.last().unwrap();
        let next = arcs
            .iter()
            .position(|a| a[0].distance(end) < tol || a[a.len() - 1].distance(end) < tol)
            .ok_or_else(|| BrepError::Degenerate("fillet arcs do not meet".into()))?;
        let mut a = arcs.remove(next);
        if a[0].distance(end) >= tol {
            a.reverse();
        }
        chain.extend(a.into_iter().skip(1));
    }
    if chain.len() > 1 && chain[0].distance(*chain.last().unwrap()) < tol {
        chain.pop();
    }
    if chain.len() < 3 {
        return Err(BrepError::Degenerate(
            "corner patch boundary too short".into(),
        ));
    }
    Ok(chain)
}

fn slerp(a: Vec3, b: Vec3, f: f64) -> Vec3 {
    let cos = a.dot(b).clamp(-1.0, 1.0);
    let omega = cos.acos();
    if omega < 1e-9 {
        return (a * (1.0 - f) + b * f).normalized().unwrap_or(a);
    }
    (a * ((1.0 - f) * omega).sin() + b * (f * omega).sin()) * (1.0 / omega.sin())
}

/// Triangulates the spherical region inside `boundary` (points on the
/// sphere about `centre`) as rings shrinking from the boundary towards its
/// middle direction, with the ring spacing at most `segment_angle`. The
/// boundary ring keeps the given points exactly. Triangles face the
/// centre: this is the cutter's surface, which lies outside the ball.
#[allow(clippy::too_many_arguments)]
fn sphere_patch(
    boundary: &[Vec3],
    centre: Vec3,
    radius: f64,
    segment_angle: f64,
    feature: u32,
    local: u32,
    polys: &mut Vec<Polygon>,
    surfaces: &mut Vec<Surface>,
) -> Result<(), BrepError> {
    let dirs: Vec<Vec3> = boundary
        .iter()
        .map(|&p| (p - centre).normalized().unwrap_or(Vec3::Z))
        .collect();
    let sum = dirs.iter().fold(Vec3::ZERO, |a, &d| a + d);
    let m = sum
        .normalized()
        .ok_or_else(|| BrepError::Degenerate("corner patch has no middle".into()))?;
    let span = dirs
        .iter()
        .map(|d| d.dot(m).clamp(-1.0, 1.0).acos())
        .fold(0.0, f64::max);
    let levels = ((span / segment_angle).ceil() as usize).max(1);
    let b = dirs.len();
    // Ring `levels` is the boundary itself; inner rings thin out with their size.
    let ring_at = |level: usize| -> Vec<Vec3> {
        if level == levels {
            return boundary.to_vec();
        }
        if level == 0 {
            return vec![centre + m * radius];
        }
        let f = level as f64 / levels as f64;
        let count = ((b as f64 * f).round() as usize).max(3).min(b);
        (0..count)
            .map(|j| {
                let i = (j * b) / count;
                centre + slerp(m, dirs[i], f) * radius
            })
            .collect()
    };
    surfaces.push(Surface::Sphere {
        center: centre,
        radius,
    });
    let surface = surfaces.len() - 1;
    let mut push_tri = |a: Vec3, bb: Vec3, c: Vec3| {
        let Some(n) = (bb - a).cross(c - a).normalized() else {
            return;
        };
        // Outward for the cutter is towards the ball's centre.
        let (tri, n) = if n.dot(centre - a) >= 0.0 {
            (vec![a, bb, c], n)
        } else {
            (vec![a, c, bb], -n)
        };
        let x = (tri[1] - tri[0]).normalized().unwrap_or(Vec3::X);
        polys.push(Polygon {
            plane: Plane {
                origin: tri[0],
                x_axis: x,
                y_axis: n.cross(x),
                normal: n,
            },
            loops: vec![tri],
            surface,
            origin: FaceOrigin { feature, local },
        });
    };
    let mut inner = ring_at(0);
    for level in 1..=levels {
        let outer = ring_at(level);
        if inner.len() == 1 {
            for j in 0..outer.len() {
                push_tri(inner[0], outer[j], outer[(j + 1) % outer.len()]);
            }
        } else {
            // Zip two closed rings by their fractional positions.
            let (ni, no) = (inner.len(), outer.len());
            let (mut i, mut j) = (0usize, 0usize);
            while i < ni || j < no {
                let next_i = (i + 1) as f64 / ni as f64;
                let next_j = (j + 1) as f64 / no as f64;
                if j >= no || (i < ni && next_i <= next_j) {
                    push_tri(inner[i % ni], outer[j % no], inner[(i + 1) % ni]);
                    i += 1;
                } else {
                    push_tri(inner[i % ni], outer[j % no], outer[(j + 1) % no]);
                    j += 1;
                }
            }
        }
        inner = outer;
    }
    Ok(())
}
