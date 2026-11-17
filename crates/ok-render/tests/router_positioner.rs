//! The two-axis nested-box router positioner (examples/router-positioner),
//! first pass. The design brief gives rules of thumb and asks for the
//! four corners of the travel to be checked; this does both from the
//! committed document, with the kernel's solids and not the build
//! script's arithmetic: every part is a closed solid; at each corner
//! of the travel no part is through another; each sliding box is in
//! its guide by a width and a half and more, and stays inside it; each
//! screw is through its nut at one end of its travel and short of what
//! it runs toward at the other; and the envelope, the length behind
//! the router and the parts too long for the printer are what the
//! build reported.

use ok_brep::{BoolOp, Solid, Transform};
use ok_math::{Plane, Vec2, Vec3};
use ok_model::{Document, TabId};
use ok_sketch::{Loop, Profile};
use serde_json::Value;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/router-positioner")
}

fn load() -> Document {
    Document::from_json(&std::fs::read_to_string(dir().join("out/positioner.okpart")).unwrap())
        .unwrap()
}

fn numbers() -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/positioner.json")).unwrap())
        .unwrap()
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn tab(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bodies_of(doc: &mut Document, name: &str) -> Vec<Solid> {
    let r = doc.regenerate_studio(tab(doc, name), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{name}: {:?}",
        r.statuses
    );
    r.bodies
        .iter()
        .map(|b| {
            b.solid.validate().unwrap();
            b.solid.clone()
        })
        .collect()
}

/// A part as the build drew it, with both axes at home, and what it moves with.
struct Piece {
    name: String,
    moves: String,
    solid: Solid,
}

/// Every body of every part, regenerated from the document.
fn pieces(doc: &mut Document, n: &Value) -> Vec<Piece> {
    let mut out = Vec::new();
    for part in n["parts"].as_array().unwrap() {
        let (name, moves) = (
            part["tab"].as_str().unwrap(),
            part["moves"].as_str().unwrap(),
        );
        let solids = bodies_of(doc, name);
        let listed = part["bodies"].as_array().unwrap();
        assert_eq!(solids.len(), listed.len(), "{name}");
        for (k, (solid, was)) in solids.into_iter().zip(listed).enumerate() {
            // the document regenerates to the bodies the build saw
            let volume = solid.volume();
            assert!(
                (volume - f(&was[1])).abs() < 1e-3 * volume.max(1.0),
                "{name} {k}"
            );
            out.push(Piece {
                name: format!("{name} {}", k + 1),
                moves: moves.to_string(),
                solid,
            });
        }
    }
    out
}

/// The parts with the quill run out by `dx` and the stage run down by `dz`.
fn at(pieces: &[Piece], dx: f64, dz: f64) -> Vec<Solid> {
    pieces
        .iter()
        .map(|p| {
            let by = match p.moves.as_str() {
                "fixed" => Vec3::new(0.0, 0.0, 0.0),
                "z" => Vec3::new(0.0, 0.0, -dz),
                "xz" => Vec3::new(-dx, 0.0, -dz),
                other => panic!("{other}"),
            };
            p.solid.transformed(&Transform::translation(by))
        })
        .collect()
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

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

fn named<'a>(pieces: &'a [Piece], solids: &'a [Solid], name: &str) -> &'a Solid {
    let i = pieces
        .iter()
        .position(|p| p.name == name)
        .unwrap_or_else(|| panic!("no piece {name}"));
    &solids[i]
}

/// What the parts near each other have in common: how many pairs
/// were tried, the largest volume shared and by which two, and how
/// many pairs the kernel could not intersect.
fn shared(pieces: &[Piece], solids: &[Solid]) -> (usize, f64, String, usize) {
    let boxes: Vec<(Vec3, Vec3)> = solids.iter().map(bounds).collect();
    let (mut tried, mut worst, mut failed) = (0, (0.0f64, String::new()), 0);
    for i in 0..solids.len() {
        for k in i + 1..solids.len() {
            let (a, b) = (boxes[i], boxes[k]);
            if a.0.x >= b.1.x
                || b.0.x >= a.1.x
                || a.0.y >= b.1.y
                || b.0.y >= a.1.y
                || a.0.z >= b.1.z
                || b.0.z >= a.1.z
            {
                continue;
            }
            tried += 1;
            match ok_brep::boolean(&solids[i], &solids[k], BoolOp::Intersection) {
                Ok(s) => {
                    let common = s.volume().abs();
                    if common > worst.0 {
                        worst = (common, format!("{} and {}", pieces[i].name, pieces[k].name));
                    }
                }
                Err(_) => failed += 1,
            }
        }
    }
    (tried, worst.0, worst.1, failed)
}

/// At each of the four corners of the travel, no part is through
/// another. Parts that bear on each other touch, and that is all:
/// strips on walls, gibs on boxes, nuts on their seats. And the check
/// can tell: run past either end of either travel, something is hit.
#[test]
fn nothing_is_through_anything_at_the_four_corners_of_the_travel() {
    let (mut doc, n) = (load(), numbers());
    let pieces = pieces(&mut doc, &n);
    assert!(pieces.len() >= 30);
    let corners = n["corners"].as_array().unwrap();
    assert_eq!(corners.len(), 4);
    let (tx, tz) = (f(&n["travel"]["x"]), f(&n["travel"]["z"]));
    for corner in corners {
        let (dx, dz) = (f(&corner[1]), f(&corner[2]));
        assert!((dx == 0.0 || dx == tx) && (dz == 0.0 || dz == tz));
        let (tried, most, which, failed) = shared(&pieces, &at(&pieces, dx, dz));
        println!(
            "{}: {tried} pairs near each other, the most in common {most:.3} mm3 ({which}); {failed} the kernel could not intersect",
            corner[0].as_str().unwrap()
        );
        assert!(tried > 60);
        assert_eq!(failed, 0);
        assert!(most < 0.5, "{which} share {most:.2} mm3");
    }
    // Past the ends: the quill run out further meets the outer box's face, drawn back further its nut boss meets
    // the drive housing; the stage run up further meets the lid, down further the floor.
    for (dx, dz, what) in [
        (tx + 12.0, 0.0, "run out too far"),
        (-8.0, 0.0, "drawn back too far"),
        (0.0, -4.0, "up too far"),
        (0.0, tz + 4.0, "down too far"),
    ] {
        let (_, most, which, _) = shared(&pieces, &at(&pieces, dx, dz));
        println!("{what}: {which} share {most:.0} mm3");
        assert!(most > 50.0, "{what}: nothing is hit");
    }
}

/// A box in its guide by less than a width and a half racks like a
/// sticky drawer. The quill is guided by the sleeve and the column
/// together; the stage by the outer box. Measured on the bodies, at
/// both ends of each travel.
#[test]
fn each_box_is_in_its_guide_by_a_width_and_a_half_everywhere() {
    let (mut doc, n) = (load(), numbers());
    let pieces = pieces(&mut doc, &n);
    let (tx, tz, engage) = (f(&n["travel"]["x"]), f(&n["travel"]["z"]), f(&n["engage"]));
    for (dx, dz) in [(0.0, 0.0), (tx, 0.0), (0.0, tz), (tx, tz)] {
        let solids = at(&pieces, dx, dz);
        let quill = bounds(named(&pieces, &solids, "X quill 1"));
        let stage = bounds(named(&pieces, &solids, "Z stage 1"));
        let column = bounds(named(&pieces, &solids, "X column 1"));
        let outer = bounds(named(&pieces, &solids, "Outer box 1"));
        // the quill: all of it between the sleeve's mouth and the column's end
        let guide = (stage.0.x, column.1.x);
        assert!(quill.0.x >= guide.0 - 1e-6 && quill.1.x <= guide.1 + 1e-6);
        let width = (quill.1.y - quill.0.y).max(quill.1.z - quill.0.z);
        let ratio = (quill.1.x - quill.0.x) / width;
        assert!(ratio >= engage, "the quill is in by {ratio:.2} widths");
        assert!((ratio - f(&n["rules"]["x_engaged"])).abs() < 0.05);
        // the stage: all of it inside the outer box, top to bottom
        let wall = f(&n["outer_wall"]);
        assert!(stage.0.z >= outer.0.z + wall + 0.5 && stage.1.z <= outer.1.z - wall - 0.5);
        let width = (stage.1.x - stage.0.x).max(stage.1.y - stage.0.y);
        let ratio = (stage.1.z - stage.0.z) / width;
        assert!(ratio >= engage, "the stage is in by {ratio:.2} widths");
        if dx == tx {
            // at full plunge the quill's front is at the sleeve's mouth: the bit as near its guide as it gets
            assert!((quill.0.x - stage.0.x).abs() < 1e-6);
        }
    }
    // The column is what tips the stage, and the outer box's right wall is cut away for it: the stage is wider
    // than the column by a rail each side, and bears on what is left of that wall.
    let home = at(&pieces, 0.0, 0.0);
    let (stage, column) = (
        bounds(named(&pieces, &home, "Z stage 1")),
        bounds(named(&pieces, &home, "X column 1")),
    );
    let rail = ((stage.1.y - stage.0.y) - (column.1.y - column.0.y)) / 2.0;
    println!("the stage is {rail:.1} mm wider than the column each side");
    assert!(rail >= f(&n["rail"]) - 1.0);
}

/// Each screw is fixed and its nut travels. At one end of the travel
/// the screw must still be right through the nut; at the other its tip
/// must stop short of what the nut is carrying it into: the router's
/// back cap for X, the sleeve's floor for Z.
#[test]
fn each_screw_is_through_its_nut_and_short_of_what_it_runs_toward() {
    let (mut doc, n) = (load(), numbers());
    let pieces = pieces(&mut doc, &n);
    let (tx, tz) = (f(&n["travel"]["x"]), f(&n["travel"]["z"]));
    let home = at(&pieces, 0.0, 0.0);
    let far = at(&pieces, tx, tz);
    // X: drawn back, the tip is nearest the router; run out, the nut is nearest the tip.
    let screw = bounds(named(&pieces, &home, "X drive 7"));
    let router = bounds(named(&pieces, &home, "Router 1"));
    let nut = bounds(named(&pieces, &far, "X nut 1"));
    let (to_router, through) = (screw.0.x - router.1.x, nut.0.x - screw.0.x);
    println!("X screw: its tip {to_router:.1} mm from the router's back drawn back, {through:.1} mm through the nut run out");
    assert!(to_router >= 3.0 && through >= 2.0);
    assert!((to_router - f(&n["rules"]["x_tip_to_router"])).abs() < 1e-6);
    // Z: up, the nut is nearest the tip; down, the tip is nearest the sleeve's floor. The floor is found with a
    // post stood on the tip: what of the stage it meets first.
    let screw = bounds(named(&pieces, &home, "Z drive 7"));
    let nut = bounds(named(&pieces, &home, "Z nut 1"));
    let through = screw.1.z - nut.1.z;
    let stage = named(&pieces, &far, "Z stage 1");
    let (cx, tip) = ((screw.0.x + screw.1.x) / 2.0, screw.1.z);
    let mut to_floor = 0.0;
    for k in 1..=400 {
        let reach = k as f64 * 0.25;
        let post = slab(
            Vec3::new(cx - 2.0, -2.0, tip),
            Vec3::new(cx + 2.0, 2.0, tip + reach),
        );
        let met = ok_brep::boolean(&post, stage, BoolOp::Intersection)
            .map(|s| s.volume().abs())
            .unwrap_or(0.0);
        if met > 1e-6 {
            break;
        }
        to_floor = reach;
    }
    println!("Z screw: {through:.1} mm through the nut up, its tip {to_floor:.2} mm under the sleeve's floor down");
    assert!(through >= 2.0 && to_floor >= 3.0);
    assert!((to_floor - f(&n["rules"]["z_tip_to_sleeve"])).abs() <= 0.25 + 1e-6);
}

/// What the brief asks to be reported before anything is detailed:
/// the envelope, how much of it is behind the router, and what will
/// not print in one piece.
#[test]
fn the_envelope_and_the_parts_too_long_to_print_are_as_reported() {
    let (mut doc, n) = (load(), numbers());
    let pieces = pieces(&mut doc, &n);
    let home = at(&pieces, 0.0, 0.0);
    let (mut lo, mut hi) = (
        Vec3::new(f64::MAX, f64::MAX, f64::MAX),
        Vec3::new(f64::MIN, f64::MIN, f64::MIN),
    );
    for (p, s) in pieces.iter().zip(&home) {
        if p.name.starts_with("Router ") {
            continue; // the bit and the router are what it holds, not the module
        }
        let b = bounds(s);
        lo = Vec3::new(lo.x.min(b.0.x), lo.y.min(b.0.y), lo.z.min(b.0.z));
        hi = Vec3::new(hi.x.max(b.1.x), hi.y.max(b.1.y), hi.z.max(b.1.z));
    }
    let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
    println!(
        "the module is {:.0} long, {:.0} deep, {:.0} tall",
        size[0], size[1], size[2]
    );
    for (k, s) in size.iter().enumerate() {
        assert!((s - f(&n["size"][k])).abs() < 0.01, "axis {k}");
    }
    let router = bounds(named(&pieces, &home, "Router 1"));
    let behind = hi.x - router.1.x;
    assert!((behind - f(&n["behind_router"]["drawn_back"])).abs() < 0.01);
    let outer = bounds(named(&pieces, &home, "Outer box 1"));
    assert!((hi.x - outer.1.x - f(&n["out_of_the_box"])).abs() < 0.01);
    // more of the module's length is column and motor than box
    assert!(hi.x - outer.1.x > outer.1.x - outer.0.x);
    // What the printer's bed will not take in one piece, of the parts that are made and not bought.
    let bed = f(&n["bed"]);
    let mut long = Vec::new();
    for name in [
        "Outer box 1",
        "Z stage 1",
        "X column 1",
        "X quill 1",
        "Spider 1",
        "X drive housing 1",
        "X drive housing 2",
    ] {
        let b = bounds(named(&pieces, &home, name));
        let longest = (b.1.x - b.0.x).max(b.1.y - b.0.y).max(b.1.z - b.0.z);
        if longest > bed {
            long.push(name.trim_end_matches(" 1").to_string());
        }
    }
    let reported: Vec<String> = n["too_long_to_print"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    println!("over the printer's {bed:.0} mm: {long:?}");
    assert_eq!(long, reported);
}
