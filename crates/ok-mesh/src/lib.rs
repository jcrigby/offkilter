//! Triangle meshes for display and export.
//!
//! Until the kernel has a boundary representation, solid bodies are
//! represented directly as closed triangle meshes. Meshes are flat shaded:
//! vertices are duplicated per face so that each triangle carries its own
//! normal.

mod extrude;

pub use extrude::extrude_profile;

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
