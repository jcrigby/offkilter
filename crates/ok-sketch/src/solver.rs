//! Levenberg–Marquardt constraint solver.
//!
//! Every non-fixed point contributes two parameters (x, y) and every circle
//! one (radius). Constraints are written as residual functions that are zero
//! when satisfied. The solver minimises the sum of squared residuals with a
//! damped Gauss–Newton iteration. The Jacobian is sparse: each constraint
//! only touches the few parameters of the entities it names, so its rows
//! are filled analytically for the common constraints and by central
//! differences over just those parameters for the rest. Under-constrained
//! sketches stay close to their starting geometry because the damping term
//! penalises movement, which matches what users expect when dragging.

use crate::{Constraint, Entity, EntityId, Sketch};
use ok_math::{tol, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

const MAX_ITERATIONS: usize = 100;
const RESIDUAL_TOL: f64 = 1e-10;
const STEP_TOL: f64 = 1e-14;
#[cfg(test)]
const RANK_TOL: f64 = 1e-8;
/// A normal-matrix pivot below this fraction of its original diagonal
/// entry counts as zero (singular values of J below about 1e-6 of the
/// column norm).
const PIVOT_TOL: f64 = 1e-12;

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

/// A Jacobian stored by rows of `(column, value)` pairs.
struct SparseJacobian {
    n: usize,
    rows: Vec<Vec<(usize, f64)>>,
}

impl SparseJacobian {
    #[cfg(test)]
    fn to_dense(&self) -> Vec<f64> {
        let mut j = vec![0.0; self.rows.len() * self.n];
        for (r, row) in self.rows.iter().enumerate() {
            for &(c, v) in row {
                j[r * self.n + c] += v;
            }
        }
        j
    }

    /// `JᵀJ` (dense n x n, row-major) and `-Jᵀ r`.
    fn normal_equations(&self, r: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.n;
        let mut jtj = vec![0.0; n * n];
        let mut jtr = vec![0.0; n];
        for (row, entries) in self.rows.iter().enumerate() {
            for &(a, ja) in entries {
                jtr[a] -= ja * r[row];
                for &(b, jb) in entries {
                    jtj[a * n + b] += ja * jb;
                }
            }
        }
        (jtj, jtr)
    }
}

/// Collects the partial derivatives of one residual row.
struct Row<'a> {
    map: &'a ParamMap,
    entries: Vec<(usize, f64)>,
}

impl Row<'_> {
    /// Adds `g` as the gradient with respect to point `id` (nothing for a fixed point).
    fn point(&mut self, id: EntityId, g: Vec2) {
        if let Some(&i) = self.map.points.get(&id) {
            self.entries.push((i, g.x));
            self.entries.push((i + 1, g.y));
        }
    }

    /// Adds `g` as the derivative with respect to the size of a line
    /// (its length), circle (its radius) or arc (its radius).
    fn size(&mut self, ev: &Eval, id: EntityId, g: f64) {
        match ev.sketch.entity(id) {
            Some(Entity::Line { start, end }) => {
                if let Some(d) = ev.line_dir(id) {
                    let u = d / d.length().max(tol::LINEAR);
                    self.point(*end, u * g);
                    self.point(*start, -u * g);
                }
            }
            Some(Entity::Circle { .. }) => {
                if let Some(&i) = self.map.radii.get(&id) {
                    self.entries.push((i, g));
                }
            }
            Some(Entity::Arc { center, start, .. }) => {
                if let (Some(c), Some(s)) = (ev.point(*center), ev.point(*start)) {
                    let d = s - c;
                    let u = d / d.length().max(tol::LINEAR);
                    self.point(*start, u * g);
                    self.point(*center, -u * g);
                }
            }
            _ => {}
        }
    }
}

impl Eval<'_> {
    /// Every parameter slot an entity depends on.
    fn slots_of(&self, id: EntityId, out: &mut Vec<usize>) {
        let mut point = |p: EntityId| {
            if let Some(&i) = self.map.points.get(&p) {
                out.push(i);
                out.push(i + 1);
            }
        };
        match self.sketch.entity(id) {
            Some(Entity::Point { .. }) => point(id),
            Some(Entity::Line { start, end }) => {
                point(*start);
                point(*end);
            }
            Some(Entity::Circle { center, .. }) => {
                point(*center);
                if let Some(&i) = self.map.radii.get(&id) {
                    out.push(i);
                }
            }
            Some(Entity::Arc { center, start, end }) => {
                point(*center);
                point(*start);
                point(*end);
            }
            None => {}
        }
    }

    /// Parameter slots a constraint depends on.
    fn constraint_slots(&self, c: &Constraint) -> Vec<usize> {
        use Constraint::*;
        let mut out = Vec::new();
        let ids: Vec<EntityId> = match c {
            Coincident { a, b } | Distance { a, b, .. } | Equal { a, b } => vec![*a, *b],
            HorizontalDistance { a, b, .. } | VerticalDistance { a, b, .. } => vec![*a, *b],
            Parallel { a, b } | Perpendicular { a, b } | Angle { a, b, .. } => vec![*a, *b],
            Fixed { .. } => vec![],
            Horizontal { line } | Vertical { line } | Length { line, .. } => vec![*line],
            Radius { entity, .. } | Diameter { entity, .. } => vec![*entity],
            PointOnLine { point, line } | Midpoint { point, line } => vec![*point, *line],
            PointOnCircle { point, entity } => vec![*point, *entity],
            Symmetric { a, b, line } => vec![*a, *b, *line],
            Rotated { a, b, center, .. } => vec![*a, *b, *center],
            Tangent { line, entity } => vec![*line, *entity],
        };
        for id in ids {
            self.slots_of(id, &mut out);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Analytic rows for the constraints with simple derivatives; `None`
    /// for the rest (they are differenced numerically).
    fn analytic_rows(&self, c: &Constraint) -> Option<Vec<Vec<(usize, f64)>>> {
        use Constraint::*;
        let row = || Row {
            map: self.map,
            entries: Vec::new(),
        };
        let rows = match c {
            Coincident { a, b } => {
                self.point(*a)?;
                self.point(*b)?;
                let (mut rx, mut ry) = (row(), row());
                rx.point(*a, Vec2::new(1.0, 0.0));
                rx.point(*b, Vec2::new(-1.0, 0.0));
                ry.point(*a, Vec2::new(0.0, 1.0));
                ry.point(*b, Vec2::new(0.0, -1.0));
                vec![rx.entries, ry.entries]
            }
            Fixed { .. } => vec![],
            Horizontal { line } | Vertical { line } => {
                self.line_points(*line)?;
                let (start, end) = match self.sketch.entity(*line)? {
                    Entity::Line { start, end } => (*start, *end),
                    _ => return None,
                };
                let g = if matches!(c, Horizontal { .. }) {
                    Vec2::new(0.0, 1.0)
                } else {
                    Vec2::new(1.0, 0.0)
                };
                let mut r = row();
                r.point(end, g);
                r.point(start, -g);
                vec![r.entries]
            }
            Distance { a, b, .. } => {
                let (pa, pb) = (self.point(*a)?, self.point(*b)?);
                let d = pa - pb;
                let u = d / d.length().max(tol::LINEAR);
                let mut r = row();
                r.point(*a, u);
                r.point(*b, -u);
                vec![r.entries]
            }
            HorizontalDistance { a, b, .. } | VerticalDistance { a, b, .. } => {
                self.point(*a)?;
                self.point(*b)?;
                let g = if matches!(c, HorizontalDistance { .. }) {
                    Vec2::new(1.0, 0.0)
                } else {
                    Vec2::new(0.0, 1.0)
                };
                let mut r = row();
                r.point(*b, g);
                r.point(*a, -g);
                vec![r.entries]
            }
            Length { line, .. } => {
                self.line_dir(*line)?;
                let mut r = row();
                r.size(self, *line, 1.0);
                vec![r.entries]
            }
            Radius { entity, .. } => {
                self.circular(*entity)?;
                let mut r = row();
                r.size(self, *entity, 1.0);
                vec![r.entries]
            }
            Diameter { entity, .. } => {
                self.circular(*entity)?;
                let mut r = row();
                r.size(self, *entity, 2.0);
                vec![r.entries]
            }
            Equal { a, b } => {
                self.size(*a)?;
                self.size(*b)?;
                let mut r = row();
                r.size(self, *a, 1.0);
                r.size(self, *b, -1.0);
                vec![r.entries]
            }
            PointOnCircle { point, entity } => {
                let p = self.point(*point)?;
                let (c0, _) = self.circular(*entity)?;
                let d = p - c0;
                let u = d / d.length().max(tol::LINEAR);
                let centre = match self.sketch.entity(*entity)? {
                    Entity::Circle { center, .. } | Entity::Arc { center, .. } => *center,
                    _ => return None,
                };
                let mut r = row();
                r.point(*point, u);
                r.point(centre, -u);
                r.size(self, *entity, -1.0);
                vec![r.entries]
            }
            Midpoint { point, line } => {
                self.point(*point)?;
                self.line_points(*line)?;
                let (start, end) = match self.sketch.entity(*line)? {
                    Entity::Line { start, end } => (*start, *end),
                    _ => return None,
                };
                let (mut rx, mut ry) = (row(), row());
                rx.point(*point, Vec2::new(1.0, 0.0));
                rx.point(start, Vec2::new(-0.5, 0.0));
                rx.point(end, Vec2::new(-0.5, 0.0));
                ry.point(*point, Vec2::new(0.0, 1.0));
                ry.point(start, Vec2::new(0.0, -0.5));
                ry.point(end, Vec2::new(0.0, -0.5));
                vec![rx.entries, ry.entries]
            }
            PointOnLine { point, line } => {
                // f = (p - s) × d / |d| with d = e - s.
                let p = self.point(*point)?;
                let (s, e) = self.line_points(*line)?;
                let (start, end) = match self.sketch.entity(*line)? {
                    Entity::Line { start, end } => (*start, *end),
                    _ => return None,
                };
                let d = e - s;
                let len = d.length().max(tol::LINEAR);
                let u = p - s;
                let cross = u.cross(d);
                let mut r = row();
                r.point(*point, Vec2::new(d.y, -d.x) / len);
                r.point(
                    end,
                    Vec2::new(-u.y, u.x) / len - d * (cross / (len * len * len)),
                );
                r.point(
                    start,
                    Vec2::new(u.y - d.y, d.x - u.x) / len + d * (cross / (len * len * len)),
                );
                vec![r.entries]
            }
            _ => return None,
        };
        Some(rows)
    }

    /// Rows of one constraint by central differences over its own slots.
    fn numeric_rows(&self, c: &Constraint, count: usize) -> Vec<Vec<(usize, f64)>> {
        let mut rows = vec![Vec::new(); count];
        let mut xp = self.x.to_vec();
        for col in self.constraint_slots(c) {
            let h = 1e-6 * self.x[col].abs().max(1.0);
            let mut rp = Vec::with_capacity(count);
            let mut rm = Vec::with_capacity(count);
            xp[col] = self.x[col] + h;
            Eval {
                sketch: self.sketch,
                map: self.map,
                x: &xp,
            }
            .push_constraint(c, &mut rp);
            xp[col] = self.x[col] - h;
            Eval {
                sketch: self.sketch,
                map: self.map,
                x: &xp,
            }
            .push_constraint(c, &mut rm);
            xp[col] = self.x[col];
            for (k, row) in rows.iter_mut().enumerate() {
                let v = (rp[k] - rm[k]) / (2.0 * h);
                if v != 0.0 {
                    row.push((col, v));
                }
            }
        }
        rows
    }

    /// The sparse Jacobian, rows in the order of `residuals`.
    fn jacobian(&self) -> SparseJacobian {
        let mut rows = Vec::new();
        for (_, c) in self.sketch.constraints() {
            let count = {
                let mut r = Vec::new();
                self.push_constraint(c, &mut r);
                r.len()
            };
            match self.analytic_rows(c) {
                Some(a) if a.len() == count => rows.extend(a),
                _ => rows.extend(self.numeric_rows(c, count)),
            }
        }
        for (_, e) in self.sketch.entities() {
            if let Entity::Arc { center, start, end } = e {
                let mut r = Row {
                    map: self.map,
                    entries: Vec::new(),
                };
                if let (Some(c), Some(s), Some(en)) =
                    (self.point(*center), self.point(*start), self.point(*end))
                {
                    let us = (s - c) / s.distance(c).max(tol::LINEAR);
                    let ue = (en - c) / en.distance(c).max(tol::LINEAR);
                    r.point(*start, us);
                    r.point(*end, -ue);
                    r.point(*center, ue - us);
                }
                rows.push(r.entries);
            }
        }
        SparseJacobian {
            n: self.x.len(),
            rows,
        }
    }
}

fn jacobian(sketch: &Sketch, map: &ParamMap, x: &[f64]) -> SparseJacobian {
    Eval { sketch, map, x }.jacobian()
}

/// Dense Jacobian (m x n, row-major) by central differences over every
/// parameter: the reference the sparse one is checked against.
#[cfg(test)]
fn dense_numeric_jacobian(sketch: &Sketch, map: &ParamMap, x: &[f64], m: usize) -> Vec<f64> {
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

/// Rank of a Jacobian from its normal matrix `JᵀJ` (dense n x n,
/// row-major, consumed): Gaussian elimination with diagonal pivots,
/// which needs no row swaps on a positive semidefinite matrix and so
/// keeps its sparsity. A pivot that has shrunk to noise relative to its
/// original diagonal entry marks a dependent parameter.
fn rank_from_normal(mut a: Vec<f64>, n: usize) -> usize {
    let diag: Vec<f64> = (0..n).map(|k| a[k * n + k]).collect();
    let mut rank = 0;
    for k in 0..n {
        let p = a[k * n + k];
        if diag[k] <= 0.0 || p <= PIVOT_TOL * diag[k] {
            continue;
        }
        rank += 1;
        for i in k + 1..n {
            let f = a[i * n + k] / p;
            if f != 0.0 {
                for c in k + 1..n {
                    a[i * n + c] -= f * a[k * n + c];
                }
            }
        }
    }
    rank
}

/// Rank of an m x n row-major matrix by modified Gram–Schmidt on rows:
/// the reference `rank_from_normal` is checked against.
#[cfg(test)]
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
                let j = jacobian(self, &map, &x);
                // Normal equations: (JᵀJ + λ diag(JᵀJ + I)) δ = -Jᵀ r
                let (jtj, jtr) = j.normal_equations(&r);
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
            let (jtj, _) = jacobian(self, &map, &x).normal_equations(&r);
            rank_from_normal(jtj, n)
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

    /// A sketch using every constraint type, deliberately unsolved.
    fn every_constraint() -> Sketch {
        let mut s = Sketch::new();
        let (l1, a1, b1) = s.add_line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 1.0));
        let (l2, a2, b2) = s.add_line(Vec2::new(10.5, 1.2), Vec2::new(11.0, 12.0));
        let (l3, _, b3) = s.add_line(Vec2::new(-3.0, 4.0), Vec2::new(4.0, 9.0));
        let (c1, cc1) = s.add_circle(Vec2::new(20.0, 5.0), 3.0);
        let (c2, _) = s.add_circle(Vec2::new(30.0, 6.0), 2.0);
        let (arc, ac, as_, ae) = s.add_arc(
            Vec2::new(0.0, 20.0),
            Vec2::new(4.0, 20.0),
            Vec2::new(0.0, 24.5),
        );
        let p = s.add_point(Vec2::new(5.0, 2.0));
        let q = s.add_point(Vec2::new(21.0, 9.0));
        s.add_constraint(Constraint::Coincident { a: b1, b: a2 });
        s.add_constraint(Constraint::Fixed { point: a1 });
        s.add_constraint(Constraint::Horizontal { line: l1 });
        s.add_constraint(Constraint::Vertical { line: l2 });
        s.add_constraint(Constraint::Distance {
            a: a1,
            b: b2,
            value: 15.0,
        });
        s.add_constraint(Constraint::HorizontalDistance {
            a: a1,
            b: b3,
            value: 4.0,
        });
        s.add_constraint(Constraint::VerticalDistance {
            a: a1,
            b: b3,
            value: 9.0,
        });
        s.add_constraint(Constraint::Length {
            line: l3,
            value: 8.0,
        });
        s.add_constraint(Constraint::Radius {
            entity: c1,
            value: 3.5,
        });
        s.add_constraint(Constraint::Diameter {
            entity: arc,
            value: 9.0,
        });
        s.add_constraint(Constraint::Equal { a: l1, b: l2 });
        s.add_constraint(Constraint::Equal { a: c1, b: arc });
        s.add_constraint(Constraint::Parallel { a: l1, b: l3 });
        s.add_constraint(Constraint::Perpendicular { a: l2, b: l3 });
        s.add_constraint(Constraint::Angle {
            a: l1,
            b: l2,
            value: 80.0,
        });
        s.add_constraint(Constraint::PointOnLine { point: p, line: l3 });
        s.add_constraint(Constraint::PointOnCircle {
            point: q,
            entity: c1,
        });
        s.add_constraint(Constraint::PointOnCircle {
            point: p,
            entity: arc,
        });
        s.add_constraint(Constraint::Midpoint { point: q, line: l2 });
        s.add_constraint(Constraint::Symmetric {
            a: p,
            b: q,
            line: l1,
        });
        s.add_constraint(Constraint::Rotated {
            a: as_,
            b: ae,
            center: ac,
            value: 70.0,
        });
        s.add_constraint(Constraint::Tangent {
            line: l3,
            entity: c2,
        });
        s.add_constraint(Constraint::Tangent {
            line: l1,
            entity: c1,
        });
        let _ = (cc1, b3);
        s
    }

    #[test]
    fn sparse_jacobian_matches_dense_central_differences() {
        let s = every_constraint();
        let map = ParamMap::build(&s);
        let x = map.initial(&s);
        let m = residuals(&s, &map, &x).len();
        let sparse = jacobian(&s, &map, &x);
        assert_eq!(sparse.rows.len(), m);
        let dense = sparse.to_dense();
        let reference = dense_numeric_jacobian(&s, &map, &x, m);
        let n = x.len();
        for row in 0..m {
            for col in 0..n {
                let (a, b) = (dense[row * n + col], reference[row * n + col]);
                assert!(
                    (a - b).abs() < 1e-6 * b.abs().max(1.0),
                    "row {row} col {col}: sparse {a} vs dense {b}"
                );
            }
        }
        // Every row is sparse: nothing touches more than four points and a radius.
        assert!(sparse.rows.iter().all(|r| r.len() <= 9));
        // Both rank computations agree, here and after solving.
        let (jtj, _) = sparse.normal_equations(&residuals(&s, &map, &x));
        assert_eq!(rank_from_normal(jtj, n), rank(&reference, m, n));
        let mut solved = s.clone();
        solved.solve();
        let map = ParamMap::build(&solved);
        let x = map.initial(&solved);
        let sparse = jacobian(&solved, &map, &x);
        let (jtj, _) = sparse.normal_equations(&residuals(&solved, &map, &x));
        assert_eq!(rank_from_normal(jtj, n), rank(&sparse.to_dense(), m, n));
    }

    #[test]
    fn dependent_constraints_do_not_count_towards_rank() {
        // A rectangle whose fourth side is both horizontal (twice) and
        // parallel to the first: the duplicates add equations, not rank.
        let mut s = Sketch::new();
        let lines = s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 5.0));
        s.add_constraint(Constraint::Horizontal { line: lines[2] });
        s.add_constraint(Constraint::Horizontal { line: lines[2] });
        s.add_constraint(Constraint::Parallel {
            a: lines[0],
            b: lines[2],
        });
        let map = ParamMap::build(&s);
        let x = map.initial(&s);
        let r = residuals(&s, &map, &x);
        let j = jacobian(&s, &map, &x);
        let (jtj, _) = j.normal_equations(&r);
        let expected = rank(&j.to_dense(), r.len(), x.len());
        assert_eq!(rank_from_normal(jtj, x.len()), expected);
        assert!(expected < r.len(), "{expected} of {} rows", r.len());
    }

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
