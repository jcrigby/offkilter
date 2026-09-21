use crate::{
    canonical_frame, BlendKind, BodyOp, BooleanFeature, BooleanOp, CopyOp, EdgeRef,
    ExtrudeDirection, ExtrudeEnd, FaceRef, FeatureId, FeatureKind, MeshFeature, PartStudio,
    PatternKind, PlaneRef, ProfileSelection, RevolveAxis, ShellFeature,
};
use ok_brep::{boolean, BoolOp, Solid, Transform};
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
    /// The part's material, when one is assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<crate::Material>,
}

/// The neighbour hashes a reference to each face's piece records (see
/// `FaceRef::near`): a sample of `piece_neighbours`.
pub fn piece_near(solid: &Solid, parts: &[u32]) -> Vec<[u32; crate::NEAR]> {
    piece_neighbours(solid, parts)
        .into_iter()
        .map(crate::near_of)
        .collect()
}

/// The origin hashes of the faces across every face's piece's edges that
/// have another origin, sorted, for every face.
pub fn piece_neighbours(solid: &Solid, parts: &[u32]) -> Vec<Vec<u32>> {
    let mut sets: BTreeMap<(u32, u32, u32), Vec<u32>> = BTreeMap::new();
    let key = |i: usize| {
        let o = solid.faces[i].origin;
        (o.feature, o.local, parts[i])
    };
    for (_, faces) in solid.edge_faces() {
        for (k, &i) in faces.iter().enumerate() {
            for &j in &faces[k + 1..] {
                let (oi, oj) = (solid.faces[i].origin, solid.faces[j].origin);
                if oi == oj {
                    continue;
                }
                sets.entry(key(i))
                    .or_default()
                    .push(crate::origin_hash(oj.feature, oj.local));
                sets.entry(key(j))
                    .or_default()
                    .push(crate::origin_hash(oi.feature, oi.local));
            }
        }
    }
    for v in sets.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    (0..solid.faces.len())
        .map(|i| sets.get(&key(i)).cloned().unwrap_or_default())
        .collect()
}

/// The faces a reference means: those of its origin in the piece it
/// names. The piece is found by the neighbours recorded in the reference
/// when it has them and one piece matches them best (so it follows the
/// piece when an edit reorders the pieces by position, and finds the one
/// piece left when the split heals); otherwise by the piece number.
pub fn faces_of_ref(solid: &Solid, r: &FaceRef) -> Vec<usize> {
    let parts = solid.face_parts();
    let matching: Vec<usize> = (0..solid.faces.len())
        .filter(|&i| r.matches(&solid.faces[i].origin))
        .collect();
    let mut pieces: Vec<u32> = matching.iter().map(|&i| parts[i]).collect();
    pieces.sort_unstable();
    pieces.dedup();
    let positional = r.part.unwrap_or(0);
    let want = if !r.has_near() {
        positional
    } else if pieces.len() <= 1 {
        pieces.first().copied().unwrap_or(positional)
    } else {
        // Scored against every neighbour a piece has now, since the
        // reference kept a sample of the neighbours it had when made.
        let near = piece_neighbours(solid, &parts);
        let score = |piece: u32| -> usize {
            let i = matching
                .iter()
                .copied()
                .find(|&i| parts[i] == piece)
                .unwrap();
            r.near
                .iter()
                .filter(|h| **h != 0 && near[i].binary_search(h).is_ok())
                .count()
        };
        let scored: Vec<(u32, usize)> = pieces.iter().map(|&p| (p, score(p))).collect();
        let best = scored.iter().map(|s| s.1).max().unwrap_or(0);
        let tied: Vec<u32> = scored.iter().filter(|s| s.1 == best).map(|s| s.0).collect();
        if tied.len() == 1 {
            tied[0]
        } else if tied.contains(&positional) {
            positional
        } else {
            tied[0]
        }
    };
    matching.into_iter().filter(|&i| parts[i] == want).collect()
}

impl Body {
    pub(crate) fn new(name: String, source: FeatureId, solid: Solid) -> Body {
        let (mesh, triangle_faces) = ok_brep::tessellate_with_faces(&solid);
        let edges = ok_brep::display_edges(&solid);
        Body {
            name,
            source,
            solid,
            mesh,
            triangle_faces,
            edges,
            material: None,
        }
    }

    /// The first face created by `face_ref`, if this body still has it.
    pub fn find_face(&self, face_ref: &FaceRef) -> Option<&ok_brep::Face> {
        faces_of_ref(&self.solid, face_ref)
            .first()
            .map(|&i| &self.solid.faces[i])
    }

    /// Indices of every face reachable from the referenced faces across
    /// shared edges without leaving their surface (so one facet of a
    /// cylinder names the whole cylinder, while two coplanar pieces left
    /// by a slot stay apart).
    pub fn faces_on_surfaces_of(&self, refs: &[FaceRef]) -> Vec<usize> {
        let seeds: Vec<usize> = refs
            .iter()
            .flat_map(|r| faces_of_ref(&self.solid, r))
            .collect();
        self.connected_on_surfaces(&seeds)
    }

    /// Faces connected to `seeds` through shared edges between faces of one
    /// surface.
    fn connected_on_surfaces(&self, seeds: &[usize]) -> Vec<usize> {
        let n = self.solid.faces.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for (_, faces) in self.solid.edge_faces() {
            for w in faces.windows(2) {
                if self.solid.faces[w[0]].surface == self.solid.faces[w[1]].surface {
                    let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
                    parent[a] = b;
                }
            }
        }
        let roots: Vec<usize> = seeds.iter().map(|&i| find(&mut parent, i)).collect();
        (0..n)
            .filter(|&i| roots.contains(&find(&mut parent, i)))
            .collect()
    }

    /// Face index pairs for every edge between the two referenced faces.
    pub fn find_edge(&self, edge: &EdgeRef) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let fa_set = faces_of_ref(&self.solid, &edge.a);
        let fb_set = faces_of_ref(&self.solid, &edge.b);
        for (_, faces) in self.solid.edge_faces() {
            if faces.len() != 2 {
                continue;
            }
            if (fa_set.contains(&faces[0]) && fb_set.contains(&faces[1]))
                || (fa_set.contains(&faces[1]) && fb_set.contains(&faces[0]))
            {
                let pair = (faces[0], faces[1]);
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        }
        out
    }

    /// Face index pairs for every edge between the surfaces of the two
    /// referenced faces (each grown across its surface as
    /// `faces_on_surfaces_of` does), so one picked segment of a faceted rim
    /// names the whole rim.
    pub fn find_edge_on_surfaces(&self, edge: &EdgeRef) -> Vec<(usize, usize)> {
        let fa = self.faces_on_surfaces_of(&[edge.a]);
        let fb = self.faces_on_surfaces_of(&[edge.b]);
        if fa.is_empty() || fb.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (_, faces) in self.solid.edge_faces() {
            if faces.len() != 2 {
                continue;
            }
            let (s0, s1) = (
                self.solid.faces[faces[0]].surface,
                self.solid.faces[faces[1]].surface,
            );
            if s0 == s1 {
                continue;
            }
            if (fa.contains(&faces[0]) && fb.contains(&faces[1]))
                || (fa.contains(&faces[1]) && fb.contains(&faces[0]))
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
    #[serde(default)]
    pub construction: bool,
    /// Projected from body geometry; fixed for the solver.
    #[serde(default)]
    pub projected: bool,
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
    /// Evaluated value of a variable feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Bodies that existed just before a boolean feature, as (source, name),
    /// so a client can offer them as targets and tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<(FeatureId, String)>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegenResult {
    pub bodies: Vec<Body>,
    pub sketches: BTreeMap<FeatureId, SketchResult>,
    pub statuses: Vec<FeatureStatus>,
    /// Variable values defined so far.
    pub variables: BTreeMap<String, f64>,
    /// Running count used to name bodies "Part N".
    next_part: usize,
    /// Tool volume and body operation of every solid feature so far, for
    /// feature patterns and mirrors to replay.
    #[serde(skip)]
    tools: BTreeMap<FeatureId, (Solid, BodyOp)>,
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
        Entity::Spline { points } => {
            let pts: Vec<Vec2> = points
                .iter()
                .map(|p| sketch.point(*p).ok())
                .collect::<Option<_>>()?;
            Some(ok_sketch::spline_polyline(
                &pts,
                ok_sketch::spline_pieces(opts),
            ))
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
                construction: sketch.is_construction(id),
                projected: sketch.is_projected(id),
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

fn feature_hash_seed(settings: &crate::Settings) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    settings.facet_angle.to_bits().hash(&mut h);
    h.finish()
}

fn feature_hash(prev: u64, f: &crate::Feature) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    prev.hash(&mut h);
    f.id.hash(&mut h);
    f.suppressed.hash(&mut h);
    for (k, v) in &f.bindings {
        k.hash(&mut h);
        v.hash(&mut h);
    }
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
        let opts = self.settings.profile_options();
        let mut result = RegenResult::default();
        let ids: Vec<FeatureId> = self.features.iter().map(|f| f.id).collect();
        // Settings seed the cache chain so a resolution change regenerates everything.
        let mut chain = feature_hash_seed(&self.settings);
        let mut cache_valid = true;
        for (index, id) in ids.into_iter().enumerate() {
            let pos = self.position(id).expect("feature present");
            // Evaluate expression bindings into their fields, then solve
            // sketches, so the hash covers the effective definition and a
            // second regeneration with no edits is a full cache hit.
            let mut binding_error: Option<String> = None;
            if !self.features[pos].suppressed {
                let bindings = self.features[pos].bindings.clone();
                for (field, expression) in &bindings {
                    match crate::expr::evaluate(expression, &result.variables) {
                        Ok(v) => {
                            if let Err(e) = self.features[pos].kind.set_field(field, v) {
                                binding_error = Some(e);
                            }
                        }
                        Err(e) => binding_error = Some(format!("{field}: {e}")),
                    }
                }
                if let FeatureKind::Sketch(sf) = &mut self.features[pos].kind {
                    if !sf.projections.is_empty() {
                        let projected = result.resolve_plane(&sf.plane).and_then(|plane| {
                            crate::project::update_projections(&result, &plane, sf)
                        });
                        if let Err(e) = projected {
                            binding_error.get_or_insert(e);
                        }
                    }
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
                result.statuses.push(FeatureStatus {
                    id,
                    error: None,
                    value: None,
                    candidates: None,
                });
                self.cache.entries.push((chain, result.clone()));
                continue;
            }
            if let Some(e) = binding_error {
                result.statuses.push(FeatureStatus {
                    id,
                    error: Some(e),
                    value: None,
                    candidates: None,
                });
                self.cache.entries.push((chain, result.clone()));
                continue;
            }
            if let FeatureKind::Variable(v) = &self.features[pos].kind {
                let status = match crate::expr::evaluate(&v.expression, &result.variables) {
                    Ok(value)
                        if v.name.chars().all(|c| c.is_alphanumeric() || c == '_')
                            && !v.name.is_empty() =>
                    {
                        result.variables.insert(v.name.clone(), value);
                        FeatureStatus {
                            id,
                            error: None,
                            value: Some(value),
                            candidates: None,
                        }
                    }
                    Ok(_) => FeatureStatus {
                        id,
                        error: Some(format!("'{}' is not a valid variable name", v.name)),
                        value: None,
                        candidates: None,
                    },
                    Err(e) => FeatureStatus {
                        id,
                        error: Some(e),
                        value: None,
                        candidates: None,
                    },
                };
                result.statuses.push(status);
                self.cache.entries.push((chain, result.clone()));
                continue;
            }
            let mut candidates = None;
            let error = match &mut self.features[pos].kind {
                FeatureKind::Sketch(sf) => {
                    let plane = match result.resolve_plane(&sf.plane) {
                        Ok(p) => p,
                        Err(e) => {
                            result.statuses.push(FeatureStatus {
                                id,
                                error: Some(e),
                                value: None,
                                candidates: None,
                            });
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
                    Self::regen_revolve(&mut result, id, &rf, pos, &self.features, &opts)
                }
                FeatureKind::Blend(bf) => {
                    let bf = bf.clone();
                    Self::regen_blend(&mut result, id, &bf, &opts)
                }
                FeatureKind::Mirror(mf) => {
                    let mf = mf.clone();
                    candidates = Some(
                        result
                            .bodies
                            .iter()
                            .map(|b| (b.source, b.name.clone()))
                            .collect(),
                    );
                    match result.resolve_plane(&mf.plane) {
                        Ok(plane) => Self::regen_copies(
                            &mut result,
                            id,
                            &[Transform::mirror(&plane)],
                            mf.op,
                            &mf.features,
                            &mf.bodies,
                        ),
                        Err(e) => Some(e),
                    }
                }
                FeatureKind::Variable(_) => unreachable!("handled above"),
                FeatureKind::Hole(hf) => {
                    let hf = hf.clone();
                    Self::regen_hole(&mut result, id, &hf, pos, &self.features, &opts)
                }
                FeatureKind::Sweep(sw) => {
                    let sw = sw.clone();
                    Self::regen_sweep(&mut result, id, &sw, pos, &self.features, &opts)
                }
                FeatureKind::Loft(lf) => {
                    let lf = lf.clone();
                    Self::regen_loft(&mut result, id, &lf, pos, &self.features)
                }
                FeatureKind::Shell(sf) => {
                    let sf = sf.clone();
                    Self::regen_shell(&mut result, id, &sf)
                }
                FeatureKind::MoveFace(mf) => {
                    let mf = mf.clone();
                    Self::regen_face_edit(&mut result, id, &mf.faces, "move", |solid, faces| {
                        ok_brep::move_faces(solid, faces, mf.distance)
                    })
                }
                FeatureKind::Draft(df) => {
                    let df = df.clone();
                    match result.resolve_plane(&df.neutral) {
                        Ok(neutral) => Self::regen_face_edit(
                            &mut result,
                            id,
                            &df.faces,
                            "draft",
                            |solid, faces| ok_brep::draft_faces(solid, faces, &neutral, df.angle),
                        ),
                        Err(e) => Some(e),
                    }
                }
                FeatureKind::Split(sp) => {
                    let sp = sp.clone();
                    candidates = Some(
                        result
                            .bodies
                            .iter()
                            .map(|b| (b.source, b.name.clone()))
                            .collect(),
                    );
                    match result.resolve_plane(&sp.plane) {
                        Ok(plane) => Self::regen_split(&mut result, id, &plane, &sp.bodies),
                        Err(e) => Some(e),
                    }
                }
                FeatureKind::Mesh(mf) => {
                    let mf = mf.clone();
                    match Self::mesh_solid(&mf, id) {
                        Ok(solid) => Self::apply_tool(&mut result, id, solid, BodyOp::New),
                        Err(e) => Some(e),
                    }
                }
                FeatureKind::Puzzle(pf) => {
                    let pf = pf.clone();
                    Self::regen_puzzle(&mut result, id, &pf, &opts)
                }
                FeatureKind::Boolean(bf) => {
                    let bf = bf.clone();
                    candidates = Some(
                        result
                            .bodies
                            .iter()
                            .map(|b| (b.source, b.name.clone()))
                            .collect(),
                    );
                    Self::regen_boolean(&mut result, id, &bf)
                }
                FeatureKind::Pattern(pf) => {
                    let pf = pf.clone();
                    candidates = Some(
                        result
                            .bodies
                            .iter()
                            .map(|b| (b.source, b.name.clone()))
                            .collect(),
                    );
                    if pf.count < 2 {
                        Some("count must be at least 2".into())
                    } else {
                        let transforms: Vec<Transform> = (1..pf.count)
                            .map(|i| match pf.kind {
                                PatternKind::Linear { axis, spacing } => {
                                    Transform::translation(axis.vector() * (spacing * i as f64))
                                }
                                PatternKind::Circular { axis, angle } => {
                                    let step = angle.to_radians() / pf.count as f64;
                                    Transform::rotation(Vec3::ZERO, axis.vector(), step * i as f64)
                                }
                            })
                            .collect();
                        Self::regen_copies(
                            &mut result,
                            id,
                            &transforms,
                            pf.op,
                            &pf.features,
                            &pf.bodies,
                        )
                    }
                }
            };
            result.statuses.push(FeatureStatus {
                id,
                error,
                value: None,
                candidates,
            });
            self.cache.entries.push((chain, result.clone()));
        }
        self.cache.entries.truncate(self.features.len());
        self.apply_part_names(&mut result);
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
        let mut r = self
            .cache
            .entries
            .get(count - 1)
            .map(|(_, r)| r.clone())
            .unwrap_or(full);
        self.apply_part_names(&mut r);
        r
    }

    /// Gives bodies their user-chosen names (by creating feature).
    fn apply_part_names(&self, result: &mut RegenResult) {
        for b in &mut result.bodies {
            if let Some(n) = self.part_names.get(&b.source) {
                b.name = n.clone();
            }
            b.material = self.part_materials.get(&b.source).cloned();
        }
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
        opts: &ProfileOptions,
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
        let seg = opts.arc_segment_angle;
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

    /// One body per piece, named by column, row and colour, faces of
    /// piece `k` carrying local indices from `k * PIECE_LOCALS`; then the
    /// alignment web (the gaps, `web` high) as one more body.
    fn regen_puzzle(
        result: &mut RegenResult,
        id: FeatureId,
        pf: &crate::PuzzleFeature,
        opts: &ProfileOptions,
    ) -> Option<String> {
        const PIECE_LOCALS: u32 = 1 << 12;
        let plane = match result.resolve_plane(&pf.plane) {
            Ok(p) => p,
            Err(e) => return Some(e),
        };
        if pf.thickness <= 0.0 || !pf.thickness.is_finite() {
            return Some("thickness must be positive".into());
        }
        if pf.web < 0.0 || !pf.web.is_finite() {
            return Some("web height must be zero or positive".into());
        }
        let jig = match ok_sketch::jigsaw::build(&pf.params()) {
            Ok(j) => j,
            Err(e) => return Some(e),
        };
        // A fabrication layout: one colour's pieces on their board, rows
        // spread a bit apart; nothing else regenerates with it.
        let layout: Vec<ok_sketch::jigsaw::Piece> = match pf.show {
            crate::PuzzleLayout::Design => Vec::new(),
            crate::PuzzleLayout::Light => ok_sketch::jigsaw::fabrication(&jig, true, pf.bit),
            crate::PuzzleLayout::Dark => ok_sketch::jigsaw::fabrication(&jig, false, pf.bit),
        };
        let design = pf.show == crate::PuzzleLayout::Design;
        let pieces: &[ok_sketch::jigsaw::Piece] = if design { &jig.pieces } else { &layout };
        let mut bodies: Vec<(String, Solid)> = Vec::new();
        let mut piece_area = 0.0;
        for (k, piece) in pieces.iter().enumerate() {
            let mut sk = ok_sketch::Sketch::new();
            ok_sketch::jigsaw::draw(&piece.outline, &mut sk);
            let mut profiles = sk.profiles(opts);
            if profiles.len() != 1 {
                return Some(format!(
                    "piece {},{} does not close into one region ({} found)",
                    piece.col + 1,
                    piece.row + 1,
                    profiles.len()
                ));
            }
            let profile = profiles.remove(0);
            piece_area += profile.area();
            let mut solid = match ok_brep::extrude(&profile, &plane, 0.0, pf.thickness, id.0) {
                Ok(s) => s,
                Err(e) => return Some(format!("piece {},{}: {e}", piece.col + 1, piece.row + 1)),
            };
            for f in &mut solid.faces {
                f.origin.local += (k as u32 + 1) * PIECE_LOCALS;
            }
            bodies.push((
                format!(
                    "Piece {},{} {}",
                    piece.col + 1,
                    piece.row + 1,
                    if piece.light { "light" } else { "dark" }
                ),
                solid,
            ));
        }
        if design && pf.web > 0.0 && pf.gap > 0.0 {
            match Self::puzzle_web(&jig, piece_area, opts) {
                Ok(profile) => match ok_brep::extrude(&profile, &plane, 0.0, pf.web, id.0) {
                    Ok(mut solid) => {
                        for f in &mut solid.faces {
                            f.origin.local += (jig.pieces.len() as u32 + 1) * PIECE_LOCALS;
                        }
                        bodies.push(("Alignment web".into(), solid));
                    }
                    Err(e) => return Some(format!("alignment web: {e}")),
                },
                Err(e) => return Some(format!("alignment web: {e}")),
            }
        }
        if design && pf.fixture > 0.0 {
            match Self::puzzle_fixture(&jig, &plane, pf.fixture, opts) {
                Ok(mut solid) => {
                    for f in &mut solid.faces {
                        f.origin.local += (jig.pieces.len() as u32 + 2) * PIECE_LOCALS;
                    }
                    bodies.push(("Printing fixture".into(), solid));
                }
                Err(e) => return Some(format!("printing fixture: {e}")),
            }
        }
        for (name, solid) in bodies {
            result.next_part += 1;
            result.bodies.push(Body::new(name, id, solid));
        }
        None
    }

    /// A tray to print: a floor under the board with a wall round it,
    /// and a pocket per piece (the piece grown by a clearance, `depth`
    /// deep) so every piece drops into place for the glue-up. Pieces cut
    /// tight share their pockets row by row.
    fn puzzle_fixture(
        jig: &ok_sketch::jigsaw::Jigsaw,
        plane: &Plane,
        depth: f64,
        opts: &ProfileOptions,
    ) -> Result<Solid, String> {
        const CLEARANCE: f64 = 0.15;
        const WALL: f64 = 6.0;
        const FLOOR: f64 = 2.0;
        let rect = |sk: &mut ok_sketch::Sketch, a: Vec2, b: Vec2| {
            sk.add_line(a, Vec2::new(b.x, a.y));
            sk.add_line(Vec2::new(b.x, a.y), b);
            sk.add_line(b, Vec2::new(a.x, b.y));
            sk.add_line(Vec2::new(a.x, b.y), a);
        };
        let mut sk = ok_sketch::Sketch::new();
        rect(
            &mut sk,
            Vec2::new(-WALL, -WALL),
            Vec2::new(jig.width + WALL, jig.height + WALL),
        );
        let slab = sk.profiles(opts).into_iter().next().ok_or("no tray")?;
        let mut tray =
            ok_brep::extrude(&slab, plane, -FLOOR, depth, 0).map_err(|e| e.to_string())?;
        for piece in &jig.pieces {
            let mut sk = ok_sketch::Sketch::new();
            ok_sketch::jigsaw::draw(
                &ok_sketch::jigsaw::offset(&piece.outline, -CLEARANCE),
                &mut sk,
            );
            let pocket = sk.profiles(opts).into_iter().next().ok_or_else(|| {
                format!("piece {},{} has no pocket", piece.col + 1, piece.row + 1)
            })?;
            let cutter =
                ok_brep::extrude(&pocket, plane, 0.0, depth + 1.0, 0).map_err(|e| e.to_string())?;
            tray = boolean(&tray, &cutter, BoolOp::Difference).map_err(|e| {
                format!("pocket for piece {},{}: {e}", piece.col + 1, piece.row + 1)
            })?;
        }
        Ok(tray)
    }

    /// The region between the pieces: every outline plus the short
    /// segments closing the gaps along the board's edges, from which the
    /// region finder yields the pieces and one lattice.
    fn puzzle_web(
        jig: &ok_sketch::jigsaw::Jigsaw,
        piece_area: f64,
        opts: &ProfileOptions,
    ) -> Result<Profile, String> {
        let mut sk = ok_sketch::Sketch::new();
        for piece in &jig.pieces {
            ok_sketch::jigsaw::draw(&piece.outline, &mut sk);
        }
        let cols = jig.pieces.iter().map(|p| p.col).max().unwrap_or(0) as usize + 1;
        let rows = jig.pieces.iter().map(|p| p.row).max().unwrap_or(0) as usize + 1;
        let piece = |i: usize, j: usize| &jig.pieces[j * cols + i];
        let corner_near = |p: &ok_sketch::jigsaw::Piece, at: Vec2| -> Vec2 {
            p.outline
                .iter()
                .map(|s| s.start())
                .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
                .unwrap_or(at)
        };
        let pitch_x = jig.width / cols as f64;
        let pitch_y = jig.height / rows as f64;
        let mut closings: Vec<(Vec2, Vec2)> = Vec::new();
        for i in 0..cols - 1 {
            let x = (i + 1) as f64 * pitch_x;
            closings.push((
                corner_near(piece(i, 0), Vec2::new(x, 0.0)),
                corner_near(piece(i + 1, 0), Vec2::new(x, 0.0)),
            ));
            closings.push((
                corner_near(piece(i, rows - 1), Vec2::new(x, jig.height)),
                corner_near(piece(i + 1, rows - 1), Vec2::new(x, jig.height)),
            ));
        }
        for j in 0..rows - 1 {
            let y = (j + 1) as f64 * pitch_y;
            closings.push((
                corner_near(piece(0, j), Vec2::new(0.0, y)),
                corner_near(piece(0, j + 1), Vec2::new(0.0, y)),
            ));
            closings.push((
                corner_near(piece(cols - 1, j), Vec2::new(jig.width, y)),
                corner_near(piece(cols - 1, j + 1), Vec2::new(jig.width, y)),
            ));
        }
        for (a, b) in &closings {
            sk.add_line(*a, *b);
        }
        let expected = jig.width * jig.height - piece_area;
        let profiles = sk.profiles(opts);
        profiles
            .into_iter()
            .min_by(|a, b| {
                (a.area() - expected)
                    .abs()
                    .total_cmp(&(b.area() - expected).abs())
            })
            .filter(|p| (p.area() - expected).abs() < 0.05 * expected.max(1.0))
            .ok_or_else(|| "the gaps do not form one region".to_string())
    }

    fn regen_hole(
        result: &mut RegenResult,
        id: FeatureId,
        hf: &crate::HoleFeature,
        pos: usize,
        features: &[crate::Feature],
        opts: &ProfileOptions,
    ) -> Option<String> {
        // The sketch must exist and precede this feature (regions are not needed).
        let sketch_pos = match features.iter().position(|f| f.id == hf.sketch) {
            None => return Some(format!("sketch {:?} no longer exists", hf.sketch)),
            Some(sp) if sp >= pos => return Some("hole must come after its sketch".into()),
            Some(sp) if features[sp].suppressed => {
                return Some(format!("sketch '{}' is suppressed", features[sp].name))
            }
            Some(sp) => sp,
        };
        let Some(sr) = result.sketches.get(&hf.sketch) else {
            return Some("sketch did not regenerate".into());
        };
        let FeatureKind::Sketch(sf) = &features[sketch_pos].kind else {
            unreachable!()
        };
        // Standalone points: not referenced by any curve.
        let referenced: std::collections::HashSet<ok_sketch::EntityId> = sf
            .sketch
            .entities()
            .flat_map(|(_, e)| e.references())
            .collect();
        let centers: Vec<Vec2> = sf
            .sketch
            .entities()
            .filter_map(|(eid, e)| match e {
                ok_sketch::Entity::Point { pos } if !referenced.contains(&eid) => Some(*pos),
                _ => None,
            })
            .collect();
        if centers.is_empty() {
            return Some("the sketch has no standalone points to drill at".into());
        }
        if !(hf.diameter.is_finite() && hf.diameter > ok_math::tol::LINEAR) {
            return Some("diameter must be positive".into());
        }
        let plane = sr.plane;
        let n = plane.normal;
        let fwd = result.extent_along(plane.origin, n);
        let back = result.extent_along(plane.origin, -n);
        let range = |depth: f64, through: bool| -> Result<(f64, f64), String> {
            if through {
                let (Some(fwd), Some(back)) = (fwd, back) else {
                    return Err("through all needs an existing body".into());
                };
                Ok(match hf.direction {
                    ExtrudeDirection::Normal => (0.0, fwd),
                    ExtrudeDirection::Reverse => (0.0, -back),
                    ExtrudeDirection::Symmetric => (-back, fwd),
                })
            } else {
                if !(depth.is_finite() && depth > ok_math::tol::LINEAR) {
                    return Err("depth must be positive".into());
                }
                Ok(match hf.direction {
                    ExtrudeDirection::Normal => (0.0, depth),
                    ExtrudeDirection::Reverse => (0.0, -depth),
                    ExtrudeDirection::Symmetric => (-depth / 2.0, depth / 2.0),
                })
            }
        };
        let cylinder =
            |c: Vec2, diameter: f64, start: f64, end: f64| -> Result<Solid, ok_brep::BrepError> {
                let mut sk = ok_sketch::Sketch::new();
                sk.add_circle(c, diameter / 2.0);
                let profile = sk.profiles(opts).remove(0);
                ok_brep::extrude(&profile, &plane, start, end, id.0)
            };
        let (start, end) = match range(hf.depth, hf.through_all) {
            Ok(r) => r,
            Err(e) => return Some(e),
        };
        // A countersink: a cone from its diameter at the plane narrowing
        // to the hole at its included angle, revolved about the hole's
        // axis, one each way for a symmetric hole.
        let countersink = |c: Vec2, cs: crate::Countersink, sign: f64| -> Result<Solid, String> {
            if cs.diameter <= hf.diameter {
                return Err("countersink diameter must exceed the hole diameter".into());
            }
            if !(cs.angle > 1.0 && cs.angle < 179.0) {
                return Err("countersink angle must be between 1 and 179 degrees".into());
            }
            let half = (cs.angle / 2.0).to_radians();
            let depth = (cs.diameter - hf.diameter) / 2.0 / half.tan();
            let centre = plane.to_world(c);
            let radial = ok_brep::exact::perpendicular(n);
            let along = n * sign;
            let section = Plane {
                origin: centre,
                x_axis: radial,
                y_axis: along,
                normal: radial.cross(along),
            };
            let mut sk = ok_sketch::Sketch::new();
            let pts = [
                Vec2::new(0.0, 0.0),
                Vec2::new(cs.diameter / 2.0, 0.0),
                Vec2::new(hf.diameter / 2.0, depth),
                Vec2::new(0.0, depth),
            ];
            let mut ids = Vec::new();
            for i in 0..pts.len() {
                let (_, a, b) = sk.add_line(pts[i], pts[(i + 1) % pts.len()]);
                ids.push((a, b));
            }
            for i in 0..ids.len() {
                let next = ids[(i + 1) % ids.len()].0;
                sk.add_constraint(ok_sketch::Constraint::Coincident {
                    a: ids[i].1,
                    b: next,
                });
            }
            let profile = sk.profiles(opts).remove(0);
            ok_brep::revolve(
                &profile,
                &section,
                Vec2::ZERO,
                Vec2::Y,
                std::f64::consts::TAU,
                opts.arc_segment_angle,
                id.0,
            )
            .map_err(|e| e.to_string())
        };
        let mut parts: Vec<Result<Solid, ok_brep::BrepError>> = Vec::new();
        for &c in &centers {
            parts.push(cylinder(c, hf.diameter, start, end));
            if let Some(cb) = hf.counterbore {
                if cb.diameter <= hf.diameter {
                    return Some("counterbore diameter must exceed the hole diameter".into());
                }
                match range(cb.depth, false) {
                    Ok((s, e)) => parts.push(cylinder(c, cb.diameter, s, e)),
                    Err(e) => return Some(format!("counterbore: {e}")),
                }
            }
            if let Some(cs) = hf.countersink {
                let signs: &[f64] = match hf.direction {
                    ExtrudeDirection::Normal => &[1.0],
                    ExtrudeDirection::Reverse => &[-1.0],
                    ExtrudeDirection::Symmetric => &[1.0, -1.0],
                };
                for &sign in signs {
                    match countersink(c, cs, sign) {
                        Ok(cone) => parts.push(Ok(cone)),
                        Err(e) => return Some(format!("countersink: {e}")),
                    }
                }
            }
        }
        let tool = match Self::build_tool(parts.into_iter()) {
            Ok(t) => t,
            Err(e) => return Some(e),
        };
        Self::apply_tool(result, id, tool, BodyOp::Remove)
    }

    /// The open chain of non-construction lines and arcs of a sketch as a
    /// 3D polyline, sampled at the facet angle.
    fn sketch_path(
        sketch: &ok_sketch::Sketch,
        plane: &Plane,
        opts: &ProfileOptions,
    ) -> Result<Vec<Vec3>, String> {
        use ok_sketch::Entity;
        // Segments as sampled 2D polylines.
        let mut segs: Vec<Vec<Vec2>> = Vec::new();
        for (id, e) in sketch.entities() {
            if sketch.is_construction(id) {
                continue;
            }
            match e {
                Entity::Line { start, end } => {
                    let (a, b) = (
                        sketch.point(*start).map_err(|e| e.to_string())?,
                        sketch.point(*end).map_err(|e| e.to_string())?,
                    );
                    if a.distance(b) > 1e-9 {
                        segs.push(vec![a, b]);
                    }
                }
                Entity::Arc { center, start, end } => {
                    let c = sketch.point(*center).map_err(|e| e.to_string())?;
                    let a = sketch.point(*start).map_err(|e| e.to_string())?;
                    let b = sketch.point(*end).map_err(|e| e.to_string())?;
                    let r = 0.5 * (a.distance(c) + b.distance(c));
                    let a0 = (a - c).angle();
                    let mut sweep = ((b - c).angle() - a0).rem_euclid(std::f64::consts::TAU);
                    if sweep < 1e-12 {
                        sweep = std::f64::consts::TAU;
                    }
                    let n = ((sweep / opts.arc_segment_angle).ceil() as usize).max(2);
                    segs.push(
                        (0..=n)
                            .map(|i| c + Vec2::from_angle(a0 + sweep * i as f64 / n as f64) * r)
                            .collect(),
                    );
                }
                Entity::Spline { .. } => {
                    let pts = sketch.spline_points(id).map_err(|e| e.to_string())?;
                    segs.push(ok_sketch::spline_polyline(
                        &pts,
                        ok_sketch::spline_pieces(opts),
                    ));
                }
                _ => {}
            }
        }
        if segs.is_empty() {
            return Err("path sketch has no lines or arcs".into());
        }
        // Chain segments end to end (either direction), starting from a free end.
        let tol = 1e-6;
        let ends = |s: &Vec<Vec2>| (s[0], *s.last().unwrap());
        let mut degree: Vec<usize> = vec![0; segs.len()];
        for i in 0..segs.len() {
            for j in 0..segs.len() {
                if i == j {
                    continue;
                }
                let (a0, a1) = ends(&segs[i]);
                let (b0, b1) = ends(&segs[j]);
                if a0.distance(b0) <= tol || a0.distance(b1) <= tol {
                    degree[i] += 1;
                }
                if a1.distance(b0) <= tol || a1.distance(b1) <= tol {
                    degree[i] += 1;
                }
            }
        }
        let start = (0..segs.len())
            .find(|&i| degree[i] < 2)
            .ok_or("path must be an open chain (no loops)")?;
        let mut used = vec![false; segs.len()];
        let mut path: Vec<Vec2> = Vec::new();
        let mut cur = start;
        // Orient the first segment so its free end comes first.
        let mut seg = segs[start].clone();
        {
            let (a0, _) = ends(&seg);
            let free_first = !(0..segs.len()).filter(|&j| j != start).any(|j| {
                let (b0, b1) = ends(&segs[j]);
                a0.distance(b0) <= tol || a0.distance(b1) <= tol
            });
            if !free_first {
                seg.reverse();
            }
        }
        loop {
            used[cur] = true;
            path.extend(seg.iter().copied());
            let tail = *path.last().unwrap();
            let next = (0..segs.len()).find(|&j| {
                let (b0, b1) = ends(&segs[j]);
                !used[j] && (b0.distance(tail) <= tol || b1.distance(tail) <= tol)
            });
            match next {
                Some(j) => {
                    seg = segs[j].clone();
                    if seg[0].distance(tail) > tol {
                        seg.reverse();
                    }
                    seg.remove(0);
                    cur = j;
                }
                None => break,
            }
        }
        if used.iter().any(|u| !u) {
            return Err("path sketch has disconnected pieces".into());
        }
        Ok(path.into_iter().map(|p| plane.to_world(p)).collect())
    }

    fn regen_sweep(
        result: &mut RegenResult,
        id: FeatureId,
        sw: &crate::SweepFeature,
        pos: usize,
        features: &[crate::Feature],
        opts: &ProfileOptions,
    ) -> Option<String> {
        let sr = match Self::source_sketch(result, sw.sketch, pos, features, "sweep") {
            Ok(sr) => sr,
            Err(e) => return Some(e),
        };
        let selected = match Self::select_profiles(sr, &sw.profiles) {
            Ok(v) => v,
            Err(e) => return Some(e),
        };
        let path_pos = match features.iter().position(|f| f.id == sw.path) {
            None => return Some("path sketch no longer exists".into()),
            Some(pp) if pp >= pos => return Some("sweep must come after its path sketch".into()),
            Some(pp) => pp,
        };
        let Some(path_result) = result.sketches.get(&sw.path) else {
            return Some("path sketch did not regenerate".into());
        };
        let FeatureKind::Sketch(path_sketch) = &features[path_pos].kind else {
            return Some("path must be a sketch".into());
        };
        let path = match Self::sketch_path(&path_sketch.sketch, &path_result.plane, opts) {
            Ok(p) => p,
            Err(e) => return Some(e),
        };
        let plane = sr.plane;
        let tool = match Self::build_tool(
            selected
                .into_iter()
                .map(|p| ok_brep::sweep(p, &plane, &path, id.0)),
        ) {
            Ok(t) => t,
            Err(e) => return Some(e),
        };
        Self::apply_tool(result, id, tool, sw.op)
    }

    fn regen_loft(
        result: &mut RegenResult,
        id: FeatureId,
        lf: &crate::LoftFeature,
        pos: usize,
        features: &[crate::Feature],
    ) -> Option<String> {
        let sa = match Self::source_sketch(result, lf.sketch, pos, features, "loft") {
            Ok(sr) => sr,
            Err(e) => return Some(e),
        };
        let sb = match Self::source_sketch(result, lf.sketch_b, pos, features, "loft") {
            Ok(sr) => sr,
            Err(e) => return Some(e),
        };
        let (pa, pb) = (&sa.profiles[0], &sb.profiles[0]);
        let tool = match ok_brep::loft(pa, &sa.plane, pb, &sb.plane, id.0) {
            Ok(t) => t,
            Err(e) => return Some(e.to_string()),
        };
        Self::apply_tool(result, id, tool, lf.op)
    }

    fn regen_blend(
        result: &mut RegenResult,
        id: FeatureId,
        bf: &crate::BlendFeature,
        opts: &ProfileOptions,
    ) -> Option<String> {
        if bf.edges.is_empty() {
            return None; // nothing selected yet: a no-op rather than an error
        }
        let kind = match bf.kind {
            BlendKind::Fillet => ok_brep::BlendKind::Fillet,
            BlendKind::Chamfer => ok_brep::BlendKind::Chamfer,
        };
        let seg = opts.arc_segment_angle;
        let mut matched = 0usize;
        for i in 0..result.bodies.len() {
            let pairs: Vec<(usize, usize)> = bf
                .edges
                .iter()
                .flat_map(|e| result.bodies[i].find_edge_on_surfaces(e))
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

    /// Applies each transform to every existing body, merging or adding copies.
    /// Applies a face edit (move, draft) to every body holding one of the
    /// referenced faces; a reference on a curved surface names all its facets.
    fn regen_face_edit(
        result: &mut RegenResult,
        _id: FeatureId,
        refs: &[FaceRef],
        what: &str,
        edit: impl Fn(&Solid, &[usize]) -> Result<Solid, ok_brep::BrepError>,
    ) -> Option<String> {
        if refs.is_empty() {
            return None; // nothing picked yet: a no-op rather than an error
        }
        let mut matched = 0usize;
        for i in 0..result.bodies.len() {
            let body = &result.bodies[i];
            let faces = body.faces_on_surfaces_of(refs);
            if faces.is_empty() {
                continue;
            }
            matched += faces.len();
            let edited = match edit(&body.solid, &faces) {
                Ok(s) => s,
                Err(e) => return Some(format!("{what} failed: {e}")),
            };
            result.bodies[i] = Body::new(body.name.clone(), body.source, edited);
        }
        if matched == 0 {
            return Some("none of the referenced faces exist any more".into());
        }
        None
    }

    /// Hollows bodies (see `ShellFeature`).
    fn regen_shell(result: &mut RegenResult, id: FeatureId, sf: &ShellFeature) -> Option<String> {
        if result.bodies.is_empty() {
            return Some("there are no bodies to shell".into());
        }
        let mut matched = 0usize;
        for i in 0..result.bodies.len() {
            let body = &result.bodies[i];
            // Open every face on the surfaces the referenced faces lie on.
            let surfaces: Vec<usize> = sf
                .faces
                .iter()
                .flat_map(|r| faces_of_ref(&body.solid, r))
                .map(|i| body.solid.faces[i].surface)
                .collect();
            if !sf.faces.is_empty() && surfaces.is_empty() {
                continue;
            }
            let open: Vec<usize> = body
                .solid
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| surfaces.contains(&f.surface))
                .map(|(i, _)| i)
                .collect();
            matched += 1;
            let shelled = match ok_brep::shell(&body.solid, sf.thickness, &open, id.0) {
                Ok(s) => s,
                Err(e) => return Some(e.to_string()),
            };
            result.bodies[i] = Body::new(body.name.clone(), body.source, shelled);
        }
        if matched == 0 {
            return Some("none of the referenced faces exist any more".into());
        }
        None
    }

    /// Combines target bodies with tool bodies (see `BooleanFeature`).
    fn regen_boolean(
        result: &mut RegenResult,
        id: FeatureId,
        bf: &BooleanFeature,
    ) -> Option<String> {
        let targets: Vec<usize> = (0..result.bodies.len())
            .filter(|&i| bf.targets.contains(&result.bodies[i].source))
            .collect();
        let tools: Vec<usize> = (0..result.bodies.len())
            .filter(|&i| bf.tools.contains(&result.bodies[i].source) && !targets.contains(&i))
            .collect();
        if targets.is_empty() {
            return Some("pick at least one target body".into());
        }
        if tools.is_empty() {
            return Some("pick at least one tool body (other than the targets)".into());
        }
        let op_name = match bf.op {
            BooleanOp::Union => "union",
            BooleanOp::Subtract => "subtract",
            BooleanOp::Intersect => "intersect",
        };
        let mut new_bodies: Vec<Body> = Vec::new();
        match bf.op {
            BooleanOp::Union => {
                let mut merged = result.bodies[targets[0]].solid.clone();
                for &i in targets.iter().skip(1).chain(tools.iter()) {
                    merged = match boolean(&merged, &result.bodies[i].solid, BoolOp::Union) {
                        Ok(s) => s,
                        Err(e) => return Some(format!("{op_name} failed: {e}")),
                    };
                }
                let first = &result.bodies[targets[0]];
                new_bodies.push(Body::new(first.name.clone(), first.source, merged));
            }
            BooleanOp::Subtract | BooleanOp::Intersect => {
                let op = if bf.op == BooleanOp::Subtract {
                    BoolOp::Difference
                } else {
                    BoolOp::Intersection
                };
                let mut any = false;
                for &t in &targets {
                    let mut solid = result.bodies[t].solid.clone();
                    for &i in &tools {
                        solid = match boolean(&solid, &result.bodies[i].solid, op) {
                            Ok(s) => s,
                            Err(e) => return Some(format!("{op_name} failed: {e}")),
                        };
                    }
                    let body = &result.bodies[t];
                    let mut shells = solid.shells().into_iter();
                    if let Some(first) = shells.next() {
                        any = true;
                        new_bodies.push(Body::new(body.name.clone(), body.source, first));
                        for extra in shells {
                            result.next_part += 1;
                            new_bodies.push(Body::new(
                                format!("Part {}", result.next_part),
                                id,
                                extra,
                            ));
                        }
                    }
                }
                if !any {
                    return Some(format!("the {op_name} left nothing of the targets"));
                }
            }
        }
        let mut remove: Vec<usize> = targets.clone();
        if !bf.keep_tools {
            remove.extend(tools.iter().copied());
        }
        result.remove_bodies(&remove);
        result.bodies.extend(new_bodies);
        None
    }

    fn regen_copies(
        result: &mut RegenResult,
        id: FeatureId,
        transforms: &[Transform],
        op: CopyOp,
        features: &[FeatureId],
        bodies: &[FeatureId],
    ) -> Option<String> {
        if !features.is_empty() {
            return Self::regen_feature_copies(result, id, transforms, features);
        }
        if result.bodies.is_empty() {
            return Some("there are no bodies to copy".into());
        }
        let count = result.bodies.len();
        let chosen: Vec<usize> = (0..count)
            .filter(|&i| bodies.is_empty() || bodies.contains(&result.bodies[i].source))
            .collect();
        if chosen.is_empty() {
            return Some("none of the chosen bodies exist any more".into());
        }
        for i in chosen {
            let original = result.bodies[i].solid.clone();
            for xf in transforms {
                let copy = original.transformed(xf);
                match op {
                    CopyOp::New => result.push_body(id, copy),
                    CopyOp::Add => {
                        let merged = match boolean(&result.bodies[i].solid, &copy, BoolOp::Union) {
                            Ok(s) => s,
                            Err(e) => return Some(format!("could not merge copy: {e}")),
                        };
                        let body = &result.bodies[i];
                        result.bodies[i] = Body::new(body.name.clone(), body.source, merged);
                    }
                }
            }
        }
        None
    }

    /// Combines a finished tool volume with the existing bodies.
    /// Replays the tool volumes of `features` under each transform, with
    /// each feature's own body operation (a feature pattern or mirror).
    fn regen_feature_copies(
        result: &mut RegenResult,
        id: FeatureId,
        transforms: &[Transform],
        features: &[FeatureId],
    ) -> Option<String> {
        let mut tools = Vec::with_capacity(features.len());
        for fid in features {
            match result.tools.get(fid) {
                Some((tool, op)) => tools.push((tool.clone(), *op)),
                None => {
                    return Some(format!(
                        "feature {} has no tool volume to copy (it must be an extrude, revolve, hole, sweep or loft that comes earlier)",
                        fid.0
                    ))
                }
            }
        }
        for xf in transforms {
            for (tool, op) in &tools {
                if let Some(e) = Self::apply_tool(result, id, tool.transformed(xf), *op) {
                    return Some(format!("copy failed: {e}"));
                }
            }
        }
        // The copies are not one tool volume, so a later pattern cannot
        // replay this feature; it names the original features instead.
        result.tools.remove(&id);
        None
    }

    /// Splits every chosen body (all when `bodies` is empty) that the plane
    /// crosses: the part against the normal keeps the body's slot and name,
    /// the part on the normal side is appended as a new part.
    fn regen_split(
        result: &mut RegenResult,
        id: FeatureId,
        plane: &Plane,
        bodies: &[FeatureId],
    ) -> Option<String> {
        let chosen: Vec<usize> = (0..result.bodies.len())
            .filter(|&i| bodies.is_empty() || bodies.contains(&result.bodies[i].source))
            .collect();
        if chosen.is_empty() {
            return Some("no bodies to split".into());
        }
        let mut split_any = false;
        let mut new_parts = Vec::new();
        for i in chosen {
            match ok_brep::split_tagged(&result.bodies[i].solid, plane, id.0) {
                Ok(Some((below, above))) => {
                    let name = result.bodies[i].name.clone();
                    let source = result.bodies[i].source;
                    result.bodies[i] = Body::new(name, source, below);
                    new_parts.push(above);
                    split_any = true;
                }
                Ok(None) => {}
                Err(e) => return Some(format!("split failed: {e}")),
            }
        }
        for solid in new_parts {
            result.push_body(id, solid);
        }
        if !split_any {
            return Some("the plane misses every chosen body".into());
        }
        None
    }

    /// A solid from an imported triangle mesh: every triangle becomes a
    /// planar polygon and the closure check of `from_polygons` decides
    /// whether the mesh bounds a volume. Degenerate triangles are skipped.
    fn mesh_solid(mf: &MeshFeature, id: FeatureId) -> Result<Solid, String> {
        let mut polys = Vec::with_capacity(mf.triangles.len());
        let mut surfaces = Vec::with_capacity(mf.triangles.len());
        for (i, t) in mf.triangles.iter().enumerate() {
            let pts: Vec<Vec3> = t
                .iter()
                .map(|&k| mf.vertices.get(k as usize).copied())
                .collect::<Option<_>>()
                .ok_or("mesh triangle refers to a missing vertex")?;
            let Some(normal) = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized() else {
                continue;
            };
            let Some(x_axis) = (pts[1] - pts[0]).normalized() else {
                continue;
            };
            let plane = Plane {
                origin: pts[0],
                x_axis,
                y_axis: normal.cross(x_axis),
                normal,
            };
            surfaces.push(ok_brep::Surface::Plane {
                normal,
                offset: normal.dot(pts[0]),
            });
            polys.push(ok_brep::Polygon {
                plane,
                loops: vec![pts],
                surface: surfaces.len() - 1,
                origin: ok_brep::FaceOrigin {
                    feature: id.0,
                    local: i as u32,
                },
            });
        }
        if polys.len() < 4 {
            return Err("a mesh body needs at least four triangles".into());
        }
        let mut solid = Solid::from_polygons(polys, surfaces)
            .map_err(|e| format!("mesh does not close a volume: {e}"))?;
        // Triangulated flat regions become one face each.
        solid.merge_coplanar_faces();
        if solid.volume() <= 0.0 {
            return Err("mesh is inside out (triangles wind clockwise seen from outside)".into());
        }
        Ok(solid)
    }

    fn apply_tool(
        result: &mut RegenResult,
        id: FeatureId,
        tool: Solid,
        op: BodyOp,
    ) -> Option<String> {
        let ef_op = op;
        let tool_bounds = tool.bounds()?;
        // Remember the tool so feature patterns and mirrors can replay it.
        result.tools.insert(id, (tool.clone(), op));

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
            PlaneRef::Rotated {
                base,
                axis,
                angle,
                offset,
            } => Ok(crate::rotated_plane(*base, *axis, *angle, *offset)),
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
            part: None,
            near: Default::default(),
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
            part: None,
            near: Default::default(),
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
            part: None,
            near: Default::default(),
        };
        let front = crate::FaceRef {
            feature: e,
            local: 2,
            part: None,
            near: Default::default(),
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
    fn mirror_and_patterns() {
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
                a: Vec2::new(1.0, 0.0),
                b: Vec2::new(3.0, 2.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s,
            depth: 1.0,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        // Mirror across the Right plane (x = 0): a separate copy.
        let m = ps
            .apply(Op::AddMirror {
                plane: PlaneRef::standard(StandardPlane::Right),
                op: crate::CopyOp::New,
                features: vec![],
                bodies: vec![],
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
        assert_eq!(r.bodies.len(), 2);
        assert!((r.bodies[1].solid.volume() - 4.0).abs() < 1e-9);
        // Mirror across a touching plane with Add merges into one body.
        ps.apply(Op::SetMirror {
            id: m,
            plane: Some(PlaneRef::Standard {
                base: StandardPlane::Right,
                offset: 1.0,
            }),
            op: Some(crate::CopyOp::Add),
            features: None,
            bodies: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert_eq!(r.bodies.len(), 1);
        assert!((r.bodies[0].solid.volume() - 8.0).abs() < 1e-9);
        // Linear pattern of 3 along X with gaps: three lumps, one body.
        ps.apply(Op::AddPattern {
            kind: PatternKind::Linear {
                axis: crate::Axis::X,
                spacing: 10.0,
            },
            count: 3,
            op: crate::CopyOp::Add,
            features: vec![],
            bodies: vec![],
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 1);
        assert!((r.bodies[0].solid.volume() - 24.0).abs() < 1e-9);
        assert_eq!(r.bodies[0].solid.shells().len(), 3);
        // Circular pattern as new bodies: 4 around Z.
        ps.apply(Op::AddPattern {
            kind: PatternKind::Circular {
                axis: crate::Axis::Z,
                angle: 360.0,
            },
            count: 4,
            op: crate::CopyOp::New,
            features: vec![],
            bodies: vec![],
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 4);
        let total: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        assert!((total - 96.0).abs() < 1e-6, "total {total}");
    }

    #[test]
    fn variables_drive_dimensions_through_bindings() {
        let mut ps = PartStudio::new("t");
        ps.apply(Op::AddVariable {
            name: "width".into(),
            expression: "40".into(),
        })
        .unwrap();
        let h = ps
            .apply(Op::AddVariable {
                name: "height".into(),
                expression: "#width / 4".into(),
            })
            .unwrap()
            .feature
            .unwrap();
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r = ps
            .apply(Op::Sketch {
                id: s,
                op: SketchOp::AddRectangle {
                    a: Vec2::ZERO,
                    b: Vec2::new(10.0, 10.0),
                },
            })
            .unwrap();
        let bottom = r.entities[0];
        let (bl, _) = match &ps.feature(s).unwrap().kind {
            FeatureKind::Sketch(sf) => sf.sketch.line(bottom).unwrap(),
            _ => unreachable!(),
        };
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::AddConstraint {
                constraint: ok_sketch::Constraint::Fixed { point: bl },
            },
        })
        .unwrap();
        let len = ps
            .apply(Op::Sketch {
                id: s,
                op: SketchOp::AddConstraint {
                    constraint: ok_sketch::Constraint::Length {
                        line: bottom,
                        value: 10.0,
                    },
                },
            })
            .unwrap()
            .constraint
            .unwrap();
        ps.apply(Op::SetBinding {
            id: s,
            field: format!("constraint.{}", len.0),
            expression: Some("#width".into()),
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
        ps.apply(Op::SetBinding {
            id: e,
            field: "depth".into(),
            expression: Some("#height".into()),
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.statuses[1].value, Some(10.0));
        assert!(
            (r.bodies[0].solid.volume() - 40.0 * 10.0 * 10.0).abs() < 1e-6,
            "vol {}",
            r.bodies[0].solid.volume()
        );
        // Change the variable: everything follows.
        ps.apply(Op::SetVariable {
            id: h,
            name: None,
            expression: Some("#width / 2".into()),
        })
        .unwrap();
        let r = ps.regenerate();
        assert!((r.bodies[0].solid.volume() - 40.0 * 10.0 * 20.0).abs() < 1e-6);
        // A bad expression is a feature error, not a crash.
        ps.apply(Op::SetBinding {
            id: e,
            field: "depth".into(),
            expression: Some("#nope * 2".into()),
        })
        .unwrap();
        let r = ps.regenerate();
        assert_eq!(r.errors().count(), 1);
        assert!(r.errors().next().unwrap().1.contains("nope"));
        // Binding an unknown field is rejected up front.
        assert!(ps
            .apply(Op::SetBinding {
                id: e,
                field: "radius".into(),
                expression: Some("1".into())
            })
            .is_err());
    }

    #[test]
    fn hole_feature_drills_points_with_counterbore() {
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
                b: Vec2::new(40.0, 20.0),
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
        // Points sketched on the top face; the hole drills into the part.
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Face {
                    face: crate::FaceRef {
                        feature: e,
                        local: 1,
                        part: None,
                        near: Default::default(),
                    },
                    offset: 0.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::AddPoint {
                pos: Vec2::new(10.0, 10.0),
            },
        })
        .unwrap();
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::AddPoint {
                pos: Vec2::new(30.0, 10.0),
            },
        })
        .unwrap();
        let h = ps
            .apply(Op::AddHole {
                sketch: s2,
                diameter: 4.0,
                depth: 0.0,
                through_all: true,
                direction: ExtrudeDirection::Reverse,
                counterbore: None,
                countersink: None,
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
        let pi = std::f64::consts::PI;
        let expected = 8000.0 - 2.0 * pi * 4.0 * 10.0;
        let vol = r.bodies[0].solid.volume();
        assert!(
            ((vol - expected) / expected).abs() < 3e-3,
            "vol {vol} expected {expected}"
        );
        // Blind 3 mm with a counterbore 8 mm wide, 2 mm deep.
        ps.apply(Op::SetHole {
            id: h,
            diameter: None,
            depth: Some(3.0),
            through_all: Some(false),
            direction: None,
            counterbore: Some(Some(crate::Counterbore {
                diameter: 8.0,
                depth: 2.0,
            })),
            countersink: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let expected = 8000.0 - 2.0 * (pi * 4.0 * 3.0 + pi * (16.0 - 4.0) * 2.0);
        let vol = r.bodies[0].solid.volume();
        assert!(
            ((vol - expected) / expected).abs() < 3e-3,
            "vol {vol} expected {expected}"
        );
        assert_eq!(ps.feature(h).unwrap().name, "Hole 1");
        // Through again, the counterbore swapped for a 90° countersink 8 mm
        // across: a 2 mm deep frustum from radius 4 to radius 2 at the top
        // of each hole, minus the hole already through it.
        ps.apply(Op::SetHole {
            id: h,
            diameter: None,
            depth: None,
            through_all: Some(true),
            direction: None,
            counterbore: Some(None),
            countersink: Some(Some(crate::Countersink {
                diameter: 8.0,
                angle: 90.0,
            })),
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let frustum = pi * 2.0 / 3.0 * (16.0 + 8.0 + 4.0) - pi * 4.0 * 2.0;
        let expected = 8000.0 - 2.0 * (pi * 4.0 * 10.0 + frustum);
        let vol = r.bodies[0].solid.volume();
        assert!(
            ((vol - expected) / expected).abs() < 3e-3,
            "vol {vol} expected {expected}"
        );
        let solid = &r.bodies[0].solid;
        let cones = solid
            .surfaces
            .iter()
            .filter(|s| matches!(s, ok_brep::Surface::Cone { .. }))
            .count();
        assert_eq!(cones, 2, "one cone per countersink");
        // A countersink no wider than the hole is an error, not a silent no-op.
        ps.apply(Op::SetHole {
            id: h,
            diameter: None,
            depth: None,
            through_all: None,
            direction: None,
            counterbore: None,
            countersink: Some(Some(crate::Countersink {
                diameter: 4.0,
                angle: 90.0,
            })),
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.errors().next().is_some());
    }

    #[test]
    fn puzzle_makes_a_body_per_piece_and_an_alignment_web() {
        let mut ps = PartStudio::new("t");
        let puzzle = |gap: f64, web: f64| Op::AddPuzzle {
            plane: PlaneRef::standard(StandardPlane::Top),
            cols: 3,
            rows: 2,
            pitch: 40.0,
            thickness: 10.0,
            gap,
            bit: 6.35,
            lock: 20.0,
            grain: crate::Grain::X,
            web,
            seed: 3,
            jitter: 2.0,
            fixture: 0.0,
            show: crate::PuzzleLayout::Design,
            name: None,
        };
        let id = ps.apply(puzzle(0.0, 0.0)).unwrap().feature.unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 6);
        assert_eq!(r.bodies[0].name, "Piece 1,1 light");
        assert_eq!(r.bodies[1].name, "Piece 2,1 dark");
        assert_eq!(r.bodies[3].name, "Piece 1,2 dark");
        let total: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        assert!(
            (total - 6.0 * 1600.0 * 10.0).abs() < 6.0 * 1600.0 * 10.0 * 2e-3,
            "{total}"
        );
        for b in &r.bodies {
            b.solid.validate().unwrap();
            // Every piece's faces carry their own local range.
            let k = r.bodies.iter().position(|x| std::ptr::eq(x, b)).unwrap() as u32 + 1;
            assert!(b
                .solid
                .faces
                .iter()
                .all(|f| f.origin.local / (1 << 12) == k));
        }
        // A gap and a web: the pieces shrink, the web fills the gaps.
        ps.apply(Op::SetPuzzle {
            id,
            cols: None,
            rows: None,
            pitch: None,
            thickness: None,
            gap: Some(1.0),
            bit: None,
            lock: None,
            grain: None,
            web: Some(4.0),
            seed: None,
            jitter: None,
            fixture: None,
            show: None,
            tabs: None,
            corners: None,
        })
        .unwrap();
        let start = std::time::Instant::now();
        let r = ps.regenerate();
        eprintln!("3x2 with web: {:?}", start.elapsed());
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 7);
        assert_eq!(r.bodies[6].name, "Alignment web");
        let pieces: f64 = r.bodies[..6].iter().map(|b| b.solid.volume()).sum();
        let web = r.bodies[6].solid.volume();
        assert!(pieces < 6.0 * 16000.0 - 1000.0, "{pieces}");
        assert!(
            (pieces / 10.0 + web / 4.0 - 6.0 * 1600.0).abs() < 6.0 * 1600.0 * 2e-3,
            "{pieces} {web}"
        );
        r.bodies[6].solid.validate().unwrap();
        // The pin router rules come back as the feature's error.
        ps.apply(Op::SetPuzzle {
            id,
            cols: None,
            rows: None,
            pitch: None,
            thickness: None,
            gap: None,
            bit: Some(14.0),
            lock: None,
            grain: None,
            web: None,
            seed: None,
            jitter: None,
            fixture: None,
            show: None,
            tabs: None,
            corners: None,
        })
        .unwrap();
        let r = ps.regenerate();
        let err = r
            .errors()
            .next()
            .map(|(_, e)| e.to_string())
            .unwrap_or_default();
        assert!(err.contains("socket opening"), "{err}");
        // Toggling one tab flips it, and its inverse puts it back.
        let before = ps.clone();
        let op = Op::SetPuzzleTab {
            id,
            edge: 2,
            out: Some(false),
            size: Some(1.3),
            width: None,
            shift: None,
        };
        let inverse = ps.apply_with_inverse(op, None).unwrap().inverse;
        let (t0, t1) = match (
            &before.feature(id).unwrap().kind,
            &ps.feature(id).unwrap().kind,
        ) {
            (FeatureKind::Puzzle(a), FeatureKind::Puzzle(b)) => (a.tabs[2], b.tabs[2]),
            _ => unreachable!(),
        };
        assert_ne!(t0, t1);
        assert!(!t1.out && (t1.size - 1.3).abs() < 1e-12);
        for op in inverse {
            ps.apply(op).unwrap();
        }
        assert_eq!(
            ps.feature(id).unwrap().kind,
            before.feature(id).unwrap().kind
        );
        // A tight fit with a tray to print, then each colour's
        // fabrication layout: its pieces alone, rows a bit apart.
        ps.apply(Op::SetPuzzle {
            id,
            cols: None,
            rows: None,
            pitch: None,
            thickness: None,
            gap: Some(0.0),
            bit: Some(6.35),
            lock: None,
            grain: None,
            web: Some(0.0),
            seed: None,
            jitter: None,
            fixture: Some(5.0),
            show: None,
            tabs: None,
            corners: None,
        })
        .unwrap();
        let start = std::time::Instant::now();
        let r = ps.regenerate();
        eprintln!("3x2 tight with fixture: {:?}", start.elapsed());
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        assert_eq!(r.bodies.len(), 7);
        assert_eq!(r.bodies[6].name, "Printing fixture");
        let pieces: f64 = r.bodies[..6].iter().map(|b| b.solid.volume()).sum();
        assert!(
            (pieces - 6.0 * 16000.0).abs() < 6.0 * 16000.0 * 2e-3,
            "{pieces}"
        );
        let fixture = &r.bodies[6].solid;
        fixture.validate().unwrap();
        let (lo, hi) = fixture.bounds().unwrap();
        assert!(
            (lo.z + 2.0).abs() < 1e-9 && (hi.z - 5.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        assert!(
            (lo.x + 6.0).abs() < 1e-9 && (hi.x - 126.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        let slab = 132.0 * 92.0 * 7.0;
        let pockets = (slab - fixture.volume()) / 5.0;
        assert!(pockets > 9600.0 && pockets < 9600.0 * 1.05, "{pockets}");
        for (show, light) in [
            (crate::PuzzleLayout::Light, true),
            (crate::PuzzleLayout::Dark, false),
        ] {
            ps.apply(Op::SetPuzzle {
                id,
                cols: None,
                rows: None,
                pitch: None,
                thickness: None,
                gap: None,
                bit: None,
                lock: None,
                grain: None,
                web: None,
                seed: None,
                jitter: None,
                fixture: None,
                show: Some(show),
                tabs: None,
                corners: None,
            })
            .unwrap();
            let r = ps.regenerate();
            assert!(
                r.errors().next().is_none(),
                "{:?}",
                r.errors().collect::<Vec<_>>()
            );
            assert_eq!(r.bodies.len(), 3, "{show:?}");
            let colour = if light { "light" } else { "dark" };
            assert!(
                r.bodies.iter().all(|b| b.name.ends_with(colour)),
                "{show:?}"
            );
            // The second row sits a bit higher than in the design.
            let top_row = r.bodies.iter().find(|b| b.name.contains(",2 ")).unwrap();
            let (lo, hi) = top_row.solid.bounds().unwrap();
            assert!(
                hi.y > 80.0 + 6.35 - 1e-6 && lo.y > 40.0 + 6.35 - 0.4 * 40.0,
                "{lo:?} {hi:?}"
            );
            let volume: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
            assert!(volume > 3.0 * 16000.0 * 0.8 && volume < 3.0 * 16000.0 * 1.2);
        }
    }

    #[test]
    fn ops_with_id_bases_commute() {
        // Two replicas apply the same two clients' ops in opposite orders.
        let base_a = 1 << 20;
        let base_b = 2 << 20;
        let ops_a = [
            (
                Op::AddSketch {
                    plane: PlaneRef::standard(StandardPlane::Top),
                    name: Some("A sketch".into()),
                },
                base_a,
            ),
            (
                Op::Sketch {
                    id: crate::FeatureId(base_a),
                    op: SketchOp::AddRectangle {
                        a: Vec2::ZERO,
                        b: Vec2::new(4.0, 4.0),
                    },
                },
                base_a + 256,
            ),
        ];
        let ops_b = [
            (
                Op::AddSketch {
                    plane: PlaneRef::standard(StandardPlane::Front),
                    name: Some("B sketch".into()),
                },
                base_b,
            ),
            (
                Op::Sketch {
                    id: crate::FeatureId(base_b),
                    op: SketchOp::AddCircle {
                        center: Vec2::ZERO,
                        radius: 1.0,
                    },
                },
                base_b + 256,
            ),
        ];
        let mut r1 = PartStudio::new("doc");
        let mut r2 = PartStudio::new("doc");
        for (op, base) in ops_a.iter().chain(ops_b.iter()) {
            r1.apply_with_base(op.clone(), Some(*base)).unwrap();
        }
        for (op, base) in ops_b.iter().chain(ops_a.iter()) {
            r2.apply_with_base(op.clone(), Some(*base)).unwrap();
        }
        // Same ids, same entities, same structural hash; only the order differs.
        let ids1: Vec<u32> = r1.features().iter().map(|f| f.id.0).collect();
        let ids2: Vec<u32> = r2.features().iter().map(|f| f.id.0).collect();
        assert_eq!(ids1, vec![base_a, base_b]);
        assert_eq!(ids2, vec![base_b, base_a]);
        r2.apply(Op::MoveFeature {
            id: crate::FeatureId(base_a),
            index: 0,
        })
        .unwrap();
        assert_eq!(r1.structural_hash(), r2.structural_hash());
        // Sketch entity ids come from the base too.
        let FeatureKind::Sketch(sf) = &r1.feature(crate::FeatureId(base_a)).unwrap().kind else {
            unreachable!()
        };
        assert!(sf.sketch.entities().all(|(id, _)| id.0 >= base_a + 256));
        // And the hash changes with structure but not with a value edit.
        let h = r1.structural_hash();
        let len_id = {
            let FeatureKind::Sketch(sf) = &r1.feature(crate::FeatureId(base_a)).unwrap().kind
            else {
                unreachable!()
            };
            sf.sketch
                .entities()
                .find(|(_, e)| e.kind_name() == "line")
                .map(|(id, _)| id)
                .unwrap()
        };
        r1.apply_with_base(
            Op::Sketch {
                id: crate::FeatureId(base_a),
                op: SketchOp::AddConstraint {
                    constraint: ok_sketch::Constraint::Length {
                        line: len_id,
                        value: 5.0,
                    },
                },
            },
            Some(base_a + 512),
        )
        .unwrap();
        assert_ne!(r1.structural_hash(), h);
        let h2 = r1.structural_hash();
        r1.apply(Op::Sketch {
            id: crate::FeatureId(base_a),
            op: SketchOp::SetConstraintValue {
                id: ok_sketch::ConstraintId(base_a + 512),
                value: 7.0,
            },
        })
        .unwrap();
        assert_eq!(r1.structural_hash(), h2);
    }

    #[test]
    fn facet_angle_setting_changes_resolution_and_invalidates_cache() {
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
                radius: 10.0,
            },
        })
        .unwrap();
        ps.apply(Op::AddExtrude {
            sketch: s,
            depth: 1.0,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        let coarse = ps.regenerate();
        let faces_coarse = coarse.bodies[0].solid.faces.len();
        let vol_coarse = coarse.bodies[0].solid.volume();
        ps.apply(Op::SetSettings { facet_angle: 1.0 }).unwrap();
        let fine = ps.regenerate();
        assert_eq!(fine.bodies[0].solid.faces.len(), 360 + 2);
        assert_eq!(faces_coarse, 72 + 2);
        let exact = std::f64::consts::PI * 100.0;
        assert!((fine.bodies[0].solid.volume() - exact).abs() < (vol_coarse - exact).abs());
        assert!((fine.bodies[0].solid.volume() - exact).abs() / exact < 1e-4);
        assert!(ps.apply(Op::SetSettings { facet_angle: 0.0 }).is_err());
        // Settings persist in the document.
        let again = PartStudio::from_json(&ps.to_json()).unwrap();
        assert_eq!(again.settings.facet_angle, 1.0);
    }

    #[test]
    fn projections_follow_the_model() {
        use crate::ProjectionSource;
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
                a: Vec2::new(0.0, 0.0),
                b: Vec2::new(10.0, 6.0),
            },
        })
        .unwrap();
        let e1 = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 5.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        // Sketch on the top face; project the face outline.
        let top = FaceRef {
            feature: e1,
            local: 1,
            part: None,
            near: Default::default(),
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
            op: SketchOp::Project {
                source: ProjectionSource::Face { face: top },
            },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let sk = &r.sketches[&s2];
        assert_eq!(sk.profiles.len(), 1);
        assert!(
            (sk.profiles[0].area().abs() - 60.0).abs() < 1e-9,
            "area {} curves {:?}",
            sk.profiles[0].area(),
            sk.curves
                .iter()
                .map(|c| (c.kind.clone(), c.points.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(sk.solve.status, ok_sketch::SolveStatus::FullyConstrained);
        let entities: Vec<EntityId> = match &ps.feature(s2).unwrap().kind {
            FeatureKind::Sketch(sf) => sf.projections[0].entities.clone(),
            _ => unreachable!(),
        };
        assert_eq!(entities.len(), 8, "4 shared points + 4 lines");
        let block = entities[0];
        // Extrude the projected outline into a second body.
        ps.apply(Op::AddExtrude {
            sketch: s2,
            depth: 3.0,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.errors().next().is_none());
        let vol: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        assert!((vol - (300.0 + 180.0)).abs() < 1e-6, "vol {vol}");
        // Widen the base rectangle: the projection follows, keeping its ids.
        let (right, _) = {
            let sk = match &ps.feature(s).unwrap().kind {
                FeatureKind::Sketch(sf) => &sf.sketch,
                _ => unreachable!(),
            };
            let p = sk
                .entities()
                .find_map(|(id, e)| match e {
                    Entity::Point { pos } if (pos.x - 10.0).abs() < 1e-9 && pos.y.abs() < 1e-9 => {
                        Some(id)
                    }
                    _ => None,
                })
                .unwrap();
            (p, ())
        };
        ps.apply(Op::Sketch {
            id: s,
            op: SketchOp::MovePoint {
                id: right,
                pos: Vec2::new(14.0, 0.0),
            },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        // The solver spreads the move over the rectangle; whatever the base
        // became, the projected outline matches it exactly.
        let base = r.bodies[0].solid.volume();
        assert!(base > 300.0 + 1.0, "base grew: {base}");
        let top = r.bodies[1].solid.volume();
        assert!(
            (top - base * 3.0 / 5.0).abs() < 1e-6,
            "base {base} top {top}"
        );
        match &ps.feature(s2).unwrap().kind {
            FeatureKind::Sketch(sf) => {
                assert_eq!(sf.projections[0].entities[0], block);
                assert!(sf.sketch.is_projected(block));
            }
            _ => unreachable!(),
        }
        // Removing the projection removes its entities and the region.
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::RemoveProjection { index: 0 },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.sketches[&s2].profiles.is_empty());
        assert!(
            r.errors().any(|(id, _)| id != s2),
            "extrude of empty sketch fails"
        );
    }

    #[test]
    fn projected_cylinder_edges_become_circles() {
        use crate::ProjectionSource;
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
                center: Vec2::new(2.0, 1.0),
                radius: 3.0,
            },
        })
        .unwrap();
        let e1 = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 5.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        // Sketch on the Top plane offset above the cylinder; project the top
        // rim edge (wall face 2 meets cap face 1) and the wall face outline.
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Standard {
                    base: StandardPlane::Top,
                    offset: 8.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let wall = FaceRef {
            feature: e1,
            local: 2,
            part: None,
            near: Default::default(),
        };
        let cap = FaceRef {
            feature: e1,
            local: 1,
            part: None,
            near: Default::default(),
        };
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::Project {
                source: ProjectionSource::Edge {
                    edge: EdgeRef { a: wall, b: cap },
                },
            },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let sf = match &ps.feature(s2).unwrap().kind {
            FeatureKind::Sketch(sf) => sf,
            _ => unreachable!(),
        };
        assert_eq!(sf.projections[0].entities.len(), 2, "centre point + circle");
        let circle = sf.sketch.entity(sf.projections[0].entities[1]).unwrap();
        match circle {
            Entity::Circle { center, radius } => {
                assert!((radius - 3.0).abs() < 1e-9);
                let c = sf.sketch.point(*center).unwrap();
                assert!(c.distance(Vec2::new(2.0, 1.0)) < 1e-9, "{c:?}");
            }
            other => panic!("{other:?}"),
        }
        let area = r.sketches[&s2].profiles[0].area().abs();
        let expected = 36.0 * 9.0 * (5.0f64).to_radians().sin(); // 72-gon
        assert!((area - expected).abs() < 1e-6, "{area} vs {expected}");
        // The whole wall projects to the same circle (both rims coincide).
        ps.apply(Op::Sketch {
            id: s2,
            op: SketchOp::Project {
                source: ProjectionSource::Face { face: wall },
            },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r.errors().next().is_none());
        let sf = match &ps.feature(s2).unwrap().kind {
            FeatureKind::Sketch(sf) => sf,
            _ => unreachable!(),
        };
        assert_eq!(sf.projections[1].entities.len(), 2);
        assert_ne!(sf.projections[0].block, sf.projections[1].block);
        // A projection of a face that comes later in the list is an error.
        let s3 = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::MoveFeature { id: s3, index: 0 }).unwrap();
        ps.apply(Op::Sketch {
            id: s3,
            op: SketchOp::Project {
                source: ProjectionSource::Face { face: cap },
            },
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(r
            .errors()
            .any(|(id, e)| id == s3 && e.contains("not found")));
    }

    #[test]
    fn fillet_on_one_rim_segment_blends_the_whole_rim() {
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
                radius: 10.0,
            },
        })
        .unwrap();
        let e1 = ps
            .apply(Op::AddExtrude {
                sketch: s,
                depth: 5.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let before = ps.regenerate().bodies[0].solid.volume();
        // One facet (local 2) against the top cap (local 1) stands for the rim.
        ps.apply(Op::AddBlend {
            kind: BlendKind::Chamfer,
            edges: vec![EdgeRef {
                a: FaceRef {
                    feature: e1,
                    local: 1,
                    part: None,
                    near: Default::default(),
                },
                b: FaceRef {
                    feature: e1,
                    local: 2,
                    part: None,
                    near: Default::default(),
                },
            }],
            size: 2.0,
            name: None,
        })
        .unwrap();
        let r = ps.regenerate();
        assert!(
            r.errors().next().is_none(),
            "{:?}",
            r.errors().collect::<Vec<_>>()
        );
        let removed = before - r.bodies[0].solid.volume();
        let expected = std::f64::consts::PI * 4.0 * (10.0 - 2.0 / 3.0);
        assert!(
            (removed - expected).abs() / expected < 0.02,
            "removed {removed}, expected {expected}"
        );
    }

    #[test]
    fn sweep_and_loft_features() {
        let mut ps = PartStudio::new("t");
        // Path: an L on the Top plane from the origin.
        let path = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        let r = ps
            .apply(Op::Sketch {
                id: path,
                op: SketchOp::AddLine {
                    a: Vec2::ZERO,
                    b: Vec2::new(10.0, 0.0),
                },
            })
            .unwrap();
        let end1 = r.entities[2];
        let r2 = ps
            .apply(Op::Sketch {
                id: path,
                op: SketchOp::AddLine {
                    a: Vec2::new(10.0, 0.0),
                    b: Vec2::new(10.0, 6.0),
                },
            })
            .unwrap();
        ps.apply(Op::Sketch {
            id: path,
            op: SketchOp::AddConstraint {
                constraint: ok_sketch::Constraint::Coincident {
                    a: end1,
                    b: r2.entities[1],
                },
            },
        })
        .unwrap();
        // Profile: 2x2 square centred on the origin of the Right plane (normal +X).
        let prof = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Right),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: prof,
            op: SketchOp::AddRectangle {
                a: Vec2::new(-1.0, -1.0),
                b: Vec2::new(1.0, 1.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddSweep {
            sketch: prof,
            path,
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
        assert!(
            (r.bodies[0].solid.volume() - 4.0 * 16.0).abs() < 1e-6,
            "vol {}",
            r.bodies[0].solid.volume()
        );

        // Loft between a square and a smaller square 3 up, as a new body.
        let a = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Standard {
                    base: StandardPlane::Top,
                    offset: 20.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: a,
            op: SketchOp::AddRectangle {
                a: Vec2::new(-2.0, -2.0),
                b: Vec2::new(2.0, 2.0),
            },
        })
        .unwrap();
        let b = ps
            .apply(Op::AddSketch {
                plane: PlaneRef::Standard {
                    base: StandardPlane::Top,
                    offset: 23.0,
                },
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(Op::Sketch {
            id: b,
            op: SketchOp::AddRectangle {
                a: Vec2::new(-1.0, -1.0),
                b: Vec2::new(1.0, 1.0),
            },
        })
        .unwrap();
        ps.apply(Op::AddLoft {
            sketch: a,
            sketch_b: b,
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
        assert_eq!(r.bodies.len(), 2);
        let expected = 16.0 + 4.0 + 8.0;
        assert!(
            (r.bodies[1].solid.volume() - expected).abs() < 1e-6,
            "vol {}",
            r.bodies[1].solid.volume()
        );
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
                        part: None,
                        near: Default::default(),
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
    /// A reference made with neighbour hashes finds its piece by
    /// topology after an edit elsewhere reorders the pieces by position.
    #[test]
    fn a_reference_follows_its_piece_when_pieces_reorder() {
        use crate::SketchOp;
        use crate::{Op, PlaneRef, StandardPlane};
        let mut ps = PartStudio::new("plate");
        let feature = |ps: &mut PartStudio, op: Op| ps.apply(op).unwrap().feature.unwrap();
        let sketch =
            |ps: &mut PartStudio, plane: PlaneRef| feature(ps, Op::AddSketch { plane, name: None });
        let rect = |ps: &mut PartStudio, id: FeatureId, a: (f64, f64), b: (f64, f64)| {
            ps.apply(Op::Sketch {
                id,
                op: SketchOp::AddRectangle {
                    a: Vec2::new(a.0, a.1),
                    b: Vec2::new(b.0, b.1),
                },
            })
            .unwrap();
        };
        let cut = |ps: &mut PartStudio, sketch: FeatureId, depth: f64| {
            feature(
                ps,
                Op::AddExtrude {
                    sketch,
                    depth,
                    direction: ExtrudeDirection::Reverse,
                    end: ExtrudeEnd::Blind,
                    profiles: ProfileSelection::All,
                    op: BodyOp::Remove,
                    name: None,
                },
            )
        };
        let base = sketch(&mut ps, PlaneRef::standard(StandardPlane::Top));
        rect(&mut ps, base, (0.0, 0.0), (100.0, 100.0));
        let plate = feature(
            &mut ps,
            Op::AddExtrude {
                sketch: base,
                depth: 10.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            },
        );
        let top_plane = PlaneRef::Standard {
            base: StandardPlane::Top,
            offset: 10.0,
        };
        // A shallow slot across the plate splits the top face into a lower
        // piece (y < 48) and an upper one (y > 52) of one body.
        let slot = sketch(&mut ps, top_plane);
        rect(&mut ps, slot, (-1.0, 48.0), (101.0, 52.0));
        cut(&mut ps, slot, 5.0);
        // Two notches in the lower piece's front edge, only one active at
        // a time: pieces are ordered by the mean of their outline corners,
        // so the left notch puts the lower piece first and the right notch
        // the upper piece.
        let left = sketch(&mut ps, top_plane);
        rect(&mut ps, left, (10.0, -1.0), (40.0, 40.0));
        let left_cut = cut(&mut ps, left, 3.0);
        let right = sketch(&mut ps, top_plane);
        rect(&mut ps, right, (60.0, -1.0), (90.0, 40.0));
        let right_cut = cut(&mut ps, right, 3.0);
        ps.apply(Op::SetSuppressed {
            id: right_cut,
            suppressed: true,
        })
        .unwrap();
        let upper_piece = |ps: &mut PartStudio| -> (u32, FaceRef) {
            let r = ps.regenerate();
            let body = &r.bodies[0];
            let parts = body.solid.face_parts();
            let near = piece_near(&body.solid, &parts);
            let (i, _) = body
                .solid
                .faces
                .iter()
                .enumerate()
                .find(|(_, f)| {
                    f.origin.feature == plate.0
                        && f.origin.local == 1
                        && f.loops[0]
                            .iter()
                            .all(|&v| body.solid.vertices[v as usize].y > 50.0)
                })
                .unwrap();
            (
                parts[i],
                FaceRef {
                    feature: plate,
                    local: 1,
                    part: Some(parts[i]),
                    near: near[i],
                },
            )
        };
        let (part, upper) = upper_piece(&mut ps);
        assert_eq!(part, 1, "the upper piece is second by position at first");
        assert!(upper.has_near());
        // A hole in the upper piece, referenced with the neighbours.
        let centres = sketch(
            &mut ps,
            PlaneRef::Face {
                face: upper,
                offset: 0.0,
            },
        );
        ps.apply(Op::Sketch {
            id: centres,
            op: SketchOp::AddPoint {
                pos: Vec2::new(50.0, 75.0),
            },
        })
        .unwrap();
        let hole = feature(
            &mut ps,
            Op::AddHole {
                sketch: centres,
                diameter: 6.0,
                depth: 0.0,
                through_all: true,
                direction: ExtrudeDirection::Reverse,
                counterbore: None,
                countersink: None,
                name: None,
            },
        );
        let hole_y = |ps: &mut PartStudio| -> f64 {
            let r = ps.regenerate();
            let errors: Vec<_> = r.errors().collect();
            assert!(errors.is_empty(), "{errors:?}");
            let body = &r.bodies[0];
            let mut sum = 0.0;
            let mut n = 0.0;
            for f in body
                .solid
                .faces
                .iter()
                .filter(|f| f.origin.feature == hole.0)
            {
                for &v in &f.loops[0] {
                    sum += body.solid.vertices[v as usize].y;
                    n += 1.0;
                }
            }
            assert!(n > 0.0, "the hole has faces");
            sum / n
        };
        assert!((hole_y(&mut ps) - 75.0).abs() < 1.0);
        // Swap the notches: the pieces trade positions.
        ps.apply(Op::SetSuppressed {
            id: left_cut,
            suppressed: true,
        })
        .unwrap();
        ps.apply(Op::SetSuppressed {
            id: right_cut,
            suppressed: false,
        })
        .unwrap();
        let (part, upper_now) = upper_piece(&mut ps);
        assert_eq!(part, 0, "the upper piece is first by position now");
        // By position alone the old reference would name the lower piece.
        let r = ps.regenerate();
        let positional = FaceRef {
            feature: plate,
            local: 1,
            part: Some(1),
            near: Default::default(),
        };
        let found = r.bodies[0].find_face(&positional).unwrap();
        assert!(found.loops[0]
            .iter()
            .all(|&v| r.bodies[0].solid.vertices[v as usize].y < 50.0));
        // With its neighbours, it still names the upper piece, so the
        // hole stays where it was drilled.
        let found = r.bodies[0].find_face(&upper).unwrap();
        assert!(found.loops[0]
            .iter()
            .all(|&v| r.bodies[0].solid.vertices[v as usize].y > 50.0));
        // The hole's facets are neighbours now too, so a fresh reference
        // samples different hashes; the old one still scores highest.
        assert!(upper_now.has_near());
        assert!((hole_y(&mut ps) - 75.0).abs() < 1.0);
        // If the slot is suppressed the split heals: the reference finds
        // the one piece left, where a bare piece number 1 would fail.
        ps.apply(Op::SetSuppressed {
            id: slot_cut_of(&ps),
            suppressed: true,
        })
        .unwrap();
        assert!((hole_y(&mut ps) - 75.0).abs() < 1.0);
    }

    /// The slot cut of the test above: the third feature after the plate.
    fn slot_cut_of(ps: &PartStudio) -> FeatureId {
        ps.features()[3].id
    }
}
