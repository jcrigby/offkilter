//! Randomised robustness tests for booleans. Shapes sit on an integer grid
//! so coplanar faces, shared edges and touching vertices are common.

use ok_brep::{boolean, extrude, BoolOp, Solid};
use ok_math::{Plane, Vec2};
use ok_sketch::{ProfileOptions, Sketch};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() % 1000) as f64 / 1000.0 < p
    }
}

fn plane_of(k: i64) -> Plane {
    match k {
        0 => Plane::XY,
        1 => Plane::XZ,
        _ => Plane::YZ,
    }
}

fn random_solid(rng: &mut Rng, id: u32) -> Solid {
    let plane = plane_of(rng.range(0, 2));
    let x0 = rng.range(-3, 3) as f64;
    let y0 = rng.range(-3, 3) as f64;
    let w = rng.range(1, 4) as f64;
    let h = rng.range(1, 4) as f64;
    let z0 = rng.range(-3, 3) as f64;
    let d = rng.range(1, 4) as f64;
    let mut s = Sketch::new();
    if rng.chance(0.3) {
        s.add_circle(Vec2::new(x0 + w / 2.0, y0 + h / 2.0), w.min(h) / 2.0);
    } else {
        s.add_rectangle(Vec2::new(x0, y0), Vec2::new(x0 + w, y0 + h));
    }
    let p = s.profiles(&ProfileOptions::default()).remove(0);
    extrude(&p, &plane, z0, z0 + d, id).unwrap()
}

fn run(seeds: std::ops::RangeInclusive<u64>) {
    let mut failures = Vec::new();
    let mut ops = 0;
    for seed in seeds {
        let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15));
        let mut body = random_solid(&mut rng, 1);
        for step in 0..6u32 {
            let tool = random_solid(&mut rng, 2 + step);
            let (op, name) = match rng.range(0, 2) {
                0 => (BoolOp::Union, "union"),
                1 => (BoolOp::Difference, "difference"),
                _ => (BoolOp::Intersection, "intersection"),
            };
            let (va, vb) = (body.volume(), tool.volume());
            ops += 1;
            match boolean(&body, &tool, op) {
                Ok(r) => {
                    if let Err(e) = r.validate() {
                        failures.push(format!(
                            "seed {seed} step {step} {name}: invalid result: {e}"
                        ));
                        break;
                    }
                    let v = r.volume();
                    let (lo, hi) = match op {
                        BoolOp::Union => (va.max(vb), va + vb),
                        BoolOp::Difference => ((va - vb).max(0.0), va),
                        BoolOp::Intersection => (0.0, va.min(vb)),
                    };
                    // Faceted cylinders make volumes slightly inexact; allow 1%.
                    let tol = 0.01 * (va + vb) + 1e-9;
                    if v < lo - tol || v > hi + tol {
                        failures.push(format!(
                            "seed {seed} step {step} {name}: volume {v} outside [{lo}, {hi}]"
                        ));
                        break;
                    }
                    if r.is_empty() {
                        body = tool; // start over from the tool
                    } else {
                        body = r;
                    }
                }
                Err(e) => {
                    failures.push(format!("seed {seed} step {step} {name}: {e}"));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {ops} operations failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn random_boolean_sequences_stay_closed() {
    run(1..=30);
}

/// Longer run: `cargo test -p ok-brep --test fuzz -- --ignored`.
#[test]
#[ignore]
fn random_boolean_sequences_stay_closed_long() {
    run(31..=400);
}
