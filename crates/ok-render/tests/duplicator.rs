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
use ok_model::{AssemblyResult, Document, MateId, TabId};
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

/// A placed body's own name: a sub-assembly's member is placed as
/// "Sub / member".
fn member(name: &str) -> &str {
    name.rsplit(" / ").next().unwrap()
}

fn body<'a>(r: &'a AssemblyResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| member(&b.name) == name)
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
        .filter(|b| member(&b.name) == prefix || member(&b.name).starts_with(&format!("{prefix} ")))
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

/// How far the slide drops from the drawn pose before the stop screw's
/// tip lands on the stop block on the carriage plate.
fn stop_landing(r: &AssemblyResult) -> f64 {
    let (screw_lo, screw_hi) = bounds(body(r, "Stop screw"));
    let (sx, sy) = (
        (screw_lo.x + screw_hi.x) / 2.0,
        (screw_lo.y + screw_hi.y) / 2.0,
    );
    let plate = body(r, "Z carriage plate");
    let top = flat_faces(plate, 1.0)
        .into_iter()
        .filter(|f| f.z < bounds(plate).1.z - 1e-6 && f.contains(sx, sy))
        .map(|f| f.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(top.is_finite(), "no stop block under the screw");
    screw_lo.z - top
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
    assert_eq!(studios, 26, "{studios} part studios");
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
    assert_eq!(placed, 54);
    let machine = tab_named(&doc, "Duplicator");
    let r = doc.regenerate_assembly(machine).unwrap();
    assert_eq!(r.bodies.len(), 54);
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    // The three motion sub-assemblies, each with its own sheet.
    for (name, want) in [("Gantry", 14), ("X slide", 14), ("Z slide", 12)] {
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
    assert!(
        y_minus > 185.0 && y_plus > 185.0,
        "Y travel: 100 beyond the puzzle"
    );
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
    let plate = body(&r, "Z carriage plate");
    assert!(
        (bounds(plate).0.z - blank_hi.z).abs() < 1e-6,
        "the carriage plate's bottom is the board top"
    );
    let landing = stop_landing(&r);
    let first_pass = raised - landing;
    println!("stop screw lands after {landing} mm: first pass {first_pass} deep");
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
    let back_off = drop - landing;
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

    // The gantry is one piece on the Y blocks: the deck spans both rails'
    // blocks and the wall stands on it, and the carriage plate runs
    // in front of the deck's front edge down to the board top.
    let deck = body(&r, "Deck");
    let (deck_lo, deck_hi) = bounds(deck);
    for b in group(&r, "Y blocks SC20UU") {
        let (lo, hi) = bounds(b);
        assert!((hi.z - deck_lo.z).abs() < 1e-6, "a Y block under the deck");
        assert!(deck_lo.x < lo.x && hi.x < deck_hi.x && deck_lo.y < lo.y && hi.y < deck_hi.y);
    }
    let wall = body(&r, "Wall");
    let (wall_lo, wall_hi) = bounds(wall);
    assert!(
        (wall_lo.z - deck_hi.z).abs() < 1e-6,
        "the wall stands on the deck"
    );
    assert!(
        (wall_lo.x - deck_lo.x).abs() < 1e-6 && (wall_hi.x - deck_hi.x).abs() < 1e-6,
        "full width"
    );
    println!("deck {} above the blank", deck_lo.z - blank_hi.z);
    assert!(deck_lo.z - blank_hi.z > 40.0, "the deck clears the work");
    for b in group(&r, "Y supports SK20") {
        assert!(
            bounds(b).1.z < deck_lo.z,
            "the deck passes over the Y supports"
        );
    }
    let (plate_lo, plate_hi2) = bounds(plate);
    assert!(
        plate_lo.z < deck_lo.z && plate_hi2.z > deck_hi.z,
        "the carriage plate hangs past the deck's height"
    );
    assert!(
        deck_lo.y > plate_hi2.y + 5.0,
        "the deck's front edge is behind the carriage plate, so the plate sweeps X clear of it"
    );

    // The tools sit in their clamps without touching the plate, and the
    // fences sit beside the work.
    for (a, b) in [
        ("Router", "Tool support"),
        ("Pilot", "Tool support"),
        ("Tool support", "Z carriage plate"),
        ("Wall", "Z carriage plate"),
        ("Deck", "Blank"),
        ("Deck", "Fences 1"),
        ("Blank", "Fences 1"),
        ("Pattern plate", "Fences 2"),
    ] {
        let v = overlap(body(&r, a), body(&r, b)).unwrap_or_else(|| panic!("{a} x {b}"));
        assert!(v < 1e-6, "{a} x {b}: {v} mm3");
    }
}

/// The tools in the work are the point: the bit in the blank (and, on
/// the last pass, half a millimetre into the platform under it and a
/// half kerf into the fence where the pattern runs to the blank's edge),
/// the pilot in the plate's grooves or its reference hole.
fn tool_in_work(a: &str, b: &str) -> bool {
    (a == "Router" || a == "Pilot")
        && [
            "Blank",
            "Pattern plate",
            "Platforms 1",
            "Platforms 2",
            "Fences 1",
            "Fences 2",
        ]
        .contains(&b)
}

/// The machine's sliders: (id, offset as drawn, the sign that moves the
/// part the positive way along its axis), by name.
fn sliders(
    doc: &mut Document,
    tab: TabId,
) -> std::collections::BTreeMap<String, (MateId, f64, f64)> {
    let asm = doc.assembly(tab).unwrap();
    let mates: Vec<(MateId, String, f64)> = asm
        .mates
        .iter()
        .map(|m| (m.id, m.name.clone(), m.offset))
        .collect();
    assert_eq!(mates.len(), 4, "four sliders");
    let mut out = std::collections::BTreeMap::new();
    for (id, name, offset) in mates {
        let (part, axis): (&str, fn(Vec3) -> f64) = match name.as_str() {
            "Y travel" => ("Deck", |v| v.y),
            "X travel" => ("Z carriage plate", |v| v.x),
            "Z travel" => ("Tool support", |v| v.z),
            "stop screw" => ("Stop screw", |v| v.z),
            other => panic!("unexpected mate {other}"),
        };
        let r0 = doc.regenerate_assembly(tab).unwrap();
        let r1 = doc
            .preview_assembly_at(tab, &[(id, 0.0, offset + 10.0)])
            .unwrap();
        let moved = axis(bounds(body(&r1, part)).0) - axis(bounds(body(&r0, part)).0);
        assert!(
            (moved.abs() - 10.0).abs() < 1e-6,
            "{name} moves {part} by {moved}"
        );
        out.insert(name, (id, offset, moved.signum()));
    }
    out
}

/// The machine swept over the work on its sliders: the bit at the
/// blank's left edge, middle and right edge, every 25 mm from the
/// reference hole's y to the blank's back edge, at the first and the
/// last pass (the stop screw backed off for the last, as the operator
/// does); the kernel resolves each pose from the mates and every pair of
/// bodies from different instances is intersected. A touch is allowed;
/// an overlap that is not a tool in the work is a collision. Checking
/// only moving parts against fixed ones once let a notch in the deck
/// round the carriage plate through, which would have pinned the X
/// travel to the notch.
#[test]
fn the_machine_clears_itself_over_the_work() {
    let mut doc = load();
    let machine = tab_named(&doc, "Duplicator");
    let sliders = sliders(&mut doc, machine);
    let r = doc.regenerate_assembly(machine).unwrap();
    let (bit_lo, bit_hi) = bounds(body(&r, "Router"));
    let bit = Vec3::new(
        (bit_lo.x + bit_hi.x) / 2.0,
        (bit_lo.y + bit_hi.y) / 2.0,
        bit_lo.z,
    );
    let (blank_lo, blank_hi) = bounds(body(&r, "Blank"));
    let (plate_lo, plate_hi) = bounds(body(&r, "Pattern plate"));
    // Where the bit has to go: the blank's corners and, through the
    // pilot, the reference hole 5 mm inside the plate's front-left corner.
    let xs = [blank_lo.x - bit.x, 0.0, blank_hi.x - bit.x];
    let (y0, y1) = (
        (plate_lo.y + 5.0 - bit.y).min(blank_lo.y - bit.y),
        blank_hi.y - bit.y,
    );
    let steps = ((y1 - y0) / 25.0).ceil() as usize;
    let ys: Vec<f64> = (0..=steps)
        .map(|i| y0 + (y1 - y0) * i as f64 / steps as f64)
        .collect();
    let first_pass = -(bit.z - blank_hi.z) + 4.0;
    let last_pass = -(bit.z - plate_hi.z) - 12.5;
    let landing = stop_landing(&r);
    let at = |name: &str, d: f64| {
        let (id, offset, sign) = sliders[name];
        (id, 0.0, offset + sign * d)
    };
    let mut hits = Vec::new();
    let mut poses = 0;
    for &dx in &xs {
        for &dy in &ys {
            for &dz in &[first_pass, last_pass] {
                poses += 1;
                let back_off = (-dz - landing).max(0.0);
                let r = doc
                    .preview_assembly_at(
                        machine,
                        &[
                            at("X travel", dx),
                            at("Y travel", dy),
                            at("Z travel", dz),
                            at("stop screw", back_off),
                        ],
                    )
                    .unwrap();
                assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
                for i in 0..r.bodies.len() {
                    for j in i + 1..r.bodies.len() {
                        if r.placed[i] == r.placed[j] {
                            continue; // one rigid group
                        }
                        let (a, b) = (member(&r.bodies[i].name), member(&r.bodies[j].name));
                        let in_work = tool_in_work(a, b) || tool_in_work(b, a);
                        match overlap(&r.bodies[i].solid, &r.bodies[j].solid) {
                            Some(v) if v > 1e-3 && !in_work => {
                                hits.push(format!("{a} x {b} at ({dx}, {dy}, {dz}): {v:.1} mm3"))
                            }
                            Some(v) if v > 100.0 => hits.push(format!(
                                "{a} x {b} at ({dx}, {dy}, {dz}): {v:.1} mm3 of tool in the work"
                            )),
                            None => hits
                                .push(format!("{a} x {b} at ({dx}, {dy}, {dz}): boolean failed")),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    println!("{poses} poses");
    for h in &hits {
        println!("{h}");
    }
    assert!(hits.is_empty(), "{} collisions", hits.len());
}

/// The pattern plate as a master for the duplicator itself: its grooves
/// are the gap wide, so a bit the size of the gap rides them and leaves
/// nothing, and the next size up (an eighth of an inch) cannot enter
/// them at all: every groove reads as a corner tighter than the bit.
#[test]
fn the_pattern_plate_is_a_master_for_a_bit_the_size_of_the_gap_and_not_for_a_bigger_one() {
    let mut doc = load();
    let tab = tab_named(&doc, "Pattern plate");
    let r = doc.regenerate_studio(tab, None).unwrap();
    let plate = &r.bodies[0].solid;
    let (lo, hi) = bounds(plate);
    let floor = flat_faces(plate, 1.0)
        .into_iter()
        .filter(|f| f.z < hi.z - 1e-6 && f.z > lo.z + 1e-6)
        .map(|f| f.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let groove = hi.z - floor;
    let slab = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let lattice = (slab - plate.volume()) / groove;
    let bit = |d: f64| ok_brep::Bit {
        diameter: d,
        reach: groove + 2.0,
        shape: ok_brep::BitShape::Flat,
    };
    let fine = ok_brep::duplicate_check(plate, &bit(2.0), Some(0.5)).unwrap();
    println!(
        "2 mm bit: residual {} mm2 up to {} mm, undercut {} mm3, {:?}",
        fine.residual_area, fine.residual_max, fine.undercut_volume, fine.problems
    );
    assert!(fine.problems.is_empty(), "{:?}", fine.problems);
    assert!((fine.depth_max - groove).abs() < 1e-6);
    let coarse = ok_brep::duplicate_check(plate, &bit(3.175), Some(0.5)).unwrap();
    println!(
        "1/8 in bit: residual {} mm2 of a {lattice:.0} mm2 lattice, up to {} mm, {:?}",
        coarse.residual_area, coarse.residual_max, coarse.problems
    );
    assert_eq!(coarse.problems.len(), 1, "{:?}", coarse.problems);
    assert!(coarse.problems[0].starts_with("concave corners"));
    assert!(
        (coarse.residual_area - lattice).abs() < 0.1 * lattice,
        "{} vs {lattice}",
        coarse.residual_area
    );
    assert!((coarse.residual_max - groove).abs() < 0.1);
}
