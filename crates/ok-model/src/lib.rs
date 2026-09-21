//! The parametric document model.
//!
//! A [`PartStudio`] is an ordered list of [`Feature`]s. Regeneration
//! evaluates the features in order, producing solved sketches and solid
//! bodies. Every edit goes through [`Op`], which keeps the door open for an
//! operation log (undo/redo, branching, real-time collaboration) later.

mod assembly;
mod describe;
mod document;
pub mod expr;
mod feature;
mod invert;
mod merge;
mod ops;
mod project;
mod regen;

pub use assembly::{
    connector_frame, describe_anchor, mate_transform, Anchor, Assembly, AssemblyResult, Connector,
    Instance, InstanceId, Interference, Mate, MateId, MateKind, Placement, TabId,
};
pub use describe::{
    assembly_op, sketch_op, studio_op, BodyReport, CylinderReport, FaceReport, FeatureReport,
    InstanceReport, MateReport, SketchReport, TabReport,
};
pub use document::{AssemblyOp, DocOp, DocOpResult, Document, DrawingDimension, Tab, TabKind};
pub use feature::{
    canonical_frame, near_of, no_near, origin_hash, rotated_plane, Axis, BlendFeature, BlendKind,
    BodyOp, BooleanFeature, BooleanOp, CopyOp, Counterbore, Countersink, DraftFeature, EdgeRef,
    ExtrudeDirection, ExtrudeEnd, ExtrudeFeature, FaceRef, Feature, FeatureId, FeatureKind, Grain,
    HoleFeature, LoftFeature, MeshFeature, MirrorFeature, MoveFaceFeature, PatternFeature,
    PatternKind, PlaneRef, ProfileSelection, Projection, ProjectionSource, PuzzleFeature,
    PuzzleTab, RevolveAxis, RevolveFeature, ShellFeature, SketchFeature, SplitFeature,
    StandardPlane, SweepFeature, VariableFeature, NEAR, PROJECTION_BLOCK,
};
pub use merge::Merge;
pub use ops::{Op, OpResult, SketchOp};
pub use regen::{
    faces_of_ref, piece_near, piece_neighbours, Body, FeatureStatus, RegenResult, SketchCurve,
    SketchResult,
};

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
    /// Regeneration settings such as facet resolution.
    #[serde(default)]
    pub settings: Settings,
    /// User names for parts, by the feature that created the body.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub part_names: std::collections::BTreeMap<FeatureId, String>,
    /// Materials assigned to parts, by the feature that created the body.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub part_materials: std::collections::BTreeMap<FeatureId, Material>,
    /// Per-feature regeneration cache; never persisted.
    #[serde(skip)]
    cache: regen::RegenCache,
}

/// A part's material: a name and a density in g/cm³, from which a body's
/// mass follows (volume is in mm³, so mass in grams is volume × density
/// / 1000).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub density: f64,
}

impl Material {
    /// Mass in grams of a body of `volume_mm3`.
    pub fn mass_g(&self, volume_mm3: f64) -> f64 {
        volume_mm3 * self.density / 1000.0
    }
}

/// Document-wide regeneration settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Maximum angle per facet when approximating arcs, circles, revolves
    /// and fillets, in degrees. Smaller is rounder and slower.
    pub facet_angle: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { facet_angle: 5.0 }
    }
}

impl Settings {
    pub fn profile_options(&self) -> ok_sketch::ProfileOptions {
        ok_sketch::ProfileOptions {
            arc_segment_angle: self.facet_angle.clamp(0.5, 30.0).to_radians(),
            ..Default::default()
        }
    }
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
            settings: Settings::default(),
            part_names: Default::default(),
            part_materials: Default::default(),
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

    /// A hash of the document's structure: feature order, ids and kinds,
    /// sketch entity and constraint ids and types, references between
    /// features. It deliberately excludes floating-point values, which can
    /// differ in the last bits between native and wasm builds, so replicas
    /// that applied the same ops agree on it.
    pub fn structural_hash(&self) -> u64 {
        // Only fixed-width writes: `Hash` impls for slices, `str` and enums
        // write `usize`/`isize` values, whose width differs between the
        // wasm client (32-bit) and the native server (64-bit).
        use std::hash::Hasher;
        let mut h = std::hash::DefaultHasher::new();
        fn str_(h: &mut std::hash::DefaultHasher, s: &str) {
            h.write_u32(s.len() as u32);
            h.write(s.as_bytes());
        }
        fn ids(h: &mut std::hash::DefaultHasher, ids: &[u32]) {
            h.write_u32(ids.len() as u32);
            for id in ids {
                h.write_u32(*id);
            }
        }
        fn face(h: &mut std::hash::DefaultHasher, f: &FaceRef) {
            h.write_u32(f.feature.0);
            h.write_u32(f.local);
        }
        h.write_u32(self.features.len() as u32);
        for f in &self.features {
            h.write_u32(f.id.0);
            h.write_u8(f.suppressed as u8);
            str_(&mut h, f.kind.display_kind());
            h.write_u32(f.kind.source_sketch().map_or(u32::MAX, |s| s.0));
            h.write_u32(f.bindings.len() as u32);
            for (k, v) in &f.bindings {
                str_(&mut h, k);
                str_(&mut h, v);
            }
            match &f.kind {
                FeatureKind::Sketch(sf) => {
                    for (id, e) in sf.sketch.entities() {
                        h.write_u32(id.0);
                        str_(&mut h, e.kind_name());
                        ids(
                            &mut h,
                            &e.references().iter().map(|r| r.0).collect::<Vec<_>>(),
                        );
                        h.write_u8(sf.sketch.is_construction(id) as u8);
                        h.write_u8(sf.sketch.is_projected(id) as u8);
                    }
                    for (id, c) in sf.sketch.constraints() {
                        h.write_u32(id.0);
                        str_(&mut h, c.kind_name());
                        ids(
                            &mut h,
                            &c.references().iter().map(|r| r.0).collect::<Vec<_>>(),
                        );
                    }
                    h.write_u32(sf.projections.len() as u32);
                    for p in &sf.projections {
                        h.write_u32(p.block.0);
                        match p.source {
                            ProjectionSource::Edge { edge } => {
                                h.write_u8(0);
                                face(&mut h, &edge.a);
                                face(&mut h, &edge.b);
                            }
                            ProjectionSource::Face { face: fr } => {
                                h.write_u8(1);
                                face(&mut h, &fr);
                            }
                        }
                    }
                }
                FeatureKind::Blend(b) => {
                    h.write_u32(b.edges.len() as u32);
                    for e in &b.edges {
                        face(&mut h, &e.a);
                        face(&mut h, &e.b);
                    }
                }
                FeatureKind::Variable(v) => {
                    str_(&mut h, &v.name);
                    str_(&mut h, &v.expression);
                }
                _ => {}
            }
        }
        h.finish()
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
            FeatureKind::Sketch(SketchFeature::new(PlaneRef::standard(StandardPlane::Top))),
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
            FeatureKind::Sketch(SketchFeature::new(PlaneRef::Face {
                face: FaceRef {
                    feature: e1,
                    local: 1,
                    part: None,
                    near: Default::default(),
                },
                offset: 0.0,
            })),
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
            FeatureKind::Sketch(SketchFeature::new(PlaneRef::standard(StandardPlane::Front))),
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
        Ok(&mut self.sketch_feature_mut(id)?.sketch)
    }

    fn sketch_feature_mut(&mut self, id: FeatureId) -> Result<&mut SketchFeature, ModelError> {
        match &mut self.feature_mut(id)?.kind {
            FeatureKind::Sketch(s) => Ok(s),
            _ => Err(ModelError::WrongFeatureKind(id, "sketch")),
        }
    }
}
