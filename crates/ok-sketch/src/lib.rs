//! 2D parametric sketches.
//!
//! A [`Sketch`] is a set of geometric [`Entity`]s (points, lines, circles,
//! arcs) and [`Constraint`]s between them. [`Sketch::solve`] moves the free
//! geometry so that every constraint is satisfied, and
//! [`Sketch::profiles`] extracts the closed regions bounded by the solved
//! geometry so they can be fed to 3D features such as extrude.

mod entity;
mod loops;
mod solver;

pub use entity::{Constraint, ConstraintId, Entity, EntityId};
pub use loops::{point_in_polygon, signed_area, Loop, Profile, ProfileOptions, SegmentCurve};
pub use solver::{SolveResult, SolveStatus};

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
}

/// A 2D sketch: geometry plus constraints, in plane coordinates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    pub fn is_projected(&self, id: EntityId) -> bool {
        self.projected.contains(&id)
    }

    pub fn projected_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.projected.iter().copied()
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
