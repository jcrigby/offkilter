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

/// A reference to a face of a body, by the feature that created the face
/// and that feature's local face index. Survives regeneration as long as
/// the originating feature still produces the face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceRef {
    pub feature: FeatureId,
    pub local: u32,
}

impl FaceRef {
    pub fn matches(&self, origin: &ok_brep::FaceOrigin) -> bool {
        origin.feature == self.feature.0 && origin.local == self.local
    }
}

/// A sketch plane: a standard plane or a planar face of a body, offset
/// along its normal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaneRef {
    Standard {
        base: StandardPlane,
        #[serde(default)]
        offset: f64,
    },
    Face {
        face: FaceRef,
        #[serde(default)]
        offset: f64,
    },
}

impl PlaneRef {
    pub fn standard(base: StandardPlane) -> Self {
        PlaneRef::Standard { base, offset: 0.0 }
    }

    pub fn offset(&self) -> f64 {
        match self {
            PlaneRef::Standard { offset, .. } | PlaneRef::Face { offset, .. } => *offset,
        }
    }

    pub fn with_offset(self, offset: f64) -> Self {
        match self {
            PlaneRef::Standard { base, .. } => PlaneRef::Standard { base, offset },
            PlaneRef::Face { face, .. } => PlaneRef::Face { face, offset },
        }
    }
}

/// A canonical sketch frame on a face plane: origin at the projection of
/// the world origin onto the plane, axes chosen from the normal alone, so
/// the frame is stable across regenerations that keep the plane.
pub fn canonical_frame(plane: &Plane) -> Plane {
    let n = plane.normal;
    let origin = n * n.dot(plane.origin);
    Plane::from_origin_normal(origin, n).unwrap_or(*plane)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchFeature {
    pub plane: PlaneRef,
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

/// Where an extrusion stops.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtrudeEnd {
    /// A fixed depth.
    #[default]
    Blind,
    /// Past every existing body in the extrude direction.
    ThroughAll,
    /// To the plane of a planar face.
    UpToFace { face: FaceRef },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    pub sketch: FeatureId,
    pub profiles: ProfileSelection,
    /// Depth for a blind extrusion; ignored for other end conditions.
    pub depth: f64,
    pub direction: ExtrudeDirection,
    #[serde(default)]
    pub end: ExtrudeEnd,
    pub op: BodyOp,
}

/// The axis a revolve turns about, in the sketch's own coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolveAxis {
    /// The sketch's horizontal axis through its origin.
    XAxis,
    /// The sketch's vertical axis through its origin.
    YAxis,
    /// A line entity of the sketch.
    Line { line: ok_sketch::EntityId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolveFeature {
    pub sketch: FeatureId,
    pub profiles: ProfileSelection,
    pub axis: RevolveAxis,
    /// Angle in degrees; positive is a right-hand turn about the axis.
    pub angle: f64,
    pub op: BodyOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    Sketch(SketchFeature),
    Extrude(ExtrudeFeature),
    Revolve(RevolveFeature),
}

impl FeatureKind {
    pub fn display_kind(&self) -> &'static str {
        match self {
            FeatureKind::Sketch(_) => "Sketch",
            FeatureKind::Extrude(_) => "Extrude",
            FeatureKind::Revolve(_) => "Revolve",
        }
    }

    /// The sketch a solid feature is built from, if any.
    pub fn source_sketch(&self) -> Option<FeatureId> {
        match self {
            FeatureKind::Sketch(_) => None,
            FeatureKind::Extrude(e) => Some(e.sketch),
            FeatureKind::Revolve(r) => Some(r.sketch),
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
