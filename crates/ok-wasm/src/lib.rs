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
    /// Index into `bodies` when the instance resolved.
    body_index: Option<usize>,
    transform: Option<ok_brep::Transform>,
    error: Option<&'a str>,
}

#[derive(Serialize)]
struct MateSummary<'a> {
    #[serde(flatten)]
    mate: &'a ok_model::Mate,
    error: Option<&'a str>,
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
        }
    }

    /// The built-in example part.
    pub fn demo() -> Doc {
        Doc {
            inner: Document::demo(),
            last: RegenResult::default(),
            bodies: Vec::new(),
        }
    }

    /// Parses a document (or a bare part studio from older files).
    pub fn from_json(json: &str) -> Result<Doc, JsError> {
        let inner = Document::from_json(json).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Doc {
            inner,
            last: RegenResult::default(),
            bodies: Vec::new(),
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
            if let Ok(a) = self.inner.assembly(tab) {
                for i in &a.instances {
                    instances.push(InstanceSummary {
                        instance: i,
                        body_index: asm_result.placed.iter().position(|p| *p == i.id),
                        transform: asm_result.transforms.get(&i.id).copied(),
                        error: asm_result.instance_errors.get(&i.id).map(|s| s.as_str()),
                    });
                }
                for m in &a.mates {
                    mates.push(MateSummary {
                        mate: m,
                        error: asm_result.mate_errors.get(&m.id).map(|s| s.as_str()),
                    });
                }
            }
        } else {
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

    pub fn body_count(&self) -> usize {
        self.bodies.len()
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
