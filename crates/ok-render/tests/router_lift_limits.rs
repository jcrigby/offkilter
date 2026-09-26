//! Mechanism-at-limit checks of the router lift (examples/router-lift),
//! the definition of done from the design brief: the carriage swept over
//! its travel and the pin arm over its hinge, with clearances, alignment
//! and interference checked at every position, plus the carriage's own
//! features. Every number prints (`--nocapture`); a line marked NOTE is
//! a finding the model reports rather than a requirement, and the
//! requirements are asserted at the end.

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, InstanceId, MateId, TabId};
use std::path::PathBuf;

fn load() -> Document {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/router-lift");
    let json = std::fs::read_to_string(dir.join("out/router_lift.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn mate_named(doc: &Document, tab: TabId, name: &str) -> (MateId, f64, f64) {
    let m = doc
        .assembly(tab)
        .unwrap()
        .mates
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no mate {name}"));
    (m.id, m.angle, m.offset)
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

/// Cylindrical surfaces of a solid: (origin, unit axis, radius).
fn cylinders(s: &Solid) -> Vec<(Vec3, Vec3, f64)> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => Some((*origin, axis.normalized().unwrap_or(Vec3::Z), *radius)),
            _ => None,
        })
        .collect()
}

fn boxes_touch(a: &Solid, b: &Solid, slack: f64) -> bool {
    let (alo, ahi) = bounds(a);
    let (blo, bhi) = bounds(b);
    !(alo.x > bhi.x + slack
        || blo.x > ahi.x + slack
        || alo.y > bhi.y + slack
        || blo.y > ahi.y + slack
        || alo.z > bhi.z + slack
        || blo.z > ahi.z + slack)
}

/// Overlap volume of two placed bodies, `None` when the boolean fails.
fn overlap(a: &Solid, b: &Solid) -> Option<f64> {
    if !boxes_touch(a, b, 1e-6) {
        return Some(0.0);
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .ok()
        .map(|x| x.volume())
}

/// Pairs of placed bodies that overlap by more than a sliver, with the
/// volume, limited to pairs where at least one name passes `of`.
fn hits(r: &AssemblyResult, of: impl Fn(&str) -> bool) -> Vec<(String, String, f64)> {
    let mut out = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            if r.placed[i] == r.placed[j] {
                continue; // one sub-assembly instance's own bodies
            }
            let (a, b) = (&r.bodies[i], &r.bodies[j]);
            if !of(&a.name) && !of(&b.name) {
                continue;
            }
            match overlap(&a.solid, &b.solid) {
                Some(v) if v > 1e-3 => out.push((a.name.clone(), b.name.clone(), v)),
                Some(_) => {}
                None => out.push((a.name.clone(), b.name.clone(), f64::NAN)),
            }
        }
    }
    out
}

/// Body pairs that overlap as drawn and are reported rather than hidden,
/// with why. The guide pin, 75 mm long with 45 mm out of the chuck, has
/// its top 5 mm into the nose (the SCAD draws it so; a clearance hole in
/// the nose or less pin out fixes it). The leveling bolt's tip rests on
/// the table when the arm is level, so lifting the nose, which turns
/// the tail down, drives the bolt into the top from the first degree:
/// the sweep reports its own contacts apart from that.
const KNOWN: &[(&str, &str, &str)] = &[
    ("arm", "guide pin", "the pin's top in the nose, as drawn"),
    (
        "router",
        "Arm assembly",
        "the bit into the guide pin, which is drawn down in the alignment ring: retract the router for that check, set the pin higher for cutting",
    ),
    // The same pair as the arm sweep names it, by member: with the router
    // seated for bit changes its bit stands 12 mm over the table at mid
    // travel, so the pin drawn down for the alignment check meets it
    // there too, not only at max rise.
    (
        "router",
        "guide pin",
        "the bit into the guide pin, which is drawn down in the alignment ring: retract the router for that check, set the pin higher for cutting",
    ),
    (
        "leveling bolt",
        "top",
        "the tail's stop against lifting the nose",
    ),
];

/// A body's own name without its sub-assembly prefix.
fn member(name: &str) -> &str {
    name.rsplit(" / ").next().unwrap_or(name)
}

fn known(a: &str, b: &str) -> Option<&'static str> {
    let (a, b) = (member(a), member(b));
    KNOWN
        .iter()
        .find(|(p, q, _)| (a == *p && b == *q) || (a == *q && b == *p))
        .map(|k| k.2)
}

struct Report {
    lines: Vec<(bool, String)>,
    failures: Vec<String>,
}

impl Report {
    fn new() -> Report {
        Report {
            lines: Vec::new(),
            failures: Vec::new(),
        }
    }
    fn must(&mut self, ok: bool, what: &str, detail: String) {
        self.lines.push((
            true,
            format!("{} {what}: {detail}", if ok { "PASS" } else { "FAIL" }),
        ));
        if !ok {
            self.failures.push(format!("{what}: {detail}"));
        }
    }
    fn note(&mut self, what: &str, detail: String) {
        self.lines.push((false, format!("NOTE {what}: {detail}")));
    }
    fn print(&self) {
        for (_, l) in &self.lines {
            eprintln!("{l}");
        }
    }
}

#[test]
fn the_lift_holds_its_clearances_over_the_travel() {
    let mut doc = load();
    let lift = tab_named(&doc, "Lift assembly");
    let (slider, _, mid) = mate_named(&doc, lift, "carriage travel");
    let mut rep = Report::new();

    // Which way the slider's offset raises the carriage.
    let at =
        |doc: &mut Document, offset: f64| doc.preview_assembly(lift, slider, 0.0, offset).unwrap();
    let z_of = |r: &AssemblyResult| bounds(body(r, "Carriage assembly / carriage")).0.z;
    let up = if z_of(&at(&mut doc, mid + 1.0)) > z_of(&at(&mut doc, mid)) {
        1.0
    } else {
        -1.0
    };
    // The design travel: from the carriage 3 mm above the lower supports
    // to 3 mm under the top (build.py: TRAVEL = 45).
    let travel = 45.0;
    let positions = [
        ("min", mid - up * travel / 2.0),
        ("mid", mid),
        ("max", mid + up * travel / 2.0),
    ];
    let names: std::collections::BTreeMap<InstanceId, String> = doc
        .assembly(lift)
        .unwrap()
        .instances
        .iter()
        .map(|i| (i.id, i.name.clone()))
        .collect();
    for (label, offset) in positions {
        let r = at(&mut doc, offset);
        assert!(r.mate_errors.is_empty(), "{label}: {:?}", r.mate_errors);
        assert!(
            r.instance_errors.is_empty(),
            "{label}: {:?}",
            r.instance_errors
        );
        // Interference across every pair of placed bodies.
        let (overlaps, failed) = r.interferences();
        let mut bad = Vec::new();
        for o in &overlaps {
            let (a, b) = (&names[&o.a], &names[&o.b]);
            if let Some(why) = known(a, b) {
                rep.note(
                    &format!("interference at {label}"),
                    format!("{a} / {b}: {:.0} mm3 ({why})", o.volume),
                );
            } else {
                bad.push(format!("{a} / {b}: {:.1} mm3", o.volume));
            }
        }
        rep.must(
            bad.is_empty() && failed.is_empty(),
            &format!("no interference at {label}"),
            if bad.is_empty() && failed.is_empty() {
                format!(
                    "{} pairs checked",
                    r.bodies.len() * (r.bodies.len() - 1) / 2
                )
            } else {
                format!("{bad:?}, {} booleans failed", failed.len())
            },
        );
        let carriage = body(&r, "Carriage assembly / carriage");
        let (clo, chi) = bounds(carriage);
        if label == "max" {
            let top_under = bounds(body(&r, "top")).0.z;
            let gap = top_under - chi.z;
            rep.must(
                gap >= 3.0 - 1e-6,
                "carriage top to top underside at max rise",
                format!("{gap:.2} mm"),
            );
            // The router through the opening with no ring: everything of
            // it above the top's underside must clear the 74 mm bore.
            let router = body(&r, "router");
            let reach = router
                .vertices
                .iter()
                .filter(|v| v.z >= top_under - 1e-9)
                .map(|v| (v.x * v.x + v.y * v.y).sqrt())
                .fold(0.0, f64::max);
            rep.must(
                reach <= 37.0,
                "router clears the 74 mm opening at max rise",
                format!(
                    "largest radius above the top's underside {reach:.2} mm, clearance {:.2} mm",
                    37.0 - reach
                ),
            );
            // What the router reaches at max rise: the instructions change
            // bits with the collet nut raised above the top, and the bit's
            // working height follows from the nut's. The nut and bit are
            // the ghost's assumptions until the real router is measured.
            let table = bounds(body(&r, "top")).1.z;
            let bit_tip = bounds(router).1.z;
            let nut_top = router
                .faces
                .iter()
                .filter(|f| f.plane.normal.z > 0.999)
                .map(|f| {
                    f.loops[0]
                        .iter()
                        .map(|&k| router.vertices[k as usize].z)
                        .fold(f64::NEG_INFINITY, f64::max)
                })
                .filter(|z| *z < bit_tip - 1e-6)
                .fold(f64::NEG_INFINITY, f64::max);
            rep.must(
                nut_top - table >= 5.0 - 1e-6,
                "collet nut top at max rise",
                format!(
                    "{:.1} mm above the table surface, for the wrenches",
                    nut_top - table
                ),
            );
            rep.note(
                "bit tip at max rise",
                format!("{:.1} mm above the table surface", bit_tip - table),
            );
            // The router sits in the clamp band where the nut's height
            // puts it. How much housing the band grips, and how much of
            // the assumed housing stands above it, are what to check
            // against the real router's cylindrical part.
            let carriage = body(&r, "Carriage assembly / carriage");
            let (car_lo, car_hi) = bounds(carriage);
            let band = (
                (car_lo.z + car_hi.z) / 2.0 - 33.0,
                (car_lo.z + car_hi.z) / 2.0 + 33.0,
            );
            let housing_bottom = bounds(router).0.z;
            let housing_top = nut_top - 16.0;
            let gripped = (housing_top.min(band.1) - housing_bottom.max(band.0)).max(0.0);
            rep.must(
                gripped >= 50.0,
                "housing in the 66 mm clamp band",
                format!(
                    "{gripped:.0} mm gripped; the housing's bottom is {:.0} mm above the band's bottom, and {:.0} mm of housing stands above the band",
                    housing_bottom - band.0,
                    housing_top - band.1
                ),
            );
            // The crank nut sits in a recess in the top, its top under the
            // surface, with ply left between the recess floor and the
            // upper bearing pocket.
            let nut_top = bounds(body(&r, "Leadscrew assembly / coupling nut")).1.z;
            rep.must(
                nut_top <= table - 0.5,
                "crank nut recessed under the table surface",
                format!("top {:.1} mm below the surface", table - nut_top),
            );
            let top = body(&r, "top");
            let nut = body(&r, "Leadscrew assembly / coupling nut")
                .centroid()
                .unwrap();
            // Horizontal faces of the top around the leadscrew axis, by the
            // height of their own vertices (a plane's origin can be
            // anywhere on the plane).
            let level = |f: &ok_brep::Face| -> Option<f64> {
                let vs: Vec<Vec3> = f.loops[0]
                    .iter()
                    .map(|&k| top.vertices[k as usize])
                    .collect();
                let near = vs
                    .iter()
                    .all(|v| ((v.x - nut.x).powi(2) + (v.y - nut.y).powi(2)).sqrt() < 12.0);
                near.then(|| vs[0].z)
            };
            let floor = top
                .faces
                .iter()
                .filter(|f| f.plane.normal.z > 0.999)
                .filter_map(level)
                .filter(|z| *z < table - 1.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let pocket = top
                .faces
                .iter()
                .filter(|f| f.plane.normal.z < -0.999)
                .filter_map(level)
                .filter(|z| *z > top_under + 1.0)
                .fold(f64::NEG_INFINITY, f64::max);
            rep.must(
                floor - pocket >= 8.0,
                "ply between the crank recess and the bearing pocket",
                format!(
                    "{:.1} mm (recess {:.0} deep)",
                    floor - pocket,
                    table - floor
                ),
            );
        }
        if label == "min" {
            let sk20_top = bounds(body(&r, "left lower support")).1.z;
            let gap = clo.z - sk20_top;
            rep.must(
                gap >= 3.0 - 1e-6,
                "carriage bottom to the lower SK20 at min",
                format!("{gap:.2} mm"),
            );
            let table = bounds(body(&r, "top")).1.z;
            let bit_tip = bounds(body(&r, "router")).1.z;
            rep.note(
                "bit tip at min rise",
                format!("{:.1} mm above the table surface", bit_tip - table),
            );
        }
        // Leadscrew parallel to both shafts: the angle between the axes,
        // as a deviation over the travel.
        let screw = cylinders(body(&r, "Leadscrew assembly / leadscrew"))
            .into_iter()
            .find(|c| (c.2 - 4.0).abs() < 1e-6)
            .expect("leadscrew cylinder");
        for shaft in ["left shaft", "right shaft"] {
            let s = cylinders(body(&r, shaft))
                .into_iter()
                .find(|c| (c.2 - 10.0).abs() < 1e-6)
                .expect("shaft cylinder");
            let sin = screw.1.cross(s.1).length();
            let dev = travel * sin;
            rep.must(
                dev <= 0.05,
                &format!("leadscrew parallel to {shaft} at {label}"),
                format!("{:.4} mm over {travel} mm of travel", dev),
            );
        }
    }
    // The sub-assemblies' own bodies against each other.
    for sub in ["Carriage assembly", "Leadscrew assembly", "Arm assembly"] {
        let tab = tab_named(&doc, sub);
        let r = doc.regenerate_assembly(tab).unwrap();
        let sub_names: std::collections::BTreeMap<InstanceId, String> = doc
            .assembly(tab)
            .unwrap()
            .instances
            .iter()
            .map(|i| (i.id, i.name.clone()))
            .collect();
        let (overlaps, failed) = r.interferences();
        let mut list = Vec::new();
        for o in &overlaps {
            let (a, b) = (&sub_names[&o.a], &sub_names[&o.b]);
            match known(a, b) {
                Some(why) => rep.note(
                    &format!("interference inside {sub}"),
                    format!("{a} / {b}: {:.1} mm3 ({why})", o.volume),
                ),
                None => list.push(format!("{a} / {b}: {:.1} mm3", o.volume)),
            }
        }
        rep.must(
            list.is_empty() && failed.is_empty(),
            &format!("no interference inside {sub}"),
            if list.is_empty() {
                format!("{} bodies", r.bodies.len())
            } else {
                format!("{list:?}")
            },
        );
    }
    rep.print();
    assert!(rep.failures.is_empty(), "{:#?}", rep.failures);
}

#[test]
fn the_carriage_features_are_where_the_shop_needs_them() {
    let mut doc = load();
    let tab = tab_named(&doc, "Carriage");
    let r = doc.regenerate_studio(tab, None).unwrap();
    assert_eq!(r.bodies.len(), 1);
    let s = &r.bodies[0].solid;
    s.validate().unwrap();
    let mut rep = Report::new();
    // Volume against the reference mesh's (OpenSCAD, $fn = 96).
    let reference = 775_369.0;
    let v = s.volume();
    rep.must(
        ((v - reference) / reference).abs() < 0.005,
        "carriage volume",
        format!(
            "{v:.0} mm3, reference {reference:.0} ({:+.2} %)",
            100.0 * (v - reference) / reference
        ),
    );
    // The router bore (65 + 0.3 clearance) is one clean cylinder: no face
    // on it has a hole in it, and the clamp slot is the only break.
    let parts = s.face_parts();
    let bore: Vec<usize> = s
        .surfaces
        .iter()
        .enumerate()
        .filter(|(_, sf)| matches!(sf, Surface::Cylinder { radius, axis, .. } if (radius - 32.8).abs() < 1e-6 && axis.z.abs() > 0.999))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(bore.len(), 1, "one bore surface");
    let bore_faces: Vec<usize> = (0..s.faces.len())
        .filter(|&i| s.faces[i].surface == bore[0])
        .collect();
    let holed = bore_faces
        .iter()
        .filter(|&&i| s.faces[i].loops.len() > 1)
        .count();
    let mut pieces: Vec<u32> = bore_faces.iter().map(|&i| parts[i]).collect();
    pieces.sort_unstable();
    pieces.dedup();
    rep.must(
        holed == 0 && pieces.len() == 1,
        "nut traps stay out of the router bore",
        format!(
            "{} bore facets, {holed} with holes, {} piece(s)",
            bore_faces.len(),
            pieces.len()
        ),
    );
    // Trap walls (planar, normal along X) sit outside the bore by a wall.
    let nearest = s
        .faces
        .iter()
        .filter(|f| f.plane.normal.x.abs() > 0.999)
        .map(|f| f.plane.origin.x.abs())
        .filter(|x| *x > 32.8 && *x < 50.0)
        .fold(f64::INFINITY, f64::min);
    rep.note(
        "wall between the traps and the bore",
        format!("{:.1} mm", nearest - 32.8),
    );
    // Every block bolt hole (M5 + 0.5 clearance, along X) reaches its
    // trap: the hole's cylinder is split in two by the slot.
    let holes: Vec<usize> = s
        .surfaces
        .iter()
        .enumerate()
        .filter(|(_, sf)| matches!(sf, Surface::Cylinder { radius, axis, .. } if (radius - 2.75).abs() < 1e-6 && axis.x.abs() > 0.999))
        .map(|(i, _)| i)
        .collect();
    let split = holes
        .iter()
        .filter(|&&si| {
            let mut p: Vec<u32> = (0..s.faces.len())
                .filter(|&i| s.faces[i].surface == si)
                .map(|i| parts[i])
                .collect();
            p.sort_unstable();
            p.dedup();
            p.len() >= 2
        })
        .count();
    rep.must(
        holes.len() == 16 && split == 16,
        "bolt holes reach the nut traps",
        format!("{} holes, {split} split by a trap", holes.len()),
    );
    rep.print();
    assert!(rep.failures.is_empty(), "{:#?}", rep.failures);
}

#[test]
fn the_pin_arm_swings_clear_and_lands_on_the_bit() {
    let mut doc = load();
    let lift = tab_named(&doc, "Lift assembly");
    let (pivot, angle0, offset) = mate_named(&doc, lift, "arm pivot");
    let mut rep = Report::new();
    let at = |doc: &mut Document, a: f64| doc.preview_assembly(lift, pivot, a, offset).unwrap();
    let r = at(&mut doc, angle0);
    let table = bounds(body(&r, "top")).1.z;
    let arm = body(&r, "Arm assembly / arm");
    let pin = body(&r, "Arm assembly / guide pin");
    let chuck = body(&r, "Arm assembly / chuck");
    let bolt = body(&r, "Arm assembly / leveling bolt");
    // Level: the guide pin on the bit axis, its tip just above the table.
    let axis = cylinders(pin)
        .into_iter()
        .find(|c| (c.2 - 3.175).abs() < 1e-6)
        .expect("pin cylinder");
    let off = (axis.0.x * axis.0.x + axis.0.y * axis.0.y).sqrt();
    rep.must(
        off <= 0.1,
        "guide pin coaxial with the bit",
        format!("{off:.4} mm off axis"),
    );
    let tip = bounds(pin).0.z - table;
    rep.must(
        (0.0..=45.0).contains(&tip),
        "pin tip above the table as drawn",
        format!("{tip:.1} mm"),
    );
    let pin_len = bounds(pin).1.z - bounds(pin).0.z;
    let (chuck_lo, _) = bounds(chuck);
    let underside = bounds(arm).0.z;
    rep.note(
        "pin tip range over the chuck adjustment",
        format!(
            "{:.0} mm (top flush with the nose) down to {:.0} mm (top at the chuck's floor)",
            underside - pin_len - table,
            chuck_lo.z - pin_len - table
        ),
    );
    let nose = underside - table;
    rep.must(
        (75.0..=77.0).contains(&nose),
        "arm underside above the table at the nose",
        format!("{nose:.1} mm"),
    );
    let bolt_tip = bounds(bolt).0.z - table;
    rep.note(
        "leveling bolt tip, level",
        format!("{bolt_tip:.2} mm above the table"),
    );
    // What stands in the stock envelope: on the table, under the arm,
    // forward of the pivot line; and how far behind the bit stock can
    // reach before it meets something.
    let pivot_y = body(&r, "pivot shaft").centroid().unwrap().y;
    let mut standing = Vec::new();
    let mut depth = f64::INFINITY;
    for b in &r.bodies {
        let (lo, hi) = bounds(&b.solid);
        let on_table = hi.z > table + 1e-6 && lo.z < table + 76.0;
        if b.name == "ring" || !on_table {
            continue;
        }
        if lo.y < pivot_y - 1e-6 {
            standing.push(format!(
                "{} (y {:.0}..{:.0}, {:.0}..{:.0} mm up)",
                b.name,
                lo.y,
                hi.y.min(pivot_y),
                (lo.z - table).max(0.0),
                hi.z - table
            ));
        }
        if lo.y > 0.0 && !b.name.starts_with("Arm assembly /") {
            depth = depth.min(lo.y);
        }
    }
    rep.note(
        "in the stock envelope forward of the pivot line",
        if standing.is_empty() {
            "nothing".into()
        } else {
            standing.join("; ")
        },
    );
    rep.note(
        "stock depth behind the bit on the table",
        format!("{depth:.0} mm to the nearest thing standing on the table"),
    );
    // Sweep the pivot: the nose lifted 0..85 degrees, nothing the arm
    // carries touching the table, the supports or the router, and the
    // angle at which the tail first meets the table.
    let nose_z = |r: &AssemblyResult| bounds(body(r, "Arm assembly / guide pin")).0.z;
    let sign = if nose_z(&at(&mut doc, angle0 + 5.0)) > nose_z(&at(&mut doc, angle0)) {
        1.0
    } else {
        -1.0
    };
    let mut first_hit: Option<(f64, String)> = None;
    let mut bolt_in = None;
    for step in 0..=17 {
        let theta = 5.0 * step as f64;
        let r = at(&mut doc, angle0 + sign * theta);
        assert!(r.mate_errors.is_empty(), "{theta}: {:?}", r.mate_errors);
        let all = hits(&r, |n| n.starts_with("Arm assembly /"));
        let found: Vec<_> = all
            .iter()
            .filter(|(a, b, _)| known(a, b).is_none())
            .collect();
        if theta == 0.0 {
            rep.must(
                found.is_empty(),
                "arm level touches nothing",
                if found.is_empty() {
                    "clear".into()
                } else {
                    format!("{found:?}")
                },
            );
        }
        if let Some((a, b, v)) = found.first() {
            if first_hit.is_none() {
                first_hit = Some((theta, format!("{a} / {b}: {v:.0} mm3 at {theta} degrees")));
            }
        }
        if bolt_in.is_none() {
            let below = table - bounds(body(&r, "Arm assembly / leveling bolt")).0.z;
            if below > 1e-6 {
                bolt_in = Some(format!("{below:.1} mm into the top at {theta} degrees"));
            }
        }
        if theta == 0.0 || theta == 45.0 || theta == 85.0 {
            rep.note(
                &format!("arm at {theta} degrees"),
                format!(
                    "pin tip {:.1} mm above the table, {} contact(s)",
                    nose_z(&r) - table,
                    found.len()
                ),
            );
        }
    }
    rep.note(
        "leveling bolt through the sweep",
        bolt_in.unwrap_or_else(|| "never below the table".into()),
    );
    // The tail is meant to meet the table at about 85 degrees, so the
    // sweep must be clear through 80 and the first contact is reported.
    rep.must(
        first_hit.as_ref().is_none_or(|(t, _)| *t > 80.0),
        "arm swings 0..80 degrees clear of the table and the supports",
        "17 positions".into(),
    );
    rep.note(
        "tail first meets the table",
        match &first_hit {
            None => "not within 85 degrees".into(),
            Some((_, what)) => what.clone(),
        },
    );
    rep.print();
    assert!(rep.failures.is_empty(), "{:#?}", rep.failures);
}
