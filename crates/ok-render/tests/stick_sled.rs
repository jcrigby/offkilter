//! The stick sled (examples/stick-sled) as a regression suite. The
//! stick only turns; the router positioner beside it sets how deep
//! the bit reaches and how high; a screw feeds the sled past the bit.
//! The table the controller reads is four numbers a row: the
//! positioner's two screws, the spindle, and how far the sled is fed.
//! This works them forward and checks the bit is where the plan wants
//! it, the whole of it never in the tile; that the passes reach the
//! whole outline; that run in the table's order no cut meets more
//! than its bite; that the blank turns clear of the positioner's face
//! and the sled's plate and the rows fit the positioner's travel;
//! that every move marked straight passes the master clear; that the
//! controller's own program, simulated, ends every row on the table's
//! steps, makes a pass for every cutting row and sets the bit again
//! when a stick is taken up part way; that the model is the sled at
//! one row with nothing through anything, wherever along its travel
//! the sled is; that the wheel, on the sled's own spindle, passes over
//! everything it goes by; that the belt alone holds the stick at
//! every cut, by an estimate made again here; that the screw's nut is
//! on the screw the whole travel and a sled that loses steps is found
//! out when it comes home; that a lock is drawn in its place and left
//! off; and, for setting a built machine up, that six touches of a
//! pin on the master find where the bit's height is level with the
//! stick's axis well enough that the table planned for it cuts true
//! on a machine built wrong, and that the probing table and the bands
//! are where the reading of them assumes.

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
/// the disc and the wheel, beyond the far end of the sled's box, never
/// come to it; the wheel's rim passes over all it goes by. Every
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
    // The stick's axis is a little above the middle of the height's travel: the wheel goes with the sled, and
    // that lifts its rim over what it passes. The rows still have a probe's window and more at each end.
    let travel = pair(&a["travel"]);
    let up = f(&s["axis_up"]);
    assert!(up > 0.0 && up <= 10.0);
    assert!((f(&p["bit_home_z"]) + travel.1 / 2.0 + up - za).abs() < 1e-9);
    assert!((f(&a["across_home"]) + travel.1 / 2.0 + up).abs() < 1e-9);
    // The two bits. Each is set as near what the table takes it to as its own length and the collet's depth allow:
    // its shank in the collet by enough, off the collet's bottom, and within the margin of the table's length.
    // And the collet nut. It is wider than the bit, so at the far end of a pass it is beside the end of the stick
    // that no bit reaches, which is the blank as it was sawn: at every cut the nut has to pass the uncut blank,
    // even with the bit set as short as it may be. Found here from the rows and the blank's corners.
    let (out, shank, nut_r, m) = (
        f(&a["bit_out"]),
        &a["shank"],
        f(&p["collet"][0]) / 2.0,
        f(&a["bit_margin"]),
    );
    let (deep, grip, off) = (f(&shank[0]), f(&shank[1]), f(&shank[2]));
    assert!(grip >= 19.0 && off >= 1.0 && deep > grip + off);
    for (tool, long) in [("straight", 73.025), ("ball", 63.5)] {
        let b = &a["bits"][tool];
        let (set, within, range) = (f(&b["out"]), f(&b["in"]), pair(&b["range"]));
        assert!((f(&b["long"]) - long).abs() < 1e-9 && (long - set - within).abs() < 1e-9);
        assert!(
            within >= grip - 1e-9 && within <= deep - off + 1e-9,
            "{tool}: {within:.1} in the collet"
        );
        assert!(range.0 <= set && set <= range.1 && (set - out).abs() <= m + 1e-9);
        assert!(range.0 >= out - m - 1e-9 && range.1 <= out + m + 1e-9);
        let (air, mut need) = (f(&b["nut_air"]), f64::MIN);
        for r in rows
            .iter()
            .filter(|r| r.tool == tool && (r.kind == "rough" || r.kind == "finish"))
        {
            let (d, e) = ((r.t.cos(), r.t.sin()), (-r.t.sin(), r.t.cos()));
            for i in 0..stock.len() {
                let (q0, q1) = (stock[i], stock[(i + 1) % stock.len()]);
                for k in 0..=2000 {
                    let s = k as f64 / 2000.0;
                    let q = (q0.0 + s * (q1.0 - q0.0), q0.1 + s * (q1.1 - q0.1));
                    if (q.0 * e.0 + q.1 * e.1 - r.across).abs() <= nut_r + air {
                        need = need.max(q.0 * d.0 + q.1 * d.1 - r.tip + air);
                    }
                }
            }
        }
        println!(
            "{}: set {set:.1} out of the nut, {within:.1} in the collet; the nut passes the uncut blank with {need:.1} out, and {:.1} is the least it may be set",
            b["name"].as_str().unwrap(),
            range.0
        );
        assert!(air >= 1.0 && (need - f(&b["nut_need"])).abs() < 0.05);
        assert!(range.0 >= need - 0.05, "{tool}: the nut meets the blank");
    }
    // and the ball's tip is never further past the axis than the planner was told to keep it
    let lim = f(&a["tip_lim"]);
    assert!(
        lim < 0.0
            && rows
                .iter()
                .filter(|r| r.kind == "finish")
                .all(|r| r.tip >= lim - 1e-6)
    );
    // The disc and the wheel are on the sled's spindle and go where it goes. Both are wider than the stick is from
    // that wall: with the sled forward, which is as near as they come, they are past the positioner's box. The
    // wheel is wider than the sled's box is high and hangs beyond its end, so its rim passes over the sled's own
    // bottom, the near fence's lip and the controller's box: each worked out here from the circle.
    let t = &n["turn"];
    let (rim, disc, disc_x, wheel_x) = (
        f(&t["wheel_rim"]),
        f(&t["disc"][0]),
        pair(&t["disc_x"]),
        pair(&t["wheel_x"]),
    );
    assert!(disc / 2.0 > face && rim > disc / 2.0 && wheel_x.0 > disc_x.1);
    let beyond = disc_x.0 - (f(&s["forward_x"]) + f(&p["half_box"]));
    assert!(beyond >= 5.0 && (beyond - f(&t["over"]["positioner"])).abs() < 1e-9);
    assert!(
        wheel_x.0 > f(&s["box_x"][1]) && wheel_x.1 < f(&s["plate_x"][1]),
        "the wheel is beyond the box's end and over the bottom"
    );
    let under = |y: f64| za - (rim * rim - y * y).sqrt(); // the rim's underside, y from the axis
    let lip_in = f(&s["plate_y"][1]) - f(&s["lip"][2]); // the near lip's inner edge: where the rim is lowest over it
    let ctl = &s["controller"];
    for (what, gap, said) in [
        (
            "the sled's bottom",
            under(0.0) - f(&s["plate_top"]),
            f(&t["over"]["bottom"]),
        ),
        ("the base", under(0.0), f(&t["over"]["base"])),
        (
            "the near fence's lip",
            under(lip_in) - f(&s["lip_top"]),
            f(&t["over"]["lip"]),
        ),
        (
            "the controller's box",
            under(f(&ctl["y"][0])) - f(&ctl["z"]),
            f(&t["over"]["controller"]),
        ),
    ] {
        println!("the wheel's rim passes {gap:.1} over {what}");
        assert!(gap >= 5.0 && (gap - said).abs() < 1e-9, "{what}");
    }
    // level with the middle of the height's travel it would have passed that lip by under two millimetres
    assert!(under(lip_in) - up - f(&s["lip_top"]) < 2.0);
    // the controller's box is where the wheel goes by: beside the positioner, under the rim's reach
    assert!(
        f(&ctl["y"][0]) > face
            && f(&ctl["y"][0]) < rim
            && f(&ctl["x"][0]) > f(&p["half_box"])
            && f(&ctl["x"][0]) < wheel_x.1 - f(&s["back_x"])
    );
    assert!(za - disc / 2.0 >= f(&s["lip_top"]) + 10.0 && f(&s["lip_top"]) > f(&s["plate_top"]));
    // the sled is a box, and its deck is where the risers stand: the stick's axis the same height over it always
    assert!(
        (za - top - f(&s["axis_over"])).abs() < 1e-9
            && top > f(&s["plate_top"]) + 2.0 * f(&s["ply"])
    );
    let w = &t["wheel"];
    let (big, small, c) = (f(&w[0]) * 2.0 / PI, f(&w[1]) * 2.0 / PI, f(&w[3]));
    let belt = 2.0 * c + PI * (big + small) / 2.0 + (big - small).powi(2) / (4.0 * c);
    assert!((belt - f(&w[2])).abs() < 0.01, "the belt is {belt}");
    assert!(
        c > rim + small / 2.0 + 15.0,
        "the motor's pulley clears the wheel"
    );
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
/// it ends every row on the table's steps, the sled drawn back; it
/// makes a pass for every cutting row; a setting of the bit ends a
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
    let per = [
        f(&sim["per"][0]),
        f(&sim["per"][1]),
        f(&sim["per"][2]),
        f(&sim["per"][3]),
    ];
    let turn_steps = (360.0 * per[2]).round();
    // a turn of the stick is a whole number of steps
    assert!((360.0 * per[2] - turn_steps).abs() < 1e-6);
    // No lock is fitted: the belt holds the stick, and the controller lets nothing go. Run as if one were fitted,
    // which the build also does, it lets it go once to find home, then for every row at a new angle and for no
    // other, and ends every row on the same steps. The table says how many times that should be.
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
        "{angles} new angles in {} rows: a lock, were one fitted, let go {unlocks} times",
        rows.len()
    );
    assert_eq!(n["lock"]["fitted"].as_bool(), Some(false));
    assert_eq!(sim["lock"].as_bool(), Some(false));
    assert_eq!(sim["unlocks"].as_u64().unwrap(), 0);
    assert_eq!(sim["with_lock"]["unlocks"].as_u64().unwrap(), unlocks);
    assert_eq!(n["lock"]["unlocks"].as_u64().unwrap(), unlocks);
    assert_eq!(n["turn"]["angles"].as_u64().unwrap(), angles);
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
            assert!(f(&s[4]) == 0.0, "row {} ends with the sled drawn back", r.n);
        }
    };
    check(sim["rows"].as_array().unwrap(), &rows);
    // Every cutting row is a pass: the sled fed the length of its travel past the bit and brought back, by its own
    // motor. The table gives that length in every such row and nothing in any other. By the pauses the controller
    // asks for between steps, a stick takes this long; no less than the feeding alone, and not much more.
    let travel = f(&n["sled"]["travel"]);
    let mut cutting = 0.0;
    for (r, told) in rows.iter().zip(n["rows"].as_array().unwrap()) {
        let pass = r.kind == "rough" || r.kind == "finish";
        cutting += pass as u8 as f64;
        assert!((f(&told["along"]) - if pass { travel } else { 0.0 }).abs() < 1e-9);
    }
    assert!(f(&sim["passes"]) == cutting && cutting > 500.0);
    assert!((f(&sim["fed"]) - cutting * (travel * per[3]).round()).abs() < 1e-9);
    assert!((f(&sim["stepped"][3]) - 2.0 * f(&sim["fed"])).abs() < 1e-9);
    let h = &n["hold"];
    let least = cutting * (travel / f(&h["feed"]) + travel / f(&h["return"]));
    let seconds = f(&sim["seconds"]);
    println!(
        "{cutting:.0} passes of {travel:.0} mm: {:.2} h by the controller's own pauses, {:.2} h of it feeding and coming back",
        seconds / 3600.0,
        least / 3600.0
    );
    assert!(seconds > least && seconds < 1.6 * least && (seconds - f(&h["seconds"])).abs() < 1e-6);
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
/// another, there or anywhere along the sled's travel.
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
    // The sled along its travel: all that rides on it, moved there, against all that stands on the base. It
    // slides between two fences under their lips. Lifted a millimetre it is in a lip, and pushed a millimetre
    // either way across it is in a fence: that is what holds it. A lock's four parts are drawn and not fitted;
    // they are among what rides, so their place is kept clear wherever the sled is.
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
        "Wheel hub",
        "Disc",
        "Index wheel",
        "Turn home sensor",
        "Wheel drive",
        "Travel nut",
        "Stick",
        "Lock post",
        "Lock anvil",
        "Toggle clamp",
        "Lock switch",
    ];
    let stands = [
        "Base",
        "Fences",
        "Slips",
        "Travel drive",
        "Stops",
        "Travel home switch",
        "Controller",
        "Router positioner",
    ];
    assert!(rides
        .iter()
        .chain(stands.iter())
        .all(|k| all.iter().any(|a| a.0 == *k)));
    assert!(
        all.iter().all(|a| a.0 == "Bit"
            || rides.contains(&a.0.as_str())
            || stands.contains(&a.0.as_str())),
        "every body rides or stands"
    );
    // the build's own second assembly, the sled drawn back, moves the same parts
    let named = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(named(&s["rides"]), rides);
    assert!(doc.tabs.iter().any(|t| t.name() == "Sled home"));
    assert!((f(&s["seated"]) - (f(&n["drawn"]["x"]) - f(&s["back_x"]))).abs() < 1e-9);
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
    let rest: Vec<&(String, Solid)> = all
        .iter()
        .filter(|b| b.0 != "Bit" && !rides.contains(&b.0.as_str()))
        .collect();
    // Drawn back, forward, and three places between; and 60 mm further forward than it goes, which cannot be:
    // there the disc and the wheel would be in the positioner's box.
    let (back, forward) = (f(&s["back_x"]), f(&s["forward_x"]));
    let mut places: Vec<(f64, bool)> = (0..=4)
        .map(|k| (back + (forward - back) * k as f64 / 4.0, true))
        .collect();
    places.push((forward + 60.0, false));
    for (at, fits) in places {
        let shift = f(&n["drawn"]["x"]) - at;
        let (mut tried, mut worst) = (0, (0.0f64, String::new()));
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
            if a.0 == "Sled plate" && fits && (at == back || at == forward) {
                let (up, far, near) = (
                    held(&moved, 0.0, 1.0),
                    held(&moved, -1.0, 0.0),
                    held(&moved, 2.0, 0.0),
                );
                println!("the bit {at:.0} along the sled: lifted 1 mm the plate is {up:.0} mm3 into a lip; pushed across, {far:.0} into the datum fence, {near:.0} into the near one");
                assert!(up > 100.0 && far > 100.0 && near > 100.0);
            }
        }
        println!(
            "the bit {at:.0} along the sled: {tried} pairs near each other, the most in common {:.3} mm3 ({})",
            worst.0, worst.1
        );
        if fits {
            assert!(
                tried >= 4 && worst.0 < 1.0,
                "the bit {at:.0} along: {} share {:.1} mm3",
                worst.1,
                worst.0
            );
        } else {
            assert!(
                worst.0 > 500.0 && worst.1.ends_with("and Router positioner"),
                "the bit {at:.0} along: {} share {:.1} mm3",
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
/// pulley in it and more; the motor is in the sled's box, its shaft out
/// past the box's end to the pulley; the wheel is screwed to the neck
/// of a disc, the disc bolted to a hub on the sled's own spindle; that
/// spindle runs in two pillow blocks bolted down on a riser on the
/// sled's deck; its end is in the boss of a printed socket whose pocket
/// holds the blank's end by its flats; the other end of the stick is on
/// the point of a second rod in two more pillow blocks, bare where the
/// bit is set against it; and a fin on the disc passes through a sensor
/// on the end of the deck once a turn.
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
    let t = &n["turn"];
    let w = &t["wheel"];
    let (big_t, small_t, c) = (f(&w[0]), f(&w[1]), f(&w[3]));
    let (big, small) = (big_t * 2.0 / PI, small_t * 2.0 / PI);
    let mesh = small_t * (PI - 2.0 * ((big - small) / 2.0 / c).asin()) / (2.0 * PI);
    println!(
        "{small_t:.0} teeth on the motor, {big_t:.0} on the wheel: {mesh:.1} of the pulley's teeth in the belt"
    );
    assert!(mesh >= 6.0 && (mesh - f(&t["mesh"])).abs() < 1e-9);
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    assert!((f(&sim["per"][2]) * 360.0 - 200.0 * 16.0 * big_t / small_t).abs() < 1e-6);
    assert!(big_t >= 300.0 && big_t / small_t == 12.0);
    // a pulley of 20 teeth, as the wheel had while its motor stood further off, would have too few in the belt here
    let small20 = 20.0 * 2.0 / PI;
    assert!(20.0 * (PI - 2.0 * ((big - small20) / 2.0 / c).asin()) / (2.0 * PI) < 6.0);
    // On the sled, from the stick outward: socket, two bearings, the hub, the disc, its neck, the wheel. The rod
    // is whole from the bottom of the socket's boss to inside the wheel.
    let spindle = bodies_of(&mut doc, "Spindle");
    assert_eq!(spindle.len(), 1);
    let (rod, rod_x) = (x_of(&spindle[0]), pair(&s["rod_x"]));
    assert!((rod.0 - rod_x.0).abs() < 1e-6 && (rod.1 - rod_x.1).abs() < 1e-6);
    let long = rod.1 - rod.0;
    assert!(
        (spindle[0].volume() - PI * rod_d * rod_d / 4.0 * long).abs() < 0.01 * spindle[0].volume()
    );
    let (disc_x, neck_x, wheel_x) = (pair(&t["disc_x"]), pair(&t["neck_x"]), pair(&t["wheel_x"]));
    let hub = x_of(&bodies_of(&mut doc, "Wheel hub")[0]);
    let head_riser = pair(&s["head_riser_x"]);
    assert!(
        (hub.1 - disc_x.0).abs() < 1e-6 && hub.0 > head_riser.1,
        "the hub is on the disc's inner face, its boss clear of the headstock"
    );
    let fin_l = f(&t["fin_l"]);
    let disc = bodies_of(&mut doc, "Disc").remove(0);
    let disc_b = disc.bounds().unwrap();
    assert!(
        (disc_b.0.x - (disc_x.0 - fin_l)).abs() < 1e-6 && (disc_b.1.x - neck_x.1).abs() < 1e-6,
        "the disc, its fin toward the headstock and its neck toward the wheel"
    );
    assert!((disc_b.1.z - disc_b.0.z - f(&t["disc"][0])).abs() < 0.2);
    let wheel = bodies_of(&mut doc, "Index wheel").remove(0);
    let wheel_b = wheel.bounds().unwrap();
    let rim = f(&t["wheel_rim"]);
    assert!((wheel_b.0.x - wheel_x.0).abs() < 1e-6 && (wheel_b.1.x - wheel_x.1).abs() < 1e-6);
    assert!(
        ((wheel_b.0.z + wheel_b.1.z) / 2.0 - za).abs() < 1e-6
            && (wheel_b.1.z - wheel_b.0.z - 2.0 * rim).abs() < 0.3
    );
    assert!(
        (rim - big / 2.0 - 1.4).abs() < 1e-9
            && (wheel_x.1 - wheel_x.0 - f(&t["wheel_w"])).abs() < 1e-9
    );
    assert!(
        (disc_x.1 - neck_x.0).abs() < 1e-9
            && (neck_x.1 - wheel_x.0).abs() < 1e-9
            && rod.1 > wheel_x.0
            && rod.1 < wheel_x.1,
        "the wheel is on the neck's end and the rod ends inside it"
    );
    assert!(
        overlap(&disc, &wheel) < 1e-6
            && overlap(&disc, &spindle[0]) < 1e-6
            && overlap(&wheel, &spindle[0]) < 1e-6
    );
    // the wheel's four screws go into the neck on a circle outside the hub's flange, which the disc is bolted to
    let (neck, hub_d) = (&t["neck"], f(&t["hub"][0]));
    assert!(f(&neck[2]) + 8.0 <= f(&neck[0]) && f(&neck[2]) >= hub_d + 6.0 && f(&neck[1]) >= 8.0);
    // The motor: in the sled's box, between its rails and under its deck, on a bracket screwed to the sled's
    // bottom; its shaft comes out past the box's end and through the bracket to the pulley, which is in the wheel's
    // plane and the belt's length from the spindle. The bracket stands beside the disc, clear of it.
    let drive = bodies_of(&mut doc, "Wheel drive");
    assert_eq!(drive.len(), 4);
    let (bracket, motor, pulley, shaft) = (
        drive[0].bounds().unwrap(),
        drive[1].bounds().unwrap(),
        drive[2].bounds().unwrap(),
        drive[3].bounds().unwrap(),
    );
    let (hollow_y, hollow_z, box_x) = (
        pair(&s["hollow_y"]),
        pair(&s["hollow_z"]),
        pair(&s["box_x"]),
    );
    let (motor_y, motor_z, drive_x) = (f(&t["motor_y"]), f(&t["motor_z"]), pair(&t["drive_x"]));
    assert!(
        ((motor.0.y + motor.1.y) / 2.0 - motor_y).abs() < 1e-6
            && ((motor.0.z + motor.1.z) / 2.0 - motor_z).abs() < 1e-6
    );
    assert!(
        motor.0.y > hollow_y.0 + 2.0
            && motor.1.y < hollow_y.1 - 2.0
            && motor.0.z > hollow_z.0
            && motor.1.z < hollow_z.1 - 2.0,
        "the motor is in the box"
    );
    assert!(
        motor.0.x < box_x.1 - 10.0
            && (motor.1.x - drive_x.0).abs() < 1e-6
            && (bracket.1.x - drive_x.1).abs() < 1e-6
    );
    assert!(
        (bracket.0.z - f(&s["plate_top"])).abs() < 1e-6 && bracket.1.x < f(&s["plate_x"][1]),
        "the bracket's foot is on the sled's bottom"
    );
    assert!(
        (motor_y.hypot(za - motor_z) - c).abs() < 1e-6,
        "the motor is the belt's length from the spindle"
    );
    assert!((pulley.0.x - wheel_x.0).abs() < 1e-6 && (pulley.1.x - wheel_x.1).abs() < 1e-6);
    assert!(
        ((pulley.0.y + pulley.1.y) / 2.0 - motor_y).abs() < 1e-6
            && ((pulley.0.z + pulley.1.z) / 2.0 - motor_z).abs() < 1e-6
    );
    assert!(
        (shaft.0.x - drive_x.0).abs() < 1e-6
            && (shaft.1.x - wheel_x.1).abs() < 1e-6
            && shaft.1.x - shaft.0.x <= f(&t["motor"][2]),
        "a NEMA 17's shaft is long enough to reach"
    );
    let corner = (bracket.1.y).hypot(za - bracket.1.z);
    println!(
        "the wheel's motor {c:.1} from the spindle, {:.1} under the deck; its bracket's nearest corner {corner:.1} from the axis, the disc's rim {:.1}",
        hollow_z.1 - motor.1.z,
        f(&t["disc"][0]) / 2.0
    );
    assert!(
        (corner - f(&t["bracket_r"])).abs() < 1e-6
            && corner >= f(&t["disc"][0]) / 2.0 + 5.0
            && bracket.1.x <= wheel_x.0 - 3.0
    );
    // the belt's lower run is over the sled's bottom all the way: at the wheel's rim and at the pulley
    let belt_low = (za - rim).min(motor_z - small / 2.0 - 1.5);
    assert!((belt_low - f(&t["belt_low"])).abs() < 1e-9 && belt_low - f(&s["plate_top"]) >= 5.0);
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
    assert!(bearings[0].0 > rod.0 && bearings[1].1 < hub.0 && bearings[1].1 <= head_riser.1 + 1e-6);
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
    // the home mark: the sensor on the end of the deck, under the spindle, facing the disc and short of it, its
    // slot at the fin's radius and the fin reaching into it
    let sensor = bodies_of(&mut doc, "Turn home sensor").remove(0);
    let (lo, hi) = sensor.bounds().unwrap();
    assert!((lo.x - box_x.1).abs() < 1e-6 && hi.x <= disc_x.0 - 1.5);
    let fin_end = disc_x.0 - fin_l;
    assert!(
        fin_end < hi.x - 3.0 && fin_end > hi.x - 8.0,
        "the fin is in the slot"
    );
    let flag_r = f(&t["flag_r"]);
    assert!(
        ((lo.z + hi.z) / 2.0 - (za - flag_r)).abs() < 1e-6 && ((lo.y + hi.y) / 2.0).abs() < 1e-6
    );
    assert!(
        hi.z <= f(&s["top"]) + 1e-6 && lo.z >= f(&s["top"]) - f(&s["ply"]) - 1e-6,
        "it is on the end of the deck"
    );
    // the fin goes round outside the hub's flange and inside where a lock's pad would be
    let lk = &n["lock"];
    assert!(
        flag_r > hub_d / 2.0 + 5.0 && flag_r < f(&lk["lock_r"]) - f(&lk["clamp"][4]) / 2.0 - 5.0
    );
    assert!(overlap(&sensor, &disc) < 1e-6);
    // The sensor says home over the fin's whole width. The controller's own homing, run by the build from nine
    // places in the fin and out of it, ends on the same edge of it every time, to a step.
    let homing = &t["homing"];
    let ends: Vec<(f64, f64)> = homing["ends"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect();
    let (fin, step) = (f(&homing["fin_deg"]), f(&homing["step_deg"]));
    assert!(
        (fin - (f(&t["fin"][1]) / flag_r).to_degrees()).abs() < 1e-9
            && f(&homing["clear_deg"]) > 1.0
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
    let at_edge = 0.02 / flag_r * f(&n["tile"]["swing"]);
    println!("the home fin at {flag_r:.0} mm: 0.02 mm there is {at_edge:.3} mm at the tile's edge");
    assert!(at_edge < 0.02);
}

/// Nothing but the belt holds the stick while it is cut. For every
/// cutting row, in the table's order, the area of section its bite
/// removes is followed again here on the planner's own grid; that, the
/// feed and how fast the bit's edge passes give how hard the bit
/// pushes; taken at the furthest thing it cuts, that is a turning on
/// the stick; and the belt's two runs and the motor's hold, one after
/// the other, give how far the tile's edge moves under it. It is an
/// estimate, nothing in it measured, and the table carries it row by
/// row: this is the same sum from the same assumptions by other code,
/// held to the same limits.
#[test]
fn the_belt_alone_holds_the_stick_at_every_cut() {
    let n = numbers();
    let (rows, tile, stock) = (rows(&n), outline(&n), blank(&n));
    let (h, t) = (&n["hold"], &n["turn"]);
    // how stiffly the stick is held: the belt's two runs, one tightening as the other slackens, and the motor
    let w = &t["wheel"];
    let (big, small, c) = (f(&w[0]) * 2.0 / PI, f(&w[1]) * 2.0 / PI, f(&w[3]));
    let span = (c * c - ((big - small) / 2.0).powi(2)).sqrt();
    let ratio = f(&w[0]) / f(&w[1]);
    let k_belt = 2.0 * f(&h["belt_ea"]) * (big / 2.0).powi(2) / span / 1000.0;
    let k_motor = 50.0 * f(&h["motor_hold"]) * ratio * ratio;
    let stiff = 1.0 / (1.0 / k_belt + 1.0 / k_motor);
    assert!((stiff - f(&h["stiff"])).abs() < 1e-6 * stiff && (span - f(&h["span"])).abs() < 1e-9);
    assert!((ratio - f(&t["ratio"])).abs() < 1e-12 && ratio >= 10.0);
    // the waste, on the planner's grid
    let cell = f(&h["cell"]);
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for q in &stock {
        lo = (lo.0.min(q.0), lo.1.min(q.1));
        hi = (hi.0.max(q.0), hi.1.max(q.1));
    }
    let mut cells: Vec<P2> = Vec::new();
    for i in 0..((hi.0 - lo.0) / cell) as usize + 1 {
        for k in 0..((hi.1 - lo.1) / cell) as usize + 1 {
            let q = (
                lo.0 + (i as f64 + 0.5) * cell,
                lo.1 + (k as f64 + 0.5) * cell,
            );
            if inside(q, &stock) && !inside(q, &tile) {
                cells.push(q);
            }
        }
    }
    let mut alive = vec![true; cells.len()];
    let ball = f(&n["bit"]) / 2.0;
    let (taper, shank) = (f(&n["taper"]).to_radians(), f(&n["shank"]) / 2.0);
    let flat = f(&n["rough"]["bit"]) / 2.0;
    let (energy, rpm, peak, feed) = (
        f(&h["cut_energy"]),
        f(&h["rpm"]),
        f(&h["peak"]),
        f(&h["feed"]),
    );
    let (swing, limit) = (f(&n["tile"]["swing"]), pair(&h["limit"]));
    let (mut worst, mut heaviest, mut cuts) = ([0.0f64; 3], (0.0f64, 0.0f64), 0);
    for (r, told) in rows.iter().zip(n["rows"].as_array().unwrap()) {
        if r.kind != "rough" && r.kind != "finish" {
            continue;
        }
        cuts += 1;
        let (mut count, mut over, mut far) = (0usize, 0.0f64, 0.0f64);
        for (i, q) in cells.iter().enumerate() {
            if !alive[i] {
                continue;
            }
            let (u, up) = seen(*q, r.t);
            let off = (u - r.across).abs();
            let stands = if r.tool == "ball" {
                match flank(off, ball, taper, shank) {
                    Some(side) => up - (r.tip + ball + side),
                    None => continue,
                }
            } else if off <= flat {
                up - r.tip
            } else {
                continue;
            };
            if stands >= 0.0 {
                alive[i] = false;
                count += 1;
                over = over.max(stands);
                far = far.max(q.0.hypot(q.1));
            }
        }
        let area = count as f64 * cell * cell;
        // how fast the edge passes: the straight bit's own diameter; the ball-nose's half way up what it is cutting
        let dia = if r.tool != "ball" {
            2.0 * flat
        } else if over / 2.0 < ball {
            let a = over / 2.0;
            (2.0 * (2.0 * ball * a - a * a).max(0.0).sqrt()).max(1.0)
        } else {
            (2.0 * shank).min(2.0 * ball + 2.0 * (over / 2.0 - ball) * taper.tan())
        };
        let push = peak * energy * 1000.0 * area * feed / (PI * dia * rpm / 60.0);
        let torque = push * far / 1000.0;
        let give = torque / stiff * swing;
        let face = r.kind == "finish" && r.last_bite;
        assert!(
            (area - f(&told["area"])).abs() < 0.05 + 0.01 * area,
            "row {}: {area:.2} mm2 here, {} in the table",
            r.n,
            told["area"]
        );
        assert!(
            (give - f(&told["give"])).abs() < 2e-5 + 0.02 * give,
            "row {}: {give:.5} here, {} in the table",
            r.n,
            told["give"]
        );
        assert_eq!(told["face"].as_bool(), Some(face));
        assert!(
            give <= if face { limit.1 } else { limit.0 } + 1e-9,
            "row {} would move the tile's edge {give:.4}",
            r.n
        );
        let k = if r.kind == "rough" {
            0
        } else if face {
            2
        } else {
            1
        };
        worst[k] = worst[k].max(give);
        if torque > heaviest.0 {
            heaviest = (torque, push);
        }
    }
    let stall = f(&h["motor_hold"]) * ratio / heaviest.0;
    println!(
        "held {stiff:.0} N m a radian ({k_belt:.0} the belt, {k_motor:.0} the motor). The heaviest of {cuts} cuts pushes {:.1} N, {:.2} N m: {stall:.0} times under what stalls the motor. \
         The tile's edge moves at most {:.1} um roughing, {:.1} in a finishing pass's earlier bites, {:.1} in one that leaves a face",
        heaviest.1,
        heaviest.0,
        1000.0 * worst[0],
        1000.0 * worst[1],
        1000.0 * worst[2]
    );
    assert!(cuts > 500 && stall > 10.0 && (stall - f(&h["stall_over"])).abs() < 0.02 * stall);
    for (k, key) in ["rough", "bite", "face"].iter().enumerate() {
        assert!((worst[k] - f(&h["worst"][*key][0])).abs() < 2e-5 + 0.02 * worst[k]);
    }
    // The limits are small beside what they guard: a fifth of the ridge a pass may leave where a face is left,
    // a twentieth of what roughing leaves on anywhere else. And the assumptions are on the heavy side: the Colt
    // on its slowest setting, and wood that takes as much to cut as any figure published for it.
    assert!(limit.1 <= f(&n["ridge"]) / 5.0 && limit.0 <= f(&n["rough"]["allow"]) / 20.0);
    assert!(rpm <= 16000.0 && energy >= 0.15 && peak >= 2.0);
    // the bites the planner was given: shallower than when a hand pushed the sled and a clamp held the stick
    assert!(f(&n["rough"]["bite"]) <= 2.0 && f(&n["plan"]["ball_bite"]) <= 1.0);
}

/// A screw feeds the sled. It lies along the base, its motor on a block
/// at the tail end beyond where the sled's end comes, and goes in
/// through the tail end of the sled's box to a nut on the sled's
/// bottom; its far end is free. The nut is on the screw from one end
/// of the travel to the other, the screw through the nut and through
/// nothing else. A stop block stands a millimetre beyond each end of
/// the travel, nearer than the collet nut comes to the socket. And
/// every pass ends with the sled finding its switch: the controller's
/// own pass, run by the build on a sled that loses steps, carries on
/// after a little lost and stops after more, whichever way it was
/// going.
#[test]
fn a_screw_feeds_the_sled_and_it_finds_its_switch_after_every_pass() {
    let n = numbers();
    let (s, tr, t) = (&n["sled"], &n["travel"], &n["turn"]);
    let mut doc = load();
    let drawn = f(&n["drawn"]["x"]);
    let (back, forward) = (f(&s["back_x"]), f(&s["forward_x"]));
    let (screw, nut) = (&tr["screw"], &tr["nut"]);
    let (y, z) = (f(&tr["y"]), f(&tr["z"]));
    // on the base: the motor's bracket, the block it stands on, the motor, a coupler, the motor's shaft, the screw
    let drive = bodies_of(&mut doc, "Travel drive");
    assert_eq!(drive.len(), 6);
    let rod = drive[5].bounds().unwrap();
    let screw_x = pair(&tr["screw_x"]);
    assert!(
        (rod.0.x - drawn - screw_x.0).abs() < 1e-6 && (rod.1.x - drawn - screw_x.1).abs() < 1e-6
    );
    assert!((rod.1.x - rod.0.x - f(&screw[2])).abs() < 1e-6);
    assert!(
        ((rod.0.y + rod.1.y) / 2.0 - y).abs() < 1e-6
            && ((rod.0.z + rod.1.z) / 2.0 - z).abs() < 1e-6
    );
    let d = f(&screw[0]);
    assert!((drive[5].volume() - PI * d * d / 4.0 * f(&screw[2])).abs() < 0.01 * drive[5].volume());
    let block = drive[1].bounds().unwrap();
    assert!(
        block.0.z.abs() < 1e-6 && (block.1.z - f(&s["t12"])).abs() < 1e-6,
        "the motor's block is on the base"
    );
    assert!(
        overlap(&drive[0], &drive[1]) < 1e-6
            && (drive[0].bounds().unwrap().0.z - block.1.z).abs() < 1e-6
    );
    // on the sled: the nut's block on the bottom, and the nut, flange and body, on its outer face and through it
    let on_sled = bodies_of(&mut doc, "Travel nut");
    assert_eq!(on_sled.len(), 3);
    let (nb, nut_b) = (on_sled[0].bounds().unwrap(), on_sled[1].bounds().unwrap());
    let nut_x = pair(&tr["nut_x"]);
    assert!(
        (nb.0.x - nut_x.0).abs() < 1e-6
            && (nb.1.x - nut_x.1).abs() < 1e-6
            && (nb.0.z - f(&s["plate_top"])).abs() < 1e-6
    );
    assert!(
        (nut_b.0.x - (nut_x.0 - f(&nut[1]))).abs() < 1e-6,
        "the nut's flange is on the block's outer face"
    );
    assert!(
        nut_x.0 > f(&s["box_x"][0]) && nut_x.0 < f(&s["box_x"][0]) + 20.0,
        "just inside the box's tail end"
    );
    // in the box: between the rails, under the deck, the nut's flange over the bottom
    let (hy, hz, ply) = (pair(&s["hollow_y"]), pair(&s["hollow_z"]), f(&s["ply"]));
    assert!(
        (hy.0 - f(&s["deck_y"][0]) - ply).abs() < 1e-9
            && (hy.1 - f(&s["rail_near"]) + ply).abs() < 1e-9
    );
    assert!((hz.0 - f(&s["plate_top"])).abs() < 1e-9 && (hz.1 - f(&s["top"]) + ply).abs() < 1e-9);
    assert!(nb.0.y > hy.0 + 5.0 && nb.1.y < hy.1 - 5.0 && nb.1.z < hz.1 - 3.0);
    assert!(z - f(&nut[0]) / 2.0 >= hz.0 + 3.0);
    // The nut is on the screw wherever the sled is. Forward, its flange is as near the coupler as it comes;
    // back, the far side of its block is as far along the screw as it goes. Both along the base, the bit at 0.
    let fwd_flange = nut_x.0 - f(&nut[1]) - forward;
    let back_far = nut_x.1 - back;
    let coupler_x = pair(&tr["coupler_x"]);
    println!(
        "the screw from {:.0} to {:.0} along the base: the nut's flange {:.1} from the coupler with the sled forward, {:.1} of screw past the nut's block with it back",
        screw_x.0,
        screw_x.1,
        fwd_flange - coupler_x.1,
        screw_x.1 - back_far
    );
    assert!(
        (fwd_flange - f(&tr["nut_fwd"])).abs() < 1e-9
            && (back_far - f(&tr["nut_back"])).abs() < 1e-9
    );
    assert!(fwd_flange - coupler_x.1 >= 5.0 && screw_x.1 - back_far >= 10.0);
    assert!(
        screw_x.0 > coupler_x.0 && screw_x.0 < coupler_x.1,
        "the screw's end is in the coupler"
    );
    // forward, the screw reaches furthest into the box: short of the wheel's motor at the other end
    assert!(screw_x.1 + forward < f(&t["drive_x"][0]) - f(&t["motor"][1]) - 10.0);
    // the sled at both ends of its travel: the screw through the nut and its block and in neither; a
    // millimetre off its line, it is in the nut
    let moved = |solid: &Solid, dx: f64, dy: f64| {
        solid.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
            dx, dy, 0.0,
        )))
    };
    for at in [back, forward] {
        for part in &on_sled {
            assert!(overlap(&moved(part, drawn - at, 0.0), &drive[5]) < 1e-3);
        }
        assert!(
            overlap(&moved(&on_sled[1], drawn - at, 1.0), &drive[5]) > 5.0,
            "the screw is in the nut"
        );
    }
    // The motor is beyond where the sled's end comes, and beyond the forward stop's face. A stop block stands a
    // little beyond each end of the travel: a sled that overruns meets one before the collet nut meets the socket.
    let (stops, guard) = (pair(&s["stops_x"]), f(&s["guard"]));
    assert!(coupler_x.1 <= stops.0 - guard - 3.0);
    let blocks = bodies_of(&mut doc, "Stops");
    assert_eq!(blocks.len(), 2);
    let (fwd, bk) = (blocks[0].bounds().unwrap(), blocks[1].bounds().unwrap());
    assert!(
        (fwd.1.x - drawn - (stops.0 - guard)).abs() < 1e-6
            && (bk.0.x - drawn - (stops.1 + guard)).abs() < 1e-6
    );
    assert!(guard > 0.0 && guard < f(&s["socket"]["nut_clear"]));
    let switch = bodies_of(&mut doc, "Travel home switch")
        .remove(0)
        .bounds()
        .unwrap();
    assert!(
        (switch.0.x - drawn - stops.1).abs() < 1e-6,
        "the switch is where the bottom's end is with the sled back"
    );
    // a four-start screw of 8 mm a turn, its driver at 4 microsteps: a hundredth of a millimetre a step
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    let per = f(&sim["per"][3]);
    assert!(
        (per - 200.0 * 4.0 / f(&screw[1])).abs() < 1e-9
            && (per - f(&tr["steps_per_mm"])).abs() < 1e-9
    );
    // Coming home. The sled runs fast to a little short of home and then finds its switch; how far that was from
    // where its steps said is what it lost. The build ran the controller's own pass on a sled that loses steps.
    let (short, slip_max) = (f(&tr["home_short"]), f(&tr["home_slip"]));
    assert!(slip_max < short && short <= 5.0 && slip_max <= 0.5);
    let came = tr["came"].as_array().unwrap();
    let (mut carried, mut stopped) = (0, 0);
    for c in came {
        let lost = f(&c["lost_out"]).max(f(&c["lost_back"]));
        let said = c["stopped"].as_str().unwrap();
        if lost > slip_max {
            assert!(!said.is_empty(), "{lost} mm lost and it carried on");
            stopped += 1;
        } else {
            assert!(
                said.is_empty()
                    && f(&c["ends"]).abs() <= 1.0 / per + 1e-9
                    && f(&c["counted"]) == 0.0
            );
            assert!((f(&c["slip"]) - lost).abs() <= 1.0 / per + 1e-9);
            carried += 1;
        }
    }
    println!("coming home on a sled that loses steps: {carried} cases carried on, home to a step; {stopped} stopped");
    assert!(carried >= 3 && stopped >= 3);
    assert!(
        came.iter().any(|c| f(&c["lost_out"]) > slip_max)
            && came.iter().any(|c| f(&c["lost_back"]) > slip_max)
    );
}

/// A lock on the spindle is an option nobody has fitted: the belt
/// holds the stick. Its four parts are drawn, each in its own tab, and
/// are in neither assembly. Their place is kept: the post and the
/// clamp's riser stand on the deck beside the headstock, the clamp's
/// pad is on the disc's inner face inside its rim, and the anvil comes
/// in behind the disc, between it and the wheel and outside the disc's
/// neck, a hair off until the clamp closes. (That they are through
/// nothing, wherever the sled is, is the model's test: they are among
/// what rides.) The controller, run as if a lock were fitted, lets it
/// go once for every new angle.
#[test]
fn a_lock_is_drawn_in_its_place_and_left_off_the_machine() {
    let n = numbers();
    let (s, t, lk) = (&n["sled"], &n["turn"], &n["lock"]);
    let mut doc = load();
    assert_eq!(lk["fitted"].as_bool(), Some(false));
    let parts: Vec<&str> = lk["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    assert_eq!(
        parts,
        ["Lock post", "Lock anvil", "Toggle clamp", "Lock switch"]
    );
    // in the document, and in neither assembly
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/sled.okpart")).unwrap())
            .unwrap();
    let mut assemblies = 0;
    for tab in saved["tabs"].as_array().unwrap() {
        let kind = &tab["kind"];
        if kind["type"].as_str() != Some("assembly") {
            continue;
        }
        assemblies += 1;
        let names: Vec<&str> = kind["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap())
            .collect();
        assert!(names.len() >= 40 && names.contains(&"Disc") && names.contains(&"Index wheel"));
        assert!(
            names
                .iter()
                .all(|k| !parts.iter().any(|p| k.starts_with(p))),
            "a lock's part is on the machine"
        );
    }
    assert_eq!(assemblies, 2);
    let (za, top) = (f(&s["za"]), f(&s["top"]));
    let (disc_d, disc_x, wheel_x, neck_d) = (
        f(&t["disc"][0]),
        pair(&t["disc_x"]),
        pair(&t["wheel_x"]),
        f(&t["neck"][0]),
    );
    let (lock_r, pad, gap) = (f(&lk["lock_r"]), f(&lk["clamp"][4]), f(&lk["anvil"][2]));
    // the post and the clamp's riser stand on the deck, beside the headstock's riser and hard against each other
    let post = bodies_of(&mut doc, "Lock post");
    assert_eq!(post.len(), 2);
    let (p_lo, p_hi) = post[0].bounds().unwrap();
    let post_x = pair(&lk["post_x"]);
    assert!(
        (p_lo.z - top).abs() < 1e-6
            && (p_lo.x - post_x.0).abs() < 1e-6
            && (p_hi.x - post_x.1).abs() < 1e-6
    );
    assert!(
        p_lo.y >= f(&s["deck_y"][0])
            && p_hi.y <= -f(&s["riser_half"]) + 1e-6
            && (post_x.1 - f(&s["head_riser_x"][1])).abs() < 1e-9
    );
    let (r_lo, r_hi) = post[1].bounds().unwrap();
    assert!(
        (r_lo.z - top).abs() < 1e-6
            && (r_hi.x - post_x.0).abs() < 1e-6
            && r_lo.x > f(&s["box_x"][0])
    );
    // the clamp: level with the axis, its pad on the disc's inner face and inside the disc's rim
    let clamp = bodies_of(&mut doc, "Toggle clamp").remove(0);
    let (c_lo, c_hi) = clamp.bounds().unwrap();
    assert!((c_hi.x - disc_x.0).abs() < 1e-6, "the pad is on the disc");
    assert!(
        lock_r + pad / 2.0 <= disc_d / 2.0 + 1e-9 && ((c_lo.z + c_hi.z) / 2.0 - za).abs() < 1e-6
    );
    // the anvil: behind the disc with a hair between, short of the wheel, and outside the disc's neck
    let anvil = bodies_of(&mut doc, "Lock anvil").remove(0);
    let (a_lo, a_hi) = anvil.bounds().unwrap();
    assert!(
        (a_hi.x - disc_x.1 - gap - f(&lk["anvil"][0])).abs() < 1e-6 && a_hi.x + 2.0 <= wheel_x.0
    );
    assert!(a_hi.y >= -(lock_r - pad / 2.0) && a_lo.y <= -(lock_r + pad / 2.0));
    assert!(
        -a_hi.y >= neck_d / 2.0 + 5.0,
        "the anvil is outside the disc's neck"
    );
    assert!(gap > 0.0 && gap <= 0.5);
    // the disc between them: clear of both, and on the anvil when pushed that hair and a little
    let disc = bodies_of(&mut doc, "Disc").remove(0);
    assert!(overlap(&disc, &anvil) < 1e-6 && overlap(&disc, &clamp) < 1e-3);
    let pushed = disc.transformed(&ok_brep::Transform::translation(ok_math::Vec3::new(
        gap + 0.5,
        0.0,
        0.0,
    )));
    assert!(overlap(&pushed, &anvil) > 10.0);
    // What it would hold, by friction at the pinch, against the turning the heaviest cut is estimated to ask.
    let holds = 2.0 * 0.2 * 500.0 * lock_r / 1000.0;
    let asked = n["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["torque"].as_f64())
        .fold(0.0, f64::max);
    println!("a 500 N clamp at {lock_r:.0} mm would hold {holds:.1} N m; the heaviest cut is estimated to ask {asked:.2}");
    assert!(asked > 0.0 && holds > 20.0 * asked);
    // the controller, run as if it were fitted
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    assert_eq!(sim["with_lock"]["unlocks"].as_u64(), lk["unlocks"].as_u64());
    assert!(lk["unlocks"].as_u64().unwrap() > 150);
}

/// The two animations, read back frame by frame. Both have a frame
/// for every cutting row of the table; the first frame is the blank
/// and nothing else; the cutter is in every pass's frame; what is left
/// of the blank never grows; the tile's middle is never cut; and the
/// last frame is the tile, to the area the planner says is left.
/// `out/passes.gif` is fixed to the stick. `out/passes_machine.gif` is
/// as the machine has it: in every pass's frame the cutter is level,
/// reaches in from the right edge, and is at the height and the depth
/// its row gives; the stick is turned by the row's own angle; and
/// between two angles far apart there are frames of the stick on its
/// way round with no cutter in them.
#[test]
fn the_animations_have_a_frame_for_every_pass_and_end_on_the_tile() {
    let n = numbers();
    let a: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/passes_anim.json")).unwrap())
            .unwrap();
    let cutting: Vec<Row> = rows(&n)
        .into_iter()
        .filter(|r| r.kind == "rough" || r.kind == "finish")
        .collect();
    let cuts = cutting.len();
    assert_eq!(
        a["cuts"].as_u64().unwrap() as usize,
        cuts,
        "a frame for every cutting row"
    );
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
    let size = a["size"].as_u64().unwrap() as usize;
    let per_mm = size as f64 / (f(&a["view"][2]) - f(&a["view"][0]));
    let blank = pair(&n["blank"]["size"]);
    let want = f(&n["tile"]["area"]) + f(&n["rough"]["left_at_end"]);
    let ms = f(&a["ms"]);
    assert!(ms >= 40.0);
    /// What a frame shows: the blank's colour as area, the cutter's pixels and the box round them, whether the
    /// tile's middle is still the blank's colour, how long it is shown, and which way what is left lies.
    struct Seen {
        left: f64,
        cutter: usize,
        boxed: (usize, usize, usize, usize),
        middle: bool,
        ms: u32,
        lies: f64,
    }
    let mut lies_at_the_end = Vec::new();
    for name in ["passes", "passes_machine"] {
        let book = &a[name];
        let frames = book["frames"].as_u64().unwrap() as usize;
        let at: Vec<usize> = book["cut_frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(at.len(), cuts);
        assert!(at.windows(2).all(|w| w[0] < w[1]) && at[cuts - 1] < frames - 1);
        // The first frame and the last are read pixel by pixel; the rest every other pixel each way, which is
        // enough to place a cutter to a quarter of a millimetre and to see what is left shrink.
        let mut seen: Vec<Seen> = Vec::new();
        let bytes = std::fs::read(dir().join(format!("out/{name}.gif"))).unwrap();
        let (w, h, read) = ok_render::flipbook::read_gif(&bytes, |k, image, ms| {
            let step = if k == 0 || k == frames - 1 { 1 } else { 2 };
            let (mut left, mut cutter) = (0.0, 0usize);
            let mut boxed = (usize::MAX, 0usize, usize::MAX, 0usize);
            let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in (0..size).step_by(step) {
                for x in (0..size).step_by(step) {
                    let c = image.pixel(x, y);
                    if near(c, blue) {
                        cutter += 1;
                        boxed = (
                            boxed.0.min(x),
                            boxed.1.max(x),
                            boxed.2.min(y),
                            boxed.3.max(y),
                        );
                    } else if c.2 < 255 && c.0 >= tan[0] {
                        // between white and the blank's colour: an edge pixel is part blank
                        let part = (255.0 - c.2 as f64) / (255.0 - tan[2] as f64);
                        let (px, py) = (x as f64, (size - y) as f64);
                        left += part;
                        sx += part * px;
                        sy += part * py;
                        sxx += part * px * px;
                        syy += part * py * py;
                        sxy += part * px * py;
                    }
                }
            }
            // which way what is left lies: the long axis of its second moments, y up
            let (mx, my) = (sx / left, sy / left);
            let lies = 0.5
                * (2.0 * (sxy / left - mx * my))
                    .atan2((sxx / left - mx * mx) - (syy / left - my * my));
            let each = (step * step) as f64 / per_mm / per_mm;
            seen.push(Seen {
                left: left * each,
                cutter: cutter * step * step,
                boxed,
                middle: near(image.pixel(size / 2, size / 2), tan),
                ms,
                lies: lies.to_degrees(),
            });
        })
        .unwrap();
        assert_eq!((w, h, read), (size, size, frames));
        let last = &seen[frames - 1];
        println!(
            "{name}: {frames} frames, {} bytes; the blank {:.0} mm2 in the first ({:.0} drawn), {:.0} left in the last (the tile {:.0} and {:.1} standing)",
            bytes.len(),
            blank.0 * blank.1,
            seen[0].left,
            last.left,
            f(&n["tile"]["area"]),
            f(&n["rough"]["left_at_end"])
        );
        // the first frame: the blank, whole, and no cutter; the last: no cutter, no outlines, and the tile
        assert!(
            (seen[0].left - blank.0 * blank.1).abs() < 0.005 * blank.0 * blank.1
                && seen[0].cutter == 0
        );
        assert!(
            last.cutter == 0 && last.middle && (last.left - want).abs() < 0.015 * want,
            "{} for {want}",
            last.left
        );
        lies_at_the_end.push(last.lies);
        // every pass: the cutter is there, the tile's middle is still the blank's colour, and it is shown long enough
        for (k, frame) in at.iter().enumerate() {
            let s = &seen[*frame];
            assert!(s.cutter > 100, "{name}: no cutter in pass {k}");
            assert!(s.middle, "{name}: the tile's middle is cut in pass {k}");
            assert!(s.ms as f64 >= ms);
        }
        // What is left only ever shrinks: frame by frame, give or take the outlines drawn over its edges and the
        // pixels not looked at, which together are under a hundredth of the blank.
        let slack = 0.01 * blank.0 * blank.1;
        let mut least = seen[0].left;
        for (k, s) in seen.iter().enumerate().skip(1) {
            assert!(
                s.left <= least + slack,
                "{name}: more is left in frame {k} than before it: {} after {least}",
                s.left
            );
            least = least.min(s.left);
        }
        // and by the end of roughing most of the waste is gone
        let roughed = seen[at[a["last_rough"].as_u64().unwrap() as usize]].left;
        assert!(
            roughed < seen[0].left - 0.75 * (seen[0].left - last.left),
            "{roughed}"
        );
        let total: f64 = seen.iter().map(|s| s.ms as f64).sum::<f64>() / 1000.0;
        assert!(
            (total - f(&book["seconds"])).abs() < 0.5 && total < 60.0,
            "{total} s"
        );
        // the file is small enough to keep: every frame is only what changed
        assert!(bytes.len() < 5_000_000, "{} bytes", bytes.len());
        let turns: Vec<f64> = book["turns"].as_array().unwrap().iter().map(f).collect();
        assert_eq!(turns.len(), frames);
        if name == "passes" {
            // fixed to the stick: a frame a pass and nothing between, and nothing ever turned
            assert!(
                at.iter().enumerate().all(|(k, v)| *v == k + 1) && turns.iter().all(|t| *t == 0.0)
            );
            continue;
        }
        // As the machine has it. Every pass's frame is turned by its row's own angle, and in it the cutter is level:
        // a bar no higher than the widest bit, in from the right edge, its middle at the row's height over the
        // stick's axis and its end at the row's depth from it.
        let widest = f(&n["rough"]["bit"]).max(f(&n["shank"]));
        let mut worst = (0.0f64, 0.0f64);
        for (k, (frame, row)) in at.iter().zip(&cutting).enumerate() {
            assert!(
                ((turns[*frame] - row.turn + 180.0).rem_euclid(360.0) - 180.0).abs() < 1e-3,
                "pass {k}"
            );
            let b = seen[*frame].boxed;
            assert!(
                b.1 >= size - 3,
                "pass {k}: the cutter does not come in from the right"
            );
            assert!(
                (b.3 - b.2) as f64 <= widest * per_mm + 4.0,
                "pass {k}: the cutter is not level"
            );
            let height = (size as f64 / 2.0 - (b.2 + b.3) as f64 / 2.0) / per_mm;
            let depth = (b.0 as f64 - size as f64 / 2.0) / per_mm;
            worst = (
                worst.0.max((height - row.across).abs()),
                worst.1.max((depth - row.tip).abs()),
            );
        }
        println!("   the cutter is within {:.2} mm of its row's height and {:.2} of its depth in every pass", worst.0, worst.1);
        assert!(worst.0 < 0.4 && worst.1 < 0.5);
        // Between two angles more than a step apart the stick is shown on its way round, no cutter in the frame.
        let step = f(&a["turn_step"]);
        let between: Vec<usize> = (1..at[cuts - 1]).filter(|k| !at.contains(k)).collect();
        assert!(between.len() > 20 && between.iter().all(|k| seen[*k].cutter == 0));
        for k in 1..=at[cuts - 1] {
            let swing = ((turns[k] - turns[k - 1] + 180.0).rem_euclid(360.0) - 180.0).abs();
            assert!(swing <= step + 1e-6, "frame {k} turns {swing}");
        }
        // The first frame is the blank as the table's first row hangs it, and the last is the tile turned as the
        // last pass left it: it lies that much round from where the other animation's last frame has it.
        let load = rows(&n).into_iter().find(|r| r.kind == "load").unwrap();
        assert!(((turns[0] - load.turn + 180.0).rem_euclid(360.0) - 180.0).abs() < 1e-3);
        let round = (lies_at_the_end[1] - lies_at_the_end[0] - turns[frames - 1] + 90.0)
            .rem_euclid(180.0)
            - 90.0;
        println!("   the tile lies {:.1} deg in the one and {:.1} in the other, {:.1} deg round: {round:.2} from the last pass's angle",
            lies_at_the_end[0], lies_at_the_end[1], turns[frames - 1]);
        assert!(round.abs() < 1.0);
    }
}

/// The sled is a flat plate on the base, between two fences. Its far
/// edge runs on the datum fence and spring plungers in its near edge
/// keep it there; it rides on two slips; each fence has a lip over it,
/// a hair clear of its top. The fences reach past both ends of its
/// travel. Drawn back, the bit is beside the master, clear of the
/// tailstock and of the stick's end; forward, its collet nut is short
/// of the socket. The sled's bottom is longer than its box: it goes on
/// under the wheel.
#[test]
fn the_sled_slides_between_two_fences_and_drawn_back_the_bit_is_beside_the_master() {
    let n = numbers();
    let s = &n["sled"];
    let (plate, stops, drawn) = (pair(&s["plate_x"]), pair(&s["stops_x"]), f(&s["drawn_x"]));
    // The ends of the travel are in the base's frame, the bit at 0: where the bottom's two ends come to. With the
    // bottom's end at one, the bit is here along the sled:
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
    // rails and deck are the box's length; the bottom goes on past them, under the wheel and a little beyond it
    let (box_x, wheel_x) = (pair(&s["box_x"]), pair(&n["turn"]["wheel_x"]));
    for part in &boxed {
        let b = part.bounds().unwrap();
        assert!((b.0.x - box_x.0).abs() < 1e-6 && (b.1.x - box_x.1).abs() < 1e-6);
    }
    assert!((lo.x - box_x.0).abs() < 1e-6 && hi.x > wheel_x.1 && box_x.1 < wheel_x.0);
    assert!((deck.1.z - f(&s["top"])).abs() < 1e-6 && (deck.1.z - deck.0.z - ply).abs() < 1e-6);
    assert!(deck.0.y >= datum.1.y + 1.5 && deck.0.z > f(&s["lip_top"]) + 20.0);
    assert!(
        (f(&s["face"]) - deck.1.y - f(&s["deck_clear"])).abs() < 1e-6 && f(&s["deck_clear"]) >= 5.0
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
    assert_eq!(bodies_of(&mut doc, "Travel home switch").len(), 1);
    // drawn mid-pass, the plate is between the ends of its travel and at neither
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
    let wheel = &n["turn"]["wheel"];
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
    // The wheel is on the stick's own spindle: between the motor and the stick there is the belt and nothing
    // else, so the wheel's runout and the pulley's are all there is to add.
    assert!(turning.get("star_seat").is_none() && turning.get("stance").is_none());
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
/// its side over the stick, in bites of 4 mm, each where the sled's
/// screw takes the stick and by the wheel's own motor.
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
    let per = [f(&sim["per"][0]), f(&sim["per"][1]), f(&sim["per"][3])];
    let done = sim["bands"].as_array().unwrap();
    assert_eq!(done.len(), rows.len());
    let (mut front, mut top, mut rounds) = (Vec::new(), Vec::new(), 0);
    let mut spots = Vec::new();
    let sled = &n["sled"];
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
                assert!(
                    f(&s[4]) == 0.0,
                    "the bit is placed with the sled drawn back"
                );
            }
            "round" => {
                // the sled's own screw takes the stick to where the band goes, and it is there while the wheel turns it
                rounds += 1;
                let along = f(&row["along"]);
                assert!(along > 0.0 && along <= f(&sled["travel"]));
                assert!((f(&s[4]) - along * per[2]).abs() <= 0.5 + 1e-6);
                spots.push(along + f(&sled["back_x"]));
            }
            _ => assert!(f(&row["along"]) == 0.0),
        }
    }
    println!("bands from the front at {front:?}, from above at {top:?}; {rounds} turns");
    assert_eq!(rounds, 12);
    // The wheel's motor turns each band, the sled away from home where its screw has put it: a whole turn each, so
    // the table ends with the wheel where it began. For each bite the far band is turned first and then the near
    // one, the same distance apart every time: the bit's end makes its bands a little further along the stick
    // than its side does, so that neither cuts where the other has.
    assert_eq!(sim["band_rounds"].as_u64().unwrap(), 12);
    assert!(done.iter().all(|s| f(&s[3]) == f(&done[0][3])));
    let (span, at) = (f(&n["bands"]["span"]), pair(&n["bands"]["at"]));
    assert_eq!(spots.len(), 12);
    for (k, two) in spots.chunks(2).enumerate() {
        let near = if k < 3 { at.0 } else { at.1 };
        assert!((two[0] - near - span).abs() < 1e-3 && (two[1] - near).abs() < 1e-3);
    }
    assert!(at.0 != at.1 && at.1 > 5.0 && at.0 + span < f(&sled["cut_to"]));
    for bands in [&front, &top] {
        assert_eq!(bands.len(), 3);
        assert!(bands
            .windows(2)
            .all(|w| w[0] > w[1] && w[0] - w[1] <= 4.0 + 1e-3));
        assert!((bands[2] - f(&n["bands"]["r"])).abs() < 1e-3);
    }
}
