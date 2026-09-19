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
}

/// A face of an instance's body, used as a mate connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    pub instance: InstanceId,
    pub face: FaceRef,
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
    /// One placed body per instance that resolved, in instance order.
    #[serde(skip)]
    pub bodies: Vec<crate::Body>,
    /// Instance ids in the same order as `bodies`.
    pub placed: Vec<InstanceId>,
    pub transforms: BTreeMap<InstanceId, Transform>,
    pub instance_errors: BTreeMap<InstanceId, String>,
    pub mate_errors: BTreeMap<MateId, String>,
}

/// Connector frame of a face in body coordinates: origin at the face
/// centroid (on the axis for a cylindrical face), z along the normal or
/// axis, x and y canonical for that normal.
pub fn connector_frame(solid: &Solid, face: &FaceRef) -> Option<Plane> {
    let faces: Vec<&ok_brep::Face> = solid
        .faces
        .iter()
        .filter(|f| face.matches(&f.origin))
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
    /// Places every instance given each instance's source solid (in body
    /// coordinates), resolving mates as chains from fixed instances.
    pub fn resolve(&self, solids: &BTreeMap<InstanceId, Solid>) -> AssemblyResult {
        let mut result = AssemblyResult::default();
        let mut placed: BTreeMap<InstanceId, Transform> = BTreeMap::new();
        for inst in &self.instances {
            if !solids.contains_key(&inst.id) {
                result
                    .instance_errors
                    .insert(inst.id, "the part studio no longer has this body".into());
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
            let solid = solids
                .get(&c.instance)
                .ok_or_else(|| "instance has no body".to_string())?;
            connector_frame(solid, &c.face).ok_or_else(|| {
                format!(
                    "face {} of feature {} not found on the instance",
                    c.face.local, c.face.feature.0
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
        for inst in &self.instances {
            let (Some(solid), Some(xf)) = (solids.get(&inst.id), placed.get(&inst.id)) else {
                continue;
            };
            result.bodies.push(crate::Body::new(
                inst.name.clone(),
                FeatureId(inst.id.0),
                solid.transformed(xf),
            ));
            result.placed.push(inst.id);
            result.transforms.insert(inst.id, *xf);
        }
        result
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
        };
        let bottom = FaceRef {
            feature: FeatureId(1),
            local: 0,
        };
        asm.mates.push(Mate {
            id: MateId(3),
            name: "m".into(),
            kind: MateKind::Fastened,
            a: Connector {
                instance: InstanceId(1),
                face: top,
            },
            b: Connector {
                instance: InstanceId(2),
                face: bottom,
            },
            offset: 0.0,
            angle: 0.0,
            flip: false,
        });
        let solids: BTreeMap<InstanceId, Solid> = [
            (InstanceId(1), solid.clone()),
            (InstanceId(2), solid.clone()),
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
        };
        let bottom = FaceRef {
            feature: FeatureId(1),
            local: 0,
        };
        let mate = |id: u32, a: u32, b: u32| Mate {
            id: MateId(id),
            name: format!("m{id}"),
            kind: MateKind::Fastened,
            a: Connector {
                instance: InstanceId(a),
                face: top,
            },
            b: Connector {
                instance: InstanceId(b),
                face: bottom,
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
        let solids: BTreeMap<InstanceId, Solid> =
            (1..=3).map(|i| (InstanceId(i), solid.clone())).collect();
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
            },
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
