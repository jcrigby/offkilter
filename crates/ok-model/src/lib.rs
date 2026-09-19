//! The parametric document model.
//!
//! A [`PartStudio`] is an ordered list of [`Feature`]s. Regeneration
//! evaluates the features in order, producing solved sketches and solid
//! bodies. Every edit goes through [`Op`], which keeps the door open for an
//! operation log (undo/redo, branching, real-time collaboration) later.

pub mod expr;
mod feature;
mod ops;
mod regen;

pub use feature::{
    canonical_frame, Axis, BlendFeature, BlendKind, BodyOp, CopyOp, EdgeRef, ExtrudeDirection,
    ExtrudeEnd, ExtrudeFeature, FaceRef, Feature, FeatureId, FeatureKind, MirrorFeature,
    PatternFeature, PatternKind, PlaneRef, ProfileSelection, RevolveAxis, RevolveFeature,
    SketchFeature, StandardPlane, VariableFeature,
};
pub use ops::{Op, OpResult, SketchOp};
pub use regen::{Body, FeatureStatus, RegenResult, SketchCurve, SketchResult};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("unknown feature {0:?}")]
    UnknownFeature(FeatureId),
    #[error("feature {0:?} is not a {1}")]
    WrongFeatureKind(FeatureId, &'static str),
    #[error("sketch error: {0}")]
    Sketch(#[from] ok_sketch::SketchError),
    #[error("invalid operation: {0}")]
    Invalid(String),
}

/// A part studio: an ordered feature list that regenerates into bodies.
///
/// ```
/// use ok_math::Vec2;
/// use ok_model::{Op, PartStudio, PlaneRef, SketchOp, StandardPlane};
///
/// let mut ps = PartStudio::new("bracket");
/// let s = ps.apply(Op::AddSketch { plane: PlaneRef::standard(StandardPlane::Top), name: None })?.feature.unwrap();
/// ps.apply(Op::Sketch { id: s, op: SketchOp::AddRectangle { a: Vec2::ZERO, b: Vec2::new(40.0, 20.0) } })?;
/// ps.apply(Op::AddExtrude { sketch: s, depth: 10.0, direction: Default::default(), end: Default::default(), profiles: Default::default(), op: Default::default(), name: None })?;
/// let result = ps.regenerate();
/// assert_eq!(result.bodies.len(), 1);
/// assert!((result.bodies[0].mesh.signed_volume() - 8000.0).abs() < 1e-3);
/// # Ok::<(), ok_model::ModelError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartStudio {
    pub name: String,
    features: Vec<Feature>,
    next_id: u32,
    /// Per-feature regeneration cache; never persisted.
    #[serde(skip)]
    cache: regen::RegenCache,
}

impl Default for PartStudio {
    fn default() -> Self {
        Self::new("Part Studio 1")
    }
}

impl PartStudio {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            features: Vec::new(),
            next_id: 1,
            cache: Default::default(),
        }
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn feature(&self, id: FeatureId) -> Result<&Feature, ModelError> {
        self.features
            .iter()
            .find(|f| f.id == id)
            .ok_or(ModelError::UnknownFeature(id))
    }

    pub fn feature_mut(&mut self, id: FeatureId) -> Result<&mut Feature, ModelError> {
        self.features
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or(ModelError::UnknownFeature(id))
    }

    fn position(&self, id: FeatureId) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }

    /// Appends a feature and returns its id. The name is auto-numbered when
    /// `name` is `None` ("Sketch 1", "Extrude 2", ...).
    pub fn push_feature(&mut self, kind: FeatureKind, name: Option<String>) -> FeatureId {
        let id = FeatureId(self.next_id);
        self.next_id += 1;
        let name = name.unwrap_or_else(|| {
            let base = kind.display_kind();
            let n = self
                .features
                .iter()
                .filter(|f| f.kind.display_kind() == base)
                .count()
                + 1;
            format!("{base} {n}")
        });
        self.features.push(Feature {
            id,
            name,
            suppressed: false,
            kind,
            bindings: Default::default(),
        });
        id
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("part studio serialises")
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// A small example part used by the web client on first launch: a
    /// dimensioned plate with a through hole, a boss on top, and a slot cut
    /// across the boss.
    pub fn demo() -> PartStudio {
        use ok_math::Vec2;
        use ok_sketch::Constraint;
        let mut ps = PartStudio::new("Demo plate");
        let s1 = ps.push_feature(
            FeatureKind::Sketch(SketchFeature {
                plane: PlaneRef::standard(StandardPlane::Top),
                sketch: ok_sketch::Sketch::new(),
            }),
            None,
        );
        {
            let sk = ps.sketch_mut(s1).unwrap();
            let [bottom, right, ..] = sk.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(60.0, 40.0));
            let (bl, _) = sk.line(bottom).unwrap();
            sk.add_constraint(Constraint::Fixed { point: bl });
            sk.add_constraint(Constraint::Length {
                line: bottom,
                value: 60.0,
            });
            sk.add_constraint(Constraint::Length {
                line: right,
                value: 40.0,
            });
            let (c, center) = sk.add_circle(Vec2::new(30.0, 20.0), 6.0);
            sk.add_constraint(Constraint::Diameter {
                entity: c,
                value: 12.0,
            });
            sk.add_constraint(Constraint::HorizontalDistance {
                a: bl,
                b: center,
                value: 30.0,
            });
            sk.add_constraint(Constraint::VerticalDistance {
                a: bl,
                b: center,
                value: 20.0,
            });
        }
        ps.push_feature(
            FeatureKind::Extrude(ExtrudeFeature {
                sketch: s1,
                profiles: ProfileSelection::Largest,
                depth: 8.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                op: BodyOp::New,
            }),
            None,
        );
        // The boss is sketched on the plate's top face (extrude local face 1).
        let e1 = ps.features.last().unwrap().id;
        let s2 = ps.push_feature(
            FeatureKind::Sketch(SketchFeature {
                plane: PlaneRef::Face {
                    face: FaceRef {
                        feature: e1,
                        local: 1,
                    },
                    offset: 0.0,
                },
                sketch: ok_sketch::Sketch::new(),
            }),
            None,
        );
        {
            let sk = ps.sketch_mut(s2).unwrap();
            let (c, center) = sk.add_circle(Vec2::new(30.0, 20.0), 10.0);
            sk.add_constraint(Constraint::Fixed { point: center });
            sk.add_constraint(Constraint::Radius {
                entity: c,
                value: 10.0,
            });
            // The boss keeps the through hole open.
            let (hole, hc) = sk.add_circle(Vec2::new(30.0, 20.0), 6.0);
            sk.add_constraint(Constraint::Coincident { a: hc, b: center });
            sk.add_constraint(Constraint::Diameter {
                entity: hole,
                value: 12.0,
            });
        }
        ps.push_feature(
            FeatureKind::Extrude(ExtrudeFeature {
                sketch: s2,
                profiles: ProfileSelection::Largest,
                depth: 6.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                op: BodyOp::Add,
            }),
            None,
        );
        let s3 = ps.push_feature(
            FeatureKind::Sketch(SketchFeature {
                plane: PlaneRef::standard(StandardPlane::Front),
                sketch: ok_sketch::Sketch::new(),
            }),
            None,
        );
        {
            let sk = ps.sketch_mut(s3).unwrap();
            let [bottom, right, ..] =
                sk.add_rectangle(Vec2::new(25.0, 10.0), Vec2::new(35.0, 20.0));
            let (bl, _) = sk.line(bottom).unwrap();
            sk.add_constraint(Constraint::Fixed { point: bl });
            sk.add_constraint(Constraint::Length {
                line: bottom,
                value: 10.0,
            });
            sk.add_constraint(Constraint::Length {
                line: right,
                value: 10.0,
            });
        }
        ps.push_feature(
            FeatureKind::Extrude(ExtrudeFeature {
                sketch: s3,
                profiles: ProfileSelection::All,
                depth: 100.0,
                direction: ExtrudeDirection::Symmetric,
                end: ExtrudeEnd::ThroughAll,
                op: BodyOp::Remove,
            }),
            None,
        );
        ps
    }

    fn sketch_mut(&mut self, id: FeatureId) -> Result<&mut ok_sketch::Sketch, ModelError> {
        match &mut self.feature_mut(id)?.kind {
            FeatureKind::Sketch(s) => Ok(&mut s.sketch),
            _ => Err(ModelError::WrongFeatureKind(id, "sketch")),
        }
    }
}
