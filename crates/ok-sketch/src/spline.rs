//! Interpolating splines: a smooth open curve through a list of points.
//!
//! The curve is a Catmull–Rom spline: each span between two consecutive
//! points is a cubic Hermite segment whose end tangents are half the
//! chord to the neighbours on either side (one-sided at the ends), so
//! the curve passes through every point and moving one point only
//! reshapes the spans around it.

use crate::ProfileOptions;
use ok_math::Vec2;

/// How many straight pieces each span is sampled into at the given
/// facet angle: finer angles give more pieces, within 4..=32.
pub fn spline_pieces(opts: &ProfileOptions) -> usize {
    ((std::f64::consts::PI / opts.arc_segment_angle.max(1e-3)).ceil() as usize).clamp(4, 32)
}

/// The tangent used at each point: half the chord between its
/// neighbours, or the chord to the one neighbour at either end.
fn tangents(points: &[Vec2]) -> Vec<Vec2> {
    let n = points.len();
    (0..n)
        .map(|i| match (i.checked_sub(1), (i + 1 < n).then_some(i + 1)) {
            (Some(p), Some(q)) => (points[q] - points[p]) * 0.5,
            (None, Some(q)) => points[q] - points[i],
            (Some(p), None) => points[i] - points[p],
            (None, None) => Vec2::ZERO,
        })
        .collect()
}

/// Samples the spline through `points` as a polyline: `pieces` straight
/// segments per span, starting at the first point and ending at the
/// last (so `pieces * (n - 1) + 1` points). Fewer than two points give
/// the points back unchanged.
pub fn spline_polyline(points: &[Vec2], pieces: usize) -> Vec<Vec2> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let pieces = pieces.max(1);
    let m = tangents(points);
    let mut out = Vec::with_capacity(pieces * (points.len() - 1) + 1);
    for i in 0..points.len() - 1 {
        let (p0, p1, m0, m1) = (points[i], points[i + 1], m[i], m[i + 1]);
        for k in 0..pieces {
            let t = k as f64 / pieces as f64;
            let (t2, t3) = (t * t, t * t * t);
            let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h10 = t3 - 2.0 * t2 + t;
            let h01 = -2.0 * t3 + 3.0 * t2;
            let h11 = t3 - t2;
            out.push(p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11);
        }
    }
    out.push(*points.last().unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_passes_through_its_points_and_is_smooth() {
        let pts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 3.0),
            Vec2::new(8.0, -1.0),
            Vec2::new(12.0, 2.0),
        ];
        let poly = spline_polyline(&pts, 8);
        assert_eq!(poly.len(), 8 * 3 + 1);
        for (i, p) in pts.iter().enumerate() {
            assert!(poly[8 * i].distance(*p) < 1e-12, "point {i}");
        }
        // Consecutive pieces never turn sharply: the curve is smooth.
        for w in poly.windows(3) {
            let (a, b) = (w[1] - w[0], w[2] - w[1]);
            let cos = a.dot(b) / (a.length() * b.length());
            assert!(cos > 0.8, "kink between {:?} and {:?}", w[0], w[2]);
        }
        // A straight row of points gives a straight line.
        let row = [
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(10.0, 0.0),
        ];
        assert!(spline_polyline(&row, 6).iter().all(|p| p.y.abs() < 1e-12));
        assert_eq!(spline_polyline(&row[..1], 6), row[..1].to_vec());
    }
}
