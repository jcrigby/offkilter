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
            assert!(r.instance_errors.is_empty(), "{:?}", r.instance_errors);
            assert!(r.mate_errors.is_empty(), "{:?}", r.mate_errors);
            // The carriage and what is bolted to it hang off one slider
            // mate; the build script drew them at their placements and
            // derived the mates from those, so the mates must resolve to
            // exactly the same poses.
            let asm = doc.assembly(*id).unwrap();
            assert!(asm.mates.len() >= 7, "{} mates", asm.mates.len());
            let moving = asm.instances.iter().filter(|i| !i.fixed).count();
            assert!(moving >= 7, "{moving} mated instances");
            for inst in &asm.instances {
                let want = inst.placement.to_transform();
                let got = r.transforms[&inst.id];
                assert!(
                    got.t.distance(want.t) < 1e-6,
                    "{}: at {:?}, drawn at {:?}",
                    inst.name,
                    got.t,
                    want.t
                );
                for i in 0..3 {
                    for j in 0..3 {
                        assert!(
                            (got.m[i][j] - want.m[i][j]).abs() < 1e-6,
                            "{}: rotation {:?} vs {:?}",
                            inst.name,
                            got.m,
                            want.m
                        );
                    }
                }
            }
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

/// The assembly's shop drawing: one balloon and one parts-list row per
/// distinct part, the sheet at a standard scale, a valid PDF.
#[test]
fn the_assembly_sheet_lists_every_part_once() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/router_lift.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let tab = doc
        .tabs
        .iter()
        .find(|t| t.kind_name() == "assembly")
        .unwrap()
        .id;
    let asm = doc.assembly(tab).unwrap();
    let mut distinct: Vec<(u32, usize)> =
        asm.instances.iter().map(|i| (i.studio.0, i.body)).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let parts = ok_sheet::parts_of(&mut doc, tab).unwrap();
    assert_eq!(parts.len(), 32);
    let refs: Vec<ok_sheet::Part> = parts
        .iter()
        .map(|(name, material, key, solid)| ok_sheet::Part {
            name: name.clone(),
            material: material.clone(),
            solid,
            key: *key,
        })
        .collect();
    let opts = ok_sheet::Options {
        sheet: ok_sheet::SheetSize::A3,
        ..ok_sheet::Options::default()
    };
    let sheet = ok_sheet::Sheet::layout(&refs, &opts).unwrap();
    assert_eq!(sheet.part_rows(), distinct.len());
    // 1:5 on A3: the views fit beside the 21-row parts list.
    assert!((sheet.scale() - 0.2).abs() < 1e-9, "{}", sheet.scale());
    let pdf = sheet.to_pdf();
    assert!(
        pdf.starts_with(b"%PDF-1.4") && pdf.len() > 20_000,
        "{} bytes",
        pdf.len()
    );
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("(Carriage) Tj") && text.contains("(SC20UU) Tj"));
    assert_eq!(text.matches("(ITEM) Tj").count(), 1);
    // Every distinct part gets a balloon number.
    for item in 1..=distinct.len() {
        assert!(text.contains(&format!("({item}) Tj")), "balloon {item}");
    }
}

/// The LINE entities of a DXF as endpoint pairs.
fn dxf_lines(text: &str) -> Vec<[Vec3; 2]> {
    let toks: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < toks.len() {
        if toks[i] == "0" && toks[i + 1] == "LINE" {
            let mut vals = std::collections::HashMap::new();
            let mut j = i + 2;
            while j + 1 < toks.len() && toks[j] != "0" {
                if let Ok(v) = toks[j + 1].parse::<f64>() {
                    vals.insert(toks[j], v);
                }
                j += 2;
            }
            out.push([
                Vec3::new(vals["10"], vals["20"], 0.0),
                Vec3::new(vals["11"], vals["21"], 0.0),
            ]);
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// The shop instructions say to print the arm template full size: the
/// plan view of the arm, exported as DXF, holds every line of the
/// template the project shipped with.
#[test]
fn the_arm_plan_view_is_the_shop_template() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/router_lift.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let arm = doc.tabs.iter().find(|t| t.name() == "Arm").unwrap().id;
    let dxf = ok_render::view_dxf(&mut doc, arm, ok_render::View::Top, false).unwrap();
    let got = dxf_lines(&dxf);
    let reference =
        dxf_lines(&std::fs::read_to_string(dir.join("reference/arm_template.dxf")).unwrap());
    assert_eq!(reference.len(), 6);
    for [a, b] in &reference {
        assert!(
            got.iter()
                .any(|[c, d]| (a.distance(*c) < 1e-6 && b.distance(*d) < 1e-6)
                    || (a.distance(*d) < 1e-6 && b.distance(*c) < 1e-6)),
            "template line {a:?} to {b:?} is not in the plan view"
        );
    }
    // The dowel holes and screw pilots are in the underside: nothing
    // round is visible from above, and they show dashed when hidden
    // lines are asked for.
    assert_eq!(dxf.matches("\nCIRCLE\n").count(), 0, "{dxf}");
    let with_hidden = ok_render::view_dxf(&mut doc, arm, ok_render::View::Top, true).unwrap();
    let hidden_circles = with_hidden.matches("CIRCLE\n8\nHIDDEN\n").count();
    assert!(hidden_circles >= 4, "{hidden_circles} hidden circles");
}
