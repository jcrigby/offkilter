//! A solid written as STEP and read back closes again with the same
//! volume and faces.

use ok_brep::{boolean, extrude, BoolOp, Solid};
use ok_math::{Plane, Vec2, Vec3};
use ok_sketch::{ProfileOptions, Sketch};
use ok_step::{read_step, write_step};

fn plate_with_hole() -> Solid {
    let mut s = Sketch::new();
    s.add_rectangle(Vec2::ZERO, Vec2::new(40.0, 30.0));
    let plate = extrude(
        &s.profiles(&ProfileOptions::default()).remove(0),
        &Plane::XY,
        0.0,
        6.0,
        1,
    )
    .unwrap();
    let mut c = Sketch::new();
    c.add_circle(Vec2::new(20.0, 15.0), 5.0);
    let drill = extrude(
        &c.profiles(&ProfileOptions::default()).remove(0),
        &Plane::XY,
        -1.0,
        7.0,
        2,
    )
    .unwrap();
    boolean(&plate, &drill, BoolOp::Difference).unwrap()
}

/// The mesh as the kernel's mesh feature builds it: welded polygons,
/// coplanar triangles merged.
fn solid_of(body: &ok_step::StepBody) -> Solid {
    let mut polys = Vec::new();
    let mut surfaces = Vec::new();
    for (i, t) in body.triangles.iter().enumerate() {
        let pts: Vec<Vec3> = t.iter().map(|&k| body.vertices[k as usize]).collect();
        let Some(normal) = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized() else {
            continue;
        };
        let Some(x_axis) = (pts[1] - pts[0]).normalized() else {
            continue;
        };
        surfaces.push(ok_brep::Surface::Plane {
            normal,
            offset: normal.dot(pts[0]),
        });
        polys.push(ok_brep::Polygon {
            plane: Plane {
                origin: pts[0],
                x_axis,
                y_axis: normal.cross(x_axis),
                normal,
            },
            loops: vec![pts],
            surface: surfaces.len() - 1,
            origin: ok_brep::FaceOrigin {
                feature: 9,
                local: i as u32,
            },
        });
    }
    let mut solid = Solid::from_polygons(polys, surfaces).unwrap();
    solid.merge_coplanar_faces();
    solid
}

#[test]
fn a_plate_with_a_hole_survives_a_step_round_trip() {
    let original = plate_with_hole();
    let text = write_step(&[("Plate", &original)], "round trip");
    if let Ok(dir) = std::env::var("OK_STEP_DUMP") {
        std::fs::write(format!("{dir}/plate.step"), &text).unwrap();
    }
    let bodies = read_step(&text).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].name, "Plate");
    let back = solid_of(&bodies[0]);
    back.validate().unwrap();
    assert!((back.volume() - original.volume()).abs() < 1e-6);
    // Six planar faces (the top and bottom with a hole loop) and the
    // facets of the hole.
    assert_eq!(back.faces.len(), original.faces.len());
    assert_eq!(back.faces.iter().filter(|f| f.loops.len() == 2).count(), 2);
}

fn cylinder(r: f64, h: f64, id: u32) -> Solid {
    let mut s = Sketch::new();
    s.add_circle(Vec2::ZERO, r);
    extrude(
        &s.profiles(&ProfileOptions::default()).remove(0),
        &Plane::XY,
        0.0,
        h,
        id,
    )
    .unwrap()
}

fn count(text: &str, entity: &str) -> usize {
    text.matches(&format!("={entity}(")).count()
}

/// A cylinder cut obliquely and drilled across: the rim is an ellipse,
/// the bottom a circle, the drill meets the wall on two quartics, and
/// both cylinders go out as surfaces with seams; it all reads back.
#[test]
fn a_cut_and_drilled_cylinder_goes_out_with_exact_curves() {
    let body = cylinder(10.0, 30.0, 1);
    let a = 30f64.to_radians();
    let normal = Vec3::new(0.0, -a.sin(), a.cos());
    let plane = Plane::from_origin_normal(Vec3::new(0.0, 0.0, 20.0), normal).unwrap();
    let mut b = Sketch::new();
    b.add_rectangle(Vec2::new(-40.0, -40.0), Vec2::new(40.0, 40.0));
    let wedge = extrude(
        &b.profiles(&ProfileOptions::default()).remove(0),
        &plane,
        0.0,
        40.0,
        2,
    )
    .unwrap();
    let cut = boolean(&body, &wedge, BoolOp::Difference).unwrap();
    let mut d = Sketch::new();
    d.add_circle(Vec2::new(0.0, 10.0), 4.0);
    let yz = Plane {
        origin: Vec3::ZERO,
        x_axis: Vec3::Y,
        y_axis: Vec3::Z,
        normal: Vec3::X,
    };
    let drill = extrude(
        &d.profiles(&ProfileOptions::default()).remove(0),
        &yz,
        -20.0,
        20.0,
        3,
    )
    .unwrap();
    let original = boolean(&cut, &drill, BoolOp::Difference).unwrap();
    let text = write_step(&[("Stub", &original)], "exact");
    assert_eq!(
        count(&text, "CYLINDRICAL_SURFACE"),
        2,
        "the wall and the drill"
    );
    assert_eq!(count(&text, "ELLIPSE"), 1, "the oblique rim");
    assert_eq!(count(&text, "CIRCLE"), 1, "the bottom rim");
    assert_eq!(
        count(&text, "B_SPLINE_CURVE_WITH_KNOTS"),
        2,
        "where the drill meets the wall"
    );
    assert_eq!(count(&text, "PLANE"), 2, "the bottom and the cut");
    assert_eq!(count(&text, "ADVANCED_FACE"), 4);
    if let Ok(dir) = std::env::var("OK_STEP_DUMP") {
        std::fs::write(format!("{dir}/drilled.step"), &text).unwrap();
    }
    let bodies = read_step(&text).unwrap();
    let back = solid_of(&bodies[0]);
    back.validate().unwrap();
    let (v0, v1) = (original.volume(), back.volume());
    assert!(((v1 - v0) / v0).abs() < 0.01, "volume {v1} vs {v0}");
}

/// A profile revolved about y: a flat bottom out to radius 10, an
/// oblique wall (a cone) up to radius 6 at height 8, a quarter-round
/// (a torus) to the axis side, and a flat top.
fn turned_part() -> Solid {
    let mut s = Sketch::new();
    let (_, p0, p1) = s.add_line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
    let (_, p2, p3) = s.add_line(Vec2::new(10.0, 0.0), Vec2::new(6.0, 8.0));
    let (_, _, p4, p5) = s.add_arc(
        Vec2::new(2.0, 8.0),
        Vec2::new(6.0, 8.0),
        Vec2::new(2.0, 12.0),
    );
    let (_, p6, p7) = s.add_line(Vec2::new(2.0, 12.0), Vec2::new(0.0, 12.0));
    let (_, p8, p9) = s.add_line(Vec2::new(0.0, 12.0), Vec2::new(0.0, 0.0));
    for (a, b) in [(p1, p2), (p3, p4), (p5, p6), (p7, p8), (p9, p0)] {
        s.add_constraint(ok_sketch::Constraint::Coincident { a, b });
    }
    let profile = s.profiles(&ProfileOptions::default()).remove(0);
    ok_brep::revolve(
        &profile,
        &Plane::XY,
        Vec2::ZERO,
        Vec2::Y,
        std::f64::consts::TAU,
        5f64.to_radians(),
        1,
    )
    .unwrap()
}

#[test]
fn a_turned_part_goes_out_with_a_cone_and_a_torus() {
    let part = turned_part();
    let text = write_step(&[("Turned", &part)], "exact");
    assert_eq!(count(&text, "CONICAL_SURFACE"), 1);
    assert_eq!(count(&text, "TOROIDAL_SURFACE"), 1);
    assert_eq!(count(&text, "PLANE"), 2, "the flats");
    assert_eq!(count(&text, "ADVANCED_FACE"), 4);
    // Three rims plus the torus's meridian seam; the cone's seam is a line.
    assert_eq!(count(&text, "CIRCLE"), 4);
    assert_eq!(count(&text, "LINE"), 1);
    if let Ok(dir) = std::env::var("OK_STEP_DUMP") {
        std::fs::write(format!("{dir}/turned.step"), &text).unwrap();
    }
    let bodies = read_step(&text).unwrap();
    let back = solid_of(&bodies[0]);
    back.validate().unwrap();
    let (v0, v1) = (part.volume(), back.volume());
    assert!(((v1 - v0) / v0).abs() < 0.01, "volume {v1} vs {v0}");
}

#[test]
fn a_filleted_corner_goes_out_as_a_sphere_patch() {
    let mut s = Sketch::new();
    s.add_rectangle(Vec2::ZERO, Vec2::new(20.0, 20.0));
    let block = extrude(
        &s.profiles(&ProfileOptions::default()).remove(0),
        &Plane::XY,
        0.0,
        20.0,
        1,
    )
    .unwrap();
    // Fillet the three edges meeting at the far top corner.
    let corner = Vec3::new(20.0, 20.0, 20.0);
    let pairs: Vec<(usize, usize)> = block
        .edge_faces()
        .into_iter()
        .filter(|((a, b), fs)| {
            fs.len() == 2 && {
                let (pa, pb) = (block.vertices[*a as usize], block.vertices[*b as usize]);
                pa.distance(corner) < 1e-9 || pb.distance(corner) < 1e-9
            }
        })
        .map(|(_, fs)| (fs[0], fs[1]))
        .collect();
    assert_eq!(pairs.len(), 3);
    let filleted = ok_brep::blend_edges(
        &block,
        &pairs,
        3.0,
        ok_brep::BlendKind::Fillet,
        5f64.to_radians(),
        2,
    )
    .unwrap();
    let text = write_step(&[("Corner", &filleted)], "exact");
    assert_eq!(count(&text, "SPHERICAL_SURFACE"), 1);
    assert_eq!(count(&text, "CYLINDRICAL_SURFACE"), 3);
    // The patch is bounded by its three circular arcs alone: no seam.
    assert_eq!(count(&text, "ADVANCED_FACE"), 6 + 3 + 1);
    if let Ok(dir) = std::env::var("OK_STEP_DUMP") {
        std::fs::write(format!("{dir}/corner.step"), &text).unwrap();
    }
    let bodies = read_step(&text).unwrap();
    let back = solid_of(&bodies[0]);
    back.validate().unwrap();
    let (v0, v1) = (filleted.volume(), back.volume());
    assert!(((v1 - v0) / v0).abs() < 0.01, "volume {v1} vs {v0}");
}

#[test]
fn a_filleted_rim_goes_out_as_a_torus() {
    let body = cylinder(10.0, 5.0, 1);
    let top = body
        .faces
        .iter()
        .position(|f| f.plane.normal.approx_eq(Vec3::Z))
        .unwrap();
    let pairs: Vec<(usize, usize)> = body
        .edge_faces()
        .into_iter()
        .filter(|(_, fs)| fs.len() == 2 && fs.contains(&top))
        .map(|(_, fs)| (fs[0], fs[1]))
        .collect();
    let filleted = ok_brep::blend_edges(
        &body,
        &pairs,
        2.0,
        ok_brep::BlendKind::Fillet,
        5f64.to_radians(),
        2,
    )
    .unwrap();
    let text = write_step(&[("Rim", &filleted)], "exact");
    if let Ok(dir) = std::env::var("OK_STEP_DUMP") {
        std::fs::write(format!("{dir}/rim.step"), &text).unwrap();
    }
    assert_eq!(count(&text, "TOROIDAL_SURFACE"), 1);
    assert_eq!(count(&text, "CYLINDRICAL_SURFACE"), 1);
    assert_eq!(count(&text, "ADVANCED_FACE"), 4);
    let bodies = read_step(&text).unwrap();
    let back = solid_of(&bodies[0]);
    back.validate().unwrap();
    let (v0, v1) = (filleted.volume(), back.volume());
    assert!(((v1 - v0) / v0).abs() < 0.01, "volume {v1} vs {v0}");
}
