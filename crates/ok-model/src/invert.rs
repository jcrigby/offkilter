//! Inverse ops: for every edit, the ops that undo it.
//!
//! Undo in a shared document must not replace the whole document (that
//! would discard everyone else's concurrent edits), so each op is undone
//! by applying its inverse as an ordinary op. Inverses are computed from
//! the state before and after the edit:
//!
//! - creating a feature is undone by deleting it; deleting one by
//!   `InsertFeature` with its saved state at its old index;
//! - `Set*` ops are undone by the same op carrying the old values of the
//!   fields they changed, so concurrent edits to other fields survive;
//! - a sketch op is undone by `SketchOp::Restore`, a patch computed by
//!   diffing the sketch before and after: entities and constraints the op
//!   created are removed, those it removed or changed are put back by id.

use crate::{FeatureKind, ModelError, Op, OpResult, PartStudio, SketchFeature, SketchOp};
use ok_sketch::{ConstraintId, EntityId};

impl PartStudio {
    /// Applies `op` like [`PartStudio::apply_with_base`] and also returns
    /// (in `OpResult::inverse`) the ops that undo it.
    pub fn apply_with_inverse(
        &mut self,
        op: Op,
        base: Option<u32>,
    ) -> Result<OpResult, ModelError> {
        let before = Before::capture(self, &op);
        let mut result = self.apply_with_base(op.clone(), base)?;
        result.inverse = self.invert(op, before, &result);
        Ok(result)
    }

    fn invert(&self, op: Op, before: Before, result: &OpResult) -> Vec<Op> {
        match op {
            // Creations: delete what was created.
            Op::AddSketch { .. }
            | Op::AddExtrude { .. }
            | Op::AddRevolve { .. }
            | Op::AddBlend { .. }
            | Op::AddMirror { .. }
            | Op::AddPattern { .. }
            | Op::AddVariable { .. }
            | Op::AddHole { .. }
            | Op::AddSweep { .. }
            | Op::AddLoft { .. }
            | Op::InsertFeature { .. } => result
                .feature
                .map(|id| vec![Op::DeleteFeature { id }])
                .unwrap_or_default(),
            Op::DeleteFeature { .. } => match before {
                Before::Feature { index, feature } => vec![Op::InsertFeature { index, feature }],
                _ => Vec::new(),
            },
            Op::MoveFeature { id, .. } => match before {
                Before::Feature { index, .. } => vec![Op::MoveFeature { id, index }],
                _ => Vec::new(),
            },
            // Field edits: the same op with the old values.
            Op::SetExtrude {
                id,
                depth,
                direction,
                end,
                profiles,
                op,
            } => match before.kind() {
                Some(FeatureKind::Extrude(e)) => vec![Op::SetExtrude {
                    id,
                    depth: depth.map(|_| e.depth),
                    direction: direction.map(|_| e.direction),
                    end: end.map(|_| e.end),
                    profiles: profiles.map(|_| e.profiles.clone()),
                    op: op.map(|_| e.op),
                }],
                _ => Vec::new(),
            },
            Op::SetRevolve {
                id,
                axis,
                angle,
                profiles,
                op,
            } => match before.kind() {
                Some(FeatureKind::Revolve(r)) => vec![Op::SetRevolve {
                    id,
                    axis: axis.map(|_| r.axis),
                    angle: angle.map(|_| r.angle),
                    profiles: profiles.map(|_| r.profiles.clone()),
                    op: op.map(|_| r.op),
                }],
                _ => Vec::new(),
            },
            Op::SetBlend { id, edges, size } => match before.kind() {
                Some(FeatureKind::Blend(b)) => vec![Op::SetBlend {
                    id,
                    edges: edges.map(|_| b.edges.clone()),
                    size: size.map(|_| b.size),
                }],
                _ => Vec::new(),
            },
            Op::SetMirror { id, plane, op } => match before.kind() {
                Some(FeatureKind::Mirror(m)) => vec![Op::SetMirror {
                    id,
                    plane: plane.map(|_| m.plane),
                    op: op.map(|_| m.op),
                }],
                _ => Vec::new(),
            },
            Op::SetPattern {
                id,
                kind,
                count,
                op,
            } => match before.kind() {
                Some(FeatureKind::Pattern(p)) => vec![Op::SetPattern {
                    id,
                    kind: kind.map(|_| p.kind),
                    count: count.map(|_| p.count),
                    op: op.map(|_| p.op),
                }],
                _ => Vec::new(),
            },
            Op::SetVariable {
                id,
                name,
                expression,
            } => match before.kind() {
                Some(FeatureKind::Variable(v)) => vec![Op::SetVariable {
                    id,
                    name: name.map(|_| v.name.clone()),
                    expression: expression.map(|_| v.expression.clone()),
                }],
                _ => Vec::new(),
            },
            Op::SetHole {
                id,
                diameter,
                depth,
                through_all,
                direction,
                counterbore,
            } => match before.kind() {
                Some(FeatureKind::Hole(h)) => vec![Op::SetHole {
                    id,
                    diameter: diameter.map(|_| h.diameter),
                    depth: depth.map(|_| h.depth),
                    through_all: through_all.map(|_| h.through_all),
                    direction: direction.map(|_| h.direction),
                    counterbore: counterbore.map(|_| h.counterbore),
                }],
                _ => Vec::new(),
            },
            Op::SetSweep {
                id,
                path,
                profiles,
                op,
            } => match before.kind() {
                Some(FeatureKind::Sweep(s)) => vec![Op::SetSweep {
                    id,
                    path: path.map(|_| s.path),
                    profiles: profiles.map(|_| s.profiles.clone()),
                    op: op.map(|_| s.op),
                }],
                _ => Vec::new(),
            },
            Op::SetLoft { id, sketch_b, op } => match before.kind() {
                Some(FeatureKind::Loft(l)) => vec![Op::SetLoft {
                    id,
                    sketch_b: sketch_b.map(|_| l.sketch_b),
                    op: op.map(|_| l.op),
                }],
                _ => Vec::new(),
            },
            // A binding overwrites its field at regeneration, so undoing
            // one also puts the field's old value back.
            Op::SetBinding { id, field, .. } => match &before {
                Before::Feature { feature, .. } => {
                    let mut ops = vec![Op::SetBinding {
                        id,
                        expression: feature.bindings.get(&field).cloned(),
                        field: field.clone(),
                    }];
                    if let Some(value) = feature.kind.field(&field) {
                        ops.push(Op::SetField { id, field, value });
                    }
                    ops
                }
                _ => Vec::new(),
            },
            Op::SetField { id, field, .. } => match &before {
                Before::Feature { feature, .. } => feature
                    .kind
                    .field(&field)
                    .map(|value| vec![Op::SetField { id, field, value }])
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            Op::SetSketchPlane { id, .. } => match before.kind() {
                Some(FeatureKind::Sketch(s)) => vec![Op::SetSketchPlane { id, plane: s.plane }],
                _ => Vec::new(),
            },
            Op::RenameFeature { id, .. } => match &before {
                Before::Feature { feature, .. } => vec![Op::RenameFeature {
                    id,
                    name: feature.name.clone(),
                }],
                _ => Vec::new(),
            },
            Op::SetSuppressed { id, .. } => match &before {
                Before::Feature { feature, .. } => vec![Op::SetSuppressed {
                    id,
                    suppressed: feature.suppressed,
                }],
                _ => Vec::new(),
            },
            Op::SetSettings { .. } => match before {
                Before::Settings(s) => vec![Op::SetSettings {
                    facet_angle: s.facet_angle,
                }],
                _ => Vec::new(),
            },
            Op::RenameStudio { .. } => match before {
                Before::Name(name) => vec![Op::RenameStudio { name }],
                _ => Vec::new(),
            },
            Op::ReplaceDocument { .. } => match before {
                Before::Document(json) => vec![Op::ReplaceDocument { json }],
                _ => Vec::new(),
            },
            Op::Sketch { id, .. } => match (before, self.feature(id)) {
                (Before::Sketch(old), Ok(f)) => match &f.kind {
                    FeatureKind::Sketch(new) => sketch_patch(&old, new)
                        .map(|op| vec![Op::Sketch { id, op }])
                        .unwrap_or_default(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            },
        }
    }
}

/// What an op may change, captured before it is applied.
enum Before {
    None,
    Feature {
        index: usize,
        feature: crate::Feature,
    },
    Sketch(SketchFeature),
    Settings(crate::Settings),
    Name(String),
    Document(String),
}

impl Before {
    fn capture(ps: &PartStudio, op: &Op) -> Before {
        match op {
            Op::Sketch { id, .. } => match ps.feature(*id) {
                Ok(f) => match &f.kind {
                    FeatureKind::Sketch(s) => Before::Sketch(s.clone()),
                    _ => Before::None,
                },
                Err(_) => Before::None,
            },
            Op::SetExtrude { id, .. }
            | Op::SetRevolve { id, .. }
            | Op::SetBlend { id, .. }
            | Op::SetMirror { id, .. }
            | Op::SetPattern { id, .. }
            | Op::SetVariable { id, .. }
            | Op::SetBinding { id, .. }
            | Op::SetField { id, .. }
            | Op::SetHole { id, .. }
            | Op::SetSweep { id, .. }
            | Op::SetLoft { id, .. }
            | Op::SetSketchPlane { id, .. }
            | Op::RenameFeature { id, .. }
            | Op::SetSuppressed { id, .. }
            | Op::DeleteFeature { id }
            | Op::MoveFeature { id, .. } => match ps.features.iter().position(|f| f.id == *id) {
                Some(index) => Before::Feature {
                    index,
                    feature: ps.features[index].clone(),
                },
                None => Before::None,
            },
            Op::SetSettings { .. } => Before::Settings(ps.settings),
            Op::RenameStudio { .. } => Before::Name(ps.name.clone()),
            Op::ReplaceDocument { .. } => Before::Document(ps.to_json()),
            _ => Before::None,
        }
    }

    fn kind(&self) -> Option<&FeatureKind> {
        match self {
            Before::Feature { feature, .. } => Some(&feature.kind),
            _ => None,
        }
    }
}

/// The `Restore` that turns `new` back into `old`, or `None` if nothing changed.
fn sketch_patch(old: &SketchFeature, new: &SketchFeature) -> Option<SketchOp> {
    let (os, ns) = (&old.sketch, &new.sketch);
    // Created by the op: remove (removal cascades to dependents and their constraints).
    let remove: Vec<EntityId> = ns
        .entities()
        .filter(|(id, _)| os.entity(*id).is_none())
        .map(|(id, _)| id)
        .collect();
    let remove_constraints: Vec<ConstraintId> = ns
        .constraints()
        .filter(|(id, _)| os.constraint(*id).is_none())
        .map(|(id, _)| id)
        .collect();
    // Removed or changed by the op: put back.
    let entities: Vec<_> = os
        .entities()
        .filter(|(id, e)| ns.entity(*id) != Some(*e))
        .map(|(id, e)| (id, e.clone()))
        .collect();
    let constraints: Vec<_> = os
        .constraints()
        .filter(|(id, c)| ns.constraint(*id) != Some(*c))
        .map(|(id, c)| (id, c.clone()))
        .collect();
    // Flags: for every entity of the old sketch whose flag differs now, or
    // that is being put back (its flags went with it).
    let restored = |id: EntityId| entities.iter().any(|(e, _)| *e == id);
    let construction: Vec<(EntityId, bool)> = os
        .entities()
        .map(|(id, _)| id)
        .filter(|&id| restored(id) || os.is_construction(id) != ns.is_construction(id))
        .map(|id| (id, os.is_construction(id)))
        .collect();
    let projected: Vec<(EntityId, bool)> = os
        .entities()
        .map(|(id, _)| id)
        .filter(|&id| restored(id) || os.is_projected(id) != ns.is_projected(id))
        .map(|id| (id, os.is_projected(id)))
        .collect();
    let projections = (old.projections != new.projections).then(|| old.projections.clone());
    if remove.is_empty()
        && remove_constraints.is_empty()
        && entities.is_empty()
        && constraints.is_empty()
        && construction.is_empty()
        && projected.is_empty()
        && projections.is_none()
    {
        return None;
    }
    Some(SketchOp::Restore {
        remove,
        remove_constraints,
        entities,
        constraints,
        construction,
        projected,
        projections,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        BlendKind, BodyOp, EdgeRef, ExtrudeDirection, ExtrudeEnd, FaceRef, Op, PartStudio,
        PlaneRef, ProfileSelection, ProjectionSource, SketchOp, StandardPlane,
    };
    use ok_math::Vec2;
    use ok_sketch::{Constraint, ConstraintId, EntityId};

    /// Document JSON without id counters: ids are never reused, so undo
    /// leaves the counters where they are.
    fn normalised(json: &str) -> serde_json::Value {
        fn strip(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Object(m) => {
                    m.remove("next_id");
                    m.remove("next_entity");
                    m.remove("next_constraint");
                    m.values_mut().for_each(strip);
                }
                serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
                _ => {}
            }
        }
        let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
        strip(&mut v);
        v
    }

    /// Applies `op`, then its inverse, and checks the document is back to
    /// where it started. Returns the result of the forward op.
    fn round_trip(ps: &mut PartStudio, op: Op) -> crate::OpResult {
        let before = ps.to_json();
        let r = ps.apply_with_inverse(op.clone(), None).unwrap();
        assert!(!r.inverse.is_empty(), "no inverse for {op:?}");
        let mut undone = ps.clone();
        for inv in &r.inverse {
            undone.apply(inv.clone()).unwrap();
        }
        assert_eq!(
            normalised(&undone.to_json()),
            normalised(&before),
            "undo of {op:?} did not restore"
        );
        r
    }

    #[test]
    fn every_edit_round_trips_through_its_inverse() {
        let mut ps = PartStudio::demo();
        let sketch = ps.features[0].id;
        let extrude = ps.features[1].id;
        // Feature-level edits.
        round_trip(
            &mut ps,
            Op::RenameFeature {
                id: extrude,
                name: "Base".into(),
            },
        );
        round_trip(
            &mut ps,
            Op::SetSuppressed {
                id: extrude,
                suppressed: true,
            },
        );
        round_trip(
            &mut ps,
            Op::SetExtrude {
                id: extrude,
                depth: Some(20.0),
                direction: None,
                end: Some(ExtrudeEnd::ThroughAll),
                profiles: None,
                op: None,
            },
        );
        round_trip(
            &mut ps,
            Op::MoveFeature {
                id: extrude,
                index: 4,
            },
        );
        round_trip(&mut ps, Op::SetSettings { facet_angle: 10.0 });
        round_trip(
            &mut ps,
            Op::RenameStudio {
                name: "renamed".into(),
            },
        );
        round_trip(
            &mut ps,
            Op::SetSketchPlane {
                id: sketch,
                plane: PlaneRef::standard(StandardPlane::Front),
            },
        );
        round_trip(
            &mut ps,
            Op::SetBinding {
                id: extrude,
                field: "depth".into(),
                expression: Some("2 * 4".into()),
            },
        );
        round_trip(
            &mut ps,
            Op::SetField {
                id: extrude,
                field: "depth".into(),
                value: 9.5,
            },
        );
        // Binding, regenerating (which writes the value in), then undoing
        // the binding restores the old value too.
        ps.apply(Op::SetSuppressed {
            id: extrude,
            suppressed: false,
        })
        .unwrap();
        ps.apply(Op::SetBinding {
            id: extrude,
            field: "depth".into(),
            expression: None,
        })
        .unwrap();
        let depth_before = ps.feature(extrude).unwrap().kind.field("depth").unwrap();
        let r = ps
            .apply_with_inverse(
                Op::SetBinding {
                    id: extrude,
                    field: "depth".into(),
                    expression: Some("2 * 4".into()),
                },
                None,
            )
            .unwrap();
        ps.regenerate();
        assert_eq!(ps.feature(extrude).unwrap().kind.field("depth"), Some(8.0));
        for inv in r.inverse {
            ps.apply(inv).unwrap();
        }
        assert_eq!(
            ps.feature(extrude).unwrap().kind.field("depth"),
            Some(depth_before)
        );
        assert!(ps.feature(extrude).unwrap().bindings.is_empty());
        // Creations and deletion.
        let r = round_trip(
            &mut ps,
            Op::AddBlend {
                kind: BlendKind::Fillet,
                edges: vec![EdgeRef {
                    a: FaceRef {
                        feature: extrude,
                        local: 0,
                    },
                    b: FaceRef {
                        feature: extrude,
                        local: 2,
                    },
                }],
                size: 1.0,
                name: None,
            },
        );
        let blend = r.feature.unwrap();
        round_trip(
            &mut ps,
            Op::SetBlend {
                id: blend,
                edges: Some(vec![]),
                size: Some(2.0),
            },
        );
        round_trip(&mut ps, Op::DeleteFeature { id: extrude });
        round_trip(
            &mut ps,
            Op::AddVariable {
                name: "w".into(),
                expression: "3".into(),
            },
        );
        // Sketch edits.
        let r = round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::AddRectangle {
                    a: Vec2::new(70.0, 0.0),
                    b: Vec2::new(80.0, 5.0),
                },
            },
        );
        let line = r.entities[0];
        let (start, _) = match &ps.feature(sketch).unwrap().kind {
            crate::FeatureKind::Sketch(sf) => sf.sketch.line(line).unwrap(),
            _ => unreachable!(),
        };
        round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::MovePoint {
                    id: start,
                    pos: Vec2::new(71.0, 1.0),
                },
            },
        );
        round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::SetConstruction {
                    id: line,
                    construction: true,
                },
            },
        );
        let r = round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::AddConstraint {
                    constraint: Constraint::Fixed { point: start },
                },
            },
        );
        let c = r.constraint.unwrap();
        round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::RemoveConstraint { id: c },
            },
        );
        // Removing a point takes its line and constraints with it; the
        // inverse puts all of them back.
        round_trip(
            &mut ps,
            Op::Sketch {
                id: sketch,
                op: SketchOp::RemoveEntity { id: start },
            },
        );
        // Projections (with regeneration in between so entities exist).
        let top = FaceRef {
            feature: extrude,
            local: 1,
        };
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Face {
                    face: top,
                    offset: 0.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        round_trip(
            &mut ps,
            Op::Sketch {
                id: s2,
                op: SketchOp::Project {
                    source: ProjectionSource::Face { face: top },
                },
            },
        );
        ps.regenerate();
        round_trip(
            &mut ps,
            Op::Sketch {
                id: s2,
                op: SketchOp::RemoveProjection { index: 0 },
            },
        );
        // Whole-document replacement.
        let other = PartStudio::new("other").to_json();
        round_trip(&mut ps, Op::ReplaceDocument { json: other });
        let _ = (
            EntityId(0),
            ConstraintId(0),
            BodyOp::New,
            ExtrudeDirection::Normal,
            ProfileSelection::All,
        );
    }

    #[test]
    fn undo_keeps_concurrent_edits() {
        // A adds a variable, B adds another; A undoes: only A's goes.
        let mut ps = PartStudio::new("t");
        let r = ps
            .apply_with_inverse(
                Op::AddVariable {
                    name: "a".into(),
                    expression: "1".into(),
                },
                Some(1 << 20),
            )
            .unwrap();
        ps.apply_with_base(
            Op::AddVariable {
                name: "b".into(),
                expression: "2".into(),
            },
            Some(2 << 20),
        )
        .unwrap();
        for inv in r.inverse {
            ps.apply(inv).unwrap();
        }
        assert_eq!(ps.features.len(), 1);
        assert_eq!(ps.features[0].name, "#b");
    }
}
