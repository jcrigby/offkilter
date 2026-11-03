//! The lint blower adapter in sheet (examples/lint-blower/build_sheet.py)
//! as a regression suite: assembled, the panels (the front as its face
//! and its panel), four bars and the cap are ten bodies that meet
//! nowhere; the panels are the sheet thick, the bars set the blade's
//! thickness, the cap's groove takes the panels' tops and its spigot is
//! the taper for the hose; and the flat panels have the areas and edges
//! of the panels in place, the front unfolded about its bend. Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::{Plane, Vec2, Vec3};
use ok_model::{Document, TabId};
use ok_sketch::{Loop, Profile};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/lint-blower")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/lint_blower_sheet.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/sheet.json")).unwrap()).unwrap()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no {k}"))
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bodies_of(doc: &mut Document, tab: &str) -> Vec<(String, Solid)> {
    let r = doc.regenerate_studio(tab_named(doc, tab), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    let out: Vec<(String, Solid)> = r
        .bodies
        .iter()
        .map(|b| (b.name.clone(), b.solid.clone()))
        .collect();
    for (_, b) in &out {
        b.validate().unwrap();
    }
    out
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    let (la, ha) = a.bounds().unwrap();
    let (lb, hb) = b.bounds().unwrap();
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return 0.0;
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

fn slab(lo: Vec3, hi: Vec3) -> Solid {
    let p = Profile {
        outer: Loop::polygon(vec![
            Vec2::new(lo.x, lo.y),
            Vec2::new(hi.x, lo.y),
            Vec2::new(hi.x, hi.y),
            Vec2::new(lo.x, hi.y),
        ]),
        holes: Vec::new(),
    };
    ok_brep::extrude(&p, &Plane::XY, lo.z, hi.z, 1).unwrap()
}

fn post(x: f64, y: f64, r: f64, z0: f64, z1: f64) -> Solid {
    let pts = (0..36)
        .map(|k| {
            let a = (k as f64) * std::f64::consts::TAU / 36.0;
            Vec2::new(x + r * a.cos(), y + r * a.sin())
        })
        .collect();
    let p = Profile {
        outer: Loop::polygon(pts),
        holes: Vec::new(),
    };
    ok_brep::extrude(&p, &Plane::XY, z0, z1, 1).unwrap()
}

#[test]
fn assembled_the_ten_parts_meet_nowhere_and_sit_where_the_numbers_say() {
    let mut doc = load();
    let n = numbers();
    let (sheet, blade, wedge, cap) = (&n["sheet"], &n["blade"], &n["wedge"], &n["cap"]);
    let parts = bodies_of(&mut doc, "assembled");
    assert_eq!(parts.len(), 10, "five plates, four bars, the cap");
    let mut worst = 0.0f64;
    for i in 0..parts.len() {
        for j in i + 1..parts.len() {
            let o = overlap(&parts[i].1, &parts[j].1);
            if o > 1e-6 {
                let hit = ok_brep::boolean(&parts[i].1, &parts[j].1, BoolOp::Intersection).unwrap();
                println!(
                    "{} meets {}: {:.3} mm3 at {:?}",
                    parts[i].0,
                    parts[j].0,
                    o,
                    hit.bounds()
                );
            }
            worst = worst.max(o);
        }
    }
    assert!(worst < 1e-6, "no two parts meet");
    let t = num(sheet, "t");
    let (w, bt, d) = (num(blade, "w"), num(blade, "t"), num(blade, "d"));
    // The kernel names bodies Part n, so the parts are told by where
    // they are: the cap is the one reaching the top; the back is the
    // sheet at y 0; the front face the sheet at y BLADE_T below the lip;
    // the bars the four 10 x 11 blocks; the rest the three leaning panels.
    let bounds = |s: &Solid| s.bounds().unwrap();
    let find = |pred: &dyn Fn(Vec3, Vec3) -> bool, what: &str| -> &Solid {
        let hits: Vec<&Solid> = parts
            .iter()
            .map(|(_, s)| s)
            .filter(|s| {
                let (lo, hi) = bounds(s);
                pred(lo, hi)
            })
            .collect();
        assert_eq!(hits.len(), 1, "one {what}, found {}", hits.len());
        hits[0]
    };
    let z_top = num(cap, "z_top");
    let cap_body = find(&|_, hi| (hi.z - z_top).abs() < 1e-6, "cap");
    let back = find(
        &|lo, hi| lo.y.abs() < 1e-6 && (hi.y - t).abs() < 1e-6 && hi.x - lo.x > 200.0,
        "back",
    );
    let front_face = find(
        &|lo, hi| {
            (hi.y - bt).abs() < 1e-6
                && (lo.y - bt + t).abs() < 1e-6
                && hi.z.abs() < 1e-6
                && hi.x - lo.x > 200.0
        },
        "front face",
    );
    let (bar_w, bar_t) = (num(blade, "bar_w"), num(blade, "bar_t"));
    let bars: Vec<&Solid> = parts
        .iter()
        .map(|(_, s)| s)
        .filter(|s| {
            let (lo, hi) = bounds(s);
            (hi.x - lo.x - bar_w).abs() < 1e-6 && (hi.y - lo.y - bar_t).abs() < 1e-6
        })
        .collect();
    assert_eq!(bars.len(), 4, "four bars");
    // The back: one piece, the sheet thick, from the blade's bottom to
    // the wedge's top; the front face the blade's thickness in front of it.
    let (lo, hi) = bounds(back);
    println!(
        "back x {:.1} to {:.1}, y {:.1} to {:.1}, z {:.1} to {:.1}",
        lo.x, hi.x, lo.y, hi.y, lo.z, hi.z
    );
    assert!(
        (hi.x - lo.x - w).abs() < 1e-6
            && (lo.z + d).abs() < 1e-6
            && (hi.z - num(wedge, "h")).abs() < 1e-6
    );
    let (lo, hi) = bounds(front_face);
    assert!((hi.x - lo.x - w).abs() < 1e-6 && (lo.z + d).abs() < 1e-6);
    // The bars: between the faces, the end ones the blade's depth and
    // flush with its ends, the ribs short of the fold bar.
    let mut ends = 0;
    for b in &bars {
        let (lo, hi) = bounds(b);
        assert!((lo.y - t).abs() < 1e-6 && (lo.z + d).abs() < 1e-6);
        if (hi.x.abs() - w / 2.0).abs() < 1e-6 || (lo.x.abs() - w / 2.0).abs() < 1e-6 {
            ends += 1;
            assert!(
                (hi.z - lo.z - d).abs() < 1e-6,
                "an end bar is the blade's depth"
            );
        } else {
            assert!(
                (hi.z - lo.z - num(blade, "rib_l")).abs() < 1e-6,
                "a rib is short of the fold bar"
            );
        }
    }
    assert_eq!(ends, 2, "two bars at the ends");
    // The cap: its groove takes the panels' tops GROOVE_D deep: the back
    // panel's top edge is that far above the cap's underside and the two
    // meet nowhere, as the first check found.
    let (clo, chi) = bounds(cap_body);
    let (_, bhi) = bounds(back);
    println!(
        "cap z {:.1} to {:.1}, the panels' tops at {:.1}",
        clo.z, chi.z, bhi.z
    );
    assert!(
        (clo.z - num(cap, "z0")).abs() < 1e-6
            && (bhi.z - clo.z - num(sheet, "groove_d")).abs() < 1e-6
    );
    assert!((chi.z - z_top).abs() < 1e-6);
}

#[test]
fn the_cap_has_its_groove_its_opening_and_the_taper() {
    let mut doc = load();
    let n = numbers();
    let (sheet, wedge, cap) = (&n["sheet"], &n["wedge"], &n["cap"]);
    let body = bodies_of(&mut doc, "cap").remove(0).1;
    let (lo, hi) = body.bounds().unwrap();
    let (cap_sq, axis_y, hs) = (
        num(wedge, "cap_sq"),
        num(wedge, "axis_y"),
        num(wedge, "sq") / 2.0,
    );
    println!(
        "cap {:.1} x {:.1} x {:.1}, {:.1} cm3",
        hi.x - lo.x,
        hi.y - lo.y,
        hi.z - lo.z,
        body.volume() / 1000.0
    );
    // The skirt spreads with the ends below the top, so the cap is wider
    // than its square; its height is the numbers'.
    assert!(
        hi.x - lo.x > cap_sq && (hi.z - lo.z - (num(cap, "z_top") - num(cap, "z0"))).abs() < 1e-6
    );
    // The groove: a ring GROOVE wide, GROOVE_D up from the underside, its
    // outer wall at the wedge's outside; a slab in it is empty, one in
    // its outer wall and one in its core are full.
    let (g, gd, z0) = (num(sheet, "groove"), num(sheet, "groove_d"), num(cap, "z0"));
    let in_groove = slab(
        Vec3::new(-20.0, axis_y - hs + 0.1, z0 + 0.5),
        Vec3::new(20.0, axis_y - hs + g - 0.1, z0 + gd - 0.5),
    );
    let in_wall = slab(
        Vec3::new(-20.0, axis_y - hs - 1.9, z0 + 0.5),
        Vec3::new(20.0, axis_y - hs - 0.1, z0 + gd - 0.5),
    );
    let in_core = slab(
        Vec3::new(-20.0, axis_y - hs + g + 0.1, z0 + 0.5),
        Vec3::new(20.0, axis_y - hs + g + 1.9, z0 + gd - 0.5),
    );
    println!(
        "groove {:.1} of {:.1} empty, wall {:.1} of {:.1} full, core {:.1} of {:.1} full",
        overlap(&body, &in_groove),
        in_groove.volume(),
        overlap(&body, &in_wall),
        in_wall.volume(),
        overlap(&body, &in_core),
        in_core.volume()
    );
    assert!(overlap(&body, &in_groove) < 1e-6);
    assert!((overlap(&body, &in_wall) - in_wall.volume()).abs() < 1e-6);
    assert!((overlap(&body, &in_core) - in_core.volume()).abs() < 1e-6);
    // The opening through the base, and the bore out the top.
    let o = num(cap, "opening");
    let through = slab(
        Vec3::new(-o / 2.0 + 0.3, axis_y - o / 2.0 + 0.3, z0 - 1.0),
        Vec3::new(
            o / 2.0 - 0.3,
            axis_y + o / 2.0 - 0.3,
            num(cap, "z_round") + 0.5,
        ),
    );
    assert!(
        overlap(&body, &through) < 1e-6,
        "the opening is clear through the base"
    );
    let r = num(cap, "r_bore");
    let free = post(
        0.0,
        axis_y,
        r - 0.3,
        num(cap, "z_spigot") + 1.0,
        num(cap, "z_top") + 1.0,
    );
    assert!(
        overlap(&body, &free) < 1e-6,
        "the bore is clear out the top"
    );
    let tapers: Vec<f64> = body
        .surfaces
        .iter()
        .filter_map(|s| match s {
            Surface::Cone {
                apex,
                axis,
                half_angle,
            } if axis.z.abs() > 1.0 - 1e-9
                && apex.x.abs() < 1e-6
                && (apex.y - axis_y).abs() < 1e-6 =>
            {
                Some(half_angle.to_degrees())
            }
            _ => None,
        })
        .collect();
    println!(
        "cones on the axis {tapers:?}, want {:.2}",
        num(cap, "spigot_taper")
    );
    assert!(tapers
        .iter()
        .any(|t| (t - num(cap, "spigot_taper")).abs() < 1e-6));
}

#[test]
fn the_flat_panels_are_the_panels_in_place() {
    let mut doc = load();
    let n = numbers();
    let t = num(&n["sheet"], "t");
    let flats: Vec<Solid> = bodies_of(&mut doc, "panels")
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    // The sheets' outlines are thinner than the sheet; the rest are panels and ribs.
    let pieces: Vec<&Solid> = flats
        .iter()
        .filter(|s| {
            let (lo, hi) = s.bounds().unwrap();
            (hi.z - lo.z - t).abs() < 1e-6
        })
        .collect();
    assert_eq!(pieces.len(), 6, "four panels and two ribs");
    let mut areas: Vec<f64> = pieces.iter().map(|s| s.volume() / t).collect();
    areas.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rib = num(&n["blade"], "rib_l") * num(&n["blade"], "bar_t");
    let mut want: Vec<f64> = n["panels"]
        .as_object()
        .unwrap()
        .values()
        .map(|p| num(p, "area"))
        .collect();
    want.push(rib);
    want.push(rib);
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "flat areas {:?}\nwant       {:?}",
        areas.iter().map(|a| a.round()).collect::<Vec<_>>(),
        want.iter().map(|a| a.round()).collect::<Vec<_>>()
    );
    for (a, w) in areas.iter().zip(want.iter()) {
        // the front's bend line is scored into it, 0.6 wide and 0.5 deep across the sheet
        assert!(
            (a - w).abs() < 50.0,
            "a flat piece has its area: {a:.1} for {w:.1}"
        );
    }
    // The edges: the corners in place give the same lengths as the flat
    // ones; the front's bend is at two corners, so its edges keep theirs.
    for (name, p) in n["panels"].as_object().unwrap() {
        let corners: Vec<Vec3> = p["corners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                Vec3::new(
                    c[0].as_f64().unwrap(),
                    c[1].as_f64().unwrap(),
                    c[2].as_f64().unwrap(),
                )
            })
            .collect();
        let flat: Vec<Vec2> = p["flat"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| Vec2::new(c[0].as_f64().unwrap(), c[1].as_f64().unwrap()))
            .collect();
        for i in 0..corners.len() {
            let j = (i + 1) % corners.len();
            let a = (corners[j] - corners[i]).length();
            let b = (flat[j] - flat[i]).length();
            assert!(
                (a - b).abs() < 1e-6,
                "{name} edge {i}: {a:.2} in place, {b:.2} flat"
            );
        }
    }
}
