//! The dummy PCIe x4 card (examples/pcie-dummy): the card regenerates
//! as one closed body of the CEM's size, with its finger tab, key notch
//! and bracket holes where the spec puts them; the bracketed card is
//! one body reaching the bracket's heights; the STLs are written.

use ok_brep::{Solid, Surface};
use ok_math::Vec3;
use ok_model::{Document, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/pcie-dummy")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/pcie_dummy.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

/// Cylinders of a solid of radius about `r` (mm): origin and unit axis.
fn cylinders(s: &Solid, r: f64) -> Vec<(Vec3, Vec3)> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder {
                origin,
                axis,
                radius,
            } if (radius - r).abs() < 1e-6 => Some((*origin, axis.normalized().unwrap_or(Vec3::Z))),
            _ => None,
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_card_is_one_closed_body_of_the_spec_size_with_its_tab_key_and_holes() {
    let mut doc = load();
    let r = doc
        .regenerate_studio(tab_named(&doc, "card"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1);
    let s = &r.bodies[0].solid;
    s.validate().unwrap();
    let (lo, hi) = bounds(s);
    println!(
        "card: {:.2} x {:.2} x {:.2} mm, {:.1} cm3",
        hi.x - lo.x,
        hi.y - lo.y,
        hi.z - lo.z,
        s.volume() / 1000.0
    );
    assert!(near(lo.x, 0.0) && near(hi.x, 167.65), "MD2 length");
    assert!(
        near(lo.y, 0.0) && near(hi.y, 68.90),
        "low profile height from the finger bottom"
    );
    assert!(near(lo.z, 0.0) && near(hi.z, 1.57), "board thickness");
    // The volume is the outline's area through the thickness: the body
    // above the fingers, the bracket-end foot, the PCI block and the
    // tab, less the chamfers, the key and the two holes.
    let tab = 34.3 * 8.25 - 2.0 * 0.5 * 0.5 / 2.0;
    let key = 1.9 * (8.25 - 0.95 + 1.0 - 1.0) + std::f64::consts::PI * 0.95 * 0.95 / 2.0;
    let holes = 2.0 * std::f64::consts::PI * 1.65 * 1.65;
    let area =
        167.65 * (68.90 - 8.25) + 15.0 * (8.25 - 4.85) + 3.65 * (8.25 - 4.85) + tab - key - holes;
    assert!(
        (s.volume() - area * 1.57).abs() < 0.5,
        "{} vs {}",
        s.volume(),
        area * 1.57
    );
    // Two bracket screw holes through the board, 7.25 in from the end
    // and 53.90 apart, and the key notch's round top on datum A, 57.15
    // from the end: all axes along z.
    let mut holes: Vec<(f64, f64)> = cylinders(s, 1.65)
        .iter()
        .map(|(o, a)| {
            assert!(a.z.abs() > 1.0 - 1e-9);
            (o.x, o.y)
        })
        .collect();
    holes.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert_eq!(holes.len(), 2, "{holes:?}");
    assert!(
        near(holes[0].0, 7.25) && near(holes[0].1, 7.25),
        "{holes:?}"
    );
    assert!(
        near(holes[1].0, 7.25) && near(holes[1].1, 7.25 + 53.90),
        "{holes:?}"
    );
    let key = cylinders(s, 0.95);
    assert_eq!(key.len(), 1);
    assert!(
        near(key[0].0.x, 57.15) && near(key[0].0.y, 8.25 - 0.95),
        "{:?}",
        key[0].0
    );
    // The finger tab: the lowest edge spans 45.0 to 79.3 only, and the
    // body's bottom edge sits 8.25 up elsewhere.
    let mut x_at_bottom: Vec<f64> = s
        .vertices
        .iter()
        .filter(|v| near(v.z, 0.0) && v.y.abs() < 1e-6)
        .map(|v| v.x)
        .collect();
    x_at_bottom.sort_by(|a, b| a.total_cmp(b));
    assert!(
        near(x_at_bottom[0], 45.0 + 0.5) && near(*x_at_bottom.last().unwrap(), 79.3 - 0.5),
        "{x_at_bottom:?}"
    );
    let body_bottom: Vec<f64> = s
        .vertices
        .iter()
        .filter(|v| near(v.z, 0.0) && near(v.y, 8.25))
        .map(|v| v.x)
        .collect();
    assert!(
        body_bottom.iter().any(|x| near(*x, 15.0)) && body_bottom.iter().any(|x| near(*x, 167.65))
    );
}

#[test]
fn the_bracketed_card_is_one_body_at_the_brackets_heights_and_the_stls_are_written() {
    let mut doc = load();
    let r = doc
        .regenerate_studio(tab_named(&doc, "card with bracket"), None)
        .unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "the bracket fuses to the card");
    let s = &r.bodies[0].solid;
    s.validate().unwrap();
    let (lo, hi) = bounds(s);
    println!(
        "with bracket: x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}",
        lo.x, hi.x, lo.y, hi.y, lo.z, hi.z
    );
    // The top tab reaches 11.84 out from the plate's outer face, the
    // plate's bottom tab ends 79.20 below the top tab's underside, which
    // is 0.5 over the card's top, and the plate stands 17.56 off the
    // solder face.
    assert!(near(lo.x, -1.6 - 11.84) && near(hi.x, 167.65));
    assert!(
        near(hi.y, 68.90 + 0.5 + 1.6) && near(lo.y, 68.90 + 0.5 - 79.20),
        "{lo:?} {hi:?}"
    );
    assert!(
        near(lo.z, 0.0) && near(hi.z, 17.56),
        "flat on the solder side"
    );
    // The screw slot's round end, 6.35 in from the outer face.
    let slot = cylinders(s, 2.2);
    assert_eq!(slot.len(), 1, "{slot:?}");
    assert!(
        near(slot[0].0.x, -1.6 - 6.35) && near(slot[0].0.z, 8.8),
        "{:?}",
        slot[0].0
    );
    for name in ["card.stl", "card_with_bracket.stl"] {
        let bytes = std::fs::read(dir().join("out").join(name)).unwrap();
        let tris = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(
            bytes.len(),
            84 + 50 * tris,
            "{name}: a binary STL of {tris} triangles"
        );
        assert!(tris > 100, "{name}: {tris} triangles");
    }
}
