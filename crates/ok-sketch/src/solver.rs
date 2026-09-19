//! Levenberg–Marquardt constraint solver.
//!
//! Every non-fixed point contributes two parameters (x, y) and every circle
//! one (radius). Constraints are written as residual functions that are zero
//! when satisfied. The solver minimises the sum of squared residuals with a
//! damped Gauss–Newton iteration using a numeric Jacobian. Under-constrained
//! sketches stay close to their starting geometry because the damping term
//! penalises movement, which matches what users expect when dragging.

use crate::{Constraint, Entity, EntityId, Sketch};
use ok_math::{tol, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

const MAX_ITERATIONS: usize = 100;
const RESIDUAL_TOL: f64 = 1e-10;
const STEP_TOL: f64 = 1e-14;
const RANK_TOL: f64 = 1e-8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveStatus {
    /// All constraints satisfied and no free degrees of freedom remain.
    FullyConstrained,
    /// All constraints satisfied; some geometry can still move.
    UnderConstrained,
    /// Constraints conflict or are redundant; the solver could not satisfy them all.
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub status: SolveStatus,
    pub iterations: usize,
    /// Largest absolute residual after solving.
    pub max_residual: f64,
    /// Remaining degrees of freedom (free parameters minus independent constraints).
    pub dof: usize,
    /// Number of scalar constraint equations.
    pub equations: usize,
    /// Number of free scalar parameters.
    pub parameters: usize,
}

impl SolveResult {
    pub fn ok(&self) -> bool {
        self.status != SolveStatus::Inconsistent
    }
}

/// Maps entities to slots in the parameter vector.
struct ParamMap {
    /// Point id -> index of x (y is index + 1).
    points: BTreeMap<EntityId, usize>,
    /// Circle id -> index of radius.
    radii: BTreeMap<EntityId, usize>,
    /// Positions of fixed points (not in the parameter vector).
    fixed: BTreeMap<EntityId, Vec2>,
    /// Radii of projected circles (not in the parameter vector).
    fixed_radii: BTreeMap<EntityId, f64>,
    len: usize,
}

impl ParamMap {
    fn build(sketch: &Sketch) -> ParamMap {
        let fixed_ids: HashSet<EntityId> = sketch
            .constraints()
            .filter_map(|(_, c)| match c {
                Constraint::Fixed { point } => Some(*point),
                _ => None,
            })
            .collect();
        let mut m = ParamMap {
            points: BTreeMap::new(),
            radii: BTreeMap::new(),
            fixed: BTreeMap::new(),
            fixed_radii: BTreeMap::new(),
            len: 0,
        };
        for (id, e) in sketch.entities() {
            match e {
                Entity::Point { pos } => {
                    if fixed_ids.contains(&id) || sketch.is_projected(id) {
                        m.fixed.insert(id, *pos);
                    } else {
                        m.points.insert(id, m.len);
                        m.len += 2;
                    }
                }
                Entity::Circle { radius, .. } => {
                    if sketch.is_projected(id) {
                        m.fixed_radii.insert(id, *radius);
                    } else {
                        m.radii.insert(id, m.len);
                        m.len += 1;
                    }
                }
                _ => {}
            }
        }
        m
    }

    fn initial(&self, sketch: &Sketch) -> Vec<f64> {
        let mut x = vec![0.0; self.len];
        for (id, e) in sketch.entities() {
            match e {
                Entity::Point { pos } => {
                    if let Some(&i) = self.points.get(&id) {
                        x[i] = pos.x;
                        x[i + 1] = pos.y;
                    }
                }
                Entity::Circle { radius, .. } => {
                    if let Some(&i) = self.radii.get(&id) {
                        x[i] = *radius;
                    }
                }
                _ => {}
            }
        }
        x
    }

    fn write_back(&self, sketch: &mut Sketch, x: &[f64]) {
        for (id, &i) in &self.points {
            sketch.set_point(*id, Vec2::new(x[i], x[i + 1]));
        }
        for (id, &i) in &self.radii {
            sketch.set_radius(*id, x[i]);
        }
    }
}

/// Evaluates geometry for a given parameter vector.
struct Eval<'a> {
    sketch: &'a Sketch,
    map: &'a ParamMap,
    x: &'a [f64],
}

impl<'a> Eval<'a> {
    fn point(&self, id: EntityId) -> Option<Vec2> {
        if let Some(&i) = self.map.points.get(&id) {
            return Some(Vec2::new(self.x[i], self.x[i + 1]));
        }
        self.map.fixed.get(&id).copied()
    }

    /// Direction vector (end - start) of a line.
    fn line_dir(&self, id: EntityId) -> Option<Vec2> {
        let (s, e) = self.line_points(id)?;
        Some(e - s)
    }

    fn line_points(&self, id: EntityId) -> Option<(Vec2, Vec2)> {
        match self.sketch.entity(id)? {
            Entity::Line { start, end } => Some((self.point(*start)?, self.point(*end)?)),
            _ => None,
        }
    }

    /// Centre and radius of a circle or arc.
    fn circular(&self, id: EntityId) -> Option<(Vec2, f64)> {
        match self.sketch.entity(id)? {
            Entity::Circle { center, .. } => {
                let c = self.point(*center)?;
                let r = match self.map.radii.get(&id) {
                    Some(&i) => self.x[i],
                    None => *self.map.fixed_radii.get(&id)?,
                };
                Some((c, r))
            }
            Entity::Arc { center, start, .. } => {
                let c = self.point(*center)?;
                let s = self.point(*start)?;
                Some((c, s.distance(c)))
            }
            _ => None,
        }
    }

    /// Length of a line or radius of a circle/arc, for `Equal`.
    fn size(&self, id: EntityId) -> Option<f64> {
        match self.sketch.entity(id)? {
            Entity::Line { .. } => Some(self.line_dir(id)?.length()),
            Entity::Circle { .. } | Entity::Arc { .. } => Some(self.circular(id)?.1),
            _ => None,
        }
    }

    fn push_constraint(&self, c: &Constraint, out: &mut Vec<f64>) {
        use Constraint::*;
        // Any constraint referencing missing/mistyped geometry contributes a
        // zero residual: it is silently inert rather than failing the solve.
        let mut r = |v: Option<f64>| out.push(v.unwrap_or(0.0));
        match c {
            Coincident { a, b } => {
                let d = (|| Some(self.point(*a)? - self.point(*b)?))();
                r(d.map(|d| d.x));
                r(d.map(|d| d.y));
            }
            Fixed { .. } => {}
            Horizontal { line } => r(self.line_dir(*line).map(|d| d.y)),
            Vertical { line } => r(self.line_dir(*line).map(|d| d.x)),
            Distance { a, b, value } => {
                r((|| Some(self.point(*a)?.distance(self.point(*b)?) - value))());
            }
            HorizontalDistance { a, b, value } => {
                r((|| Some(self.point(*b)?.x - self.point(*a)?.x - value))());
            }
            VerticalDistance { a, b, value } => {
                r((|| Some(self.point(*b)?.y - self.point(*a)?.y - value))());
            }
            Length { line, value } => r(self.line_dir(*line).map(|d| d.length() - value)),
            Radius { entity, value } => r(self.circular(*entity).map(|(_, rad)| rad - value)),
            Diameter { entity, value } => {
                r(self.circular(*entity).map(|(_, rad)| 2.0 * rad - value))
            }
            Equal { a, b } => r((|| Some(self.size(*a)? - self.size(*b)?))()),
            Parallel { a, b } => {
                r((|| {
                    let (da, db) = (self.line_dir(*a)?, self.line_dir(*b)?);
                    Some(da.cross(db) / (da.length() * db.length()).max(tol::LINEAR))
                })());
            }
            Perpendicular { a, b } => {
                r((|| {
                    let (da, db) = (self.line_dir(*a)?, self.line_dir(*b)?);
                    Some(da.dot(db) / (da.length() * db.length()).max(tol::LINEAR))
                })());
            }
            Angle { a, b, value } => {
                r((|| {
                    let (da, db) = (self.line_dir(*a)?, self.line_dir(*b)?);
                    let ang = da.cross(db).atan2(da.dot(db));
                    let target = value.to_radians();
                    // Wrap the difference into (-pi, pi] so the residual is smooth.
                    let mut diff = ang - target;
                    while diff > std::f64::consts::PI {
                        diff -= 2.0 * std::f64::consts::PI;
                    }
                    while diff <= -std::f64::consts::PI {
                        diff += 2.0 * std::f64::consts::PI;
                    }
                    Some(diff)
                })());
            }
            PointOnLine { point, line } => {
                r((|| {
                    let p = self.point(*point)?;
                    let (s, e) = self.line_points(*line)?;
                    let d = e - s;
                    Some((p - s).cross(d) / d.length().max(tol::LINEAR))
                })());
            }
            PointOnCircle { point, entity } => {
                r((|| {
                    let p = self.point(*point)?;
                    let (c, rad) = self.circular(*entity)?;
                    Some(p.distance(c) - rad)
                })());
            }
            Midpoint { point, line } => {
                let d = (|| {
                    let p = self.point(*point)?;
                    let (s, e) = self.line_points(*line)?;
                    Some(p - (s + e) * 0.5)
                })();
                r(d.map(|d| d.x));
                r(d.map(|d| d.y));
            }
            Symmetric { a, b, line } => {
                // The midpoint lies on the line and a-b is perpendicular to it.
                let v = (|| {
                    let (pa, pb) = (self.point(*a)?, self.point(*b)?);
                    let (s, e) = self.line_points(*line)?;
                    let d = e - s;
                    let len = d.length().max(tol::LINEAR);
                    let mid = (pa + pb) * 0.5;
                    Some(((mid - s).cross(d) / len, (pb - pa).dot(d) / len))
                })();
                r(v.map(|v| v.0));
                r(v.map(|v| v.1));
            }
            Rotated {
                a,
                b,
                center,
                value,
            } => {
                let v = (|| {
                    let (pa, pb, c) = (self.point(*a)?, self.point(*b)?, self.point(*center)?);
                    let (sn, cs) = value.to_radians().sin_cos();
                    let d = pa - c;
                    let turned = Vec2::new(d.x * cs - d.y * sn, d.x * sn + d.y * cs);
                    let diff = pb - c - turned;
                    Some((diff.x, diff.y))
                })();
                r(v.map(|v| v.0));
                r(v.map(|v| v.1));
            }
            Tangent { line, entity } => {
                r((|| {
                    let (s, e) = self.line_points(*line)?;
                    let (c, rad) = self.circular(*entity)?;
                    let d = e - s;
                    let len = d.length().max(tol::LINEAR);
                    // When an endpoint sits on the circle (a slot's line
                    // meeting its arc) the distance form is at a maximum
                    // there and has no gradient; the radius must then be
                    // perpendicular to the line instead.
                    let near = 1e-6 * rad.max(1.0);
                    let at_end = [s, e]
                        .into_iter()
                        .find(|p| (p.distance(c) - rad).abs() <= near);
                    Some(match at_end {
                        Some(p) => (p - c).dot(d) / len,
                        None => ((c - s).cross(d) / len).abs() - rad,
                    })
                })());
            }
        }
    }

    fn residuals(&self) -> Vec<f64> {
        let mut out = Vec::new();
        for (_, c) in self.sketch.constraints() {
            self.push_constraint(c, &mut out);
        }
        // Implicit arc constraint: start and end are equidistant from centre.
        for (_, e) in self.sketch.entities() {
            if let Entity::Arc { center, start, end } = e {
                let v = (|| {
                    let c = self.point(*center)?;
                    Some(self.point(*start)?.distance(c) - self.point(*end)?.distance(c))
                })();
                out.push(v.unwrap_or(0.0));
            }
        }
        out
    }
}

fn residuals(sketch: &Sketch, map: &ParamMap, x: &[f64]) -> Vec<f64> {
    Eval { sketch, map, x }.residuals()
}

/// Numeric Jacobian (m x n, row-major) by central differences.
fn jacobian(sketch: &Sketch, map: &ParamMap, x: &[f64], m: usize) -> Vec<f64> {
    let n = x.len();
    let mut j = vec![0.0; m * n];
    let mut xp = x.to_vec();
    for col in 0..n {
        let h = 1e-6 * x[col].abs().max(1.0);
        xp[col] = x[col] + h;
        let rp = residuals(sketch, map, &xp);
        xp[col] = x[col] - h;
        let rm = residuals(sketch, map, &xp);
        xp[col] = x[col];
        for row in 0..m {
            j[row * n + col] = (rp[row] - rm[row]) / (2.0 * h);
        }
    }
    j
}

/// Solves `a x = b` for a dense n x n matrix (row-major) by Gaussian
/// elimination with partial pivoting. Returns `None` if singular.
fn solve_dense(mut a: Vec<f64>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for k in 0..n {
        let mut p = k;
        for i in k + 1..n {
            if a[i * n + k].abs() > a[p * n + k].abs() {
                p = i;
            }
        }
        if a[p * n + k].abs() < 1e-300 {
            return None;
        }
        if p != k {
            for c in 0..n {
                a.swap(k * n + c, p * n + c);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            if f != 0.0 {
                for c in k..n {
                    a[i * n + c] -= f * a[k * n + c];
                }
                b[i] -= f * b[k];
            }
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for c in i + 1..n {
            s -= a[i * n + c] * x[c];
        }
        x[i] = s / a[i * n + i];
    }
    Some(x)
}

/// Rank of an m x n row-major matrix by modified Gram–Schmidt on rows.
fn rank(j: &[f64], m: usize, n: usize) -> usize {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for row in 0..m {
        let mut v: Vec<f64> = j[row * n..(row + 1) * n].to_vec();
        let norm0 = v.iter().map(|a| a * a).sum::<f64>().sqrt();
        if norm0 < RANK_TOL {
            continue;
        }
        for b in &basis {
            let d: f64 = v.iter().zip(b).map(|(a, b)| a * b).sum();
            for (vi, bi) in v.iter_mut().zip(b) {
                *vi -= d * bi;
            }
        }
        let norm: f64 = v.iter().map(|a| a * a).sum::<f64>().sqrt();
        if norm > RANK_TOL * norm0.max(1.0) {
            for vi in v.iter_mut() {
                *vi /= norm;
            }
            basis.push(v);
        }
    }
    basis.len()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, r| m.max(r.abs()))
}

impl Sketch {
    /// Solves the sketch constraints in place and reports the outcome.
    pub fn solve(&mut self) -> SolveResult {
        let map = ParamMap::build(self);
        let n = map.len;
        let mut x = map.initial(self);
        let mut r = residuals(self, &map, &x);
        let m = r.len();

        let mut lambda = 1e-3;
        let mut iterations = 0;
        let mut cost = r.iter().map(|v| v * v).sum::<f64>();

        if n > 0 && m > 0 {
            while iterations < MAX_ITERATIONS && max_abs(&r) > RESIDUAL_TOL {
                iterations += 1;
                let j = jacobian(self, &map, &x, m);
                // Normal equations: (JᵀJ + λ diag(JᵀJ + I)) δ = -Jᵀ r
                let mut jtj = vec![0.0; n * n];
                let mut jtr = vec![0.0; n];
                for row in 0..m {
                    for a in 0..n {
                        let ja = j[row * n + a];
                        if ja == 0.0 {
                            continue;
                        }
                        jtr[a] -= ja * r[row];
                        for b in 0..n {
                            jtj[a * n + b] += ja * j[row * n + b];
                        }
                    }
                }
                let mut accepted = false;
                for _ in 0..12 {
                    let mut a = jtj.clone();
                    for d in 0..n {
                        a[d * n + d] += lambda * (jtj[d * n + d] + 1.0);
                    }
                    let Some(delta) = solve_dense(a, jtr.clone()) else {
                        lambda *= 10.0;
                        continue;
                    };
                    let xn: Vec<f64> = x.iter().zip(&delta).map(|(a, d)| a + d).collect();
                    let rn = residuals(self, &map, &xn);
                    let cn = rn.iter().map(|v| v * v).sum::<f64>();
                    if cn < cost {
                        let step = max_abs(&delta);
                        x = xn;
                        r = rn;
                        cost = cn;
                        lambda = (lambda * 0.3).max(1e-12);
                        accepted = true;
                        if step < STEP_TOL {
                            iterations = MAX_ITERATIONS; // converged as far as we can go
                        }
                        break;
                    }
                    lambda *= 10.0;
                }
                if !accepted {
                    break;
                }
            }
        }

        let max_residual = max_abs(&r);
        let converged = max_residual <= 1e-7;
        let rk = if n > 0 && m > 0 {
            rank(&jacobian(self, &map, &x, m), m, n)
        } else {
            0
        };
        let dof = n.saturating_sub(rk);

        map.write_back(self, &x);

        let status = if !converged {
            SolveStatus::Inconsistent
        } else if dof == 0 {
            SolveStatus::FullyConstrained
        } else {
            SolveStatus::UnderConstrained
        };
        SolveResult {
            status,
            iterations,
            max_residual,
            dof,
            equations: m,
            parameters: n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_with_dimensions_is_fully_constrained() {
        let mut s = Sketch::new();
        let [bottom, right, _top, _left] =
            s.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(3.0, 2.0));
        let (bl, _) = s.line(bottom).unwrap();
        s.add_constraint(Constraint::Fixed { point: bl });
        s.add_constraint(Constraint::Length {
            line: bottom,
            value: 10.0,
        });
        s.add_constraint(Constraint::Length {
            line: right,
            value: 4.0,
        });
        let res = s.solve();
        assert_eq!(res.status, SolveStatus::FullyConstrained, "{res:?}");
        let (_, br) = s.line(bottom).unwrap();
        assert!(s.point(br).unwrap().approx_eq(Vec2::new(10.0, 0.0)));
        let (_, tr) = s.line(right).unwrap();
        assert!((s.point(tr).unwrap().y - 4.0).abs() < 1e-6);
    }

    #[test]
    fn under_constrained_rectangle_reports_dof() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(3.0, 2.0));
        let res = s.solve();
        assert_eq!(res.status, SolveStatus::UnderConstrained);
        // Position (2) + width (1) + height (1) remain free.
        assert_eq!(res.dof, 4);
    }

    #[test]
    fn conflicting_constraints_are_inconsistent() {
        let mut s = Sketch::new();
        let (l, _, _) = s.add_line(Vec2::ZERO, Vec2::new(1.0, 0.0));
        s.add_constraint(Constraint::Length {
            line: l,
            value: 5.0,
        });
        s.add_constraint(Constraint::Length {
            line: l,
            value: 7.0,
        });
        assert_eq!(s.solve().status, SolveStatus::Inconsistent);
    }

    #[test]
    fn circle_radius_and_tangent_line() {
        let mut s = Sketch::new();
        let (c, center) = s.add_circle(Vec2::new(0.0, 0.0), 1.0);
        s.add_constraint(Constraint::Fixed { point: center });
        s.add_constraint(Constraint::Radius {
            entity: c,
            value: 5.0,
        });
        let (l, a, _b) = s.add_line(Vec2::new(-3.0, 4.0), Vec2::new(3.0, 4.0));
        s.add_constraint(Constraint::Horizontal { line: l });
        s.add_constraint(Constraint::Tangent { line: l, entity: c });
        let res = s.solve();
        assert!(res.ok(), "{res:?}");
        let (pa, pb) = (s.point(a).unwrap(), s.point(_b).unwrap());
        assert!((pa.y - pb.y).abs() < 1e-6);
        assert!((pa.y.abs() - 5.0).abs() < 1e-6, "{pa:?}");
    }

    #[test]
    fn angle_constraint() {
        let mut s = Sketch::new();
        let (a, a0, a1) = s.add_line(Vec2::ZERO, Vec2::new(1.0, 0.0));
        let (b, b0, b1) = s.add_line(Vec2::ZERO, Vec2::new(1.0, 0.2));
        s.add_constraint(Constraint::Fixed { point: a0 });
        s.add_constraint(Constraint::Fixed { point: a1 });
        s.add_constraint(Constraint::Coincident { a: a0, b: b0 });
        s.add_constraint(Constraint::Length {
            line: b,
            value: 2.0,
        });
        s.add_constraint(Constraint::Angle { a, b, value: 30.0 });
        let res = s.solve();
        assert_eq!(res.status, SolveStatus::FullyConstrained, "{res:?}");
        let p = s.point(b1).unwrap();
        assert!((p.x - 2.0 * 30f64.to_radians().cos()).abs() < 1e-6);
        assert!((p.y - 2.0 * 30f64.to_radians().sin()).abs() < 1e-6);
    }
}
