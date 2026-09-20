//! Compares a kernel part (.okpart) with a reference STL: volumes, and the
//! material each has that the other lacks, with its extent.
use ok_brep::{boolean, BoolOp, Solid};
use ok_math::{Plane, Vec3};

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
    let mut s = Solid::from_polygons(polys, surfaces).expect("stl closes");
    s.merge_coplanar_faces();
    s
}

fn main() {
    // compare_stl part.okpart reference.stl [out.png] [--tab NAME]
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let tab_name = args.iter().position(|a| a == "--tab").map(|i| {
        let name = args[i + 1].clone();
        args.drain(i..=i + 1);
        name
    });
    let (part, stl, png) = (&args[0], &args[1], args.get(2));
    let mut doc = ok_model::Document::from_json(&std::fs::read_to_string(part).unwrap()).unwrap();
    let tab = match tab_name {
        Some(name) => {
            doc.tabs
                .iter()
                .find(|t| t.name() == name)
                .unwrap_or_else(|| panic!("no tab {name}"))
                .id
        }
        None => doc.tabs[0].id,
    };
    let result = doc.regenerate_studio(tab, None).unwrap();
    let kernel = &result.bodies[0].solid;
    let mesh = ok_mesh::from_stl(&std::fs::read(stl).unwrap()).unwrap();
    let reference = solid_of(&mesh);
    println!(
        "kernel volume {:.0}, stl volume {:.0}",
        kernel.volume(),
        reference.volume()
    );
    for (name, a, b) in [
        ("stl has, kernel lacks", &reference, kernel),
        ("kernel has, stl lacks", kernel, &reference),
    ] {
        match boolean(a, b, BoolOp::Difference) {
            Ok(d) => {
                for (i, shell) in d.shells().iter().enumerate() {
                    let (lo, hi) = shell.bounds().unwrap();
                    println!("{name}: lump {i} volume {:.0}  x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}", shell.volume(), lo.x, hi.x, lo.y, hi.y, lo.z, hi.z);
                }
            }
            Err(e) => println!("{name}: boolean failed: {e}"),
        }
    }
    if let Some(png) = png {
        let options = ok_render::Options {
            width: 900,
            height: 700,
            view: ok_render::View::parse("iso").unwrap(),
            section: None,
            edges: true,
            triad: false,
            fit: None,
        };
        std::fs::write(png, ok_render::screenshot(&mut doc, tab, &options).unwrap()).unwrap();
    }
}
