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
        let x_axis = (pts[1] - pts[0]).normalized().unwrap();
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
