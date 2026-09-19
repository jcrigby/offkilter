//! WebAssembly bindings.
//!
//! The API is deliberately narrow: JSON in, JSON out, plus typed arrays for
//! mesh data. All document edits go through [`Studio::apply`] with an
//! `ok_model::Op` encoded as JSON, so the JavaScript side never mutates the
//! model directly.

use ok_model::{PartStudio, RegenResult};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Studio {
    inner: PartStudio,
    last: RegenResult,
}

/// Regeneration summary handed to the client. Mesh buffers are fetched
/// separately by body index to avoid JSON-encoding large arrays.
#[derive(Serialize)]
struct Summary<'a> {
    name: &'a str,
    features: Vec<FeatureSummary<'a>>,
    bodies: Vec<BodySummary<'a>>,
    sketches: &'a std::collections::BTreeMap<ok_model::FeatureId, ok_model::SketchResult>,
}

#[derive(Serialize)]
struct FeatureSummary<'a> {
    id: ok_model::FeatureId,
    name: &'a str,
    suppressed: bool,
    kind: &'a ok_model::FeatureKind,
    error: Option<&'a str>,
}

#[derive(Serialize)]
struct BodySummary<'a> {
    name: &'a str,
    source: ok_model::FeatureId,
    vertices: usize,
    triangles: usize,
    faces: usize,
    bounds: Option<(ok_math::Vec3, ok_math::Vec3)>,
    volume: f64,
}

#[wasm_bindgen]
impl Studio {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Studio {
        Studio {
            inner: PartStudio::default(),
            last: RegenResult::default(),
        }
    }

    /// The built-in example part.
    pub fn demo() -> Studio {
        Studio {
            inner: PartStudio::demo(),
            last: RegenResult::default(),
        }
    }

    pub fn from_json(json: &str) -> Result<Studio, JsError> {
        let inner = PartStudio::from_json(json).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Studio {
            inner,
            last: RegenResult::default(),
        })
    }

    pub fn to_json(&self) -> String {
        self.inner.to_json()
    }

    /// Applies an `Op` (JSON) and returns the `OpResult` as JSON.
    pub fn apply(&mut self, op_json: &str) -> Result<String, JsError> {
        let r = self
            .inner
            .apply_json(op_json)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(serde_json::to_string(&r).unwrap())
    }

    /// Regenerates the part and returns a JSON summary.
    pub fn regenerate(&mut self) -> String {
        self.last = self.inner.regenerate();
        let features = self
            .inner
            .features()
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
            })
            .collect();
        let bodies = self
            .last
            .bodies
            .iter()
            .map(|b| BodySummary {
                name: &b.name,
                source: b.source,
                vertices: b.mesh.vertex_count(),
                triangles: b.mesh.triangle_count(),
                faces: b.solid.faces.len(),
                bounds: b.solid.bounds(),
                volume: b.solid.volume(),
            })
            .collect();
        serde_json::to_string(&Summary {
            name: &self.inner.name,
            features,
            bodies,
            sketches: &self.last.sketches,
        })
        .unwrap()
    }

    pub fn body_count(&self) -> usize {
        self.last.bodies.len()
    }

    pub fn body_positions(&self, i: usize) -> Vec<f32> {
        self.last
            .bodies
            .get(i)
            .map(|b| b.mesh.positions.clone())
            .unwrap_or_default()
    }

    pub fn body_normals(&self, i: usize) -> Vec<f32> {
        self.last
            .bodies
            .get(i)
            .map(|b| b.mesh.normals.clone())
            .unwrap_or_default()
    }

    pub fn body_indices(&self, i: usize) -> Vec<u32> {
        self.last
            .bodies
            .get(i)
            .map(|b| b.mesh.indices.clone())
            .unwrap_or_default()
    }

    /// Display edges as flat xyz pairs: `[x0, y0, z0, x1, y1, z1, ...]`.
    pub fn body_edges(&self, i: usize) -> Vec<f32> {
        self.last
            .bodies
            .get(i)
            .map(|b| {
                b.edges
                    .iter()
                    .flat_map(|[a, b]| {
                        [
                            a.x as f32, a.y as f32, a.z as f32, b.x as f32, b.y as f32, b.z as f32,
                        ]
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}

/// Kernel version, for the client's about box.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
