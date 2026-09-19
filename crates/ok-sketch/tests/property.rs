//! Randomised checks on the solver and region extraction: many seeds, no
//! hand-picked geometry. Every seed must satisfy the stated property.

use ok_math::Vec2;
use ok_sketch::{Constraint, ProfileOptions, Sketch, SolveStatus};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// Shoelace area of a polygon.
fn area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum::<f64>() / 2.0
}

/// A random star-shaped polygon around a random centre.
fn random_polygon(rng: &mut Rng) -> Vec<Vec2> {
    let n = 3 + (rng.next() % 9) as usize;
    let c = Vec2::new(rng.range(-50.0, 50.0), rng.range(-50.0, 50.0));
    // Jittered but evenly spread angles: every gap is under a half turn,
    // so the polygon is star-shaped about `c` and therefore simple.
    let step = std::f64::consts::TAU / n as f64;
    let angles: Vec<f64> = (0..n)
        .map(|i| step * i as f64 + rng.range(-0.4, 0.4) * step)
        .collect();
    angles
        .iter()
        .map(|&t| {
            let r = rng.range(5.0, 30.0);
            c + Vec2::new(t.cos() * r, t.sin() * r)
        })
        .collect()
}

#[test]
fn every_simple_polygon_of_lines_is_one_region_with_its_shoelace_area() {
    for seed in 1..=150u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let pts = random_polygon(&mut rng);
        let mut s = Sketch::new();
        let mut ends = Vec::new();
        for i in 0..pts.len() {
            let (_, a, b) = s.add_line(pts[i], pts[(i + 1) % pts.len()]);
            ends.push((a, b));
        }
        for i in 0..pts.len() {
            let (_, e) = ends[i];
            let (a, _) = ends[(i + 1) % pts.len()];
            s.add_constraint(Constraint::Coincident { a: e, b: a });
        }
        let profiles = s.profiles(&ProfileOptions::default());
        assert_eq!(profiles.len(), 1, "seed {seed}: {} regions", profiles.len());
        let expected = area(&pts).abs();
        let got = profiles[0].area().abs();
        assert!(
            (got - expected).abs() < 1e-6 * expected.max(1.0),
            "seed {seed}: area {got} vs {expected}"
        );
    }
}

#[test]
fn random_dimensioned_rectangles_solve_to_their_dimensions() {
    for seed in 1..=150u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD1B54A32D192ED03) | 1);
        let mut s = Sketch::new();
        let a = Vec2::new(rng.range(-40.0, 40.0), rng.range(-40.0, 40.0));
        let b = a + Vec2::new(rng.range(2.0, 40.0), rng.range(2.0, 40.0));
        let lines = s.add_rectangle(a, b);
        // Ask for a different width and height than drawn, and pin a corner.
        let (w, h) = (rng.range(1.0, 60.0), rng.range(1.0, 60.0));
        s.add_constraint(Constraint::Length {
            line: lines[0],
            value: w,
        });
        s.add_constraint(Constraint::Length {
            line: lines[1],
            value: h,
        });
        let (corner, _) = s.line(lines[0]).unwrap();
        s.add_constraint(Constraint::Fixed { point: corner });
        let res = s.solve();
        assert_eq!(
            res.status,
            SolveStatus::FullyConstrained,
            "seed {seed}: {res:?}"
        );
        assert!(
            res.max_residual < 1e-6,
            "seed {seed}: residual {}",
            res.max_residual
        );
        let profiles = s.profiles(&ProfileOptions::default());
        assert_eq!(profiles.len(), 1, "seed {seed}");
        let got = profiles[0].area().abs();
        assert!(
            (got - w * h).abs() < 1e-5 * (w * h),
            "seed {seed}: area {got} vs {}",
            w * h
        );
    }
}

/// A grid of dimensioned, chained rectangles: `OK_BENCH_CELLS` per side
/// (default 8), printed with its solve time. Run with `--ignored --nocapture`.
#[test]
#[ignore]
fn solve_time_for_a_grid_of_dimensioned_rectangles() {
    let cells: usize = std::env::var("OK_BENCH_CELLS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let mut s = Sketch::new();
    let mut corners = Vec::new();
    for i in 0..cells {
        for j in 0..cells {
            let a = Vec2::new(i as f64 * 10.3, j as f64 * 10.7);
            let lines = s.add_rectangle(a, a + Vec2::new(9.0, 9.0));
            s.add_constraint(Constraint::Length {
                line: lines[0],
                value: 10.0,
            });
            s.add_constraint(Constraint::Length {
                line: lines[1],
                value: 10.0,
            });
            let (corner, _) = s.line(lines[0]).unwrap();
            corners.push(corner);
        }
    }
    // Chain: each rectangle's first corner sits 10 right (or up) of the previous.
    for w in corners.windows(2) {
        s.add_constraint(Constraint::Distance {
            a: w[0],
            b: w[1],
            value: 10.0,
        });
    }
    s.add_constraint(Constraint::Fixed { point: corners[0] });
    let t = std::time::Instant::now();
    let res = s.solve();
    println!(
        "{} params, {} equations: {:?} in {} iterations, {:.1} ms",
        res.parameters,
        res.equations,
        res.status,
        res.iterations,
        t.elapsed().as_secs_f64() * 1e3
    );
    assert!(res.max_residual < 1e-6, "{res:?}");
}
