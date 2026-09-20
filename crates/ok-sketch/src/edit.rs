//! Geometric editing of a sketch: trim, offset and mirror.
//!
//! These build ordinary entities and constraints, so the result stays
//! editable and solvable like anything drawn by hand:
//!
//! - `trim` removes the piece of a curve between its nearest intersections
//!   (with any other entity, construction included), keeping the original
//!   endpoints and tying the new cut points to what cut them;
//! - `offset` copies a chain of lines and arcs at a distance, mitring
//!   line corners, trimming overlaps and rounding gaps, with arcs kept
//!   concentric by sharing their centre point;
//! - `mirror` copies entities across a line with `Symmetric` constraints
//!   on every point pair, so the copy follows the original.

use crate::{Constraint, ConstraintId, Entity, EntityId, Sketch, SketchError};
use ok_math::Vec2;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Points closer than this are the same place.
const TOL: f64 = 1e-6;

/// Evaluated curve geometry.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Line {
        a: Vec2,
        b: Vec2,
    },
    /// Counter-clockwise from angle `a0` over `sweep` (0, 2π].
    Arc {
        c: Vec2,
        r: f64,
        a0: f64,
        sweep: f64,
    },
    Circle {
        c: Vec2,
        r: f64,
    },
}

impl Shape {
    /// Parameter in [0, 1] of the point on the shape nearest `p`.
    fn param(&self, p: Vec2) -> f64 {
        match *self {
            Shape::Line { a, b } => {
                let d = b - a;
                let l2 = d.length_squared();
                if l2 <= TOL * TOL {
                    0.0
                } else {
                    ((p - a).dot(d) / l2).clamp(0.0, 1.0)
                }
            }
            Shape::Arc { c, a0, sweep, .. } => {
                let t = ((p - c).angle() - a0).rem_euclid(TAU) / sweep;
                if t <= 1.0 {
                    t
                } else if (t - 1.0) * sweep < (TAU - t * sweep) {
                    1.0
                } else {
                    0.0
                }
            }
            Shape::Circle { c, .. } => (p - c).angle().rem_euclid(TAU) / TAU,
        }
    }

    fn at(&self, t: f64) -> Vec2 {
        match *self {
            Shape::Line { a, b } => a.lerp(b, t),
            Shape::Arc { c, r, a0, sweep } => c + Vec2::from_angle(a0 + sweep * t) * r,
            Shape::Circle { c, r } => c + Vec2::from_angle(TAU * t) * r,
        }
    }

    /// Whether `p` lies on the shape (within `TOL`), with its parameter.
    fn contains(&self, p: Vec2) -> Option<f64> {
        let t = self.param(p);
        (self.at(t).distance(p) <= TOL).then_some(t)
    }

    /// Centre and radius for round shapes.
    fn circle(&self) -> Option<(Vec2, f64)> {
        match *self {
            Shape::Arc { c, r, .. } | Shape::Circle { c, r } => Some((c, r)),
            Shape::Line { .. } => None,
        }
    }

    /// Intersections of the underlying infinite lines / full circles.
    fn intersections_unbounded(&self, o: &Shape) -> Vec<Vec2> {
        match (self.circle(), o.circle()) {
            (None, None) => {
                let (Shape::Line { a: a1, b: b1 }, Shape::Line { a: a2, b: b2 }) = (self, o) else {
                    unreachable!()
                };
                let (d1, d2) = (*b1 - *a1, *b2 - *a2);
                let den = d1.cross(d2);
                if den.abs() <= 1e-12 {
                    return Vec::new();
                }
                let t = (*a2 - *a1).cross(d2) / den;
                vec![*a1 + d1 * t]
            }
            (None, Some((c, r))) | (Some((c, r)), None) => {
                let (a, b) = match (self, o) {
                    (Shape::Line { a, b }, _) | (_, Shape::Line { a, b }) => (a, b),
                    _ => unreachable!(),
                };
                let d = *b - *a;
                let f = *a - c;
                let qa = d.dot(d);
                let qb = 2.0 * f.dot(d);
                let qc = f.dot(f) - r * r;
                let disc = qb * qb - 4.0 * qa * qc;
                if disc < 0.0 || qa <= 1e-18 {
                    return Vec::new();
                }
                let s = disc.sqrt();
                let mut out = vec![*a + d * ((-qb - s) / (2.0 * qa))];
                if s > 1e-12 {
                    out.push(*a + d * ((-qb + s) / (2.0 * qa)));
                }
                out
            }
            (Some((c1, r1)), Some((c2, r2))) => {
                let d = c2.distance(c1);
                if d <= 1e-12 || d > r1 + r2 + TOL || d < (r1 - r2).abs() - TOL {
                    return Vec::new();
                }
                let x = (d * d - r2 * r2 + r1 * r1) / (2.0 * d);
                let h = (r1 * r1 - x * x).max(0.0).sqrt();
                let dir = (c2 - c1) / d;
                let base = c1 + dir * x;
                let mut out = vec![base + dir.perp() * h];
                if h > 1e-12 {
                    out.push(base - dir.perp() * h);
                }
                out
            }
        }
    }

    /// Intersections lying on both shapes.
    fn intersections(&self, o: &Shape) -> Vec<Vec2> {
        self.intersections_unbounded(o)
            .into_iter()
            .filter(|p| self.contains(*p).is_some() && o.contains(*p).is_some())
            .collect()
    }
}

fn shape_of(sketch: &Sketch, id: EntityId) -> Result<Shape, SketchError> {
    match sketch.entity(id).ok_or(SketchError::UnknownEntity(id))? {
        Entity::Line { start, end } => Ok(Shape::Line {
            a: sketch.point(*start)?,
            b: sketch.point(*end)?,
        }),
        Entity::Arc { center, start, end } => {
            let c = sketch.point(*center)?;
            let (s, e) = (sketch.point(*start)?, sketch.point(*end)?);
            let r = 0.5 * (s.distance(c) + e.distance(c));
            let a0 = (s - c).angle();
            let mut sweep = ((e - c).angle() - a0).rem_euclid(TAU);
            if sweep <= 1e-12 {
                sweep = TAU;
            }
            Ok(Shape::Arc { c, r, a0, sweep })
        }
        Entity::Circle { center, radius } => Ok(Shape::Circle {
            c: sketch.point(*center)?,
            r: *radius,
        }),
        Entity::Point { .. } | Entity::Spline { .. } => {
            Err(SketchError::WrongKind(id, "line, arc or circle"))
        }
    }
}

/// What produced a cut on a trimmed curve.
#[derive(Clone, Copy)]
enum CutBy {
    Point(EntityId),
    Curve(EntityId),
}

/// Where two lines meet, for fillets and chamfers.
struct Corner {
    /// The endpoint of each line at the corner.
    ca: EntityId,
    cb: EntityId,
    /// The other endpoints.
    far_a: EntityId,
    far_b: EntityId,
    /// The corner's position and the unit directions away from it.
    pos: Vec2,
    da: Vec2,
    db: Vec2,
    /// Half the angle between the lines.
    half: f64,
}

impl Sketch {
    /// Removes the piece of curve `id` that contains the point nearest
    /// `at`, bounded by its intersections with other entities. A curve with
    /// no intersections is removed entirely; a circle becomes an arc.
    /// Returns the entities that replace it.
    pub fn trim(&mut self, id: EntityId, at: Vec2) -> Result<Vec<EntityId>, SketchError> {
        let shape = shape_of(self, id)?;
        let own: Vec<EntityId> = self.entity(id).unwrap().references();
        // Cuts: other curves crossing this one, and points lying on it.
        let mut cuts: Vec<(f64, CutBy)> = Vec::new();
        let inside = |t: f64| t > 1e-9 && t < 1.0 - 1e-9;
        let others: Vec<(EntityId, Entity)> = self
            .entities()
            .filter(|(oid, _)| *oid != id && !own.contains(oid))
            .map(|(oid, e)| (oid, e.clone()))
            .collect();
        for (oid, e) in &others {
            match e {
                Entity::Point { pos } => {
                    if let Some(t) = shape.contains(*pos) {
                        if inside(t) || matches!(shape, Shape::Circle { .. }) {
                            cuts.push((t, CutBy::Point(*oid)));
                        }
                    }
                }
                _ => {
                    let Ok(os) = shape_of(self, *oid) else {
                        continue;
                    };
                    for p in shape.intersections(&os) {
                        let t = shape.param(p);
                        if inside(t) || matches!(shape, Shape::Circle { .. }) {
                            cuts.push((t, CutBy::Curve(*oid)));
                        }
                    }
                }
            }
        }
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        cuts.dedup_by(|x, y| (x.0 - y.0).abs() <= 1e-9);
        let t = shape.param(at);
        let construction = self.is_construction(id);
        let entity = self.entity(id).unwrap().clone();
        let mut out = Vec::new();

        // A new point at a cut, tied to whatever made the cut.
        let cut_point = |sk: &mut Sketch, (tc, by): (f64, CutBy)| -> EntityId {
            let p = sk.add_point(shape.at(tc));
            match by {
                CutBy::Point(q) => {
                    sk.add_constraint(Constraint::Coincident { a: p, b: q });
                }
                CutBy::Curve(c) => match sk.entity(c) {
                    Some(Entity::Line { .. }) => {
                        sk.add_constraint(Constraint::PointOnLine { point: p, line: c });
                    }
                    Some(_) => {
                        sk.add_constraint(Constraint::PointOnCircle {
                            point: p,
                            entity: c,
                        });
                    }
                    None => {}
                },
            }
            p
        };

        match entity {
            Entity::Circle { center, .. } => {
                if cuts.is_empty() {
                    return Err(SketchError::WrongKind(id, "circle crossed by something"));
                }
                // Remaining arc runs counter-clockwise from the cut after
                // the click round to the cut before it.
                let hi = cuts.iter().copied().find(|c| c.0 >= t).unwrap_or(cuts[0]);
                let lo = cuts
                    .iter()
                    .rev()
                    .copied()
                    .find(|c| c.0 <= t)
                    .unwrap_or(*cuts.last().unwrap());
                let start = cut_point(self, hi);
                let end = if cuts.len() == 1 {
                    // One cut: the whole circle minus nothing measurable;
                    // open it at the cut so it becomes a full-turn arc.
                    start
                } else {
                    cut_point(self, lo)
                };
                let arc = self.alloc_entity(Entity::Arc { center, start, end });
                out.push(arc);
            }
            Entity::Line { start, end } | Entity::Arc { start, end, .. } => {
                if cuts.is_empty() {
                    self.remove_entity(id);
                    return Ok(out);
                }
                let lo = cuts.iter().rev().copied().find(|c| c.0 <= t);
                let hi = cuts.iter().copied().find(|c| c.0 >= t);
                let center = match entity {
                    Entity::Arc { center, .. } => Some(center),
                    _ => None,
                };
                let piece = |sk: &mut Sketch, s: EntityId, e: EntityId| -> EntityId {
                    match center {
                        Some(center) => sk.alloc_entity(Entity::Arc {
                            center,
                            start: s,
                            end: e,
                        }),
                        None => sk.add_line_between(s, e),
                    }
                };
                if let Some(lo) = lo {
                    let p = cut_point(self, lo);
                    out.push(piece(self, start, p));
                }
                if let Some(hi) = hi {
                    let p = cut_point(self, hi);
                    out.push(piece(self, p, end));
                }
            }
            // `shape_of` refused these already.
            Entity::Point { .. } | Entity::Spline { .. } => unreachable!(),
        }
        for e in &out {
            if construction {
                let _ = self.set_construction(*e, true);
            }
        }
        // The original goes last so its endpoints stay for the pieces.
        self.remove_entity(id);
        Ok(out)
    }

    /// Offsets a chain of connected lines and arcs (or one circle) by
    /// `distance` to the left of the chain's direction, taken from the
    /// first entity; negative distances offset to the right. Line corners
    /// are mitred; where offset curves overlap they are trimmed to their
    /// intersection, where they leave a gap the corner is rounded. Arcs
    /// share the original centre point. Returns the new curve entities.
    pub fn offset(
        &mut self,
        ids: &[EntityId],
        distance: f64,
    ) -> Result<Vec<EntityId>, SketchError> {
        let first = *ids
            .first()
            .ok_or(SketchError::WrongKind(EntityId(0), "selection"))?;
        if let (1, Some(Entity::Circle { center, radius })) = (ids.len(), self.entity(first)) {
            let (center, radius) = (*center, *radius);
            let r = radius + distance;
            if r <= TOL {
                return Err(SketchError::WrongKind(
                    first,
                    "circle larger than the offset",
                ));
            }
            let c = self.alloc_entity(Entity::Circle { center, radius: r });
            return Ok(vec![c]);
        }
        // Oriented segments: (entity, reversed, start, end).
        struct Seg {
            id: EntityId,
            reversed: bool,
            start: Vec2,
            end: Vec2,
        }
        let ends = |sk: &Sketch, id: EntityId| -> Result<(Vec2, Vec2), SketchError> {
            match sk.entity(id) {
                Some(Entity::Line { start, end }) | Some(Entity::Arc { start, end, .. }) => {
                    Ok((sk.point(*start)?, sk.point(*end)?))
                }
                Some(_) => Err(SketchError::WrongKind(id, "line or arc")),
                None => Err(SketchError::UnknownEntity(id)),
            }
        };
        let mut unused: Vec<EntityId> = ids[1..].to_vec();
        let (s0, e0) = ends(self, first)?;
        let mut chain = vec![Seg {
            id: first,
            reversed: false,
            start: s0,
            end: e0,
        }];
        // Grow forward, then backward.
        loop {
            let tail = chain.last().unwrap().end;
            let Some(pos) = unused.iter().position(|&id| {
                ends(self, id)
                    .is_ok_and(|(s, e)| s.distance(tail) <= TOL || e.distance(tail) <= TOL)
            }) else {
                break;
            };
            let id = unused.remove(pos);
            let (s, e) = ends(self, id)?;
            let reversed = s.distance(tail) > TOL;
            chain.push(Seg {
                id,
                reversed,
                start: if reversed { e } else { s },
                end: if reversed { s } else { e },
            });
        }
        loop {
            let head = chain[0].start;
            let Some(pos) = unused.iter().position(|&id| {
                ends(self, id)
                    .is_ok_and(|(s, e)| s.distance(head) <= TOL || e.distance(head) <= TOL)
            }) else {
                break;
            };
            let id = unused.remove(pos);
            let (s, e) = ends(self, id)?;
            let reversed = e.distance(head) > TOL;
            chain.insert(
                0,
                Seg {
                    id,
                    reversed,
                    start: if reversed { e } else { s },
                    end: if reversed { s } else { e },
                },
            );
        }
        if let Some(&stray) = unused.first() {
            return Err(SketchError::WrongKind(stray, "part of one connected chain"));
        }
        let closed = chain.len() > 1 && chain[0].start.distance(chain.last().unwrap().end) <= TOL;

        // Offset each segment: its new endpoints and shape.
        struct Off {
            shape: Shape,
            start: Vec2,
            end: Vec2,
            center: Option<EntityId>,
        }
        let mut offs: Vec<Off> = Vec::with_capacity(chain.len());
        for seg in &chain {
            let e = self.entity(seg.id).unwrap().clone();
            match e {
                Entity::Line { .. } => {
                    let n = (seg.end - seg.start)
                        .normalized()
                        .ok_or(SketchError::WrongKind(seg.id, "line with length"))?
                        .perp();
                    let (a, b) = (seg.start + n * distance, seg.end + n * distance);
                    offs.push(Off {
                        shape: Shape::Line { a, b },
                        start: a,
                        end: b,
                        center: None,
                    });
                }
                Entity::Arc { center, .. } => {
                    let Shape::Arc { c, r, a0, sweep } = shape_of(self, seg.id)? else {
                        unreachable!()
                    };
                    // Travelling CCW the centre is on the left.
                    let r2 = if seg.reversed {
                        r + distance
                    } else {
                        r - distance
                    };
                    if r2 <= TOL {
                        return Err(SketchError::WrongKind(seg.id, "arc larger than the offset"));
                    }
                    let shape = Shape::Arc {
                        c,
                        r: r2,
                        a0,
                        sweep,
                    };
                    let (s, e) = (shape.at(0.0), shape.at(1.0));
                    offs.push(Off {
                        shape,
                        start: if seg.reversed { e } else { s },
                        end: if seg.reversed { s } else { e },
                        center: Some(center),
                    });
                }
                _ => return Err(SketchError::WrongKind(seg.id, "line or arc")),
            }
        }

        // Joints between consecutive offset segments.
        enum Joint {
            Point(Vec2),
            /// Rounded corner about the original vertex, from the end of
            /// one offset to the start of the next.
            Round {
                corner: Vec2,
                from: Vec2,
                to: Vec2,
            },
        }
        let n = offs.len();
        let joint_count = if closed { n } else { n - 1 };
        let mut joints: Vec<Joint> = Vec::with_capacity(joint_count);
        for i in 0..joint_count {
            let (a, b) = (&offs[i], &offs[(i + 1) % n]);
            let corner = chain[i].end;
            if a.end.distance(b.start) <= TOL {
                joints.push(Joint::Point((a.end + b.start) * 0.5));
                continue;
            }
            let both_lines = a.center.is_none() && b.center.is_none();
            let candidates = a.shape.intersections_unbounded(&b.shape);
            let pick = candidates
                .into_iter()
                .filter(|p| {
                    both_lines || (a.shape.contains(*p).is_some() && b.shape.contains(*p).is_some())
                })
                .min_by(|p, q| p.distance(corner).total_cmp(&q.distance(corner)));
            match pick {
                Some(p) if both_lines || p.distance(corner) <= distance.abs() * 4.0 + TOL => {
                    joints.push(Joint::Point(p));
                }
                _ => joints.push(Joint::Round {
                    corner,
                    from: a.end,
                    to: b.start,
                }),
            }
        }

        // Points: one per plain joint, two per rounded joint (where the
        // corner arc starts and ends), plus the open ends.
        let mut jp: Vec<(EntityId, EntityId)> = Vec::with_capacity(joint_count); // (end of i, start of i+1)
        let mut rounds: Vec<(Vec2, EntityId, EntityId)> = Vec::new(); // corner, from, to
        for j in &joints {
            match j {
                Joint::Point(p) => {
                    let id = self.add_point(*p);
                    jp.push((id, id));
                }
                Joint::Round { corner, from, to } => {
                    let (f, t) = (self.add_point(*from), self.add_point(*to));
                    rounds.push((*corner, f, t));
                    jp.push((f, t));
                }
            }
        }
        let mut starts: Vec<EntityId> = Vec::with_capacity(n);
        let mut ends_: Vec<EntityId> = Vec::with_capacity(n);
        for i in 0..n {
            let start = if i == 0 && !closed {
                self.add_point(offs[0].start)
            } else {
                jp[(i + n - 1) % n].1
            };
            let end = if i == n - 1 && !closed {
                self.add_point(offs[i].end)
            } else {
                jp[i].0
            };
            starts.push(start);
            ends_.push(end);
        }
        let mut out = Vec::new();
        for i in 0..n {
            let (s, e) = (starts[i], ends_[i]);
            let id = match offs[i].center {
                None => {
                    let l = self.add_line_between(s, e);
                    self.add_constraint(Constraint::Parallel {
                        a: chain[i].id,
                        b: l,
                    });
                    l
                }
                Some(center) => {
                    let (start, end) = if chain[i].reversed { (e, s) } else { (s, e) };
                    self.alloc_entity(Entity::Arc { center, start, end })
                }
            };
            out.push(id);
        }
        for (corner, from, to) in rounds {
            let center = self.add_point(corner);
            let (pf, pt) = (self.point(from)?, self.point(to)?);
            // Counter-clockwise from `from` to `to` if that is the short way.
            let ccw = ((pt - corner).angle() - (pf - corner).angle()).rem_euclid(TAU)
                <= std::f64::consts::PI;
            let (start, end) = if ccw { (from, to) } else { (to, from) };
            out.push(self.alloc_entity(Entity::Arc { center, start, end }));
        }
        Ok(out)
    }

    /// The corner where lines `a` and `b` meet: the endpoint of each at
    /// it (coincident, or at the same position), the far endpoints, the
    /// corner position and the unit directions away from it along each
    /// line, which must meet at an angle.
    fn corner_of(&self, a: EntityId, b: EntityId, what: &str) -> Result<Corner, SketchError> {
        let (a0, a1) = self.line(a)?;
        let (b0, b1) = self.line(b)?;
        let (ca, cb) = [(a0, b0), (a0, b1), (a1, b0), (a1, b1)]
            .into_iter()
            .find(|&(pa, pb)| {
                pa == pb
                    || self.constraints().any(|(_, c)| {
                        matches!(c, Constraint::Coincident { a: x, b: y }
                            if (*x == pa && *y == pb) || (*x == pb && *y == pa))
                    })
                    || self
                        .point(pa)
                        .ok()
                        .zip(self.point(pb).ok())
                        .is_some_and(|(p, q)| p.distance(q) <= TOL)
            })
            .ok_or_else(|| {
                SketchError::Invalid(format!("{what} needs two lines meeting at a corner"))
            })?;
        let far_a = if ca == a0 { a1 } else { a0 };
        let far_b = if cb == b0 { b1 } else { b0 };
        let pos = self.point(ca)?;
        let da = (self.point(far_a)? - pos)
            .normalized()
            .ok_or(SketchError::WrongKind(a, "line with length"))?;
        let db = (self.point(far_b)? - pos)
            .normalized()
            .ok_or(SketchError::WrongKind(b, "line with length"))?;
        let cos = da.dot(db).clamp(-1.0, 1.0);
        let half = cos.acos() / 2.0;
        if half <= 1e-6 || half >= std::f64::consts::FRAC_PI_2 - 1e-6 {
            return Err(SketchError::Invalid(format!(
                "{what} needs lines that meet at an angle"
            )));
        }
        Ok(Corner {
            ca,
            cb,
            far_a,
            far_b,
            pos,
            da,
            db,
            half,
        })
    }

    /// Splits a shared corner so each line keeps its own endpoint, moved
    /// to `ta` and `tb`: when one point entity serves both lines, `b` gets
    /// a fresh one; a coincident constraint between them is dropped.
    /// Returns the two endpoints (a's, b's).
    fn split_corner(
        &mut self,
        b: EntityId,
        ca: EntityId,
        cb: EntityId,
        ta: Vec2,
        tb: Vec2,
    ) -> Result<(EntityId, EntityId), SketchError> {
        let cb = if ca == cb {
            let fresh = self.add_point(tb);
            let (s, e) = self.line(b)?;
            let new_line = Entity::Line {
                start: if s == cb { fresh } else { s },
                end: if e == cb { fresh } else { e },
            };
            self.entities.insert(b, new_line);
            fresh
        } else {
            cb
        };
        let coincident: Vec<ConstraintId> = self
            .constraints()
            .filter(|(_, c)| {
                matches!(c, Constraint::Coincident { a: x, b: y }
                    if (*x == ca && *y == cb) || (*x == cb && *y == ca))
            })
            .map(|(id, _)| id)
            .collect();
        for id in coincident {
            self.remove_constraint(id);
        }
        self.set_point(ca, ta);
        self.set_point(cb, tb);
        Ok((ca, cb))
    }

    /// Rounds the corner where two lines meet: both lines are shortened
    /// to the tangent points and a tangent arc of `radius` joins them,
    /// held by coincident, tangent and radius constraints. The lines must
    /// share an endpoint (coincident, or at the same position). Returns
    /// the arc.
    pub fn fillet(
        &mut self,
        a: EntityId,
        b: EntityId,
        radius: f64,
    ) -> Result<EntityId, SketchError> {
        if radius <= 0.0 || !radius.is_finite() {
            return Err(SketchError::Invalid(
                "fillet radius must be positive".into(),
            ));
        }
        let corner = self.corner_of(a, b, "fillet")?;
        let Corner {
            ca,
            cb,
            far_a,
            far_b,
            pos,
            da,
            db,
            half,
        } = corner;
        // Tangent points sit `r / tan(θ/2)` from the corner along each line;
        // the centre lies on the bisector `r / sin(θ/2)` away.
        let t = radius / half.tan();
        let len_a = self.point(far_a)?.distance(pos);
        let len_b = self.point(far_b)?.distance(pos);
        if t >= len_a - TOL || t >= len_b - TOL {
            return Err(SketchError::Invalid(
                "fillet radius is too large for these lines".into(),
            ));
        }
        let ta = pos + da * t;
        let tb = pos + db * t;
        let bisector = (da + db)
            .normalized()
            .ok_or(SketchError::Invalid("fillet lines are collinear".into()))?;
        let centre = pos + bisector * (radius / half.sin());
        let (ca, cb) = self.split_corner(b, ca, cb, ta, tb)?;
        // Arc from a's tangent point to b's, counter-clockwise about the
        // centre: swap the ends when that would be the long way round.
        let ccw = (ta - centre).cross(tb - centre) > 0.0;
        let (start_pos, end_pos) = if ccw { (ta, tb) } else { (tb, ta) };
        let (arc, _c, start, end) = self.add_arc(centre, start_pos, end_pos);
        let (start_on, end_on) = if ccw { (ca, cb) } else { (cb, ca) };
        self.add_constraint(Constraint::Coincident {
            a: start,
            b: start_on,
        });
        self.add_constraint(Constraint::Coincident { a: end, b: end_on });
        self.add_constraint(Constraint::Tangent {
            line: a,
            entity: arc,
        });
        self.add_constraint(Constraint::Tangent {
            line: b,
            entity: arc,
        });
        self.add_constraint(Constraint::Radius {
            entity: arc,
            value: radius,
        });
        Ok(arc)
    }

    /// Cuts the corner where two lines meet: both are shortened by
    /// `distance` from the corner and a line joins the cut ends, held by
    /// coincident constraints and its length. The lines must share an
    /// endpoint (coincident, or at the same position). Returns the new
    /// line.
    pub fn chamfer(
        &mut self,
        a: EntityId,
        b: EntityId,
        distance: f64,
    ) -> Result<EntityId, SketchError> {
        if distance <= 0.0 || !distance.is_finite() {
            return Err(SketchError::Invalid(
                "chamfer distance must be positive".into(),
            ));
        }
        let Corner {
            ca,
            cb,
            far_a,
            far_b,
            pos,
            da,
            db,
            ..
        } = self.corner_of(a, b, "chamfer")?;
        let len_a = self.point(far_a)?.distance(pos);
        let len_b = self.point(far_b)?.distance(pos);
        if distance >= len_a - TOL || distance >= len_b - TOL {
            return Err(SketchError::Invalid(
                "chamfer distance is too large for these lines".into(),
            ));
        }
        let ta = pos + da * distance;
        let tb = pos + db * distance;
        let (ca, cb) = self.split_corner(b, ca, cb, ta, tb)?;
        let (line, start, end) = self.add_line(ta, tb);
        self.add_constraint(Constraint::Coincident { a: start, b: ca });
        self.add_constraint(Constraint::Coincident { a: end, b: cb });
        self.add_constraint(Constraint::Length {
            line,
            value: ta.distance(tb),
        });
        Ok(line)
    }

    /// Mirrors entities across the line `axis`, adding a `Symmetric`
    /// constraint for every point pair (and `Equal` for circle radii).
    /// Returns the new entities (curves and standalone points).
    pub fn mirror(
        &mut self,
        ids: &[EntityId],
        axis: EntityId,
    ) -> Result<Vec<EntityId>, SketchError> {
        let (ls, le) = self.line(axis)?;
        let (s, e) = (self.point(ls)?, self.point(le)?);
        let d = (e - s)
            .normalized()
            .ok_or(SketchError::WrongKind(axis, "line with length"))?;
        let reflect = |p: Vec2| -> Vec2 {
            let v = p - s;
            let along = d * v.dot(d);
            s + along * 2.0 - v
        };
        let mut mirrored: HashMap<EntityId, EntityId> = HashMap::new();
        let mut point_of = |sk: &mut Sketch, pid: EntityId| -> Result<EntityId, SketchError> {
            if let Some(&m) = mirrored.get(&pid) {
                return Ok(m);
            }
            let p = sk.point(pid)?;
            let m = sk.add_point(reflect(p));
            sk.add_constraint(Constraint::Symmetric {
                a: pid,
                b: m,
                line: axis,
            });
            mirrored.insert(pid, m);
            Ok(m)
        };
        let mut out = Vec::new();
        for &id in ids {
            if id == axis {
                continue;
            }
            let e = self
                .entity(id)
                .ok_or(SketchError::UnknownEntity(id))?
                .clone();
            let construction = self.is_construction(id);
            let new = match e {
                Entity::Point { .. } => point_of(self, id)?,
                Entity::Line { start, end } => {
                    let (a, b) = (point_of(self, start)?, point_of(self, end)?);
                    self.add_line_between(a, b)
                }
                Entity::Arc { center, start, end } => {
                    let c = point_of(self, center)?;
                    // A reflection reverses orientation: swap the ends.
                    let (a, b) = (point_of(self, end)?, point_of(self, start)?);
                    self.alloc_entity(Entity::Arc {
                        center: c,
                        start: a,
                        end: b,
                    })
                }
                Entity::Circle { center, radius } => {
                    let c = point_of(self, center)?;
                    let n = self.alloc_entity(Entity::Circle { center: c, radius });
                    self.add_constraint(Constraint::Equal { a: id, b: n });
                    n
                }
                Entity::Spline { points } => {
                    let ps = points
                        .iter()
                        .map(|p| point_of(self, *p))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.alloc_entity(Entity::Spline { points: ps })
                }
            };
            if construction {
                let _ = self.set_construction(new, true);
            }
            out.push(new);
        }
        Ok(out)
    }
}

impl Sketch {
    /// Copies entities `count - 1` times, each copy shifted by one more
    /// `step`. Every copied point is held at its offset from the original
    /// by horizontal and vertical distance constraints, so the copies follow
    /// the originals. Returns the new entity ids.
    pub fn pattern_linear(
        &mut self,
        ids: &[EntityId],
        count: usize,
        step: Vec2,
    ) -> Result<Vec<EntityId>, SketchError> {
        let mut out = Vec::new();
        for k in 1..count.max(1) {
            let shift = step * k as f64;
            let copied = self.copy_entities(ids, &|p| p + shift, &mut |sk, orig, copy| {
                sk.add_constraint(Constraint::HorizontalDistance {
                    a: orig,
                    b: copy,
                    value: shift.x,
                });
                sk.add_constraint(Constraint::VerticalDistance {
                    a: orig,
                    b: copy,
                    value: shift.y,
                });
            })?;
            out.extend(copied);
        }
        Ok(out)
    }

    /// Copies entities `count - 1` times around `center`, each copy turned by
    /// one more `angle_deg`. Every copied point is held rotated about the
    /// centre point (a new construction point) from its original.
    pub fn pattern_circular(
        &mut self,
        ids: &[EntityId],
        count: usize,
        center: Vec2,
        angle_deg: f64,
    ) -> Result<Vec<EntityId>, SketchError> {
        let c = self.add_point(center);
        self.set_construction(c, true)?;
        let mut out = vec![c];
        for k in 1..count.max(1) {
            let angle = angle_deg * k as f64;
            let (sn, cs) = angle.to_radians().sin_cos();
            let turn = move |p: Vec2| {
                let d = p - center;
                center + Vec2::new(d.x * cs - d.y * sn, d.x * sn + d.y * cs)
            };
            let copied = self.copy_entities(ids, &turn, &mut |sk, orig, copy| {
                sk.add_constraint(Constraint::Rotated {
                    a: orig,
                    b: copy,
                    center: c,
                    value: angle,
                });
            })?;
            out.extend(copied);
        }
        Ok(out)
    }

    /// Copies entities under a rigid map, calling `tie` for every copied
    /// point (original, copy) so the caller can constrain it. Circles keep
    /// an equal radius; construction status is copied.
    fn copy_entities(
        &mut self,
        ids: &[EntityId],
        map: &dyn Fn(Vec2) -> Vec2,
        tie: &mut dyn FnMut(&mut Sketch, EntityId, EntityId),
    ) -> Result<Vec<EntityId>, SketchError> {
        let mut copies: HashMap<EntityId, EntityId> = HashMap::new();
        let mut point_of = |sk: &mut Sketch,
                            pid: EntityId,
                            tie: &mut dyn FnMut(&mut Sketch, EntityId, EntityId)|
         -> Result<EntityId, SketchError> {
            if let Some(&m) = copies.get(&pid) {
                return Ok(m);
            }
            let p = sk.point(pid)?;
            let m = sk.add_point(map(p));
            tie(sk, pid, m);
            copies.insert(pid, m);
            Ok(m)
        };
        let mut out = Vec::new();
        for &id in ids {
            let e = self
                .entity(id)
                .ok_or(SketchError::UnknownEntity(id))?
                .clone();
            let construction = self.is_construction(id);
            let new = match e {
                Entity::Point { .. } => point_of(self, id, tie)?,
                Entity::Line { start, end } => {
                    let (a, b) = (point_of(self, start, tie)?, point_of(self, end, tie)?);
                    self.add_line_between(a, b)
                }
                Entity::Arc { center, start, end } => {
                    let c = point_of(self, center, tie)?;
                    let (a, b) = (point_of(self, start, tie)?, point_of(self, end, tie)?);
                    self.alloc_entity(Entity::Arc {
                        center: c,
                        start: a,
                        end: b,
                    })
                }
                Entity::Circle { center, radius } => {
                    let c = point_of(self, center, tie)?;
                    let n = self.alloc_entity(Entity::Circle { center: c, radius });
                    self.add_constraint(Constraint::Equal { a: id, b: n });
                    n
                }
                Entity::Spline { points } => {
                    let ps = points
                        .iter()
                        .map(|p| point_of(self, *p, tie))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.alloc_entity(Entity::Spline { points: ps })
                }
            };
            if construction {
                self.set_construction(new, true)?;
            }
            out.push(new);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileOptions, SolveStatus};
    use std::f64::consts::PI;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn line_ends(s: &Sketch, id: EntityId) -> (Vec2, Vec2) {
        let (a, b) = s.line(id).unwrap();
        (s.point(a).unwrap(), s.point(b).unwrap())
    }

    #[test]
    fn fillet_rounds_a_rectangle_corner_and_stays_solved() {
        let mut s = Sketch::new();
        let lines = s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 6.0));
        // Bottom (0,0)->(10,0) and right (10,0)->(10,6) meet at (10,0).
        let arc = s.fillet(lines[0], lines[1], 2.0).unwrap();
        let r = s.solve();
        assert!(r.max_residual < 1e-7, "{r:?}");
        let (start, end) = match s.entity(arc) {
            Some(Entity::Arc { start, end, .. }) => (*start, *end),
            _ => panic!("no arc"),
        };
        let (ps, pe) = (s.point(start).unwrap(), s.point(end).unwrap());
        // Tangent points 2 mm from the corner along each line.
        let mut ends = [ps, pe];
        ends.sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap());
        assert!(ends[0].distance(Vec2::new(8.0, 0.0)) < 1e-6, "{ends:?}");
        assert!(ends[1].distance(Vec2::new(10.0, 2.0)) < 1e-6, "{ends:?}");
        // One region whose area is the rectangle minus the corner's
        // rounded-off piece, (4 - π) r² / 4... i.e. r² (1 - π/4).
        let p = s.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 1);
        let expect = 60.0 - 4.0 * (1.0 - std::f64::consts::PI / 4.0);
        assert!((p[0].area() - expect).abs() < 0.02, "{}", p[0].area());
        // The radius dimension drives the fillet: change it and re-solve.
        let rid = s
            .constraints()
            .find(|(_, c)| matches!(c, Constraint::Radius { .. }))
            .map(|(id, _)| id)
            .unwrap();
        s.set_constraint_value(rid, 3.0);
        let r = s.solve();
        assert!(r.max_residual < 1e-7, "{r:?}");
        let ps = s.point(start).unwrap();
        let pe = s.point(end).unwrap();
        assert!(
            ((ps.distance(pe) / 2.0f64.sqrt()) - 3.0).abs() < 1e-6,
            "chord {}",
            ps.distance(pe)
        );
        // Too large a radius or lines that do not meet are refused.
        let mut t = Sketch::new();
        let l = t.add_rectangle(Vec2::ZERO, Vec2::new(4.0, 4.0));
        assert!(t.fillet(l[0], l[1], 5.0).is_err());
        assert!(t.fillet(l[0], l[2], 1.0).is_err());
    }

    #[test]
    fn chamfer_cuts_a_rectangle_corner_and_stays_solved() {
        let mut s = Sketch::new();
        let lines = s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 6.0));
        let cut = s.chamfer(lines[0], lines[1], 2.0).unwrap();
        let r = s.solve();
        assert!(r.max_residual < 1e-7, "{r:?}");
        let (start, end) = s.line(cut).unwrap();
        let mut ends = [s.point(start).unwrap(), s.point(end).unwrap()];
        ends.sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap());
        assert!(ends[0].distance(Vec2::new(8.0, 0.0)) < 1e-6, "{ends:?}");
        assert!(ends[1].distance(Vec2::new(10.0, 2.0)) < 1e-6, "{ends:?}");
        // The shortened lines end where the chamfer starts.
        let (_, a_end) = s.line(lines[0]).unwrap();
        let (b_start, _) = s.line(lines[1]).unwrap();
        assert!(s.point(a_end).unwrap().distance(Vec2::new(8.0, 0.0)) < 1e-6);
        assert!(s.point(b_start).unwrap().distance(Vec2::new(10.0, 2.0)) < 1e-6);
        // One region: the rectangle less the cut triangle.
        let p = s.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 1);
        assert!((p[0].area() - 58.0).abs() < 1e-6, "{}", p[0].area());
        // Its length is the dimension that drives it.
        let lid = s
            .constraints()
            .find(|(_, c)| matches!(c, Constraint::Length { line, .. } if *line == cut))
            .map(|(id, _)| id)
            .unwrap();
        s.set_constraint_value(lid, 3.0 * 2.0f64.sqrt());
        let r = s.solve();
        assert!(r.max_residual < 1e-7, "{r:?}");
        let (ps, pe) = (s.point(start).unwrap(), s.point(end).unwrap());
        assert!(
            (ps.distance(pe) - 3.0 * 2.0f64.sqrt()).abs() < 1e-6,
            "chamfer {}",
            ps.distance(pe)
        );
        assert_eq!(s.profiles(&ProfileOptions::default()).len(), 1);
        // Too long a chamfer or lines that do not meet are refused.
        let mut t = Sketch::new();
        let l = t.add_rectangle(Vec2::ZERO, Vec2::new(4.0, 4.0));
        assert!(t.chamfer(l[0], l[1], 4.0).is_err());
        assert!(t.chamfer(l[0], l[2], 1.0).is_err());
        assert!(t.chamfer(l[0], l[1], 0.0).is_err());
    }

    #[test]
    fn trim_removes_the_middle_of_a_crossed_line() {
        let mut s = Sketch::new();
        let (l, _, _) = s.add_line(v(0.0, 0.0), v(10.0, 0.0));
        s.add_line(v(3.0, -1.0), v(3.0, 1.0));
        s.add_line(v(7.0, -1.0), v(7.0, 1.0));
        let pieces = s.trim(l, v(5.0, 0.2)).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(s.entity(l).is_none());
        let (a0, a1) = line_ends(&s, pieces[0]);
        let (b0, b1) = line_ends(&s, pieces[1]);
        assert!(
            a0.approx_eq(v(0.0, 0.0)) && a1.approx_eq(v(3.0, 0.0)),
            "{a0:?} {a1:?}"
        );
        assert!(
            b0.approx_eq(v(7.0, 0.0)) && b1.approx_eq(v(10.0, 0.0)),
            "{b0:?} {b1:?}"
        );
        // The cut points sit on the cutters.
        assert_eq!(
            s.constraints()
                .filter(|(_, c)| matches!(c, Constraint::PointOnLine { .. }))
                .count(),
            2
        );
        // Trimming an end piece keeps the rest as one line.
        let rest = s.trim(pieces[0], v(1.0, 0.0)).unwrap();
        assert!(rest.is_empty() || rest.len() == 1);
        let mut t = Sketch::new();
        let (l, _, _) = t.add_line(v(0.0, 0.0), v(10.0, 0.0));
        t.add_line(v(3.0, -1.0), v(3.0, 1.0));
        let rest = t.trim(l, v(1.0, 0.0)).unwrap();
        assert_eq!(rest.len(), 1);
        let (r0, r1) = line_ends(&t, rest[0]);
        assert!(r0.approx_eq(v(3.0, 0.0)) && r1.approx_eq(v(10.0, 0.0)));
        // Nothing crossing: the whole line goes.
        let mut u = Sketch::new();
        let (l, _, _) = u.add_line(v(0.0, 0.0), v(10.0, 0.0));
        assert!(u.trim(l, v(5.0, 0.0)).unwrap().is_empty());
        assert!(u.entity(l).is_none());
    }

    #[test]
    fn trim_turns_a_circle_into_an_arc() {
        let mut s = Sketch::new();
        let (c, _) = s.add_circle(v(0.0, 0.0), 5.0);
        s.add_line(v(-10.0, 0.0), v(10.0, 0.0));
        let out = s.trim(c, v(0.0, 5.0)).unwrap();
        assert_eq!(out.len(), 1);
        let Some(Entity::Arc { start, end, .. }) = s.entity(out[0]).cloned() else {
            panic!("expected an arc");
        };
        assert!(s.point(start).unwrap().approx_eq(v(-5.0, 0.0)));
        assert!(s.point(end).unwrap().approx_eq(v(5.0, 0.0)));
        // Bottom half disc with the line: one region of area πr²/2.
        let profiles = s.profiles(&ProfileOptions::default());
        assert_eq!(profiles.len(), 1);
        let area = profiles[0].area().abs();
        assert!((area - PI * 25.0 / 2.0).abs() / area < 2e-3, "{area}");
    }

    #[test]
    fn trim_an_arc_at_a_crossing_line() {
        let mut s = Sketch::new();
        // Upper half circle from (5,0) to (-5,0), cut by the vertical line x = 0.
        let (arc, _, _, _) = s.add_arc(v(0.0, 0.0), v(5.0, 0.0), v(-5.0, 0.0));
        s.add_line(v(0.0, -1.0), v(0.0, 6.0));
        let out = s.trim(arc, v(3.5, 3.5)).unwrap();
        assert_eq!(out.len(), 1);
        let Some(Entity::Arc { start, end, .. }) = s.entity(out[0]).cloned() else {
            panic!("expected an arc");
        };
        assert!(s.point(start).unwrap().approx_eq(v(0.0, 5.0)));
        assert!(s.point(end).unwrap().approx_eq(v(-5.0, 0.0)));
    }

    #[test]
    fn offset_a_rectangle_outward() {
        let mut s = Sketch::new();
        let lines = s.add_rectangle(v(0.0, 0.0), v(10.0, 6.0));
        // add_rectangle runs counter-clockwise, so left is inward: -1 offsets outward.
        let out = s.offset(&lines, -1.0).unwrap();
        assert_eq!(out.len(), 4);
        let mut areas: Vec<f64> = s
            .profiles(&ProfileOptions::default())
            .iter()
            .map(|p| p.area().abs())
            .collect();
        areas.sort_by(|a, b| a.total_cmp(b));
        // Inner 10x6 region and the 1 mm ring around it (12x8 minus 10x6).
        assert_eq!(areas.len(), 2, "{areas:?}");
        assert!(
            (areas[0] - 36.0).abs() < 1e-9 && (areas[1] - 60.0).abs() < 1e-9,
            "{areas:?}"
        );
        // Inward by 1 shrinks it to 8x4.
        let mut t = Sketch::new();
        let lines = t.add_rectangle(v(0.0, 0.0), v(10.0, 6.0));
        t.offset(&lines, 1.0).unwrap();
        let mut areas: Vec<f64> = t
            .profiles(&ProfileOptions::default())
            .iter()
            .map(|p| p.area().abs())
            .collect();
        areas.sort_by(|a, b| a.total_cmp(b));
        assert!(
            (areas[0] - 28.0).abs() < 1e-9 && (areas[1] - 32.0).abs() < 1e-9,
            "{areas:?}"
        );
    }

    #[test]
    fn offset_an_open_corner_and_a_slot() {
        // Two lines meeting at a right angle: the outside offset mitres.
        let mut s = Sketch::new();
        let (l1, _, e1) = s.add_line(v(0.0, 0.0), v(10.0, 0.0));
        let (l2, s2, _) = s.add_line(v(10.0, 0.0), v(10.0, 10.0));
        s.add_constraint(Constraint::Coincident { a: e1, b: s2 });
        let out = s.offset(&[l1, l2], -1.0).unwrap();
        assert_eq!(out.len(), 2);
        let (a0, a1) = line_ends(&s, out[0]);
        let (b0, b1) = line_ends(&s, out[1]);
        assert!(
            a0.approx_eq(v(0.0, -1.0)) && a1.approx_eq(v(11.0, -1.0)),
            "{a0:?} {a1:?}"
        );
        assert!(
            b0.approx_eq(v(11.0, -1.0)) && b1.approx_eq(v(11.0, 10.0)),
            "{b0:?} {b1:?}"
        );
        assert_eq!(
            s.line(out[0]).unwrap().1,
            s.line(out[1]).unwrap().0,
            "shared corner point"
        );
        // A slot: line, semicircle, line, semicircle. Offset outward keeps
        // the arcs concentric and tangent, so the region is a bigger slot.
        let mut t = Sketch::new();
        let (a, _, _) = t.add_line(v(0.0, -2.0), v(10.0, -2.0));
        let (arc1, _, _, _) = t.add_arc(v(10.0, 0.0), v(10.0, -2.0), v(10.0, 2.0));
        let (b, _, _) = t.add_line(v(10.0, 2.0), v(0.0, 2.0));
        let (arc2, _, _, _) = t.add_arc(v(0.0, 0.0), v(0.0, 2.0), v(0.0, -2.0));
        let out = t.offset(&[a, arc1, b, arc2], -1.0).unwrap();
        assert_eq!(out.len(), 4);
        let mut areas: Vec<f64> = t
            .profiles(&ProfileOptions::default())
            .iter()
            .map(|p| p.area().abs())
            .collect();
        areas.sort_by(|a, b| a.total_cmp(b));
        let slot = |r: f64| 10.0 * 2.0 * r + PI * r * r;
        assert_eq!(areas.len(), 2, "{areas:?}");
        assert!((areas[1] - slot(2.0)).abs() / slot(2.0) < 2e-3, "{areas:?}");
        assert!(
            (areas[0] - (slot(3.0) - slot(2.0))).abs() / slot(2.0) < 2e-3,
            "{areas:?}"
        );
        // A lone circle offsets to a concentric circle.
        let mut u = Sketch::new();
        let (c, center) = u.add_circle(v(1.0, 1.0), 2.0);
        let out = u.offset(&[c], 0.5).unwrap();
        assert!(
            matches!(u.entity(out[0]), Some(Entity::Circle { center: k, radius }) if *k == center && (*radius - 2.5).abs() < 1e-12)
        );
    }

    #[test]
    fn linear_and_circular_patterns_copy_regions_and_follow_the_original() {
        let mut s = Sketch::new();
        let lines = s.add_rectangle(v(0.0, 0.0), v(10.0, 5.0));
        let copies = s.pattern_linear(&lines, 3, v(20.0, 0.0)).unwrap();
        assert_eq!(copies.len(), 8);
        let solve = s.solve();
        assert_eq!(solve.status, SolveStatus::UnderConstrained, "{solve:?}");
        // Copies add no freedom: still the original rectangle's four.
        assert_eq!(solve.dof, 4);
        assert_eq!(s.profiles(&ProfileOptions::default()).len(), 3);
        // Dragging a corner of the original keeps every copy at its offset.
        let (_, corner) = s.line(lines[0]).unwrap();
        let before = s.point(corner).unwrap();
        s.set_point_pub(corner, before + v(0.0, 3.0));
        s.solve();
        let after = s.point(corner).unwrap();
        let (_, c2) = s.line(copies[0]).unwrap();
        let moved = s.point(c2).unwrap();
        assert!(
            (moved.x - (after.x + 20.0)).abs() < 1e-6 && (moved.y - after.y).abs() < 1e-6,
            "{moved:?} vs {after:?}"
        );

        let mut s = Sketch::new();
        let (circle, _) = s.add_circle(v(30.0, 0.0), 4.0);
        let copies = s.pattern_circular(&[circle], 6, v(0.0, 0.0), 60.0).unwrap();
        assert_eq!(copies.len(), 6, "centre point plus five circles");
        let solve = s.solve();
        assert_eq!(solve.status, SolveStatus::UnderConstrained, "{solve:?}");
        assert_eq!(s.profiles(&ProfileOptions::default()).len(), 6);
        // Circle, its centre, radius and the pattern centre: 2 + 1 + 2 = 5.
        assert_eq!(solve.dof, 5);
    }

    #[test]
    fn regular_polygon_and_slot_form_regions_with_the_expected_areas() {
        let mut s = Sketch::new();
        let lines = s.add_regular_polygon(v(0.0, 0.0), v(10.0, 0.0), 6);
        assert_eq!(lines.len(), 6);
        let solve = s.solve();
        assert!(matches!(solve.status, SolveStatus::UnderConstrained));
        let profiles = s.profiles(&ProfileOptions::default());
        assert_eq!(profiles.len(), 1);
        let hexagon = 1.5 * 3f64.sqrt() * 100.0;
        assert!(
            (profiles[0].area().abs() - hexagon).abs() < 1e-6,
            "{}",
            profiles[0].area()
        );
        // Centre (2), one corner (2): four degrees of freedom remain.
        assert_eq!(solve.dof, 4);

        let mut s = Sketch::new();
        let ids = s.add_slot(v(0.0, 0.0), v(20.0, 0.0), 6.0);
        assert_eq!(ids.len(), 4);
        let solve = s.solve();
        assert!(matches!(solve.status, SolveStatus::UnderConstrained));
        let opts = ProfileOptions {
            arc_segment_angle: 0.5f64.to_radians(),
            ..Default::default()
        };
        let profiles = s.profiles(&opts);
        assert_eq!(profiles.len(), 1);
        let slot = 20.0 * 6.0 + std::f64::consts::PI * 9.0;
        assert!(
            (profiles[0].area().abs() - slot).abs() < 0.05,
            "{}",
            profiles[0].area()
        );
        // Two centres (4) and the radius (1).
        assert_eq!(solve.dof, 5, "{solve:?}");
    }

    #[test]
    fn mirror_a_triangle_across_a_construction_line() {
        let mut s = Sketch::new();
        let (axis, _, _) = s.add_line(v(0.0, -10.0), v(0.0, 10.0));
        s.set_construction(axis, true).unwrap();
        let (l1, p0, p1) = s.add_line(v(1.0, 0.0), v(5.0, 0.0));
        let (l2, p2, p3) = s.add_line(v(5.0, 0.0), v(3.0, 4.0));
        let (l3, p4, p5) = s.add_line(v(3.0, 4.0), v(1.0, 0.0));
        for (a, b) in [(p1, p2), (p3, p4), (p5, p0)] {
            s.add_constraint(Constraint::Coincident { a, b });
        }
        let out = s.mirror(&[l1, l2, l3], axis).unwrap();
        assert_eq!(out.len(), 3);
        let symmetric = s
            .constraints()
            .filter(|(_, c)| matches!(c, Constraint::Symmetric { .. }))
            .count();
        assert_eq!(symmetric, 6, "one per mirrored point");
        let (m0, m1) = line_ends(&s, out[0]);
        assert!(
            m0.approx_eq(v(-1.0, 0.0)) && m1.approx_eq(v(-5.0, 0.0)),
            "{m0:?} {m1:?}"
        );
        let r = s.solve();
        assert_ne!(r.status, SolveStatus::Inconsistent);
        let areas: Vec<f64> = s
            .profiles(&ProfileOptions::default())
            .iter()
            .map(|p| p.area().abs())
            .collect();
        assert_eq!(areas.len(), 2);
        assert!(
            (areas[0] - 8.0).abs() < 1e-9 && (areas[1] - 8.0).abs() < 1e-9,
            "{areas:?}"
        );
        // With the originals fixed, moving one drags its mirror image along.
        for p in [p0, p1, p2, p3, p4, p5] {
            s.add_constraint(Constraint::Fixed { point: p });
        }
        let (a0, a1) = s.line(axis).unwrap();
        s.add_constraint(Constraint::Fixed { point: a0 });
        s.add_constraint(Constraint::Fixed { point: a1 });
        s.set_point_pub(p3, v(3.0, 6.0));
        s.set_point_pub(p4, v(3.0, 6.0));
        s.solve();
        let (_, top) = line_ends(&s, out[1]);
        assert!(top.approx_eq(v(-3.0, 6.0)), "{top:?}");
    }
}
