use crate::{
    canonical_frame, BlendKind, BodyOp, EdgeRef, ExtrudeDirection, ExtrudeEnd, FaceRef, FeatureId,
    FeatureKind, PartStudio, PlaneRef, ProfileSelection, RevolveAxis,
};
use ok_brep::{boolean, BoolOp, Solid};
use ok_math::{Plane, Vec2, Vec3};
use ok_mesh::TriMesh;
use ok_sketch::{Entity, EntityId, Profile, ProfileOptions, SolveResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A solid body produced by regeneration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub name: String,
    /// Feature that created the body.
    pub source: FeatureId,
    pub solid: Solid,
    /// Display tessellation of `solid`.
    pub mesh: TriMesh,
    /// Index into `solid.faces` for every triangle of `mesh`, for picking.
    pub triangle_faces: Vec<u32>,
    /// Display edges of `solid` (between distinct surfaces).
    pub edges: Vec<ok_brep::DisplayEdge>,
}

impl Body {
    fn new(name: String, source: FeatureId, solid: Solid) -> Body {
        let (mesh, triangle_faces) = ok_brep::tessellate_with_faces(&solid);
        let edges = ok_brep::display_edges(&solid);
        Body {
            name,
            source,
            solid,
            mesh,
            triangle_faces,
            edges,
        }
    }

    /// The first face created by `face_ref`, if this body still has it.
    pub fn find_face(&self, face_ref: &FaceRef) -> Option<&ok_brep::Face> {
        self.solid
            .faces
            .iter()
            .find(|f| face_ref.matches(&f.origin))
    }

    /// Face index pairs for every edge between the two referenced faces.
    pub fn find_edge(&self, edge: &EdgeRef) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (_, faces) in self.solid.edge_faces() {
            if faces.len() != 2 {
                continue;
            }
            let (fa, fb) = (
                &self.solid.faces[faces[0]].origin,
                &self.solid.faces[faces[1]].origin,
            );
            if (edge.a.matches(fa) && edge.b.matches(fb))
                || (edge.a.matches(fb) && edge.b.matches(fa))
            {
                let pair = (faces[0], faces[1]);
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        }
        out
    }

    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.solid.bounds()
    }
}

/// A sketch entity tessellated into model space for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchCurve {
    pub entity: EntityId,
    pub kind: String,
    pub points: Vec<Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchResult {
    pub plane: Plane,
    pub solve: SolveResult,
    /// Closed regions in area-descending order.
    pub profiles: Vec<Profile>,
    pub curves: Vec<SketchCurve>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatus {
    pub id: FeatureId,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegenResult {
    pub bodies: Vec<Body>,
    pub sketches: BTreeMap<FeatureId, SketchResult>,
    pub statuses: Vec<FeatureStatus>,
    /// Running count used to name bodies "Part N".
    next_part: usize,
}

impl RegenResult {
    pub fn errors(&self) -> impl Iterator<Item = (FeatureId, &str)> {
        self.statuses
            .iter()
            .filter_map(|s| s.error.as_deref().map(|e| (s.id, e)))
    }
}

fn tessellate_entity(
    sketch: &ok_sketch::Sketch,
    e: &Entity,
    opts: &ProfileOptions,
) -> Option<Vec<Vec2>> {
    match e {
        Entity::Point { pos } => Some(vec![*pos]),
        Entity::Line { start, end } => {
            Some(vec![sketch.point(*start).ok()?, sketch.point(*end).ok()?])
        }
        Entity::Circle { center, radius } => {
            let c = sketch.point(*center).ok()?;
            let n = ((std::f64::consts::TAU / opts.arc_segment_angle).ceil() as usize).max(8);
            Some(
                (0..=n)
                    .map(|i| {
                        c + Vec2::from_angle(std::f64::consts::TAU * i as f64 / n as f64) * *radius
                    })
                    .collect(),
            )
        }
        Entity::Arc { center, start, end } => {
            let c = sketch.point(*center).ok()?;
            let s = sketch.point(*start).ok()?;
            let t = sketch.point(*end).ok()?;
            let r = 0.5 * (s.distance(c) + t.distance(c));
            let a0 = (s - c).angle();
            let mut sweep = ((t - c).angle() - a0).rem_euclid(std::f64::consts::TAU);
            if sweep < 1e-12 {
                sweep = std::f64::consts::TAU;
            }
            let n = ((sweep / opts.arc_segment_angle).ceil() as usize).max(2);
            Some(
                (0..=n)
                    .map(|i| c + Vec2::from_angle(a0 + sweep * i as f64 / n as f64) * r)
                    .collect(),
            )
        }
    }
}

fn tessellate_sketch(
    sketch: &ok_sketch::Sketch,
    plane: &Plane,
    opts: &ProfileOptions,
) -> Vec<SketchCurve> {
    let mut out = Vec::new();
    for (id, e) in sketch.entities() {
        if let Some(pts) = tessellate_entity(sketch, e, opts) {
            out.push(SketchCurve {
                entity: id,
                kind: e.kind_name().to_string(),
                points: pts.into_iter().map(|p| plane.to_world(p)).collect(),
            });
        }
    }
    out
}

/// Regeneration cache: the result state after each feature, keyed by a
/// hash chain of the (solved) feature definitions up to that point. A
/// regeneration reuses the longest unchanged prefix, so editing the last
/// feature, or dragging a point in the last sketch, does not re-run the
/// booleans of everything before it.
#[derive(Debug, Clone, Default)]
pub struct RegenCache {
    entries: Vec<(u64, RegenResult)>,
}

impl RegenCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn feature_hash(prev: u64, f: &crate::Feature) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    prev.hash(&mut h);
    f.id.hash(&mut h);
    f.suppressed.hash(&mut h);
    // The kind (including solved sketch geometry) as JSON; cheap for the
    // sizes involved and avoids a second hashing scheme for every type.
    serde_json::to_string(&f.kind)
        .unwrap_or_default()
        .hash(&mut h);
    h.finish()
}

impl PartStudio {
    /// Evaluates every feature in order. Sketches are solved in place so the
    /// document always stores solved geometry. Unchanged prefixes of the
    /// feature list are served from the cache.
    pub fn regenerate(&mut self) -> RegenResult {
        let opts = ProfileOptions::default();
        let mut result = RegenResult::default();
        let ids: Vec<FeatureId> = self.features.iter().map(|f| f.id).collect();
        let mut chain = 0u64;
        let mut cache_valid = true;
        for (index, id) in ids.into_iter().enumerate() {
            let pos = self.position(id).expect("feature present");
            // Solve sketches first so the hash covers solved geometry, which
            // makes a second regeneration with no edits a full cache hit.
            if !self.features[pos].suppressed {
                if let FeatureKind::Sketch(sf) = &mut self.features[pos].kind {
                    sf.sketch.solve();
                }
            }
            chain = feature_hash(chain, &self.features[pos]);
            if cache_valid {
                if let Some((h, cached)) = self.cache.entries.get(index) {
                    if *h == chain {
                        result = cached.clone();
                        continue;
                    }
                }
                cache_valid = false;
                self.cache.entries.truncate(index);
            }
            if self.features[pos].suppressed {
                result.statuses.push(FeatureStatus { id, error: None });
                self.cache.entries.push((chain, result.clone()));
                continue;
            }
            let error = match &mut self.features[pos].kind {
                FeatureKind::Sketch(sf) => {
                    let plane = match result.resolve_plane(&sf.plane) {
                        Ok(p) => p,
                        Err(e) => {
                            result.statuses.push(FeatureStatus { id, error: Some(e) });
                            self.cache.entries.push((chain, result.clone()));
                            continue;
                        }
                    };
                    let solve = sf.sketch.solve();
                    let mut profiles = sf.sketch.profiles(&opts);
                    profiles.sort_by(|a, b| {
                        b.area()
                            .partial_cmp(&a.area())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let curves = tessellate_sketch(&sf.sketch, &plane, &opts);
                    let err = if solve.ok() {
                        None
                    } else {
                        Some(format!(
                            "sketch constraints are inconsistent (max residual {:.3e})",
                            solve.max_residual
                        ))
                    };
                    result.sketches.insert(
                        id,
                        SketchResult {
                            plane,
                            solve,
                            profiles,
                            curves,
                        },
                    );
                    err
                }
                FeatureKind::Extrude(ef) => {
                    let ef = ef.clone();
                    Self::regen_extrude(&mut result, id, &ef, pos, &self.features)
                }
                FeatureKind::Revolve(rf) => {
                    let rf = rf.clone();
                    Self::regen_revolve(&mut result, id, &rf, pos, &self.features)
                }
                FeatureKind::Blend(bf) => {
                    let bf = bf.clone();
                    Self::regen_blend(&mut result, id, &bf)
                }
            };
            result.statuses.push(FeatureStatus { id, error });
            self.cache.entries.push((chain, result.clone()));
        }
        self.cache.entries.truncate(self.features.len());
        result
    }

    /// Regenerates fully, then returns the state after the first `count`
    /// features (a "rollback" view used while editing a feature).
    pub fn regenerate_to(&mut self, count: usize) -> RegenResult {
        let full = self.regenerate();
        if count >= self.features.len() {
            return full;
        }
        if count == 0 {
            return RegenResult::default();
        }
        self.cache
            .entries
            .get(count - 1)
            .map(|(_, r)| r.clone())
            .unwrap_or(full)
    }

    /// Drops the regeneration cache (e.g. after loading a document).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Checks that a solid feature's sketch exists, precedes it, and is
    /// active, then returns its regenerated result.
    fn source_sketch<'a>(
        result: &'a RegenResult,
        sketch: FeatureId,
        pos: usize,
        features: &[crate::Feature],
        what: &str,
    ) -> Result<&'a SketchResult, String> {
        match features.iter().position(|f| f.id == sketch) {
            None => return Err(format!("sketch {:?} no longer exists", sketch)),
            Some(sp) if sp >= pos => return Err(format!("{what} must come after its sketch")),
            Some(sp) if features[sp].suppressed => {
                return Err(format!("sketch '{}' is suppressed", features[sp].name))
            }
            _ => {}
        }
        let sr = result
            .sketches
            .get(&sketch)
            .ok_or("sketch did not regenerate")?;
        if sr.profiles.is_empty() {
            return Err(format!("sketch has no closed regions to {what}"));
        }
        Ok(sr)
    }

    fn select_profiles<'a>(
        sr: &'a SketchResult,
        selection: &ProfileSelection,
    ) -> Result<Vec<&'a Profile>, String> {
        let selected: Vec<&Profile> = match selection {
            ProfileSelection::All => sr.profiles.iter().collect(),
            ProfileSelection::Largest => sr.profiles.iter().take(1).collect(),
            ProfileSelection::Indices { indices } => {
                let mut v = Vec::new();
                for &i in indices {
                    match sr.profiles.get(i) {
                        Some(p) => v.push(p),
                        None => {
                            return Err(format!(
                                "region {i} does not exist (sketch has {})",
                                sr.profiles.len()
                            ))
                        }
                    }
                }
                v
            }
        };
        if selected.is_empty() {
            return Err("no regions selected".into());
        }
        Ok(selected)
    }

    /// Unions per-region solids into one tool volume.
    fn build_tool(
        parts: impl Iterator<Item = Result<Solid, ok_brep::BrepError>>,
    ) -> Result<Solid, String> {
        let mut tool = Solid::default();
        for part in parts {
            let part = part.map_err(|e| e.to_string())?;
            tool = boolean(&tool, &part, BoolOp::Union)
                .map_err(|e| format!("could not combine regions: {e}"))?;
        }
        Ok(tool)
    }

    fn regen_extrude(
        result: &mut RegenResult,
        id: FeatureId,
        ef: &crate::ExtrudeFeature,
        pos: usize,
        features: &[crate::Feature],
    ) -> Option<String> {
        let sr = match Self::source_sketch(result, ef.sketch, pos, features, "extrude") {
            Ok(sr) => sr,
            Err(e) => return Some(e),
        };
        let selected = match Self::select_profiles(sr, &ef.profiles) {
            Ok(v) => v,
            Err(e) => return Some(e),
        };
        let (start, end) = match result.extrude_range(ef, &sr.plane) {
            Ok(r) => r,
            Err(e) => return Some(e),
        };
        let plane = sr.plane;
        let tool = match Self::build_tool(
            selected
                .into_iter()
                .map(|p| ok_brep::extrude(p, &plane, start, end, id.0)),
        ) {
            Ok(t) => t,
            Err(e) => return Some(e),
        };
        Self::apply_tool(result, id, tool, ef.op)
    }

    fn regen_revolve(
        result: &mut RegenResult,
        id: FeatureId,
        rf: &crate::RevolveFeature,
        pos: usize,
        features: &[crate::Feature],
    ) -> Option<String> {
        let sr = match Self::source_sketch(result, rf.sketch, pos, features, "revolve") {
            Ok(sr) => sr,
            Err(e) => return Some(e),
        };
        let selected = match Self::select_profiles(sr, &rf.profiles) {
            Ok(v) => v,
            Err(e) => return Some(e),
        };
        let (axis_point, axis_dir) = match rf.axis {
            RevolveAxis::XAxis => (Vec2::ZERO, Vec2::X),
            RevolveAxis::YAxis => (Vec2::ZERO, Vec2::Y),
            RevolveAxis::Line { line } => {
                let sketch_pos = features.iter().position(|f| f.id == rf.sketch).unwrap();
                let FeatureKind::Sketch(sf) = &features[sketch_pos].kind else {
                    unreachable!()
                };
                let Ok((a, b)) = sf.sketch.line(line) else {
                    return Some(format!("axis line {:?} does not exist in the sketch", line));
                };
                let (pa, pb) = (sf.sketch.point(a).ok()?, sf.sketch.point(b).ok()?);
                (pa, pb - pa)
            }
        };
        if !rf.angle.is_finite() || rf.angle.abs() < 1e-9 {
            return Some("angle must be non-zero".into());
        }
        let angle = rf.angle.clamp(-360.0, 360.0).to_radians();
        let plane = sr.plane;
        let seg = ProfileOptions::default().arc_segment_angle;
        let tool = match Self::build_tool(
            selected
                .into_iter()
                .map(|p| ok_brep::revolve(p, &plane, axis_point, axis_dir, angle, seg, id.0)),
        ) {
            Ok(t) => t,
            Err(e) => return Some(e),
        };
        Self::apply_tool(result, id, tool, rf.op)
    }

    fn regen_blend(
        result: &mut RegenResult,
        id: FeatureId,
        bf: &crate::BlendFeature,
    ) -> Option<String> {
        if bf.edges.is_empty() {
            return None; // nothing selected yet: a no-op rather than an error
        }
        let kind = match bf.kind {
            BlendKind::Fillet => ok_brep::BlendKind::Fillet,
            BlendKind::Chamfer => ok_brep::BlendKind::Chamfer,
        };
        let seg = ProfileOptions::default().arc_segment_angle;
        let mut matched = 0usize;
        for i in 0..result.bodies.len() {
            let pairs: Vec<(usize, usize)> = bf
                .edges
                .iter()
                .flat_map(|e| result.bodies[i].find_edge(e))
                .collect();
            if pairs.is_empty() {
                continue;
            }
            matched += pairs.len();
            let blended = match ok_brep::blend_edges(
                &result.bodies[i].solid,
                &pairs,
                bf.size,
                kind,
                seg,
                id.0,
            ) {
                Ok(s) => s,
                Err(e) => return Some(e.to_string()),
            };
            let body = &result.bodies[i];
            result.bodies[i] = Body::new(body.name.clone(), body.source, blended);
        }
        if matched == 0 {
            return Some("none of the referenced edges exist any more".into());
        }
        None
    }

    /// Combines a finished tool volume with the existing bodies.
    fn apply_tool(
        result: &mut RegenResult,
        id: FeatureId,
        tool: Solid,
        op: BodyOp,
    ) -> Option<String> {
        let ef_op = op;
        let tool_bounds = tool.bounds()?;

        // Bodies the tool touches (bounding boxes overlap).
        let touched: Vec<usize> = result
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.bounds().is_some_and(|bb| boxes_touch(bb, tool_bounds)))
            .map(|(i, _)| i)
            .collect();

        match ef_op {
            BodyOp::New => {
                result.push_body(id, tool);
            }
            BodyOp::Add if touched.is_empty() => {
                result.push_body(id, tool);
            }
            BodyOp::Add => {
                let mut merged = tool;
                for &i in &touched {
                    merged = match boolean(&result.bodies[i].solid, &merged, BoolOp::Union) {
                        Ok(s) => s,
                        Err(e) => return Some(format!("union failed: {e}")),
                    };
                }
                let name = result.bodies[touched[0]].name.clone();
                let source = result.bodies[touched[0]].source;
                result.remove_bodies(&touched);
                result.bodies.push(Body::new(name, source, merged));
            }
            BodyOp::Remove | BodyOp::Intersect => {
                if touched.is_empty() {
                    return Some("the tool volume does not touch any body".into());
                }
                let op = if ef_op == BodyOp::Remove {
                    BoolOp::Difference
                } else {
                    BoolOp::Intersection
                };
                let mut replacements: Vec<(usize, Vec<Solid>)> = Vec::new();
                for &i in &touched {
                    match boolean(&result.bodies[i].solid, &tool, op) {
                        Ok(s) => replacements.push((i, s.shells())),
                        Err(e) => {
                            return Some(format!(
                                "{} failed: {e}",
                                if op == BoolOp::Difference {
                                    "cut"
                                } else {
                                    "intersect"
                                }
                            ))
                        }
                    }
                }
                let mut removed_everything = true;
                for (i, shells) in replacements {
                    let body = &result.bodies[i];
                    let (name, source) = (body.name.clone(), body.source);
                    let mut shells = shells.into_iter();
                    match shells.next() {
                        Some(first) => {
                            removed_everything = false;
                            result.bodies[i] = Body::new(name, source, first);
                            for extra in shells {
                                result.push_body(id, extra);
                            }
                        }
                        None => result.bodies[i].solid = Solid::default(),
                    }
                }
                result.bodies.retain(|b| !b.solid.is_empty());
                if removed_everything {
                    return Some("the operation removed every touched body".into());
                }
            }
        }
        None
    }
}

fn boxes_touch(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let t = 1e-6;
    a.0.x <= b.1.x + t
        && b.0.x <= a.1.x + t
        && a.0.y <= b.1.y + t
        && b.0.y <= a.1.y + t
        && a.0.z <= b.1.z + t
        && b.0.z <= a.1.z + t
}

impl RegenResult {
    /// Finds a referenced face among the current bodies.
    fn find_face(&self, face_ref: &FaceRef) -> Result<&ok_brep::Face, String> {
        self.bodies
            .iter()
            .find_map(|b| b.find_face(face_ref))
            .ok_or_else(|| {
                format!(
                    "referenced face (feature {}, face {}) no longer exists",
                    face_ref.feature.0, face_ref.local
                )
            })
    }

    /// Finds a referenced face and requires it to be planar.
    fn find_planar_face(&self, face_ref: &FaceRef) -> Result<Plane, String> {
        let face = self.find_face(face_ref)?;
        let solid_face_plane = face.plane;
        // Surface lookup: any body that owns the face has the matching surface table.
        let planar = self.bodies.iter().any(|b| {
            b.solid.faces.iter().any(|f| std::ptr::eq(f, face))
                && matches!(
                    b.solid.surfaces.get(face.surface),
                    Some(ok_brep::Surface::Plane { .. })
                )
        });
        if !planar {
            return Err("referenced face is not planar".into());
        }
        Ok(solid_face_plane)
    }

    /// Resolves a plane reference to a sketch frame.
    pub fn resolve_plane(&self, plane: &PlaneRef) -> Result<Plane, String> {
        match plane {
            PlaneRef::Standard { base, offset } => Ok(base.plane().offset(*offset)),
            PlaneRef::Face { face, offset } => {
                let p = self.find_planar_face(face)?;
                Ok(canonical_frame(&p).offset(*offset))
            }
        }
    }

    /// Largest extent of all bodies from `origin` along `dir`, plus margin.
    fn extent_along(&self, origin: Vec3, dir: Vec3) -> Option<f64> {
        let mut max: Option<f64> = None;
        for b in &self.bodies {
            if let Some((lo, hi)) = b.bounds() {
                for corner in [
                    Vec3::new(lo.x, lo.y, lo.z),
                    Vec3::new(hi.x, lo.y, lo.z),
                    Vec3::new(lo.x, hi.y, lo.z),
                    Vec3::new(hi.x, hi.y, lo.z),
                    Vec3::new(lo.x, lo.y, hi.z),
                    Vec3::new(hi.x, lo.y, hi.z),
                    Vec3::new(lo.x, hi.y, hi.z),
                    Vec3::new(hi.x, hi.y, hi.z),
                ] {
                    let d = (corner - origin).dot(dir);
                    max = Some(max.map_or(d, |m: f64| m.max(d)));
                }
            }
        }
        max.map(|m| m.max(0.0) + 1.0)
    }

    /// Start and end heights (along the sketch normal) of an extrusion.
    fn extrude_range(
        &self,
        ef: &crate::ExtrudeFeature,
        plane: &Plane,
    ) -> Result<(f64, f64), String> {
        let n = plane.normal;
        match ef.end {
            ExtrudeEnd::Blind
                if !(ef.depth.is_finite() && ef.depth.abs() > ok_math::tol::LINEAR) =>
            {
                Err("depth must be non-zero".into())
            }
            ExtrudeEnd::Blind => Ok(match ef.direction {
                ExtrudeDirection::Normal => (0.0, ef.depth),
                ExtrudeDirection::Reverse => (0.0, -ef.depth),
                ExtrudeDirection::Symmetric => (-ef.depth / 2.0, ef.depth / 2.0),
            }),
            ExtrudeEnd::ThroughAll => {
                let fwd = self.extent_along(plane.origin, n);
                let back = self.extent_along(plane.origin, -n);
                let (Some(fwd), Some(back)) = (fwd, back) else {
                    return Err("through all needs an existing body".into());
                };
                Ok(match ef.direction {
                    ExtrudeDirection::Normal => (0.0, fwd),
                    ExtrudeDirection::Reverse => (0.0, -back),
                    ExtrudeDirection::Symmetric => (-back, fwd),
                })
            }
            ExtrudeEnd::UpToFace { face } => {
                let target = self.find_planar_face(&face)?;
                let dir = match ef.direction {
                    ExtrudeDirection::Normal => n,
                    ExtrudeDirection::Reverse => -n,
                    ExtrudeDirection::Symmetric => {
                        return Err("up to face cannot be symmetric".into())
                    }
                };
                let denom = target.normal.dot(dir);
                if denom.abs() < 1e-9 {
                    return Err("target face is parallel to the extrude direction".into());
                }
                let t = target.normal.dot(target.origin - plane.origin) / denom;
                if t <= ok_math::tol::LINEAR {
                    return Err("target face is behind the sketch plane".into());
                }
                Ok(if ef.direction == ExtrudeDirection::Reverse {
                    (0.0, -t)
                } else {
                    (0.0, t)
                })
            }
        }
    }

    fn push_body(&mut self, source: FeatureId, solid: Solid) {
        self.next_part += 1;
        self.bodies
            .push(Body::new(format!("Part {}", self.next_part), source, solid));
    }

    fn remove_bodies(&mut self, indices: &[usize]) {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        for i in sorted {
            self.bodies.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExtrudeEnd, Op, PlaneRef, SketchOp, StandardPlane};
    use ok_sketch::Constraint;

    #[test]
    fn demo_regenerates_without_errors() {
        let mut ps = PartStudio::demo();
        let r = ps.regenerate();
        let errors: Vec<_> = r.errors().collect();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(r.bodies.len(), 1);
        let plate = 60.0 * 40.0 * 8.0 - std::f64::consts::PI * 36.0 * 8.0;
        let boss = std::f64::consts::PI * (100.0 - 36.0) * 6.0;
        // The slot removes the boss material within |x - 30| < 5 for z in 10..14:
        // 4 * (band area of the r=10 disc minus band area of the r=6 hole).
        let band = |r: f64| 2.0 * (5.0 * (r * r - 25.0).sqrt() + r * r * (5.0 / r).asin());
        let slot = 4.0 * (band(10.0) - band(6.0));
        let expected = plate + boss - slot;
        let vol = r.bodies[0].solid.volume();
        assert!(
            (vol - expected).abs() / expected < 5e-3,
            "vol {vol}, expected {expected}"
        );
        r.bodies[0].solid.validate().unwrap();
    }

    #[test]
    fn ops_build_a_box_and_round_trip_json() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Front),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r = ps
            .apply(Op::Sketch {
                id: s,
                op: SketchOp::AddRectangle {
                    a: Vec2::new(-1.0, -1.0),
                    b: Vec2::new(1.0, 1.0),
                },
            })
            .unwrap();
        assert_eq!(r.entities.len(), 4);
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddConstraint {
                constraint: Constraint::Length {
                    line: r.entities[0],
                    value: 4.0,
                },
            },
        })
        .unwrap();
        let e = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 5.0,
                direction: ExtrudeDirection::Symmetric,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let json = ps.to_json();
        let mut ps2 = PartStudio::from_json(&json).unwrap();
        let r = ps2.regenerate();
        assert!(r.errors().next().is_none());
        assert_eq!(r.bodies.len(), 1);
        assert!((r.bodies[0].solid.volume() - 4.0 * 2.0 * 5.0).abs() < 1e-6);
        let (min, max) = r.bodies[0].mesh.bounds().unwrap();
        assert!(
            (min.y + 2.5).abs() < 1e-6 && (max.y - 2.5).abs() < 1e-6,
            "symmetric about the front plane"
        );
        assert_eq!(ps2.feature(e).unwrap().name, "Extrude 1");
    }

    #[test]
    fn extrude_before_sketch_is_an_error() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddCircle {
                center: Vec2::ZERO,
                radius: 1.0,
            },
        })
        .unwrap();
        let e = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 1.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::MoveFeature { id: e, index: 0 }).unwrap();
        let r = ps.regenerate();
        assert_eq!(r.errors().count(), 1);
        assert!(r.bodies.is_empty());
    }

    #[test]
    fn apply_json_ops() {
        let mut ps = PartStudio::new("t");
        let r = ps
            .apply_json(
                r#"{"type":"add_sketch","plane":{"type":"standard","base":"top"},"name":null}"#,
            )
            .unwrap();
        let id = r.feature.unwrap().0;
        ps.apply_json(&format!(r#"{{"type":"sketch","id":{id},"op":{{"type":"add_circle","center":{{"x":0,"y":0}},"radius":3}}}}"#)).unwrap();
        ps.apply_json(&format!(
            r#"{{"type":"add_extrude","sketch":{id},"depth":2,"name":null}}"#
        ))
        .unwrap();
        let r = ps.regenerate();
        assert_eq!(r.bodies.len(), 1);
        assert!((r.bodies[0].solid.volume() - std::f64::consts::PI * 9.0 * 2.0).abs() < 0.2);
    }

    #[test]
    fn remove_cuts_and_splits_bodies() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddRectangle {
                a: Vec2::ZERO,
                b: Vec2::new(10.0, 2.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s,
            depth: 2.0,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::AddRectangle {
                a: Vec2::new(4.0, -1.0),
                b: Vec2::new(6.0, 3.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s2,
            depth: 10.0,
            direction: ExtrudeDirection::Symmetric,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::Remove,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 2, "the cut split the bar in two");
        let total: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        assert!((total - 32.0).abs() < 1e-6);
    }

    #[test]
    fn sketch_on_face_and_extrude_up_to_face() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddRectangle {
                a: Vec2::ZERO,
                b: Vec2::new(10.0, 10.0),
            },
        })
        .unwrap();
        let e = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 4.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        // Sketch on the top face (local 1) of the block; a 2x2 square at its centre.
        let top = crate::FaceRef {
            feature: e,
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
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::AddRectangle {
                a: Vec2::new(4.0, 4.0),
                b: Vec2::new(6.0, 6.0),
            },
        })
        .unwrap();
        let e2 = ps
            .apply(Op::AddExtrude {
                sketch: s2,
                depth: 3.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::Add,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 1);
        assert!(
            (r.bodies[0].solid.volume() - 412.0).abs() < 1e-6,
            "boss sits on top: {}",
            r.bodies[0].solid.volume()
        );
        let (_, max) = r.bodies[0].bounds().unwrap();
        assert!((max.z - 7.0).abs() < 1e-9);

        // Grow the block: the boss sketch follows the top face.
        ps.apply(Op::SetExtrude {
            id: e,
            depth: Some(6.0),
            direction: None,
            end: None,
            profiles: None,
            op: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.errors().next().is_none());
        let (_, max) = r.bodies[0].bounds().unwrap();
        assert!((max.z - 9.0).abs() < 1e-9);

        // A cut from the boss top down to the block's top face.
        let boss_top = crate::FaceRef {
            feature: e2,
            local: 1,
        };
        let s3 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Face {
                    face: boss_top,
                    offset: 0.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s3,
            op: SketchOp::AddCircle {
                center: Vec2::new(5.0, 5.0),
                radius: 0.5,
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s3,
            depth: 0.0,
            direction: ExtrudeDirection::Reverse,
            end: ExtrudeEnd::UpToFace { face: top },
            profiles: ProfileSelection::All,
            op: BodyOp::Remove,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let expected = 600.0 + 12.0 - std::f64::consts::PI * 0.25 * 3.0;
        let vol = r.bodies[0].solid.volume();
        assert!(
            (vol - expected).abs() < 0.05,
            "vol {vol} expected {expected}"
        );

        // Through all from the side splits nothing but cuts the whole width.
        let s4 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Front),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s4,
            op: SketchOp::AddRectangle {
                a: Vec2::new(1.0, 1.0),
                b: Vec2::new(2.0, 2.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s4,
            depth: 0.0,
            direction: ExtrudeDirection::Symmetric,
            end: ExtrudeEnd::ThroughAll,
            profiles: ProfileSelection::All,
            op: BodyOp::Remove,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let vol2 = r.bodies[0].solid.volume();
        assert!((vol2 - (vol - 10.0)).abs() < 0.05, "vol2 {vol2}");
    }

    #[test]
    fn revolve_feature_makes_a_ring_and_cuts() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Front),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddRectangle {
                a: Vec2::new(3.0, 0.0),
                b: Vec2::new(5.0, 1.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddRevolve {
            sketch: s,
            axis: crate::RevolveAxis::YAxis,
            angle: 360.0,
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let expected = std::f64::consts::TAU * 4.0 * 2.0;
        let vol = r.bodies[0].solid.volume();
        assert!(((vol - expected) / expected).abs() < 3e-3, "vol {vol}");
        // A revolve about a sketch line, removing material: a groove.
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Front),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r2 = ps
            .apply(Op::Sketch {
                id: s2,
                op: SketchOp::AddLine {
                    a: Vec2::new(0.0, -10.0),
                    b: Vec2::new(0.0, 10.0),
                },
            })
            .unwrap();
        let axis_line = r2.entities[0];
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::AddRectangle {
                a: Vec2::new(4.5, 0.4),
                b: Vec2::new(6.0, 0.6),
            },
        })
        .unwrap();
        ps.apply(Op::AddRevolve {
            sketch: s2,
            axis: crate::RevolveAxis::Line { line: axis_line },
            angle: 360.0,
            profiles: ProfileSelection::All,
            op: BodyOp::Remove,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let groove = std::f64::consts::TAU * 4.75 * (0.5 * 0.2);
        let vol2 = r.bodies[0].solid.volume();
        assert!(
            ((vol2 - (vol - groove)) / vol).abs() < 3e-3,
            "vol2 {vol2}, expected {}",
            vol - groove
        );
    }

    #[test]
    fn fillet_feature_by_edge_reference() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddRectangle {
                a: Vec2::ZERO,
                b: Vec2::new(10.0, 10.0),
            },
        })
        .unwrap();
        let e = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 10.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        // Top cap is local 1; walls are local 2.. in loop order (bottom, right, top, left).
        let top = crate::FaceRef {
            feature: e,
            local: 1,
        };
        let front = crate::FaceRef {
            feature: e,
            local: 2,
        };
        let f = ps
            .apply(Op::AddBlend {
                kind: BlendKind::Fillet,
                edges: vec![crate::EdgeRef { a: top, b: front }],
                size: 2.0,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let expected = 1000.0 - (4.0 - std::f64::consts::PI) * 10.0;
        let vol = r.bodies[0].solid.volume();
        assert!(((vol - expected) / expected).abs() < 2e-3, "vol {vol}");
        assert_eq!(ps.feature(f).unwrap().name, "Fillet 1");
        // Rollback view before the fillet shows the plain block.
        let rolled = ps.regenerate_to(2);
        assert!((rolled.bodies[0].solid.volume() - 1000.0).abs() < 1e-6);
        // Growing the block keeps the fillet on the same edge.
        ps.apply(Op::SetExtrude {
            id: e,
            depth: Some(20.0),
            direction: None,
            end: None,
            profiles: None,
            op: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.errors().next().is_none());
        let expected = 2000.0 - (4.0 - std::f64::consts::PI) * 10.0;
        assert!(((r.bodies[0].solid.volume() - expected) / expected).abs() < 2e-3);
    }

    #[test]
    fn deleted_face_reference_is_an_error() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddRectangle {
                a: Vec2::ZERO,
                b: Vec2::new(2.0, 2.0),
            },
        })
        .unwrap();
        let e = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 1.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Face {
                    face: crate::FaceRef {
                        feature: e,
                        local: 1,
                    },
                    offset: 0.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::SetSuppressed {
            id: e,
            suppressed: true,
        })
        .unwrap();
        let r = ps.regenerate();
        assert_eq!(r.errors().count(), 1);
        assert!(r.errors().next().unwrap().0 == s2);
    }

    #[test]
    fn cache_reuses_unchanged_prefix_and_invalidates_edits() {
        let mut ps = PartStudio::demo();
        let r1 = ps.regenerate();
        let v1 = r1.bodies[0].solid.volume();
        // A no-op regeneration is a full cache hit and gives the same result.
        let before = ps.cache.entries.len();
        let r2 = ps.regenerate();
        assert_eq!(before, ps.cache.entries.len());
        assert!((r2.bodies[0].solid.volume() - v1).abs() < 1e-9);
        // Editing the last feature keeps the earlier entries.
        let last = ps.features().last().unwrap().id;
        ps.apply(Op::SetSuppressed {
            id: last,
            suppressed: true,
        })
        .unwrap();
        let r3 = ps.regenerate();
        assert!(r3.bodies[0].solid.volume() > v1, "slot no longer cut");
        // Editing the first sketch invalidates everything after it.
        let first = ps.features()[0].id;
        let c = ps.features()[0].kind.clone();
        let dim = match c {
            FeatureKind::Sketch(s) => s
                .sketch
                .constraints()
                .find(|(_, c)| c.value() == Some(60.0))
                .map(|(id, _)| id)
                .unwrap(),
            _ => unreachable!(),
        };
        ps.apply(Op::Sketch {
            id: first,
            op: SketchOp::SetConstraintValue {
                id: dim,
                value: 80.0,
            },
        })
        .unwrap();
        let r4 = ps.regenerate();
        let (min, max) = r4.bodies[0].bounds().unwrap();
        assert!((max.x - min.x - 80.0).abs() < 1e-6, "plate is now 80 wide");
        assert!(r4.errors().next().is_none());
    }

    #[test]
    fn remove_touching_nothing_is_an_error() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddCircle {
                center: Vec2::ZERO,
                radius: 1.0,
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s,
            depth: 1.0,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::Remove,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert_eq!(r.errors().count(), 1);
        assert!(r.bodies.is_empty());
    }
}
