use crate::{Solid, Surface};
use ok_math::Vec3;
use ok_mesh::TriMesh;
use std::collections::HashMap;

/// Triangulates every face. Planar faces get their face normal; facets on a
/// cylinder get analytic per-vertex normals so the surface shades smoothly.
pub fn tessellate(solid: &Solid) -> TriMesh {
    tessellate_with_faces(solid).0
}

/// Like [`tessellate`], also returning the face index of every triangle.
pub fn tessellate_with_faces(solid: &Solid) -> (TriMesh, Vec<u32>) {
    let mut mesh = TriMesh::new();
    let mut triangle_faces = Vec::new();
    // Area-weighted normal sums per (surface, vertex) for smooth surfaces
    // without an analytic normal (surfaces of revolution).
    let mut averaged: HashMap<(usize, u32), Vec3> = HashMap::new();
    for f in &solid.faces {
        if matches!(
            solid.surfaces.get(f.surface),
            Some(Surface::Revolved { .. } | Surface::Ruled)
        ) {
            let l = &f.loops[0];
            let mut n = Vec3::ZERO;
            for i in 0..l.len() {
                n += solid.vertices[l[i] as usize]
                    .cross(solid.vertices[l[(i + 1) % l.len()] as usize]);
            }
            for loop_ in &f.loops {
                for &v in loop_ {
                    *averaged.entry((f.surface, v)).or_insert(Vec3::ZERO) += n;
                }
            }
        }
    }
    for (fi, f) in solid.faces.iter().enumerate() {
        let mut flat: Vec<f64> = Vec::new();
        let mut holes: Vec<usize> = Vec::new();
        let mut verts: Vec<(u32, Vec3)> = Vec::new();
        for (li, l) in f.loops.iter().enumerate() {
            if li > 0 {
                holes.push(flat.len() / 2);
            }
            for &v in l {
                let p = solid.vertices[v as usize];
                let q = f.plane.to_plane(p);
                flat.extend([q.x, q.y]);
                verts.push((v, p));
            }
        }
        let tris = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
        let surface = solid.surfaces.get(f.surface).copied();
        let normal_at = |v: u32, p: Vec3| -> Vec3 {
            match surface {
                Some(Surface::Revolved { .. } | Surface::Ruled) => averaged
                    .get(&(f.surface, v))
                    .and_then(|n| n.normalized())
                    .unwrap_or(f.plane.normal),
                Some(Surface::Cylinder { origin, axis, .. }) => {
                    let d = p - origin;
                    let radial = d - axis * d.dot(axis);
                    match radial.normalized() {
                        Some(r) => {
                            if r.dot(f.plane.normal) >= 0.0 {
                                r
                            } else {
                                -r
                            }
                        }
                        None => f.plane.normal,
                    }
                }
                _ => f.plane.normal,
            }
        };
        let base = mesh.vertex_count() as u32;
        for (v, p) in verts.iter() {
            let n = normal_at(*v, *p);
            mesh.positions.extend([p.x as f32, p.y as f32, p.z as f32]);
            mesh.normals.extend([n.x as f32, n.y as f32, n.z as f32]);
        }
        for t in tris.chunks_exact(3) {
            mesh.indices
                .extend([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
            triangle_faces.push(fi as u32);
        }
    }
    (mesh, triangle_faces)
}

/// A drawable edge with the (first two) faces it separates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DisplayEdge {
    pub points: [Vec3; 2],
    pub faces: [usize; 2],
}

/// Edges worth drawing: those between different surfaces, excluding
/// seams between coplanar planar faces.
pub fn display_edges(solid: &Solid) -> Vec<DisplayEdge> {
    let mut out = Vec::new();
    for ((a, b), faces) in solid.edge_faces() {
        let show = match faces.as_slice() {
            [f0, f1] => {
                let (fa, fb) = (&solid.faces[*f0], &solid.faces[*f1]);
                if fa.surface == fb.surface {
                    false
                } else {
                    let coplanar = fa.plane.normal.dot(fb.plane.normal) > 1.0 - 1e-9;
                    let both_planar = !solid.surfaces[fa.surface].is_smooth()
                        && !solid.surfaces[fb.surface].is_smooth();
                    !(coplanar && both_planar)
                }
            }
            _ => true,
        };
        if show {
            out.push(DisplayEdge {
                points: [solid.vertices[a as usize], solid.vertices[b as usize]],
                faces: [faces[0], *faces.get(1).unwrap_or(&faces[0])],
            });
        }
    }
    out
}
