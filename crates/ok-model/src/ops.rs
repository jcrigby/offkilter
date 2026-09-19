use crate::{
    BlendFeature, BlendKind, BodyOp, CopyOp, Counterbore, EdgeRef, ExtrudeDirection, ExtrudeEnd,
    ExtrudeFeature, FeatureId, FeatureKind, HoleFeature, LoftFeature, MirrorFeature, ModelError,
    PartStudio, PatternFeature, PatternKind, PlaneRef, ProfileSelection, Projection,
    ProjectionSource, RevolveAxis, RevolveFeature, SketchFeature, SweepFeature, VariableFeature,
    PROJECTION_BLOCK,
};
use ok_math::Vec2;
use ok_sketch::{Constraint, ConstraintId, EntityId};
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
    /// Project body geometry into the sketch ("Use"). The entities are
    /// built by regeneration and follow the model.
    Project {
        source: ProjectionSource,
    },
    /// Remove a projection (by index) and the entities it built.
    RemoveProjection {
        index: usize,
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
        name: Option<String>,
    },
    SetMirror {
        id: FeatureId,
        #[serde(default)]
        plane: Option<PlaneRef>,
        #[serde(default)]
        op: Option<CopyOp>,
    },
    AddPattern {
        kind: PatternKind,
        count: u32,
        #[serde(default)]
        op: CopyOp,
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
            Op::AddMirror { plane, op, name } => {
                out.feature =
                    Some(self.push_feature(FeatureKind::Mirror(MirrorFeature { plane, op }), name));
            }
            Op::SetMirror { id, plane, op } => match &mut self.feature_mut(id)?.kind {
                FeatureKind::Mirror(m) => {
                    if let Some(p) = plane {
                        m.plane = p;
                    }
                    if let Some(o) = op {
                        m.op = o;
                    }
                }
                _ => return Err(ModelError::WrongFeatureKind(id, "mirror")),
            },
            Op::AddPattern {
                kind,
                count,
                op,
                name,
            } => {
                out.feature = Some(self.push_feature(
                    FeatureKind::Pattern(PatternFeature { kind, count, op }),
                    name,
                ));
            }
            Op::SetPattern {
                id,
                kind,
                count,
                op,
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
                        SketchOp::Project { .. } | SketchOp::RemoveProjection { .. } => {
                            unreachable!()
                        }
                    }
                }
            },
            Op::RenameStudio { name } => self.name = name,
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
