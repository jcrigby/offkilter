//! Core numeric types for the offkilter kernel.
//!
//! Everything in the kernel is `f64`. Geometry is compared with the
//! tolerances in [`tol`]; never compare floating point values for exact
//! equality outside of this crate.

pub mod tol {
    /// Linear tolerance in model units (millimetres). Two points closer than
    /// this are considered coincident.
    pub const LINEAR: f64 = 1e-7;
    /// Angular tolerance in radians.
    pub const ANGULAR: f64 = 1e-9;

    #[inline]
    pub fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() <= LINEAR
    }

    #[inline]
    pub fn is_zero(a: f64) -> bool {
        a.abs() <= LINEAR
    }
}

mod plane;
mod vec2;
mod vec3;

pub use plane::Plane;
pub use vec2::Vec2;
pub use vec3::Vec3;
