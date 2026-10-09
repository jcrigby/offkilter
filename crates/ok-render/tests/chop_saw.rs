//! The angle grinder chop saw (examples/chop-saw): the head swung on its
//! pivot from the depth stop up to 20 degrees, with the cut depth, the
//! gearhead's clearance over the work and interference checked at each
//! position. Every number prints (`--nocapture`); NOTE lines are
//! findings, the requirements are asserted.

use ok_brep::{BoolOp, Solid};
use ok_model::{AssemblyResult, Document, TabId};
use std::path::PathBuf;

fn load() -> Document {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chop-saw");
    let json = std::fs::read_to_string(dir.join("out/chop_saw.okpart")).unwrap();
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

fn lo_z(s: &Solid) -> f64 {
    s.bounds().unwrap().0.z
}

fn hi_z(s: &Solid) -> f64 {
    s.bounds().unwrap().1.z
}

/// Pairs of bodies that overlap by more than a sliver, with the volume;
/// pairs inside one sub-assembly instance (the head's own parts) are
/// rigid together and skipped.
fn hits(r: &AssemblyResult) -> Vec<(String, String, f64)> {
    let mut out = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            if r.placed[i] == r.placed[j] {
                continue;
            }
            let (a, b) = (&r.bodies[i], &r.bodies[j]);
            let (alo, ahi) = a.solid.bounds().unwrap();
            let (blo, bhi) = b.solid.bounds().unwrap();
            if alo.x > bhi.x
                || blo.x > ahi.x
                || alo.y > bhi.y
                || blo.y > ahi.y
                || alo.z > bhi.z
                || blo.z > ahi.z
            {
                continue;
            }
            match ok_brep::boolean(&a.solid, &b.solid, BoolOp::Intersection) {
                Ok(x) if x.volume() > 1e-3 => {
                    out.push((a.name.clone(), b.name.clone(), x.volume()))
                }
                Ok(_) => {}
                Err(_) => out.push((a.name.clone(), b.name.clone(), f64::NAN)),
            }
        }
    }
    out
}

#[test]
fn the_head_cuts_to_the_stop_and_clears_the_work() {
    let mut doc = load();
    let saw = tab_named(&doc, "chop saw");
    let (pivot, angle0, offset) = {
        let m = doc
            .assembly(saw)
            .unwrap()
            .mates
            .iter()
            .find(|m| m.name == "pivot")
            .unwrap();
        (m.id, m.angle, m.offset)
    };
    let at = |doc: &mut Document, a: f64| doc.preview_assembly(saw, pivot, a, offset).unwrap();
    let disc_low = |r: &AssemblyResult| lo_z(body(r, "head / disc"));
    // Which way the mate's angle lifts the head.
    let sign = if disc_low(&at(&mut doc, angle0 + 5.0)) > disc_low(&at(&mut doc, angle0)) {
        1.0
    } else {
        -1.0
    };

    let rod = {
        let r = at(&mut doc, angle0);
        let s = body(&r, "rod");
        (lo_z(s) + hi_z(s)) / 2.0
    };
    let rod_top = rod + 10.0;
    let mut failures = Vec::new();
    let mut must = |ok: bool, what: String| {
        eprintln!("{} {what}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures.push(what);
        }
    };
    for deg in [0.0, 5.0, 10.0, 20.0] {
        let r = at(&mut doc, angle0 + sign * deg);
        assert!(r.mate_errors.is_empty(), "{deg}: {:?}", r.mate_errors);
        assert!(
            r.instance_errors.is_empty(),
            "{deg}: {:?}",
            r.instance_errors
        );
        let low = disc_low(&r);
        let gearhead = lo_z(body(&r, "head / grinder"));
        let v_top = hi_z(body(&r, "outfeed V-block"));
        let all = hits(&r);
        let in_cut = all.iter().any(|(a, b, _)| {
            (a == "head / disc" && b == "rod") || (a == "rod" && b == "head / disc")
        });
        must(
            in_cut == (deg == 0.0),
            format!(
                "up {deg} degrees: the disc {} the rod",
                if in_cut { "is in" } else { "is clear of" }
            ),
        );
        let bad: Vec<_> = all
            .into_iter()
            .filter(|(a, b, _)| {
                !(deg == 0.0
                    && ((a == "head / disc" && b == "rod") || (a == "rod" && b == "head / disc")))
            })
            .collect();
        must(
            bad.is_empty(),
            format!("up {deg} degrees: no interference but the disc in its cut: {bad:?}"),
        );
        // The guard stays well clear of the depth stop however it is
        // turned: the post and its bolt behind the guard's whole circle
        // (62.5 mm about the spindle, build.py GUARD_R).
        let (dlo, dhi) = body(&r, "head / disc").bounds().unwrap();
        let spindle_y = (dlo.y + dhi.y) / 2.0;
        for stop in ["stop post", "stop bolt"] {
            let g = body(&r, stop).bounds().unwrap().0.y - (spindle_y + 62.5);
            must(
                g >= 20.0,
                format!("up {deg} degrees: the {stop} is {g:.1} mm behind the guard's circle"),
            );
        }
        if deg == 0.0 {
            // The boss is a 10 mm wall into the gearcase: the bolt's tip
            // must stay inside it.
            let reach = lo_z(body(&r, "head / arm")) - lo_z(body(&r, "head / boss bolt"));
            must(
                (8.0..10.0).contains(&reach),
                format!(
                    "the boss bolt reaches {reach:.1} mm below the arm, inside the 10 mm boss wall"
                ),
            );
            must(
                (rod - low - 0.5).abs() < 0.05,
                format!(
                    "on the stop the disc is {:.2} mm past the rod's axis (0.5 wanted)",
                    rod - low
                ),
            );
            must(
                gearhead - rod_top >= 8.0,
                format!(
                    "on the stop the grinder is {:.1} mm over the rod",
                    gearhead - rod_top
                ),
            );
            must(
                gearhead - v_top >= 8.0,
                format!(
                    "on the stop the grinder is {:.1} mm over the V-blocks",
                    gearhead - v_top
                ),
            );
            eprintln!(
                "NOTE a disc worn to {:.0} mm diameter puts the gearhead on the rod (the parts list says change it 2 mm before)",
                114.3 - 2.0 * (gearhead - rod_top)
            );
        }
        if deg == 20.0 {
            must(
                low - rod_top >= 25.0,
                format!(
                    "raised 20 degrees the disc is {:.1} mm over the rod",
                    low - rod_top
                ),
            );
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
