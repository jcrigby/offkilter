use ok_math::{Plane, Vec3};
use ok_sketch::{EntityId, Sketch};
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
    /// A standard plane turned `angle` degrees about a world axis through
    /// the origin, then offset along its new normal: an angled datum.
    Rotated {
        base: StandardPlane,
        axis: Axis,
        angle: f64,
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
            PlaneRef::Standard { offset, .. }
            | PlaneRef::Face { offset, .. }
            | PlaneRef::Rotated { offset, .. } => *offset,
        }
    }

    pub fn with_offset(self, offset: f64) -> Self {
        match self {
            PlaneRef::Standard { base, .. } => PlaneRef::Standard { base, offset },
            PlaneRef::Face { face, .. } => PlaneRef::Face { face, offset },
            PlaneRef::Rotated {
                base, axis, angle, ..
            } => PlaneRef::Rotated {
                base,
                axis,
                angle,
                offset,
            },
        }
    }

    /// The angle of a rotated plane, if it is one.
    pub fn angle(&self) -> Option<f64> {
        match self {
            PlaneRef::Rotated { angle, .. } => Some(*angle),
            _ => None,
        }
    }

    /// A rotated plane with a new angle; other kinds are returned unchanged.
    pub fn with_angle(self, new_angle: f64) -> Self {
        match self {
            PlaneRef::Rotated {
                base, axis, offset, ..
            } => PlaneRef::Rotated {
                base,
                axis,
                angle: new_angle,
                offset,
            },
            other => other,
        }
    }
}

/// The plane of a rotated reference: the standard plane turned about the
/// world axis, then offset along its normal.
pub fn rotated_plane(base: StandardPlane, axis: Axis, angle: f64, offset: f64) -> Plane {
    let p = base.plane();
    let t = ok_brep::Transform::rotation(ok_math::Vec3::ZERO, axis.vector(), angle.to_radians());
    Plane {
        origin: t.apply_point(p.origin),
        x_axis: t.apply_vector(p.x_axis),
        y_axis: t.apply_vector(p.y_axis),
        normal: t.apply_vector(p.normal),
    }
    .offset(offset)
}

/// A canonical sketch frame on a face plane: origin at the projection of
/// the world origin onto the plane, axes chosen from the normal alone, so
/// the frame is stable across regenerations that keep the plane.
pub fn canonical_frame(plane: &Plane) -> Plane {
    let n = plane.normal;
    let origin = n * n.dot(plane.origin);
    Plane::from_origin_normal(origin, n).unwrap_or(*plane)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchFeature {
    pub plane: PlaneRef,
    pub sketch: Sketch,
    /// Body geometry projected into this sketch ("Use"); rebuilt on every
    /// regeneration from the bodies that exist before the sketch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<Projection>,
}

impl SketchFeature {
    pub fn new(plane: PlaneRef) -> SketchFeature {
        SketchFeature {
            plane,
            sketch: Sketch::new(),
            projections: Vec::new(),
        }
    }
}

/// Entity ids reserved for each projection; its geometry may not need more.
pub const PROJECTION_BLOCK: u32 = 192;

/// What a projection copies into the sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectionSource {
    /// One edge (all of its display segments), by the faces meeting there.
    Edge { edge: EdgeRef },
    /// The boundary of a face.
    Face { face: FaceRef },
}

/// Body geometry mirrored into a sketch. The sketch entities are fixed for
/// the solver and follow the model when it changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projection {
    pub source: ProjectionSource,
    /// First id of the reserved entity block; the entities are `block + k`.
    pub block: EntityId,
    /// The sketch entities currently built for this projection.
    #[serde(default)]
    pub entities: Vec<EntityId>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevolveFeature {
    pub sketch: FeatureId,
    pub profiles: ProfileSelection,
    pub axis: RevolveAxis,
    /// Angle in degrees; positive is a right-hand turn about the axis.
    pub angle: f64,
    pub op: BodyOp,
}

/// An edge of a body, named by the two faces that meet there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeRef {
    pub a: FaceRef,
    pub b: FaceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendKind {
    Fillet,
    Chamfer,
}

/// A fillet (rounded) or chamfer (flat) blend along edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendFeature {
    pub kind: BlendKind,
    pub edges: Vec<EdgeRef>,
    /// Fillet radius or chamfer distance.
    pub size: f64,
}

/// How copies made by a mirror or pattern combine with the originals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyOp {
    /// Union each copy into the body it was made from.
    #[default]
    Add,
    /// Keep copies as separate bodies.
    New,
}

/// Mirrors every body across a plane, or, when `features` is set, replays
/// those features' tool volumes mirrored (a feature mirror).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MirrorFeature {
    pub plane: PlaneRef,
    #[serde(default)]
    pub op: CopyOp,
    /// Solid features whose tools are copied instead of whole bodies;
    /// empty copies every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureId>,
    /// Bodies (by creating feature) to copy when `features` is empty;
    /// empty copies every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies: Vec<FeatureId>,
}

/// A world axis direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn vector(self) -> ok_math::Vec3 {
        match self {
            Axis::X => ok_math::Vec3::X,
            Axis::Y => ok_math::Vec3::Y,
            Axis::Z => ok_math::Vec3::Z,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatternKind {
    /// Copies spaced along a world axis.
    Linear { axis: Axis, spacing: f64 },
    /// Copies rotated about a world axis through the origin, spread over `angle` degrees.
    Circular { axis: Axis, angle: f64 },
}

/// Repeats every body `count` times (the original included), or, when
/// `features` is set, replays those features' tool volumes at each step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternFeature {
    pub kind: PatternKind,
    pub count: u32,
    #[serde(default)]
    pub op: CopyOp,
    /// Solid features whose tools are copied instead of whole bodies;
    /// empty copies every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureId>,
    /// Bodies (by creating feature) to copy when `features` is empty;
    /// empty copies every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies: Vec<FeatureId>,
}

/// A drilled hole at every standalone point of a sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleFeature {
    pub sketch: FeatureId,
    pub diameter: f64,
    /// Depth from the sketch plane; ignored when `through_all`.
    pub depth: f64,
    #[serde(default)]
    pub through_all: bool,
    /// Drilling direction relative to the sketch normal.
    #[serde(default = "reverse")]
    pub direction: ExtrudeDirection,
    /// Optional counterbore: a wider, shallower cylinder from the plane.
    #[serde(default)]
    pub counterbore: Option<Counterbore>,
}

fn reverse() -> ExtrudeDirection {
    ExtrudeDirection::Reverse
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Counterbore {
    pub diameter: f64,
    pub depth: f64,
}

/// Sweeps a profile region along the open chain of curves in another sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepFeature {
    pub sketch: FeatureId,
    pub profiles: ProfileSelection,
    /// Sketch whose (non-construction) lines and arcs form the path.
    pub path: FeatureId,
    pub op: BodyOp,
}

/// Lofts between the largest region of two sketches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoftFeature {
    pub sketch: FeatureId,
    pub sketch_b: FeatureId,
    pub op: BodyOp,
}

/// Hollows bodies to a uniform wall thickness. `faces` are removed so the
/// cavity is reachable (a face on a curved surface opens the whole
/// surface); with none, every body gets a closed void. When faces are
/// given only the bodies holding them are shelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellFeature {
    pub thickness: f64,
    #[serde(default)]
    pub faces: Vec<FaceRef>,
}

/// Splits bodies by a plane into two bodies each: the part against the
/// plane's normal keeps the body's name, the other becomes a new part.
/// With `bodies` empty every body the plane crosses is split.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitFeature {
    pub plane: PlaneRef,
    /// Bodies (by creating feature) to split; empty splits every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies: Vec<FeatureId>,
}

/// A body imported as a triangle mesh (an STL file, say). The triangles
/// must close a volume; coplanar neighbours are merged into one face
/// when the solid is built, so a boxy mesh becomes a boxy body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshFeature {
    pub vertices: Vec<Vec3>,
    /// Vertex indices, counter-clockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

/// Moves planar faces along their normals (a direct edit: push or pull).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveFaceFeature {
    pub faces: Vec<FaceRef>,
    /// Along the face normal; negative pushes into the body.
    pub distance: f64,
}

/// Tilts planar faces about the line where each meets a neutral plane, so
/// the body tapers towards the neutral plane's normal (the pull direction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftFeature {
    pub faces: Vec<FaceRef>,
    pub neutral: PlaneRef,
    /// Degrees; negative tapers the other way.
    pub angle: f64,
}

/// How a boolean feature combines its target bodies with its tool bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    /// One body from all targets and tools.
    Union,
    /// Each target minus every tool.
    Subtract,
    /// Each target intersected with every tool.
    Intersect,
}

/// Combines existing bodies. Bodies are named by the feature that created
/// them (`Body::source`), so the reference follows the body through later
/// edits of its defining features.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BooleanFeature {
    pub op: BooleanOp,
    pub targets: Vec<FeatureId>,
    pub tools: Vec<FeatureId>,
    /// Leave the tool bodies in place instead of consuming them.
    #[serde(default)]
    pub keep_tools: bool,
}

/// A named value later features can use in expressions as `#name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableFeature {
    pub name: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    Sketch(SketchFeature),
    Extrude(ExtrudeFeature),
    Revolve(RevolveFeature),
    Blend(BlendFeature),
    Mirror(MirrorFeature),
    Pattern(PatternFeature),
    Variable(VariableFeature),
    Hole(HoleFeature),
    Sweep(SweepFeature),
    Loft(LoftFeature),
    Boolean(BooleanFeature),
    Shell(ShellFeature),
    MoveFace(MoveFaceFeature),
    Draft(DraftFeature),
    Mesh(MeshFeature),
    Split(SplitFeature),
}

impl FeatureKind {
    pub fn display_kind(&self) -> &'static str {
        match self {
            FeatureKind::Sketch(_) => "Sketch",
            FeatureKind::Extrude(_) => "Extrude",
            FeatureKind::Revolve(_) => "Revolve",
            FeatureKind::Blend(b) => match b.kind {
                BlendKind::Fillet => "Fillet",
                BlendKind::Chamfer => "Chamfer",
            },
            FeatureKind::Mirror(_) => "Mirror",
            FeatureKind::Pattern(_) => "Pattern",
            FeatureKind::Variable(_) => "Variable",
            FeatureKind::Hole(_) => "Hole",
            FeatureKind::Sweep(_) => "Sweep",
            FeatureKind::Loft(_) => "Loft",
            FeatureKind::Boolean(b) => match b.op {
                BooleanOp::Union => "Union",
                BooleanOp::Subtract => "Subtract",
                BooleanOp::Intersect => "Intersect",
            },
            FeatureKind::Shell(_) => "Shell",
            FeatureKind::MoveFace(_) => "Move face",
            FeatureKind::Draft(_) => "Draft",
            FeatureKind::Mesh(_) => "Mesh",
            FeatureKind::Split(_) => "Split",
        }
    }

    /// Numeric fields that accept expression bindings, by name.
    pub fn bindable_fields(&self) -> Vec<String> {
        match self {
            FeatureKind::Sketch(s) => {
                let mut v = vec!["plane.offset".to_string()];
                if s.plane.angle().is_some() {
                    v.push("plane.angle".to_string());
                }
                v.extend(
                    s.sketch
                        .constraints()
                        .filter(|(_, c)| c.value().is_some())
                        .map(|(id, _)| format!("constraint.{}", id.0)),
                );
                v
            }
            FeatureKind::Extrude(_) => vec!["depth".into()],
            FeatureKind::Revolve(_) => vec!["angle".into()],
            FeatureKind::Blend(_) => vec!["size".into()],
            FeatureKind::Mirror(_) => vec!["plane.offset".into()],
            FeatureKind::Pattern(p) => match p.kind {
                PatternKind::Linear { .. } => vec!["spacing".into(), "count".into()],
                PatternKind::Circular { .. } => vec!["angle".into(), "count".into()],
            },
            FeatureKind::Variable(_) => vec![],
            FeatureKind::Sweep(_)
            | FeatureKind::Loft(_)
            | FeatureKind::Boolean(_)
            | FeatureKind::Mesh(_) => vec![],
            FeatureKind::Split(_) => vec!["plane.offset".into()],
            FeatureKind::Shell(_) => vec!["thickness".into()],
            FeatureKind::MoveFace(_) => vec!["distance".into()],
            FeatureKind::Draft(_) => vec!["angle".into(), "plane.offset".into()],
            FeatureKind::Hole(_) => vec![
                "diameter".into(),
                "depth".into(),
                "cbore_diameter".into(),
                "cbore_depth".into(),
            ],
        }
    }

    /// Writes an evaluated binding into the named numeric field.
    /// Current value of a bindable numeric field (see `set_field`).
    pub fn field(&self, field: &str) -> Option<f64> {
        match (self, field) {
            (FeatureKind::Sketch(s), "plane.offset") => Some(s.plane.offset()),
            (FeatureKind::Sketch(s), "plane.angle") => s.plane.angle(),
            (FeatureKind::Sketch(s), f) if f.starts_with("constraint.") => {
                let id: u32 = f["constraint.".len()..].parse().ok()?;
                s.sketch
                    .constraint(ok_sketch::ConstraintId(id))
                    .and_then(|c| c.value())
            }
            (FeatureKind::Extrude(e), "depth") => Some(e.depth),
            (FeatureKind::Revolve(r), "angle") => Some(r.angle),
            (FeatureKind::Blend(b), "size") => Some(b.size),
            (FeatureKind::Shell(sh), "thickness") => Some(sh.thickness),
            (FeatureKind::MoveFace(m), "distance") => Some(m.distance),
            (FeatureKind::Draft(d), "angle") => Some(d.angle),
            (FeatureKind::Draft(d), "plane.offset") => Some(d.neutral.offset()),
            (FeatureKind::Mirror(m), "plane.offset") => Some(m.plane.offset()),
            (FeatureKind::Split(sp), "plane.offset") => Some(sp.plane.offset()),
            (FeatureKind::Pattern(p), "count") => Some(p.count as f64),
            (FeatureKind::Pattern(p), "spacing") => match &p.kind {
                PatternKind::Linear { spacing, .. } => Some(*spacing),
                _ => None,
            },
            (FeatureKind::Pattern(p), "angle") => match &p.kind {
                PatternKind::Circular { angle, .. } => Some(*angle),
                _ => None,
            },
            (FeatureKind::Hole(h), "diameter") => Some(h.diameter),
            (FeatureKind::Hole(h), "depth") => Some(h.depth),
            (FeatureKind::Hole(h), "cbore_diameter") => h.counterbore.map(|c| c.diameter),
            (FeatureKind::Hole(h), "cbore_depth") => h.counterbore.map(|c| c.depth),
            _ => None,
        }
    }

    pub fn set_field(&mut self, field: &str, value: f64) -> Result<(), String> {
        match (self, field) {
            (FeatureKind::Sketch(s), "plane.offset") => {
                s.plane = s.plane.with_offset(value);
                Ok(())
            }
            (FeatureKind::Sketch(s), "plane.angle") if s.plane.angle().is_some() => {
                s.plane = s.plane.with_angle(value);
                Ok(())
            }
            (FeatureKind::Sketch(s), f) if f.starts_with("constraint.") => {
                let id: u32 = f["constraint.".len()..]
                    .parse()
                    .map_err(|_| format!("bad field {f}"))?;
                if s.sketch
                    .set_constraint_value(ok_sketch::ConstraintId(id), value)
                {
                    Ok(())
                } else {
                    Err(format!("constraint {id} has no value"))
                }
            }
            (FeatureKind::Extrude(e), "depth") => {
                e.depth = value;
                Ok(())
            }
            (FeatureKind::Revolve(r), "angle") => {
                r.angle = value;
                Ok(())
            }
            (FeatureKind::Blend(b), "size") => {
                b.size = value;
                Ok(())
            }
            (FeatureKind::Shell(sh), "thickness") => {
                sh.thickness = value;
                Ok(())
            }
            (FeatureKind::MoveFace(m), "distance") => {
                m.distance = value;
                Ok(())
            }
            (FeatureKind::Draft(d), "angle") => {
                d.angle = value;
                Ok(())
            }
            (FeatureKind::Draft(d), "plane.offset") => {
                d.neutral = d.neutral.with_offset(value);
                Ok(())
            }
            (FeatureKind::Mirror(m), "plane.offset") => {
                m.plane = m.plane.with_offset(value);
                Ok(())
            }
            (FeatureKind::Split(m), "plane.offset") => {
                m.plane = m.plane.with_offset(value);
                Ok(())
            }
            (FeatureKind::Pattern(p), "count") => {
                p.count = value.round().max(0.0) as u32;
                Ok(())
            }
            (FeatureKind::Pattern(p), "spacing") => match &mut p.kind {
                PatternKind::Linear { spacing, .. } => {
                    *spacing = value;
                    Ok(())
                }
                _ => Err("spacing applies to linear patterns".into()),
            },
            (FeatureKind::Pattern(p), "angle") => match &mut p.kind {
                PatternKind::Circular { angle, .. } => {
                    *angle = value;
                    Ok(())
                }
                _ => Err("angle applies to circular patterns".into()),
            },
            (FeatureKind::Hole(h), "diameter") => {
                h.diameter = value;
                Ok(())
            }
            (FeatureKind::Hole(h), "depth") => {
                h.depth = value;
                Ok(())
            }
            (FeatureKind::Hole(h), "cbore_diameter") => {
                let mut cb = h.counterbore.unwrap_or(Counterbore {
                    diameter: value,
                    depth: 1.0,
                });
                cb.diameter = value;
                h.counterbore = Some(cb);
                Ok(())
            }
            (FeatureKind::Hole(h), "cbore_depth") => {
                let mut cb = h.counterbore.unwrap_or(Counterbore {
                    diameter: h.diameter * 2.0,
                    depth: value,
                });
                cb.depth = value;
                h.counterbore = Some(cb);
                Ok(())
            }
            (_, f) => Err(format!("no bindable field '{f}'")),
        }
    }

    /// The sketch a solid feature is built from, if any.
    pub fn source_sketch(&self) -> Option<FeatureId> {
        match self {
            FeatureKind::Sketch(_) => None,
            FeatureKind::Extrude(e) => Some(e.sketch),
            FeatureKind::Revolve(r) => Some(r.sketch),
            FeatureKind::Hole(h) => Some(h.sketch),
            FeatureKind::Sweep(sw) => Some(sw.sketch),
            FeatureKind::Loft(l) => Some(l.sketch),
            FeatureKind::Blend(_)
            | FeatureKind::Mirror(_)
            | FeatureKind::Pattern(_)
            | FeatureKind::Variable(_)
            | FeatureKind::Boolean(_)
            | FeatureKind::Shell(_)
            | FeatureKind::MoveFace(_)
            | FeatureKind::Draft(_)
            | FeatureKind::Mesh(_)
            | FeatureKind::Split(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    pub kind: FeatureKind,
    /// Expressions bound to numeric fields (see `FeatureKind::bindable_fields`),
    /// evaluated at regeneration and written into the fields.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub bindings: std::collections::BTreeMap<String, String>,
}
