use ok_math::Plane;
use ok_sketch::Sketch;
use serde::{Deserialize, Serialize};

/// Stable identifier of a feature within a part studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FeatureId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardPlane {
    /// XY plane, normal +Z.
    Top,
    /// XZ plane, normal -Y.
    Front,
    /// YZ plane, normal +X.
    Right,
}

impl StandardPlane {
    pub fn plane(self) -> Plane {
        match self {
            StandardPlane::Top => Plane::XY,
            StandardPlane::Front => Plane::XZ,
            StandardPlane::Right => Plane::YZ,
        }
    }
}

/// A sketch plane: a standard plane offset along its normal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlaneSpec {
    pub base: StandardPlane,
    #[serde(default)]
    pub offset: f64,
}

impl PlaneSpec {
    pub fn standard(base: StandardPlane) -> Self {
        Self { base, offset: 0.0 }
    }
    pub fn plane(&self) -> Plane {
        self.base.plane().offset(self.offset)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchFeature {
    pub plane: PlaneSpec,
    pub sketch: Sketch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtrudeDirection {
    /// Along the sketch plane normal.
    #[default]
    Normal,
    /// Against the sketch plane normal.
    Reverse,
    /// Half the depth each way.
    Symmetric,
}

/// How a feature's result combines with existing bodies. `Add`, `Remove`
/// and `Intersect` act on every existing body the tool volume touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOp {
    /// Create a new body.
    #[default]
    New,
    /// Union with the bodies it touches (or a new body if none).
    Add,
    /// Subtract from the bodies it touches.
    Remove,
    /// Keep only the overlap with the bodies it touches.
    Intersect,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileSelection {
    /// Every closed region of the sketch.
    #[default]
    All,
    /// Only the region with the largest area (the typical "outer" region).
    Largest,
    /// Regions by index, in area-descending order.
    Indices { indices: Vec<usize> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    pub sketch: FeatureId,
    pub profiles: ProfileSelection,
    pub depth: f64,
    pub direction: ExtrudeDirection,
    pub op: BodyOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    Sketch(SketchFeature),
    Extrude(ExtrudeFeature),
}

impl FeatureKind {
    pub fn display_kind(&self) -> &'static str {
        match self {
            FeatureKind::Sketch(_) => "Sketch",
            FeatureKind::Extrude(_) => "Extrude",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    pub kind: FeatureKind,
}
