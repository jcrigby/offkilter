//! The scan supports (examples/scan-supports): a 20 mm cube and a
//! 20 x 20 x 40 post, each one closed body of six planar faces square
//! to the axes with its base at z = 0 and its centre in plan at the
//! origin, so a block centred on a mark of the measuring field is a
//! known height at a known place; the volumes are the blocks'; and the
//! STLs are written, twelve triangles each.

use ok_brep::{Solid, Surface};
use ok_math::Vec3;
use ok_model::{Document, TabId};
use std::path::PathBuf;

const SIDE: f64 = 20.0;
const POST: f64 = 40.0;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/scan-supports")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/scan_supports.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

#[test]
fn each_block_is_one_square_body_at_its_height_and_its_stl_is_written() {
    let mut doc = load();
    let numbers: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir().join("out/scan_supports.json")).unwrap(),
    )
    .unwrap();
    assert!(near(numbers["side"].as_f64().unwrap(), SIDE));
    assert!(near(numbers["post"].as_f64().unwrap(), POST));
    for (name, height) in [("cube", SIDE), ("post", POST)] {
        let r = doc.regenerate_studio(tab_named(&doc, name), None).unwrap();
        assert!(
            r.statuses.iter().all(|f| f.error.is_none()),
            "{name}: {:?}",
            r.statuses
        );
        assert_eq!(r.bodies.len(), 1, "{name}: one block");
        let b = &r.bodies[0];
        b.solid.validate().unwrap();
        // Centred in plan, base on the table, the height the scanner
        // checks the laser's plane against.
        let (lo, hi) = bounds(&b.solid);
        assert!(
            near(lo.x, -SIDE / 2.0) && near(hi.x, SIDE / 2.0),
            "{name}: x {}..{}",
            lo.x,
            hi.x
        );
        assert!(
            near(lo.y, -SIDE / 2.0) && near(hi.y, SIDE / 2.0),
            "{name}: y {}..{}",
            lo.y,
            hi.y
        );
        assert!(
            near(lo.z, 0.0) && near(hi.z, height),
            "{name}: z {}..{}",
            lo.z,
            hi.z
        );
        // Six planar faces, every one square to an axis: it prints
        // flat on any side and its top is a level plane.
        assert_eq!(b.solid.surfaces.len(), 6, "{name}: six faces");
        for s in &b.solid.surfaces {
            match s {
                Surface::Plane { normal, .. } => {
                    let n = normal.normalized().unwrap();
                    assert!(
                        n.x.abs() > 0.999 || n.y.abs() > 0.999 || n.z.abs() > 0.999,
                        "{name}: a face off the axes: {n:?}"
                    );
                }
                other => panic!("{name}: not a plane: {other:?}"),
            }
        }
        let volume = SIDE * SIDE * height;
        assert!(
            near(b.solid.volume(), volume),
            "{name}: volume {} of {volume}",
            b.solid.volume()
        );
        let recorded = numbers[format!("{name}_volume")].as_f64().unwrap();
        assert!(
            near(recorded, volume),
            "{name}: the script recorded {recorded}"
        );
        // The STL: binary, the block's twelve triangles.
        let bytes = std::fs::read(dir().join(format!("out/{name}{height}.stl"))).unwrap();
        let tris = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 84 + 50 * tris, "{name}: a binary STL");
        assert_eq!(tris, 12, "{name}: twelve triangles");
    }
}
