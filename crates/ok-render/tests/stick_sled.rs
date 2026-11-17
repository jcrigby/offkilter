//! The stick sled (examples/stick-sled) as a regression suite. The
//! stick only turns; the router positioner beside it sets how deep
//! the bit reaches and how high. The table the controller reads is
//! three numbers a row: the positioner's two screws and the spindle.
//! This works them forward and checks the bit is where the plan wants
//! it, the whole of it never in the tile; that the passes reach the
//! whole outline; that run in the table's order no cut meets more
//! than its bite; that the blank turns clear of the positioner's face
//! and the sled's plate and the rows fit the positioner's travel;
//! that every move marked straight passes the master clear; that the
//! controller's own program, simulated, ends every row on the table's
//! steps and sets the bit again when a stick is taken up part way;
//! that the model is the sled at one row with nothing through
//! anything; that the sled's travel fits its shafts and its stops
//! leave the bit beside the master and past the stick; and, for
//! setting a built machine up, that six touches of a pin on the
//! master find where the bit's height is level with the stick's axis
//! well enough that the table planned for it cuts true on a machine
//! built wrong, and that the probing table and the bands are where
//! the reading of them assumes.

//! and comes at nothing blind, that a sled built wrong, probed by the
//! controller's own program, is found well enough that the table
//! planned for what was found cuts true on the sled as it really is,
//! and that the bands turned to see whether the stick runs true are
//! turned where the reading of them assumes.

use ok_brep::{BoolOp, Solid};
use ok_model::{Document, TabId};
use serde_json::Value;
use std::f64::consts::{PI, TAU};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-sled")
}

fn load() -> Document {
    Document::from_json(&std::fs::read_to_string(dir().join("out/sled.okpart")).unwrap()).unwrap()
}

fn numbers() -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/sled.json")).unwrap()).unwrap()
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

type P2 = (f64, f64);

fn pair(v: &Value) -> P2 {
    (f(&v[0]), f(&v[1]))
}

struct Row {
    n: u64,
    kind: String,
    tool: String,
    t: f64,
    across: f64,
    tip: f64,
    depth: f64,
    height: f64,
    turn: f64,
    straight: bool,
    last_bite: bool,
}

fn rows(n: &Value) -> Vec<Row> {
    n["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let label = r["label"].as_str().unwrap();
            let words: Vec<&str> = label.split(' ').collect();
            let k = words.len();
            Row {
                n: r["n"].as_u64().unwrap(),
                kind: r["kind"].as_str().unwrap().to_string(),
                tool: r["tool"].as_str().unwrap().to_string(),
                t: f(&r["t"]),
                across: f(&r["across"]),
                tip: f(&r["tip"]),
                depth: f(&r["depth"]),
                height: f(&r["height"]),
                turn: f(&r["turn"]),
                straight: r["via"].as_str() == Some("d"),
                last_bite: k >= 3 && words[k - 2] == "of" && words[k - 3] == words[k - 1],
            }
        })
        .collect()
}

fn outline(n: &Value) -> Vec<P2> {
    n["tile"]["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect()
}

/// The ball-nose's outline: how far toward the router from the ball's
/// centre its surface is, `off` from its axis. None beyond the shank.
fn flank(off: f64, r: f64, taper: f64, shank: f64) -> Option<f64> {
    if off <= r * taper.cos() {
        Some(-(r * r - off * off).sqrt())
    } else if off < shank {
        Some((off * taper.cos() - r) / taper.sin())
    } else {
        None
    }
}

/// A point of the section as the cutter sees it with the stick at t: across, and toward the router.
fn seen(p: P2, t: f64) -> P2 {
    (
        -p.0 * t.sin() + p.1 * t.cos(),
        p.0 * t.cos() + p.1 * t.sin(),
    )
}

/// The blank's section: the four corners of a rectangle, in the tile's frame.
fn blank(n: &Value) -> Vec<P2> {
    n["blank"]["corners"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect()
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
        "{:?}",
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

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// The positioner's two axes, as plan.py has them, written again here
/// so the table is checked by other code than made it.
struct Axes {
    tip_home: f64,
    across_home: f64,
    scale: P2,
    shear: f64,
}

impl Axes {
    fn new(n: &Value) -> Axes {
        let a = &n["axes"];
        Axes {
            tip_home: f(&a["tip_home"]),
            across_home: f(&a["across_home"]),
            scale: pair(&a["scale"]),
            shear: f(&a["shear"]),
        }
    }

    /// From a row's three numbers to where the bit's tip is in the stick's section: (across, tip, t).
    /// The quill run out brings the tip toward the stick's axis and past it; the stage run up raises
    /// the bit; the section sits in the machine turned by the spindle's angle, so t is that angle
    /// the other way.
    fn cutter(&self, depth: f64, height: f64, turn: f64) -> (f64, f64, f64) {
        (
            self.across_home + self.scale.1 * height + self.shear * depth,
            self.tip_home - self.scale.0 * depth,
            (-turn.to_radians()).rem_euclid(TAU),
        )
    }
}

/// Each row's three numbers put the bit where the plan says, and the
/// bit is never in the tile.
#[test]
fn every_row_puts_the_bit_where_the_plan_wants_it_and_never_in_the_tile() {
    let n = numbers();
    let (axes, rows, tile) = (Axes::new(&n), rows(&n), outline(&n));
    let ball = f(&n["bit"]) / 2.0;
    let (flat, allow) = (f(&n["rough"]["bit"]) / 2.0, f(&n["rough"]["allow"]));
    let mut worst = 0.0f64;
    let mut gouge = 0.0f64;
    // The finishing bit as it is: a ball, a flank leaning out above it, a shank. The plan must keep all of it out of the tile.
    let (taper, shank) = (f(&n["taper"]).to_radians(), f(&n["shank"]) / 2.0);
    let master = f(&n["sled"]["master_r"]);
    assert!(
        taper > 0.03 && shank >= 2.0 * ball - 1e-9,
        "planned for a tapered bit"
    );
    for r in &rows {
        let (across, tip, t) = axes.cutter(r.depth, r.height, r.turn);
        let miss = (across - r.across)
            .abs()
            .max((tip - r.tip).abs())
            .max((t - r.t).sin().abs() * 30.0);
        worst = worst.max(miss);
        assert!(miss < 2e-3, "row {} is {miss} from its cut", r.n);
        match r.kind.as_str() {
            "finish" => {
                for q in &tile {
                    let (u, h) = seen(*q, r.t);
                    let off = (u - r.across).abs();
                    if let Some(up) = flank(off, ball, taper, shank) {
                        gouge = gouge.max(h - (r.tip + ball + up));
                    }
                }
            }
            "rough" => {
                for q in &tile {
                    let (u, h) = seen(*q, r.t);
                    if (u - r.across).abs() <= flat {
                        assert!(
                            h <= r.tip - allow + 1e-3,
                            "row {} roughs into the tile",
                            r.n
                        );
                    }
                }
            }
            "set" => {
                // run out at the master, level with the stick's axis
                assert!(r.across.abs() < 1e-9 && (r.tip - master).abs() < 1e-9);
            }
            "load" => {
                assert!(r.across.abs() < 1e-9 && (r.tip - f(&n["blank_swing"])).abs() < 1e-9);
            }
            other => panic!("row {} is a {other}", r.n),
        }
    }
    println!(
        "{} rows, the worst {worst:.1e} mm from its cut; the finishing bit, flank and shank too, at most {gouge:.1e} mm into the tile",
        rows.len()
    );
    assert!(gouge < 1e-6);
    let count = |k: &str| rows.iter().filter(|r| r.kind == k).count() as u64;
    for k in ["load", "set", "rough", "finish"] {
        assert_eq!(count(k), n["count"][k].as_u64().unwrap());
    }
    // Two settings of the bit, each on the master's radius: the straight bit before roughing, the ball-nose before the finish.
    let sets: Vec<&Row> = rows.iter().filter(|r| r.kind == "set").collect();
    assert_eq!(sets.len(), 2);
    assert!(sets[0].tool == "straight" && sets[1].tool == "ball");
    // The master is smaller than anything a pass comes near, so the bit, which is beside it with the sled back, is
    // never in it at a row.
    assert!(master < f(&n["tile"]["circle"]) - 1.0 && master > f(&n["spindle_d"]) / 2.0);
    assert!(
        rows[0].kind == "load" && rows[1].kind == "set" && rows.last().unwrap().kind == "finish"
    );
    // the blank goes on with no bit in mind; then the straight bit's rows, then the ball-nose's
    assert!(rows
        .iter()
        .skip(1)
        .skip_while(|r| r.tool == "straight")
        .all(|r| r.tool == "ball"));
}

/// The last bite of every finishing pass, together: the whole outline
/// within a ridge of one, each pass from its own direction, the
/// bit never further above or below the axis than the plan keeps it.
#[test]
fn the_finishing_passes_reach_the_whole_outline_each_from_its_own_direction() {
    let n = numbers();
    let (rows, tile) = (rows(&n), outline(&n));
    let ball = f(&n["bit"]) / 2.0;
    let last: Vec<&Row> = rows
        .iter()
        .filter(|r| r.kind == "finish" && r.last_bite)
        .collect();
    assert_eq!(last.len() as u64, n["plan"]["passes"].as_u64().unwrap());
    let mut left = vec![9.0f64; tile.len()];
    for r in &last {
        let (d, e) = ((r.t.cos(), r.t.sin()), (-r.t.sin(), r.t.cos()));
        let c = r.tip + ball;
        let centre = (r.across * e.0 + c * d.0, r.across * e.1 + c * d.1);
        for (i, q) in tile.iter().enumerate() {
            left[i] = left[i].min((q.0 - centre.0).hypot(q.1 - centre.1) - ball);
        }
        assert!(r.across.abs() <= f(&n["plan"]["across_limit"]) + 1e-9);
    }
    let covered = left.iter().filter(|v| **v < 0.08).count() as f64 / left.len() as f64;
    let worst = left.iter().cloned().fold(0.0, f64::max);
    let mut dirs: Vec<f64> = last.iter().map(|r| r.t).collect();
    dirs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let apart = dirs.windows(2).filter(|w| w[1] - w[0] > 1e-6).count() + 1;
    println!(
        "{} passes from {apart} directions: {:.1} % of the outline within 0.08, the worst {worst:.3}",
        last.len(),
        100.0 * covered
    );
    assert!(covered > 0.995 && worst < 0.1);
    assert!((worst - f(&n["plan"]["worst"])).abs() < 1e-6);
    assert!(
        apart * 10 > last.len() * 6,
        "the passes do not share a dozen stops"
    );
    // Where each pass meets the tile, round the ball from its tip: the point of the outline nearest the ball's centre.
    // The tip's dead centre does not turn, so the plan keeps off it where it can.
    let mut round: Vec<f64> = last
        .iter()
        .map(|r| {
            let (d, e) = ((r.t.cos(), r.t.sin()), (-r.t.sin(), r.t.cos()));
            let c = r.tip + ball;
            let centre = (r.across * e.0 + c * d.0, r.across * e.1 + c * d.1);
            let q = tile
                .iter()
                .min_by(|a, b| {
                    (a.0 - centre.0)
                        .hypot(a.1 - centre.1)
                        .partial_cmp(&(b.0 - centre.0).hypot(b.1 - centre.1))
                        .unwrap()
                })
                .unwrap();
            let v = (q.0 - centre.0, q.1 - centre.1);
            (-(v.0 * d.0 + v.1 * d.1) / v.0.hypot(v.1))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        })
        .collect();
    round.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let dead = round.iter().filter(|a| **a < 5.0).count();
    println!(
        "passes meet the tile a median {:.0} deg round the ball from its tip; {dead} within 5 deg of dead centre",
        round[round.len() / 2]
    );
    assert!(
        dead * 20 < last.len()
            && (round[round.len() / 2] - f(&n["plan"]["off_centre"])).abs() < 4.0
    );
    assert!(last.len() > 150 && last.len() < 400);
    assert!(n["plan"]["orphans"].as_u64() == Some(0));
    // Bites: every pass within the ball-nose's limit, the roughing before it within the straight bit's, most of the waste to the straight bit.
    let g = &n["rough"];
    let (waste, rough, end) = (f(&g["waste"]), f(&g["rough_area"]), f(&g["left_at_end"]));
    println!(
        "waste {waste:.0} mm2: {} roughing runs take {rough:.0}, {} finishing runs the rest, {end:.1} stands",
        g["runs"], n["plan"]["finish_runs"]
    );
    assert!(rough > 0.7 * waste && end < 0.01 * waste);
    assert!(f(&n["plan"]["deepest"]) / f(&n["plan"]["ball_bite"]) < 12.0);
    for pair in rows.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.kind == b.kind
            && (a.kind == "rough" || a.kind == "finish")
            && !a.last_bite
            && (a.t - b.t).abs() < 1e-9
            && (a.across - b.across).abs() < 1e-9
        {
            let bite = a.tip - b.tip;
            let most = if a.kind == "rough" {
                f(&g["bite"])
            } else {
                f(&n["plan"]["ball_bite"])
            };
            assert!(
                bite > 0.0 && bite <= most + 1e-6,
                "row {} bites {bite}",
                b.n
            );
        }
    }
}

fn inside(q: P2, poly: &[P2]) -> bool {
    let mut hit = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[(i + poly.len() - 1) % poly.len()], poly[i]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) {
            hit = !hit;
        }
    }
    hit
}

/// The waste, the blank less the tile, on a grid of its own, and the
/// table's cutting rows run over it in the table's order: no row
/// meets more than a hair-wide fin standing higher than its bit's bite
/// in what the rows before it left, and at the end nothing but the
/// ridges stands. The planner counts
/// its bites the same way; this is that count made again, from the
/// rows as written, so an order the bites were not counted in shows.
#[test]
fn in_the_table_s_order_no_row_takes_more_than_its_bite_of_what_stands() {
    let n = numbers();
    let (rows, tile) = (rows(&n), outline(&n));
    let stock = blank(&n);
    let cell = 0.25;
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for q in &stock {
        lo = (lo.0.min(q.0), lo.1.min(q.1));
        hi = (hi.0.max(q.0), hi.1.max(q.1));
    }
    let mut cells: Vec<P2> = Vec::new();
    let (nx, ny) = (
        ((hi.0 - lo.0) / cell) as usize + 1,
        ((hi.1 - lo.1) / cell) as usize + 1,
    );
    for i in 0..nx {
        for k in 0..ny {
            let q = (
                lo.0 + (i as f64 + 0.5) * cell,
                lo.1 + (k as f64 + 0.5) * cell,
            );
            if inside(q, &stock) && !inside(q, &tile) {
                cells.push(q);
            }
        }
    }
    let waste = cells.len() as f64 * cell * cell;
    assert!((waste - f(&n["rough"]["waste"])).abs() < 0.02 * waste);
    let mut alive = vec![true; cells.len()];
    let ball = f(&n["bit"]) / 2.0;
    let (taper, shank) = (f(&n["taper"]).to_radians(), f(&n["shank"]) / 2.0);
    let flat = f(&n["rough"]["bit"]) / 2.0;
    let (rough_bite, ball_bite) = (f(&n["rough"]["bite"]), f(&n["plan"]["ball_bite"]));
    let (mut deep_rough, mut deep_ball, mut rough_took, mut most_fin) =
        (0.0f64, 0.0f64, 0usize, 0.0f64);
    for r in rows
        .iter()
        .filter(|r| r.kind == "rough" || r.kind == "finish")
    {
        let most = if r.tool == "ball" {
            ball_bite
        } else {
            rough_bite
        };
        let (mut depth, mut tall) = (0.0f64, 0usize);
        for (i, q) in cells.iter().enumerate() {
            if !alive[i] {
                continue;
            }
            let (u, h) = seen(*q, r.t);
            let off = (u - r.across).abs();
            let stands = if r.tool == "ball" {
                match flank(off, ball, taper, shank) {
                    Some(up) => h - (r.tip + ball + up),
                    None => continue,
                }
            } else if off <= flat {
                h - r.tip
            } else {
                continue;
            };
            if stands >= 0.0 {
                alive[i] = false;
                depth = depth.max(stands);
                if stands > most + 0.1 {
                    tall += 1;
                }
                if r.tool != "ball" {
                    rough_took += 1;
                }
            }
        }
        // What stands higher than a bite over the cutter is, at most, a fin left between two earlier cuts: a hair wide.
        // The area of it says so; its height alone would call a fin a full cut.
        let fin = tall as f64 * cell * cell;
        most_fin = most_fin.max(fin);
        assert!(
            fin <= 0.4,
            "row {} meets {fin:.2} mm2 standing more than its bit's bite of {most}, up to {depth:.1} mm",
            r.n
        );
        if r.tool == "ball" {
            deep_ball = deep_ball.max(depth)
        } else {
            deep_rough = deep_rough.max(depth)
        }
    }
    let left = alive.iter().filter(|a| **a).count() as f64 * cell * cell;
    let rough = rough_took as f64 * cell * cell;
    println!(
        "waste {waste:.0} mm2 run in the table's order: at most {most_fin:.2} mm2 ever stands over a bite \
         (fins, up to {deep_rough:.1} and {deep_ball:.1} mm high); roughing takes {rough:.0}, {left:.1} stands at the end"
    );
    assert!(rough > 0.7 * waste && left < 0.015 * waste);
}

/// The bit's outline from its tip: points (toward the router, across).
fn bit_edge(n: &Value, tool: &str) -> Vec<P2> {
    let mut pts = Vec::new();
    if tool == "ball" {
        let ball = f(&n["bit"]) / 2.0;
        let (taper, shank) = (f(&n["taper"]).to_radians(), f(&n["shank"]) / 2.0);
        let mut far = 0.0f64;
        for k in 0..=80 {
            let off = shank * k as f64 / 80.0;
            if let Some(up) = flank(off, ball, taper, shank) {
                pts.push((ball + up, off));
                pts.push((ball + up, -off));
                far = far.max(ball + up);
            }
        }
        for k in 1..80 {
            pts.push((far + 0.5 * k as f64, shank));
            pts.push((far + 0.5 * k as f64, -shank));
        }
    } else {
        let flat = f(&n["rough"]["bit"]) / 2.0;
        for k in -12..=12 {
            pts.push((0.0, flat * k as f64 / 12.0));
        }
        for k in 1..120 {
            pts.push((0.5 * k as f64, flat));
            pts.push((0.5 * k as f64, -flat));
        }
    }
    pts
}

/// The positioner's face is a wall beside the stick and the sled's
/// plate a floor under it: the uncut blank turns clear of both. The
/// headstock and tailstock pass that wall as the sled travels, and
/// the wheel, hung off the plate's head end, never comes to it. Every
/// row is within the positioner's travel, with room for a bit set a
/// little long or short. The collet nut never comes within the
/// blank's swing, and that is what sets how far the positioner
/// stands from the stick: the nut is out of its face at the deepest
/// cut, by the router's nose.
#[test]
fn the_blank_turns_clear_and_the_rows_fit_the_positioner_s_travel() {
    let n = numbers();
    let (rows, stock) = (rows(&n), blank(&n));
    let (s, a, p) = (&n["sled"], &n["axes"], &n["positioner"]);
    let swing = stock.iter().map(|q| q.0.hypot(q.1)).fold(0.0, f64::max);
    assert!((swing - f(&n["blank_swing"])).abs() < 1e-9);
    let (face, za, top) = (f(&s["face"]), f(&s["za"]), f(&s["top"]));
    println!(
        "the blank swings {swing:.1}: {:.1} clear of the positioner's face, {:.1} of the sled's plate",
        face - swing,
        za - top - swing
    );
    assert!(face - swing >= f(&s["face_clear"]) - 1e-9);
    assert!(za - top - swing >= f(&s["bed_clear"]));
    assert!((f(&p["box_y"][0]) - face).abs() < 1e-9);
    // the uprights and the plate stay on the stick's side of that face
    assert!(f(&s["stock_half"]) + f(&s["face_clear"]) <= face && f(&s["plate_y"][1]) < face - 1.0);
    // the stick's axis is level with the middle of the height's travel
    let travel = pair(&a["travel"]);
    assert!((f(&p["bit_home_z"]) + travel.1 / 2.0 - za).abs() < 1e-9);
    assert!((f(&a["across_home"]) + travel.1 / 2.0).abs() < 1e-9);
    // The wheel: lower than the plate's top, so it hangs off the plate's head end, clear of the positioner's box
    // with the sled back, which is as near as it comes; and over the shafts' supports.
    let (rim, wx) = (f(&s["wheel_rim"]), pair(&s["wheel_x"]));
    assert!(za - rim < top && wx.1 < f(&s["plate_x"][0]));
    assert!(f(&s["back_x"]) - f(&p["half_box"]) - wx.1 >= 5.0);
    assert!(za - rim >= f(&s["sk"][3]) + 3.0);
    let w = &s["wheel"];
    let (big, small, c) = (f(&w[0]) * 2.0 / PI, f(&w[1]) * 2.0 / PI, f(&w[3]));
    let belt = 2.0 * c + PI * (big + small) / 2.0 + (big - small).powi(2) / (4.0 * c);
    assert!((belt - f(&w[2])).abs() < 0.01, "the belt is {belt}");
    assert!(c > rim + small / 2.0 + 20.0, "the motor clears the wheel");
    // the rows in the travel
    let (margin, past, probe) = (f(&a["bit_margin"]), f(&a["past"]), pair(&a["probe"]));
    let (mut d, mut h) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    let mut nut = f64::MAX;
    for r in &rows {
        d = (d.0.min(r.depth), d.1.max(r.depth));
        h = (h.0.min(r.height), h.1.max(r.height));
        nut = nut.min(r.tip + f(&a["bit_out"]));
    }
    println!(
        "depth {:.1} to {:.1} of {:.0}, height {:.1} to {:.1} of {:.0}; the collet nut never nearer the axis than {nut:.1}",
        d.0, d.1, travel.0, h.0, h.1, travel.1
    );
    assert!(d.0 >= past + margin && d.1 <= travel.0 - margin + 1e-6);
    assert!(h.0 >= probe.1 + 2.0 && h.1 <= travel.1 - probe.1 - 2.0);
    assert!(nut >= swing);
    // Run out for the deepest cut the nut is this far in front of the positioner's face: more than the blank's own
    // clearance of the face, so it is the nut that says where the face is, and the face is no further off than that.
    let nut_out = travel.0 - margin - f(&p["nut_behind"]);
    assert!(
        nut_out > f(&s["face_clear"]),
        "the nut is {nut_out} out of the face"
    );
    assert!(face - swing >= nut_out && face - swing < nut_out + 1.0);
    assert!((nut - (face - nut_out)).abs() < 1e-3);
    // a bit is set within the travel too, its probe's window and the margin with it
    let set = rows.iter().find(|r| r.kind == "set").unwrap();
    assert!(set.depth - probe.0 - margin > 0.0 && set.depth + probe.0 + margin < travel.0);
    // the bit stands out of the collet nut by what that takes: long, but a tapered ball-nose is made that long
    assert!(f(&a["bit_out"]) > 35.0 && f(&a["bit_out"]) < 50.0);
}

/// With the sled back the bit is beside the master. A move the table
/// marks straight goes to its row without first drawing the bit back:
/// the height first, a move down going a little past and coming up,
/// then the bit run out or in. Followed along that path, the whole
/// of the bit stays clear of the master. And drawn back to the safe
/// depth, it is clear of the master at any height.
#[test]
fn every_move_marked_straight_passes_the_master_clear() {
    let n = numbers();
    let (axes, rows) = (Axes::new(&n), rows(&n));
    let a = &n["axes"];
    let (master, clear, past) = (
        f(&n["sled"]["master_r"]),
        f(&a["move_clear"]),
        f(&a["past"]),
    );
    let edges = [bit_edge(&n, "straight"), bit_edge(&n, "ball")];
    let gap = |tool: &str, depth: f64, height: f64| -> f64 {
        let (across, tip, _) = axes.cutter(depth, height, 0.0);
        edges[if tool == "ball" { 1 } else { 0 }]
            .iter()
            .map(|q| (tip + q.0).hypot(across + q.1))
            .fold(f64::MAX, f64::min)
            - master
    };
    let (mut straight, mut least) = (0, f64::MAX);
    for i in 1..rows.len() {
        let (was, row) = (&rows[i - 1], &rows[i]);
        if !row.straight {
            continue;
        }
        straight += 1;
        let cutting = |k: &str| k == "rough" || k == "finish";
        assert!(
            cutting(&was.kind) && cutting(&row.kind),
            "a setting of the bit is never gone to or left straight"
        );
        let low = if row.height < was.height {
            row.height - past
        } else {
            row.height
        };
        let steps = ((low - was.height).abs() / 0.1).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let h = was.height + (low - was.height) * k as f64 / steps as f64;
            least = least.min(gap(&row.tool, was.depth, h));
        }
        let steps = ((row.depth - was.depth).abs() / 0.1).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let d = was.depth + (row.depth - was.depth) * k as f64 / steps as f64;
            least = least.min(gap(&row.tool, d, row.height));
        }
    }
    println!(
        "{straight} of {} moves go straight: the bit never nearer the master than {least:.2}",
        rows.len()
    );
    assert_eq!(straight as u64, n["straight"].as_u64().unwrap());
    assert!(straight * 10 > rows.len() * 6 && least >= clear - 1e-3);
    // at its own row a bit is clear of the master too: the master is inside the tile, and the bit never is
    for r in rows
        .iter()
        .filter(|r| r.kind == "rough" || r.kind == "finish")
    {
        assert!(gap(&r.tool, r.depth, r.height) > 1.0, "row {}", r.n);
    }
    // drawn back to the safe depth
    let (safe, travel) = (f(&a["safe"]), pair(&a["travel"]));
    for tool in ["straight", "ball"] {
        for k in 0..=160 {
            let g = gap(tool, safe, travel.1 * k as f64 / 160.0);
            assert!(g >= f(&a["safe_clear"]) - 1e-6);
        }
    }
}

/// The controller's own program, run on a computer over the table:
/// it ends every row on the table's steps; a setting of the bit ends a
/// probe's window short of the master, where its touch begins; it
/// turns the spindle little; and it never changes height or turns
/// with the bit further out than the safe depth, except on the moves
/// the table marks straight. Taken up half way, it sets the bit again
/// before it cuts.
#[test]
fn the_controller_ends_every_row_on_the_table_and_sets_the_bit_again_when_taken_up() {
    let n = numbers();
    let rows = rows(&n);
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    let per = [f(&sim["per"][0]), f(&sim["per"][1]), f(&sim["per"][2])];
    let turn_steps = (360.0 * per[2]).round();
    // a turn of the spindle is a whole number of steps
    assert!((360.0 * per[2] - turn_steps).abs() < 1e-9);
    let window = f(&n["axes"]["probe"][0]);
    let check = |done: &[Value], rows: &[Row]| {
        assert_eq!(done.len(), rows.len());
        for (r, s) in rows.iter().zip(done) {
            assert_eq!(s[0].as_u64().unwrap(), r.n);
            let depth = if r.kind == "set" {
                (r.depth - window).max(0.0)
            } else {
                r.depth
            };
            assert!(
                (f(&s[1]) - depth * per[0]).abs() <= 0.5 + 1e-6,
                "row {} depth",
                r.n
            );
            assert!(
                (f(&s[2]) - r.height * per[1]).abs() <= 0.5 + 1e-6,
                "row {} height",
                r.n
            );
            let off = (f(&s[3]) - r.turn * per[2]).rem_euclid(turn_steps);
            assert!(
                off <= 0.5 + 1e-6 || off >= turn_steps - 0.5 - 1e-6,
                "row {} turn",
                r.n
            );
        }
    };
    check(sim["rows"].as_array().unwrap(), &rows);
    let turned = f(&sim["turned"]) / per[2];
    println!(
        "the spindle turns {turned:.0} deg in all, coming up to its {} angles",
        rows.len()
    );
    assert!(turned < 4.0 * 360.0);
    let safe = f(&n["axes"]["safe"]);
    assert!((f(&sim["safe_steps"]) - (safe * per[0]).round()).abs() < 1e-9);
    assert!(f(&sim["deepest_crossing"]) <= f(&sim["safe_steps"]));
    assert_eq!(sim["went_straight"].as_u64(), n["straight"].as_u64());
    // two bits set, on a computer each found just where the table expects it
    let bits = sim["bits"].as_array().unwrap();
    assert_eq!(bits.len(), 2);
    assert!(bits.iter().all(|b| f(&b[1]).abs() < 1e-9));
    // Left half way and taken up again: it homes, sets the bit that was in the collet again, and ends every
    // remaining row on the same steps.
    let again = &sim["resumed"];
    let after = again["after"].as_u64().unwrap() as usize;
    assert!(after > 100 && after < rows.len() - 100);
    check(again["rows"].as_array().unwrap(), &rows[after..]);
    let reset = again["bits"].as_array().unwrap();
    assert_eq!(reset.len(), 1);
    let last_set = rows[..after]
        .iter()
        .rev()
        .find(|r| r.kind == "set")
        .unwrap();
    assert_eq!(reset[0][0].as_u64().unwrap(), last_set.n);
    assert!(f(&again["deepest_crossing"]) <= f(&again["safe_steps"]));
}

/// The document is the sled at one finishing row: the stick is the
/// tile's prism, the bit's tip is on it, and no part is through
/// another.
#[test]
fn the_model_is_the_sled_at_one_row_with_the_bit_on_the_stick() {
    let n = numbers();
    let (rows, tile) = (rows(&n), outline(&n));
    let mut doc = load();
    let row = &rows[n["drawn"]["row"].as_u64().unwrap() as usize - 1];
    assert!(row.kind == "finish" && row.last_bite);
    let s = &n["sled"];
    let (za, face) = (f(&s["za"]), f(&s["face"]));
    // the stick: the tile's section, the stick's length
    let stick = bodies_of(&mut doc, "Stick").remove(0);
    let expect = f(&n["tile"]["area"]) * f(&n["stick"]);
    assert!((stick.volume() - expect).abs() < 0.002 * expect);
    // the bit: its tip where the row has it from the stick's axis, level with the row's height
    let bit = bodies_of(&mut doc, "Bit").remove(0);
    let (lo, hi) = bit.bounds().unwrap();
    let ball = f(&n["bit"]) / 2.0;
    assert!(
        (lo.y - row.tip).abs() < 1e-6,
        "the tip is {} from the axis",
        lo.y
    );
    // the table's numbers are written to a tenth of a micrometre
    assert!(((lo.z + hi.z) / 2.0 - (za + row.across)).abs() < 1e-3);
    assert!(((lo.x + hi.x) / 2.0 - f(&n["drawn"]["x"])).abs() < 1e-6);
    assert!((hi.y - lo.y - f(&n["axes"]["bit_out"])).abs() < 1e-6);
    // and the ball on the tile: its centre a radius from the outline
    let (d, e) = ((row.t.cos(), row.t.sin()), (-row.t.sin(), row.t.cos()));
    let c = row.tip + ball;
    let centre = (row.across * e.0 + c * d.0, row.across * e.1 + c * d.1);
    let off = tile
        .iter()
        .map(|q| (q.0 - centre.0).hypot(q.1 - centre.1))
        .fold(f64::MAX, f64::min)
        - ball;
    assert!(off.abs() < 0.05, "the ball is {off} off the tile");
    // every part, and what is through what. Parts fitted on each other share a surface: a shaft in its supports and
    // blocks, a bearing in its seat. The bit is drawn as a plain cylinder to its tip, so it is left out.
    let mut all: Vec<(String, Solid)> = Vec::new();
    for (name, list) in n["bodies"].as_object().unwrap() {
        let solids = bodies_of(&mut doc, name);
        assert_eq!(solids.len(), list.as_array().unwrap().len(), "{name}");
        for solid in solids {
            all.push((name.clone(), solid));
        }
    }
    let fitted = |a: &str, b: &str| {
        let pair = |x: &str, y: &str| (a == x && b == y) || (a == y && b == x);
        pair("Shafts", "Shaft supports SK20")
            || pair("Shafts", "Bearing blocks SC20UU")
            || pair("Bearings 6201", "Headstock")
    };
    let (mut tried, mut worst) = (0, (0.0f64, String::new()));
    for i in 0..all.len() {
        for k in i + 1..all.len() {
            let (a, b) = (&all[i], &all[k]);
            if a.0 == "Bit" || b.0 == "Bit" || fitted(&a.0, &b.0) {
                continue;
            }
            let (p, q) = (a.1.bounds().unwrap(), b.1.bounds().unwrap());
            if p.0.x >= q.1.x
                || q.0.x >= p.1.x
                || p.0.y >= q.1.y
                || q.0.y >= p.1.y
                || p.0.z >= q.1.z
                || q.0.z >= p.1.z
            {
                continue;
            }
            tried += 1;
            let common = overlap(&a.1, &b.1);
            if common > worst.0 {
                worst = (common, format!("{} and {}", a.0, b.0));
            }
        }
    }
    println!(
        "{} bodies, {tried} pairs near each other: the most in common {:.3} mm3 ({})",
        all.len(),
        worst.0,
        worst.1
    );
    assert!(all.len() >= 30 && tried >= 10);
    assert!(worst.0 < 1.0, "{} share {:.1} mm3", worst.1, worst.0);
    // the positioner is drawn where the row puts it: its face a wall at the stick's side
    let box_ = bodies_of(&mut doc, "Router positioner").remove(0);
    assert!((box_.bounds().unwrap().0.y - face).abs() < 1e-6);
    // and the check can tell: the stick moved its own width toward that wall is in it
    let moved = stick.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
        0.0, 30.0, 0.0,
    )));
    assert!(overlap(&moved, &box_) > 100.0);
}

/// The blank is a thing a saw and a planer make: a rectangle, whole
/// millimetres each way, with the tile inside it and 2 mm to spare on
/// every side. The table's first row turns the spindle to where it
/// goes on with a wide face level and underneath, and says where the
/// centre is from that face and from the one toward the router.
#[test]
fn the_blank_is_a_rectangle_about_the_tile_and_goes_on_a_wide_face_down() {
    let n = numbers();
    let (axes, rows, tile, stock) = (Axes::new(&n), rows(&n), outline(&n), blank(&n));
    let b = &n["blank"];
    let (size, over) = (pair(&b["size"]), f(&b["over"]));
    assert_eq!(stock.len(), 4);
    let side = |i: usize| {
        let (p, q) = (stock[i], stock[(i + 1) % 4]);
        (q.0 - p.0, q.1 - p.1)
    };
    for i in 0..4 {
        let (u, v) = (side(i), side((i + 1) % 4));
        assert!((u.0 * v.0 + u.1 * v.1).abs() < 1e-6, "a square corner");
        let want = if i % 2 == 0 { size.0 } else { size.1 };
        assert!((u.0.hypot(u.1) - want).abs() < 1e-6);
    }
    assert!((size.0 - size.0.round()).abs() < 1e-9 && (size.1 - size.1.round()).abs() < 1e-9);
    let mut least = [f64::MAX; 4];
    for q in &tile {
        assert!(inside(*q, &stock));
        for (i, slot) in least.iter_mut().enumerate() {
            let (p, u) = (stock[i], side(i));
            let off = ((q.0 - p.0) * u.1 - (q.1 - p.1) * u.0).abs() / u.0.hypot(u.1);
            *slot = slot.min(off);
        }
    }
    println!(
        "blank {:.0} x {:.0}: the tile is {:.2} to {:.2} mm inside its faces",
        size.0,
        size.1,
        least.iter().fold(f64::MAX, |m, x| m.min(*x)),
        least.iter().fold(0.0f64, |m, x| m.max(*x))
    );
    assert!(least
        .iter()
        .all(|&m| m >= over - 1e-6 && m < over + 0.5 + 1e-6));
    // The first row: nothing is cut. The blank as it hangs there: up, and toward the router.
    let load = &rows[0];
    assert_eq!(load.kind, "load");
    assert!(rows.iter().skip(1).all(|r| r.kind != "load"));
    let (_, _, t) = axes.cutter(load.depth, load.height, load.turn);
    let hung: Vec<P2> = stock.iter().map(|q| seen(*q, t)).collect();
    let mut order: Vec<usize> = (0..4).collect();
    order.sort_by(|&i, &k| hung[i].0.partial_cmp(&hung[k].0).unwrap());
    let (a, c) = (hung[order[0]], hung[order[1]]);
    assert!((a.0 - c.0).abs() < 1e-3, "the face underneath is level");
    assert!(
        ((a.1 - c.1).abs() - size.0.max(size.1)).abs() < 1e-3,
        "and it is a wide one"
    );
    let (under, toward) = (f(&b["load"]["under"]), f(&b["load"]["toward"]));
    assert!(
        (-a.0 - under).abs() < 1e-3,
        "the centre is that far up from it"
    );
    let router_side = hung.iter().map(|q| q.1).fold(f64::MIN, f64::max);
    assert!(
        (router_side - toward).abs() < 1e-3,
        "and that far in from the face toward the router"
    );
    assert!(under > 15.0 && toward > 15.0);
}

/// The sled rides the owner's two shafts on four bearing blocks. Over
/// the whole of its travel every block stays on its shaft between the
/// supports; a stop post on the base meets the plate at each end:
/// against the back one the bit is beside the master, clear of the
/// headstock and of the stick's end, against the forward one it is
/// past the stick's far end.
#[test]
fn the_sled_s_travel_fits_its_shafts_and_its_stops_put_the_bit_beside_the_master() {
    let n = numbers();
    let s = &n["sled"];
    let (plate, stops, drawn) = (pair(&s["plate_x"]), pair(&s["stops_x"]), f(&s["drawn_x"]));
    // The stops are in the base's frame, the bit at 0. With the plate's end against one, the bit is here along the sled:
    let back = plate.1 - stops.1;
    let forward = plate.0 - stops.0;
    let (master, bit_r) = (pair(&s["master_x"]), f(&n["rough"]["bit"]) / 2.0);
    println!(
        "sled back, the bit is {back:.0} along it, the master from {:.0} to {:.0}; forward, {forward:.0}, the stick {:.0} long; {:.0} of travel",
        master.0,
        master.1,
        f(&n["stick"]),
        forward - back
    );
    assert!((back - f(&s["back_x"])).abs() < 1e-9);
    assert!(back - bit_r > master.0 + 5.0 && back + bit_r < master.1 - 2.0);
    assert!((forward - (f(&n["stick"]) + f(&s["run"]))).abs() < 1e-9);
    assert!((forward - back - f(&s["travel"])).abs() < 1e-9);
    // the shafts
    let (sk_t, blk_l) = (f(&s["sk"][1]), f(&s["blk"][1]));
    let mut doc = load();
    let shafts = bodies_of(&mut doc, "Shafts");
    assert_eq!(shafts.len(), 2);
    for i in 0..2 {
        let (span, blocks, length) = (
            pair(&s["spans"][i]),
            pair(&s["blocks_x"][i]),
            f(&s["shafts"][i]),
        );
        assert!((span.1 - span.0 - length).abs() < 1e-9);
        // the blocks' ends in the base's frame, the sled forward and back
        let lo = blocks.0 - blk_l / 2.0 - forward;
        let hi = blocks.1 + blk_l / 2.0 - back;
        let room = (lo - (span.0 + sk_t)).min(span.1 - sk_t - hi);
        println!(
            "the {length:.0} shaft: blocks {:.0} apart, {room:.1} mm to spare at each end of the travel",
            blocks.1 - blocks.0
        );
        assert!(room >= 2.0);
        let (lo, hi) = shafts[i].bounds().unwrap();
        assert!((hi.x - lo.x - length).abs() < 1e-6);
        assert!(
            (lo.x - (span.0 + drawn)).abs() < 1e-6,
            "drawn where the bit is {drawn} along the sled"
        );
    }
    assert_eq!(bodies_of(&mut doc, "Shaft supports SK20").len(), 4);
    assert_eq!(bodies_of(&mut doc, "Bearing blocks SC20UU").len(), 4);
    assert_eq!(bodies_of(&mut doc, "Stops").len(), 2);
    assert_eq!(bodies_of(&mut doc, "Sled-back switch").len(), 1);
    // drawn mid-pass, the plate is between its stops and touches neither
    assert!(stops.0 + drawn < plate.0 - 50.0 && stops.1 + drawn > plate.1 + 50.0);
}

/// A machine's two travels as they really are, for the setting up.
struct Real {
    tip_home: f64,
    across_home: f64,
    scale: P2,
    shear: f64,
    ecc: P2,
    master_r: f64,
    pin_r: f64,
}

impl Real {
    /// (tip, across) of the tool's tip.
    fn tip(&self, depth: f64, height: f64) -> P2 {
        (
            self.tip_home - self.scale.0 * depth,
            self.across_home + self.scale.1 * height + self.shear * depth,
        )
    }

    /// The master's middle from the stick's axis, the spindle turned: (toward the router, up).
    fn master(&self, turn: f64) -> P2 {
        let a = turn.to_radians();
        (
            self.ecc.0 * a.cos() - self.ecc.1 * a.sin(),
            self.ecc.0 * a.sin() + self.ecc.1 * a.cos(),
        )
    }

    /// Between a pin in the collet, a round bar with a flat end, and the master.
    fn pin_gap(&self, depth: f64, height: f64, turn: f64) -> f64 {
        let ((tip, across), c) = (self.tip(depth, height), self.master(turn));
        (tip - c.0)
            .max(0.0)
            .hypot(((across - c.1).abs() - self.pin_r).max(0.0))
            - self.master_r
    }

    /// The depth at which a ball-nose run out at this height meets the master.
    fn ball_touch(&self, height: f64, ball: f64) -> f64 {
        let c = self.master(0.0);
        let (mut lo, mut hi) = (0.0, (self.tip_home - c.0) / self.scale.0);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            let (tip, across) = self.tip(mid, height);
            if (tip + ball - c.0).hypot(across - c.1) > self.master_r + ball {
                lo = mid
            } else {
                hi = mid
            }
        }
        hi
    }
}

/// One machine built wrong, the whole way: the controller's own
/// program touching a pin on its master at six places, the arithmetic
/// on what it found, and the table planned for that run on the
/// machine as it really is, its bit set by touch as the controller
/// sets it. Worked again here from the recorded touches.
#[test]
fn six_touches_find_the_bit_s_height_and_the_table_planned_for_it_cuts_true() {
    let n = numbers();
    let c: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/calibration.json")).unwrap())
            .unwrap();
    let (d, e) = (&c["drawn"], &c["example"]);
    let t = &e["true"];
    let real = Real {
        tip_home: f(&d["tip_home"]),
        across_home: f(&t["across_home"]),
        scale: pair(&t["scale"]),
        shear: f(&t["shear"]),
        ecc: pair(&t["ecc"]),
        master_r: f(&t["master_r"]),
        pin_r: f(&t["pin_r"]),
    };
    // it is built wrong: the height's home is not where the drawing has it
    assert!((real.across_home - f(&d["across_home"])).abs() > 0.5);
    let (master_r, pin_r) = (f(&d["master_r"]), f(&c["pin_d"]) / 2.0);
    let touches = e["touches"].as_array().unwrap();
    assert_eq!(touches.len(), 6);
    let at = |kind: &str, turn: f64| -> P2 {
        let t = touches
            .iter()
            .find(|t| t[1].as_str() == Some(kind) && (f(&t[4]) - turn).abs() < 1e-6)
            .unwrap_or_else(|| panic!("no {kind} at {turn}"));
        (f(&t[2]), f(&t[3]))
    };
    for t in touches {
        let g = real.pin_gap(f(&t[2]), f(&t[3]), f(&t[4]));
        assert!(g.abs() < 0.03, "row {} is {g} from a touch", t[0]);
        assert!(f(&t[5]) <= 0.05);
    }
    // The arithmetic: under and over the master are the master and the pin apart; their middle is level with it;
    // half a turn puts its runout the other way.
    let reach = master_r + pin_r;
    let apart = [
        at("dn", 0.0).1 - at("up", 0.0).1,
        at("dn", 180.0).1 - at("up", 180.0).1,
    ];
    let scale = 2.0 * reach / ((apart[0] + apart[1]) / 2.0);
    let middle = [
        (at("dn", 0.0).1 + at("up", 0.0).1) / 2.0,
        (at("dn", 180.0).1 + at("up", 180.0).1) / 2.0,
    ];
    let level = (middle[0] + middle[1]) / 2.0;
    let ecc = (
        (at("in", 180.0).0 - at("in", 0.0).0) / 2.0,
        scale * (middle[0] - middle[1]) / 2.0,
    );
    let found = &e["found"];
    assert!((scale - f(&found["scale_z"])).abs() < 1e-9);
    assert!((-scale * level - f(&found["across_home"])).abs() < 1e-9);
    assert!(
        (ecc.0 - f(&found["ecc"][0])).abs() < 1e-9 && (ecc.1 - f(&found["ecc"][1])).abs() < 1e-9
    );
    // and it is right: at the depth the touches were made, that height is level with the stick's axis
    let off = real.tip(at("up", 0.0).0, level).1;
    println!(
        "found level at height {level:.3}: {:.0} um off the stick's axis there; the screw {scale:.4} to one (really {:.4}); the master's runout ({:.3}, {:.3}) (really ({:.3}, {:.3}))",
        1000.0 * off,
        real.scale.1,
        ecc.0,
        ecc.1,
        real.ecc.0,
        real.ecc.1
    );
    assert!(off.abs() < 0.02 && (scale - real.scale.1).abs() < 0.002);
    assert!((ecc.0 - real.ecc.0).abs() < 0.02 && (ecc.1 - real.ecc.1).abs() < 0.02);
    // The table planned for what was found, on the real machine. The bit is run out, level with where the master's
    // middle is taken to be, until it touches; its tip is then taken to be the master's radius and runout from the
    // axis, and every depth is counted from there.
    let ball = f(&c["ball_r"]);
    let across_home = -scale * level;
    let height_for = |across: f64| (across - across_home) / scale;
    let (set_tip, set_across) = (master_r + ecc.0, ecc.1);
    let touched = real.ball_touch(height_for(set_across), ball);
    let mut error = 0.0f64;
    let mut count = 0;
    for r in rows(&n)
        .iter()
        .filter(|r| r.kind == "finish" && r.last_bite)
    {
        let depth = touched + (set_tip - r.tip);
        let (tip, across) = real.tip(depth, height_for(r.across));
        let q = seen_back((across, tip + ball), r.t);
        let p = seen_back((r.across, r.tip + ball), r.t);
        error = error.max((q.0 - p.0).hypot(q.1 - p.1));
        count += 1;
    }
    println!(
        "{count} finishing cuts land within {error:.3} mm of where they were wanted; from the drawing's table they would be {:.2} out",
        f(&e["error_as_drawn"])
    );
    assert!(count > 200 && error < 0.09 && (error - f(&e["error"])).abs() < 1e-6);
    assert!(f(&e["error_as_drawn"]) > 0.3);
    // And over three hundred such machines, as calibrate.py's study recorded it. What the touches do not find is
    // whether the two travels are square: with that true as well, the cuts are three times nearer.
    let s = &c["study"];
    assert!(s["trials"].as_u64().unwrap() >= 300);
    assert!(f(&s["as_drawn"][0]) > 1.0);
    assert!(f(&s["fitted"][0]) < 0.06 && f(&s["fitted"][2]) < 0.1);
    assert!(f(&s["and_square"][0]) < 0.02 && f(&s["and_square"][2]) < 0.05);
    assert!(f(&s["and_square"][0]) * 2.0 < f(&s["fitted"][0]));
}

/// A point given as (across, toward the router) with the stick turned to t, back in the tile's frame.
fn seen_back(q: P2, t: f64) -> P2 {
    (
        -q.0 * t.sin() + q.1 * t.cos(),
        q.0 * t.cos() + q.1 * t.sin(),
    )
}

/// The probing table, on the machine as drawn: each of its six rows is
/// the pin touching the master, from the front level with the axis and
/// from below and above with its end past it, at the spindle's mark
/// and half a turn on, each come to by the quill's home. And the bands
/// are turned with the bit's end at the stick from the front and with
/// its side over the stick, in bites of 4 mm.
#[test]
fn the_probing_table_and_the_bands_are_where_the_reading_of_them_assumes() {
    let n = numbers();
    let cal = &n["calibration"];
    let d = &cal["drawn"];
    let drawn = Real {
        tip_home: f(&d["tip_home"]),
        across_home: f(&d["across_home"]),
        scale: (1.0, 1.0),
        shear: 0.0,
        ecc: (0.0, 0.0),
        master_r: f(&d["master_r"]),
        pin_r: f(&cal["pin_d"]) / 2.0,
    };
    let (travel, probe) = (pair(&n["axes"]["travel"]), pair(&n["axes"]["probe"]));
    let mut kinds = Vec::new();
    for row in cal["rows"].as_array().unwrap() {
        let kind = row["kind"].as_str().unwrap();
        if !["in", "up", "dn"].contains(&kind) {
            continue;
        }
        let (depth, height, turn) = (f(&row["depth"]), f(&row["height"]), f(&row["turn"]));
        assert!(
            drawn.pin_gap(depth, height, turn).abs() < 1e-3,
            "row {}",
            row["n"]
        );
        assert_eq!(row["via"].as_str(), Some("h"));
        let (tip, across) = drawn.tip(depth, height);
        if kind == "in" {
            assert!(across.abs() < 1e-3 && depth - probe.0 > 1.0 && depth + probe.0 < travel.0);
        } else {
            // the pin's end past the stick's axis, so its side is what meets the master
            assert!((tip + f(&cal["over"])).abs() < 1e-3);
            assert!((across > 0.0) == (kind == "dn"));
            assert!(height - probe.1 > 1.0 && height + probe.1 < travel.1 - 1.0);
        }
        kinds.push((kind.to_string(), turn as i64));
    }
    kinds.sort();
    let want: Vec<(String, i64)> = [
        ("dn", 0),
        ("dn", 180),
        ("in", 0),
        ("in", 180),
        ("up", 0),
        ("up", 180),
    ]
    .iter()
    .map(|&(k, t)| (k.to_string(), t))
    .collect();
    assert_eq!(kinds, want);
    // the bands
    let axes = Axes::new(&n);
    let bit_r = f(&n["rough"]["bit"]) / 2.0;
    let rows = n["bands"]["rows"].as_array().unwrap();
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    let per = [f(&sim["per"][0]), f(&sim["per"][1])];
    let done = sim["bands"].as_array().unwrap();
    assert_eq!(done.len(), rows.len());
    let (mut front, mut top, mut rounds) = (Vec::new(), Vec::new(), 0);
    for (row, s) in rows.iter().zip(done) {
        match row["kind"].as_str().unwrap() {
            "place" => {
                let (depth, height) = (f(&row["depth"]), f(&row["height"]));
                let (across, tip, _) = axes.cutter(depth, height, f(&row["turn"]));
                if across.abs() < 1e-3 {
                    front.push(tip);
                } else {
                    assert!((tip + bit_r).abs() < 1e-3 && across > 0.0);
                    top.push(across - bit_r);
                }
                assert!((f(&s[1]) - depth * per[0]).abs() <= 0.5 + 1e-6);
                assert!((f(&s[2]) - height * per[1]).abs() <= 0.5 + 1e-6);
                assert!(
                    depth < travel.0 - f(&n["axes"]["bit_margin"]) + 1e-6
                        && height < travel.1 - 2.0
                );
            }
            "round" => rounds += 1,
            _ => {}
        }
    }
    println!("bands from the front at {front:?}, from above at {top:?}; {rounds} turns");
    assert_eq!(rounds, 12);
    for bands in [&front, &top] {
        assert_eq!(bands.len(), 3);
        assert!(bands
            .windows(2)
            .all(|w| w[0] > w[1] && w[0] - w[1] <= 4.0 + 1e-3));
        assert!((bands[2] - f(&n["bands"]["r"])).abs() < 1e-3);
    }
}
