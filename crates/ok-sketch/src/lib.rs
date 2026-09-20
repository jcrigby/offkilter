//! 2D parametric sketches.
//!
//! A [`Sketch`] is a set of geometric [`Entity`]s (points, lines, circles,
//! arcs) and [`Constraint`]s between them. [`Sketch::solve`] moves the free
//! geometry so that every constraint is satisfied, and
//! [`Sketch::profiles`] extracts the closed regions bounded by the solved
//! geometry so they can be fed to 3D features such as extrude.

mod edit;
mod entity;
mod loops;
mod solver;
mod spline;

pub use entity::{Constraint, ConstraintId, Entity, EntityId};
pub use loops::{point_in_polygon, signed_area, Loop, Profile, ProfileOptions, SegmentCurve};
pub use solver::{SolveResult, SolveStatus};
pub use spline::{spline_pieces, spline_polyline};

use ok_math::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Serialises an id-keyed map as a JSON array of `{"id": .., ...}` objects,
/// which is both readable and avoids non-string map keys.
mod id_map {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize)]
    struct Entry<K, V> {
        id: K,
        #[serde(flatten)]
        value: V,
    }

    pub fn serialize<K, V, S>(map: &BTreeMap<K, V>, s: S) -> Result<S::Ok, S::Error>
    where
        K: Serialize + Copy,
        V: Serialize,
        S: Serializer,
    {
        s.collect_seq(map.iter().map(|(k, v)| Entry { id: *k, value: v }))
    }

    pub fn deserialize<'de, K, V, D>(d: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let entries: Vec<Entry<K, V>> = Vec::deserialize(d)?;
        Ok(entries.into_iter().map(|e| (e.id, e.value)).collect())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SketchError {
    #[error("unknown entity {0:?}")]
    UnknownEntity(EntityId),
    #[error("entity {0:?} is not a {1}")]
    WrongKind(EntityId, &'static str),
    #[error("entity {0:?} already exists")]
    DuplicateEntity(EntityId),
    #[error("{0}")]
    Invalid(String),
}

/// A 2D sketch: geometry plus constraints, in plane coordinates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    #[serde(with = "id_map")]
    entities: BTreeMap<EntityId, Entity>,
    #[serde(with = "id_map")]
    constraints: BTreeMap<ConstraintId, Constraint>,
    /// Construction entities take part in constraints but not in regions.
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    construction: std::collections::BTreeSet<EntityId>,
    /// Entities projected from body geometry: driven by the model, so the
    /// solver holds them fixed. Their ids come from a reserved block.
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    projected: std::collections::BTreeSet<EntityId>,
    next_entity: u32,
    next_constraint: u32,
}

impl Sketch {
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------------------------------------------------------- access

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.entities.iter().map(|(id, e)| (*id, e))
    }

    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> {
        self.constraints.iter().map(|(id, c)| (*id, c))
    }

    /// Position of a point entity.
    pub fn point(&self, id: EntityId) -> Result<Vec2, SketchError> {
        match self.entities.get(&id) {
            Some(Entity::Point { pos }) => Ok(*pos),
            Some(_) => Err(SketchError::WrongKind(id, "point")),
            None => Err(SketchError::UnknownEntity(id)),
        }
    }

    /// Endpoints of a line entity.
    pub fn line(&self, id: EntityId) -> Result<(EntityId, EntityId), SketchError> {
        match self.entities.get(&id) {
            Some(Entity::Line { start, end }) => Ok((*start, *end)),
            Some(_) => Err(SketchError::WrongKind(id, "line")),
            None => Err(SketchError::UnknownEntity(id)),
        }
    }

    // -------------------------------------------------------------- building

    /// Makes the next allocated entity and constraint ids start at `base`.
    /// Used by collaborative editing, where each client owns an id range so
    /// that ops allocate the same ids whatever order they are applied in.
    pub fn set_id_base(&mut self, base: u32) {
        self.next_entity = base;
        self.next_constraint = base;
    }

    fn alloc_entity(&mut self, e: Entity) -> EntityId {
        let id = EntityId(self.next_entity);
        self.next_entity += 1;
        self.entities.insert(id, e);
        id
    }

    /// Reserves `n` consecutive entity ids and returns the first. Projected
    /// geometry is rebuilt on every regeneration but must keep stable,
    /// deterministic ids, so the block is allocated once when the projection
    /// is added and reused through [`Sketch::insert_projected`].
    pub fn reserve_entity_ids(&mut self, n: u32) -> EntityId {
        let id = EntityId(self.next_entity);
        self.next_entity += n;
        id
    }

    /// Inserts a projected entity under a chosen id (from a reserved block).
    pub fn insert_projected(&mut self, id: EntityId, e: Entity) -> Result<(), SketchError> {
        if self.entities.contains_key(&id) {
            return Err(SketchError::DuplicateEntity(id));
        }
        self.entities.insert(id, e);
        self.projected.insert(id);
        Ok(())
    }

    /// Replaces a projected entity's geometry in place (same id and kind).
    pub fn replace_projected(&mut self, id: EntityId, e: Entity) {
        if self.projected.contains(&id) {
            self.entities.insert(id, e);
        }
    }

    /// Inserts or overwrites an entity under a chosen id (undo of a removal
    /// or a move). Counters are untouched: the id was allocated before.
    pub fn insert_entity_with_id(&mut self, id: EntityId, e: Entity) {
        self.entities.insert(id, e);
    }

    /// Inserts or overwrites a constraint under a chosen id.
    pub fn insert_constraint_with_id(&mut self, id: ConstraintId, c: Constraint) {
        self.constraints.insert(id, c);
    }

    /// Marks an entity as projected (or not) without touching its geometry.
    pub fn set_projected(&mut self, id: EntityId, projected: bool) {
        if projected {
            if self.entities.contains_key(&id) {
                self.projected.insert(id);
            }
        } else {
            self.projected.remove(&id);
        }
    }

    pub fn is_projected(&self, id: EntityId) -> bool {
        self.projected.contains(&id)
    }

    pub fn projected_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.projected.iter().copied()
    }

    /// Adds a curve entity referencing existing point entities.
    pub fn add_entity(&mut self, e: Entity) -> Result<EntityId, SketchError> {
        for r in e.references() {
            self.point(r)?;
        }
        Ok(self.alloc_entity(e))
    }

    pub fn add_point(&mut self, pos: Vec2) -> EntityId {
        self.alloc_entity(Entity::Point { pos })
    }

    /// Adds a line between two existing points.
    pub fn add_line_between(&mut self, start: EntityId, end: EntityId) -> EntityId {
        self.alloc_entity(Entity::Line { start, end })
    }

    /// Adds a line with fresh endpoints. Returns `(line, start, end)`.
    pub fn add_line(&mut self, a: Vec2, b: Vec2) -> (EntityId, EntityId, EntityId) {
        let s = self.add_point(a);
        let e = self.add_point(b);
        (self.add_line_between(s, e), s, e)
    }

    /// Adds a circle with a fresh centre point. Returns `(circle, center)`.
    pub fn add_circle(&mut self, center: Vec2, radius: f64) -> (EntityId, EntityId) {
        let c = self.add_point(center);
        (self.alloc_entity(Entity::Circle { center: c, radius }), c)
    }

    /// Adds a counter-clockwise arc from `start` to `end` about `center`, all
    /// as fresh points. Returns `(arc, center, start, end)`.
    pub fn add_arc(
        &mut self,
        center: Vec2,
        start: Vec2,
        end: Vec2,
    ) -> (EntityId, EntityId, EntityId, EntityId) {
        let c = self.add_point(center);
        let s = self.add_point(start);
        let e = self.add_point(end);
        (
            self.alloc_entity(Entity::Arc {
                center: c,
                start: s,
                end: e,
            }),
            c,
            s,
            e,
        )
    }

    /// Adds a spline through fresh points at `points` (at least two).
    /// Returns `(spline, points)`.
    pub fn add_spline(
        &mut self,
        points: &[Vec2],
    ) -> Result<(EntityId, Vec<EntityId>), SketchError> {
        if points.len() < 2 {
            return Err(SketchError::Invalid(
                "a spline needs at least two points".into(),
            ));
        }
        let ids: Vec<EntityId> = points.iter().map(|p| self.add_point(*p)).collect();
        let id = self.alloc_entity(Entity::Spline {
            points: ids.clone(),
        });
        Ok((id, ids))
    }

    /// Centre and radius of a circle or arc at the current geometry.
    pub fn circular_geometry(&self, id: EntityId) -> Option<(Vec2, f64)> {
        match self.entity(id)? {
            Entity::Circle { center, radius } => Some((self.point(*center).ok()?, *radius)),
            Entity::Arc { center, start, .. } => {
                let c = self.point(*center).ok()?;
                Some((c, self.point(*start).ok()?.distance(c)))
            }
            _ => None,
        }
    }

    /// The positions of a spline's points in order.
    pub fn spline_points(&self, id: EntityId) -> Result<Vec<Vec2>, SketchError> {
        match self.entity(id) {
            Some(Entity::Spline { points }) => points.iter().map(|p| self.point(*p)).collect(),
            Some(_) => Err(SketchError::WrongKind(id, "spline")),
            None => Err(SketchError::UnknownEntity(id)),
        }
    }

    /// Adds an axis-aligned rectangle made of four lines whose corners are
    /// tied together with coincident constraints and whose edges are held
    /// horizontal / vertical. Returns the four line ids in order
    /// bottom, right, top, left.
    pub fn add_rectangle(&mut self, corner_a: Vec2, corner_b: Vec2) -> [EntityId; 4] {
        let (x0, x1) = (corner_a.x.min(corner_b.x), corner_a.x.max(corner_b.x));
        let (y0, y1) = (corner_a.y.min(corner_b.y), corner_a.y.max(corner_b.y));
        let corners = [
            Vec2::new(x0, y0),
            Vec2::new(x1, y0),
            Vec2::new(x1, y1),
            Vec2::new(x0, y1),
        ];
        let mut lines = [EntityId(0); 4];
        let mut ends: Vec<(EntityId, EntityId)> = Vec::new();
        for i in 0..4 {
            let (l, s, e) = self.add_line(corners[i], corners[(i + 1) % 4]);
            lines[i] = l;
            ends.push((s, e));
        }
        for i in 0..4 {
            let (_, e) = ends[i];
            let (s, _) = ends[(i + 1) % 4];
            self.add_constraint(Constraint::Coincident { a: e, b: s });
        }
        self.add_constraint(Constraint::Horizontal { line: lines[0] });
        self.add_constraint(Constraint::Vertical { line: lines[1] });
        self.add_constraint(Constraint::Horizontal { line: lines[2] });
        self.add_constraint(Constraint::Vertical { line: lines[3] });
        lines
    }

    /// Adds a regular polygon with `sides` sides centred on `center` with
    /// one corner at `vertex`: lines tied corner to corner, held equal in
    /// length and at equal turns, so it stays regular while its centre,
    /// size and rotation move. Returns the line ids in order.
    pub fn add_regular_polygon(
        &mut self,
        center: Vec2,
        vertex: Vec2,
        sides: usize,
    ) -> Vec<EntityId> {
        let sides = sides.max(3);
        let r = vertex - center;
        let corners: Vec<Vec2> = (0..sides)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / sides as f64;
                let (sn, cs) = t.sin_cos();
                center + Vec2::new(r.x * cs - r.y * sn, r.x * sn + r.y * cs)
            })
            .collect();
        let mut lines = Vec::with_capacity(sides);
        let mut ends: Vec<(EntityId, EntityId)> = Vec::new();
        for i in 0..sides {
            let (l, a, b) = self.add_line(corners[i], corners[(i + 1) % sides]);
            lines.push(l);
            ends.push((a, b));
        }
        for i in 0..sides {
            let (_, e) = ends[i];
            let (a, _) = ends[(i + 1) % sides];
            self.add_constraint(Constraint::Coincident { a: e, b: a });
        }
        for i in 1..sides {
            self.add_constraint(Constraint::Equal {
                a: lines[0],
                b: lines[i],
            });
        }
        // Equal sides alone still flex; fixing the turn between
        // consecutive sides makes it regular. The last two turns follow
        // from closure, so constraining them would be redundant.
        for i in 0..sides.saturating_sub(3) {
            self.add_constraint(Constraint::Angle {
                a: lines[i],
                b: lines[i + 1],
                value: 360.0 / sides as f64,
            });
        }
        lines
    }

    /// Adds a slot: two lines joined by semicircular ends of `width / 2`
    /// radius around the centres `a` and `b`. The lines are tangent to the
    /// arcs and the arcs share their radius. Returns [line, arc at `b`,
    /// line, arc at `a`].
    pub fn add_slot(&mut self, a: Vec2, b: Vec2, width: f64) -> Vec<EntityId> {
        let r = width.abs() / 2.0;
        let d = b - a;
        let len = d.length().max(1e-9);
        let perp = Vec2::new(-d.y / len, d.x / len) * r;
        let (p1, p2, p3, p4) = (a + perp, b + perp, b - perp, a - perp);
        let (l1, l1s, l1e) = self.add_line(p1, p2);
        let (arc_b, cb, bs, be) = self.add_arc(b, p3, p2);
        let (l2, l2s, l2e) = self.add_line(p3, p4);
        let (arc_a, ca, as_, ae) = self.add_arc(a, p1, p4);
        for (x, y) in [(l1e, be), (bs, l2s), (l2e, ae), (as_, l1s)] {
            self.add_constraint(Constraint::Coincident { a: x, b: y });
        }
        for (line, entity) in [(l1, arc_a), (l1, arc_b), (l2, arc_a), (l2, arc_b)] {
            self.add_constraint(Constraint::Tangent { line, entity });
        }
        self.add_constraint(Constraint::Equal { a: arc_a, b: arc_b });
        let _ = (cb, ca);
        vec![l1, arc_b, l2, arc_a]
    }

    pub fn is_construction(&self, id: EntityId) -> bool {
        self.construction.contains(&id)
    }

    pub fn construction_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.construction.iter().copied()
    }

    /// Marks an entity as construction geometry (or back to regular).
    pub fn set_construction(
        &mut self,
        id: EntityId,
        construction: bool,
    ) -> Result<(), SketchError> {
        if !self.entities.contains_key(&id) {
            return Err(SketchError::UnknownEntity(id));
        }
        if construction {
            self.construction.insert(id);
        } else {
            self.construction.remove(&id);
        }
        Ok(())
    }

    pub fn add_constraint(&mut self, c: Constraint) -> ConstraintId {
        let id = ConstraintId(self.next_constraint);
        self.next_constraint += 1;
        self.constraints.insert(id, c);
        id
    }

    pub fn remove_constraint(&mut self, id: ConstraintId) -> Option<Constraint> {
        self.constraints.remove(&id)
    }

    /// Removes an entity together with every constraint that references it.
    /// Removing a point that a line/arc/circle depends on removes those too.
    pub fn remove_entity(&mut self, id: EntityId) {
        let Some(_) = self.entities.remove(&id) else {
            return;
        };
        let dependents: Vec<EntityId> = self
            .entities
            .iter()
            .filter(|(_, e)| e.references().contains(&id))
            .map(|(k, _)| *k)
            .collect();
        for d in dependents {
            self.remove_entity(d);
        }
        self.constraints
            .retain(|_, c| !c.references().contains(&id));
        self.construction.remove(&id);
        self.projected.remove(&id);
    }

    /// Replaces the numeric value of a dimensional constraint.
    pub fn set_constraint_value(&mut self, id: ConstraintId, value: f64) -> bool {
        match self.constraints.get_mut(&id) {
            Some(c) => c.set_value(value),
            None => false,
        }
    }

    /// Moves a point without solving (used for dragging).
    pub fn set_point_pub(&mut self, id: EntityId, pos: Vec2) {
        self.set_point(id, pos);
    }

    pub(crate) fn set_point(&mut self, id: EntityId, pos: Vec2) {
        if let Some(Entity::Point { pos: p }) = self.entities.get_mut(&id) {
            *p = pos;
        }
    }

    pub(crate) fn set_radius(&mut self, id: EntityId, r: f64) {
        if let Some(Entity::Circle { radius, .. }) = self.entities.get_mut(&id) {
            *radius = r;
        }
    }
}
