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
    // The master is the spindle's own rod, smaller than anything a pass comes near, so the bit, which is beside it
    // with the sled back, is never in it at a row.
    assert!(
        master < f(&n["tile"]["circle"]) - 1.0 && (master - f(&n["spindle_d"]) / 2.0).abs() < 1e-9
    );
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
/// the lock disc, hung off the plate's far end, never comes to it. Every
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
    // The deck reaches out over the near fence toward that face and stops short of it; the risers are on the deck
    // and the pillow blocks' feet on the risers.
    let (deck_near, riser_half, kp) = (f(&s["deck_y"][1]), f(&s["riser_half"]), &s["kp"]);
    assert!(deck_near + 5.0 <= face && f(&s["plate_y"][1]) < face - 1.0);
    assert!(f(&kp[1]) / 2.0 <= riser_half && riser_half < deck_near);
    // The stick's axis is a riser and a pillow block's centre height over the deck: two layers of 1/2 in and 19.
    assert!((f(&s["riser"]) - 2.0 * f(&s["t12"])).abs() < 1e-9);
    assert!((f(&s["axis_over"]) - f(&s["riser"]) - f(&kp[3])).abs() < 1e-9);
    // The collet nut goes lower than a riser's top and comes nearer the axis than a riser is wide, so along the
    // sled it must never be over one: past the tail's inner block with the sled back, short of the headstock with
    // it forward. It is always well over the deck.
    let nut_r = f(&p["collet"][0]) / 2.0;
    let lowest = rows.iter().map(|r| r.across).fold(f64::MAX, f64::min) - nut_r;
    let (tail_end, head_start) = (f(&s["tail_riser_x"][1]), f(&s["head_riser_x"][0]));
    println!(
        "the collet nut comes {:.1} below the axis, {:.1} under a riser's top; it is {:.1} past the tail's riser with the sled back and {:.1} short of the head's with it forward",
        -lowest,
        -lowest - f(&kp[3]),
        f(&s["back_x"]) - nut_r - tail_end,
        head_start - f(&s["forward_x"]) - nut_r
    );
    assert!(
        f(&s["back_x"]) - nut_r - tail_end >= 3.0 && head_start - f(&s["forward_x"]) - nut_r >= 3.0
    );
    assert!((f(&s["nut_past"]) - (f(&s["back_x"]) - nut_r - tail_end)).abs() < 1e-9);
    assert!(
        lowest + f(&s["axis_over"]) >= 10.0,
        "the nut passes over the deck"
    );
    // the stick's axis is level with the middle of the height's travel
    let travel = pair(&a["travel"]);
    assert!((f(&p["bit_home_z"]) + travel.1 / 2.0 - za).abs() < 1e-9);
    assert!((f(&a["across_home"]) + travel.1 / 2.0).abs() < 1e-9);
    // The lock disc is wider than the stick is from that wall: it is past the positioner's box with the sled
    // forward, which is as near as it comes, and well over the fences' lips. The index wheel never moves: it is
    // clear of the base, and its rim and its belt dip into the slot in its own plate.
    let ix = &n["index"];
    let (rim, disc, disc_x) = (f(&s["wheel_rim"]), f(&ix["disc"][0]), pair(&ix["disc_x"]));
    assert!(disc / 2.0 > face && disc_x.0 - (f(&s["forward_x"]) + f(&p["half_box"])) >= 5.0);
    assert!(za - disc / 2.0 >= f(&s["lip_top"]) + 10.0 && f(&s["lip_top"]) > f(&s["plate_top"]));
    let slot = pair(&ix["slot"][1]);
    assert!(
        za - rim >= 5.0 && za - rim < f(&ix["head_z"][1]),
        "the wheel dips into its plate's slot and clears the base"
    );
    assert!(f(&ix["rim_chord"]) + 3.0 <= slot.1.min(-slot.0) && f(&ix["belt_over"]) >= 2.0);
    // the sled is a box, and its deck is where the uprights stand: the stick's axis the same height over it always
    assert!(
        (za - top - f(&s["axis_over"])).abs() < 1e-9
            && top > f(&s["plate_top"]) + 2.0 * f(&s["ply"])
    );
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
    // a turn of the stick is a whole number of steps
    assert!((360.0 * per[2] - turn_steps).abs() < 1e-9);
    // The clamp: opened once to find home, then for every row at a new angle and for no other. The controller will
    // not turn the wheel with it thrown, and counts its openings; the table says how many there should be.
    let (mut at, mut locked, mut unlocks, mut angles) = (0.0, false, 1u64, 0u64);
    for r in &rows {
        let to = (r.turn * per[2]).round().rem_euclid(turn_steps);
        if to != at {
            unlocks += locked as u64;
            angles += 1;
            at = to;
        }
        locked = true;
    }
    println!(
        "{angles} new angles in {} rows: the clamp opened {unlocks} times",
        rows.len()
    );
    assert_eq!(sim["unlocks"].as_u64().unwrap(), unlocks);
    assert_eq!(n["index"]["unlocks"].as_u64().unwrap(), unlocks);
    assert!(angles > 150 && (angles as usize) < rows.len() / 2);
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
    // the stick: the tile's section
    let stick = bodies_of(&mut doc, "Stick").remove(0);
    // milled as far as the bit reaches, and still the blank from there into the socket
    let (cut_to, size) = (f(&s["cut_to"]), pair(&n["blank"]["size"]));
    let expect = f(&n["tile"]["area"]) * cut_to + size.0 * size.1 * (f(&n["stick"]) - cut_to);
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
    // every part, and what is through what. The bit is drawn as a plain cylinder to its tip, so it is left out.
    let mut all: Vec<(String, Solid)> = Vec::new();
    for (name, list) in n["bodies"].as_object().unwrap() {
        let solids = bodies_of(&mut doc, name);
        assert_eq!(solids.len(), list.as_array().unwrap().len(), "{name}");
        for solid in solids {
            all.push((name.clone(), solid));
        }
    }
    let (mut tried, mut worst) = (0, (0.0f64, String::new()));
    for i in 0..all.len() {
        for k in i + 1..all.len() {
            let (a, b) = (&all[i], &all[k]);
            if a.0 == "Bit" || b.0 == "Bit" {
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
    // The sled at each end of its travel: all that rides on it, moved there, against all that stands on the base.
    // It slides between two fences under their lips. Lifted a millimetre it is in a lip, and pushed a millimetre
    // either way across it is in a fence: that is what holds it.
    let rides = [
        "Sled plate",
        "Sled box",
        "Spring plungers",
        "Tailstock",
        "Tail bearings KP001",
        "Tail spindle",
        "Headstock",
        "Head bearings KP001",
        "Spindle",
        "Blank socket",
        "Star hub",
        "Index star and lock disc",
        "Lock post",
        "Lock anvil",
        "Toggle clamp",
        "Lock switch",
        "Catch strike",
        "Stick",
    ];
    // The index head is on the base, but not fastened to it: the star pushes it back as it comes home.
    let floats = [
        "Index head",
        "Head feet",
        "Index bearings KFL001",
        "Index shaft",
        "Index wheel",
        "Wheel hub",
        "Turn home sensor",
        "Wheel drive",
        "Head bolts",
        "Head springs",
    ];
    let stands = [
        "Base",
        "Fences",
        "Slips",
        "Stops",
        "Sled-back switch",
        "Catch magnet",
        "Spring post",
        "Controller",
        "Router positioner",
    ];
    assert!(rides
        .iter()
        .chain(floats.iter())
        .chain(stands.iter())
        .all(|k| all.iter().any(|a| a.0 == *k)));
    assert!(
        all.iter().all(|a| a.0 == "Bit"
            || rides.contains(&a.0.as_str())
            || floats.contains(&a.0.as_str())
            || stands.contains(&a.0.as_str())),
        "every body rides, floats or stands"
    );
    // the build's own second assembly, the sled home, moves the same parts
    let named = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(named(&n["index"]["rides"]), rides);
    assert_eq!(named(&n["index"]["floats"]), floats);
    assert!(doc.tabs.iter().any(|t| t.name() == "Sled home"));
    let fences: Vec<&Solid> = all
        .iter()
        .filter(|a| a.0 == "Fences")
        .map(|a| &a.1)
        .collect();
    assert_eq!(fences.len(), 2);
    let held = |solid: &Solid, dy: f64, dz: f64| {
        let moved = solid.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
            0.0, dy, dz,
        )));
        fences
            .iter()
            .map(|f| overlap(&moved, f))
            .fold(0.0, f64::max)
    };
    // The sled back, the star home and the head pushed back by it; the sled back and the head not giving way, which
    // cannot be; and the sled forward, the head waiting where its springs leave it.
    let seat = f(&n["index"]["seat"]);
    let along = |solid: &Solid, dx: f64| {
        solid.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
            dx, 0.0, 0.0,
        )))
    };
    let apart = |p: &(ok_math::Vec3, ok_math::Vec3), q: &(ok_math::Vec3, ok_math::Vec3)| {
        p.0.x >= q.1.x
            || q.0.x >= p.1.x
            || p.0.y >= q.1.y
            || q.0.y >= p.1.y
            || p.0.z >= q.1.z
            || q.0.z >= p.1.z
    };
    for (end, at, head, fits) in [
        ("back", f(&s["back_x"]), seat, true),
        (
            "back and the head not giving way",
            f(&s["back_x"]),
            0.0,
            false,
        ),
        ("forward", f(&s["forward_x"]), 0.0, true),
    ] {
        let shift = f(&n["drawn"]["x"]) - at;
        let (mut tried, mut worst) = (0, (0.0f64, String::new()));
        // all that does not ride: what stands, and the head where this case has it
        let rest: Vec<(&str, Solid)> = all
            .iter()
            .filter(|b| b.0 != "Bit" && !rides.contains(&b.0.as_str()))
            .map(|b| {
                let dx = if floats.contains(&b.0.as_str()) {
                    head
                } else {
                    0.0
                };
                (b.0.as_str(), along(&b.1, dx))
            })
            .collect();
        for a in all.iter().filter(|a| rides.contains(&a.0.as_str())) {
            let moved = along(&a.1, shift);
            let p = moved.bounds().unwrap();
            for b in &rest {
                if apart(&p, &b.1.bounds().unwrap()) {
                    continue;
                }
                tried += 1;
                let common = overlap(&moved, &b.1);
                if common > worst.0 {
                    worst = (common, format!("{} and {}", a.0, b.0));
                }
            }
            if a.0 == "Sled plate" && fits {
                let (up, far, near) = (
                    held(&moved, 0.0, 1.0),
                    held(&moved, -1.0, 0.0),
                    held(&moved, 2.0, 0.0),
                );
                println!("sled {end}: lifted 1 mm the plate is {up:.0} mm3 into a lip; pushed across, {far:.0} into the datum fence, {near:.0} into the near one");
                assert!(up > 100.0 && far > 100.0 && near > 100.0);
            }
        }
        // and the head itself, pushed back, against what stands behind it
        for a in rest.iter().filter(|a| floats.contains(&a.0)) {
            for b in rest.iter().filter(|b| stands.contains(&b.0)) {
                if apart(&a.1.bounds().unwrap(), &b.1.bounds().unwrap()) {
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
            "sled {end}: {tried} pairs near each other, the most in common {:.3} mm3 ({})",
            worst.0, worst.1
        );
        if fits {
            assert!(
                tried >= 4 && worst.0 < 1.0,
                "sled {end}: {} share {:.1} mm3",
                worst.1,
                worst.0
            );
        } else {
            assert!(
                worst.0 > 500.0 && worst.1 == "Index star and lock disc and Index wheel",
                "sled {end}: {} share {:.1} mm3",
                worst.1,
                worst.0
            );
        }
    }
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

/// From the motor to the far end of the stick, every part is joined to
/// the next by something drawn. The belt has six teeth of the motor's
/// pulley in it; the index wheel is on a hub on its own shaft, in two
/// flange bearings on the index head; the star on the sled's spindle
/// goes into the wheel's face (the next test); that spindle runs in
/// two pillow blocks bolted down on a riser on the sled's deck; its
/// end is in the boss of a printed socket whose pocket holds the
/// blank's end by its flats; the other end of the stick is on the
/// point of a second rod in two more pillow blocks, bare where the
/// bit is set against it; and a fin on the wheel passes through a
/// sensor once a turn.
#[test]
fn the_stick_s_drive_is_joined_from_the_motor_to_the_tail_spindle() {
    let n = numbers();
    let s = &n["sled"];
    let mut doc = load();
    let (za, rod_d, stick) = (f(&s["za"]), f(&n["spindle_d"]), f(&n["stick"]));
    let x_of = |b: &Solid| {
        let (lo, hi) = b.bounds().unwrap();
        (lo.x, hi.x)
    };
    // the belt: teeth of the small pulley in mesh, and the controller's steps for a turn follow the two pulleys
    let w = &s["wheel"];
    let (big_t, small_t, c) = (f(&w[0]), f(&w[1]), f(&w[3]));
    let (big, small) = (big_t * 2.0 / PI, small_t * 2.0 / PI);
    let mesh = small_t * (PI - 2.0 * ((big - small) / 2.0 / c).asin()) / (2.0 * PI);
    println!(
        "{small_t:.0} teeth on the motor, {big_t:.0} on the index wheel: {mesh:.1} of the pulley's teeth in the belt"
    );
    assert!(mesh >= 6.0 && (mesh - f(&s["mesh"])).abs() < 1e-9);
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    assert!((f(&sim["per"][2]) * 360.0 - 200.0 * 16.0 * big_t / small_t).abs() < 1e-6);
    assert!(big_t >= 300.0 && big_t / small_t == 18.0);
    // On the sled, from the stick outward: socket, two bearings each on the stick's side of its upright, the star's
    // hub, the lock disc, the star. The rod is whole from the bottom of the socket's boss to inside the star.
    let spindle = bodies_of(&mut doc, "Spindle");
    assert_eq!(spindle.len(), 1);
    let (rod, rod_x) = (x_of(&spindle[0]), pair(&s["rod_x"]));
    assert!((rod.0 - rod_x.0).abs() < 1e-6 && (rod.1 - rod_x.1).abs() < 1e-6);
    let long = rod.1 - rod.0;
    assert!(
        (spindle[0].volume() - PI * rod_d * rod_d / 4.0 * long).abs() < 0.01 * spindle[0].volume()
    );
    let ix = &n["index"];
    let (disc_x, star_x) = (pair(&ix["disc_x"]), pair(&ix["star_x"]));
    let star_hub = x_of(&bodies_of(&mut doc, "Star hub")[0]);
    let head_riser = pair(&s["head_riser_x"]);
    assert!(
        (star_hub.1 - disc_x.0).abs() < 1e-6 && star_hub.0 > head_riser.1,
        "the star's hub is on the disc's inner face, its boss clear of the headstock"
    );
    assert!(
        (disc_x.1 - star_x.0).abs() < 1e-9 && rod.1 > star_x.0 && rod.1 < star_x.1,
        "the rod ends inside the star"
    );
    // On the index head, drawn where it waits: the wheel, its hub on its back, the home sensor, an upright with a
    // bearing, a bearing and its upright. Its shaft is whole from inside the wheel to the second upright.
    let park = f(&ix["head_at"]) + f(&n["drawn"]["x"]);
    let wheel = pair(&s["wheel_x"]);
    let wheel = (wheel.0 + park, wheel.1 + park);
    let hub = x_of(&bodies_of(&mut doc, "Wheel hub")[0]);
    assert!(
        (hub.0 - wheel.1).abs() < 1e-6,
        "the hub is on the wheel's back"
    );
    let ups = [pair(&ix["idx_x"][0]), pair(&ix["idx_x"][1])];
    let shaft = bodies_of(&mut doc, "Index shaft");
    let (sh, sh_x) = (x_of(&shaft[0]), pair(&ix["idx_rod_x"]));
    assert!((sh.0 - sh_x.0 - park).abs() < 1e-6 && (sh.1 - sh_x.1 - park).abs() < 1e-6);
    assert!(
        (shaft[0].volume() - PI * rod_d * rod_d / 4.0 * (sh.1 - sh.0)).abs()
            < 0.01 * shaft[0].volume()
    );
    assert!(sh.0 < wheel.1 && sh.0 > wheel.0 && sh.1 > hub.1 && sh.1 > ups[1].0 + park);
    let on_head: Vec<(f64, f64)> = bodies_of(&mut doc, "Index bearings KFL001")
        .iter()
        .map(x_of)
        .collect();
    assert_eq!(on_head.len(), 2);
    assert!(
        (on_head[0].0 - ups[0].1 - park).abs() < 1e-6,
        "on the first upright's back"
    );
    assert!(
        (on_head[1].1 - ups[1].0 - park).abs() < 1e-6,
        "on the second upright's front"
    );
    assert!(on_head[1].0 - on_head[0].1 > 4.0 && hub.1 < ups[0].0 + park);
    // the sled's rod and the wheel's shaft never meet: with the star home there is this much between their ends
    let between = sh_x.0 - rod_x.1;
    assert!(between > 3.0, "{between}");
    // On the sled each spindle runs in two pillow blocks bolted down on a riser: a solid block glued face down on
    // the deck. Each block stands on its riser with its foot inside the riser's edges, the shaft's centre its
    // catalogue height over the foot, and the two of a pair are well apart.
    let kp: Vec<f64> = s["kp"].as_array().unwrap().iter().map(f).collect();
    let deck_top = f(&s["top"]);
    let mut on_blocks = |blocks: &str, riser: &str, want: &Value| -> Vec<(f64, f64)> {
        let riser = bodies_of(&mut doc, riser).remove(0);
        let (r_lo, r_hi) = riser.bounds().unwrap();
        assert!(
            (r_lo.z - deck_top).abs() < 1e-6 && (r_hi.z - r_lo.z - f(&s["riser"])).abs() < 1e-6
        );
        let glue = (r_hi.x - r_lo.x) * (r_hi.y - r_lo.y);
        assert!(
            (riser.volume() - glue * f(&s["riser"])).abs() < 1.0,
            "a solid block"
        );
        let found = bodies_of(&mut doc, blocks);
        assert_eq!(found.len(), 2);
        let mut along = Vec::new();
        for (block, x) in found.iter().zip(want.as_array().unwrap()) {
            let (lo, hi) = block.bounds().unwrap();
            let x = pair(x);
            assert!(
                (lo.x - x.0).abs() < 1e-6
                    && (hi.x - x.1).abs() < 1e-6
                    && (hi.x - lo.x - kp[0]).abs() < 1e-6
            );
            assert!((lo.z - r_hi.z).abs() < 1e-6, "it stands on the riser");
            assert!((za - lo.z - kp[3]).abs() < 1e-6 && (hi.z - lo.z - kp[2]).abs() < 0.05);
            assert!((hi.y - lo.y - kp[1]).abs() < 1e-6 && lo.y >= r_lo.y && hi.y <= r_hi.y);
            assert!(lo.x >= r_lo.x - 1e-6 && hi.x <= r_hi.x + 1e-6 && kp[4] / 2.0 + 8.0 <= r_hi.y);
            assert!(overlap(block, &riser) < 1e-6);
            along.push((lo.x, hi.x));
        }
        // a push of 200 N along the rod, which is more than the jack screw needs, on the riser's glue
        println!(
            "{riser_name}: {glue:.0} mm2 of glue under it, {:.3} MPa for 200 N along the rod",
            200.0 / glue,
            riser_name = blocks
        );
        assert!(glue > 4000.0 && along[1].0 - along[0].1 >= 25.0);
        along
    };
    let bearings = on_blocks("Head bearings KP001", "Headstock", &s["head_pb_x"]);
    let tail_bearings = on_blocks("Tail bearings KP001", "Tailstock", &s["tail_pb_x"]);
    assert!(
        bearings[0].0 > rod.0 && bearings[1].1 < disc_x.0 && bearings[1].1 <= head_riser.1 + 1e-6
    );
    // The socket: its boss on the rod's end, its pocket round the stick's end with room behind it to push the blank
    // in further, and a fit loose enough to go on by hand. Turning, its corners clear the plate and the positioner.
    let socket = &s["socket"];
    let body = bodies_of(&mut doc, "Blank socket").remove(0);
    let (lo, hi) = body.bounds().unwrap();
    let (pocket, boss) = (pair(&socket["x"]), pair(&socket["boss_x"]));
    assert!((lo.x - pocket.0).abs() < 1e-6 && (hi.x - boss.1).abs() < 1e-6);
    assert!(
        (boss.0 - rod.0).abs() < 1e-6 && boss.1 < bearings[0].0,
        "the rod is in the boss"
    );
    assert!(
        boss.1 - boss.0 >= 1.5 * rod_d,
        "the rod is {} into the boss",
        boss.1 - boss.0
    );
    assert!((stick - pocket.0 - f(&socket["in"])).abs() < 1e-9 && f(&socket["in"]) >= 10.0);
    assert!((pocket.1 - stick - f(&socket["slide"])).abs() < 1e-9 && f(&socket["slide"]) >= 5.0);
    let swing = f(&socket["swing"]);
    let reach = [
        (lo.y, lo.z - za),
        (lo.y, hi.z - za),
        (hi.y, lo.z - za),
        (hi.y, hi.z - za),
    ]
    .iter()
    .map(|q| q.0.hypot(q.1))
    .fold(0.0, f64::max);
    assert!(swing <= reach + 1e-6 && swing > f(&n["blank_swing"]) + f(&socket["wall"]) - 1.0);
    println!(
        "the socket swings {swing:.1}: {:.1} clear of the plate, {:.1} of the positioner's face",
        za - f(&s["top"]) - swing,
        f(&s["face"]) - swing
    );
    assert!(za - f(&s["top"]) - swing >= f(&s["bed_clear"]) && f(&s["face"]) - swing >= 5.0);
    // the blank in it: the stick's end, still the blank's rectangle, with the fit all round and nothing through
    let wood = bodies_of(&mut doc, "Stick").remove(0);
    assert!(overlap(&wood, &body) < 1e-3);
    let pushed = wood.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
        0.0,
        2.0 * f(&socket["fit"]) + 0.5,
        0.0,
    )));
    assert!(
        overlap(&pushed, &body) > 1.0,
        "the pocket holds the blank across"
    );
    // The tail spindle, where the bit waits: the same rod in two pillow blocks, its end at the stick's end, bare
    // from its inner block to the stick. That is the master.
    let tail = x_of(&bodies_of(&mut doc, "Tail spindle")[0]);
    assert!(tail.1.abs() < 1e-6 && tail.0 < tail_bearings[0].0);
    let master = pair(&s["master_x"]);
    assert!((master.0 - tail_bearings[1].1).abs() < 1e-6 && master.1.abs() < 1e-6);
    assert!(
        (f(&s["master_r"]) - rod_d / 2.0).abs() < 1e-9,
        "the master is the rod itself"
    );
    assert!(f(&s["master_r"]) + 1.0 <= f(&n["tile"]["circle"]));
    // the home mark: the sensor on the index head's first upright, facing the wheel's back and short of it, its
    // slot at the fin's radius and the fin reaching into it
    let sensor = bodies_of(&mut doc, "Turn home sensor").remove(0);
    let (lo, hi) = sensor.bounds().unwrap();
    assert!((hi.x - ups[0].0 - park).abs() < 1e-6 && lo.x >= wheel.1 + 1.5);
    let fin_end = wheel.1 + f(&s["fin_l"]);
    assert!(
        fin_end > lo.x + 3.0 && fin_end < lo.x + 8.0,
        "the fin is in the slot"
    );
    assert!(
        ((lo.z + hi.z) / 2.0 - za - f(&s["flag_r"])).abs() < 1e-6
            && hi.z <= za + f(&s["stock_above"])
    );
    // The sensor says home over the fin's whole width. The controller's own homing, run by the build from nine
    // places in the fin and out of it, ends on the same edge of it every time, to a step.
    let homing = &s["homing"];
    let ends: Vec<(f64, f64)> = homing["ends"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect();
    let (fin, step) = (f(&homing["fin_deg"]), f(&homing["step_deg"]));
    assert!(
        (fin - (6.0 / f(&s["flag_r"])).to_degrees()).abs() < 1e-9 && f(&homing["clear_deg"]) > 1.0
    );
    assert!(ends.len() >= 8 && ends.iter().filter(|e| e.0 >= 0.0 && e.0 < fin).count() >= 3);
    for e in &ends {
        assert!(
            (e.1 - ends[0].1).abs() <= 1.5 * step && (e.1 - fin).abs() <= 1.5 * step,
            "from {} it ends at {}",
            e.0,
            e.1
        );
    }
    // a fin 0.02 mm out where the sensor reads it turns the stick this far wrong at the tile's edge
    let at_edge = 0.02 / f(&s["flag_r"]) * f(&n["tile"]["swing"]);
    println!(
        "the home fin at {:.0} mm: 0.02 mm there is {at_edge:.3} mm at the tile's edge",
        f(&s["flag_r"])
    );
    assert!(at_edge < 0.02);
}

/// The owner's index. A tapered twelve-point star on the sled's
/// spindle goes into its negative in the face of a wheel that stays
/// at the end of the base: home, the star's flanks are on the
/// pocket's with nothing to spare and nothing through, the head
/// pushed back on its springs by the seat; a hair further in, or
/// turned a degree, it is through the pocket's walls, which is what
/// makes the wheel's angle the stick's. A clamp on the sled pinches
/// the disc behind the star against an anvil, outside the pocket, so
/// the stick keeps its angle when the sled takes it away. The head is
/// not fastened down: it stands on three feet and hangs back on two
/// bolts through slack holes in a post.
#[test]
fn the_star_goes_home_in_the_index_wheel_and_a_clamp_keeps_the_angle_when_it_leaves() {
    let n = numbers();
    let (s, ix) = (&n["sled"], &n["index"]);
    let mut doc = load();
    let za = f(&s["za"]);
    let star = &ix["star"];
    // the star: two hexagons, twelve points, tapered a side's angle at the points over its thickness
    let (r, inside, t, k) = (
        f(&star["r"]),
        f(&star["inside"]),
        f(&star["t"]),
        f(&star["k"]),
    );
    assert!((r - f(&star["af"]) / 3f64.sqrt()).abs() < 1e-9);
    assert!((inside - f(&star["af"]) / 2.0 / 15f64.to_radians().cos()).abs() < 1e-9);
    assert!(((r * (1.0 - k) / t).atan().to_degrees() - f(&star["taper"])).abs() < 1e-9);
    assert_eq!(star["section"].as_array().unwrap().len(), 36);
    // The body: the disc and the star in one, bored clear of the rod. The star's volume is a frustum's of its section.
    let section: Vec<P2> = star["section"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect();
    let area = section
        .iter()
        .zip(section.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        .abs()
        / 2.0;
    let (disc, disc_x, star_x) = (
        (f(&ix["disc"][0]), f(&ix["disc"][1])),
        pair(&ix["disc_x"]),
        pair(&ix["star_x"]),
    );
    let bore = f(&n["spindle_d"]) + 1.0;
    let expect = PI * disc.0 * disc.0 / 4.0 * disc.1 + area * t * (1.0 + k + k * k) / 3.0
        - PI * bore * bore / 4.0 * (disc.1 + t);
    let body = bodies_of(&mut doc, "Index star and lock disc");
    assert_eq!(body.len(), 1);
    let body = &body[0];
    println!(
        "star {:.1} over its points, {:.1} at its small end; with its disc {:.0} mm3 (a frustum and a disc: {expect:.0})",
        2.0 * r,
        2.0 * r * k,
        body.volume()
    );
    assert!((body.volume() - expect).abs() < 0.005 * expect);
    let (lo, hi) = body.bounds().unwrap();
    assert!((lo.x - disc_x.0).abs() < 1e-6 && (hi.x - star_x.1).abs() < 1e-6);
    assert!((hi.z - za - disc.0 / 2.0).abs() < 0.05);
    // Home. The sled back, the head pushed back by the seat: the star and the wheel share nothing.
    let wheel = bodies_of(&mut doc, "Index wheel").remove(0);
    let (seat, back) = (f(&ix["seat"]), f(&n["drawn"]["x"]) - f(&s["back_x"]));
    assert!((f(&ix["head_at"]) + f(&s["back_x"]) + seat).abs() < 1e-9);
    let shift = |solid: &Solid, dx: f64| {
        solid.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
            dx, 0.0, 0.0,
        )))
    };
    let home = shift(&wheel, seat);
    let at_home = overlap(&shift(body, back), &home);
    // a hair further in, and the flanks are through each other all round; so is a star turned a degree in its seat
    let deeper = overlap(&shift(body, back + 0.5), &home);
    let turned = |deg: f64| {
        let about = ok_brep::Transform::rotation(
            ok_math::Vec3::new(0.0, 0.0, za),
            ok_math::Vec3::new(1.0, 0.0, 0.0),
            deg.to_radians(),
        );
        overlap(&shift(&body.transformed(&about), back), &home)
    };
    let (one, thirty) = (turned(1.0), turned(30.0));
    // and short of home by the seat, where the head waits for it, it has a degree of play: the room it finds its way in by
    let waiting = overlap(
        &shift(body, back - seat).transformed(&ok_brep::Transform::rotation(
            ok_math::Vec3::new(0.0, 0.0, za),
            ok_math::Vec3::new(1.0, 0.0, 0.0),
            1f64.to_radians(),
        )),
        &home,
    );
    println!(
        "home the star and the wheel share {at_home:.3} mm3; 0.5 mm deeper {deeper:.0}; turned a degree {one:.0}; turned to the next point {thirty:.3}; \
         {seat} short of home and turned a degree, {waiting:.3}"
    );
    assert!(
        at_home < 1.0 && deeper > 100.0 && one > 100.0,
        "the star is on its seat"
    );
    assert!(thirty < 1.0, "twelve ways in, a mark says which");
    assert!(waiting < 1.0);
    // the pocket ends inside the wheel, with a gap behind the star's small end so the flanks seat first
    let (pocket_x, wheel_x) = (pair(&ix["pocket_x"]), pair(&s["wheel_x"]));
    assert!((pocket_x.0 - star_x.0 - f(&star["stand_off"])).abs() < 1e-9);
    assert!((wheel_x.0 - pocket_x.1).abs() < 1e-9);
    assert!(star_x.1 + f(&star["back_gap"]) <= wheel_x.1 - 3.0);
    assert!(
        f(&ix["pocket_boss"][0]) / 2.0 >= r + 6.0,
        "a wall round the pocket"
    );
    // The lock. The clamp's pad is on the disc's inner face, inside its rim; the anvil is behind the disc, a hair
    // clear of it until the clamp closes, and behind the pad; and the pinch is outside the pocket's boss, so the
    // anvil passes the wheel's face as the star goes home.
    let (lock_r, pad) = (f(&ix["lock_r"]), f(&ix["clamp"][4]));
    let clamp = bodies_of(&mut doc, "Toggle clamp").remove(0);
    let anvil = bodies_of(&mut doc, "Lock anvil").remove(0);
    let (c_lo, c_hi) = clamp.bounds().unwrap();
    let (a_lo, a_hi) = anvil.bounds().unwrap();
    assert!((c_hi.x - disc_x.0).abs() < 1e-6, "the pad is on the disc");
    assert!(lock_r + pad / 2.0 <= disc.0 / 2.0 + 1e-9);
    assert!(((c_lo.z + c_hi.z) / 2.0 - za).abs() < 1e-6);
    let gap = f(&ix["anvil"][2]);
    assert!(overlap(&anvil, body) < 1e-6 && overlap(&clamp, body) < 1e-6);
    assert!(
        overlap(&shift(&anvil, -gap - 0.2), body) > 10.0,
        "the anvil is behind the disc"
    );
    assert!(a_hi.y >= -(lock_r - pad / 2.0) && a_lo.y <= -(lock_r + pad / 2.0));
    assert!(
        -a_hi.y >= f(&ix["pocket_boss"][0]) / 2.0 + 3.0,
        "the anvil is outside the pocket's boss"
    );
    assert!(
        (a_hi.x - disc_x.1 - gap - f(&ix["anvil"][0])).abs() < 1e-6 && a_hi.x + 3.0 <= wheel_x.0
    );
    // what the clamp is asked to hold: a cut's push at the blank's corner, against friction at the pinch
    let holds = 2.0 * 0.2 * 500.0 * lock_r / 1000.0;
    let asked = 60.0 * f(&n["blank_swing"]) / 1000.0;
    println!("a 500 N clamp at {lock_r:.0} mm holds {holds:.1} N m; 60 N at the blank's corner asks {asked:.1}");
    assert!(holds > 5.0 * asked);
    // The head. Three feet under its plate, on the base. Home, it stands on the star and the two that are not by
    // the wheel, and the star's seat is inside them; what rolls it is the star's lift over the stance.
    let feet = bodies_of(&mut doc, "Head feet");
    let head = bodies_of(&mut doc, "Index head");
    assert_eq!((feet.len(), head.len()), (3, 3));
    let plate = head[0].bounds().unwrap();
    let head_z = pair(&ix["head_z"]);
    assert!((plate.0.z - head_z.0).abs() < 1e-6 && (plate.1.z - head_z.1).abs() < 1e-6);
    for foot in &feet {
        let (lo, hi) = foot.bounds().unwrap();
        assert!(lo.z.abs() < 1e-6 && (hi.z - head_z.0).abs() < 1e-6);
        assert!(lo.x > plate.0.x && hi.x < plate.1.x && lo.y > plate.0.y && hi.y < plate.1.y);
    }
    let at: Vec<P2> = ix["feet"].as_array().unwrap().iter().map(pair).collect();
    let seat_x = (star_x.0 + star_x.1) / 2.0;
    let (b, c) = (at[1], at[2]);
    let stance =
        ((c.0 - b.0) * (0.0 - b.1) - (c.1 - b.1) * (seat_x - b.0)).abs() / (c.0 - b.0).abs();
    println!(
        "the head's stance {stance:.0} mm: a star 0.03 off its centre rolls it {:.4} deg",
        (0.03 / stance).to_degrees()
    );
    assert!((stance - f(&ix["stance"])).abs() < 1e-6 && stance > 200.0);
    assert!(at[0].0 < seat_x + 5.0 && at[1].0 > seat_x + 80.0 && at[2].1 > 150.0);
    // It hangs back on two bolts through slack holes in the post, level with the axis: their heads are the stop
    // the springs push it up to, and behind the head there is room to be pushed back the seat and more.
    let post = bodies_of(&mut doc, "Spring post").remove(0);
    let bolts = bodies_of(&mut doc, "Head bolts");
    let springs = bodies_of(&mut doc, "Head springs");
    assert_eq!((bolts.len(), springs.len()), (2, 2));
    let post_x = pair(&ix["post_x"]);
    let drawn = f(&n["drawn"]["x"]);
    let (bolt_d, slack, each_side) = (f(&ix["bolt"][0]), f(&ix["bolt"][1]), f(&ix["bolt"][2]));
    assert!(
        slack - bolt_d >= 3.0,
        "the head can shift a millimetre and a half any way"
    );
    for (bolt, spring) in bolts.iter().zip(&springs) {
        let (lo, hi) = bolt.bounds().unwrap();
        assert!(
            ((lo.z + hi.z) / 2.0 - za).abs() < 1e-6
                && ((lo.y + hi.y) / 2.0).abs() - each_side < 1e-6
        );
        assert!(
            (hi.x - post_x.1 - drawn - f(&ix["bolt"][4])).abs() < 1e-6,
            "its head is on the post's back"
        );
        assert!(overlap(bolt, &post) < 1e-6 && overlap(&shift(bolt, seat + 2.0), &post) < 1e-6);
        // sideways by more than the slack it is in the post
        let aside = bolt.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
            0.0,
            (slack - bolt_d) / 2.0 + 1.0,
            0.0,
        )));
        assert!(overlap(&aside, &post) > 5.0);
        let (s_lo, s_hi) = spring.bounds().unwrap();
        assert!(s_hi.x - s_lo.x <= f(&ix["spring_l"]) - seat - 1.0);
        assert!(overlap(&shift(spring, seat + 1.0), &post) < 1e-6);
    }
    // The catch: a magnet flush in the back stop's face and steel flush in the sled's end. With the sled back they meet.
    let magnet = bodies_of(&mut doc, "Catch magnet").remove(0);
    let strike = bodies_of(&mut doc, "Catch strike").remove(0);
    let (m, k) = (magnet.bounds().unwrap(), strike.bounds().unwrap());
    assert!(
        (m.0.x - (k.1.x + back)).abs() < 1e-6,
        "they meet with the sled back"
    );
    assert!(
        ((m.0.y + m.1.y) - (k.0.y + k.1.y)).abs() < 1e-6
            && ((m.0.z + m.1.z) - (k.0.z + k.1.z)).abs() < 1e-6
    );
    assert!(
        (m.0.x - drawn - f(&s["stops_x"][1])).abs() < 1e-6
            && (k.1.x - f(&s["plate_x"][1])).abs() < 1e-6
    );
}

/// The animation, `out/passes.gif`, read back frame by frame: a frame
/// for every cutting row of the table and no fewer; the first is the
/// blank and nothing else; the cutter is in every pass's frame; what is
/// left of the blank never grows; the tile's middle is never cut; and
/// the last frame is the tile, to the area the planner says is left.
#[test]
fn the_animation_has_a_frame_for_every_pass_and_ends_on_the_tile() {
    let n = numbers();
    let a: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/passes_anim.json")).unwrap())
            .unwrap();
    let cuts = rows(&n)
        .iter()
        .filter(|r| r.kind == "rough" || r.kind == "finish")
        .count();
    let whole = |k: &str| a[k].as_u64().unwrap() as usize;
    assert_eq!(whole("cuts"), cuts, "a frame for every cutting row");
    let (first, tail) = (whole("first_cut"), whole("tail"));
    assert!(whole("frames") > first + cuts && whole("frames") <= first + cuts + tail + 1);
    let colour = |k: &str| {
        let v = u32::from_str_radix(&a[k].as_str().unwrap()[1..], 16).unwrap();
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    let (tan, blue) = (colour("blank"), colour("cut"));
    assert_eq!(colour("background"), [255, 255, 255]);
    let near = |c: ok_render::Rgb, want: [u8; 3]| {
        [c.0, c.1, c.2]
            .iter()
            .zip(want)
            .all(|(p, q)| (*p as i32 - q as i32).abs() <= 8)
    };
    let size = whole("size");
    let per_mm = size as f64 / (f(&a["view"][2]) - f(&a["view"][0]));
    // What a frame shows of each: the blank's colour as area, and how many pixels are the cutter's. The first
    // frame and the last are read pixel by pixel; the passes between, every third pixel each way, which is
    // enough to see a cutter and to see what is left shrink.
    let mut seen: Vec<(f64, usize, bool, u32)> = Vec::new();
    let bytes = std::fs::read(dir().join("out/passes.gif")).unwrap();
    let ends = [0, whole("frames") - 1];
    let (w, h, frames) = ok_render::flipbook::read_gif(&bytes, |k, image, ms| {
        let step = if ends.contains(&k) { 1 } else { 3 };
        let (mut left, mut cutter) = (0.0, 0usize);
        for y in (0..size).step_by(step) {
            for x in (0..size).step_by(step) {
                let c = image.pixel(x, y);
                if near(c, blue) {
                    cutter += 1;
                } else if c.2 < 255 && c.0 >= tan[0] {
                    // between white and the blank's colour: an edge pixel is part blank
                    left += (255.0 - c.2 as f64) / (255.0 - tan[2] as f64);
                }
            }
        }
        let middle = image.pixel(size / 2, size / 2);
        let each = (step * step) as f64 / per_mm / per_mm;
        seen.push((left * each, cutter * step * step, near(middle, tan), ms));
    })
    .unwrap();
    assert_eq!((w, h, frames), (size, size, whole("frames")));
    // the first frame: the blank, whole, and no cutter
    let blank = pair(&n["blank"]["size"]);
    println!(
        "{frames} frames, {} bytes; the blank {:.0} mm2 in the first ({:.0} drawn), {:.0} left in the last (the tile {:.0} and {:.1} standing)",
        bytes.len(),
        blank.0 * blank.1,
        seen[0].0,
        seen[frames - 1].0,
        f(&n["tile"]["area"]),
        f(&n["rough"]["left_at_end"])
    );
    assert!((seen[0].0 - blank.0 * blank.1).abs() < 0.005 * blank.0 * blank.1 && seen[0].1 == 0);
    // every pass: the cutter is there, and the tile's middle is still the blank's colour
    for (k, s) in seen.iter().enumerate().skip(first).take(cuts) {
        assert!(s.1 > 100, "no cutter in frame {k}");
        assert!(s.2, "the tile's middle is cut in frame {k}");
    }
    // What is left only ever shrinks: frame by frame, give or take the outlines drawn over its edges and the
    // pixels not looked at, which together are under a hundredth of the blank.
    let slack = 0.01 * blank.0 * blank.1;
    let mut least = seen[0].0;
    for (k, s) in seen.iter().enumerate().skip(first) {
        assert!(
            s.0 <= least + slack,
            "more is left in frame {k} than before it: {} after {least}",
            s.0
        );
        least = least.min(s.0);
    }
    // and by the end of roughing most of the waste is gone
    let roughed = seen[first + whole("last_rough")].0;
    assert!(
        roughed < seen[0].0 - 0.75 * (seen[0].0 - seen[frames - 1].0),
        "{roughed}"
    );
    // the last frame: no cutter, no outlines, and the tile
    let last = seen[frames - 1];
    let want = f(&n["tile"]["area"]) + f(&n["rough"]["left_at_end"]);
    assert!(
        last.1 == 0 && last.2 && (last.0 - want).abs() < 0.015 * want,
        "{} for {want}",
        last.0
    );
    // a pass is on the screen long enough to see, and the whole of it is under a minute
    let ms = f(&a["ms"]);
    assert!(seen[first..first + cuts].iter().all(|s| s.3 as f64 >= ms) && ms >= 40.0);
    let total: f64 = seen.iter().map(|s| s.3 as f64).sum::<f64>() / 1000.0;
    assert!(
        (total - f(&a["seconds"])).abs() < 0.5 && total < 60.0,
        "{total} s"
    );
    // the file is small enough to keep: every frame is only what changed
    assert!(bytes.len() < 4_000_000, "{} bytes", bytes.len());
}

/// The sled is a flat plate on the base, between two fences. Its far
/// edge runs on the datum fence and spring plungers in its near edge
/// keep it there; it rides on two slips; each fence has a lip over it,
/// a hair clear of its top. The fences reach past both ends of its
/// travel. A stop on the base meets the plate at each end: against the
/// back one the bit is beside the master, clear of the tailstock and
/// of the stick's end, against the forward one its collet nut is
/// short of the socket.
#[test]
fn the_sled_slides_between_two_fences_and_its_stops_put_the_bit_beside_the_master() {
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
    // forward, the collet nut stops short of the socket, so the bit does not reach the stick's last 21 mm
    let socket = &s["socket"];
    assert!((forward - f(&s["forward_x"])).abs() < 1e-9);
    let nut_r = f(&n["positioner"]["collet"][0]) / 2.0;
    assert!((f(&socket["x"][0]) - (forward + nut_r) - f(&socket["nut_clear"])).abs() < 1e-9);
    assert!(
        (f(&s["cut_to"]) - (forward + bit_r)).abs() < 1e-9
            && f(&n["stick"]) - f(&s["cut_to"]) < 25.0
    );
    assert!((forward - back - f(&s["travel"])).abs() < 1e-9);
    // the fences: the plate's far edge on the datum, a spring's gap at the near one, a lip over each edge
    let mut doc = load();
    let plate_body = bodies_of(&mut doc, "Sled plate").remove(0);
    let (lo, hi) = plate_body.bounds().unwrap();
    let (top, ply, slip) = (f(&s["plate_top"]), f(&s["ply"]), f(&s["slip"]));
    assert!(
        (hi.z - top).abs() < 1e-6 && (lo.z - slip).abs() < 1e-6 && (top - slip - ply).abs() < 1e-9
    );
    let fences = bodies_of(&mut doc, "Fences");
    assert_eq!(fences.len(), 2);
    let (datum, near) = (fences[0].bounds().unwrap(), fences[1].bounds().unwrap());
    let lip = &s["lip"];
    let (gap, clear) = (f(&s["spring_gap"]), f(&lip[3]));
    println!(
        "the plate {:.0} wide: on the datum fence, {gap} from the near one, a lip {:.0} and {:.0} over its edges and {clear} clear of its top",
        hi.y - lo.y,
        f(&lip[1]),
        f(&lip[2])
    );
    // the datum fence's body stops at the plate's edge and its lip reaches over it; the near one the same, a gap off
    assert!((datum.1.y - (lo.y + f(&lip[1]))).abs() < 1e-6 && f(&lip[1]) >= 5.0);
    assert!((near.0.y - (hi.y - f(&lip[2]))).abs() < 1e-6 && f(&lip[2]) >= 5.0);
    assert!((0.5..=1.5).contains(&gap) && clear > 0.0 && clear <= 0.5);
    for fence in [&datum, &near] {
        assert!((fence.1.z - f(&s["lip_top"])).abs() < 1e-6 && fence.0.z.abs() < 1e-6);
        // and reaches past the plate at both ends of its travel
        assert!(fence.0.x - drawn <= plate.0 - forward && fence.1.x - drawn >= plate.1 - back);
    }
    // the near fence stands against the positioner's face, and the near rail is inside its lip
    assert!((near.1.y - f(&s["face"])).abs() < 1e-6);
    assert!(f(&s["rail_near"]) + 2.0 <= near.0.y);
    // The box on the sled's bottom: two rails on edge, inside both lips, and a deck whose top is where the risers
    // stand. The deck is far over the fences, and reaches out over the near one to short of the positioner's face.
    let boxed = bodies_of(&mut doc, "Sled box");
    assert_eq!(boxed.len(), 3);
    let deck = boxed[2].bounds().unwrap();
    assert!((deck.1.z - f(&s["top"])).abs() < 1e-6 && (deck.1.z - deck.0.z - ply).abs() < 1e-6);
    assert!(deck.0.y >= datum.1.y + 1.5 && deck.0.z > f(&s["lip_top"]) + 20.0);
    assert!(
        (f(&s["face"]) - deck.1.y - f(&s["deck_clear"])).abs() < 1e-6 && f(&s["deck_clear"]) >= 8.0
    );
    assert!(
        deck.1.y - f(&s["rail_near"]) <= 2.0 * ply,
        "the deck overhangs its rail by less than two thicknesses"
    );
    for rail in &boxed[..2] {
        let r = rail.bounds().unwrap();
        assert!(
            (r.0.z - top).abs() < 1e-6
                && (r.1.z - deck.0.z).abs() < 1e-6
                && (r.1.y - r.0.y - ply).abs() < 1e-6
        );
        assert!(r.0.y >= deck.0.y - 1e-6 && r.1.y <= deck.1.y + 1e-6 && r.1.z - r.0.z >= 20.0);
        assert!(
            r.0.y >= datum.1.y + 1.5 && r.1.y <= near.0.y - 1.5,
            "inside both lips"
        );
    }
    // the plungers' noses fill the gap, and both stay on the fence over the whole travel
    let plungers = bodies_of(&mut doc, "Spring plungers");
    assert_eq!(plungers.len(), 2);
    for nose in &plungers {
        let (a, b) = nose.bounds().unwrap();
        assert!((a.y - hi.y).abs() < 1e-6 && (b.y - a.y - gap).abs() < 1e-6);
        assert!(a.z > lo.z && b.z < hi.z && a.x > lo.x + 20.0 && b.x < hi.x - 20.0);
    }
    // two slips under the plate, as long as the travel between the stops
    let slips = bodies_of(&mut doc, "Slips");
    assert_eq!(slips.len(), 2);
    for strip in &slips {
        let (a, b) = strip.bounds().unwrap();
        assert!(a.z.abs() < 1e-6 && (b.z - slip).abs() < 1e-6 && a.y >= lo.y && b.y <= hi.y);
        assert!((a.x - drawn - stops.0).abs() < 1e-6 && (b.x - drawn - stops.1).abs() < 1e-6);
    }
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
    // What no touch on a round master can find is how far the spindle has really turned. A wheel off its turning
    // centre lets the belt in and out once a turn: that much over the wheel's pitch radius, as an angle, and at the
    // furthest finishing cut from the axis that angle is this far. The study adds it to the same machines.
    let turning = &s["turning"];
    let wheel = &n["sled"]["wheel"];
    assert!((f(&turning["wheel_r"]) - f(&wheel[0]) / PI).abs() < 1e-9);
    assert!((f(&turning["ratio"]) - f(&wheel[0]) / f(&wheel[1])).abs() < 1e-9);
    let far = rows(&n)
        .iter()
        .filter(|r| r.kind == "finish" && r.last_bite)
        .map(|r| r.across.hypot(r.tip + ball))
        .fold(0.0, f64::max);
    let per = 0.01 / f(&turning["wheel_r"]) * far;
    println!(
        "0.01 mm off the wheel's centre is {per:.4} mm at the furthest cut; trued to {} the cuts are within {} (median), left {} off, {}",
        turning["wheel_off"], turning["trued"][0], turning["wheel_loose"], turning["loose"][0]
    );
    assert!((per - f(&turning["per_hundredth"])).abs() < 1e-9 && per < 0.01);
    assert!(
        per < 0.004,
        "a wheel this large makes little of its own runout"
    );
    // The index adds two things. The star comes home in the wheel for every new angle, and not quite where it did
    // the time before: that much at its flanks' radius is this at the furthest cut. And a star or a pocket off its
    // centre lifts the head's near end as they turn, which rolls the head by the lift over its feet's stance.
    let ix = &n["index"];
    let (seat, flank_r, stance) = (
        f(&turning["star_seat"]),
        f(&turning["flank_r"]),
        f(&turning["stance"]),
    );
    assert!(
        (flank_r - f(&ix["star"]["inside"])).abs() < 1e-9
            && (stance - f(&ix["stance"])).abs() < 1e-9
    );
    let (homing, rolling) = (
        seat / flank_r * far,
        2.0 * f(&turning["star_off"]) / stance * far,
    );
    println!("the star home within {seat} is {homing:.4} mm at the furthest cut; star and pocket each {} off, {rolling:.4}", turning["star_off"]);
    assert!(homing < 0.008 && rolling < 0.006);
    // all of it together, trued, costs less than a hundredth; and the wheel left as printed no longer costs much more
    assert!(f(&turning["trued"][0]) >= f(&s["fitted"][0]));
    assert!(f(&turning["trued"][0]) < f(&s["fitted"][0]) + 0.01 && f(&turning["trued"][2]) < 0.1);
    assert!(f(&turning["loose"][0]) > f(&turning["trued"][0]));
    assert!(
        f(&turning["loose"][0]) < f(&turning["trued"][0]) + 0.01 && f(&turning["loose"][2]) < 0.11
    );
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
    // Away from its back stop the sled's star is out of the wheel, so those turns are made by hand, by the lock
    // disc: the controller waits for each and moves no motor. It ends the table with the wheel where it began.
    assert_eq!(sim["band_rounds"].as_u64().unwrap(), 12);
    assert!(done.iter().all(|s| f(&s[3]) == f(&done[0][3])));
    for bands in [&front, &top] {
        assert_eq!(bands.len(), 3);
        assert!(bands
            .windows(2)
            .all(|w| w[0] > w[1] && w[0] - w[1] <= 4.0 + 1e-3));
        assert!((bands[2] - f(&n["bands"]["r"])).abs() < 1e-3);
    }
}
