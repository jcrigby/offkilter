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

// ---------------------------------------------------------------------------
// General position and near-tangent geometry: rotated tools, fractional
// offsets, and tiny epsilons that put faces a hair from coincident.

use ok_brep::Transform;
use ok_math::Vec3;

fn frac(rng: &mut Rng, lo: f64, hi: f64) -> f64 {
    lo + (rng.next() % 10_000) as f64 / 10_000.0 * (hi - lo)
}

fn random_general_solid(rng: &mut Rng, id: u32) -> Solid {
    let mut s = Sketch::new();
    let w = frac(rng, 0.5, 4.0);
    let h = frac(rng, 0.5, 4.0);
    match rng.range(0, 3) {
        0 => {
            s.add_circle(Vec2::new(w / 2.0, h / 2.0), w.min(h) / 2.0);
        }
        1 => {
            // A triangle: oblique faces.
            let apex = Vec2::new(frac(rng, 0.0, w), h);
            s.add_line(Vec2::ZERO, Vec2::new(w, 0.0));
            s.add_line(Vec2::new(w, 0.0), apex);
            s.add_line(apex, Vec2::ZERO);
        }
        _ => {
            s.add_rectangle(Vec2::ZERO, Vec2::new(w, h));
        }
    }
    let p = s.profiles(&ProfileOptions::default()).remove(0);
    let d = frac(rng, 0.5, 4.0);
    let solid = extrude(&p, &Plane::XY, 0.0, d, id).unwrap();
    // Random rigid placement; sometimes axis-aligned so coplanar cases stay common.
    let xf = if rng.chance(0.4) {
        Transform::translation(Vec3::new(
            rng.range(-2, 2) as f64,
            rng.range(-2, 2) as f64,
            rng.range(-2, 2) as f64,
        ))
    } else {
        let axis = Vec3::new(
            frac(rng, -1.0, 1.0),
            frac(rng, -1.0, 1.0),
            frac(rng, -1.0, 1.0),
        )
        .normalized()
        .unwrap_or(Vec3::Z);
        let angle = frac(rng, 0.0, std::f64::consts::TAU);
        let rot = Transform::rotation(Vec3::ZERO, axis, angle);
        let t = Vec3::new(
            frac(rng, -2.0, 2.0),
            frac(rng, -2.0, 2.0),
            frac(rng, -2.0, 2.0),
        );
        Transform { m: rot.m, t }
    };
    let mut out = solid.transformed(&xf);
    // Occasionally nudge by a tiny epsilon so faces land almost, but not
    // exactly, on each other.
    if rng.chance(0.3) {
        // OK_FUZZ_EPS="1e-3,1e-8" overrides the nudge sizes for experiments.
        let choices: Vec<f64> = std::env::var("OK_FUZZ_EPS")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .filter(|v: &Vec<f64>| !v.is_empty())
            .unwrap_or_else(|| vec![1e-7, 1e-9, 1e-5]);
        let eps = choices[(rng.next() % choices.len() as u64) as usize];
        out = out.transformed(&Transform::translation(Vec3::new(eps, -eps, eps)));
    }
    out
}

fn run_general(seeds: std::ops::RangeInclusive<u64>) {
    let mut failures = Vec::new();
    let mut ops = 0;
    for seed in seeds {
        let mut rng = Rng(seed.wrapping_mul(0xD1B54A32D192ED03) | 1);
        let mut body = random_general_solid(&mut rng, 1);
        for step in 0..5u32 {
            let tool = random_general_solid(&mut rng, 2 + step);
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
                    let tol = 0.01 * (va + vb) + 1e-9;
                    if v < lo - tol || v > hi + tol {
                        failures.push(format!(
                            "seed {seed} step {step} {name}: volume {v} outside [{lo}, {hi}]"
                        ));
                        break;
                    }
                    body = if r.is_empty() { tool } else { r };
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
fn general_position_boolean_sequences_stay_closed() {
    run_general(1..=40);
}

/// Longer run: `cargo test -p ok-brep --test fuzz -- --ignored`.
#[test]
#[ignore]
fn general_position_boolean_sequences_stay_closed_long() {
    run_general(41..=400);
}

/// Reruns one general-position seed: `OK_FUZZ_SEED=186 cargo test -p ok-brep --test fuzz general_position_seed -- --ignored --nocapture`.
#[test]
#[ignore]
fn general_position_seed() {
    let seed: u64 = std::env::var("OK_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    run_general(seed..=seed);
}
