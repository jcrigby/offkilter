//! The carving duplicator (examples/duplicator) as a regression suite:
//! every part regenerates closed, the assembly places all of them, and
//! the machine's limits are read off the placed bodies: the travel each
//! axis has between its shaft supports, whether the bit and the pilot
//! reach over the blank and the pattern's lattice, and the depth
//! sequence (the stop screw limiting the early passes, the pilot
//! bottoming in the groove limiting the last) within the Z travel.
//! Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Transform};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, MateId, TabId};
use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/duplicator")
}

fn load_at(rel: &str) -> Document {
    let json = std::fs::read_to_string(example_dir().join(rel)).unwrap();
    Document::from_json(&json).unwrap()
}

fn load() -> Document {
    load_at("out/duplicator.okpart")
}

/// The portal variant (`build.py --portal`): the Y blocks under the
/// tools, a leg each side, the base without its back third.
fn load_portal() -> Document {
    load_at("out/portal/duplicator.okpart")
}

/// The arm variant (`build.py --arm`): the Woodsmith's arc, the arm
/// swinging and sliding on one X shaft, the head on its own pivot.
fn load_arm() -> Document {
    load_at("out/arm/duplicator.okpart")
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

fn every_part_regenerates_closed_and_the_machine_places_every_body_in(
    mut doc: Document,
    want_studios: usize,
    want_bodies: usize,
    subs: &[(&str, usize)],
) {
    let tabs: Vec<(TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    let studios = tabs.iter().filter(|t| t.2 == "part_studio").count();
    assert_eq!(studios, want_studios, "{studios} part studios");
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
    assert_eq!(placed, want_bodies);
    let machine = tab_named(&doc, "Duplicator");
    let r = doc.regenerate_assembly(machine).unwrap();
    assert_eq!(r.bodies.len(), want_bodies);
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    // The three motion sub-assemblies, each with its own sheet.
    for &(name, want) in subs {
        let sub = tab_named(&doc, name);
        let r = doc.regenerate_assembly(sub).unwrap();
        assert_eq!(r.bodies.len(), want, "{name}");
    }
}

const SUBS: [(&str, usize); 3] = [("Gantry", 14), ("X slide", 14), ("Z slide", 18)];

#[test]
fn every_part_regenerates_closed_and_the_machine_places_every_body() {
    every_part_regenerates_closed_and_the_machine_places_every_body_in(load(), 28, 63, &SUBS);
}

#[test]
fn every_part_of_the_portal_regenerates_closed_and_the_machine_places_every_body() {
    every_part_regenerates_closed_and_the_machine_places_every_body_in(
        load_portal(),
        28,
        63,
        &SUBS,
    );
}

#[test]
fn every_part_of_the_arm_regenerates_closed_and_the_machine_places_every_body() {
    every_part_regenerates_closed_and_the_machine_places_every_body_in(
        load_arm(),
        29,
        55,
        &[("Gantry", 10), ("Arm", 10), ("Head", 18)],
    );
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
        ("Pilot", "Stylus block 1"),
        ("Stylus block 1", "Tool support"),
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

    // The stylus block adjusts: it moves a millimetre each way in x and
    // in y in the tool plate's pocket before it meets the plate, and not
    // much more (the head's bolt holes have the same play), which covers
    // the half millimetre the plate's bores are drilled to. Its chuck's
    // shank goes up through the block to the height screw in the bridge,
    // and the stylus's tip is level with the bit's, as the test above
    // says, so z is the screw's.
    let block = body(&r, "Stylus block 1");
    let support = body(&r, "Tool support");
    let play = 1.0;
    for (dx, dy) in [(play, 0.0), (-play, 0.0), (0.0, play), (0.0, -play)] {
        let moved = block.transformed(&Transform::translation(Vec3::new(dx, dy, 0.0)));
        let v = overlap(&moved, support).unwrap_or_else(|| panic!("block moved {dx},{dy}"));
        assert!(
            v < 1e-6,
            "the block moved ({dx}, {dy}) meets the plate: {v} mm3"
        );
    }
    for (dx, dy) in [(play + 0.3, 0.0), (0.0, -(play + 0.3))] {
        let moved = block.transformed(&Transform::translation(Vec3::new(dx, dy, 0.0)));
        let v = overlap(&moved, support).unwrap_or_else(|| panic!("block moved {dx},{dy}"));
        assert!(
            v > 1.0,
            "the block moved ({dx}, {dy}) is still free: the pocket is bigger than the play"
        );
    }
    let screw = body(&r, "Stylus block 2");
    let (slo, shi) = bounds(screw);
    let (plo, phi) = bounds(body(&r, "Pilot"));
    assert!(
        (slo.z - phi.z).abs() < 1e-6 && ((slo.x + shi.x) - (plo.x + phi.x)).abs() < 1e-6,
        "the height screw's tip is on the chuck's shank top: screw {} over shank {}",
        slo.z,
        phi.z
    );
    assert!(
        shi.z > bounds(block).1.z,
        "the height screw's knob is above the bridge"
    );
    println!(
        "stylus block: {} square in the pocket, play {play} each way, chuck shank {} tall under the height screw",
        bounds(block).1.x - bounds(block).0.x - 36.0,
        phi.z - plo.z
    );
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
/// The arm's shafts and the blocks that turn on them.
fn turns_in(shaft: &str, block: &str) -> bool {
    (shaft == "X shafts" && block.starts_with("X blocks SC20UU"))
        || (shaft == "Head pivot shaft" && block.starts_with("Head blocks SC20UU"))
}

/// Every pair of bodies that move differently, intersected: a touch is
/// allowed, an overlap that is not a tool in the work is a collision,
/// and a tool more than 100 mm3 into the work is one too.
fn collisions(r: &AssemblyResult, pose: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            if r.placed[i] == r.placed[j] {
                continue; // one rigid group
            }
            let (a, b) = (member(&r.bodies[i].name), member(&r.bodies[j].name));
            let in_work = tool_in_work(a, b) || tool_in_work(b, a);
            // A shaft turned in a block's bore: two faceted cylinders
            // whose facets no longer line up overlap by a cubic
            // millimetre or two at every pose but the drawn one.
            let bearing = turns_in(a, b) || turns_in(b, a);
            match overlap(&r.bodies[i].solid, &r.bodies[j].solid) {
                Some(v) if v > 1e-3 && !in_work && !(bearing && v < 5.0) => {
                    hits.push(format!("{a} x {b} at {pose}: {v:.1} mm3"))
                }
                Some(v) if v > 100.0 => hits.push(format!(
                    "{a} x {b} at {pose}: {v:.1} mm3 of tool in the work"
                )),
                None => hits.push(format!("{a} x {b} at {pose}: boolean failed")),
                _ => {}
            }
        }
    }
    hits
}

fn the_machine_clears_itself_over_the_work_in(mut doc: Document) {
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
                hits.extend(collisions(&r, &format!("({dx}, {dy}, {dz})")));
            }
        }
    }
    println!("{poses} poses");
    for h in &hits {
        println!("{h}");
    }
    assert!(hits.is_empty(), "{} collisions", hits.len());
}

#[test]
fn the_machine_clears_itself_over_the_work() {
    the_machine_clears_itself_over_the_work_in(load());
}

#[test]
fn the_portal_clears_itself_over_the_work() {
    the_machine_clears_itself_over_the_work_in(load_portal());
}

/// The portal: the Y rails run to the base's front edge and the gantry's
/// blocks sit just behind the tools, so the gantry's body no longer parks
/// behind the work and the base is 640 deep instead of 855. Each side is a
/// leg, a foot on the rail's two blocks with a cheek standing on it in
/// front of the wall; between the cheeks a deck strip 150 deep behind
/// the wall keeps the shear plate. The tools still reach the blank and
/// the lattice, and the machine's own sweep is the test above.
#[test]
fn the_portal_is_a_third_shorter_with_its_blocks_under_the_tools() {
    let mut gantry = load();
    let mut doc = load_portal();
    let g = gantry
        .regenerate_assembly(tab_named(&gantry, "Duplicator"))
        .unwrap();
    let r = doc
        .regenerate_assembly(tab_named(&doc, "Duplicator"))
        .unwrap();
    let (blo, bhi) = bounds(body(&r, "Base"));
    let (glo, ghi) = bounds(body(&g, "Base"));
    let (depth, width) = (bhi.y - blo.y, bhi.x - blo.x);
    println!(
        "portal base {width} x {depth} against the gantry's {} x {}",
        ghi.x - glo.x,
        ghi.y - glo.y
    );
    assert!(depth < 0.76 * (ghi.y - glo.y), "depth {depth}");
    assert!(width < 1.1 * (ghi.x - glo.x), "width {width}");
    assert!(depth * width < 0.85 * (ghi.y - glo.y) * (ghi.x - glo.x));
    // The same work at the same place, the same tools, the same travel in
    // X and Z: nothing in front of the wall changed.
    for name in [
        "Blank",
        "Pattern plate",
        "Router",
        "Pilot",
        "Z carriage plate",
        "Wall",
    ] {
        let (a, b) = (bounds(body(&r, name)), bounds(body(&g, name)));
        let same = |p: Vec3, q: Vec3| (p.y - q.y).abs() < 1e-6 && (p.z - q.z).abs() < 1e-6;
        assert!(same(a.0, b.0) && same(a.1, b.1), "{name} moved in y or z");
    }
    // The blocks just behind the tools, the rails from the base's front
    // edge, and the travel still covers the work.
    let (bit_lo, bit_hi) = bounds(body(&r, "Router"));
    let bit = Vec3::new(
        (bit_lo.x + bit_hi.x) / 2.0,
        (bit_lo.y + bit_hi.y) / 2.0,
        bit_lo.z,
    );
    let blocks = group(&r, "Y blocks SC20UU");
    let front = blocks
        .iter()
        .map(|b| bounds(b).0.y)
        .fold(f64::INFINITY, f64::min);
    println!("the tools {} ahead of the front Y blocks", front - bit.y);
    assert!(
        front - bit.y < 60.0,
        "the blocks are under the tools, not behind them"
    );
    let supports = group(&r, "Y supports SK20");
    let support_front = supports
        .iter()
        .map(|s| bounds(s).0.y)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (support_front - blo.y).abs() < 1e-6,
        "the front supports stand at the base's front edge"
    );
    let (y_minus, y_plus) = travel(&supports, &blocks, |v| v.y);
    println!("y travel -{y_minus} +{y_plus}");
    let (blank_lo, blank_hi) = bounds(body(&r, "Blank"));
    let (plate_lo, plate_hi) = bounds(body(&r, "Pattern plate"));
    assert!(
        bit.y - y_minus <= plate_lo.y + 5.0 - 10.0,
        "the pilot reaches the reference hole with 10 to spare"
    );
    assert!(
        bit.y + y_plus >= blank_hi.y + 10.0,
        "the bit reaches the blank's back edge with 10 to spare"
    );
    assert!(bit.y - y_minus <= blank_lo.y && bit.y + y_plus >= blank_hi.y);
    assert!(bit.y - y_minus <= plate_lo.y + 10.0 && bit.y + y_plus >= plate_hi.y - 10.0);
    // The legs: a foot on each rail's blocks and a cheek on it, outboard
    // of everything the X slide sweeps; the deck strip on the back blocks
    // behind the wall; every block under a foot or the deck.
    let legs = group(&r, "Legs");
    assert_eq!(legs.len(), 2);
    let deck = body(&r, "Deck");
    let (deck_lo, deck_hi) = bounds(deck);
    println!(
        "deck strip {} deep, {} above the blank",
        deck_hi.y - deck_lo.y,
        deck_lo.z - blank_hi.z
    );
    assert!(deck_hi.y - deck_lo.y < 160.0 && deck_lo.z - blank_hi.z > 40.0);
    let wall = body(&r, "Wall");
    let (wall_lo, wall_hi) = bounds(wall);
    assert!(
        (wall_lo.y - deck_lo.y).abs() < 1e-6,
        "the wall stands on the deck strip's front edge"
    );
    assert!((wall_lo.z - deck_hi.z).abs() < 1e-6);
    for leg in &legs {
        let (lo, hi) = bounds(leg);
        assert!(
            (lo.z - deck_lo.z).abs() < 1e-6,
            "the foot is at the blocks' tops, the deck's height"
        );
        assert!(
            (hi.z - wall_hi.z).abs() < 1e-6,
            "the cheek is the wall's height"
        );
        assert!(
            lo.y < bit.y + 30.0,
            "the foot starts within 30 of the tools"
        );
        // The cheek butts the wall's end: the wall spans exactly between the cheeks.
        let inner = if lo.x > 0.0 { lo.x } else { hi.x };
        let cheek_inner = if lo.x > 0.0 { hi.x - 19.0 } else { lo.x + 19.0 };
        assert!(
            (cheek_inner - if lo.x > 0.0 { wall_hi.x } else { wall_lo.x }).abs() < 1e-6,
            "wall to cheek"
        );
        assert!(
            inner.abs() > 400.0,
            "the foot is outboard of the X slide's sweep"
        );
    }
    for b in &blocks {
        let (lo, hi) = bounds(b);
        let under_deck = (hi.z - deck_lo.z).abs() < 1e-6
            && deck_lo.y <= lo.y
            && hi.y <= deck_hi.y
            && deck_lo.x <= lo.x
            && hi.x <= deck_hi.x;
        let under_foot = legs.iter().any(|l| {
            let (flo, fhi) = bounds(l);
            (hi.z - flo.z).abs() < 1e-6
                && flo.y <= lo.y
                && hi.y <= fhi.y
                && flo.x <= lo.x
                && hi.x <= fhi.x
        });
        assert!(under_deck || under_foot, "a Y block with nothing on it");
    }
    // The gantry passes over the Y supports at any pose: its lowest parts
    // are the feet and the deck, both above the supports.
    for s in &supports {
        assert!(bounds(s).1.z < deck_lo.z);
    }
    // The X slide's widest part is the tool plate with the stylus's x
    // knob past its end: at the blank's right edge it stops short of the
    // right foot (the sweep test says so at every pose; this says by
    // how much).
    let knob = group(&r, "Stylus block")
        .iter()
        .map(|b| bounds(b).1.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let right_foot = legs
        .iter()
        .map(|l| bounds(l).0.x)
        .filter(|&x| x > 0.0)
        .fold(f64::INFINITY, f64::min);
    let reach = knob + (blank_hi.x - bit.x);
    println!(
        "the x knob reaches {reach} at the blank's right edge, the foot starts at {right_foot}"
    );
    assert!(reach < right_foot - 5.0);
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

// ---------------------------------------------------------------------
// The arm variant: the Woodsmith's own motion. The arm frame rides one
// X shaft on two blocks that slide for X and turn for depth; the head
// hangs on a second shaft across the arm's front on two more blocks, a
// swivel locked by a split clamp; a sand box at the tail balances it.
// ---------------------------------------------------------------------

/// One of the arm's mates: as drawn, and the sign of each parameter
/// that moves its part the positive way (offset: +x, +y or up; angle:
/// the arm's nose down, the head's tips back).
#[derive(Clone, Copy)]
struct ArmMate {
    id: MateId,
    angle: f64,
    offset: f64,
    sign_offset: f64,
    sign_angle: f64,
}

fn arm_mates(doc: &mut Document, tab: TabId) -> std::collections::BTreeMap<String, ArmMate> {
    let asm = doc.assembly(tab).unwrap();
    let mates: Vec<(MateId, String, f64, f64)> = asm
        .mates
        .iter()
        .map(|m| (m.id, m.name.clone(), m.angle, m.offset))
        .collect();
    assert_eq!(mates.len(), 4, "four mates");
    let r0 = doc.regenerate_assembly(tab).unwrap();
    let tip0 = bounds(body(&r0, "Router")).0;
    let mut out = std::collections::BTreeMap::new();
    for (id, name, angle, offset) in mates {
        let (part, axis): (&str, fn(Vec3) -> f64) = match name.as_str() {
            "Y travel" => ("Deck", |v| v.y),
            "arm" => ("Arm frame", |v| v.x),
            "tilt" => ("Tool support", |v| v.x),
            "stop screw" => ("Stop screw", |v| v.z),
            other => panic!("unexpected mate {other}"),
        };
        let mut m = ArmMate {
            id,
            angle,
            offset,
            sign_offset: 0.0,
            sign_angle: 0.0,
        };
        if name != "tilt" {
            let r1 = doc
                .preview_assembly_at(tab, &[(id, angle, offset + 10.0)])
                .unwrap();
            let moved = axis(bounds(body(&r1, part)).0) - axis(bounds(body(&r0, part)).0);
            assert!(
                (moved.abs() - 10.0).abs() < 1e-6,
                "{name} moves {part} by {moved}"
            );
            m.sign_offset = moved.signum();
        }
        if name == "arm" || name == "tilt" {
            let r1 = doc
                .preview_assembly_at(tab, &[(id, angle + 5.0, offset)])
                .unwrap();
            let dz = bounds(body(&r1, "Router")).0.z - tip0.z;
            assert!(dz.abs() > 1.0, "{name} turns the router by {dz}");
            // The arm's nose down lowers the tips; the head's tips back
            // (its top toward the operator) lowers them too, the tips
            // hanging ahead of the head pivot.
            m.sign_angle = -dz.signum();
        }
        out.insert(name, m);
    }
    out
}

/// The arm's angle, degrees nose-down from the drawn pose, at which
/// `f` of the resolved assembly crosses zero (bisection on the kernel).
fn arm_angle_where(
    doc: &mut Document,
    tab: TabId,
    mates: &std::collections::BTreeMap<String, ArmMate>,
    extra: &[(MateId, f64, f64)],
    f: impl Fn(&AssemblyResult) -> f64,
) -> f64 {
    let arm = mates["arm"];
    let at = |a: f64| (arm.id, arm.angle + arm.sign_angle * a, arm.offset);
    let (mut lo, mut hi) = (0.0, 20.0);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        let mut tweaks = vec![at(mid)];
        tweaks.extend_from_slice(extra);
        let r = doc.preview_assembly_at(tab, &tweaks).unwrap();
        if f(&r) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// The arm swings the tools to depth on an arc about the X shaft, 366
/// mm behind the head pivot: the last pass (the pilot on the groove's
/// floor) is 6.4 degrees down, the bit tilted by as much and the tips
/// 9.8 mm back, the bit and the pilot alike since they are one body;
/// the stop screw through the bed lands on the deck at the first
/// pass, 4 mm into the blank, and backs off 3.5 for the last; the X
/// and Y travel reach the blank's edges and the reference hole; and
/// the sand in the tail box balances everything that swings to half a
/// kilogram nose-down at the tool tips, read off the parts' densities.
#[test]
fn the_arm_swings_the_tools_to_depth_on_the_woodsmith_arc_and_balances() {
    let mut doc = load_arm();
    let machine = tab_named(&doc, "Duplicator");
    let mates = arm_mates(&mut doc, machine);
    let r = doc.regenerate_assembly(machine).unwrap();
    let (blank_lo, blank_hi) = bounds(body(&r, "Blank"));
    let (plate_lo, plate_hi) = bounds(body(&r, "Pattern plate"));
    let (bit_lo, bit_hi) = bounds(body(&r, "Router"));
    let bit_x = (bit_lo.x + bit_hi.x) / 2.0;
    let bit_y = (bit_lo.y + bit_hi.y) / 2.0;
    let tip = Vec3::new(bit_x, bit_y, bit_lo.z);
    // Travel: the supports against the blocks, and the bed against the posts.
    let (xm, xp) = travel(
        &group(&r, "X supports SK20"),
        &group(&r, "X blocks SC20UU"),
        |v| v.x,
    );
    let (bed_lo, bed_hi) = bounds(body(&r, "Arm frame"));
    let post_in = group(&r, "Posts")
        .iter()
        .map(|p| {
            let (lo, hi) = bounds(p);
            lo.x.abs().min(hi.x.abs())
        })
        .fold(f64::INFINITY, f64::min);
    let (xm, xp) = (xm.min(post_in + bed_lo.x), xp.min(post_in - bed_hi.x));
    let (ym, yp) = travel(
        &group(&r, "Y supports SK20"),
        &group(&r, "Y blocks SC20UU"),
        |v| v.y,
    );
    println!(
        "X travel -{xm} +{xp} (the bed {} from the posts), Y travel -{ym} +{yp}",
        post_in - bed_hi.x
    );
    assert!(
        xm >= bit_x - blank_lo.x && xp >= blank_hi.x - bit_x,
        "X reach"
    );
    assert!(
        ym >= bit_y - (plate_lo.y + 5.0) && yp >= blank_hi.y - bit_y,
        "Y reach"
    );
    // The arc. The last pass: the tips on the groove's floor, 12.5 below the plate's top.
    let last = arm_angle_where(&mut doc, machine, &mates, &[], |r| {
        bounds(body(r, "Router")).0.z - (plate_hi.z - 12.5)
    });
    let arm = mates["arm"];
    let r_last = doc
        .preview_assembly_at(
            machine,
            &[(arm.id, arm.angle + arm.sign_angle * last, arm.offset)],
        )
        .unwrap();
    let i = r_last
        .bodies
        .iter()
        .position(|b| member(&b.name) == "Router")
        .unwrap();
    let axis = r_last.transforms[&r_last.placed[i]].apply_vector(Vec3::new(0.0, 0.0, 1.0));
    let tilt = axis.z.clamp(-1.0, 1.0).acos().to_degrees();
    let placed_tip = r_last.transforms[&r_last.placed[i]].apply_point(tip);
    let shift = placed_tip.y - tip.y;
    println!(
        "last pass {last:.2} degrees down, the bit tilted {tilt:.2}, the tips {shift:.2} back"
    );
    assert!((5.5..7.0).contains(&last), "last pass {last}");
    assert!((tilt - last).abs() < 1e-6, "the bit tilts with the arm");
    // Ahead of the pivot by 480 they come in 480 (1 - cos), and 115
    // below it they swing back 115 sin: both back, 15.7 together.
    assert!((14.0..18.0).contains(&shift), "the tips move back {shift}");
    // The pilot is the same rigid body as the bit: the same transform.
    let j = r_last
        .bodies
        .iter()
        .position(|b| member(&b.name) == "Pilot")
        .unwrap();
    assert_eq!(r_last.placed[i], r_last.placed[j]);
    // The stop screw as drawn lands on the deck at the first pass, 4 mm in.
    let deck_top = bounds(body(&r, "Deck")).1.z;
    let landing = arm_angle_where(&mut doc, machine, &mates, &[], |r| {
        bounds(body(r, "Stop screw")).0.z - deck_top
    });
    let r_first = doc
        .preview_assembly_at(
            machine,
            &[(arm.id, arm.angle + arm.sign_angle * landing, arm.offset)],
        )
        .unwrap();
    let depth = blank_hi.z - bounds(body(&r_first, "Router")).0.z;
    println!("the screw lands at {landing:.2} degrees, the bit {depth:.2} into the blank");
    // The screw is tilted with the arm, so the rim of its flat tip lands
    // 0.4 mm before its axis would, which is 0.8 at the tools; the
    // operator sets the screw by trial anyway.
    assert!((depth - 4.0).abs() < 1.0, "first pass {depth}");
    // For the last pass it has to back off: how far its tip would be in the deck.
    let into_deck = deck_top - bounds(body(&r_last, "Stop screw")).0.z;
    println!("backed off {into_deck:.2} for the last pass");
    assert!((2.0..6.0).contains(&into_deck), "back off {into_deck}");
    // The balance, about the X shaft's axis, of everything that swings.
    let shaft = bounds(body(&r, "X shafts"));
    let pivot_y = (shaft.0.y + shaft.1.y) / 2.0;
    let (mut moment, mut mass) = (0.0, 0.0);
    for b in r.bodies.iter() {
        let swings =
            b.name.starts_with("Arm / ") || b.name.starts_with("Head / ") || b.name == "Stop screw";
        if !swings {
            continue;
        }
        let m = b
            .material
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no material", b.name))
            .mass_g(b.solid.volume());
        let c = b.solid.centroid().unwrap();
        moment += m * (c.y - pivot_y);
        mass += m;
    }
    let at_tips = moment / (bit_y - pivot_y); // nose-heavy: both negative
    println!(
        "{:.2} kg swings on the pivot, {at_tips:.0} g nose-down at the tool tips",
        mass / 1000.0
    );
    assert!((450.0..550.0).contains(&at_tips), "{at_tips} g at the tips");
    assert!(mass > 15_000.0 && mass < 22_000.0, "{mass} g");
}

/// The arm swept over the work as the other variants are: the bit at
/// the blank's edges and middle, every 25 mm in Y, at the first and the
/// last pass with the screw backed off for the last; then the head
/// tilted 20 degrees each way at the centre with the arm raised or
/// lowered to keep the tips at the drawn height. No two bodies that
/// move differently meet but the tools in the work.
#[test]
fn the_arm_clears_itself_over_the_work_and_at_full_tilt() {
    let mut doc = load_arm();
    let machine = tab_named(&doc, "Duplicator");
    let mates = arm_mates(&mut doc, machine);
    let r = doc.regenerate_assembly(machine).unwrap();
    let (bit_lo, bit_hi) = bounds(body(&r, "Router"));
    let bit = Vec3::new(
        (bit_lo.x + bit_hi.x) / 2.0,
        (bit_lo.y + bit_hi.y) / 2.0,
        bit_lo.z,
    );
    let (blank_lo, blank_hi) = bounds(body(&r, "Blank"));
    let (plate_lo, plate_hi) = bounds(body(&r, "Pattern plate"));
    let deck_top = bounds(body(&r, "Deck")).1.z;
    let xs = [blank_lo.x - bit.x, 0.0, blank_hi.x - bit.x];
    let (y0, y1) = (
        (plate_lo.y + 5.0 - bit.y).min(blank_lo.y - bit.y),
        blank_hi.y - bit.y,
    );
    let steps = ((y1 - y0) / 25.0).ceil() as usize;
    let ys: Vec<f64> = (0..=steps)
        .map(|i| y0 + (y1 - y0) * i as f64 / steps as f64)
        .collect();
    let first = arm_angle_where(&mut doc, machine, &mates, &[], |r| {
        bounds(body(r, "Stop screw")).0.z - deck_top
    });
    let last = arm_angle_where(&mut doc, machine, &mates, &[], |r| {
        bounds(body(r, "Router")).0.z - (plate_hi.z - 12.5)
    });
    let (arm, y, tilt, screw) = (
        mates["arm"],
        mates["Y travel"],
        mates["tilt"],
        mates["stop screw"],
    );
    let r_last = doc
        .preview_assembly_at(
            machine,
            &[(arm.id, arm.angle + arm.sign_angle * last, arm.offset)],
        )
        .unwrap();
    let back_off = deck_top - bounds(body(&r_last, "Stop screw")).0.z + 0.5;
    let mut hits = Vec::new();
    let mut poses = 0;
    for &dx in &xs {
        for &dy in &ys {
            for &(a, off) in &[(first, 0.0), (last, back_off)] {
                poses += 1;
                let r = doc
                    .preview_assembly_at(
                        machine,
                        &[
                            (
                                arm.id,
                                arm.angle + arm.sign_angle * a,
                                arm.offset + arm.sign_offset * dx,
                            ),
                            (y.id, y.angle, y.offset + y.sign_offset * dy),
                            (
                                screw.id,
                                screw.angle,
                                screw.offset + screw.sign_offset * off,
                            ),
                        ],
                    )
                    .unwrap();
                assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
                hits.extend(collisions(&r, &format!("({dx}, {dy}, {a:.1} degrees)")));
            }
        }
    }
    // The tilt: the tips back and forward 20 degrees, the arm swung so
    // the bit's tip stays where it was drawn, 40 above the board.
    for t in [-20.0, 20.0] {
        poses += 1;
        let tilted = [(tilt.id, tilt.angle + tilt.sign_angle * t, tilt.offset)];
        let r_t = doc.preview_assembly_at(machine, &tilted).unwrap();
        let drop = bit.z - bounds(body(&r_t, "Router")).0.z;
        // Nose up (negative) when the tilt lowered the tips: bisection
        // on a signed angle, by trying the arm each way.
        let a = if drop > 0.0 {
            -arm_angle_up(&mut doc, machine, &mates, &tilted, bit.z)
        } else {
            arm_angle_where(&mut doc, machine, &mates, &tilted, |r| {
                bounds(body(r, "Router")).0.z - bit.z
            })
        };
        let r = doc
            .preview_assembly_at(
                machine,
                &[
                    (arm.id, arm.angle + arm.sign_angle * a, arm.offset),
                    tilted[0],
                ],
            )
            .unwrap();
        let tip_z = bounds(body(&r, "Router")).0.z;
        println!(
            "tilt {t}: the arm {a:.2} degrees, the tip at {tip_z:.2} (drawn {:.2})",
            bit.z
        );
        assert!(
            (tip_z - bit.z).abs() < 0.01,
            "the arm takes up the tilt's drop"
        );
        hits.extend(collisions(&r, &format!("tilt {t}")));
    }
    println!("{poses} poses");
    for h in &hits {
        println!("{h}");
    }
    assert!(hits.is_empty(), "{} collisions", hits.len());
}

/// The arm's angle nose-up that brings the bit's tip back to `z`.
fn arm_angle_up(
    doc: &mut Document,
    tab: TabId,
    mates: &std::collections::BTreeMap<String, ArmMate>,
    extra: &[(MateId, f64, f64)],
    z: f64,
) -> f64 {
    let arm = mates["arm"];
    let (mut lo, mut hi) = (0.0, 20.0);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        let mut tweaks = vec![(arm.id, arm.angle - arm.sign_angle * mid, arm.offset)];
        tweaks.extend_from_slice(extra);
        let r = doc.preview_assembly_at(tab, &tweaks).unwrap();
        if bounds(body(&r, "Router")).0.z < z {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}
