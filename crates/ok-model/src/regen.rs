use crate::{BodyOp, ExtrudeDirection, FeatureId, FeatureKind, PartStudio, ProfileSelection};
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
    /// Display edges of `solid` (between distinct surfaces).
    pub edges: Vec<[Vec3; 2]>,
}

impl Body {
    fn new(name: String, source: FeatureId, solid: Solid) -> Body {
        let mesh = ok_brep::tessellate(&solid);
        let edges = ok_brep::display_edges(&solid);
        Body {
            name,
            source,
            solid,
            mesh,
            edges,
        }
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

impl PartStudio {
    /// Evaluates every feature in order. Sketches are solved in place so the
    /// document always stores solved geometry.
    pub fn regenerate(&mut self) -> RegenResult {
        let opts = ProfileOptions::default();
        let mut result = RegenResult::default();
        let ids: Vec<FeatureId> = self.features.iter().map(|f| f.id).collect();
        for id in ids {
            let pos = self.position(id).expect("feature present");
            if self.features[pos].suppressed {
                result.statuses.push(FeatureStatus { id, error: None });
                continue;
            }
            let error = match &mut self.features[pos].kind {
                FeatureKind::Sketch(sf) => {
                    let plane = sf.plane.plane();
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
            };
            result.statuses.push(FeatureStatus { id, error });
        }
        result
    }

    fn regen_extrude(
        result: &mut RegenResult,
        id: FeatureId,
        ef: &crate::ExtrudeFeature,
        pos: usize,
        features: &[crate::Feature],
    ) -> Option<String> {
        let sketch_pos = features.iter().position(|f| f.id == ef.sketch);
        match sketch_pos {
            None => return Some(format!("sketch {:?} no longer exists", ef.sketch)),
            Some(sp) if sp >= pos => return Some("extrude must come after its sketch".into()),
            Some(sp) if features[sp].suppressed => {
                return Some(format!("sketch '{}' is suppressed", features[sp].name))
            }
            _ => {}
        }
        let Some(sr) = result.sketches.get(&ef.sketch) else {
            return Some("sketch did not regenerate".into());
        };
        if sr.profiles.is_empty() {
            return Some("sketch has no closed regions to extrude".into());
        }
        let selected: Vec<&Profile> = match &ef.profiles {
            ProfileSelection::All => sr.profiles.iter().collect(),
            ProfileSelection::Largest => sr.profiles.iter().take(1).collect(),
            ProfileSelection::Indices { indices } => {
                let mut v = Vec::new();
                for &i in indices {
                    match sr.profiles.get(i) {
                        Some(p) => v.push(p),
                        None => {
                            return Some(format!(
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
            return Some("no regions selected".into());
        }
        if !(ef.depth.is_finite() && ef.depth.abs() > ok_math::tol::LINEAR) {
            return Some("depth must be non-zero".into());
        }
        let (start, end) = match ef.direction {
            ExtrudeDirection::Normal => (0.0, ef.depth),
            ExtrudeDirection::Reverse => (0.0, -ef.depth),
            ExtrudeDirection::Symmetric => (-ef.depth / 2.0, ef.depth / 2.0),
        };

        // Build the tool volume: the union of the selected regions.
        let mut tool = Solid::default();
        for p in selected {
            let part = match ok_brep::extrude(p, &sr.plane, start, end, id.0) {
                Ok(s) => s,
                Err(e) => return Some(e.to_string()),
            };
            tool = match boolean(&tool, &part, BoolOp::Union) {
                Ok(s) => s,
                Err(e) => return Some(format!("could not combine regions: {e}")),
            };
        }
        let tool_bounds = tool.bounds()?;

        // Bodies the tool touches (bounding boxes overlap).
        let touched: Vec<usize> = result
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.bounds().is_some_and(|bb| boxes_touch(bb, tool_bounds)))
            .map(|(i, _)| i)
            .collect();

        match ef.op {
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
                let op = if ef.op == BodyOp::Remove {
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
    use crate::{Op, PlaneSpec, SketchOp, StandardPlane};
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
                plane: PlaneSpec::standard(StandardPlane::Front),
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
                plane: PlaneSpec::standard(StandardPlane::Top),
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
            .apply_json(r#"{"type":"add_sketch","plane":{"base":"top"},"name":null}"#)
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
                plane: PlaneSpec::standard(StandardPlane::Top),
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
            profiles: ProfileSelection::All,
            op: BodyOp::New,
            name: None,
        })
        .unwrap();
        let s2 = ps
            .apply(Op::AddSketch {
                plane: PlaneSpec::standard(StandardPlane::Top),
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
    fn remove_touching_nothing_is_an_error() {
        let mut ps = PartStudio::new("t");
        let s = ps
            .apply(Op::AddSketch {
                plane: PlaneSpec::standard(StandardPlane::Top),
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
