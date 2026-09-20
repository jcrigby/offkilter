//! A readable report of a tab for scripts and language models: the
//! features with their ids, kinds and errors; the bodies with their faces
//! (each with the reference a later feature can use), cylinders, volumes
//! and bounds; the sketches with their entities, constraints and solver
//! state; and, for assemblies, the instances and mates. Everything an
//! agent needs to decide the next op, in one JSON document.

use crate::{
    AssemblyOp, Body, DocOp, Document, FaceRef, FeatureId, FeatureKind, Op, SketchOp, TabId,
};
use ok_math::Vec3;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TabReport {
    pub tab: TabId,
    pub name: String,
    pub kind: &'static str,
    pub ok: bool,
    pub features: Vec<FeatureReport>,
    pub bodies: Vec<BodyReport>,
    pub sketches: Vec<SketchReport>,
    pub variables: std::collections::BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<InstanceReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mates: Vec<MateReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureReport {
    pub id: FeatureId,
    pub name: String,
    pub kind: String,
    pub suppressed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The feature's parameters as stored (the same shape `set_*` ops take).
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct BodyReport {
    pub index: usize,
    pub name: String,
    pub source: FeatureId,
    pub volume: f64,
    pub area: f64,
    pub bounds: Option<(Vec3, Vec3)>,
    pub centroid: Option<Vec3>,
    pub faces: Vec<FaceReport>,
    pub cylinders: Vec<CylinderReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<crate::Material>,
}

/// One face of a body, with the reference (`feature`, `local`, `part`)
/// that names it in later ops, its plane or cylinder, and where it is.
#[derive(Debug, Clone, Serialize)]
pub struct FaceReport {
    pub index: usize,
    pub reference: FaceRef,
    pub surface: &'static str,
    pub normal: Vec3,
    pub centroid: Vec3,
    pub area: f64,
    /// How many facets of one curved surface this face stands for (they
    /// share the surface and merge under one reference).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CylinderReport {
    pub origin: Vec3,
    pub axis: Vec3,
    pub radius: f64,
    pub hole: bool,
    /// A face reference on this cylinder, usable in ops.
    pub reference: FaceRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct SketchReport {
    pub feature: FeatureId,
    pub name: String,
    pub plane: ok_math::Plane,
    pub solve: ok_sketch::SolveResult,
    pub regions: usize,
    /// Entities with their ids, as the `sketch` ops address them.
    pub entities: serde_json::Value,
    pub constraints: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceReport {
    pub id: crate::InstanceId,
    pub name: String,
    pub studio: TabId,
    pub body: usize,
    pub fixed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MateReport {
    pub id: crate::MateId,
    pub name: String,
    pub kind: crate::MateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn face_reports(body: &Body) -> (Vec<FaceReport>, Vec<CylinderReport>) {
    let solid = &body.solid;
    let parts = solid.face_parts();
    let near = crate::regen::piece_near(solid, &parts);
    let mut faces: Vec<FaceReport> = Vec::new();
    let mut cylinders: Vec<CylinderReport> = Vec::new();
    // One report per (origin, part), and one per curved surface: the facets
    // of a cylinder have their own origins, but any one of them names the
    // whole cylinder in an op, so they collapse into one line.
    #[derive(PartialEq, Eq, Hash)]
    enum Key {
        Flat(u32, u32, u32),
        Curved(usize),
    }
    let mut seen: std::collections::HashMap<Key, usize> = std::collections::HashMap::new();
    for (i, f) in solid.faces.iter().enumerate() {
        let pts: Vec<Vec3> = f.loops[0]
            .iter()
            .map(|&v| solid.vertices[v as usize])
            .collect();
        let area = ok_brep_area(&pts);
        let centroid = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len().max(1) as f64);
        let reference = FaceRef {
            feature: FeatureId(f.origin.feature),
            local: f.origin.local,
            part: Some(parts[i]),
            near: near[i],
        };
        let surface = match solid.surfaces.get(f.surface) {
            Some(ok_brep::Surface::Cylinder { .. }) => "cylinder",
            Some(ok_brep::Surface::Plane { .. }) => "plane",
            _ => "curved",
        };
        let key = if surface == "plane" {
            Key::Flat(f.origin.feature, f.origin.local, parts[i])
        } else {
            Key::Curved(f.surface)
        };
        if let Some(ok_brep::Surface::Cylinder {
            origin,
            axis,
            radius,
        }) = solid.surfaces.get(f.surface).copied()
        {
            if !seen.contains_key(&key) {
                let d = centroid - origin;
                let radial = d - axis * d.dot(axis);
                cylinders.push(CylinderReport {
                    origin,
                    axis,
                    radius,
                    hole: f.plane.normal.dot(radial) < 0.0,
                    reference,
                });
            }
        }
        match seen.get(&key) {
            Some(&k) => {
                let r = &mut faces[k];
                let total = r.area + area;
                if total > 0.0 {
                    r.centroid = (r.centroid * r.area + centroid * area) * (1.0 / total);
                }
                r.area = total;
                r.facets = Some(r.facets.unwrap_or(1) + 1);
            }
            None => {
                seen.insert(key, faces.len());
                faces.push(FaceReport {
                    index: i,
                    reference,
                    surface,
                    normal: f.plane.normal,
                    centroid,
                    area,
                    facets: None,
                });
            }
        }
    }
    (faces, cylinders)
}

fn ok_brep_area(pts: &[Vec3]) -> f64 {
    let mut n = Vec3::ZERO;
    for i in 0..pts.len() {
        n += pts[i].cross(pts[(i + 1) % pts.len()]);
    }
    n.length() / 2.0
}

fn body_report(index: usize, b: &Body) -> BodyReport {
    let (faces, cylinders) = face_reports(b);
    BodyReport {
        index,
        name: b.name.clone(),
        source: b.source,
        volume: b.solid.volume(),
        area: b.solid.surface_area(),
        bounds: b.solid.bounds(),
        centroid: b.solid.centroid(),
        faces,
        cylinders,
        material: b.material.clone(),
    }
}

impl Document {
    /// Regenerates `tab` and reports what is there. Assemblies report their
    /// placed bodies, instances and mates; part studios their features,
    /// bodies and sketches.
    pub fn describe(&mut self, tab: TabId) -> Result<TabReport, String> {
        let t = self.tab(tab).ok_or_else(|| format!("no tab {}", tab.0))?;
        let (name, kind) = (t.name().to_string(), t.kind_name());
        let mut report = TabReport {
            tab,
            name,
            kind,
            ok: true,
            features: Vec::new(),
            bodies: Vec::new(),
            sketches: Vec::new(),
            variables: Default::default(),
            instances: Vec::new(),
            mates: Vec::new(),
            errors: Vec::new(),
        };
        if kind == "assembly" {
            let asm = self.assembly(tab).map_err(|e| e.to_string())?.clone();
            match self.regenerate_assembly(tab) {
                Ok(r) => {
                    report.bodies = r
                        .bodies
                        .iter()
                        .enumerate()
                        .map(|(i, b)| body_report(i, b))
                        .collect();
                    report.instances = asm
                        .instances
                        .iter()
                        .map(|i| InstanceReport {
                            id: i.id,
                            name: i.name.clone(),
                            studio: i.studio,
                            body: i.body,
                            fixed: i.fixed,
                            error: r.instance_errors.get(&i.id).cloned(),
                        })
                        .collect();
                    report.mates = asm
                        .mates
                        .iter()
                        .map(|m| MateReport {
                            id: m.id,
                            name: m.name.clone(),
                            kind: m.kind,
                            error: r.mate_errors.get(&m.id).cloned(),
                        })
                        .collect();
                    for (i, e) in &r.instance_errors {
                        report.errors.push(format!("instance {}: {e}", i.0));
                    }
                    for (m, e) in &r.mate_errors {
                        report.errors.push(format!("mate {}: {e}", m.0));
                    }
                }
                Err(e) => report.errors.push(e.to_string()),
            }
        } else {
            let features: Vec<crate::Feature> = self
                .studio(tab)
                .map_err(|e| e.to_string())?
                .features()
                .to_vec();
            match self.regenerate_studio(tab, None) {
                Ok(r) => {
                    report.variables = r.variables.clone();
                    for f in &features {
                        let status = r.statuses.iter().find(|s| s.id == f.id);
                        let params = serde_json::to_value(&f.kind).unwrap_or_default();
                        let kind = params
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let error = status.and_then(|s| s.error.clone());
                        if let Some(e) = &error {
                            report.errors.push(format!("{} ({}): {e}", f.name, f.id.0));
                        }
                        report.features.push(FeatureReport {
                            id: f.id,
                            name: f.name.clone(),
                            kind,
                            suppressed: f.suppressed,
                            error,
                            value: status.and_then(|s| s.value),
                            params,
                        });
                        if let FeatureKind::Sketch(sk) = &f.kind {
                            if let Some(res) = r.sketches.get(&f.id) {
                                let data = serde_json::to_value(&sk.sketch).unwrap_or_default();
                                report.sketches.push(SketchReport {
                                    feature: f.id,
                                    name: f.name.clone(),
                                    plane: res.plane,
                                    solve: res.solve.clone(),
                                    regions: res.profiles.len(),
                                    entities: data.get("entities").cloned().unwrap_or_default(),
                                    constraints: data
                                        .get("constraints")
                                        .cloned()
                                        .unwrap_or_default(),
                                });
                            }
                        }
                    }
                    report.bodies = r
                        .bodies
                        .iter()
                        .enumerate()
                        .map(|(i, b)| body_report(i, b))
                        .collect();
                }
                Err(e) => report.errors.push(e.to_string()),
            }
        }
        report.ok = report.errors.is_empty();
        Ok(report)
    }

    /// Applies ops one after another, stopping at the first that fails.
    /// Returns each op's result; the error names the op that failed.
    pub fn apply_all(&mut self, ops: Vec<DocOp>) -> Result<Vec<crate::DocOpResult>, String> {
        let mut out = Vec::with_capacity(ops.len());
        for (i, op) in ops.into_iter().enumerate() {
            match self.apply_with_base(op, None) {
                Ok(r) => out.push(r),
                Err(e) => return Err(format!("op {i} failed: {e}")),
            }
        }
        Ok(out)
    }
}

/// Wraps a studio op for the tab, as scripts usually mean.
pub fn studio_op(tab: TabId, op: Op) -> DocOp {
    DocOp::Studio { tab, op }
}

/// Wraps a sketch op for a sketch feature of the tab.
pub fn sketch_op(tab: TabId, sketch: FeatureId, op: SketchOp) -> DocOp {
    DocOp::Studio {
        tab,
        op: Op::Sketch { id: sketch, op },
    }
}

/// Wraps an assembly op for the tab.
pub fn assembly_op(tab: TabId, op: AssemblyOp) -> DocOp {
    DocOp::Assembly { tab, op }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyOp, ExtrudeDirection, ExtrudeEnd, PlaneRef, ProfileSelection, StandardPlane};
    use ok_math::Vec2;

    #[test]
    fn a_report_names_faces_and_sketch_state() {
        let mut doc = Document::new("doc");
        let tab = doc.tabs[0].id;
        let results = doc
            .apply_all(vec![
                studio_op(
                    tab,
                    Op::AddSketch {
                        plane: PlaneRef::standard(StandardPlane::Top),
                        name: None,
                    },
                ),
                sketch_op(
                    tab,
                    FeatureId(1),
                    SketchOp::AddRectangle {
                        a: Vec2::new(0.0, 0.0),
                        b: Vec2::new(20.0, 10.0),
                    },
                ),
                studio_op(
                    tab,
                    Op::AddExtrude {
                        sketch: FeatureId(1),
                        depth: 5.0,
                        direction: ExtrudeDirection::Normal,
                        end: ExtrudeEnd::Blind,
                        profiles: ProfileSelection::All,
                        op: BodyOp::New,
                        name: None,
                    },
                ),
            ])
            .unwrap();
        assert_eq!(results.len(), 3);
        let report = doc.describe(tab).unwrap();
        assert!(report.ok, "{:?}", report.errors);
        assert_eq!(report.features.len(), 2);
        assert_eq!(report.features[1].kind, "extrude");
        assert_eq!(report.bodies.len(), 1);
        assert!((report.bodies[0].volume - 1000.0).abs() < 1e-9);
        assert_eq!(report.bodies[0].faces.len(), 6);
        let top = report.bodies[0]
            .faces
            .iter()
            .find(|f| f.normal.z > 0.9)
            .unwrap();
        assert_eq!(top.reference.feature, FeatureId(2));
        assert_eq!(top.reference.part, Some(0));
        assert!(top.reference.has_near(), "neighbours recorded");
        assert!((top.area - 200.0).abs() < 1e-9);
        assert_eq!(report.sketches.len(), 1);
        assert_eq!(report.sketches[0].regions, 1);
        assert!(report.sketches[0].entities.as_array().unwrap().len() >= 8);
        // A failing op names its index and leaves the earlier ones applied.
        let err = doc
            .apply_all(vec![
                studio_op(
                    tab,
                    Op::AddVariable {
                        name: "w".into(),
                        expression: "1".into(),
                    },
                ),
                studio_op(tab, Op::DeleteFeature { id: FeatureId(99) }),
            ])
            .unwrap_err();
        assert!(err.starts_with("op 1 failed"), "{err}");
        assert_eq!(doc.describe(tab).unwrap().features.len(), 3);
    }
}
