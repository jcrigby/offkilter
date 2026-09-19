use crate::{
    BlendFeature, BlendKind, BodyOp, EdgeRef, ExtrudeDirection, ExtrudeEnd, ExtrudeFeature,
    FeatureId, FeatureKind, ModelError, PartStudio, PlaneRef, ProfileSelection, RevolveAxis,
    RevolveFeature, SketchFeature,
};
use ok_math::Vec2;
use ok_sketch::{Constraint, ConstraintId, EntityId, Sketch};
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
                out.feature = Some(self.push_feature(
                    FeatureKind::Sketch(SketchFeature {
                        plane,
                        sketch: Sketch::new(),
                    }),
                    name,
                ));
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
            Op::Sketch { id, op } => {
                let sk = self.sketch_mut(id)?;
                match op {
                    SketchOp::AddPoint { pos } => out.entities.push(sk.add_point(pos)),
                    SketchOp::AddLine { a, b } => {
                        let (l, s, e) = sk.add_line(a, b);
                        out.entities.extend([l, s, e]);
                    }
                    SketchOp::AddRectangle { a, b } => out.entities.extend(sk.add_rectangle(a, b)),
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
                    SketchOp::RemoveEntity { id } => sk.remove_entity(id),
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
                }
            }
            Op::RenameStudio { name } => self.name = name,
        }
        Ok(out)
    }

    /// Parses and applies an op given as JSON.
    pub fn apply_json(&mut self, op: &str) -> Result<OpResult, ModelError> {
        let op: Op = serde_json::from_str(op).map_err(|e| ModelError::Invalid(e.to_string()))?;
        self.apply(op)
    }
}
