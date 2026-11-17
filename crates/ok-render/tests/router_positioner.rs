//! The two-axis nested-box router positioner (examples/router-positioner),
//! first pass. The design brief gives rules of thumb and asks for the
//! four corners of the travel to be checked; this does both from the
//! committed document, with the kernel's solids and not the build
//! script's arithmetic: every part is a closed solid; at each corner
//! of the travel no part is through another; each sliding box is in
//! its guide by a width and a half and more, and stays inside it; each
//! screw is through its nut at one end of its travel and short of what
//! it runs toward at the other; the envelope and the length behind
//! the router are what the build reported; and every box is pieces of
//! one of two plywoods, each of them on the cut list.

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

/// The box round every body of one part: a tube is four panels, a stage is cheeks and a foot.
fn whole(pieces: &[Piece], solids: &[Solid], part: &str) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (
        Vec3::new(f64::MAX, f64::MAX, f64::MAX),
        Vec3::new(f64::MIN, f64::MIN, f64::MIN),
    );
    let mut found = 0;
    for (p, s) in pieces.iter().zip(solids) {
        if p.name.rsplit_once(' ').map(|n| n.0) == Some(part) {
            let b = bounds(s);
            lo = Vec3::new(lo.x.min(b.0.x), lo.y.min(b.0.y), lo.z.min(b.0.z));
            hi = Vec3::new(hi.x.max(b.1.x), hi.y.max(b.1.y), hi.z.max(b.1.z));
            found += 1;
        }
    }
    assert!(found > 0, "no part {part}");
    (lo, hi)
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
        assert!(tried > 40);
        assert_eq!(failed, 0);
        assert!(most < 0.5, "{which} share {most:.2} mm3");
    }
    // Past the ends: the quill run out further meets the outer box's face, drawn back further its nut's cage meets
    // the collar on the screw; the stage run up further meets the lid, down further the floor.
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
/// sticky drawer. The quill is guided by one tube, sleeve and column
/// in a piece; the stage by the outer box. Measured on the bodies, at
/// both ends of each travel.
#[test]
fn each_box_is_in_its_guide_by_a_width_and_a_half_everywhere() {
    let (mut doc, n) = (load(), numbers());
    let pieces = pieces(&mut doc, &n);
    let (tx, tz, engage) = (f(&n["travel"]["x"]), f(&n["travel"]["z"]), f(&n["engage"]));
    for (dx, dz) in [(0.0, 0.0), (tx, 0.0), (0.0, tz), (tx, tz)] {
        let solids = at(&pieces, dx, dz);
        let quill = whole(&pieces, &solids, "X quill");
        let stage = whole(&pieces, &solids, "Z stage");
        let tube = whole(&pieces, &solids, "X tube");
        let outer = whole(&pieces, &solids, "Outer box");
        // the quill: all of it between the tube's mouth, which is the stage's, and where the screw's collar is
        assert!((tube.0.x - stage.0.x).abs() < 1e-6);
        let guide = (tube.0.x, f(&n["x"]["column_end"]));
        assert!(quill.0.x >= guide.0 - 1e-6 && quill.1.x <= guide.1 + 1e-6 && guide.1 < tube.1.x);
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
    // The tube is what tips the stage, and the outer box's right wall is cut away for it: the stage is wider than
    // the tube by a cheek each side, and the cheeks' edges bear on what is left of that wall.
    let home = at(&pieces, 0.0, 0.0);
    let (stage, tube) = (
        whole(&pieces, &home, "Z stage"),
        whole(&pieces, &home, "X tube"),
    );
    let rail = ((stage.1.y - stage.0.y) - (tube.1.y - tube.0.y)) / 2.0;
    println!("the stage is {rail:.1} mm wider than the tube each side");
    assert!(rail >= f(&n["rail"]) - 1.0);
    // and the tube is the stage's top: one piece of work, not a column fixed to a block
    assert!((stage.1.z - tube.1.z).abs() < 1e-6);
}

/// Each screw is fixed and its nut travels. At one end of the travel
/// the screw must still be right through the nut; at the other its tip
/// must stop short of what the nut is carrying it into: the router's
/// back cap for X, the tube's floor for Z.
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
    // Z: up, the nut is nearest the tip; down, the tip is nearest the tube's floor. The floor is found with a
    // post stood on the tip: what of the stage or the tube it meets first.
    let screw = bounds(named(&pieces, &home, "Z drive 7"));
    let nut = bounds(named(&pieces, &home, "Z nut 1"));
    let through = screw.1.z - nut.1.z;
    let over: Vec<&Solid> = pieces
        .iter()
        .zip(&far)
        .filter(|(p, _)| p.name.starts_with("Z stage ") || p.name.starts_with("X tube "))
        .map(|(_, s)| s)
        .collect();
    assert!(over.len() >= 10);
    let (cx, tip) = ((screw.0.x + screw.1.x) / 2.0, screw.1.z);
    let mut to_floor = 0.0;
    for k in 1..=400 {
        let reach = k as f64 * 0.25;
        let post = slab(
            Vec3::new(cx - 2.0, -2.0, tip),
            Vec3::new(cx + 2.0, 2.0, tip + reach),
        );
        let met = over
            .iter()
            .map(|s| {
                ok_brep::boolean(&post, s, BoolOp::Intersection)
                    .map(|s| s.volume().abs())
                    .unwrap_or(0.0)
            })
            .fold(0.0, f64::max);
        if met > 1e-6 {
            break;
        }
        to_floor = reach;
    }
    println!("Z screw: {through:.1} mm through the nut up, its tip {to_floor:.2} mm under the tube's floor down");
    assert!(through >= 2.0 && to_floor >= 3.0);
    assert!((to_floor - f(&n["rules"]["z_tip_to_sleeve"])).abs() <= 0.25 + 1e-6);
}

/// What the brief asks to be reported before anything is detailed:
/// the envelope and how much of it is behind the router. And the
/// plywood: every body of every box is a piece of one of the two
/// sheets, or two layers of one glued up, and is on the cut list at
/// its size, with nothing on the list left over.
#[test]
fn the_envelope_is_as_reported_and_every_box_is_pieces_on_the_cut_list() {
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
    let outer = whole(&pieces, &home, "Outer box");
    assert!((hi.x - outer.1.x - f(&n["out_of_the_box"])).abs() < 0.01);
    // more of the module's length is tube and motor than box
    assert!(hi.x - outer.1.x > outer.1.x - outer.0.x);
    // The plywood. The list as the build wrote it: thickness, part, piece, long, wide, how many.
    let ply = [f(&n["ply"][0]), f(&n["ply"][1])];
    assert!((ply[0] - 12.7).abs() < 1.5 && (ply[1] - 19.05).abs() < 1.5 && ply[0] < ply[1]);
    let mut list: Vec<(f64, String, f64, f64, i64)> = n["panels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                f(&r["t"]),
                r["part"].as_str().unwrap().to_string(),
                f(&r["long"]),
                f(&r["wide"]),
                r["count"].as_i64().unwrap(),
            )
        })
        .collect();
    let listed: i64 = list.iter().map(|r| r.4).sum();
    assert!(list.iter().all(|r| r.0 == ply[0] || r.0 == ply[1]));
    let area = |t: f64| -> f64 {
        list.iter()
            .filter(|r| r.0 == t)
            .map(|r| r.2 * r.3 * r.4 as f64)
            .sum()
    };
    assert!((area(ply[1]) - f(&n["area"]["t34"])).abs() < 1.0);
    assert!((area(ply[0]) - f(&n["area"]["t12"])).abs() < 1.0);
    let sheet = f(&n["area"]["sheet"]);
    println!(
        "{listed} pieces: {:.0} % of a sheet of the thick plywood, {:.0} % of one of the thin",
        100.0 * area(ply[1]) / sheet,
        100.0 * area(ply[0]) / sheet
    );
    assert!(area(ply[0]) < sheet && area(ply[1]) < sheet);
    // Every body of the plywood parts is one of those pieces: a box of that thickness, or of two of them.
    let mut bodies = 0;
    for part in n["plywood"].as_array().unwrap() {
        let part = part.as_str().unwrap();
        for (p, s) in pieces.iter().zip(&home) {
            if p.name.rsplit_once(' ').map(|x| x.0) != Some(part) {
                continue;
            }
            bodies += 1;
            let b = bounds(s);
            let d = [b.1.x - b.0.x, b.1.y - b.0.y, b.1.z - b.0.z];
            let mut taken = false;
            'find: for layers in [1i64, 2] {
                for t in ply {
                    for k in 0..3 {
                        if (d[k] - layers as f64 * t).abs() > 1e-6 {
                            continue;
                        }
                        let (a, c) = (d[(k + 1) % 3], d[(k + 2) % 3]);
                        let (long, wide) = (a.max(c), a.min(c));
                        if let Some(row) = list.iter_mut().find(|r| {
                            r.0 == t
                                && r.1 == part
                                && (r.2 - long).abs() < 0.02
                                && (r.3 - wide).abs() < 0.02
                                && r.4 >= layers
                        }) {
                            row.4 -= layers;
                            taken = true;
                            break 'find;
                        }
                    }
                }
            }
            assert!(taken, "{} is {d:?}: not a piece on the list", p.name);
        }
    }
    assert!(bodies >= 30);
    assert!(
        list.iter().all(|r| r.4 == 0),
        "left on the list: {:?}",
        list.iter().filter(|r| r.4 != 0).collect::<Vec<_>>()
    );
}
