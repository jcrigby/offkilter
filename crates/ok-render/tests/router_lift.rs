//! The router lift project (examples/router-lift) as a regression suite:
//! the document its build script wrote through the MCP server is
//! regenerated here, every part must be a closed solid, the printed parts
//! must match the OpenSCAD reference meshes in volume and extent, and
//! the assembly must place every instance without error.

use ok_brep::Solid;
use ok_math::{Plane, Vec3};
use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/router-lift")
}

fn solid_of(m: &ok_mesh::ReadMesh) -> Solid {
    let mut polys = Vec::new();
    let mut surfaces = Vec::new();
    for (i, t) in m.triangles.iter().enumerate() {
        let pts: Vec<Vec3> = t.iter().map(|&k| m.vertices[k as usize]).collect();
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
                feature: 99,
                local: i as u32,
            },
        });
    }
    Solid::from_polygons(polys, surfaces).expect("reference mesh closes")
}

/// Printed parts and their reference STLs (OpenSCAD, $fn = 96).
const PRINTED: &[(&str, &str)] = &[
    ("Carriage", "carriage"),
    ("Ring blank", "ring_blank"),
    ("Ring 30", "ring_30"),
    ("Ring 40", "ring_40"),
    ("Ring 55", "ring_55"),
    ("Ring align", "ring_align"),
    ("Post round", "pin_post_round"),
    ("Post slot", "pin_post_slot"),
    ("Chuck", "pin_chuck"),
];

#[test]
fn every_part_regenerates_closed_and_the_printed_ones_match_their_references() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/router_lift.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let tabs: Vec<(ok_model::TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    assert!(tabs.len() >= 26, "{} tabs", tabs.len());
    let mut checked = 0;
    for (id, name, kind) in &tabs {
        if kind == "assembly" {
            let r = doc.regenerate_assembly(*id).unwrap();
            assert!(
                r.bodies.len() >= 32,
                "{name}: {} placed bodies",
                r.bodies.len()
            );
            continue;
        }
        let r = doc.regenerate_studio(*id, None).unwrap();
        let errors: Vec<_> = r.errors().collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        assert_eq!(r.bodies.len(), 1, "{name}: one body");
        let solid = &r.bodies[0].solid;
        solid.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(solid.volume() > 0.0, "{name}: volume");
        let Some((_, stem)) = PRINTED.iter().find(|(t, _)| t == name) else {
            continue;
        };
        let bytes = std::fs::read(dir.join(format!("reference/{stem}.stl"))).unwrap();
        let reference = solid_of(&ok_mesh::from_stl(&bytes).unwrap());
        let (v, r) = (solid.volume(), reference.volume());
        assert!(
            ((v - r) / r).abs() < 0.005,
            "{name}: volume {v:.0} vs reference {r:.0} ({:+.2} %)",
            (v - r) / r * 100.0
        );
        let (lo, hi) = solid.bounds().unwrap();
        let (rlo, rhi) = reference.bounds().unwrap();
        for (a, b) in [(lo, rlo), (hi, rhi)] {
            assert!(
                a.distance(b) < 0.2,
                "{name}: extent {a:?} vs reference {b:?}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, PRINTED.len());
}
