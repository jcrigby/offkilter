use crate::{
    BlendFeature, BlendKind, BodyOp, BooleanFeature, BooleanOp, CopyOp, Counterbore, DraftFeature,
    EdgeRef, ExtrudeDirection, ExtrudeEnd, ExtrudeFeature, FaceRef, Feature, FeatureId,
    FeatureKind, HoleFeature, LoftFeature, MeshFeature, MirrorFeature, ModelError, MoveFaceFeature,
    PartStudio, PatternFeature, PatternKind, PlaneRef, ProfileSelection, Projection,
    ProjectionSource, RevolveAxis, RevolveFeature, ShellFeature, SketchFeature, SweepFeature,
    VariableFeature, PROJECTION_BLOCK,
};
use ok_math::{Vec2, Vec3};
use ok_sketch::{Constraint, ConstraintId, Entity, EntityId};
use serde::{Deserialize, Serialize};

/// Edits to a sketch feature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SketchOp {
    AddPoint {
        pos: Vec2,
    },
    AddLine {
        a: Vec2,
        b: Vec2,
    },
    AddRectangle {
        a: Vec2,
        b: Vec2,
    },
    AddCircle {
        center: Vec2,
        radius: f64,
    },
    AddArc {
        center: Vec2,
        start: Vec2,
        end: Vec2,
    },
    /// Smooth open curve through `points` in order (at least two).
    AddSpline {
        points: Vec<Vec2>,
    },
    /// Regular polygon centred on `center` with a corner at `vertex`.
    AddPolygon {
        center: Vec2,
        vertex: Vec2,
        sides: u32,
    },
    /// Slot between centres `a` and `b` of the given width.
    AddSlot {
        a: Vec2,
        b: Vec2,
        width: f64,
    },
    AddConstraint {
        constraint: Constraint,
    },
    RemoveConstraint {
        id: ConstraintId,
    },
    RemoveEntity {
        id: EntityId,
    },
    SetConstraintValue {
        id: ConstraintId,
        value: f64,
    },
    /// Move a point (e.g. while dragging); the solver will pull it back
    /// toward a consistent state on the next regeneration.
    MovePoint {
        id: EntityId,
        pos: Vec2,
    },
    /// Mark geometry as construction (used by constraints and axes, not regions).
    SetConstruction {
        id: EntityId,
        construction: bool,
    },
    /// Remove the piece of a curve nearest `at`, between its intersections.
    Trim {
        entity: EntityId,
        at: Vec2,
    },
    /// Offset a connected chain of lines and arcs (or a circle) to the left
    /// of the first entity's direction; negative distances go right.
    Offset {
        entities: Vec<EntityId>,
        distance: f64,
    },
    /// Round the corner where two lines meet with a tangent arc.
    Fillet {
        a: EntityId,
        b: EntityId,
        radius: f64,
    },
    /// Mirror entities across a line with symmetric constraints.
    Mirror {
        entities: Vec<EntityId>,
        axis: EntityId,
    },
    /// Copies entities along a step, `count` in total, tied to the originals.
    PatternLinear {
        entities: Vec<EntityId>,
        count: u32,
        step: Vec2,
    },
    /// Copies entities around a centre, `count` in total, `angle` degrees apart.
    PatternCircular {
        entities: Vec<EntityId>,
        count: u32,
        center: Vec2,
        angle: f64,
    },
    /// Project body geometry into the sketch ("Use"). The entities are
    /// built by regeneration and follow the model.
    Project {
        source: ProjectionSource,
    },
    /// Remove a projection (by index) and the entities it built.
    RemoveProjection {
        index: usize,
    },
    /// Puts entities, constraints and flags back exactly as given (the
    /// inverse of any other sketch op, computed by `apply_with_inverse`).
    /// Removals happen first, then inserts overwrite by id.
    Restore {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<EntityId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove_constraints: Vec<ConstraintId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<(EntityId, Entity)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        constraints: Vec<(ConstraintId, Constraint)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        construction: Vec<(EntityId, bool)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        projected: Vec<(EntityId, bool)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projections: Option<Vec<Projection>>,
    },
}

/// Edits to a part studio. Every mutation goes through here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Op {
    AddSketch {
        plane: PlaneRef,
        name: Option<String>,
    },
    AddExtrude {
        sketch: FeatureId,
        depth: f64,
        #[serde(default = "default_direction")]
        direction: ExtrudeDirection,
        #[serde(default)]
        end: ExtrudeEnd,
        #[serde(default = "default_profiles")]
        profiles: ProfileSelection,
        #[serde(default = "default_body_op")]
        op: BodyOp,
        name: Option<String>,
    },
    SetExtrude {
        id: FeatureId,
        #[serde(default)]
        depth: Option<f64>,
        #[serde(default)]
        direction: Option<ExtrudeDirection>,
        #[serde(default)]
        end: Option<ExtrudeEnd>,
        #[serde(default)]
        profiles: Option<ProfileSelection>,
        #[serde(default)]
        op: Option<BodyOp>,
    },
    AddRevolve {
        sketch: FeatureId,
        axis: RevolveAxis,
        #[serde(default = "default_angle")]
        angle: f64,
        #[serde(default = "default_profiles")]
        profiles: ProfileSelection,
        #[serde(default = "default_body_op")]
        op: BodyOp,
        name: Option<String>,
    },
    SetRevolve {
        id: FeatureId,
        #[serde(default)]
        axis: Option<RevolveAxis>,
        #[serde(default)]
        angle: Option<f64>,
        #[serde(default)]
        profiles: Option<ProfileSelection>,
        #[serde(default)]
        op: Option<BodyOp>,
    },
    AddBlend {
        kind: BlendKind,
        #[serde(default)]
        edges: Vec<EdgeRef>,
        size: f64,
        name: Option<String>,
    },
    SetBlend {
        id: FeatureId,
        #[serde(default)]
        edges: Option<Vec<EdgeRef>>,
        #[serde(default)]
        size: Option<f64>,
    },
    AddMirror {
        plane: PlaneRef,
        #[serde(default)]
        op: CopyOp,
        #[serde(default)]
        features: Vec<FeatureId>,
        #[serde(default)]
        bodies: Vec<FeatureId>,
        name: Option<String>,
    },
    SetMirror {
        id: FeatureId,
        #[serde(default)]
        plane: Option<PlaneRef>,
        #[serde(default)]
        op: Option<CopyOp>,
        #[serde(default)]
        features: Option<Vec<FeatureId>>,
        #[serde(default)]
        bodies: Option<Vec<FeatureId>>,
    },
    AddPattern {
        kind: PatternKind,
        count: u32,
        #[serde(default)]
        op: CopyOp,
        #[serde(default)]
        features: Vec<FeatureId>,
        #[serde(default)]
        bodies: Vec<FeatureId>,
        name: Option<String>,
    },
    SetPattern {
        id: FeatureId,
        #[serde(default)]
        kind: Option<PatternKind>,
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        op: Option<CopyOp>,
        #[serde(default)]
        features: Option<Vec<FeatureId>>,
        #[serde(default)]
        bodies: Option<Vec<FeatureId>>,
    },
    AddVariable {
        name: String,
        expression: String,
    },
    SetVariable {
        id: FeatureId,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        expression: Option<String>,
    },
    /// Binds an expression to a numeric field, or clears it with `None`.
    SetBinding {
        id: FeatureId,
        field: String,
        expression: Option<String>,
    },
    /// Sets a bindable numeric field by name (see `FeatureKind::set_field`).
    SetField {
        id: FeatureId,
        field: String,
        value: f64,
    },
    AddHole {
        sketch: FeatureId,
        diameter: f64,
        #[serde(default)]
        depth: f64,
        #[serde(default)]
        through_all: bool,
        #[serde(default = "default_reverse")]
        direction: ExtrudeDirection,
        #[serde(default)]
        counterbore: Option<Counterbore>,
        name: Option<String>,
    },
    SetHole {
        id: FeatureId,
        #[serde(default)]
        diameter: Option<f64>,
        #[serde(default)]
        depth: Option<f64>,
        #[serde(default)]
        through_all: Option<bool>,
        #[serde(default)]
        direction: Option<ExtrudeDirection>,
        /// `Some(None)` clears the counterbore; `None` leaves it as is.
        #[serde(default, with = "double_option")]
        counterbore: Option<Option<Counterbore>>,
    },
    AddSweep {
        sketch: FeatureId,
        path: FeatureId,
        #[serde(default = "default_profiles")]
        profiles: ProfileSelection,
        #[serde(default = "default_body_op")]
        op: BodyOp,
        name: Option<String>,
    },
    SetSweep {
        id: FeatureId,
        #[serde(default)]
        path: Option<FeatureId>,
        #[serde(default)]
        profiles: Option<ProfileSelection>,
        #[serde(default)]
        op: Option<BodyOp>,
    },
    AddLoft {
        sketch: FeatureId,
        sketch_b: FeatureId,
        #[serde(default = "default_body_op")]
        op: BodyOp,
        name: Option<String>,
    },
    SetLoft {
        id: FeatureId,
        #[serde(default)]
        sketch_b: Option<FeatureId>,
        #[serde(default)]
        op: Option<BodyOp>,
    },
    AddBoolean {
        op: BooleanOp,
        targets: Vec<FeatureId>,
        tools: Vec<FeatureId>,
        #[serde(default)]
        keep_tools: bool,
        name: Option<String>,
    },
    SetBoolean {
        id: FeatureId,
        #[serde(default)]
        op: Option<BooleanOp>,
        #[serde(default)]
        targets: Option<Vec<FeatureId>>,
        #[serde(default)]
        tools: Option<Vec<FeatureId>>,
        #[serde(default)]
        keep_tools: Option<bool>,
    },
    AddShell {
        thickness: f64,
        #[serde(default)]
        faces: Vec<FaceRef>,
        name: Option<String>,
    },
    SetShell {
        id: FeatureId,
        #[serde(default)]
        thickness: Option<f64>,
        #[serde(default)]
        faces: Option<Vec<FaceRef>>,
    },
    AddMoveFace {
        #[serde(default)]
        faces: Vec<FaceRef>,
        distance: f64,
        name: Option<String>,
    },
    SetMoveFace {
        id: FeatureId,
        #[serde(default)]
        faces: Option<Vec<FaceRef>>,
        #[serde(default)]
        distance: Option<f64>,
    },
    AddDraft {
        #[serde(default)]
        faces: Vec<FaceRef>,
        neutral: PlaneRef,
        angle: f64,
        name: Option<String>,
    },
    /// A body from a closed triangle mesh (imported STL).
    AddMesh {
        vertices: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        name: Option<String>,
    },
    SetDraft {
        id: FeatureId,
        #[serde(default)]
        faces: Option<Vec<FaceRef>>,
        #[serde(default)]
        neutral: Option<PlaneRef>,
        #[serde(default)]
        angle: Option<f64>,
    },
    /// Sets document-wide regeneration settings.
    SetSettings {
        facet_angle: f64,
    },
    /// Replaces the whole document (used to sync undo/redo between clients).
    ReplaceDocument {
        json: String,
    },
    SetSketchPlane {
        id: FeatureId,
        plane: PlaneRef,
    },
    RenameFeature {
        id: FeatureId,
        name: String,
    },
    SetSuppressed {
        id: FeatureId,
        suppressed: bool,
    },
    DeleteFeature {
        id: FeatureId,
    },
    /// Puts a feature back at `index` with its id and state (the inverse of
    /// `DeleteFeature`).
    InsertFeature {
        index: usize,
        feature: Feature,
    },
    /// Move a feature to a new index in the feature list.
    MoveFeature {
        id: FeatureId,
        index: usize,
    },
    Sketch {
        id: FeatureId,
        op: SketchOp,
    },
    RenameStudio {
        name: String,
    },
    /// Names the part(s) created by a feature; `None` restores "Part N".
    RenamePart {
        source: FeatureId,
        name: Option<String>,
    },
}

fn default_reverse() -> ExtrudeDirection {
    ExtrudeDirection::Reverse
}

/// Serde helper distinguishing "absent" from "explicitly null".
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(
        v: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match v {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

fn default_angle() -> f64 {
    360.0
}
fn default_direction() -> ExtrudeDirection {
    ExtrudeDirection::Normal
}
fn default_profiles() -> ProfileSelection {
    ProfileSelection::All
}
fn default_body_op() -> BodyOp {
    BodyOp::New
}

/// What an [`Op`] created, so callers can refer to it afterwards.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpResult {
    pub feature: Option<FeatureId>,
    pub entities: Vec<EntityId>,
    pub constraint: Option<ConstraintId>,
    /// Ops that undo this one, in the order to apply them (filled by
    /// `apply_with_inverse`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inverse: Vec<Op>,
}

impl PartStudio {
    pub fn apply(&mut self, op: Op) -> Result<OpResult, ModelError> {
        let mut out = OpResult::default();
        match op {
            Op::AddSketch { plane, name } => {
                out.feature =
                    Some(self.push_feature(FeatureKind::Sketch(SketchFeature::new(plane)), name));
            }
            Op::AddExtrude {
                sketch,
                depth,
                direction,
                end,
                profiles,
                op,
                name,
            } => {
                match &self.feature(sketch)?.kind {
                    FeatureKind::Sketch(_) => {}
                    _ => return Err(ModelError::WrongFeatureKind(sketch, "sketch")),
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Extrude(ExtrudeFeature {
                        sketch,
                        profiles,
                        depth,
                        direction,
                        end,
                        op,
                    }),
                    name,
                ));
            }
            Op::SetExtrude {
                id,
                depth,
                direction,
                end,
                profiles,
                op,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Extrude(e) => {
                    if let Some(d) = depth {
                        e.depth = d;
                    }
                    if let Some(d) = direction {
                        e.direction = d;
                    }
                    if let Some(x) = end {
                        e.end = x;
                    }
                    if let Some(p) = profiles {
                        e.profiles = p;
                    }
                    if let Some(o) = op {
                        e.op = o;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "extrude")),
            },
            Op::AddRevolve {
                sketch,
                axis,
                angle,
                profiles,
                op,
                name,
            } => {
                match &self.feature(sketch)?.kind {
                    FeatureKind::Sketch(_) => {}
                    _ => return Err(ModelError::WrongFeatureKind(sketch, "sketch")),
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Revolve(RevolveFeature {
                        sketch,
                        profiles,
                        axis,
                        angle,
                        op,
                    }),
                    name,
                ));
            }
            Op::SetRevolve {
                id,
                axis,
                angle,
                profiles,
                op,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Revolve(r) => {
                    if let Some(a) = axis {
                        r.axis = a;
                    }
                    if let Some(a) = angle {
                        r.angle = a;
                    }
                    if let Some(p) = profiles {
                        r.profiles = p;
                    }
                    if let Some(o) = op {
                        r.op = o;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "revolve")),
            },
            Op::AddBlend {
                kind,
                edges,
                size,
                name,
            } => {
                out.feature = Some(
                    self.push_feature(FeatureKind::Blend(BlendFeature { kind, edges, size }), name),
                );
            }
            Op::SetBlend { id, edges, size } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Blend(b) => {
                    if let Some(e) = edges {
                        b.edges = e;
                    }
                    if let Some(s) = size {
                        b.size = s;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "blend")),
            },
            Op::AddMirror {
                plane,
                op,
                features,
                bodies,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::Mirror(MirrorFeature {
                        plane,
                        op,
                        features,
                        bodies,
                    }),
                    name,
                ));
            }
            Op::SetMirror {
                id,
                plane,
                op,
                features,
                bodies,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Mirror(m) => {
                    if let Some(p) = plane {
                        m.plane = p;
                    }
                    if let Some(o) = op {
                        m.op = o;
                    }
                    if let Some(f) = features {
                        m.features = f;
                    }
                    if let Some(b) = bodies {
                        m.bodies = b;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "mirror")),
            },
            Op::AddPattern {
                kind,
                count,
                op,
                features,
                bodies,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::Pattern(PatternFeature {
                        kind,
                        count,
                        op,
                        features,
                        bodies,
                    }),
                    name,
                ));
            }
            Op::SetPattern {
                id,
                kind,
                count,
                op,
                features,
                bodies,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Pattern(p) => {
                    if let Some(k) = kind {
                        p.kind = k;
                    }
                    if let Some(c) = count {
                        p.count = c;
                    }
                    if let Some(o) = op {
                        p.op = o;
                    }
                    if let Some(f) = features {
                        p.features = f;
                    }
                    if let Some(b) = bodies {
                        p.bodies = b;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "pattern")),
            },
            Op::AddVariable { name, expression } => {
                let label = format!("#{name}");
                out.feature = Some(self.push_feature(
                    FeatureKind::Variable(VariableFeature { name, expression }),
                    Some(label),
                ));
            }
            Op::SetVariable {
                id,
                name,
                expression,
            } => {
                let f = self.feature_mut(id)?;
                match &mut f.kind {
                    FeatureKind::Variable(v) => {
                        if let Some(n) = name {
                            v.name = n.clone();
                            f.name = format!("#{n}");
                        }
                        if let Some(e) = expression {
                            v.expression = e;
                        }
                    }
                    _ => return Err(ModelError::WrongFeatureKind(id, "variable")),
                }
            }
            Op::SetField { id, field, value } => {
                if !value.is_finite() {
                    return Err(ModelError::Invalid(format!("{field} must be finite")));
                }
                self.feature_mut(id)?
                    .kind
                    .set_field(&field, value)
                    .map_err(ModelError::Invalid)?;
            }
            Op::SetBinding {
                id,
                field,
                expression,
            } => {
                let f = self.feature_mut(id)?;
                if !f.kind.bindable_fields().contains(&field) {
                    return Err(ModelError::Invalid(format!(
                        "field '{field}' cannot be bound"
                    )));
                }
                match expression {
                    Some(e) => {
                        f.bindings.insert(field, e);
                    }
                    None => {
                        f.bindings.remove(&field);
                    }
                }
            }
            Op::AddHole {
                sketch,
                diameter,
                depth,
                through_all,
                direction,
                counterbore,
                name,
            } => {
                match &self.feature(sketch)?.kind {
                    FeatureKind::Sketch(_) => {}
                    _ => return Err(ModelError::WrongFeatureKind(sketch, "sketch")),
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Hole(HoleFeature {
                        sketch,
                        diameter,
                        depth,
                        through_all,
                        direction,
                        counterbore,
                    }),
                    name,
                ));
            }
            Op::SetHole {
                id,
                diameter,
                depth,
                through_all,
                direction,
                counterbore,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Hole(h) => {
                    if let Some(v) = diameter {
                        h.diameter = v;
                    }
                    if let Some(v) = depth {
                        h.depth = v;
                    }
                    if let Some(v) = through_all {
                        h.through_all = v;
                    }
                    if let Some(v) = direction {
                        h.direction = v;
                    }
                    if let Some(v) = counterbore {
                        h.counterbore = v;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "hole")),
            },
            Op::AddSweep {
                sketch,
                path,
                profiles,
                op,
                name,
            } => {
                for id in [sketch, path] {
                    match &self.feature(id)?.kind {
                        FeatureKind::Sketch(_) => {}
                        _ => return Err(ModelError::WrongFeatureKind(id, "sketch")),
                    }
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Sweep(SweepFeature {
                        sketch,
                        profiles,
                        path,
                        op,
                    }),
                    name,
                ));
            }
            Op::SetSweep {
                id,
                path,
                profiles,
                op,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Sweep(sw) => {
                    if let Some(p) = path {
                        sw.path = p;
                    }
                    if let Some(p) = profiles {
                        sw.profiles = p;
                    }
                    if let Some(o) = op {
                        sw.op = o;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "sweep")),
            },
            Op::AddLoft {
                sketch,
                sketch_b,
                op,
                name,
            } => {
                for id in [sketch, sketch_b] {
                    match &self.feature(id)?.kind {
                        FeatureKind::Sketch(_) => {}
                        _ => return Err(ModelError::WrongFeatureKind(id, "sketch")),
                    }
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Loft(LoftFeature {
                        sketch,
                        sketch_b,
                        op,
                    }),
                    name,
                ));
            }
            Op::AddBoolean {
                op,
                targets,
                tools,
                keep_tools,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::Boolean(BooleanFeature {
                        op,
                        targets,
                        tools,
                        keep_tools,
                    }),
                    name,
                ));
            }
            Op::SetBoolean {
                id,
                op,
                targets,
                tools,
                keep_tools,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Boolean(b) => {
                    if let Some(o) = op {
                        b.op = o;
                    }
                    if let Some(t) = targets {
                        b.targets = t;
                    }
                    if let Some(t) = tools {
                        b.tools = t;
                    }
                    if let Some(k) = keep_tools {
                        b.keep_tools = k;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "boolean")),
            },
            Op::AddShell {
                thickness,
                faces,
                name,
            } => {
                out.feature = Some(
                    self.push_feature(FeatureKind::Shell(ShellFeature { thickness, faces }), name),
                );
            }
            Op::SetShell {
                id,
                thickness,
                faces,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Shell(sh) => {
                    if let Some(t) = thickness {
                        sh.thickness = t;
                    }
                    if let Some(f) = faces {
                        sh.faces = f;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "shell")),
            },
            Op::AddMoveFace {
                faces,
                distance,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::MoveFace(MoveFaceFeature { faces, distance }),
                    name,
                ));
            }
            Op::SetMoveFace {
                id,
                faces,
                distance,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::MoveFace(m) => {
                    if let Some(f) = faces {
                        m.faces = f;
                    }
                    if let Some(d) = distance {
                        m.distance = d;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "move face")),
            },
            Op::AddMesh {
                vertices,
                triangles,
                name,
            } => {
                for t in &triangles {
                    if t.iter().any(|&i| i as usize >= vertices.len()) {
                        return Err(ModelError::Invalid(
                            "mesh triangle refers to a missing vertex".into(),
                        ));
                    }
                }
                out.feature = Some(self.push_feature(
                    FeatureKind::Mesh(MeshFeature {
                        vertices,
                        triangles,
                    }),
                    name,
                ));
            }
            Op::AddDraft {
                faces,
                neutral,
                angle,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::Draft(DraftFeature {
                        faces,
                        neutral,
                        angle,
                    }),
                    name,
                ));
            }
            Op::SetDraft {
                id,
                faces,
                neutral,
                angle,
            } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Draft(d) => {
                    if let Some(f) = faces {
                        d.faces = f;
                    }
                    if let Some(n) = neutral {
                        d.neutral = n;
                    }
                    if let Some(a) = angle {
                        d.angle = a;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "draft")),
            },
            Op::SetLoft { id, sketch_b, op } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Loft(l) => {
                    if let Some(b) = sketch_b {
                        l.sketch_b = b;
                    }
                    if let Some(o) = op {
                        l.op = o;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "loft")),
            },
            Op::SetSettings { facet_angle } => {
                if !(facet_angle.is_finite() && (0.5..=30.0).contains(&facet_angle)) {
                    return Err(ModelError::Invalid(
                        "facet angle must be between 0.5 and 30 degrees".into(),
                    ));
                }
                self.settings.facet_angle = facet_angle;
            }
            Op::ReplaceDocument { json } => {
                let replacement =
                    PartStudio::from_json(&json).map_err(|e| ModelError::Invalid(e.to_string()))?;
                *self = replacement;
            }
            Op::SetSketchPlane { id, plane } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Sketch(s) => s.plane = plane,
                _ => return Err(ModelError::WrongFeatureKind(id, "sketch")),
            },
            Op::RenameFeature { id, name } => self.feature_mut(id)?.name = name,
            Op::SetSuppressed { id, suppressed } => self.feature_mut(id)?.suppressed = suppressed,
            Op::DeleteFeature { id } => {
                let pos = self.position(id).ok_or(ModelError::UnknownFeature(id))?;
                self.features.remove(pos);
                // Dependent features lose their input; they will report an
                // error at regeneration rather than being deleted silently.
            }
            Op::MoveFeature { id, index } => {
                let pos = self.position(id).ok_or(ModelError::UnknownFeature(id))?;
                let f = self.features.remove(pos);
                let index = index.min(self.features.len());
                self.features.insert(index, f);
            }
            Op::InsertFeature { index, feature } => {
                if self.position(feature.id).is_some() {
                    return Err(ModelError::Invalid(format!(
                        "feature {:?} already exists",
                        feature.id
                    )));
                }
                self.next_id = self.next_id.max(feature.id.0 + 1);
                let index = index.min(self.features.len());
                out.feature = Some(feature.id);
                self.features.insert(index, feature);
            }
            Op::Sketch { id, op } => match op {
                SketchOp::Project { source } => {
                    let sf = self.sketch_feature_mut(id)?;
                    let block = sf.sketch.reserve_entity_ids(PROJECTION_BLOCK);
                    sf.projections.push(Projection {
                        source,
                        block,
                        entities: Vec::new(),
                    });
                }
                SketchOp::RemoveProjection { index } => {
                    let sf = self.sketch_feature_mut(id)?;
                    if index >= sf.projections.len() {
                        return Err(ModelError::Invalid(format!("no projection {index}")));
                    }
                    let p = sf.projections.remove(index);
                    for e in p.entities {
                        sf.sketch.remove_entity(e);
                    }
                }
                SketchOp::Restore {
                    remove,
                    remove_constraints,
                    entities,
                    constraints,
                    construction,
                    projected,
                    projections,
                } => {
                    let sf = self.sketch_feature_mut(id)?;
                    for e in remove {
                        sf.sketch.remove_entity(e);
                    }
                    for c in remove_constraints {
                        sf.sketch.remove_constraint(c);
                    }
                    // Points before curves so nothing dangles in between.
                    let (points, curves): (Vec<_>, Vec<_>) = entities
                        .into_iter()
                        .partition(|(_, e)| matches!(e, Entity::Point { .. }));
                    for (eid, e) in points.into_iter().chain(curves) {
                        sf.sketch.insert_entity_with_id(eid, e);
                    }
                    for (cid, c) in constraints {
                        sf.sketch.insert_constraint_with_id(cid, c);
                    }
                    for (eid, on) in construction {
                        let _ = sf.sketch.set_construction(eid, on);
                    }
                    for (eid, on) in projected {
                        sf.sketch.set_projected(eid, on);
                    }
                    if let Some(p) = projections {
                        sf.projections = p;
                    }
                }
                op => {
                    let sk = self.sketch_mut(id)?;
                    match op {
                        SketchOp::AddPoint { pos } => out.entities.push(sk.add_point(pos)),
                        SketchOp::AddLine { a, b } => {
                            let (l, s, e) = sk.add_line(a, b);
                            out.entities.extend([l, s, e]);
                        }
                        SketchOp::AddRectangle { a, b } => {
                            out.entities.extend(sk.add_rectangle(a, b))
                        }
                        SketchOp::AddCircle { center, radius } => {
                            let (c, p) = sk.add_circle(center, radius);
                            out.entities.extend([c, p]);
                        }
                        SketchOp::AddArc { center, start, end } => {
                            let (a, c, s, e) = sk.add_arc(center, start, end);
                            out.entities.extend([a, c, s, e]);
                        }
                        SketchOp::AddSpline { points } => {
                            let (id, pts) = sk.add_spline(&points)?;
                            out.entities.push(id);
                            out.entities.extend(pts);
                        }
                        SketchOp::AddPolygon {
                            center,
                            vertex,
                            sides,
                        } => {
                            if !(3..=64).contains(&sides) {
                                return Err(ModelError::Invalid(
                                    "a polygon needs between 3 and 64 sides".into(),
                                ));
                            }
                            out.entities.extend(sk.add_regular_polygon(
                                center,
                                vertex,
                                sides as usize,
                            ));
                        }
                        SketchOp::AddSlot { a, b, width } => {
                            if width <= 0.0 || !width.is_finite() || (b - a).length() < 1e-9 {
                                return Err(ModelError::Invalid(
                                    "a slot needs distinct centres and a positive width".into(),
                                ));
                            }
                            out.entities.extend(sk.add_slot(a, b, width));
                        }
                        SketchOp::AddConstraint { constraint } => {
                            for r in constraint.references() {
                                if sk.entity(r).is_none() {
                                    return Err(ModelError::Sketch(
                                        ok_sketch::SketchError::UnknownEntity(r),
                                    ));
                                }
                            }
                            out.constraint = Some(sk.add_constraint(constraint));
                        }
                        SketchOp::RemoveConstraint { id } => {
                            sk.remove_constraint(id);
                        }
                        SketchOp::RemoveEntity { id } => {
                            if sk.is_projected(id) {
                                return Err(ModelError::Invalid(
                                "entity is projected from the model; remove the projection instead"
                                    .into(),
                            ));
                            }
                            sk.remove_entity(id)
                        }
                        SketchOp::SetConstraintValue { id, value } => {
                            if !sk.set_constraint_value(id, value) {
                                return Err(ModelError::Invalid(format!(
                                    "constraint {id:?} has no value"
                                )));
                            }
                        }
                        SketchOp::MovePoint { id, pos } => {
                            sk.point(id)?;
                            sk.set_point_pub(id, pos);
                        }
                        SketchOp::SetConstruction { id, construction } => {
                            sk.set_construction(id, construction)?;
                        }
                        SketchOp::Trim { entity, at } => {
                            if sk.is_projected(entity) {
                                return Err(ModelError::Invalid(
                                    "projected geometry cannot be trimmed".into(),
                                ));
                            }
                            out.entities.extend(sk.trim(entity, at)?);
                        }
                        SketchOp::Offset { entities, distance } => {
                            if !distance.is_finite() || distance.abs() <= 1e-9 {
                                return Err(ModelError::Invalid(
                                    "offset distance must be non-zero".into(),
                                ));
                            }
                            out.entities.extend(sk.offset(&entities, distance)?);
                        }
                        SketchOp::Mirror { entities, axis } => {
                            out.entities.extend(sk.mirror(&entities, axis)?);
                        }
                        SketchOp::Fillet { a, b, radius } => {
                            out.entities.push(sk.fillet(a, b, radius)?);
                        }
                        SketchOp::PatternLinear {
                            entities,
                            count,
                            step,
                        } => {
                            if !(2..=200).contains(&count) {
                                return Err(ModelError::Invalid(
                                    "a pattern needs between 2 and 200 copies".into(),
                                ));
                            }
                            out.entities.extend(sk.pattern_linear(
                                &entities,
                                count as usize,
                                step,
                            )?);
                        }
                        SketchOp::PatternCircular {
                            entities,
                            count,
                            center,
                            angle,
                        } => {
                            if !(2..=200).contains(&count) {
                                return Err(ModelError::Invalid(
                                    "a pattern needs between 2 and 200 copies".into(),
                                ));
                            }
                            out.entities.extend(sk.pattern_circular(
                                &entities,
                                count as usize,
                                center,
                                angle,
                            )?);
                        }
                        SketchOp::Project { .. }
                        | SketchOp::RemoveProjection { .. }
                        | SketchOp::Restore { .. } => unreachable!(),
                    }
                }
            },
            Op::RenameStudio { name } => self.name = name,
            Op::RenamePart { source, name } => match name.map(|n| n.trim().to_string()) {
                Some(n) if !n.is_empty() => {
                    self.part_names.insert(source, n);
                }
                _ => {
                    self.part_names.remove(&source);
                }
            },
        }
        Ok(out)
    }

    /// Parses and applies an op given as JSON.
    pub fn apply_json(&mut self, op: &str) -> Result<OpResult, ModelError> {
        let op: Op = serde_json::from_str(op).map_err(|e| ModelError::Invalid(e.to_string()))?;
        self.apply(op)
    }

    /// Applies an op allocating any new ids from `base` upward. Each
    /// collaborating client passes bases from its own range, so concurrent
    /// ops produce the same ids on every replica regardless of order.
    pub fn apply_with_base(&mut self, op: Op, base: Option<u32>) -> Result<OpResult, ModelError> {
        if let Some(b) = base {
            self.next_id = b;
            if let Op::Sketch { id, .. } = &op {
                if let Ok(f) = self.feature_mut(*id) {
                    if let FeatureKind::Sketch(sf) = &mut f.kind {
                        sf.sketch.set_id_base(b);
                    }
                }
            }
        }
        self.apply(op)
    }

    pub fn apply_json_with_base(
        &mut self,
        op: &str,
        base: Option<u32>,
    ) -> Result<OpResult, ModelError> {
        let op: Op = serde_json::from_str(op).map_err(|e| ModelError::Invalid(e.to_string()))?;
        self.apply_with_base(op, base)
    }
}
