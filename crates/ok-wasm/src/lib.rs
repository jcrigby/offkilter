//! WebAssembly bindings.
//!
//! The API is deliberately narrow: JSON in, JSON out, plus typed arrays for
//! mesh data. All edits go through [`Doc::apply`] with an `ok_model::DocOp`
//! encoded as JSON, so the JavaScript side never mutates the model
//! directly. Regeneration is per tab: a part studio tab yields features,
//! sketches and bodies; an assembly tab yields placed instances and mates.

use ok_model::{Body, DocOp, Document, RegenResult, TabId, TabKind};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Doc {
    inner: Document,
    /// Last studio regeneration (sketches, statuses), if the tab was a studio.
    last: RegenResult,
    /// Bodies shown for the last regenerated tab (studio bodies or placed instances).
    bodies: Vec<Body>,
    /// Last assembly regeneration, for on-demand checks.
    assembly: Option<ok_model::AssemblyResult>,
}

#[derive(Serialize)]
struct TabSummary<'a> {
    id: TabId,
    name: &'a str,
    kind: &'static str,
}

/// Regeneration summary handed to the client. Mesh buffers are fetched
/// separately by body index to avoid JSON-encoding large arrays.
#[derive(Serialize)]
struct Summary<'a> {
    name: &'a str,
    tabs: Vec<TabSummary<'a>>,
    tab: TabId,
    kind: &'static str,
    tab_name: &'a str,
    features: Vec<FeatureSummary<'a>>,
    bodies: Vec<BodySummary<'a>>,
    variables: std::collections::BTreeMap<String, f64>,
    sketches: std::collections::BTreeMap<ok_model::FeatureId, ok_model::SketchResult>,
    settings: ok_model::Settings,
    instances: Vec<InstanceSummary<'a>>,
    mates: Vec<MateSummary<'a>>,
}

#[derive(Serialize)]
struct FeatureSummary<'a> {
    id: ok_model::FeatureId,
    name: &'a str,
    suppressed: bool,
    kind: &'a ok_model::FeatureKind,
    error: Option<&'a str>,
    bindings: &'a std::collections::BTreeMap<String, String>,
    value: Option<f64>,
    /// Bodies available to a boolean feature, as (source feature, name).
    #[serde(skip_serializing_if = "Option::is_none")]
    candidates: Option<&'a [(ok_model::FeatureId, String)]>,
}

#[derive(Serialize)]
struct BodySummary<'a> {
    name: &'a str,
    source: ok_model::FeatureId,
    vertices: usize,
    triangles: usize,
    face_count: usize,
    faces: Vec<FaceInfo>,
    bounds: Option<(ok_math::Vec3, ok_math::Vec3)>,
    volume: f64,
    area: f64,
    centroid: Option<ok_math::Vec3>,
}

#[derive(Serialize)]
struct FaceInfo {
    origin: ok_brep::FaceOrigin,
    surface: &'static str,
    normal: ok_math::Vec3,
}

#[derive(Serialize)]
struct InstanceSummary<'a> {
    #[serde(flatten)]
    instance: &'a ok_model::Instance,
    /// Indices into `bodies` of the instance's placed bodies (empty if unresolved).
    body_indices: Vec<usize>,
    transform: Option<ok_brep::Transform>,
    error: Option<&'a str>,
}

#[derive(Serialize)]
struct MateSummary<'a> {
    #[serde(flatten)]
    mate: &'a ok_model::Mate,
    error: Option<&'a str>,
    /// World connector frames at the solution, for drawing.
    frame_a: Option<ok_math::Plane>,
    frame_b: Option<ok_math::Plane>,
}

/// World connector frame of a face on a placed instance: the placed bodies
/// are already in world coordinates.
fn world_frame(
    result: &ok_model::AssemblyResult,
    c: &ok_model::Connector,
) -> Option<ok_math::Plane> {
    result
        .bodies
        .iter()
        .zip(&result.placed)
        .filter(|(_, id)| **id == c.instance)
        .find_map(|(b, _)| ok_model::connector_frame(&b.solid, &c.face, &c.anchor))
}

fn body_summary(b: &Body) -> BodySummary<'_> {
    BodySummary {
        name: &b.name,
        source: b.source,
        vertices: b.mesh.vertex_count(),
        triangles: b.mesh.triangle_count(),
        face_count: b.solid.faces.len(),
        faces: b
            .solid
            .faces
            .iter()
            .map(|f| FaceInfo {
                origin: f.origin,
                surface: match b.solid.surfaces.get(f.surface) {
                    Some(ok_brep::Surface::Cylinder { .. }) => "cylinder",
                    _ => "plane",
                },
                normal: f.plane.normal,
            })
            .collect(),
        bounds: b.solid.bounds(),
        volume: b.solid.volume(),
        area: b.solid.surface_area(),
        centroid: b.solid.centroid(),
    }
}

#[wasm_bindgen]
impl Doc {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Doc {
        Doc {
            inner: Document::default(),
            last: RegenResult::default(),
            bodies: Vec::new(),
            assembly: None,
        }
    }

    /// The built-in example part.
    pub fn demo() -> Doc {
        Doc {
            inner: Document::demo(),
            last: RegenResult::default(),
            bodies: Vec::new(),
            assembly: None,
        }
    }

    /// Parses a document (or a bare part studio from older files).
    pub fn from_json(json: &str) -> Result<Doc, JsError> {
        let inner = Document::from_json(json).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Doc {
            inner,
            last: RegenResult::default(),
            bodies: Vec::new(),
            assembly: None,
        })
    }

    pub fn to_json(&self) -> String {
        self.inner.to_json()
    }

    /// Applies a `DocOp` (JSON) and returns the `DocOpResult` as JSON,
    /// including the `inverse` ops that undo it.
    pub fn apply(&mut self, op_json: &str, base: Option<u32>) -> Result<String, JsError> {
        let op: DocOp = serde_json::from_str(op_json).map_err(|e| JsError::new(&e.to_string()))?;
        let r = self
            .inner
            .apply_with_inverse(op, base)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(serde_json::to_string(&r).unwrap())
    }

    /// Structural hash of the document as hex, for replica consistency checks.
    pub fn structural_hash(&self) -> String {
        format!("{:016x}", self.inner.structural_hash())
    }

    /// The first part studio tab's id (what older clients edit).
    pub fn first_studio(&self) -> Option<u32> {
        self.inner.first_studio().map(|t| t.0)
    }

    /// Names of the bodies a part studio tab produces (for inserting instances).
    pub fn studio_bodies(&mut self, tab: u32) -> String {
        let names: Vec<String> = self
            .inner
            .regenerate_studio(TabId(tab), None)
            .map(|r| r.bodies.iter().map(|b| b.name.clone()).collect())
            .unwrap_or_default();
        serde_json::to_string(&names).unwrap()
    }

    /// Regenerates a tab and returns a JSON summary. For a studio, with
    /// `rollback` set, the summary and meshes reflect the state after that
    /// many features (used while editing a feature). An unknown tab falls
    /// back to the first part studio.
    pub fn regenerate(&mut self, tab: Option<u32>, rollback: Option<usize>) -> String {
        let tab = tab
            .map(TabId)
            .filter(|t| self.inner.tab(*t).is_some())
            .or_else(|| self.inner.first_studio())
            .unwrap_or(TabId(0));
        let kind = self
            .inner
            .tab(tab)
            .map(|t| t.kind_name())
            .unwrap_or("part_studio");
        let mut instances = Vec::new();
        let mut mates = Vec::new();
        let asm_result;
        if kind == "assembly" {
            self.last = RegenResult::default();
            asm_result = self.inner.regenerate_assembly(tab).unwrap_or_default();
            self.bodies = asm_result.bodies.clone();
            self.assembly = Some(asm_result.clone());
            if let Ok(a) = self.inner.assembly(tab) {
                for i in &a.instances {
                    instances.push(InstanceSummary {
                        instance: i,
                        body_indices: asm_result
                            .placed
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| **p == i.id)
                            .map(|(k, _)| k)
                            .collect(),
                        transform: asm_result.transforms.get(&i.id).copied(),
                        error: asm_result.instance_errors.get(&i.id).map(|s| s.as_str()),
                    });
                }
                for m in &a.mates {
                    mates.push(MateSummary {
                        mate: m,
                        error: asm_result.mate_errors.get(&m.id).map(|s| s.as_str()),
                        frame_a: world_frame(&asm_result, &m.a),
                        frame_b: world_frame(&asm_result, &m.b),
                    });
                }
            }
        } else {
            self.assembly = None;
            self.last = self
                .inner
                .regenerate_studio(tab, rollback)
                .unwrap_or_default();
            self.bodies = self.last.bodies.clone();
        }
        let studio = match self.inner.tab(tab).map(|t| &t.kind) {
            Some(TabKind::PartStudio(p)) => Some(p),
            _ => None,
        };
        let features = studio
            .map(|p| {
                p.features()
                    .iter()
                    .map(|f| FeatureSummary {
                        id: f.id,
                        name: &f.name,
                        suppressed: f.suppressed,
                        kind: &f.kind,
                        error: self
                            .last
                            .statuses
                            .iter()
                            .find(|s| s.id == f.id)
                            .and_then(|s| s.error.as_deref()),
                        bindings: &f.bindings,
                        value: self
                            .last
                            .statuses
                            .iter()
                            .find(|s| s.id == f.id)
                            .and_then(|s| s.value),
                        candidates: self
                            .last
                            .statuses
                            .iter()
                            .find(|s| s.id == f.id)
                            .and_then(|s| s.candidates.as_deref()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let summary = Summary {
            name: &self.inner.name,
            tabs: self
                .inner
                .tabs
                .iter()
                .map(|t| TabSummary {
                    id: t.id,
                    name: t.name(),
                    kind: t.kind_name(),
                })
                .collect(),
            tab,
            kind,
            tab_name: self.inner.tab(tab).map(|t| t.name()).unwrap_or(""),
            features,
            bodies: self.bodies.iter().map(body_summary).collect(),
            sketches: self.last.sketches.clone(),
            variables: self.last.variables.clone(),
            settings: studio.map(|p| p.settings).unwrap_or_default(),
            instances,
            mates,
        };
        serde_json::to_string(&summary).unwrap()
    }

    /// World frame of a connector (JSON `{instance, face, anchor?}`) on the
    /// last regenerated assembly tab (JSON plane, or null).
    pub fn connector_frame(&self, connector_json: &str) -> String {
        let c: ok_model::Connector = match serde_json::from_str(connector_json) {
            Ok(c) => c,
            Err(_) => return "null".into(),
        };
        let frame = self.assembly.as_ref().and_then(|r| world_frame(r, &c));
        serde_json::to_string(&frame).unwrap()
    }

    /// Overlapping instance pairs of the last regenerated assembly tab, as
    /// JSON `{ "overlaps": [{a, b, volume}], "failed": [[a, b]] }`.
    pub fn interferences(&self) -> String {
        let (overlaps, failed) = self
            .assembly
            .as_ref()
            .map(|r| r.interferences())
            .unwrap_or_default();
        serde_json::json!({ "overlaps": overlaps, "failed": failed }).to_string()
    }

    /// One frame of a mate animation: the last regenerated assembly tab
    /// resolved as if mate `mate` had `angle` and `offset`, as a JSON list
    /// of rigid transforms (row-major 3x3 `m` and `t`), one per shown
    /// body, that move it from where it is to where it would be; `null`
    /// for a body that does not move, or overall when the frame cannot be
    /// resolved (an unknown mate, or one that then fails).
    pub fn mate_preview(&mut self, tab: u32, mate: u32, angle: f64, offset: f64) -> String {
        let Some(current) = self.assembly.as_ref() else {
            return "null".into();
        };
        let mate = ok_model::MateId(mate);
        let Ok(next) = self.inner.preview_assembly(TabId(tab), mate, angle, offset) else {
            return "null".into();
        };
        if next.mate_errors.contains_key(&mate) || next.placed != current.placed {
            return "null".into();
        }
        let deltas: Vec<Option<ok_brep::Transform>> = current
            .placed
            .iter()
            .map(|id| {
                let (from, to) = (current.transforms.get(id)?, next.transforms.get(id)?);
                Some(to.then_inverse_of(from))
            })
            .collect();
        serde_json::to_string(&deltas).unwrap()
    }

    /// A section view of the current tab's bodies: `view_json` is
    /// `{"dir":[..],"up":[..],"origin":[..],"normal":[..]}`; the material
    /// on the plane's normal side is removed and the rest drawn looking
    /// along `dir`. The result adds `"cut":[[[x,y],..],..]`, the outlines
    /// of the faces in the cut plane, to the visible and hidden lines.
    pub fn drawing_section(&self, view_json: &str) -> String {
        #[derive(serde::Deserialize)]
        struct SectionSpec {
            dir: [f64; 3],
            up: [f64; 3],
            origin: [f64; 3],
            normal: [f64; 3],
        }
        let spec: SectionSpec = match serde_json::from_str(view_json) {
            Ok(v) => v,
            Err(e) => return serde_json::json!({ "error": e.to_string() }).to_string(),
        };
        let v3 = |a: [f64; 3]| ok_math::Vec3::new(a[0], a[1], a[2]);
        let Some(plane) = ok_math::Plane::from_origin_normal(v3(spec.origin), v3(spec.normal))
        else {
            return serde_json::json!({ "error": "degenerate section plane" }).to_string();
        };
        let solids: Vec<&ok_brep::Solid> = self.bodies.iter().map(|b| &b.solid).collect();
        let view = ok_brep::View {
            dir: v3(spec.dir),
            up: v3(spec.up),
        };
        serde_json::to_string(&ok_brep::section_view(&solids, view, &plane)).unwrap()
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Orthographic view of the current tab's bodies with hidden lines
    /// removed. `view_json` is `{"dir":[x,y,z],"up":[x,y,z]}` (the viewer
    /// looks along `dir`); the result is `{"visible":[[[x,y],[x,y]],..],
    /// "hidden":[..]}` in view millimetres, x right and y up.
    pub fn drawing_view(&self, view_json: &str) -> String {
        #[derive(serde::Deserialize)]
        struct ViewSpec {
            dir: [f64; 3],
            up: [f64; 3],
        }
        let spec: ViewSpec = match serde_json::from_str(view_json) {
            Ok(v) => v,
            Err(e) => return serde_json::json!({ "error": e.to_string() }).to_string(),
        };
        let solids: Vec<&ok_brep::Solid> = self.bodies.iter().map(|b| &b.solid).collect();
        let view = ok_brep::View {
            dir: ok_math::Vec3::new(spec.dir[0], spec.dir[1], spec.dir[2]),
            up: ok_math::Vec3::new(spec.up[0], spec.up[1], spec.up[2]),
        };
        serde_json::to_string(&ok_brep::project_view(&solids, view)).unwrap()
    }

    pub fn body_positions(&self, i: usize) -> Vec<f32> {
        self.bodies
            .get(i)
            .map(|b| b.mesh.positions.clone())
            .unwrap_or_default()
    }

    pub fn body_normals(&self, i: usize) -> Vec<f32> {
        self.bodies
            .get(i)
            .map(|b| b.mesh.normals.clone())
            .unwrap_or_default()
    }

    pub fn body_indices(&self, i: usize) -> Vec<u32> {
        self.bodies
            .get(i)
            .map(|b| b.mesh.indices.clone())
            .unwrap_or_default()
    }

    /// Face index of every triangle, parallel to `body_indices` / 3.
    pub fn body_face_ids(&self, i: usize) -> Vec<u32> {
        self.bodies
            .get(i)
            .map(|b| b.triangle_faces.clone())
            .unwrap_or_default()
    }

    /// Surface index of every face, so the client can treat all facets of
    /// one curved surface as one face (edge highlighting, picking).
    pub fn body_face_surfaces(&self, i: usize) -> Vec<u32> {
        self.bodies
            .get(i)
            .map(|b| b.solid.faces.iter().map(|f| f.surface as u32).collect())
            .unwrap_or_default()
    }

    /// Display edges as flat xyz pairs: `[x0, y0, z0, x1, y1, z1, ...]`.
    pub fn body_edges(&self, i: usize) -> Vec<f32> {
        self.bodies
            .get(i)
            .map(|b| {
                b.edges
                    .iter()
                    .flat_map(|e| {
                        let [a, b] = e.points;
                        [
                            a.x as f32, a.y as f32, a.z as f32, b.x as f32, b.y as f32, b.z as f32,
                        ]
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The two face indices of every display edge, parallel to `body_edges`.
    pub fn body_edge_faces(&self, i: usize) -> Vec<u32> {
        self.bodies
            .get(i)
            .map(|b| {
                b.edges
                    .iter()
                    .flat_map(|e| [e.faces[0] as u32, e.faces[1] as u32])
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for Doc {
    fn default() -> Self {
        Self::new()
    }
}

/// Kernel version, for the client's about box.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
