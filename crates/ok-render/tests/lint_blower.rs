//! The LG lint duct blower adapter (examples/lint-blower) as a
//! regression suite: the adapter is one closed body whose sleeve stands
//! on the lip with the flange round its foot, whose wedge keeps the
//! wall through its leaning faces, whose socket is the Samsung part's
//! cone (the throat, the half angle, the hollow behind the axis) under a
//! mouth tilted toward the user; the blade is the filter's width and
//! thickness less the clearances, a box open at its top and its bottom
//! edge and nowhere else, that goes up the sleeve the glue gap inside
//! it; and the five pieces fit a 256 mm bed, meet nowhere, and leave
//! their glue gaps. Every number prints (`--nocapture`).

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

fn bodies_of(doc: &mut Document, tab: &str, n: usize) -> Vec<Solid> {
    let r = doc.regenerate_studio(tab_named(doc, tab), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), n, "{n} bodies in {tab}");
    let out: Vec<Solid> = r.bodies.iter().map(|b| b.solid.clone()).collect();
    for b in &out {
        b.validate().unwrap();
    }
    out
}

fn body_of(doc: &mut Document, tab: &str) -> Solid {
    bodies_of(doc, tab, 1).remove(0)
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
fn the_adapter_stands_on_the_lip_and_keeps_the_samsung_socket() {
    let mut doc = load();
    let n = numbers();
    let a = &n["adapter"];
    let body = body_of(&mut doc, "adapter");
    let (lo, hi) = body.bounds().unwrap();
    let (stub_l, stub_t, stub_d, bore_l, bore_t, wall) = (
        num(a, "stub_l"),
        num(a, "stub_t"),
        num(a, "stub_d"),
        num(a, "bore_l"),
        num(a, "bore_t"),
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
    // The sleeve stands on the lip, z = 0, and is a wall each side
    // bigger than its bore; the back face is the datum and the socket box
    // sets the depth; the flange sets the length.
    assert!(
        (stub_l - bore_l - 2.0 * wall).abs() < 1e-9 && (stub_t - bore_t - 2.0 * wall).abs() < 1e-9
    );
    assert!(lo.z.abs() < 1e-6, "the adapter stands on the lip");
    assert!((hi.z - z_top).abs() < 1e-6);
    assert!(lo.y.abs() < 1e-6 && (hi.y - socket_d).abs() < 1e-6);
    assert!((hi.x - (stub_l / 2.0 + flange_out)).abs() < 1e-6 && (lo.x + hi.x).abs() < 1e-6);
    // Through the sleeve above the flange: a ring of wall round the bore.
    let z = flange_t + (stub_d - flange_t) / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let ring = stub_l * stub_t - bore_l * bore_t;
    let got = overlap(&body, &cut);
    println!("sleeve section {got:.1} mm2, walls of {wall} want {ring:.1}");
    assert!((got - ring).abs() < 1.0);
    // Through the flange: the plate past the sleeve at the front and the
    // ends, flush at the back, round the same bore.
    let z = flange_t / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let plate = (stub_l + 2.0 * flange_out) * (stub_t + flange_out) - bore_l * bore_t;
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
    let z_loft = stub_d;
    let z = z_loft + (z_socket - z_loft) / 2.0;
    let cut = slab(Vec3::new(-200.0, -1.0, z), Vec3::new(200.0, 200.0, z + 1.0));
    let (in_front, in_end) = (num(a, "in_front"), num(a, "in_end"));
    // The section changes with height, so the slab's volume is the ring
    // at its middle, half a millimetre up.
    let f = (z + 0.5 - z_loft) / (z_socket - z_loft);
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
fn the_blade_is_the_filter_less_clearance_open_only_top_and_bottom_and_goes_up_the_sleeve() {
    let mut doc = load();
    let n = numbers();
    let (fr, b, a) = (&n["frame"], &n["blade"], &n["adapter"]);
    let blade = body_of(&mut doc, "blade");
    let (lo, hi) = blade.bounds().unwrap();
    let (t, w, d, r, y0, wall) = (
        num(b, "t"),
        num(b, "w"),
        num(b, "d"),
        num(b, "r"),
        num(b, "y0"),
        num(a, "wall"),
    );
    println!(
        "blade {:.1} x {:.1}, z {:.1} to {:.1}, {:.1} cm3",
        hi.x - lo.x,
        hi.y - lo.y,
        lo.z,
        hi.z,
        blade.volume() / 1000.0
    );
    // The filter less the clearances, each way, and BLADE_D below the lip.
    assert!((w - (num(fr, "w") - 2.0 * num(b, "clear_w"))).abs() < 1e-9);
    assert!((t - (num(fr, "t") - 2.0 * num(b, "clear_t"))).abs() < 1e-9);
    assert!((hi.x - lo.x - w).abs() < 1e-6 && (hi.y - lo.y - t).abs() < 1e-6);
    assert!((lo.z + d).abs() < 1e-6 && (hi.z - num(b, "z_top")).abs() < 1e-6);
    assert!(
        d > num(fr, "grille_d") + 20.0,
        "the blade reaches past the grille with 20 to spare"
    );
    // Both faces solid: a slab a hair inside each face, the blade's
    // extent less the corners, is all in the body.
    for y in [y0 + 0.5, y0 + t - 0.5] {
        let probe = slab(
            Vec3::new(lo.x + r + 1.0, y - 0.2, lo.z + r + 1.0),
            Vec3::new(hi.x - r - 1.0, y + 0.2, hi.z - 1.0),
        );
        let got = overlap(&blade, &probe);
        println!("face at y {y:.1}: {got:.0} of {:.0} mm3", probe.volume());
        assert!((got - probe.volume()).abs() < 1.0, "a face is solid");
    }
    // Open at the top: a post the hollow's size through the top edge
    // meets nothing; and at the bottom: the same through the bottom edge
    // between the corners and the ribs; closed down the sides: a slab
    // just inside each side rail is all body.
    let ribs = num(b, "ribs") as usize;
    let bay = w / (ribs as f64 + 1.0);
    let x0 = lo.x + wall + 1.0;
    let x1 = lo.x + bay - wall / 2.0 - 1.0;
    let top = slab(
        Vec3::new(x0, y0 + wall + 0.2, hi.z - 3.0),
        Vec3::new(x1, y0 + t - wall - 0.2, hi.z + 1.0),
    );
    assert!(overlap(&blade, &top) < 1e-6, "open at the top");
    let x0b = lo.x + r.max(wall) + 1.0;
    let bottom = slab(
        Vec3::new(x0b, y0 + wall + 0.2, lo.z - 1.0),
        Vec3::new(x1, y0 + t - wall - 0.2, lo.z + 3.0),
    );
    assert!(
        overlap(&blade, &bottom) < 1e-6,
        "open at the bottom between the corners"
    );
    for x in [lo.x + wall / 2.0, hi.x - wall / 2.0] {
        let rail = slab(
            Vec3::new(x - 0.2, y0 + 0.5, lo.z + r + 1.0),
            Vec3::new(x + 0.2, y0 + t - 0.5, hi.z - 1.0),
        );
        assert!(
            (overlap(&blade, &rail) - rail.volume()).abs() < 1.0,
            "a side rail is solid"
        );
    }
    // The ribs: face to face, down the blade, one per bay boundary.
    for k in 1..=ribs {
        let x = lo.x + k as f64 * bay;
        let rib = slab(
            Vec3::new(x - 0.2, y0 + wall + 0.2, lo.z + wall + 1.0),
            Vec3::new(x + 0.2, y0 + t - wall - 0.2, -3.0),
        );
        assert!(
            (overlap(&blade, &rib) - rib.volume()).abs() < 1.0,
            "rib {k} runs face to face"
        );
    }
    // Up the sleeve: both together, the blade meets the adapter nowhere,
    // and the gap round it is the fit each side.
    let both = bodies_of(&mut doc, "adapter with blade", 2);
    // The blade is the one that hangs below the lip.
    let (ad, bl) = if both[0].bounds().unwrap().0.z < -1.0 {
        (&both[1], &both[0])
    } else {
        (&both[0], &both[1])
    };
    assert!(overlap(ad, bl) < 1e-6, "the blade meets the sleeve nowhere");
    let fit = num(a, "sleeve_fit");
    let (blo, bhi) = bl.bounds().unwrap();
    assert!(
        (blo.y - wall - fit).abs() < 1e-6 && (num(a, "stub_t") - wall - fit - bhi.y).abs() < 1e-6,
        "the fit each face"
    );
    assert!(
        (blo.x + num(a, "bore_l") / 2.0 - fit).abs() < 1e-6,
        "the fit each end"
    );
    assert!(
        bhi.z > num(a, "stub_d") - 2.0 && bhi.z < num(a, "stub_d"),
        "the blade reaches up the sleeve to just under the wedge"
    );
}

#[test]
fn the_five_pieces_fit_a_256_bed_meet_nowhere_and_leave_their_glue_gaps() {
    let mut doc = load();
    let n = numbers();
    let (a, b, pc) = (&n["adapter"], &n["blade"], &n["pieces"]);
    let (x_cut, tab_t, tab_in, tab_out, clear) = (
        num(pc, "x_cut"),
        num(pc, "tab_t"),
        num(pc, "tab_in"),
        num(pc, "tab_out"),
        num(pc, "tab_clear"),
    );
    let (tab_z0, tab_z1, wall) = (num(pc, "tab_z0"), num(pc, "tab_z1"), num(a, "wall"));
    let whole = body_of(&mut doc, "adapter");
    let blade = body_of(&mut doc, "blade");
    let middle = body_of(&mut doc, "middle");
    let right = body_of(&mut doc, "right wing");
    let left = body_of(&mut doc, "left wing");
    let rblade = body_of(&mut doc, "right blade");
    let lblade = body_of(&mut doc, "left blade");
    let bed = 256.0;
    for (name, p) in [
        ("middle", &middle),
        ("right wing", &right),
        ("left wing", &left),
        ("right blade", &rblade),
        ("left blade", &lblade),
    ] {
        let (lo, hi) = p.bounds().unwrap();
        let (dx, dy, dz) = (hi.x - lo.x, hi.y - lo.y, hi.z - lo.z);
        println!(
            "{name}: {dx:.0} x {dy:.0} x {dz:.0}, {:.1} cm3",
            p.volume() / 1000.0
        );
        assert!(dx < bed && dy < bed && dz < bed, "{name} fits the bed");
    }
    // The wedge's cuts are at the socket box's ends: the wings stop where
    // the socket starts, the middle reaches past the cuts only by its
    // tabs, and the three add up to the whole plus the tabs.
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
    // The blade's halves: cut at the middle, the right half reaching past
    // the joint by its lap strips, one inside each face, the halves
    // meeting nowhere and adding up to the blade plus the laps.
    let (lap_w, lap_t, y0, t) = (num(b, "lap_w"), num(b, "lap_t"), num(b, "y0"), num(b, "t"));
    let (lo, hi) = lblade.bounds().unwrap();
    assert!(hi.x.abs() < 1e-6 && (lo.x + num(b, "w") / 2.0).abs() < 1e-6);
    let (lo, _) = rblade.bounds().unwrap();
    assert!(
        (lo.x + lap_w).abs() < 1e-6,
        "the laps reach {lap_w} past the joint"
    );
    assert!(overlap(&rblade, &lblade) < 1e-6, "the halves meet nowhere");
    let laps = rblade.volume() + lblade.volume() - blade.volume();
    println!("laps add {:.1} cm3", laps / 1000.0);
    assert!(laps > 0.0 && laps < 10_000.0);
    for y in [y0 + wall, y0 + t - wall - lap_t] {
        let strip = slab(
            Vec3::new(-lap_w + 0.5, y + 0.1, -100.0),
            Vec3::new(-0.5, y + lap_t - 0.1, -50.0),
        );
        assert!(
            (overlap(&rblade, &strip) - strip.volume()).abs() < 1.0,
            "a lap strip under the left plate at y {y}"
        );
        assert!(
            overlap(&lblade, &strip) < 1e-6,
            "the left half leaves the lap room"
        );
    }
}
