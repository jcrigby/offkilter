use crate::TriMesh;
use ok_math::{Plane, Vec2};
use ok_sketch::Profile;

/// Extrudes a planar profile along the plane normal from `start` to `end`
/// (heights measured along the normal). The result is a closed, outward
/// facing mesh. `end` may be less than `start`.
pub fn extrude_profile(profile: &Profile, plane: &Plane, start: f64, end: f64) -> TriMesh {
    let mut mesh = TriMesh::new();
    if profile.outer.len() < 3 || (end - start).abs() < ok_math::tol::LINEAR {
        return mesh;
    }
    let (lo, hi) = if start < end {
        (start, end)
    } else {
        (end, start)
    };

    // Caps. The profile outer loop is CCW in plane coordinates, holes CW,
    // which is what earcut expects for a flat vertex list with hole starts.
    let mut flat: Vec<f64> = Vec::new();
    let mut hole_starts: Vec<usize> = Vec::new();
    for p in &profile.outer {
        flat.extend([p.x, p.y]);
    }
    for h in &profile.holes {
        hole_starts.push(flat.len() / 2);
        for p in h {
            flat.extend([p.x, p.y]);
        }
    }
    let tris = earcutr::earcut(&flat, &hole_starts, 2).unwrap_or_default();
    let at = |i: usize| Vec2::new(flat[2 * i], flat[2 * i + 1]);
    for t in tris.chunks_exact(3) {
        let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
        // Top cap faces +normal: keep CCW. Bottom cap faces -normal: flip.
        mesh.push_triangle(
            plane.to_world_at(a, hi),
            plane.to_world_at(b, hi),
            plane.to_world_at(c, hi),
        );
        mesh.push_triangle(
            plane.to_world_at(a, lo),
            plane.to_world_at(c, lo),
            plane.to_world_at(b, lo),
        );
    }

    // Side walls. Walking a CCW outer loop, the outward side is to the
    // right; the quad (a_lo, b_lo, b_hi, a_hi) is then CCW from outside.
    // Holes are CW so the same rule points their walls inward-facing-out.
    let mut walls = |ring: &[Vec2]| {
        let n = ring.len();
        for i in 0..n {
            let a = ring[i];
            let b = ring[(i + 1) % n];
            mesh.push_quad(
                plane.to_world_at(a, lo),
                plane.to_world_at(b, lo),
                plane.to_world_at(b, hi),
                plane.to_world_at(a, hi),
            );
        }
    };
    walls(&profile.outer);
    for h in &profile.holes {
        walls(h);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_sketch::{ProfileOptions, Sketch};

    #[test]
    fn box_volume_is_positive_and_correct() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(4.0, 2.0));
        let p = s.profiles(&ProfileOptions::default());
        let m = extrude_profile(&p[0], &Plane::XY, 0.0, 3.0);
        assert_eq!(m.triangle_count(), 12);
        assert!((m.signed_volume() - 24.0).abs() < 1e-4);
        let (min, max) = m.bounds().unwrap();
        assert!(min.approx_eq(ok_math::Vec3::ZERO));
        assert!(max.approx_eq(ok_math::Vec3::new(4.0, 2.0, 3.0)));
    }

    #[test]
    fn reversed_extrude_still_outward() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(1.0, 1.0));
        let p = s.profiles(&ProfileOptions::default());
        let m = extrude_profile(&p[0], &Plane::XZ, 0.0, -2.0);
        assert!((m.signed_volume() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tube_volume_subtracts_hole() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 10.0));
        s.add_circle(Vec2::new(5.0, 5.0), 2.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let m = extrude_profile(&p[0], &Plane::XY, 0.0, 1.0);
        let expected = 100.0 - std::f64::consts::PI * 4.0;
        assert!(
            (m.signed_volume() - expected).abs() < 0.1,
            "vol {}",
            m.signed_volume()
        );
    }
}
