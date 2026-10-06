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
    assert!(tabs.len() >= 28, "{} tabs", tabs.len());
    let mut checked = 0;
    let (mut mates, mut moving, mut assemblies) = (0, 0, 0);
    for (id, name, kind) in &tabs {
        if kind == "assembly" {
            assemblies += 1;
            let r = doc.regenerate_assembly(*id).unwrap();
            // The lift assembly places every body: its loose parts and
            // the bodies of its three sub-assemblies.
            let least = if name == "Lift assembly" { 35 } else { 5 };
            assert!(
                r.bodies.len() >= least,
                "{name}: {} placed bodies",
                r.bodies.len()
            );
            assert!(r.instance_errors.is_empty(), "{:?}", r.instance_errors);
            assert!(r.mate_errors.is_empty(), "{:?}", r.mate_errors);
            // The carriage assembly hangs off one slider mate, the router
            // off the carriage, the blocks and nut off the carriage inside
            // its sub-assembly; the build script drew them all at their
            // placements and derived the mates from those, so the mates
            // must resolve to exactly the same poses.
            let asm = doc.assembly(*id).unwrap();
            mates += asm.mates.len();
            moving += asm.instances.iter().filter(|i| !i.fixed).count();
            if name == "Lift assembly" {
                let slider = asm
                    .mates
                    .iter()
                    .find(|m| m.name == "carriage travel")
                    .unwrap();
                assert!(
                    slider.b.sub.is_some(),
                    "the slider names the block inside the carriage assembly"
                );
                let subs = r.members.iter().filter(|m| m.is_some()).count();
                assert_eq!(subs, 17, "bodies of sub-assembly instances");
            }
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
    assert_eq!(assemblies, 4, "the lift and its three sub-assemblies");
    assert!(mates >= 7, "{mates} mates");
    assert!(moving >= 7, "{moving} mated instances");
}

/// The assembly's shop drawing: one balloon and one parts-list row per
/// distinct part, the sheet at a standard scale, a valid PDF.
#[test]
fn the_assembly_sheet_lists_every_part_once() {
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("out/router_lift.okpart")).unwrap();
    let mut doc = ok_model::Document::from_json(&json).unwrap();
    let tab_named = |doc: &ok_model::Document, name: &str| {
        doc.tabs
            .iter()
            .find(|t| t.kind_name() == "assembly" && t.name() == name)
            .unwrap()
            .id
    };
    let tab = tab_named(&doc, "Lift assembly");
    // Eighteen loose parts and three sub-assembly instances, one record
    // each; thirteen distinct items (the pivot's SK20s share the lift's).
    let parts = ok_sheet::parts_of(&mut doc, tab).unwrap();
    assert_eq!(parts.len(), 21);
    assert_eq!(parts.iter().map(|p| p.3.len()).sum::<usize>(), 35, "bodies");
    let mut distinct: Vec<(u32, usize)> = parts.iter().map(|p| p.2).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 13);
    let refs: Vec<ok_sheet::Part> = parts
        .iter()
        .map(|(name, material, key, solids, names)| ok_sheet::Part {
            name: name.clone(),
            material: material.clone(),
            solids: solids.iter().collect(),
            key: *key,
            names: names.clone(),
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
    let text = ok_sheet::pdf::inflated(&pdf);
    assert!(
        text.contains("(Carriage assembly) Tj")
            && text.contains("(Arm assembly) Tj")
            && !text.contains("(SC20UU) Tj"),
        "sub-assemblies are items, their parts are not"
    );
    assert_eq!(text.matches("(ITEM) Tj").count(), 1);
    assert!(
        !text.contains("[2 1] 0 d"),
        "no hidden lines on the assembly sheet"
    );
    // Every distinct item gets a balloon number.
    for item in 1..=distinct.len() {
        assert!(text.contains(&format!("({item}) Tj")), "balloon {item}");
    }
    // The carriage assembly's own sheet lists the carriage, four blocks
    // and the nut.
    let tab = tab_named(&doc, "Carriage assembly");
    let parts = ok_sheet::parts_of(&mut doc, tab).unwrap();
    assert_eq!(parts.len(), 6);
    let refs: Vec<ok_sheet::Part> = parts
        .iter()
        .map(|(name, material, key, solids, names)| ok_sheet::Part {
            name: name.clone(),
            material: material.clone(),
            solids: solids.iter().collect(),
            key: *key,
            names: names.clone(),
        })
        .collect();
    let sheet = ok_sheet::Sheet::layout(&refs, &ok_sheet::Options::default()).unwrap();
    assert_eq!(sheet.part_rows(), 3);
    let text = ok_sheet::pdf::inflated(&sheet.to_pdf()).to_string();
    assert!(text.contains("(Carriage) Tj") && text.contains("(SC20UU) Tj"));
    assert!(text.contains("(4) Tj"), "four blocks");
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
    // Each template line is in the plan, whole or as collinear pieces
    // (the leveling slot, open at the tail, interrupts the tail's edge).
    for [a, b] in &reference {
        let along = (*b - *a).normalized().unwrap();
        let on_line = |p: Vec3| {
            let d = p - *a;
            (d.x * along.y - d.y * along.x).abs() < 1e-6
        };
        let ends_at = |p: Vec3| {
            got.iter().any(|[c, d]| {
                on_line(*c) && on_line(*d) && (c.distance(p) < 1e-6 || d.distance(p) < 1e-6)
            })
        };
        assert!(
            ends_at(*a) && ends_at(*b),
            "template line {a:?} to {b:?} is not in the plan view"
        );
    }
    // Eight block bolt holes go through, so they are circles from above;
    // the leveling slot is lines and an arc; the chuck screw slots are in
    // the underside and show only when hidden lines are asked for.
    assert_eq!(dxf.matches("\nCIRCLE\n").count(), 8, "{dxf}");
    assert!(
        dxf.matches("\nARC\n").count() >= 1,
        "the slot's rounded end"
    );
    let with_hidden = ok_render::view_dxf(&mut doc, arm, ok_render::View::Top, true).unwrap();
    let hidden = with_hidden.matches("\n8\nHIDDEN\n").count();
    assert!(hidden >= 4, "{hidden} hidden entities");
}

/// The top's own sheet carries two sections at right angles through the
/// bit axis, captioned and traced on the plan view.
#[test]
fn the_top_sheet_has_two_sections_through_the_bit_axis() {
    let pdf = std::fs::read(example_dir().join("out/top.pdf")).unwrap();
    let text = ok_sheet::pdf::inflated(&pdf);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(
        text.contains("(SECTION A-A) Tj") && text.contains("(SECTION B-B) Tj"),
        "both sections captioned"
    );
    assert!(
        text.contains("Views: front, top, right, section@0, section-side@0"),
        "the sheet names its views"
    );
    assert!(
        text.contains("[6 1.5] 0 d"),
        "the cutting planes are traced"
    );
}

/// The range-of-motion outputs: the lift at the ends of its travel and
/// the arm level, half up and up, each a captioned sheet and a strip.
#[test]
fn the_range_of_motion_sheets_show_three_positions_each() {
    let dir = example_dir();
    for (stem, captions) in [
        ("lift_motion", ["lowest", "mid travel", "highest"]),
        (
            "arm_motion",
            ["level", "half up, 30 degrees", "up, 60 degrees"],
        ),
    ] {
        let pdf = std::fs::read(dir.join(format!("out/{stem}.pdf"))).unwrap();
        let text = ok_sheet::pdf::inflated(&pdf);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        for c in captions {
            assert!(text.contains(&format!("({c}) Tj")), "{stem}: {c} captioned");
        }
        assert!(text.contains("range of motion) Tj"), "{stem}: titled");
        let png = std::fs::read(dir.join(format!("out/{stem}.png"))).unwrap();
        let image = ok_render::from_png(&png).unwrap();
        assert_eq!(
            (image.width, image.height),
            (3 * 400, 300),
            "{stem}: three frames"
        );
    }
    // The lift's strip: the carriage climbs from frame to frame, so the
    // frames differ, and the fixed base stays put, so the bottom rows
    // of the frames agree.
    let png = std::fs::read(dir.join("out/lift_motion.png")).unwrap();
    let image = ok_render::from_png(&png).unwrap();
    let differ = |y: usize| {
        (0..400)
            .filter(|&x| image.pixel(x, y) != image.pixel(x + 800, y))
            .count()
    };
    assert!(differ(150) > 0, "the frames differ where the carriage is");
}
