use crate::{Solid, Surface};
use ok_math::Vec3;
use ok_mesh::TriMesh;

/// Triangulates every face. Planar faces get their face normal; facets on a
/// cylinder get analytic per-vertex normals so the surface shades smoothly.
pub fn tessellate(solid: &Solid) -> TriMesh {
    tessellate_with_faces(solid).0
}

/// Like [`tessellate`], also returning the face index of every triangle.
pub fn tessellate_with_faces(solid: &Solid) -> (TriMesh, Vec<u32>) {
    let mut mesh = TriMesh::new();
    let mut triangle_faces = Vec::new();
    for (fi, f) in solid.faces.iter().enumerate() {
        let mut flat: Vec<f64> = Vec::new();
        let mut holes: Vec<usize> = Vec::new();
        let mut verts: Vec<Vec3> = Vec::new();
        for (li, l) in f.loops.iter().enumerate() {
            if li > 0 {
                holes.push(flat.len() / 2);
            }
            for &v in l {
                let p = solid.vertices[v as usize];
                let q = f.plane.to_plane(p);
                flat.extend([q.x, q.y]);
                verts.push(p);
            }
        }
        let tris = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
        let surface = solid.surfaces.get(f.surface).copied();
        let normal_at = |p: Vec3| -> Vec3 {
            match surface {
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
        for (i, p) in verts.iter().enumerate() {
            let n = normal_at(*p);
            let _ = i;
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

/// Edges worth drawing: those between different surfaces, excluding
/// seams between coplanar planar faces.
pub fn display_edges(solid: &Solid) -> Vec<[Vec3; 2]> {
    let mut out = Vec::new();
    for ((a, b), faces) in solid.edge_faces() {
        let show = match faces.as_slice() {
            [f0, f1] => {
                let (fa, fb) = (&solid.faces[*f0], &solid.faces[*f1]);
                if fa.surface == fb.surface {
                    false
                } else {
                    let coplanar = fa.plane.normal.dot(fb.plane.normal) > 1.0 - 1e-9;
                    let both_planar = matches!(solid.surfaces[fa.surface], Surface::Plane { .. })
                        && matches!(solid.surfaces[fb.surface], Surface::Plane { .. });
                    !(coplanar && both_planar)
                }
            }
            _ => true,
        };
        if show {
            out.push([solid.vertices[a as usize], solid.vertices[b as usize]]);
        }
    }
    out
}
