use crate::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// An oriented plane with an origin and an orthonormal (x, y, normal) frame.
///
/// Sketches live in plane coordinates; [`Plane::to_world`] maps sketch
/// points into model space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub normal: Vec3,
}

impl Plane {
    /// The XY plane (Top): normal +Z.
    pub const XY: Plane = Plane {
        origin: Vec3::ZERO,
        x_axis: Vec3::X,
        y_axis: Vec3::Y,
        normal: Vec3::Z,
    };
    /// The YZ plane (Right): normal +X.
    pub const YZ: Plane = Plane {
        origin: Vec3::ZERO,
        x_axis: Vec3::Y,
        y_axis: Vec3::Z,
        normal: Vec3::X,
    };
    /// The XZ plane (Front): normal -Y so that its (x, y) frame is right-handed.
    pub const XZ: Plane = Plane {
        origin: Vec3::ZERO,
        x_axis: Vec3::X,
        y_axis: Vec3::Z,
        normal: Vec3 {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
    };

    /// Builds a plane from an origin and normal; the x axis is chosen to be
    /// perpendicular to the normal and as close to world X as possible.
    pub fn from_origin_normal(origin: Vec3, normal: Vec3) -> Option<Plane> {
        let normal = normal.normalized()?;
        let hint = if normal.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let y_axis = normal.cross(hint).normalized()?;
        let x_axis = y_axis.cross(normal);
        Some(Plane {
            origin,
            x_axis,
            y_axis,
            normal,
        })
    }

    /// Returns a copy of this plane translated along its normal.
    pub fn offset(&self, distance: f64) -> Plane {
        Plane {
            origin: self.origin + self.normal * distance,
            ..*self
        }
    }

    #[inline]
    pub fn to_world(&self, p: Vec2) -> Vec3 {
        self.origin + self.x_axis * p.x + self.y_axis * p.y
    }

    #[inline]
    pub fn to_world_at(&self, p: Vec2, height: f64) -> Vec3 {
        self.to_world(p) + self.normal * height
    }

    /// Projects a world point onto the plane and returns plane coordinates.
    #[inline]
    pub fn to_plane(&self, p: Vec3) -> Vec2 {
        let d = p - self.origin;
        Vec2::new(d.dot(self.x_axis), d.dot(self.y_axis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_planes_are_right_handed() {
        for p in [Plane::XY, Plane::YZ, Plane::XZ] {
            assert!(p.x_axis.cross(p.y_axis).approx_eq(p.normal));
        }
    }

    #[test]
    fn round_trip() {
        let p =
            Plane::from_origin_normal(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 1.0, 1.0)).unwrap();
        let s = Vec2::new(3.5, -2.0);
        assert!(p.to_plane(p.to_world(s)).approx_eq(s));
    }
}
