//! Triangle meshes for display and export.
//!
//! Solids are tessellated into these for the viewport. Vertices are
//! duplicated per face so each face carries its own normals; curved
//! surfaces get per-vertex analytic normals so they shade smoothly.

use ok_math::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TriMesh {
    /// Flat xyz triples.
    pub positions: Vec<f32>,
    /// Flat xyz triples, one per position.
    pub normals: Vec<f32>,
    /// Triangle vertex indices.
    pub indices: Vec<u32>,
}

impl TriMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Appends a flat-shaded triangle. Winding `a, b, c` is counter-clockwise
    /// when viewed from outside.
    pub fn push_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let n = (b - a).cross(c - a).normalized().unwrap_or(Vec3::Z);
        let base = self.vertex_count() as u32;
        for p in [a, b, c] {
            self.positions.extend([p.x as f32, p.y as f32, p.z as f32]);
            self.normals.extend([n.x as f32, n.y as f32, n.z as f32]);
        }
        self.indices.extend([base, base + 1, base + 2]);
    }

    /// Appends a flat-shaded quad `a b c d` (CCW from outside) as two triangles.
    pub fn push_quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        self.push_triangle(a, b, c);
        self.push_triangle(a, c, d);
    }

    pub fn append(&mut self, other: &TriMesh) {
        let base = self.vertex_count() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Axis-aligned bounding box as `(min, max)`, or `None` when empty.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        if self.positions.is_empty() {
            return None;
        }
        let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = -min;
        for p in self.positions.chunks_exact(3) {
            let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
            min = Vec3::new(min.x.min(x), min.y.min(y), min.z.min(z));
            max = Vec3::new(max.x.max(x), max.y.max(y), max.z.max(z));
        }
        Some((min, max))
    }

    /// Signed volume via the divergence theorem. Positive for outward-facing
    /// closed meshes.
    pub fn signed_volume(&self) -> f64 {
        let p = |i: u32| {
            let i = i as usize * 3;
            Vec3::new(
                self.positions[i] as f64,
                self.positions[i + 1] as f64,
                self.positions[i + 2] as f64,
            )
        };
        self.indices
            .chunks_exact(3)
            .map(|t| {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }
}

/// Binary STL of the meshes, with an 80-byte header made from `label`.
pub fn to_stl(meshes: &[TriMesh], label: &str) -> Vec<u8> {
    let count: usize = meshes.iter().map(|m| m.triangle_count()).sum();
    let mut out = Vec::with_capacity(84 + count * 50);
    let mut header = label.as_bytes().to_vec();
    header.resize(80, 0);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for m in meshes {
        for tri in m.indices.chunks_exact(3) {
            let p = |i: u32| {
                let k = i as usize * 3;
                [m.positions[k], m.positions[k + 1], m.positions[k + 2]]
            };
            let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let n = if len > 0.0 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                [0.0, 0.0, 0.0]
            };
            for f in n.iter().chain(a.iter()).chain(b.iter()).chain(c.iter()) {
                out.extend_from_slice(&f.to_le_bytes());
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}

/// A mesh read from a file: welded vertices and triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// Reads an STL file, binary or ASCII, welding the corners STL repeats
/// per triangle.
pub fn from_stl(bytes: &[u8]) -> Result<ReadMesh, String> {
    let mut corners: Vec<Vec3> = Vec::new();
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        if 84 + count * 50 == bytes.len() {
            let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as f64;
            for i in 0..count {
                let at = 84 + i * 50 + 12;
                for k in 0..3 {
                    let o = at + k * 12;
                    corners.push(Vec3::new(f(o), f(o + 4), f(o + 8)));
                }
            }
            return Ok(weld(corners));
        }
    }
    let text = String::from_utf8_lossy(bytes);
    if !text.trim_start().starts_with("solid") {
        return Err("not an STL file".into());
    }
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() == Some("vertex") {
            let mut v = [0.0; 3];
            for c in v.iter_mut() {
                *c = words
                    .next()
                    .and_then(|w| w.parse().ok())
                    .ok_or("bad vertex line in STL")?;
            }
            corners.push(Vec3::new(v[0], v[1], v[2]));
        }
    }
    if !corners.len().is_multiple_of(3) || corners.is_empty() {
        return Err("STL file has no complete triangles".into());
    }
    Ok(weld(corners))
}

/// Reads a Wavefront OBJ file's vertices and faces (polygons fanned into
/// triangles).
pub fn from_obj(text: &str) -> Result<ReadMesh, String> {
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let mut v = [0.0; 3];
                for c in v.iter_mut() {
                    *c = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or("bad vertex line in OBJ")?;
                }
                vertices.push(Vec3::new(v[0], v[1], v[2]));
            }
            Some("f") => {
                let ids: Vec<u32> = words
                    .map(|w| {
                        let i: i64 = w
                            .split('/')
                            .next()
                            .and_then(|t| t.parse().ok())
                            .ok_or("bad face line in OBJ")?;
                        let i = if i < 0 {
                            vertices.len() as i64 + i
                        } else {
                            i - 1
                        };
                        if i < 0 || i as usize >= vertices.len() {
                            return Err("OBJ face refers to a missing vertex".to_string());
                        }
                        Ok(i as u32)
                    })
                    .collect::<Result<_, _>>()?;
                for k in 1..ids.len().saturating_sub(1) {
                    triangles.push([ids[0], ids[k], ids[k + 1]]);
                }
            }
            _ => {}
        }
    }
    if triangles.is_empty() {
        return Err("OBJ file has no faces".into());
    }
    Ok(ReadMesh {
        vertices,
        triangles,
    })
}

/// Welds corners that are exactly equal into shared vertices.
fn weld(corners: Vec<Vec3>) -> ReadMesh {
    let mut seen: std::collections::HashMap<[u64; 3], u32> = std::collections::HashMap::new();
    let mut vertices = Vec::new();
    let mut ids = Vec::with_capacity(corners.len());
    for p in corners {
        let key = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let id = *seen.entry(key).or_insert_with(|| {
            vertices.push(p);
            vertices.len() as u32 - 1
        });
        ids.push(id);
    }
    ReadMesh {
        vertices,
        triangles: ids.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect(),
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    #[test]
    fn stl_written_here_reads_back_welded() {
        let mut m = TriMesh::new();
        let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        m.push_triangle(p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0));
        m.push_triangle(p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0));
        let bytes = to_stl(&[m], "two");
        let back = from_stl(&bytes).unwrap();
        assert_eq!(back.vertices.len(), 4, "shared corners welded");
        assert_eq!(back.triangles.len(), 2);
        let ascii = "solid a\n facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n vertex 0 1 0\n endloop\n endfacet\nendsolid a\n";
        let back = from_stl(ascii.as_bytes()).unwrap();
        assert_eq!(back.triangles, vec![[0, 1, 2]]);
        assert!(from_stl(b"nonsense").is_err());
    }

    #[test]
    fn obj_faces_fan_into_triangles() {
        let text = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1/1 2/2/2 3/3/3 4/4/4\n";
        let back = from_obj(text).unwrap();
        assert_eq!(back.vertices.len(), 4);
        assert_eq!(back.triangles, vec![[0, 1, 2], [0, 2, 3]]);
        assert!(from_obj("v 0 0 0\nf 1 2 5\n").is_err());
    }
}
