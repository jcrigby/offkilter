//! The LG lint duct blower adapter (examples/lint-blower) as a
//! regression suite: the adapter is one closed body whose stub is the
//! slot's size less the clearance with walls of the thickness asked,
//! whose flange reaches past the stub at the front and the ends and
//! stays flush at the back, whose wedge keeps the wall through its
//! leaning faces, whose socket is the Samsung part's cone (the throat,
//! the half angle, the hollow behind the axis) under a mouth tilted
//! toward the user; and the gauge is one comb whose teeth read their
//! thicknesses. Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::{Plane, Vec2, Vec3};
use ok_model::{Document, TabId};
use ok_sketch::{Loop, Profile};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/lint-blower")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/lint_blower.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/adapter.json")).unwrap()).unwrap()
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

fn body_of(doc: &mut Document, tab: &str) -> Solid {
    let r = doc.regenerate_studio(tab_named(doc, tab), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one body in {tab}");
    let body = r.bodies[0].solid.clone();
    body.validate().unwrap();
    body
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// A box from `lo` to `hi`, extruded up the z axis.
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

/// A 36-gon inscribed in the circle of radius `r` about (x, y), from
/// z0 to z1: every vertex on the circle, so it lies within the bore
/// of that radius.
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
fn the_adapter_fits_the_slot_and_keeps_the_samsung_socket() {
    let mut doc = load();
    let n = numbers();
    let (slot, a) = (&n["slot"], &n["adapter"]);
    let body = body_of(&mut doc, "adapter");
    let (lo, hi) = body.bounds().unwrap();
    let (stub_l, stub_t, stub_d, clear, wall) = (
        num(a, "stub_l"),
        num(a, "stub_t"),
        num(a, "stub_d"),
        num(a, "clear"),
        num(a, "wall"),
    );
    let (flange_t, flange_out) = (num(a, "flange_t"), num(a, "flange_out"));
    let (z_socket, z_top, socket_w, socket_d) = (
        num(a, "z_socket"),
        num(a, "z_top"),
        num(a, "socket_w"),
        num(a, "socket_d"),
    );
    let (socket_front, throat_d, cone_half, axis_y) = (
        num(a, "socket_front"),
        num(a, "throat_d"),
        num(a, "cone_half"),
        num(a, "axis_y"),
    );
    println!(
        "adapter x {:.1} to {:.1}, y {:.1} to {:.1}, z {:.1} to {:.1}, {:.1} cm3",
        lo.x,
        hi.x,
        lo.y,
        hi.y,
        lo.z,
        hi.z,
        body.volume() / 1000.0
    );
    // The stub is the slot less the clearance each side, and reaches
    // down the slot; the back face is the datum and the socket box sets
    // the depth; the flange sets the length.
    assert!((stub_l - (num(slot, "l") - 2.0 * clear)).abs() < 1e-9);
    assert!((stub_t - (num(slot, "w") - 2.0 * clear)).abs() < 1e-9);
    assert!(
        (lo.z + stub_d).abs() < 1e-6,
        "the stub reaches {} down",
        -lo.z
    );
    assert!((hi.z - z_top).abs() < 1e-6);
    assert!(lo.y.abs() < 1e-6 && (hi.y - socket_d).abs() < 1e-6);
    assert!((hi.x - (stub_l / 2.0 + flange_out)).abs() < 1e-6 && (lo.x + hi.x).abs() < 1e-6);
    // Through the stub: a ring of wall round the slot's section.
    let z = -stub_d / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let ring = stub_l * stub_t - (stub_l - 2.0 * wall) * (stub_t - 2.0 * wall);
    let got = overlap(&body, &cut);
    println!("stub section {got:.1} mm2, walls of {wall} want {ring:.1}");
    assert!((got - ring).abs() < 1.0);
    // Through the flange: the plate past the stub at the front and the
    // ends, flush at the back, round the same bore.
    let z = flange_t / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let plate = (stub_l + 2.0 * flange_out) * (stub_t + flange_out)
        - (stub_l - 2.0 * wall) * (stub_t - 2.0 * wall);
    let got = overlap(&body, &cut);
    println!("flange section {got:.1} mm2, want {plate:.1}");
    assert!((got - plate).abs() < 1.0);
    let back = slab(
        Vec3::new(-200.0, -5.0, -100.0),
        Vec3::new(200.0, -1e-3, 200.0),
    );
    assert!(overlap(&body, &back) < 1e-6, "nothing behind the back face");
    // Through the wedge, halfway up: the wall through the leaning faces
    // is `wall` measured square to them, so more in the plane of the cut.
    let z = flange_t + (z_socket - flange_t) / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let (in_front, in_end) = (num(a, "in_front"), num(a, "in_end"));
    // The section changes with height, so the slab's volume is the ring
    // at its middle, half a millimetre up.
    let f = (z + 0.5 - flange_t) / (z_socket - flange_t);
    let w_out = stub_l + (socket_w - stub_l) * f;
    let d_out = stub_t + (socket_d - stub_t) * f;
    let want = w_out * d_out - (w_out - 2.0 * in_end) * (d_out - wall - in_front);
    let got = overlap(&body, &cut);
    println!(
        "wedge section {got:.1} mm2, want {want:.1} (insets {in_front:.2} front, {in_end:.2} ends)"
    );
    assert!((got - want).abs() < 2.0);
    assert!(in_front > wall && in_end > wall);
    // The socket: the cone's throat where the box meets the wedge, a
    // post just inside it passing clean, one past it meeting the front
    // half; the cone's surface at the half angle on the axis; and the
    // hollow behind the axis, a box, when the STL's is kept.
    let r = throat_d / 2.0;
    let free = post(0.0, axis_y, r - 0.3, z_socket + 0.5, z_socket + 4.0);
    let tight = post(0.0, axis_y, r + 1.5, z_socket + 0.5, z_socket + 4.0);
    println!(
        "throat: a post of {:.1} overlaps {:.2}, one of {:.1} overlaps {:.1}",
        2.0 * (r - 0.3),
        overlap(&body, &free),
        2.0 * (r + 1.5),
        overlap(&body, &tight)
    );
    assert!(overlap(&body, &free) < 1e-6);
    assert!(overlap(&body, &tight) > 50.0);
    let cones: Vec<(Vec3, Vec3, f64)> = body
        .surfaces
        .iter()
        .filter_map(|s| match s {
            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => Some((*apex, *axis, half_angle.to_degrees())),
            _ => None,
        })
        .collect();
    println!("cones {cones:?}");
    assert_eq!(cones.len(), 1, "one cone");
    let (apex, axis, half) = cones[0];
    assert!((half - cone_half).abs() < 1e-6);
    assert!(axis.z.abs() > 1.0 - 1e-9 && apex.x.abs() < 1e-6 && (apex.y - axis_y).abs() < 1e-6);
    let apex_z = z_socket - r / cone_half.to_radians().tan();
    assert!(
        (apex.z - apex_z).abs() < 1e-6,
        "apex at {} wants {apex_z}",
        apex.z
    );
    if a["back_open"].as_bool().unwrap() {
        let hollow = slab(
            Vec3::new(-socket_w / 2.0 + wall + 0.3, wall + 0.3, z_socket + 0.5),
            Vec3::new(
                socket_w / 2.0 - wall - 0.3,
                axis_y - 0.3,
                z_socket + socket_front - 0.5,
            ),
        );
        assert!(overlap(&body, &hollow) < 1e-6, "hollow behind the axis");
    }
    // The mouth: the back edge is the top of the part, the front edge
    // `socket_front` above the wedge.
    let front_lip = slab(
        Vec3::new(-200.0, socket_d - 1.0, -100.0),
        Vec3::new(200.0, socket_d + 1.0, 200.0),
    );
    let (_, lip_hi) = ok_brep::boolean(&body, &front_lip, BoolOp::Intersection)
        .unwrap()
        .bounds()
        .unwrap();
    println!(
        "mouth: {:.1} at the front, {:.1} at the back, tilted {:.1} deg",
        lip_hi.z - z_socket,
        z_top - z_socket,
        num(a, "mouth_tilt")
    );
    assert!((lip_hi.z - (z_socket + socket_front)).abs() < 0.5);
    assert!(z_top > lip_hi.z + 10.0);
}

#[test]
fn the_gauge_is_one_comb_whose_teeth_read_their_thicknesses() {
    let mut doc = load();
    let n = numbers();
    let g = &n["gauge"];
    let teeth: Vec<f64> = g["teeth"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let (tooth_w, tooth_gap, tooth_d, spine_t) = (
        num(g, "tooth_w"),
        num(g, "tooth_gap"),
        num(g, "tooth_d"),
        num(g, "spine_t"),
    );
    let comb = body_of(&mut doc, "slot gauge");
    let (lo, hi) = comb.bounds().unwrap();
    println!(
        "comb {:.0} long, {:.0} across, teeth {:.0} down, {:.1} cm3",
        hi.x - lo.x,
        hi.y - lo.y,
        -lo.z,
        comb.volume() / 1000.0
    );
    assert!((lo.z + tooth_d).abs() < 1e-6 && (hi.z - spine_t).abs() < 1e-6);
    let thickest = teeth.iter().cloned().fold(0.0, f64::max);
    assert!(
        (hi.y - lo.y - (thickest + 2.0)).abs() < 1e-6,
        "the spine clears the thickest tooth"
    );
    let mut last = 0.0;
    for (k, t) in teeth.iter().enumerate() {
        let x = k as f64 * (tooth_w + tooth_gap);
        let cut = slab(
            Vec3::new(x - 1.0, -1.0, -tooth_d / 2.0),
            Vec3::new(x + tooth_w + 1.0, hi.y + 1.0, -tooth_d / 2.0 + 1.0),
        );
        let (tl, th) = ok_brep::boolean(&comb, &cut, BoolOp::Intersection)
            .unwrap()
            .bounds()
            .unwrap();
        let read = th.y - tl.y;
        println!("tooth {} at x {x:.0}: {read:.2} thick", k + 1);
        assert!((read - t).abs() < 1e-6);
        assert!(*t > last, "teeth thicken along the comb");
        last = *t;
    }
}

#[test]
fn the_three_pieces_fit_a_256_bed_and_their_tabs_leave_the_glue_gap() {
    let mut doc = load();
    let n = numbers();
    let (a, pc) = (&n["adapter"], &n["pieces"]);
    let (x_cut, tab_t, tab_in, tab_out, clear) = (
        num(pc, "x_cut"),
        num(pc, "tab_t"),
        num(pc, "tab_in"),
        num(pc, "tab_out"),
        num(pc, "tab_clear"),
    );
    let (tab_z0, tab_z1, wall) = (num(pc, "tab_z0"), num(pc, "tab_z1"), num(a, "wall"));
    let whole = body_of(&mut doc, "adapter");
    let middle = body_of(&mut doc, "middle");
    let right = body_of(&mut doc, "right wing");
    let left = body_of(&mut doc, "left wing");
    let bed = 256.0;
    for (name, p) in [
        ("middle", &middle),
        ("right wing", &right),
        ("left wing", &left),
    ] {
        let (lo, hi) = p.bounds().unwrap();
        let (dx, dy, dz) = (hi.x - lo.x, hi.y - lo.y, hi.z - lo.z);
        println!(
            "{name}: {dx:.0} x {dy:.0} x {dz:.0}, {:.1} cm3",
            p.volume() / 1000.0
        );
        assert!(
            dx < bed && dy < bed && dz < bed,
            "{name} fits the bed standing"
        );
    }
    // The cuts are at the socket box's ends: the wings stop where the
    // socket starts, the middle reaches past the cuts only by its tabs,
    // and the three add up to the whole plus the tabs.
    let (lo, hi) = right.bounds().unwrap();
    assert!((lo.x - x_cut).abs() < 1e-6 && (hi.z - num(a, "z_socket")).abs() < 1e-6);
    let (lo, hi) = middle.bounds().unwrap();
    assert!((hi.x - (x_cut + tab_out)).abs() < 1e-6 && (lo.x + hi.x).abs() < 1e-6);
    let extra = middle.volume() + right.volume() + left.volume() - whole.volume();
    println!("tabs add {:.1} cm3", extra / 1000.0);
    assert!(extra > 0.0 && extra < 30_000.0);
    // No piece meets another, and the tabs stand the clearance off the
    // wing's walls: a box the clearance thick between tab and wall is
    // empty, one a hair thicker touches the tab.
    for (name, wing) in [("right", &right), ("left", &left)] {
        let hit = ok_brep::boolean(&middle, wing, BoolOp::Intersection).unwrap();
        println!(
            "middle against the {name} wing: {:.3} mm3 at {:?}",
            hit.volume().abs(),
            hit.bounds()
        );
    }
    assert!(overlap(&middle, &right) < 1e-6 && overlap(&middle, &left) < 1e-6);
    let zmid = (tab_z0 + tab_z1) / 2.0;
    let gap = slab(
        Vec3::new(x_cut + 1.0, wall + 1e-3, zmid),
        Vec3::new(x_cut + tab_out - 1.0, wall + clear - 1e-3, zmid + 1.0),
    );
    let more = slab(
        Vec3::new(x_cut + 1.0, wall + 1e-3, zmid),
        Vec3::new(x_cut + tab_out - 1.0, wall + clear + 0.5, zmid + 1.0),
    );
    assert!(
        overlap(&middle, &gap) < 1e-6 && overlap(&right, &gap) < 1e-6,
        "the glue gap behind the back tab is empty"
    );
    assert!(
        overlap(&middle, &more) > 1.0,
        "the back tab is just past the gap"
    );
    // The tabs run tab_in into the middle, fused to its walls, and tab_out
    // past the cut, with the wall's thickness.
    let inside = slab(
        Vec3::new(x_cut - tab_in + 1.0, wall + clear + tab_t - 0.5, zmid),
        Vec3::new(x_cut - 1.0, wall + clear + tab_t + 0.5, zmid + 1.0),
    );
    let got = overlap(&middle, &inside);
    println!(
        "back tab through the middle's wall: {got:.2} mm3 of {:.2}",
        (tab_in - 2.0) * 0.5
    );
    assert!((got - (tab_in - 2.0) * 0.5).abs() < 0.05);
    let (lo, hi) = ok_brep::boolean(
        &middle,
        &slab(
            Vec3::new(x_cut + 0.5, -1.0, -100.0),
            Vec3::new(x_cut + tab_out + 1.0, 200.0, 200.0),
        ),
        BoolOp::Intersection,
    )
    .unwrap()
    .bounds()
    .unwrap();
    println!(
        "tabs past the cut: y {:.2} to {:.2}, z {:.1} to {:.1}",
        lo.y, hi.y, lo.z, hi.z
    );
    assert!(
        (lo.y - (wall + clear)).abs() < 1e-6
            && (lo.z - tab_z0).abs() < 1e-6
            && (hi.z - tab_z1).abs() < 1e-6
    );
}
