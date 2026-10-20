//! The sliding cutoff grinder (examples/cutoff-grinder): the carriage
//! slid to its stops and the handle swung from the stop to full plunge,
//! with interference, the wheel over the channel, the cut depth, the
//! gearhead's clearance, the stop's catch, the pins in their slots and
//! the stock capacity in front of the fence checked at each position.
//! Every number prints (`--nocapture`); NOTE lines are findings, the
//! requirements are asserted.

use ok_brep::{BoolOp, Solid};
use ok_model::{AssemblyResult, Document, MateId, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;

fn load() -> Document {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cutoff-grinder");
    let json = std::fs::read_to_string(dir.join("out/cutoff_grinder.okpart")).unwrap();
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

fn lo(s: &Solid) -> ok_math::Vec3 {
    s.bounds().unwrap().0
}

fn hi(s: &Solid) -> ok_math::Vec3 {
    s.bounds().unwrap().1
}

/// Pairs of bodies that overlap by more than a sliver, with the volume;
/// pairs inside one sub-assembly instance are rigid together and skipped.
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

fn pair(h: &[(String, String, f64)], a: &str, b: &str) -> bool {
    h.iter()
        .any(|(p, q, _)| (p == a && q == b) || (p == b && q == a))
}

struct Mates {
    slide: (MateId, f64, f64),
    pivot: (MateId, f64, f64),
    slide_sign: f64,
    pivot_sign: f64,
}

/// The assembly with the carriage `dy` inches from the drawn pose and
/// the handle `deg` degrees down from rest.
fn at(doc: &mut Document, saw: TabId, m: &Mates, dy: f64, deg: f64) -> AssemblyResult {
    let (s, sa, so) = m.slide;
    let (p, pa, po) = m.pivot;
    doc.preview_assembly_at(
        saw,
        &[
            (s, sa, so + m.slide_sign * dy * IN),
            (p, pa + m.pivot_sign * deg, po),
        ],
    )
    .unwrap()
}

#[test]
fn the_carriage_slides_and_the_grinder_plunges_clear_of_everything() {
    let mut doc = load();
    let saw = tab_named(&doc, "sliding cutoff grinder");
    let mut m = {
        let asm = doc.assembly(saw).unwrap();
        let find = |name: &str| {
            let x = asm.mates.iter().find(|m| m.name == name).unwrap();
            (x.id, x.angle, x.offset)
        };
        Mates {
            slide: find("slide"),
            pivot: find("pivot"),
            slide_sign: 1.0,
            pivot_sign: 1.0,
        }
    };
    // Which way each mate's value moves the wheel: +dy slides it back,
    // +deg brings it down.
    let wheel_y = |r: &AssemblyResult| lo(body(r, "handle assembly / wheel")).y;
    let wheel_z = |r: &AssemblyResult| lo(body(r, "handle assembly / wheel")).z;
    let home = at(&mut doc, saw, &m, 0.0, 0.0);
    if wheel_y(&at(&mut doc, saw, &m, 0.5, 0.0)) < wheel_y(&home) {
        m.slide_sign = -1.0;
    }
    if wheel_z(&at(&mut doc, saw, &m, 0.0, 5.0)) > wheel_z(&home) {
        m.pivot_sign = -1.0;
    }
    let base_top = hi(body(&home, "base")).z;
    let channel = body(&home, "channel");
    let floor = lo(channel).z + IN / 8.0;
    let (ch_lo, ch_hi) = (lo(channel).x + IN / 8.0, hi(channel).x - IN / 8.0);
    let fence_front = lo(body(&home, "left fence")).y;
    let leg_top = base_top + IN / 8.0; // the fence's horizontal leg, which the gearhead passes over

    let mut failures = Vec::new();
    let mut must = |ok: bool, what: String| {
        eprintln!("{} {what}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures.push(what);
        }
    };

    // Full plunge: the wheel 1 mm off the channel's floor or the grinder
    // 1.5 mm off the fence's leg, whichever comes first.
    let (mut lo_deg, mut hi_deg) = (0.0f64, 40.0f64);
    for _ in 0..30 {
        let mid = (lo_deg + hi_deg) / 2.0;
        let r = at(&mut doc, saw, &m, 0.0, mid);
        let ok = wheel_z(&r) > floor + 1.0
            && lo(body(&r, "handle assembly / grinder")).z > leg_top + 1.5;
        if ok {
            lo_deg = mid;
        } else {
            hi_deg = mid;
        }
    }
    let plunge = lo_deg;
    eprintln!("NOTE full plunge is {plunge:.1} degrees");
    must(
        plunge > 15.0 && plunge < 35.0,
        format!("the handle swings {plunge:.1} degrees from rest to full plunge"),
    );

    // The stops: how far the carriage slides each way before a pin meets
    // the closed end of its slot.
    let overlap = |r: &AssemblyResult, a: &str, b: &str| {
        ok_brep::boolean(body(r, a), body(r, b), BoolOp::Intersection)
            .map(|x| x.volume() > 1e-3)
            .unwrap_or(true)
    };
    let mut stop = |doc: &mut Document, sign: f64, pin: &str| {
        let (mut free, mut hit) = (0.0f64, 6.0f64);
        for _ in 0..16 {
            let mid = (free + hit) / 2.0;
            if overlap(
                &at(doc, saw, &m, sign * mid, 0.0),
                pin,
                "carriage / carriage plate",
            ) {
                hit = mid;
            } else {
                free = mid;
            }
        }
        free
    };
    let forward = stop(&mut doc, -1.0, "front pin");
    let backward = stop(&mut doc, 1.0, "back pin");
    eprintln!("NOTE the carriage slides {forward:.2} in forward and {backward:.2} in back from the drawn pose: {:.2} in of travel", forward + backward);
    must(
        forward + backward >= 4.0,
        format!("the travel is {:.2} in", forward + backward),
    );
    let travel = forward.min(backward) - 0.05;
    let positions = [-forward + 0.05, 0.0, backward - 0.05];
    for dy in positions {
        for deg in [0.0, plunge / 2.0, plunge] {
            let r = at(&mut doc, saw, &m, dy, deg);
            assert!(r.mate_errors.is_empty(), "{dy} {deg}: {:?}", r.mate_errors);
            assert!(
                r.instance_errors.is_empty(),
                "{dy} {deg}: {:?}",
                r.instance_errors
            );
            let where_ = format!("carriage {dy:+.0} in, down {deg:.1} degrees");
            let bad = hits(&r);
            must(
                bad.is_empty(),
                format!("{where_}: no interference: {bad:?}"),
            );
            let w = body(&r, "handle assembly / wheel");
            must(
                lo(w).x > ch_lo && hi(w).x < ch_hi,
                format!(
                    "{where_}: the wheel runs between the channel's walls ({:.2} to {:.2} of {:.2} to {:.2} mm)",
                    lo(w).x,
                    hi(w).x,
                    ch_lo,
                    ch_hi
                ),
            );
            let g = lo(body(&r, "handle assembly / grinder")).z - leg_top;
            must(
                g >= 1.0,
                format!("{where_}: the gearhead is {:.1} mm over the fence's leg", g),
            );
            if deg == 0.0 {
                let clear = (wheel_z(&r) - base_top) / IN;
                must(
                    clear >= 1.0,
                    format!("{where_}: at rest the wheel is {clear:.2} in over the base, room for stock under it"),
                );
            }
            if deg == plunge {
                let depth = (base_top - wheel_z(&r)) / IN;
                must(
                    depth >= 0.3,
                    format!("{where_}: plunged, the wheel reaches {depth:.2} in below the base's top, through anything lying on it"),
                );
                must(
                    wheel_z(&r) > floor,
                    format!(
                        "{where_}: and stays {:.1} mm off the channel's floor",
                        wheel_z(&r) - floor
                    ),
                );
            }
        }
    }

    // Capacity: stock against the fence, from the wheel's forward reach
    // at the forward stop to its back reach at the back stop.
    let reach = |r: &AssemblyResult, back: bool| {
        let w = body(r, "handle assembly / wheel");
        let (l, h) = (lo(w), hi(w));
        let (yc, zc, rad) = ((l.y + h.y) / 2.0, (l.z + h.z) / 2.0, (h.z - l.z) / 2.0);
        let half = (rad * rad - (zc - base_top).powi(2)).max(0.0).sqrt();
        if back {
            yc + half
        } else {
            yc - half
        }
    };
    let front = reach(&at(&mut doc, saw, &m, positions[0], plunge), false);
    let back = reach(&at(&mut doc, saw, &m, positions[2], plunge), true);
    let capacity = (fence_front - front) / IN;
    eprintln!(
        "NOTE the cut at the base's surface runs from y {:.1} to {:.1} mm; the fence's front face is at {:.1}",
        front, back, fence_front
    );
    must(
        capacity >= 3.0,
        format!("stock up to {capacity:.2} in wide fits between the fence and the wheel's forward reach"),
    );
    must(
        back >= fence_front,
        format!(
            "the wheel reaches the fence at the back stop ({:.1} past it)",
            back - fence_front
        ),
    );

    // The stop: the pivot plate's top back corner meets it just above
    // rest, so the spring holds the handle a little above level.
    let up2 = hits(&at(&mut doc, saw, &m, 0.0, -2.0));
    let up5 = hits(&at(&mut doc, saw, &m, 0.0, -5.0));
    must(
        !pair(&up2, "carriage / stop", "handle assembly / pivot plate"),
        format!("2 degrees above rest the pivot plate is still clear of the stop: {up2:?}"),
    );
    must(
        pair(&up5, "carriage / stop", "handle assembly / pivot plate"),
        format!("5 degrees above rest the stop has caught the pivot plate: {up5:?}"),
    );

    // The pins stop the carriage by its slots' closed ends and nothing
    // else does: a little past each stop only the pin and the plate meet.
    for (dy, pin) in [(-forward - 0.1, "front pin"), (backward + 0.1, "back pin")] {
        let past = hits(&at(&mut doc, saw, &m, dy, 0.0));
        must(
            past.len() == 1 && pair(&past, pin, "carriage / carriage plate"),
            format!(
                "past the {} stop only the {pin} meets the carriage plate: {past:?}",
                if dy < 0.0 { "forward" } else { "back" }
            ),
        );
    }

    // The handle's height, for the hand: about the plan's 15 in overall.
    let top = hi(body(&home, "handle assembly / handle")).z / IN;
    must(
        top > 13.0 && top < 15.5,
        format!("the handle's top is {top:.1} in above the bench"),
    );
    // The guides never run off the slides.
    for dy in [-forward, backward] {
        let r = at(&mut doc, saw, &m, dy, 0.0);
        let (g, s) = (body(&r, "carriage / top guide"), body(&r, "top slide"));
        must(
            lo(g).y >= lo(s).y && hi(g).y <= hi(s).y,
            format!("carriage {dy:+.0} in: the guides ride on the slides"),
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
