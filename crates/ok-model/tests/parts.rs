//! A corpus of realistic parts built through ops, as a regression net for
//! the kernel: every part must regenerate without feature errors, every
//! body must validate as closed, and volumes must land where hand
//! calculation says.

use ok_math::{Vec2, Vec3};
use ok_model::{
    Axis, BlendKind, BodyOp, BooleanOp, CopyOp, EdgeRef, ExtrudeDirection, ExtrudeEnd, FaceRef,
    FeatureId, Op, PartStudio, PatternKind, PlaneRef, ProfileSelection, RegenResult, RevolveAxis,
    SketchOp, StandardPlane,
};
use ok_sketch::EntityId;
use std::f64::consts::PI;

/// Builder around a part studio that panics with context on any failure.
struct Part {
    ps: PartStudio,
}

impl Part {
    fn new() -> Part {
        Part {
            ps: PartStudio::new("part"),
        }
    }

    fn op(&mut self, op: Op) -> FeatureId {
        let r = self
            .ps
            .apply(op.clone())
            .unwrap_or_else(|e| panic!("{op:?}: {e}"));
        r.feature.unwrap_or(FeatureId(0))
    }

    fn sketch(&mut self, plane: PlaneRef) -> FeatureId {
        self.op(Op::AddSketch { plane, name: None })
    }

    fn sketch_on(&mut self, face: FaceRef) -> FeatureId {
        self.sketch(PlaneRef::Face { face, offset: 0.0 })
    }

    fn draw(&mut self, sketch: FeatureId, op: SketchOp) -> Vec<EntityId> {
        self.ps
            .apply(Op::Sketch { id: sketch, op })
            .unwrap()
            .entities
    }

    fn rect(&mut self, sketch: FeatureId, a: (f64, f64), b: (f64, f64)) {
        self.draw(
            sketch,
            SketchOp::AddRectangle {
                a: Vec2::new(a.0, a.1),
                b: Vec2::new(b.0, b.1),
            },
        );
    }

    fn circle(&mut self, sketch: FeatureId, c: (f64, f64), r: f64) {
        self.draw(
            sketch,
            SketchOp::AddCircle {
                center: Vec2::new(c.0, c.1),
                radius: r,
            },
        );
    }

    fn point(&mut self, sketch: FeatureId, p: (f64, f64)) {
        self.draw(
            sketch,
            SketchOp::AddPoint {
                pos: Vec2::new(p.0, p.1),
            },
        );
    }

    fn polygon(&mut self, sketch: FeatureId, pts: &[(f64, f64)]) {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.draw(
                sketch,
                SketchOp::AddLine {
                    a: Vec2::new(a.0, a.1),
                    b: Vec2::new(b.0, b.1),
                },
            );
        }
    }

    fn extrude(&mut self, sketch: FeatureId, depth: f64, op: BodyOp) -> FeatureId {
        self.op(Op::AddExtrude {
            sketch,
            depth,
            direction: ExtrudeDirection::Normal,
            end: ExtrudeEnd::Blind,
            profiles: ProfileSelection::All,
            op,
            name: None,
        })
    }

    fn extrude_dir(
        &mut self,
        sketch: FeatureId,
        depth: f64,
        direction: ExtrudeDirection,
        end: ExtrudeEnd,
        op: BodyOp,
    ) -> FeatureId {
        self.op(Op::AddExtrude {
            sketch,
            depth,
            direction,
            end,
            profiles: ProfileSelection::All,
            op,
            name: None,
        })
    }

    fn hole(
        &mut self,
        sketch: FeatureId,
        diameter: f64,
        counterbore: Option<ok_model::Counterbore>,
    ) {
        self.op(Op::AddHole {
            sketch,
            diameter,
            depth: 0.0,
            through_all: true,
            direction: ExtrudeDirection::Reverse,
            counterbore,
            name: None,
        });
    }

    fn regen(&mut self) -> RegenResult {
        let r = self.ps.regenerate();
        let errors: Vec<_> = r.errors().collect();
        assert!(errors.is_empty(), "feature errors: {errors:?}");
        for b in &r.bodies {
            b.solid
                .validate()
                .unwrap_or_else(|e| panic!("body {} invalid: {e}", b.name));
        }
        r
    }

    fn volume(&mut self) -> f64 {
        self.regen().bodies.iter().map(|b| b.solid.volume()).sum()
    }

    /// The reference of the first face matching `pred(normal, is_cylinder, centroid)`.
    fn face(&mut self, pred: impl Fn(Vec3, bool, Vec3) -> bool) -> FaceRef {
        let r = self.regen();
        for b in &r.bodies {
            for f in &b.solid.faces {
                let cyl = matches!(
                    b.solid.surfaces[f.surface],
                    ok_brep::Surface::Cylinder { .. }
                );
                let centroid = f.loops[0]
                    .iter()
                    .fold(Vec3::ZERO, |acc, &v| acc + b.solid.vertices[v as usize])
                    / f.loops[0].len() as f64;
                if pred(f.plane.normal, cyl, centroid) {
                    return FaceRef {
                        feature: FeatureId(f.origin.feature),
                        local: f.origin.local,
                        part: None,
                    };
                }
            }
        }
        panic!("no face matches");
    }

    fn blend(&mut self, kind: BlendKind, edges: Vec<EdgeRef>, size: f64) -> FeatureId {
        self.op(Op::AddBlend {
            kind,
            edges,
            size,
            name: None,
        })
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1.0)
}

#[test]
fn l_bracket_with_holes_fillet_and_chamfer() {
    let mut p = Part::new();
    // Base plate 60 x 40 x 6, wall 60 x 6 x 30 along the back edge.
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (60.0, 40.0));
    let base = p.extrude(s, 6.0, BodyOp::New);
    let top = FaceRef {
        feature: base,
        local: 1,
        part: None,
    };
    let s2 = p.sketch_on(top);
    p.rect(s2, (0.0, 34.0), (60.0, 40.0));
    p.extrude(s2, 30.0, BodyOp::Add);
    assert!(close(
        p.volume(),
        60.0 * 40.0 * 6.0 + 60.0 * 6.0 * 30.0,
        1e-9
    ));
    // Two through holes in the base.
    let s3 = p.sketch_on(top);
    p.point(s3, (15.0, 15.0));
    p.point(s3, (45.0, 15.0));
    p.hole(s3, 6.0, None);
    let v = p.volume();
    let holes = 2.0 * PI * 9.0 * 6.0;
    assert!(
        close(v, 60.0 * 40.0 * 6.0 + 60.0 * 6.0 * 30.0 - holes, 3e-3),
        "{v}"
    );
    // Fillet the inner corner (base top meets wall front) and chamfer the wall top edges.
    let wall_front = p.face(|n, cyl, c| !cyl && (n.y + 1.0).abs() < 1e-9 && c.z > 6.0);
    let base_top = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && c.z < 7.0 && c.y < 34.0);
    p.blend(
        BlendKind::Fillet,
        vec![EdgeRef {
            a: base_top,
            b: wall_front,
        }],
        3.0,
    );
    let v2 = p.volume();
    // A concave fillet adds material: (1 - π/4) r² per unit length.
    let added = (1.0 - PI / 4.0) * 9.0 * 60.0;
    assert!(close(v2, v + added, 2e-2), "{v2} vs {}", v + added);
    let wall_top = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && c.z > 30.0);
    let wall_back = p.face(|n, cyl, _| !cyl && (n.y - 1.0).abs() < 1e-9);
    p.blend(
        BlendKind::Chamfer,
        vec![EdgeRef {
            a: wall_top,
            b: wall_back,
        }],
        2.0,
    );
    let v3 = p.volume();
    assert!(
        close(v3, v2 - 2.0 * 60.0, 2e-2),
        "{v3} vs {}",
        v2 - 2.0 * 60.0
    );
}

#[test]
fn flanged_bushing_by_revolve_with_bolt_holes() {
    let mut p = Part::new();
    // Half profile on the Front plane, revolved about the sketch Y axis:
    // bore r=5, body r=10 for 30 tall, flange r=20 by 5 thick at the bottom.
    let s = p.sketch(PlaneRef::standard(StandardPlane::Front));
    p.polygon(
        s,
        &[
            (5.0, 0.0),
            (20.0, 0.0),
            (20.0, 5.0),
            (10.0, 5.0),
            (10.0, 30.0),
            (5.0, 30.0),
        ],
    );
    p.op(Op::AddRevolve {
        sketch: s,
        axis: RevolveAxis::YAxis,
        angle: 360.0,
        profiles: ProfileSelection::All,
        op: BodyOp::New,
        name: None,
    });
    let v = p.volume();
    let expected = PI * (400.0 - 25.0) * 5.0 + PI * (100.0 - 25.0) * 25.0;
    assert!(close(v, expected, 5e-3), "{v} vs {expected}");
    // Six bolt holes through the flange, sketched on its top face.
    // The Front sketch's Y axis is world Z, so the flange top is the
    // annular face at z = 5 facing +Z.
    let flange_top =
        p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && (c.z - 5.0).abs() < 1e-6);
    let s2 = p.sketch_on(flange_top);
    for k in 0..6 {
        let a = k as f64 * PI / 3.0;
        p.point(s2, (15.0 * a.cos(), 15.0 * a.sin()));
    }
    p.hole(s2, 3.0, None);
    let v2 = p.volume();
    assert!(close(v2, expected - 6.0 * PI * 2.25 * 5.0, 5e-3), "{v2}");
}

#[test]
fn pocketed_box_with_rounded_corners_and_counterbored_holes() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (80.0, 50.0));
    let body = p.extrude(s, 20.0, BodyOp::New);
    // Fillet all four vertical edges.
    let sides: Vec<FaceRef> = (2..6)
        .map(|local| FaceRef {
            feature: body,
            local,
            part: None,
        })
        .collect();
    let edges: Vec<EdgeRef> = (0..4)
        .map(|i| EdgeRef {
            a: sides[i],
            b: sides[(i + 1) % 4],
        })
        .collect();
    p.blend(BlendKind::Fillet, edges, 8.0);
    let v = p.volume();
    let expected = 80.0 * 50.0 * 20.0 - 4.0 * (1.0 - PI / 4.0) * 64.0 * 20.0;
    assert!(close(v, expected, 2e-2), "{v} vs {expected}");
    // Pocket from the top, 5 mm walls, 15 deep.
    let top = FaceRef {
        feature: body,
        local: 1,
        part: None,
    };
    let s2 = p.sketch_on(top);
    p.rect(s2, (5.0, 5.0), (75.0, 45.0));
    p.extrude_dir(
        s2,
        15.0,
        ExtrudeDirection::Reverse,
        ExtrudeEnd::Blind,
        BodyOp::Remove,
    );
    let v2 = p.volume();
    assert!(close(v2, expected - 70.0 * 40.0 * 15.0, 2e-2), "{v2}");
    // Counterbored mounting holes in the floor of the pocket.
    let floor = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && (c.z - 5.0).abs() < 1e-6);
    let s3 = p.sketch_on(floor);
    p.point(s3, (12.0, 12.0));
    p.point(s3, (68.0, 38.0));
    p.hole(
        s3,
        4.0,
        Some(ok_model::Counterbore {
            diameter: 8.0,
            depth: 2.0,
        }),
    );
    let v3 = p.volume();
    let removed = 2.0 * (PI * 4.0 * 5.0 + PI * 16.0 * 2.0 - PI * 4.0 * 2.0);
    assert!(close(v3, v2 - removed, 2e-2), "{v3} vs {}", v2 - removed);
}

#[test]
fn boss_on_an_oblique_face_with_a_filleted_rim() {
    let mut p = Part::new();
    // A wedge: right triangle profile extruded, giving one oblique face.
    let s = p.sketch(PlaneRef::standard(StandardPlane::Front));
    p.polygon(s, &[(0.0, 0.0), (40.0, 0.0), (0.0, 30.0)]);
    p.extrude(s, 30.0, BodyOp::New);
    let v = p.volume();
    assert!(close(v, 0.5 * 40.0 * 30.0 * 30.0, 1e-9));
    // The hypotenuse face: the one whose normal has both x and z components.
    let slope = p.face(|n, cyl, _| !cyl && n.x.abs() > 0.3 && n.z.abs() > 0.3);
    let s2 = p.sketch_on(slope);
    p.circle(s2, (0.0, 0.0), 6.0);
    p.extrude(s2, 8.0, BodyOp::Add);
    let v2 = p.volume();
    assert!(close(v2, v + PI * 36.0 * 8.0, 5e-3), "{v2}");
    // Fillet where the boss meets the slope (a concave rim on an oblique plane).
    let boss_wall = p.face(|_, cyl, _| cyl);
    p.blend(
        BlendKind::Fillet,
        vec![EdgeRef {
            a: slope,
            b: boss_wall,
        }],
        1.5,
    );
    let v3 = p.volume();
    assert!(v3 > v2 && v3 < v2 + 2.0 * PI * 7.5 * 2.25, "{v3} vs {v2}");
}

#[test]
fn pulley_with_keyway_lightening_holes_and_mirror() {
    let mut p = Part::new();
    // V-groove pulley profile revolved about Y: outer r 40, bore r 6.
    let s = p.sketch(PlaneRef::standard(StandardPlane::Front));
    p.polygon(
        s,
        &[
            (6.0, 0.0),
            (40.0, 0.0),
            (40.0, 4.0),
            (34.0, 10.0),
            (40.0, 16.0),
            (40.0, 20.0),
            (6.0, 20.0),
        ],
    );
    p.op(Op::AddRevolve {
        sketch: s,
        axis: RevolveAxis::YAxis,
        angle: 360.0,
        profiles: ProfileSelection::All,
        op: BodyOp::New,
        name: None,
    });
    let v = p.volume();
    assert!(v > 0.0);
    // Keyway: a slot along the bore, cut through everything.
    let s2 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s2, (4.0, -2.0), (9.0, 2.0));
    p.extrude_dir(
        s2,
        100.0,
        ExtrudeDirection::Symmetric,
        ExtrudeEnd::ThroughAll,
        BodyOp::Remove,
    );
    let v2 = p.volume();
    assert!(v2 < v && v2 > v - 5.0 * 4.0 * 20.0, "{v2} vs {v}");
    // Three lightening holes through the web, then mirror the part across the Top plane.
    let s3 = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 20.0,
    });
    for k in 0..3 {
        let a = k as f64 * 2.0 * PI / 3.0;
        p.point(s3, (22.0 * a.cos(), 22.0 * a.sin()));
    }
    p.hole(s3, 8.0, None);
    let v3 = p.volume();
    assert!(v3 < v2, "{v3} vs {v2}");
    p.op(Op::AddMirror {
        plane: PlaneRef::standard(StandardPlane::Top),
        op: CopyOp::Add,
        features: vec![],
        bodies: vec![],
        name: None,
    });
    let v4 = p.volume();
    assert!(close(v4, 2.0 * v3, 1e-6), "{v4} vs {}", 2.0 * v3);
}

#[test]
fn grazing_cuts_tangent_bosses_and_coincident_cylinders() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (20.0, 20.0));
    let block = p.extrude(s, 10.0, BodyOp::New);
    // A cut whose edge passes exactly through the block's vertical edge.
    let s2 = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 10.0,
    });
    p.polygon(s2, &[(20.0, 20.0), (30.0, 20.0), (20.0, 30.0)]);
    p.extrude_dir(
        s2,
        4.0,
        ExtrudeDirection::Reverse,
        ExtrudeEnd::Blind,
        BodyOp::Remove,
    );
    assert!(close(p.volume(), 4000.0, 1e-9));
    // A boss tangent to a side of the block, unioned.
    let s3 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.circle(s3, (25.0, 10.0), 5.0);
    p.extrude(s3, 10.0, BodyOp::Add);
    let v = p.volume();
    assert!(close(v, 4000.0 + PI * 25.0 * 10.0, 5e-3), "{v}");
    // A second cylinder on the same axis and radius stacked on the first:
    // coincident cylinder surfaces along the seam.
    let boss_top = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && c.x > 20.0);
    let s4 = p.sketch_on(boss_top);
    p.circle(s4, (0.0, 0.0), 5.0);
    p.extrude(s4, 6.0, BodyOp::Add);
    let v2 = p.volume();
    assert!(close(v2, v + PI * 25.0 * 6.0, 5e-3), "{v2}");
    // A cut exactly the size of the top face, one unit deep (coplanar all round).
    let s5 = p.sketch_on(FaceRef {
        feature: block,
        local: 1,
        part: None,
    });
    p.rect(s5, (0.0, 0.0), (20.0, 20.0));
    p.extrude_dir(
        s5,
        1.0,
        ExtrudeDirection::Reverse,
        ExtrudeEnd::Blind,
        BodyOp::Remove,
    );
    let v3 = p.volume();
    assert!(close(v3, v2 - 400.0, 5e-3), "{v3} vs {}", v2 - 400.0);
}

#[test]
fn sweep_tube_union_block_and_loft_cut_by_hole() {
    let mut p = Part::new();
    // Path: an L on the Top plane; profile: a ring on the Right plane.
    let path = p.sketch(PlaneRef::standard(StandardPlane::Top));
    let r1 = p.draw(
        path,
        SketchOp::AddLine {
            a: Vec2::ZERO,
            b: Vec2::new(30.0, 0.0),
        },
    );
    let r2 = p.draw(
        path,
        SketchOp::AddLine {
            a: Vec2::new(30.0, 0.0),
            b: Vec2::new(30.0, 25.0),
        },
    );
    p.draw(
        path,
        SketchOp::AddConstraint {
            constraint: ok_sketch::Constraint::Coincident { a: r1[2], b: r2[1] },
        },
    );
    let prof = p.sketch(PlaneRef::standard(StandardPlane::Right));
    p.circle(prof, (0.0, 0.0), 4.0);
    p.circle(prof, (0.0, 0.0), 3.0);
    // Regions come area-descending: the inner disc first, the annulus second.
    p.op(Op::AddSweep {
        sketch: prof,
        path,
        profiles: ProfileSelection::Indices { indices: vec![1] },
        op: BodyOp::New,
        name: None,
    });
    let v = p.volume();
    let ring = PI * (16.0 - 9.0);
    assert!(close(v, ring * 55.0, 2e-2), "{v} vs {}", ring * 55.0);
    // A block around the corner of the tube, unioned.
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (25.0, -5.0), (35.0, 5.0));
    p.extrude_dir(
        s,
        12.0,
        ExtrudeDirection::Symmetric,
        ExtrudeEnd::Blind,
        BodyOp::Add,
    );
    let v2 = p.volume();
    assert!(v2 > v && v2 < v + 1200.0, "{v2} vs {v}");
    // A lofted frustum elsewhere, then a hole through it.
    let a = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 40.0,
    });
    p.rect(a, (-10.0, -10.0), (10.0, 10.0));
    let b = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 55.0,
    });
    p.circle(b, (0.0, 0.0), 5.0);
    p.op(Op::AddLoft {
        sketch: a,
        sketch_b: b,
        op: BodyOp::New,
        name: None,
    });
    let v3 = p.volume();
    assert!(v3 > v2 + 1000.0 && v3 < v2 + 5000.0, "{v3} vs {v2}");
    let h = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 55.0,
    });
    p.point(h, (0.0, 0.0));
    p.hole(h, 4.0, None);
    let v4 = p.volume();
    assert!(
        close(v4, v3 - PI * 4.0 * 15.0, 2e-2),
        "{v4} vs {}",
        v3 - PI * 4.0 * 15.0
    );
}

#[test]
fn linear_pattern_of_a_ribbed_plate_then_fillet_after_pattern() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (10.0, 30.0));
    let plate = p.extrude(s, 4.0, BodyOp::New);
    let s2 = p.sketch_on(FaceRef {
        feature: plate,
        local: 1,
        part: None,
    });
    p.rect(s2, (4.0, 0.0), (6.0, 30.0));
    p.extrude(s2, 10.0, BodyOp::Add);
    let v = p.volume();
    assert!(close(v, 10.0 * 30.0 * 4.0 + 2.0 * 30.0 * 10.0, 1e-9));
    p.op(Op::AddPattern {
        kind: PatternKind::Linear {
            axis: Axis::X,
            spacing: 10.0,
        },
        count: 4,
        op: CopyOp::Add,
        features: vec![],
        bodies: vec![],
        name: None,
    });
    let v2 = p.volume();
    assert!(close(v2, 4.0 * v, 1e-6), "{v2} vs {}", 4.0 * v);
    // The copies share faces edge to edge and merge into one plate. Fillet
    // one rib's top edge.
    let rib_top = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && c.z > 13.0 && c.x < 10.0);
    let rib_side =
        p.face(|n, cyl, c| !cyl && (n.x + 1.0).abs() < 1e-9 && c.z > 4.0 && c.x < 5.0 && c.x > 3.0);
    p.blend(
        BlendKind::Fillet,
        vec![EdgeRef {
            a: rib_top,
            b: rib_side,
        }],
        0.5,
    );
    let v3 = p.volume();
    assert!(v3 < v2 && v3 > v2 - 30.0, "{v3} vs {v2}");
}

/// Two separate bodies combined by boolean features: a block with a
/// cylinder subtracted (tool consumed), then the same cylinder kept as a
/// tool and intersected, then everything unioned back into one body.
#[test]
fn boolean_feature_between_bodies() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (40.0, 30.0));
    let block = p.extrude(s, 10.0, BodyOp::New);
    let s2 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.circle(s2, (20.0, 15.0), 5.0);
    let pin = p.extrude(s2, 30.0, BodyOp::New);
    let r = p.regen();
    assert_eq!(r.bodies.len(), 2);
    let v_block = 40.0 * 30.0 * 10.0;
    let v_pin_in_block = r.bodies[1].solid.volume() / 3.0;

    // Subtract: one body left, the pin's slice through the block removed.
    let cut = p.op(Op::AddBoolean {
        op: BooleanOp::Subtract,
        targets: vec![block],
        tools: vec![pin],
        keep_tools: false,
        name: None,
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1, "tool consumed");
    assert!(close(
        r.bodies[0].solid.volume(),
        v_block - v_pin_in_block,
        1e-6
    ));
    let candidates = r
        .statuses
        .iter()
        .find(|s| s.id == cut)
        .and_then(|s| s.candidates.clone())
        .expect("boolean status lists candidate bodies");
    assert_eq!(candidates.len(), 2);

    // Keep the tool: two bodies, the pin untouched.
    p.op(Op::SetBoolean {
        id: cut,
        op: None,
        targets: None,
        tools: None,
        keep_tools: Some(true),
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 2);
    let kept = r.bodies.iter().find(|b| b.source == pin).expect("pin kept");
    assert!(close(kept.solid.volume(), 3.0 * v_pin_in_block, 1e-6));

    // Intersect instead: only the pin's slice through the block remains.
    p.op(Op::SetBoolean {
        id: cut,
        op: Some(BooleanOp::Intersect),
        targets: None,
        tools: None,
        keep_tools: Some(false),
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1);
    assert!(close(r.bodies[0].solid.volume(), v_pin_in_block, 1e-6));

    // Union of both: block plus the parts of the pin outside it.
    p.op(Op::SetBoolean {
        id: cut,
        op: Some(BooleanOp::Union),
        targets: None,
        tools: None,
        keep_tools: None,
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1);
    assert!(close(
        r.bodies[0].solid.volume(),
        v_block + 2.0 * v_pin_in_block,
        1e-6
    ));

    // A boolean whose target no longer exists reports an error, not a panic.
    p.ps.apply(Op::SetBoolean {
        id: cut,
        op: None,
        targets: Some(vec![FeatureId(999)]),
        tools: None,
        keep_tools: None,
    })
    .unwrap();
    let r = p.ps.regenerate();
    assert!(r.errors().next().is_some());
}

/// A feature pattern replays a boss and a hole (with their own add / remove
/// operations) instead of copying whole bodies, so the plate stays one
/// body and grows by exactly the copied features.
#[test]
fn feature_pattern_and_mirror_replay_tools() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (100.0, 30.0));
    p.extrude(s, 5.0, BodyOp::New);
    let plate = p.volume();
    let s2 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.circle(s2, (10.0, 15.0), 4.0);
    let boss = p.extrude_dir(
        s2,
        10.0,
        ExtrudeDirection::Normal,
        ExtrudeEnd::Blind,
        BodyOp::Add,
    );
    let with_boss = p.volume();
    let boss_v = with_boss - plate; // 5 mm of it stands above the plate
    let s3 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.point(s3, (10.0, 15.0));
    let hole = p.op(Op::AddHole {
        sketch: s3,
        diameter: 2.0,
        depth: 0.0,
        through_all: true,
        direction: ExtrudeDirection::Normal,
        counterbore: None,
        name: None,
    });
    let with_hole = p.volume();
    let hole_v = with_boss - with_hole;
    assert!(hole_v > 0.0);

    // Pattern the boss and hole along X: four in total.
    let pat = p.op(Op::AddPattern {
        kind: PatternKind::Linear {
            axis: Axis::X,
            spacing: 25.0,
        },
        count: 4,
        op: CopyOp::Add,
        features: vec![boss, hole],
        bodies: vec![],
        name: None,
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1, "copies merge into the plate");
    let v = r.bodies[0].solid.volume();
    let expected = with_hole + 3.0 * (boss_v - hole_v);
    assert!(close(v, expected, 1e-6), "{v} vs {expected}");

    // Mirror the same features across x = 52.5: the original boss and hole
    // land at x = 95, still on the plate and clear of the pattern.
    p.op(Op::AddMirror {
        plane: PlaneRef::Standard {
            base: StandardPlane::Right,
            offset: 52.5,
        },
        op: CopyOp::Add,
        features: vec![boss, hole],
        bodies: vec![],
        name: None,
    });
    // The mirror names the original features only, so it adds one boss
    // and one hole mirrored, not the whole pattern.
    let v2 = p.volume();
    let expected2 = expected + boss_v - hole_v;
    assert!(close(v2, expected2, 1e-6), "{v2} vs {expected2}");

    // Naming a feature without a tool volume (the pattern itself) is an error.
    p.ps.apply(Op::AddPattern {
        kind: PatternKind::Linear {
            axis: Axis::Y,
            spacing: 10.0,
        },
        count: 2,
        op: CopyOp::Add,
        features: vec![pat],
        bodies: vec![],
        name: None,
    })
    .unwrap();
    let r = p.ps.regenerate();
    assert!(r.errors().any(|(_, e)| e.contains("no tool volume")));
}

/// An open-top enclosure: a box shelled to 2 mm with its top face open,
/// then a boss added inside and a hole through the floor.
#[test]
fn shelled_enclosure_with_a_boss_inside() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (60.0, 40.0));
    let box_id = p.extrude(s, 25.0, BodyOp::New);
    let top = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && (c.z - 25.0).abs() < 1e-6);
    p.op(Op::AddShell {
        thickness: 2.0,
        faces: vec![top],
        name: None,
    });
    let v = p.volume();
    let expected = 60.0 * 40.0 * 25.0 - 56.0 * 36.0 * 23.0;
    assert!(close(v, expected, 1e-9), "{v} vs {expected}");
    // The floor's inside face (z = 2, normal +Z) exists and is planar: sketch on it.
    let floor = p.face(|n, cyl, c| !cyl && (n.z - 1.0).abs() < 1e-9 && (c.z - 2.0).abs() < 1e-6);
    let s2 = p.sketch_on(floor);
    p.circle(s2, (30.0, 20.0), 5.0);
    p.extrude(s2, 10.0, BodyOp::Add);
    let v2 = p.volume();
    let boss = v2 - v;
    assert!(
        boss > 0.0 && close(boss, PI * 25.0 * 10.0, 5e-3),
        "boss {boss}"
    );
    // Closed shell of a separate block: two shells, one body.
    let s3 = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 40.0,
    });
    p.rect(s3, (0.0, 0.0), (10.0, 10.0));
    p.extrude(s3, 10.0, BodyOp::New);
    p.op(Op::AddShell {
        thickness: 1.0,
        faces: vec![],
        name: None,
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 2);
    let block = r.bodies.iter().find(|b| b.source != box_id).unwrap();
    assert!(close(block.solid.volume(), 1000.0 - 512.0, 1e-9));
    assert_eq!(block.solid.shells().len(), 2);
}

/// Direct edits on a bracket: the top face is pulled up, a side face is
/// pushed in, and the four walls get a draft about the base.
#[test]
fn move_face_and_draft_on_a_block() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (60.0, 40.0));
    let block = p.extrude(s, 20.0, BodyOp::New);
    let top = FaceRef {
        feature: block,
        local: 1,
        part: None,
    };
    let mv = p.op(Op::AddMoveFace {
        faces: vec![top],
        distance: 5.0,
        name: None,
    });
    assert!(close(p.volume(), 60.0 * 40.0 * 25.0, 1e-9));
    // Push the +X wall in by 10 instead.
    let east = p.face(|n, cyl, _| !cyl && (n.x - 1.0).abs() < 1e-9);
    p.op(Op::SetMoveFace {
        id: mv,
        faces: Some(vec![east]),
        distance: Some(-10.0),
    });
    assert!(close(p.volume(), 50.0 * 40.0 * 20.0, 1e-9));
    // Draft all four walls 8° about the base plane (pull +Z).
    let walls: Vec<FaceRef> = (2..6)
        .map(|local| FaceRef {
            feature: block,
            local,
            part: None,
        })
        .collect();
    p.op(Op::AddDraft {
        faces: walls,
        neutral: PlaneRef::standard(StandardPlane::Top),
        angle: 8.0,
        name: None,
    });
    let k = 8f64.to_radians().tan();
    let expected: f64 = (0..2000)
        .map(|i| {
            let z = (i as f64 + 0.5) / 100.0;
            (50.0 - 2.0 * k * z) * (40.0 - 2.0 * k * z) * 0.01
        })
        .sum();
    let v = p.volume();
    assert!(close(v, expected, 1e-4), "{v} vs {expected}");
    // The drafted walls are still four planar faces and the body is one shell.
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1);
    assert_eq!(r.bodies[0].solid.faces.len(), 6);
}

/// A mirror restricted to one of two bodies copies only that body.
#[test]
fn mirror_of_chosen_bodies_only() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (2.0, 0.0), (10.0, 10.0));
    let a = p.extrude(s, 5.0, BodyOp::New);
    let s2 = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s2, (2.0, 20.0), (10.0, 30.0));
    let b = p.extrude(s2, 5.0, BodyOp::New);
    assert_eq!(p.regen().bodies.len(), 2);
    let m = p.op(Op::AddMirror {
        plane: PlaneRef::standard(StandardPlane::Right),
        op: CopyOp::New,
        features: vec![],
        bodies: vec![a],
        name: None,
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 3);
    assert_eq!(r.bodies.iter().filter(|x| x.source == b).count(), 1);
    // Choosing both (or none) mirrors both.
    p.op(Op::SetMirror {
        id: m,
        plane: None,
        op: None,
        features: None,
        bodies: Some(vec![]),
    });
    assert_eq!(p.regen().bodies.len(), 4);
}

/// The twelve triangles of an axis-aligned box, wound outward.
fn box_mesh(w: f64, d: f64, h: f64) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let v = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(w, 0.0, 0.0),
        Vec3::new(w, d, 0.0),
        Vec3::new(0.0, d, 0.0),
        Vec3::new(0.0, 0.0, h),
        Vec3::new(w, 0.0, h),
        Vec3::new(w, d, h),
        Vec3::new(0.0, d, h),
    ];
    let quads: [[u32; 4]; 6] = [
        [0, 3, 2, 1], // bottom
        [4, 5, 6, 7], // top
        [0, 1, 5, 4], // front
        [1, 2, 6, 5], // right
        [2, 3, 7, 6], // back
        [3, 0, 4, 7], // left
    ];
    let t = quads
        .iter()
        .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
        .collect();
    (v, t)
}

#[test]
fn imported_mesh_becomes_a_body_with_merged_faces() {
    let mut p = Part::new();
    let (vertices, triangles) = box_mesh(10.0, 20.0, 5.0);
    let mesh = p.op(Op::AddMesh {
        vertices,
        triangles,
        name: Some("Imported".into()),
    });
    assert!((p.volume() - 1000.0).abs() < 1e-9);
    let r = p.regen();
    assert_eq!(r.bodies.len(), 1);
    // Coplanar triangles merge: a box is six faces again.
    assert_eq!(r.bodies[0].solid.faces.len(), 6);
    assert!(r.bodies[0]
        .solid
        .faces
        .iter()
        .all(|f| f.origin.feature == mesh.0));

    // The body is ordinary: a hole through it, then a fillet reference by faces.
    let top = p.face(|n, _, _| (n.z - 1.0).abs() < 1e-9);
    let s = p.sketch_on(top);
    p.circle(s, (5.0, 10.0), 2.0);
    p.op(Op::AddExtrude {
        sketch: s,
        profiles: ProfileSelection::All,
        depth: 5.0,
        direction: ExtrudeDirection::Reverse,
        end: ExtrudeEnd::Blind,
        op: BodyOp::Remove,
        name: None,
    });
    assert!((p.volume() - (1000.0 - PI * 4.0 * 5.0)).abs() < 0.5);
}

#[test]
fn open_and_inside_out_meshes_are_reported() {
    let mut p = Part::new();
    let (vertices, mut triangles) = box_mesh(10.0, 10.0, 10.0);
    triangles.pop();
    p.op(Op::AddMesh {
        vertices: vertices.clone(),
        triangles: triangles.clone(),
        name: None,
    });
    let r = p.ps.regenerate();
    assert!(
        r.errors().any(|(_, e)| e.contains("does not close")),
        "{:?}",
        r.errors().collect::<Vec<_>>()
    );
    assert!(r.bodies.is_empty());

    let mut p = Part::new();
    let (vertices, triangles) = box_mesh(10.0, 10.0, 10.0);
    let flipped: Vec<[u32; 3]> = triangles.iter().map(|t| [t[0], t[2], t[1]]).collect();
    p.op(Op::AddMesh {
        vertices,
        triangles: flipped,
        name: None,
    });
    let r = p.ps.regenerate();
    assert!(r.errors().any(|(_, e)| e.contains("inside out")));
}

#[test]
fn split_by_a_plane_makes_two_parts_and_follows_the_offset() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::Standard {
        base: StandardPlane::Top,
        offset: 0.0,
    });
    p.rect(s, (0.0, 0.0), (20.0, 10.0));
    p.op(Op::AddExtrude {
        sketch: s,
        profiles: ProfileSelection::All,
        depth: 5.0,
        direction: ExtrudeDirection::Normal,
        end: ExtrudeEnd::Blind,
        op: BodyOp::New,
        name: Some("Block".into()),
    });
    // Split at x = 6 (the Right plane, normal +X, offset 6).
    let split = p.op(Op::AddSplit {
        plane: PlaneRef::Standard {
            base: StandardPlane::Right,
            offset: 6.0,
        },
        bodies: Vec::new(),
        name: Some("Split".into()),
    });
    let r = p.regen();
    assert_eq!(r.bodies.len(), 2);
    let mut vols: Vec<f64> = r.bodies.iter().map(|b| b.solid.volume()).collect();
    vols.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (vols[0] - 300.0).abs() < 1e-9 && (vols[1] - 700.0).abs() < 1e-9,
        "{vols:?}"
    );
    // The first body keeps its name; the new part is the split's.
    assert_eq!(r.bodies[0].name, "Part 1");
    assert_eq!(r.bodies[1].source, split);
    // Moving the plane moves the cut; a plane past the block is reported.
    p.op(Op::SetSplit {
        id: split,
        plane: Some(PlaneRef::Standard {
            base: StandardPlane::Right,
            offset: 15.0,
        }),
        bodies: None,
    });
    let r = p.regen();
    let mut vols: Vec<f64> = r.bodies.iter().map(|b| b.solid.volume()).collect();
    vols.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (vols[0] - 250.0).abs() < 1e-9 && (vols[1] - 750.0).abs() < 1e-9,
        "{vols:?}"
    );
    p.op(Op::SetSplit {
        id: split,
        plane: Some(PlaneRef::Standard {
            base: StandardPlane::Right,
            offset: 40.0,
        }),
        bodies: None,
    });
    let r = p.ps.regenerate();
    assert!(r.errors().any(|(_, e)| e.contains("misses")));
    assert_eq!(r.bodies.len(), 1);
}

#[test]
fn sketch_on_an_angled_plane_extrudes_a_tilted_block() {
    use ok_model::Axis;
    let mut p = Part::new();
    // The Top plane turned 30° about X: its normal tilts from +Z towards -Y.
    let s = p.sketch(PlaneRef::Rotated {
        base: StandardPlane::Top,
        axis: Axis::X,
        angle: 30.0,
        offset: 0.0,
    });
    p.rect(s, (0.0, 0.0), (10.0, 10.0));
    p.op(Op::AddExtrude {
        sketch: s,
        profiles: ProfileSelection::All,
        depth: 4.0,
        direction: ExtrudeDirection::Normal,
        end: ExtrudeEnd::Blind,
        op: BodyOp::New,
        name: None,
    });
    let r = p.regen();
    let v = p.volume();
    assert!(
        (v - 400.0).abs() < 1e-3,
        "volume {v}, bodies {}",
        r.bodies.len()
    );
    let top = r.bodies[0]
        .solid
        .faces
        .iter()
        .find(|f| f.origin.local == 1)
        .unwrap();
    let n = top.plane.normal;
    let expect = Vec3::new(0.0, -(30f64.to_radians().sin()), 30f64.to_radians().cos());
    assert!(n.distance(expect) < 1e-6, "{n:?}");
    // The angle is bindable through the sketch's plane.angle field.
    let f = p.ps.feature(s).unwrap();
    assert!(f.kind.bindable_fields().iter().any(|x| x == "plane.angle"));
    assert_eq!(f.kind.field("plane.angle"), Some(30.0));
}

/// A slot across a block splits its top face in two; references name the
/// piece they mean, numbered by position, so a later move face acts on
/// that piece alone.
#[test]
fn split_faces_are_named_by_piece() {
    let mut p = Part::new();
    let s = p.sketch(PlaneRef::standard(StandardPlane::Top));
    p.rect(s, (0.0, 0.0), (30.0, 10.0));
    let block = p.extrude(s, 5.0, BodyOp::New);
    let top = p.face(|n, _, _| (n.z - 1.0).abs() < 1e-9);
    assert_eq!(top.part, None);
    let s2 = p.sketch_on(top);
    p.rect(s2, (10.0, -1.0), (12.0, 11.0));
    p.extrude_dir(
        s2,
        3.0,
        ExtrudeDirection::Reverse,
        ExtrudeEnd::Blind,
        BodyOp::Remove,
    );
    let base = p.volume();
    assert!(close(base, 1500.0 - 2.0 * 10.0 * 3.0, 1e-9));
    // Piece 0 is the left part (x < 10, 100 mm²), piece 1 the right (180 mm²).
    let piece = |part: u32| FaceRef {
        feature: block,
        local: 1,
        part: Some(part),
    };
    let mv = p.op(Op::AddMoveFace {
        faces: vec![piece(1)],
        distance: 2.0,
        name: None,
    });
    assert!(
        close(p.volume(), base + 2.0 * 180.0, 1e-6),
        "{}",
        p.volume()
    );
    p.op(Op::SetMoveFace {
        id: mv,
        faces: Some(vec![piece(0)]),
        distance: Some(2.0),
    });
    assert!(
        close(p.volume(), base + 2.0 * 100.0, 1e-6),
        "{}",
        p.volume()
    );
    // Without a piece number, piece 0 is meant.
    p.op(Op::SetMoveFace {
        id: mv,
        faces: Some(vec![FaceRef {
            feature: block,
            local: 1,
            part: None,
        }]),
        distance: Some(2.0),
    });
    assert!(
        close(p.volume(), base + 2.0 * 100.0, 1e-6),
        "{}",
        p.volume()
    );
}
