//! Regeneration timings on representative parts. Not a pass/fail test:
//! run with `scripts/bench.sh` (release build, `--ignored`) and read the
//! numbers. Each figure is the median of several runs.

use ok_math::Vec2;
use ok_model::{
    BodyOp, ExtrudeDirection, ExtrudeEnd, FaceRef, Op, PartStudio, PlaneRef, ProfileSelection,
    SketchOp, StandardPlane,
};
use std::time::Instant;

fn median_ms(mut run: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..5)
        .map(|_| {
            let t0 = Instant::now();
            run();
            t0.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

/// A 200 x 100 x 10 plate with 24 through holes, four bosses on top and a
/// 2 mm shell open at the bottom: the shape of a typical machined cover.
fn cover() -> PartStudio {
    let mut ps = PartStudio::new("cover");
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
            b: Vec2::new(200.0, 100.0),
        },
    })
    .unwrap();
    let plate = ps
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
    let top = FaceRef {
        feature: plate,
        local: 1,
        part: None,
    };
    let bosses = ps
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
    for (x, y) in [(40.0, 30.0), (160.0, 30.0), (40.0, 70.0), (160.0, 70.0)] {
        ps.apply(Op::Sketch {
            id: bosses,
            op: SketchOp::AddCircle {
                center: Vec2::new(x, y),
                radius: 12.0,
            },
        })
        .unwrap();
    }
    ps.apply(Op::AddExtrude {
        sketch: bosses,
        depth: 15.0,
        direction: ExtrudeDirection::Normal,
        end: ExtrudeEnd::Blind,
        profiles: ProfileSelection::All,
        op: BodyOp::Add,
        name: None,
    })
    .unwrap();
    let holes = ps
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
    for i in 0..8 {
        for j in 0..3 {
            ps.apply(Op::Sketch {
                id: holes,
                op: SketchOp::AddPoint {
                    pos: Vec2::new(15.0 + 24.0 * i as f64, 15.0 + 35.0 * j as f64),
                },
            })
            .unwrap();
        }
    }
    ps.apply(Op::AddHole {
        sketch: holes,
        diameter: 6.0,
        depth: 0.0,
        through_all: true,
        direction: ExtrudeDirection::Reverse,
        counterbore: None,
        name: None,
    })
    .unwrap();
    ps.apply(Op::AddShell {
        thickness: 2.0,
        faces: vec![FaceRef {
            feature: plate,
            local: 0,
            part: None,
        }],
        name: None,
    })
    .unwrap();
    ps
}

#[test]
#[ignore]
fn regeneration_timings() {
    struct Report;
    impl Drop for Report {
        fn drop(&mut self) {
            ok_brep::report_times();
        }
    }
    let _report = Report;

    let mut demo = PartStudio::demo();
    let cold = median_ms(|| {
        demo.clear_cache();
        demo.regenerate();
    });
    let warm = median_ms(|| {
        demo.regenerate();
    });
    println!("demo plate: cold {cold:.1} ms, cached {warm:.2} ms");

    let mut cover = cover();
    let r = cover.regenerate();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{errors:?}");
    let faces: usize = r.bodies.iter().map(|b| b.solid.faces.len()).sum();
    let cold = median_ms(|| {
        cover.clear_cache();
        cover.regenerate();
    });
    println!("cover ({faces} faces): cold {cold:.1} ms");
    // Editing the last feature only re-runs it.
    let shell = cover.features().last().unwrap().id;
    let mut t = 2.0;
    let last = median_ms(|| {
        t = if t == 2.0 { 2.5 } else { 2.0 };
        cover
            .apply(Op::SetShell {
                id: shell,
                thickness: Some(t),
                faces: None,
            })
            .unwrap();
        cover.regenerate();
    });
    println!("cover: edit last feature {last:.1} ms");
}
