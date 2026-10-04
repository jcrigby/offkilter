//! The carving duplicator (examples/duplicator) as a regression suite:
//! every part regenerates closed, the assembly places all of them, and
//! the machine's limits are read off the placed bodies: the travel each
//! axis has between its shaft supports, whether the bit and the pilot
//! reach over the blank and the pattern's lattice, and the depth
//! sequence (the stop screw limiting the early passes, the pilot
//! bottoming in the groove limiting the last) within the Z travel.
//! Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, TabId};
use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/duplicator")
}

fn load() -> Document {
    let json = std::fs::read_to_string(example_dir().join("out/duplicator.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn body<'a>(r: &'a AssemblyResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
        .solid
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

/// The bodies placed from one studio ("X shafts 1", "X shafts 2").
fn group<'a>(r: &'a AssemblyResult, prefix: &str) -> Vec<&'a Solid> {
    let v: Vec<_> = r
        .bodies
        .iter()
        .filter(|b| b.name == prefix || b.name.starts_with(&format!("{prefix} ")))
        .map(|b| &b.solid)
        .collect();
    assert!(!v.is_empty(), "no bodies {prefix}");
    v
}

/// Overlap volume of two placed bodies, None when the boolean fails.
fn overlap(a: &Solid, b: &Solid) -> Option<f64> {
    let (la, ha) = bounds(a);
    let (lb, hb) = bounds(b);
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return Some(0.0);
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .ok()
        .map(|s| s.volume().abs())
}

/// How far a carriage moves along `axis` between its shaft supports: the
/// gap between the supports' inner faces less the blocks' outer span, as
/// (minus, plus) from the pose the document is drawn in.
fn travel(supports: &[&Solid], blocks: &[&Solid], axis: fn(Vec3) -> f64) -> (f64, f64) {
    let (mut lo_face, mut hi_face) = (f64::NEG_INFINITY, f64::INFINITY);
    let (mut blo, mut bhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in blocks {
        let (lo, hi) = bounds(b);
        blo = blo.min(axis(lo));
        bhi = bhi.max(axis(hi));
    }
    for s in supports {
        let (lo, hi) = bounds(s);
        if axis(hi) <= blo + 1e-6 {
            lo_face = lo_face.max(axis(hi));
        } else if axis(lo) >= bhi - 1e-6 {
            hi_face = hi_face.min(axis(lo));
        } else {
            panic!("a support among the blocks");
        }
    }
    (blo - lo_face, hi_face - bhi)
}

/// A horizontal face of a solid: its height and its outline's extent.
struct Flat {
    z: f64,
    lo: (f64, f64),
    hi: (f64, f64),
}

impl Flat {
    fn contains(&self, x: f64, y: f64) -> bool {
        self.lo.0 <= x && x <= self.hi.0 && self.lo.1 <= y && y <= self.hi.1
    }
}

/// The horizontal faces of a solid whose normal points `up` (1.0) or down (-1.0).
fn flat_faces(s: &Solid, up: f64) -> Vec<Flat> {
    s.faces
        .iter()
        .filter(|f| (f.plane.normal.z - up).abs() < 1e-9)
        .map(|f| {
            let pts: Vec<Vec3> = f.loops[0].iter().map(|&i| s.vertices[i as usize]).collect();
            let mut flat = Flat {
                z: pts[0].z,
                lo: (f64::INFINITY, f64::INFINITY),
                hi: (f64::NEG_INFINITY, f64::NEG_INFINITY),
            };
            for p in &pts {
                flat.lo.0 = flat.lo.0.min(p.x);
                flat.lo.1 = flat.lo.1.min(p.y);
                flat.hi.0 = flat.hi.0.max(p.x);
                flat.hi.1 = flat.hi.1.max(p.y);
            }
            flat
        })
        .collect()
}

#[test]
fn every_part_regenerates_closed_and_the_machine_places_every_body() {
    let mut doc = load();
    let tabs: Vec<(TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    let studios = tabs.iter().filter(|t| t.2 == "part_studio").count();
    assert_eq!(studios, 24, "{studios} part studios");
    let mut placed = 0;
    for (id, name, kind) in &tabs {
        if kind == "part_studio" {
            let r = doc.regenerate_studio(*id, None).unwrap();
            assert!(!r.bodies.is_empty(), "{name}: no bodies");
            for b in &r.bodies {
                assert!(b.solid.volume() > 0.0, "{name} has volume");
                b.solid.validate().unwrap();
            }
            assert!(
                r.statuses.iter().all(|f| f.error.is_none()),
                "{name}: {:?}",
                r.statuses
            );
            placed += r.bodies.len();
        }
    }
    assert_eq!(placed, 53);
    let machine = tab_named(&doc, "Duplicator");
    let r = doc.regenerate_assembly(machine).unwrap();
    assert_eq!(r.bodies.len(), 53);
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    // The sub-assembly sheets draw subsets of the same bodies.
    for (name, want) in [
        ("Base and Y axis", 17),
        ("X axis", 16),
        ("Z axis", 15),
        ("Tool holder", 9),
    ] {
        let sub = tab_named(&doc, name);
        let r = doc.regenerate_assembly(sub).unwrap();
        assert_eq!(r.bodies.len(), want, "{name}");
    }
}

#[test]
fn the_tools_reach_over_the_work_and_the_depth_sequence_fits_the_z_travel() {
    let mut doc = load();
    let machine = tab_named(&doc, "Duplicator");
    let r = doc.regenerate_assembly(machine).unwrap();

    // Travel per axis, from the placed supports and blocks.
    let (x_minus, x_plus) = travel(
        &group(&r, "X supports SK20"),
        &group(&r, "X blocks SC20UU"),
        |v| v.x,
    );
    let (y_minus, y_plus) = travel(
        &group(&r, "Y supports SK20"),
        &group(&r, "Y blocks SC20UU"),
        |v| v.y,
    );
    let (z_down, z_up) = travel(
        &group(&r, "Z supports SK20"),
        &group(&r, "Z blocks SC20UU"),
        |v| v.z,
    );
    println!("travel x -{x_minus} +{x_plus}, y -{y_minus} +{y_plus}, z -{z_down} +{z_up}");
    assert!(x_minus > 250.0 && x_plus > 250.0, "X travel");
    assert!(y_minus > 135.0 && y_plus > 135.0, "Y travel");
    assert!(z_down > 50.0, "Z travel down {z_down}");

    // Reach: the bit over the whole blank, the pilot over the whole
    // lattice (the pattern plate less its 10 mm margin), from the drawn
    // pose plus the travel each way.
    let (bit_lo, bit_hi) = bounds(body(&r, "Router"));
    let bit = Vec3::new(
        (bit_lo.x + bit_hi.x) / 2.0,
        (bit_lo.y + bit_hi.y) / 2.0,
        bit_lo.z,
    );
    let (pilot_lo, pilot_hi) = bounds(body(&r, "Pilot"));
    let pilot = Vec3::new(
        (pilot_lo.x + pilot_hi.x) / 2.0,
        (pilot_lo.y + pilot_hi.y) / 2.0,
        pilot_lo.z,
    );
    assert!((bit.z - pilot.z).abs() < 1e-6, "tips level");
    let (blank_lo, blank_hi) = bounds(body(&r, "Blank"));
    let (plate_lo, plate_hi) = bounds(body(&r, "Pattern plate"));
    assert!(
        (blank_hi.z - plate_hi.z).abs() < 1e-6,
        "blank and plate tops level"
    );
    assert!(
        (bit.x - pilot.x - (blank_lo.x - plate_lo.x - 10.0)).abs() < 1e-6,
        "the blank sits where the lattice does"
    );
    println!(
        "bit at {bit:?}, blank x {}..{} y {}..{}",
        blank_lo.x, blank_hi.x, blank_lo.y, blank_hi.y
    );
    assert!(
        bit.x - x_minus <= blank_lo.x && bit.x + x_plus >= blank_hi.x,
        "bit over the blank in x"
    );
    assert!(
        bit.y - y_minus <= blank_lo.y && bit.y + y_plus >= blank_hi.y,
        "bit over the blank in y"
    );
    assert!(
        pilot.x - x_minus <= plate_lo.x + 10.0 && pilot.x + x_plus >= plate_hi.x - 10.0,
        "pilot over the lattice in x"
    );
    assert!(
        pilot.y - y_minus <= plate_lo.y + 10.0 && pilot.y + y_plus >= plate_hi.y - 10.0,
        "pilot over the lattice in y"
    );

    // Depth. The slide is drawn raised, the tips above the board. The stop
    // screw's tip landing on the stop block sets the early passes' depth;
    // the pilot bottoming in the groove sets the last.
    let raised = bit.z - blank_hi.z;
    println!("tips {raised} above the board");
    assert!(raised >= 30.0);
    let (screw_lo, screw_hi) = bounds(body(&r, "Stop screw"));
    let screw = Vec3::new(
        (screw_lo.x + screw_hi.x) / 2.0,
        (screw_lo.y + screw_hi.y) / 2.0,
        screw_lo.z,
    );
    let plate = body(&r, "Z carriage plate");
    assert!(
        (bounds(plate).0.z - blank_hi.z).abs() < 1e-6,
        "the carriage plate's bottom is the board top"
    );
    let stop_block_top = flat_faces(plate, 1.0)
        .into_iter()
        .filter(|f| f.z < bounds(plate).1.z - 1e-6 && f.contains(screw.x, screw.y))
        .map(|f| f.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(stop_block_top.is_finite(), "no stop block under the screw");
    let first_pass = raised - (screw_lo.z - stop_block_top);
    println!(
        "stop screw lands after {} mm: first pass {first_pass} deep",
        screw_lo.z - stop_block_top
    );
    assert!(
        first_pass > 2.0 && first_pass < 6.0,
        "first pass {first_pass}"
    );
    let pattern = body(&r, "Pattern plate");
    let floor = flat_faces(pattern, 1.0)
        .into_iter()
        .filter(|f| f.z < plate_hi.z - 1e-6 && f.z > plate_lo.z + 1e-6)
        .map(|f| f.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let last_pass = plate_hi.z - floor;
    println!("groove {last_pass} deep: the last pass");
    assert!((last_pass - 12.5).abs() < 1e-6);
    let drop = raised + last_pass;
    assert!(
        drop <= z_down,
        "the last pass needs {drop} of the {z_down} down travel"
    );
    // For the pilot to bottom, the screw backs off until it lands after
    // the whole drop: so many turns of M8 from the first-pass setting.
    let back_off = drop - (screw.z - stop_block_top);
    println!(
        "back the stop screw off {back_off} mm ({:.0} turns) for the last pass",
        back_off / 1.25
    );
    assert!(back_off > 0.0 && back_off < 25.0);
    // At the last pass the chuck clears the pattern plate and the tool
    // plate clears the blank.
    let chuck_bottom = flat_faces(body(&r, "Pilot"), -1.0)
        .into_iter()
        .filter(|f| f.z > pilot.z + 1e-6)
        .map(|f| f.z)
        .fold(f64::INFINITY, f64::min);
    println!(
        "chuck bottom {} above the plate at the last pass",
        chuck_bottom - drop - plate_hi.z
    );
    assert!(
        chuck_bottom - drop > plate_hi.z + 3.0,
        "chuck clears the plate"
    );
    let support_bottom = bounds(body(&r, "Tool support")).0.z;
    assert!(
        support_bottom - drop > blank_hi.z + 30.0,
        "tool plate clears the blank"
    );

    // The tools sit in their clamps without touching the plate, and the
    // fences sit beside the work.
    for (a, b) in [
        ("Router", "Tool support"),
        ("Pilot", "Tool support"),
        ("Tool support", "Z carriage plate"),
        ("Blank", "Fences 1"),
        ("Pattern plate", "Fences 2"),
    ] {
        let v = overlap(body(&r, a), body(&r, b)).unwrap_or_else(|| panic!("{a} x {b}"));
        assert!(v < 1e-6, "{a} x {b}: {v} mm3");
    }
}
