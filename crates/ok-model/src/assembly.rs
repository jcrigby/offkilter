//! Assemblies: instances of part-studio bodies placed by mates.
//!
//! An instance refers to a body of a part studio tab in the same document.
//! A mate joins two instances through connectors, each a face of an
//! instance's body: the connector frame has its origin at the face
//! centroid and its z axis along the face normal (or the cylinder axis for
//! a cylindrical face). Mates are resolved as a chain: fixed instances and
//! instances without mates sit at their own placement; every other
//! instance is moved so that its connector frame meets the other side's,
//! z axes opposed (or aligned with `flip`), rotated by `angle` about z and
//! separated by `offset` along it. Revolute, slider and cylindrical mates
//! use the same placement; their kind says which of `angle` and `offset`
//! the user may vary. Closed loops and second mates on an already placed
//! instance are not solved; they are reported.

use crate::{FaceRef, FeatureId};
use ok_brep::{Solid, Surface, Transform};
use ok_math::{Plane, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TabId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MateId(pub u32);

/// A rigid placement: rotation about x, then y, then z (degrees), then a
/// translation. Easier to edit than a matrix.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Placement {
    #[serde(default)]
    pub position: Vec3,
    /// Euler angles in degrees, applied x then y then z.
    #[serde(default)]
    pub rotation: Vec3,
}

impl Placement {
    pub fn to_transform(self) -> Transform {
        let rx = Transform::rotation(Vec3::ZERO, Vec3::X, self.rotation.x.to_radians());
        let ry = Transform::rotation(Vec3::ZERO, Vec3::Y, self.rotation.y.to_radians());
        let rz = Transform::rotation(Vec3::ZERO, Vec3::Z, self.rotation.z.to_radians());
        let r = compose(&rz, &compose(&ry, &rx));
        Transform {
            m: r.m,
            t: self.position,
        }
    }
}

/// `a` after `b`: the transform applying `b` first, then `a`.
pub fn compose(a: &Transform, b: &Transform) -> Transform {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a.m[i][k] * b.m[k][j]).sum();
        }
    }
    Transform {
        m,
        t: a.apply_point(b.t),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: InstanceId,
    pub name: String,
    /// The part studio tab the body comes from.
    pub studio: TabId,
    /// Index of the body in that studio's regenerated bodies.
    pub body: usize,
    /// Fixed instances never move; they anchor mate chains.
    #[serde(default)]
    pub fixed: bool,
    /// Where the instance sits when no mate positions it.
    #[serde(default)]
    pub placement: Placement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MateKind {
    #[default]
    Fastened,
    /// Rotation about the connector z axis is free (`angle`).
    Revolute,
    /// Translation along the connector z axis is free (`offset`).
    Slider,
    /// Both rotation and translation along z are free.
    Cylindrical,
    /// Faces stay parallel at `offset`; sliding and spinning in the plane are free.
    Planar,
    /// Only the connector origins coincide (a ball joint).
    Ball,
}

/// Where on its face a connector sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Anchor {
    /// The face itself: origin at its centroid, z along its normal.
    #[default]
    Face,
    /// The edge the face shares with `other`: origin at the middle of the
    /// edge, z along it (or along the axis of a circular edge).
    Edge { other: FaceRef },
    /// The vertex where the face meets both `others`: origin there, z
    /// along the face normal.
    Vertex { others: [FaceRef; 2] },
}

/// A face, edge or vertex of an instance's body, used as a mate connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    pub instance: InstanceId,
    pub face: FaceRef,
    #[serde(default, skip_serializing_if = "is_face")]
    pub anchor: Anchor,
}

fn is_face(a: &Anchor) -> bool {
    *a == Anchor::Face
}

/// "face 2", "edge 2/3" or "vertex 2/3/4" for messages.
pub fn describe_anchor(c: &Connector) -> String {
    match c.anchor {
        Anchor::Face => format!("face {}", c.face.local),
        Anchor::Edge { other } => format!("edge {}/{}", c.face.local, other.local),
        Anchor::Vertex { others } => format!(
            "vertex {}/{}/{}",
            c.face.local, others[0].local, others[1].local
        ),
    }
}

impl Connector {
    /// A connector on a face.
    pub fn face(instance: InstanceId, face: FaceRef) -> Connector {
        Connector {
            instance,
            face,
            anchor: Anchor::Face,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mate {
    pub id: MateId,
    pub name: String,
    #[serde(default)]
    pub kind: MateKind,
    pub a: Connector,
    pub b: Connector,
    /// Separation along the connector z axis (mm).
    #[serde(default)]
    pub offset: f64,
    /// Rotation about the connector z axis (degrees).
    #[serde(default)]
    pub angle: f64,
    /// Align the z axes instead of opposing them.
    #[serde(default)]
    pub flip: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    pub name: String,
    #[serde(default)]
    pub instances: Vec<Instance>,
    #[serde(default)]
    pub mates: Vec<Mate>,
}

impl Assembly {
    pub fn new(name: impl Into<String>) -> Assembly {
        Assembly {
            name: name.into(),
            instances: Vec::new(),
            mates: Vec::new(),
        }
    }

    pub fn instance(&self, id: InstanceId) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn mate(&self, id: MateId) -> Option<&Mate> {
        self.mates.iter().find(|m| m.id == id)
    }
}

/// Where the assembly put everything.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssemblyResult {
    /// The placed bodies of every instance that resolved, in instance
    /// order (a sub-assembly instance contributes several).
    #[serde(skip)]
    pub bodies: Vec<crate::Body>,
    /// The instance each entry of `bodies` belongs to.
    pub placed: Vec<InstanceId>,
    pub transforms: BTreeMap<InstanceId, Transform>,
    pub instance_errors: BTreeMap<InstanceId, String>,
    pub mate_errors: BTreeMap<MateId, String>,
}

/// Connector frame on the first body of a group that has the face.
pub fn group_frame(group: &[Solid], face: &FaceRef, anchor: &Anchor) -> Option<Plane> {
    group.iter().find_map(|s| connector_frame(s, face, anchor))
}

/// Connector frame in body coordinates. On a face: origin at the face
/// centroid (on the axis for a cylindrical face), z along the normal or
/// axis, x and y canonical for that normal. On an edge: origin at the
/// middle of the edge, z along it, x along the face normal (a circular
/// edge takes the cylinder axis and its centre). On a vertex: origin at
/// the vertex, z along the face normal, x along the edge shared with the
/// first other face.
pub fn connector_frame(solid: &Solid, face: &FaceRef, anchor: &Anchor) -> Option<Plane> {
    match anchor {
        Anchor::Face => face_frame(solid, face),
        Anchor::Edge { other } => edge_frame(solid, face, other),
        Anchor::Vertex { others } => vertex_frame(solid, face, others),
    }
}

/// Every facet of the surfaces that the referenced face lies on.
fn surface_facets<'a>(solid: &'a Solid, face: &FaceRef) -> Vec<&'a ok_brep::Face> {
    let surfaces: Vec<usize> = crate::regen::faces_of_ref(solid, face)
        .into_iter()
        .map(|i| solid.faces[i].surface)
        .collect();
    solid
        .faces
        .iter()
        .filter(|f| surfaces.contains(&f.surface))
        .collect()
}

/// The axis of the surface under a face, if it is a surface of revolution.
fn surface_axis(solid: &Solid, f: &ok_brep::Face) -> Option<(Vec3, Vec3)> {
    match solid.surfaces.get(f.surface) {
        Some(Surface::Cylinder { origin, axis, .. }) | Some(Surface::Revolved { origin, axis }) => {
            Some((*origin, *axis))
        }
        _ => None,
    }
}

/// A frame with the given z and an x hint (projected perpendicular to z).
fn frame_with_x(origin: Vec3, z: Vec3, x_hint: Vec3) -> Option<Plane> {
    let z = z.normalized()?;
    let x = (x_hint - z * x_hint.dot(z)).normalized();
    match x {
        Some(x) => Some(Plane {
            origin,
            x_axis: x,
            y_axis: z.cross(x),
            normal: z,
        }),
        None => Plane::from_origin_normal(origin, z),
    }
}

fn edge_frame(solid: &Solid, face: &FaceRef, other: &FaceRef) -> Option<Plane> {
    let mine = surface_facets(solid, face);
    let theirs = surface_facets(solid, other);
    let mine_idx: Vec<usize> = mine.iter().map(|f| index_of(solid, f)).collect();
    let their_idx: Vec<usize> = theirs.iter().map(|f| index_of(solid, f)).collect();
    // Directed segments of my facets that a facet of the other face walks
    // the opposite way.
    let mut segments: Vec<(Vec3, Vec3)> = Vec::new();
    for (a, b, f) in solid.directed_edges() {
        if !mine_idx.contains(&f) {
            continue;
        }
        let shared = solid
            .directed_edges()
            .any(|(c, d, g)| c == b && d == a && their_idx.contains(&g));
        if shared {
            segments.push((solid.vertices[a as usize], solid.vertices[b as usize]));
        }
    }
    if segments.is_empty() {
        return None;
    }
    let first = *mine.first()?;
    let centroid = segments
        .iter()
        .fold(Vec3::ZERO, |s, (a, b)| s + (*a + *b) * 0.5)
        / segments.len() as f64;
    // A rim between a cylinder and something else is a circle: its frame
    // sits on the axis, z along it.
    let axis = surface_axis(solid, first).or_else(|| surface_axis(solid, theirs.first()?));
    if let Some((origin, axis)) = axis {
        if segments.len() > 1 {
            let on_axis = origin + axis * (centroid - origin).dot(axis);
            return Plane::from_origin_normal(on_axis, axis);
        }
    }
    let (a, b) = segments
        .iter()
        .max_by(|p, q| p.0.distance(p.1).total_cmp(&q.0.distance(q.1)))?;
    frame_with_x(centroid, *b - *a, first.plane.normal)
}

fn index_of(solid: &Solid, f: &ok_brep::Face) -> usize {
    solid
        .faces
        .iter()
        .position(|g| std::ptr::eq(g, f))
        .unwrap_or(usize::MAX)
}

fn vertex_frame(solid: &Solid, face: &FaceRef, others: &[FaceRef; 2]) -> Option<Plane> {
    let mine = surface_facets(solid, face);
    let vertex_set = |facets: &[&ok_brep::Face]| -> Vec<u32> {
        let mut vs: Vec<u32> = facets
            .iter()
            .flat_map(|f| f.loops.iter().flatten().copied())
            .collect();
        vs.sort_unstable();
        vs.dedup();
        vs
    };
    let mut common = vertex_set(&mine);
    for o in others {
        let vs = vertex_set(&surface_facets(solid, o));
        common.retain(|v| vs.contains(v));
    }
    let v = *common.first()?;
    let origin = solid.vertices[v as usize];
    let facet = mine
        .iter()
        .find(|f| f.loops.iter().flatten().any(|&i| i == v))?;
    // x runs along the edge shared with the first other face, if there is one.
    let x_hint = solid
        .directed_edges()
        .filter(|(a, b, f)| (*a == v || *b == v) && std::ptr::eq(&solid.faces[*f], *facet))
        .map(|(a, b, _)| {
            let far = if a == v { b } else { a };
            solid.vertices[far as usize] - origin
        })
        .find(|d| d.length() > 0.0)
        .unwrap_or(Vec3::X);
    frame_with_x(origin, facet.plane.normal, x_hint)
}

fn face_frame(solid: &Solid, face: &FaceRef) -> Option<Plane> {
    let faces: Vec<&ok_brep::Face> = crate::regen::faces_of_ref(solid, face)
        .into_iter()
        .map(|i| &solid.faces[i])
        .collect();
    let first = *faces.first()?;
    // Centroid over every facet sharing the surface, so a cylinder's
    // connector sits at the middle of the whole face.
    let same_surface: Vec<&ok_brep::Face> = solid
        .faces
        .iter()
        .filter(|f| f.surface == first.surface)
        .collect();
    let mut sum = Vec3::ZERO;
    let mut n = 0usize;
    for f in &same_surface {
        for &v in &f.loops[0] {
            sum += solid.vertices[v as usize];
            n += 1;
        }
    }
    let centroid = sum / n.max(1) as f64;
    match solid.surfaces.get(first.surface) {
        Some(Surface::Cylinder { origin, axis, .. }) => {
            let on_axis = *origin + *axis * (centroid - *origin).dot(*axis);
            Plane::from_origin_normal(on_axis, *axis)
        }
        Some(Surface::Revolved { origin, axis }) => {
            let on_axis = *origin + *axis * (centroid - *origin).dot(*axis);
            Plane::from_origin_normal(on_axis, *axis)
        }
        _ => Plane::from_origin_normal(centroid, first.plane.normal),
    }
}

fn columns(x: Vec3, y: Vec3, z: Vec3) -> [[f64; 3]; 3] {
    [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]]
}

fn transpose(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut t = [[0.0; 3]; 3];
    for (i, row) in m.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            t[j][i] = *v;
        }
    }
    t
}

/// The transform placing a body so that its local connector `local` lands
/// on `target` (a world frame): frames coincide with z opposed unless
/// `flip`, rotated by `angle` degrees about z and moved `offset` along it.
pub fn mate_transform(
    target: &Plane,
    local: &Plane,
    offset: f64,
    angle: f64,
    flip: bool,
) -> Transform {
    let z = if flip { target.normal } else { -target.normal };
    let origin = target.origin + target.normal * offset;
    // x of the moving frame: target x rotated by `angle` about z.
    let spin = Transform::rotation(Vec3::ZERO, z, angle.to_radians());
    let x = spin.apply_vector(target.x_axis);
    let y = z.cross(x);
    let world = columns(x, y, z);
    let body = transpose(columns(local.x_axis, local.y_axis, local.normal));
    let r = Transform {
        m: [[0.0; 3]; 3],
        t: Vec3::ZERO,
    };
    let mut m = r.m;
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| world[i][k] * body[k][j]).sum();
        }
    }
    let rot = Transform { m, t: Vec3::ZERO };
    Transform {
        m,
        t: origin - rot.apply_vector(local.origin),
    }
}

/// Applies a transform to a plane frame.
pub fn transform_plane(xf: &Transform, p: &Plane) -> Plane {
    Plane {
        origin: xf.apply_point(p.origin),
        x_axis: xf.apply_vector(p.x_axis),
        y_axis: xf.apply_vector(p.y_axis),
        normal: xf.apply_vector(p.normal),
    }
}

impl Assembly {
    /// Places every instance given each instance's source solids (a body,
    /// or every body of a sub-assembly, in source coordinates), resolving
    /// mates as chains from fixed instances and then numerically.
    pub fn resolve(&self, solids: &BTreeMap<InstanceId, Vec<Solid>>) -> AssemblyResult {
        let mut result = AssemblyResult::default();
        let mut placed: BTreeMap<InstanceId, Transform> = BTreeMap::new();
        for inst in &self.instances {
            if !solids.contains_key(&inst.id) {
                result.instance_errors.insert(
                    inst.id,
                    "the source tab no longer has this body, or would contain itself".into(),
                );
                continue;
            }
            let mated = self
                .mates
                .iter()
                .any(|m| m.a.instance == inst.id || m.b.instance == inst.id);
            if inst.fixed || !mated {
                placed.insert(inst.id, inst.placement.to_transform());
            }
        }
        let frame = |c: &Connector| -> Result<Plane, String> {
            let group = solids
                .get(&c.instance)
                .ok_or_else(|| "instance has no body".to_string())?;
            group_frame(group, &c.face, &c.anchor).ok_or_else(|| {
                format!(
                    "{} of feature {} not found on the instance",
                    describe_anchor(c),
                    c.face.feature.0
                )
            })
        };
        // Chain resolution: repeat until no mate can place anything new.
        let mut resolved: Vec<bool> = vec![false; self.mates.len()];
        loop {
            let mut progress = false;
            for (mi, mate) in self.mates.iter().enumerate() {
                if resolved[mi] {
                    continue;
                }
                let (pa, pb) = (
                    placed.contains_key(&mate.a.instance),
                    placed.contains_key(&mate.b.instance),
                );
                // Which side moves: b onto a by default, a onto b if only b is placed.
                let (anchor, moving, sign) = match (pa, pb) {
                    (true, false) => (&mate.a, &mate.b, 1.0),
                    (false, true) => (&mate.b, &mate.a, -1.0),
                    (true, true) => {
                        resolved[mi] = true;
                        // Both already placed by something else: check, don't move.
                        match (frame(&mate.a), frame(&mate.b)) {
                            (Ok(fa), Ok(fb)) => {
                                let wa = transform_plane(&placed[&mate.a.instance], &fa);
                                let wb = transform_plane(&placed[&mate.b.instance], &fb);
                                let want =
                                    mate_transform(&wa, &fb, mate.offset, mate.angle, mate.flip);
                                let got = placed[&mate.b.instance];
                                let drift = (0..3)
                                    .map(|i| {
                                        (0..3)
                                            .map(|j| (want.m[i][j] - got.m[i][j]).abs())
                                            .sum::<f64>()
                                    })
                                    .sum::<f64>()
                                    + want.t.distance(got.t);
                                let _ = wb;
                                if drift > 1e-6 {
                                    result.mate_errors.insert(
                                        mate.id,
                                        "both instances are already positioned by other mates; this mate is not satisfied".into(),
                                    );
                                }
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                result.mate_errors.insert(mate.id, e);
                            }
                        }
                        progress = true;
                        continue;
                    }
                    (false, false) => continue,
                };
                resolved[mi] = true;
                progress = true;
                let (fa, fb) = match (frame(anchor), frame(moving)) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(e), _) | (_, Err(e)) => {
                        result.mate_errors.insert(mate.id, e);
                        continue;
                    }
                };
                let world_anchor = transform_plane(&placed[&anchor.instance], &fa);
                let xf = mate_transform(
                    &world_anchor,
                    &fb,
                    mate.offset,
                    sign * mate.angle,
                    mate.flip,
                );
                placed.insert(moving.instance, xf);
            }
            if !progress {
                break;
            }
        }
        for inst in &self.instances {
            if !placed.contains_key(&inst.id) && solids.contains_key(&inst.id) {
                // Only mated to instances that are themselves unplaced: a
                // loop with nothing fixed. Use its own placement and say so.
                placed.insert(inst.id, inst.placement.to_transform());
                result.instance_errors.insert(
                    inst.id,
                    "no fixed instance anchors this mate chain; using its own placement".into(),
                );
            }
        }
        // Numeric refinement: closed loops and redundant mates are solved
        // over the free degrees of freedom, starting from the chain result.
        let unsatisfied = self.refine(solids, &mut placed);
        for (mate, residual) in unsatisfied {
            if residual > 1e-5 {
                result.mate_errors.insert(
                    mate,
                    format!("cannot be satisfied together with the other mates (residual {residual:.3})"),
                );
            } else {
                result.mate_errors.remove(&mate);
            }
        }
        for inst in &self.instances {
            let (Some(group), Some(xf)) = (solids.get(&inst.id), placed.get(&inst.id)) else {
                continue;
            };
            for (k, solid) in group.iter().enumerate() {
                let name = if group.len() == 1 {
                    inst.name.clone()
                } else {
                    format!("{} / {}", inst.name, k + 1)
                };
                result.bodies.push(crate::Body::new(
                    name,
                    FeatureId(inst.id.0),
                    solid.transformed(xf),
                ));
                result.placed.push(inst.id);
            }
            result.transforms.insert(inst.id, *xf);
        }
        result
    }
}

/// Rotation vector `r` (axis times angle, radians) as a transform about the origin.
fn rotation_vector(r: Vec3) -> Transform {
    let angle = r.length();
    if angle <= 1e-15 {
        return Transform::IDENTITY;
    }
    Transform::rotation(Vec3::ZERO, r / angle, angle)
}

impl Assembly {
    /// Residuals of one mate given both world connector frames: what a
    /// fastened mate pins fully, a revolute mate frees about z, a slider
    /// along z, a cylindrical mate both.
    fn mate_residuals(mate: &Mate, fa: &Plane, fb: &Plane, out: &mut Vec<f64>) {
        let z = if mate.flip { fa.normal } else { -fa.normal };
        let spin = Transform::rotation(Vec3::ZERO, z, mate.angle.to_radians());
        let x = spin.apply_vector(fa.x_axis);
        let origin = fa.origin + fa.normal * mate.offset;
        let d = fb.origin - origin;
        let push = |out: &mut Vec<f64>, v: Vec3| out.extend([v.x, v.y, v.z]);
        if mate.kind != MateKind::Ball {
            push(out, fb.normal - z);
        }
        match mate.kind {
            MateKind::Fastened => {
                push(out, d);
                push(out, fb.x_axis - x);
            }
            MateKind::Revolute | MateKind::Ball => push(out, d),
            MateKind::Slider => {
                push(out, d - z * d.dot(z));
                push(out, fb.x_axis - x);
            }
            MateKind::Cylindrical => push(out, d - z * d.dot(z)),
            MateKind::Planar => out.push(d.dot(z)),
        }
    }

    /// Levenberg–Marquardt over the pose (rotation vector + translation)
    /// of every movable mated instance, applied on top of `placed`.
    /// Returns each mate's final residual norm.
    fn refine(
        &self,
        solids: &BTreeMap<InstanceId, Vec<Solid>>,
        placed: &mut BTreeMap<InstanceId, Transform>,
    ) -> Vec<(MateId, f64)> {
        let mates: Vec<&Mate> = self
            .mates
            .iter()
            .filter(|m| placed.contains_key(&m.a.instance) && placed.contains_key(&m.b.instance))
            .collect();
        if mates.is_empty() {
            return Vec::new();
        }
        let frames: Vec<(Connector, Plane)> = mates
            .iter()
            .flat_map(|m| [m.a, m.b])
            .filter_map(|c| {
                let f = group_frame(solids.get(&c.instance)?, &c.face, &c.anchor)?;
                Some((c, f))
            })
            .collect();
        let frame_of = |c: &Connector| frames.iter().find(|(k, _)| k == c).map(|(_, f)| f);
        let movable: Vec<InstanceId> = self
            .instances
            .iter()
            .filter(|i| !i.fixed && placed.contains_key(&i.id))
            .filter(|i| {
                mates
                    .iter()
                    .any(|m| m.a.instance == i.id || m.b.instance == i.id)
            })
            .map(|i| i.id)
            .collect();
        let base: Vec<Transform> = movable.iter().map(|id| placed[id]).collect();
        let fixed_poses: BTreeMap<InstanceId, Transform> = placed.clone();
        let pose = |x: &[f64], k: usize| -> Transform {
            let r = Vec3::new(x[6 * k], x[6 * k + 1], x[6 * k + 2]);
            let t = Vec3::new(x[6 * k + 3], x[6 * k + 4], x[6 * k + 5]);
            let delta = Transform {
                t,
                ..rotation_vector(r)
            };
            compose(&delta, &base[k])
        };
        let world = |x: &[f64], id: InstanceId| -> Transform {
            match movable.iter().position(|m| *m == id) {
                Some(k) => pose(x, k),
                None => fixed_poses[&id],
            }
        };
        let residuals = |x: &[f64]| -> Vec<f64> {
            let mut out = Vec::new();
            for m in &mates {
                let (Some(fa), Some(fb)) = (frame_of(&m.a), frame_of(&m.b)) else {
                    continue;
                };
                let wa = transform_plane(&world(x, m.a.instance), fa);
                let wb = transform_plane(&world(x, m.b.instance), fb);
                Self::mate_residuals(m, &wa, &wb, &mut out);
            }
            out
        };
        let n = 6 * movable.len();
        let mut x = vec![0.0; n];
        let norm = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut r = residuals(&x);
        let mut lambda = 1e-3;
        if n > 0 {
            for _ in 0..60 {
                let cost = norm(&r);
                if cost < 1e-10 {
                    break;
                }
                // Numeric Jacobian (central differences).
                let m = r.len();
                let mut jac = vec![vec![0.0; n]; m];
                for j in 0..n {
                    let h = 1e-6;
                    let mut xp = x.clone();
                    xp[j] += h;
                    let mut xm = x.clone();
                    xm[j] -= h;
                    let (rp, rm) = (residuals(&xp), residuals(&xm));
                    for i in 0..m {
                        jac[i][j] = (rp[i] - rm[i]) / (2.0 * h);
                    }
                }
                // Normal equations (JᵀJ + λ diag) δ = -Jᵀr, solved by Gaussian elimination.
                let mut a = vec![vec![0.0; n + 1]; n];
                for i in 0..n {
                    for j in 0..n {
                        a[i][j] = (0..m).map(|k| jac[k][i] * jac[k][j]).sum();
                    }
                    a[i][n] = -(0..m).map(|k| jac[k][i] * r[k]).sum::<f64>();
                    a[i][i] += lambda * (a[i][i] + 1e-9);
                }
                let mut delta = vec![0.0; n];
                let mut singular = false;
                for col in 0..n {
                    let pivot = (col..n)
                        .max_by(|p, q| a[*p][col].abs().total_cmp(&a[*q][col].abs()))
                        .unwrap();
                    a.swap(col, pivot);
                    if a[col][col].abs() < 1e-14 {
                        singular = true;
                        break;
                    }
                    let pivot_row = a[col].clone();
                    for (row, entries) in a.iter_mut().enumerate() {
                        if row != col {
                            let f = entries[col] / pivot_row[col];
                            for (e, p) in entries.iter_mut().zip(&pivot_row).skip(col) {
                                *e -= f * p;
                            }
                        }
                    }
                }
                if singular {
                    lambda *= 10.0;
                    continue;
                }
                for i in 0..n {
                    delta[i] = a[i][n] / a[i][i];
                }
                let xn: Vec<f64> = x.iter().zip(&delta).map(|(a, d)| a + d).collect();
                let rn = residuals(&xn);
                if norm(&rn) < cost {
                    x = xn;
                    r = rn;
                    lambda = (lambda * 0.3).max(1e-12);
                } else {
                    lambda *= 10.0;
                    if lambda > 1e8 {
                        break;
                    }
                }
            }
            for (k, id) in movable.iter().enumerate() {
                placed.insert(*id, pose(&x, k));
            }
        }
        // Per-mate residual norms at the solution.
        let mut per_mate = Vec::new();
        for m in &mates {
            let (Some(fa), Some(fb)) = (frame_of(&m.a), frame_of(&m.b)) else {
                continue;
            };
            let wa = transform_plane(&world(&x, m.a.instance), fa);
            let wb = transform_plane(&world(&x, m.b.instance), fb);
            let mut out = Vec::new();
            Self::mate_residuals(m, &wa, &wb, &mut out);
            per_mate.push((m.id, norm(&out)));
        }
        per_mate
    }
}

/// Overlap between two placed instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interference {
    pub a: InstanceId,
    pub b: InstanceId,
    /// Overlapping volume (mm³).
    pub volume: f64,
}

impl AssemblyResult {
    /// Pairs of placed instances whose bodies overlap, by boolean
    /// intersection of their solids. Bounding boxes prune the pairs first.
    /// Pairs whose boolean fails are skipped (reported as `None`).
    pub fn interferences(&self) -> (Vec<Interference>, Vec<(InstanceId, InstanceId)>) {
        let mut out = Vec::new();
        let mut failed = Vec::new();
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                if self.placed[i] == self.placed[j] {
                    continue; // bodies of one sub-assembly instance
                }
                let (a, b) = (&self.bodies[i].solid, &self.bodies[j].solid);
                let (Some((alo, ahi)), Some((blo, bhi))) = (a.bounds(), b.bounds()) else {
                    continue;
                };
                let tol = ok_math::tol::LINEAR;
                if alo.x > bhi.x + tol
                    || blo.x > ahi.x + tol
                    || alo.y > bhi.y + tol
                    || blo.y > ahi.y + tol
                    || alo.z > bhi.z + tol
                    || blo.z > ahi.z + tol
                {
                    continue;
                }
                match ok_brep::boolean(a, b, ok_brep::BoolOp::Intersection) {
                    Ok(x) => {
                        let volume = x.volume();
                        if volume > 1e-6 {
                            out.push(Interference {
                                a: self.placed[i],
                                b: self.placed[j],
                                volume,
                            });
                        }
                    }
                    Err(_) => failed.push((self.placed[i], self.placed[j])),
                }
            }
        }
        (out, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn block(w: f64, d: f64, h: f64, feature: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        ok_brep::extrude(&p, &Plane::XY, 0.0, h, feature).unwrap()
    }

    #[test]
    fn placement_composes_rotation_then_translation() {
        let p = Placement {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::new(0.0, 0.0, 90.0),
        };
        let q = p.to_transform().apply_point(Vec3::new(1.0, 0.0, 0.0));
        assert!(q.distance(Vec3::new(1.0, 3.0, 3.0)) < 1e-9, "{q:?}");
    }

    #[test]
    fn fastened_mate_stacks_a_block_on_another() {
        // Two 10x10x5 blocks; mate the top of A (local 1) to the bottom of
        // B (local 0): B sits on A, faces touching, normals opposed.
        let solid = block(10.0, 10.0, 5.0, 1);
        let mut asm = Assembly::new("asm");
        asm.instances.push(Instance {
            id: InstanceId(1),
            name: "A".into(),
            studio: TabId(1),
            body: 0,
            fixed: true,
            placement: Placement::default(),
        });
        asm.instances.push(Instance {
            id: InstanceId(2),
            name: "B".into(),
            studio: TabId(1),
            body: 0,
            fixed: false,
            placement: Placement::default(),
        });
        let top = FaceRef {
            feature: FeatureId(1),
            local: 1,
            part: None,
            near: Default::default(),
        };
        let bottom = FaceRef {
            feature: FeatureId(1),
            local: 0,
            part: None,
            near: Default::default(),
        };
        asm.mates.push(Mate {
            id: MateId(3),
            name: "m".into(),
            kind: MateKind::Fastened,
            a: Connector {
                instance: InstanceId(1),
                face: top,
                anchor: Anchor::Face,
            },
            b: Connector {
                instance: InstanceId(2),
                face: bottom,
                anchor: Anchor::Face,
            },
            offset: 0.0,
            angle: 0.0,
            flip: false,
        });
        let solids: BTreeMap<InstanceId, Vec<Solid>> = [
            (InstanceId(1), vec![solid.clone()]),
            (InstanceId(2), vec![solid.clone()]),
        ]
        .into_iter()
        .collect();
        let r = asm.resolve(&solids);
        assert!(r.mate_errors.is_empty(), "{:?}", r.mate_errors);
        assert_eq!(r.bodies.len(), 2);
        let (lo, hi) = r.bodies[1].solid.bounds().unwrap();
        assert!(
            (lo.z - 5.0).abs() < 1e-9 && (hi.z - 10.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        assert!(
            (lo.x - 0.0).abs() < 1e-9 && (hi.x - 10.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        // The union of both would be 10x10x10: volumes add, no overlap.
        let total: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        assert!((total - 1000.0).abs() < 1e-9);
        // Touching faces are not an interference; sinking B by 1 mm is.
        let (overlaps, failed) = r.interferences();
        assert!(
            overlaps.is_empty() && failed.is_empty(),
            "{overlaps:?} {failed:?}"
        );
        asm.mates[0].offset = -1.0;
        let (overlaps, _) = asm.resolve(&solids).interferences();
        assert_eq!(overlaps.len(), 1);
        assert!((overlaps[0].volume - 100.0).abs() < 1e-6, "{overlaps:?}");
        // An offset lifts B; flip turns it upside down onto the same face.
        asm.mates[0].offset = 2.0;
        let r = asm.resolve(&solids);
        let (lo, _) = r.bodies[1].solid.bounds().unwrap();
        assert!((lo.z - 7.0).abs() < 1e-9, "{lo:?}");
        asm.mates[0].offset = 0.0;
        asm.mates[0].flip = true;
        let r = asm.resolve(&solids);
        let (lo, hi) = r.bodies[1].solid.bounds().unwrap();
        assert!(
            (lo.z - 0.0).abs() < 1e-9 && (hi.z - 5.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        // Rotating by 90° about z keeps the block over the same footprint.
        asm.mates[0].flip = false;
        asm.mates[0].angle = 90.0;
        let r = asm.resolve(&solids);
        let (lo, hi) = r.bodies[1].solid.bounds().unwrap();
        assert!(
            (lo.z - 5.0).abs() < 1e-9 && hi.x - lo.x > 9.999,
            "{lo:?} {hi:?}"
        );
    }

    #[test]
    fn chains_resolve_through_moved_instances_and_loops_are_reported() {
        let solid = block(10.0, 10.0, 5.0, 1);
        let mk = |id: u32, fixed: bool| Instance {
            id: InstanceId(id),
            name: format!("i{id}"),
            studio: TabId(1),
            body: 0,
            fixed,
            placement: Placement::default(),
        };
        let top = FaceRef {
            feature: FeatureId(1),
            local: 1,
            part: None,
            near: Default::default(),
        };
        let bottom = FaceRef {
            feature: FeatureId(1),
            local: 0,
            part: None,
            near: Default::default(),
        };
        let mate = |id: u32, a: u32, b: u32| Mate {
            id: MateId(id),
            name: format!("m{id}"),
            kind: MateKind::Fastened,
            a: Connector {
                instance: InstanceId(a),
                face: top,
                anchor: Anchor::Face,
            },
            b: Connector {
                instance: InstanceId(b),
                face: bottom,
                anchor: Anchor::Face,
            },
            offset: 0.0,
            angle: 0.0,
            flip: false,
        };
        let mut asm = Assembly::new("asm");
        asm.instances
            .extend([mk(1, true), mk(2, false), mk(3, false)]);
        // 3 sits on 2, 2 sits on 1: listed in the "wrong" order on purpose.
        asm.mates.push(mate(10, 2, 3));
        asm.mates.push(mate(11, 1, 2));
        let solids: BTreeMap<InstanceId, Vec<Solid>> = (1..=3)
            .map(|i| (InstanceId(i), vec![solid.clone()]))
            .collect();
        let r = asm.resolve(&solids);
        assert!(r.mate_errors.is_empty() && r.instance_errors.is_empty());
        let (lo, _) = r.bodies[2].solid.bounds().unwrap();
        assert!((lo.z - 10.0).abs() < 1e-9, "{lo:?}");
        // Nothing fixed: a loop that cannot be anchored is reported, not silently placed.
        asm.instances[0].fixed = false;
        let r = asm.resolve(&solids);
        assert_eq!(r.instance_errors.len(), 3);
        assert_eq!(r.bodies.len(), 3);
        // A missing body is reported per instance.
        let mut fewer = solids.clone();
        fewer.remove(&InstanceId(3));
        let r = asm.resolve(&fewer);
        assert!(r.instance_errors.contains_key(&InstanceId(3)));
    }

    #[test]
    fn closed_loop_is_solved_through_free_degrees_of_freedom() {
        // A fixed. B sits on A's top through a cylindrical mate (free spin
        // and height), so the chain places it at 45° on top. A second,
        // slider mate between the +x faces demands parallel faces and no
        // sideways offset: the solver spins B back to 0° and drops it so it
        // coincides with A.
        let solid = block(10.0, 10.0, 5.0, 1);
        let mk = |id: u32, fixed: bool| Instance {
            id: InstanceId(id),
            name: format!("i{id}"),
            studio: TabId(1),
            body: 0,
            fixed,
            placement: Placement::default(),
        };
        let face = |local: u32| FaceRef {
            feature: FeatureId(1),
            local,
            part: None,
            near: Default::default(),
        };
        let mut asm = Assembly::new("loop");
        asm.instances.extend([mk(1, true), mk(2, false)]);
        asm.mates.push(Mate {
            id: MateId(10),
            name: "spin".into(),
            kind: MateKind::Cylindrical,
            a: Connector {
                instance: InstanceId(1),
                face: face(1),
                anchor: Anchor::Face,
            },
            b: Connector {
                instance: InstanceId(2),
                face: face(0),
                anchor: Anchor::Face,
            },
            offset: 0.0,
            angle: 45.0,
            flip: false,
        });
        asm.mates.push(Mate {
            id: MateId(11),
            name: "side".into(),
            kind: MateKind::Slider,
            a: Connector {
                instance: InstanceId(1),
                face: face(3),
                anchor: Anchor::Face,
            },
            b: Connector {
                instance: InstanceId(2),
                face: face(3),
                anchor: Anchor::Face,
            },
            offset: 0.0,
            angle: 0.0,
            flip: true,
        });
        let solids: BTreeMap<InstanceId, Vec<Solid>> = [
            (InstanceId(1), vec![solid.clone()]),
            (InstanceId(2), vec![solid.clone()]),
        ]
        .into_iter()
        .collect();
        let r = asm.resolve(&solids);
        assert!(r.mate_errors.is_empty(), "{:?}", r.mate_errors);
        let (lo, hi) = r.bodies[1].solid.bounds().unwrap();
        assert!(
            lo.distance(Vec3::ZERO) < 1e-6 && hi.distance(Vec3::new(10.0, 10.0, 5.0)) < 1e-6,
            "{lo:?} {hi:?}"
        );
        // An inconsistent extra mate stays reported, without breaking the rest.
        asm.mates.push(Mate {
            id: MateId(12),
            name: "bad".into(),
            kind: MateKind::Fastened,
            a: Connector {
                instance: InstanceId(1),
                face: face(1),
                anchor: Anchor::Face,
            },
            b: Connector {
                instance: InstanceId(2),
                face: face(1),
                anchor: Anchor::Face,
            },
            offset: 7.0,
            angle: 0.0,
            flip: false,
        });
        let r = asm.resolve(&solids);
        assert!(!r.mate_errors.is_empty());
        assert_eq!(r.bodies.len(), 2);
    }

    fn fref(local: u32) -> FaceRef {
        FaceRef {
            feature: FeatureId(1),
            local,
            part: None,
            near: Default::default(),
        }
    }

    #[test]
    fn edge_connector_sits_mid_edge_with_z_along_it() {
        // Block 10x10x5: top is local 1; walls 2..6 follow the rectangle
        // (0,0)->(10,0)->(10,10)->(0,10). The edge between the top and the
        // first wall (y = 0) runs along x at y = 0, z = 5.
        let b = block(10.0, 10.0, 5.0, 1);
        let f = connector_frame(&b, &fref(1), &Anchor::Edge { other: fref(2) }).unwrap();
        assert!(
            f.origin.distance(Vec3::new(5.0, 0.0, 5.0)) < 1e-9,
            "{:?}",
            f.origin
        );
        assert!((f.normal.x.abs() - 1.0).abs() < 1e-9, "{:?}", f.normal);
        // x follows the top face normal.
        assert!((f.x_axis.z - 1.0).abs() < 1e-9, "{:?}", f.x_axis);
        // The same edge named from the wall side has x along the wall normal.
        let g = connector_frame(&b, &fref(2), &Anchor::Edge { other: fref(1) }).unwrap();
        assert!(g.origin.distance(f.origin) < 1e-9);
        assert!((g.x_axis.y + 1.0).abs() < 1e-9, "{:?}", g.x_axis);
        // Faces that share no edge give no frame.
        assert!(connector_frame(&b, &fref(1), &Anchor::Edge { other: fref(0) }).is_none());
    }

    #[test]
    fn circular_edge_connector_sits_at_the_circle_centre() {
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(4.0, 3.0), 2.0);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        let cyl = ok_brep::extrude(&p, &Plane::XY, 0.0, 6.0, 1).unwrap();
        // Top rim: the top face (1) against the cylinder wall (2).
        let f = connector_frame(&cyl, &fref(1), &Anchor::Edge { other: fref(2) }).unwrap();
        assert!(
            f.origin.distance(Vec3::new(4.0, 3.0, 6.0)) < 1e-9,
            "{:?}",
            f.origin
        );
        assert!((f.normal.z.abs() - 1.0).abs() < 1e-9, "{:?}", f.normal);
    }

    #[test]
    fn vertex_connector_sits_on_the_corner() {
        let b = block(10.0, 10.0, 5.0, 1);
        // Top (1), wall y=0 (2) and wall x=10 (3) meet at (10,0,5).
        let f = connector_frame(
            &b,
            &fref(1),
            &Anchor::Vertex {
                others: [fref(2), fref(3)],
            },
        )
        .unwrap();
        assert!(
            f.origin.distance(Vec3::new(10.0, 0.0, 5.0)) < 1e-9,
            "{:?}",
            f.origin
        );
        assert!((f.normal.z - 1.0).abs() < 1e-9, "{:?}", f.normal);
        assert!(f.x_axis.z.abs() < 1e-9 && f.x_axis.length() > 0.999);
        // Opposite walls never meet the top at one vertex.
        assert!(connector_frame(
            &b,
            &fref(1),
            &Anchor::Vertex {
                others: [fref(2), fref(4)],
            },
        )
        .is_none());
    }

    #[test]
    fn edge_mate_hinges_two_blocks_along_a_shared_edge() {
        // B's bottom-front edge meets A's top-front edge (z opposed, so the
        // edges run opposite ways and the blocks stack corner to corner).
        let solid = block(10.0, 10.0, 5.0, 1);
        let mut asm = Assembly::new("asm");
        for (id, name, fixed) in [(1, "A", true), (2, "B", false)] {
            asm.instances.push(Instance {
                id: InstanceId(id),
                name: name.into(),
                studio: TabId(1),
                body: 0,
                fixed,
                placement: Placement::default(),
            });
        }
        asm.mates.push(Mate {
            id: MateId(10),
            name: "hinge".into(),
            kind: MateKind::Revolute,
            a: Connector {
                instance: InstanceId(1),
                face: fref(1),
                anchor: Anchor::Edge { other: fref(2) },
            },
            b: Connector {
                instance: InstanceId(2),
                face: fref(0),
                anchor: Anchor::Edge { other: fref(2) },
            },
            offset: 0.0,
            angle: 0.0,
            flip: false,
        });
        let mut solids = BTreeMap::new();
        solids.insert(InstanceId(1), vec![solid.clone()]);
        solids.insert(InstanceId(2), vec![solid]);
        let r = asm.resolve(&solids);
        assert!(r.mate_errors.is_empty(), "{:?}", r.mate_errors);
        let t = r.transforms[&InstanceId(2)];
        // B's bottom-front edge midpoint (5,0,0) lands on A's (5,0,5).
        let m = t.apply_point(Vec3::new(5.0, 0.0, 0.0));
        assert!(m.distance(Vec3::new(5.0, 0.0, 5.0)) < 1e-6, "{m:?}");
        // The edge stays along x.
        let d = t.apply_vector(Vec3::X);
        assert!(d.y.abs() < 1e-6 && d.z.abs() < 1e-6, "{d:?}");
    }

    #[test]
    fn connector_without_anchor_still_deserialises_as_a_face() {
        let c: Connector =
            serde_json::from_str(r#"{"instance":1,"face":{"feature":1,"local":2}}"#).unwrap();
        assert_eq!(c.anchor, Anchor::Face);
        let s = serde_json::to_string(&c).unwrap();
        assert!(!s.contains("anchor"), "{s}");
        let e = Connector {
            anchor: Anchor::Edge { other: fref(3) },
            ..c
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains(r#""anchor":{"type":"edge""#), "{s}");
        assert_eq!(serde_json::from_str::<Connector>(&s).unwrap(), e);
    }

    #[test]
    fn cylinder_connector_sits_on_the_axis() {
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(4.0, 3.0), 2.0);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        let cyl = ok_brep::extrude(&p, &Plane::XY, 0.0, 6.0, 1).unwrap();
        let f = connector_frame(
            &cyl,
            &FaceRef {
                feature: FeatureId(1),
                local: 2,
                part: None,
                near: Default::default(),
            },
            &Anchor::Face,
        )
        .unwrap();
        assert!(
            f.origin.distance(Vec3::new(4.0, 3.0, 3.0)) < 1e-9,
            "{:?}",
            f.origin
        );
        assert!((f.normal.z.abs() - 1.0).abs() < 1e-9);
    }
}
