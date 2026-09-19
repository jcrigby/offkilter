//! Rigid transforms and reflections of solids.

use crate::{Face, Solid, Surface};
use ok_math::{Plane, Vec3};
use serde::{Deserialize, Serialize};

/// An affine map `p -> m * p + t` with an orthogonal 3x3 matrix `m`
/// (rotation, possibly with a reflection).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// Row-major 3x3 matrix.
    pub m: [[f64; 3]; 3],
    pub t: Vec3,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: Vec3::ZERO,
    };

    pub fn translation(t: Vec3) -> Transform {
        Transform {
            t,
            ..Transform::IDENTITY
        }
    }

    /// Rotation by `angle` radians about the line through `origin` along unit `axis`.
    pub fn rotation(origin: Vec3, axis: Vec3, angle: f64) -> Transform {
        let (s, c) = angle.sin_cos();
        let (x, y, z) = (axis.x, axis.y, axis.z);
        let oc = 1.0 - c;
        let m = [
            [c + x * x * oc, x * y * oc - z * s, x * z * oc + y * s],
            [y * x * oc + z * s, c + y * y * oc, y * z * oc - x * s],
            [z * x * oc - y * s, z * y * oc + x * s, c + z * z * oc],
        ];
        let r = Transform { m, t: Vec3::ZERO };
        let t = origin - r.apply_vector(origin);
        Transform { m, t }
    }

    /// Reflection across a plane.
    pub fn mirror(plane: &Plane) -> Transform {
        let n = plane.normal;
        let m = [
            [1.0 - 2.0 * n.x * n.x, -2.0 * n.x * n.y, -2.0 * n.x * n.z],
            [-2.0 * n.y * n.x, 1.0 - 2.0 * n.y * n.y, -2.0 * n.y * n.z],
            [-2.0 * n.z * n.x, -2.0 * n.z * n.y, 1.0 - 2.0 * n.z * n.z],
        ];
        let d = n.dot(plane.origin);
        Transform {
            m,
            t: n * (2.0 * d),
        }
    }

    pub fn apply_vector(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }

    pub fn apply_point(&self, p: Vec3) -> Vec3 {
        self.apply_vector(p) + self.t
    }

    /// The rigid transform taking points placed by `from` to where this
    /// transform places them: `self ∘ from⁻¹`. Both must be rigid (their
    /// matrices orthonormal), as assembly placements are.
    pub fn then_inverse_of(&self, from: &Transform) -> Transform {
        // from⁻¹ = (Rᵀ, -Rᵀ t) for a rigid `from`.
        let mut rt = [[0.0; 3]; 3];
        for (i, row) in from.m.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                rt[j][i] = *v;
            }
        }
        let inv = Transform { m: rt, t: Vec3::ZERO };
        let inv = Transform {
            m: rt,
            t: -inv.apply_vector(from.t),
        };
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.m[i][k] * inv.m[k][j]).sum();
            }
        }
        Transform {
            m,
            t: self.apply_point(inv.t),
        }
    }

    pub fn determinant(&self) -> f64 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// Whether this map reverses orientation (contains a reflection).
    pub fn is_reflection(&self) -> bool {
        self.determinant() < 0.0
    }
}

impl Solid {
    /// Returns the solid mapped by `xf`. Reflections reverse every face so
    /// the result stays outward-oriented.
    pub fn transformed(&self, xf: &Transform) -> Solid {
        let reflect = xf.is_reflection();
        let vertices: Vec<Vec3> = self.vertices.iter().map(|&v| xf.apply_point(v)).collect();
        let surfaces: Vec<Surface> = self
            .surfaces
            .iter()
            .map(|s| match *s {
                Surface::Plane { normal, offset } => {
                    let n = xf.apply_vector(normal);
                    let p = xf.apply_point(normal * offset);
                    Surface::Plane {
                        normal: n,
                        offset: n.dot(p),
                    }
                }
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } => Surface::Cylinder {
                    origin: xf.apply_point(origin),
                    axis: xf.apply_vector(axis),
                    radius,
                },
                Surface::Ruled => Surface::Ruled,
                Surface::Revolved { origin, axis } => Surface::Revolved {
                    origin: xf.apply_point(origin),
                    axis: xf.apply_vector(axis),
                },
            })
            .collect();
        let faces: Vec<Face> = self
            .faces
            .iter()
            .map(|f| {
                let origin = xf.apply_point(f.plane.origin);
                let x_axis = xf.apply_vector(f.plane.x_axis);
                let y_axis = xf.apply_vector(f.plane.y_axis);
                let normal = xf.apply_vector(f.plane.normal);
                // A reflected frame is left-handed; swapping the axes and
                // reversing the loops restores a right-handed outward face.
                let (plane, loops) = if reflect {
                    (
                        Plane {
                            origin,
                            x_axis: y_axis,
                            y_axis: x_axis,
                            normal,
                        },
                        f.loops
                            .iter()
                            .map(|l| l.iter().rev().copied().collect())
                            .collect(),
                    )
                } else {
                    (
                        Plane {
                            origin,
                            x_axis,
                            y_axis,
                            normal,
                        },
                        f.loops.clone(),
                    )
                };
                Face {
                    plane,
                    loops,
                    surface: f.surface,
                    origin: f.origin,
                }
            })
            .collect();
        Solid {
            vertices,
            faces,
            surfaces,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn then_inverse_of_maps_between_two_placements() {
        let from = Transform::rotation(Vec3::new(1.0, 2.0, 3.0), Vec3::Z, 0.7);
        let to = Transform::rotation(Vec3::new(-2.0, 0.5, 1.0), Vec3::new(0.0, 1.0, 0.0), -1.3);
        let to = Transform {
            t: to.t + Vec3::new(4.0, 5.0, 6.0),
            ..to
        };
        let d = to.then_inverse_of(&from);
        for p in [Vec3::ZERO, Vec3::new(1.0, -2.0, 0.5), Vec3::new(-3.0, 4.0, 9.0)] {
            let placed = from.apply_point(p);
            assert!(d.apply_point(placed).distance(to.apply_point(p)) < 1e-12);
        }
    }
    use crate::extrude;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn block() -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::new(1.0, 0.0), Vec2::new(3.0, 2.0));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, 4.0, 1).unwrap()
    }

    #[test]
    fn translation_and_rotation_keep_volume_and_validity() {
        let b = block();
        let t = b.transformed(&Transform::translation(Vec3::new(5.0, -2.0, 1.0)));
        t.validate().unwrap();
        assert!((t.volume() - 16.0).abs() < 1e-9);
        assert!(t.bounds().unwrap().0.approx_eq(Vec3::new(6.0, -2.0, 1.0)));
        let r = b.transformed(&Transform::rotation(
            Vec3::ZERO,
            Vec3::Z,
            std::f64::consts::FRAC_PI_2,
        ));
        r.validate().unwrap();
        assert!((r.volume() - 16.0).abs() < 1e-9);
        let (min, max) = r.bounds().unwrap();
        assert!(
            min.approx_eq(Vec3::new(-2.0, 1.0, 0.0)) && max.approx_eq(Vec3::new(0.0, 3.0, 4.0)),
            "{min:?} {max:?}"
        );
    }

    #[test]
    fn mirror_reverses_faces_and_keeps_positive_volume() {
        let b = block();
        let m = b.transformed(&Transform::mirror(&Plane::YZ));
        m.validate().unwrap();
        assert!((m.volume() - 16.0).abs() < 1e-9, "volume {}", m.volume());
        let (min, max) = m.bounds().unwrap();
        assert!((min.x + 3.0).abs() < 1e-9 && (max.x + 1.0).abs() < 1e-9);
        // Mirror then union with the original across a shared face merges cleanly.
        let m2 = b.transformed(&Transform::mirror(&Plane::YZ.offset(1.0)));
        let u = crate::boolean(&b, &m2, crate::BoolOp::Union).unwrap();
        assert!((u.volume() - 32.0).abs() < 1e-9);
        assert_eq!(u.faces.len(), 6);
    }
}
