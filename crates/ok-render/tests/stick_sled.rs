//! The stick sled (examples/stick-sled) as a regression suite. The
//! table of rows the controller reads is three numbers a row, where
//! two screws' noses and a spindle stand; this works the hinges and
//! links forward from them and checks the cutter is where the plan
//! wants it, the whole of the bit never in the tile, that the passes
//! reach the whole outline, that run in the table's order no cut
//! meets more than its bite, that the stick, the pulley and the
//! hangers clear the table at every row, that the controller's own
//! program, simulated, ends every row on the table's steps and never
//! swings the stick across the cutter below the safe height, and that
//! the model is the sled at one of the rows. And the setting up of a
//! built sled: that the probing table touches what it says it touches
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

fn rot(a: f64, v: P2) -> P2 {
    (v.0 * a.cos() - v.1 * a.sin(), v.0 * a.sin() + v.1 * a.cos())
}

fn add(a: P2, b: P2) -> P2 {
    (a.0 + b.0, a.1 + b.1)
}

/// The sled's hinges, as plan.py has them, written again here so the
/// table is checked by other code than made it.
struct Sled {
    h1: P2,
    arm: P2,
    drop: f64,
    p1_y: f64,
    e1: P2,
    l1: f64,
    p2_z: f64,
    e2: P2,
    l2: f64,
    c1_home: f64,
    c2_home: f64,
    cutter_y: f64,
    bit_up: f64,
}

struct Pose {
    alpha: f64,
    hang: f64,
    axis: P2,
    across: f64,
    tip: f64,
    t: f64,
}

impl Sled {
    fn new(n: &Value) -> Sled {
        let s = &n["sled"];
        Sled {
            h1: pair(&s["h1"]),
            arm: pair(&s["arm"]),
            drop: f(&s["drop"]),
            p1_y: f(&s["p1_y"]),
            e1: pair(&s["e1"]),
            l1: f(&s["l1"]),
            p2_z: f(&s["p2_z"]),
            e2: pair(&s["e2"]),
            l2: f(&s["l2"]),
            c1_home: f(&s["c1_home"]),
            c2_home: f(&s["c2_home"]),
            cutter_y: f(&s["cutter_y"]),
            bit_up: f(&n["bit_up"]),
        }
    }

    fn hinge2(&self, alpha: f64) -> P2 {
        add(self.h1, rot(alpha, self.arm))
    }

    /// Link 1's pin on the plate, and link 2's on the hanging frame.
    fn pin1(&self, alpha: f64) -> P2 {
        add(self.h1, rot(alpha, self.e1))
    }

    fn pin2(&self, alpha: f64, hang: f64) -> P2 {
        add(self.hinge2(alpha), rot(hang, self.e2))
    }

    /// Nose 1's pin on its upright screw, a link's length under the plate's.
    fn pusher1(&self, alpha: f64) -> f64 {
        let q = self.pin1(alpha);
        q.1 - (self.l1 * self.l1 - (q.0 - self.p1_y).powi(2)).sqrt()
    }

    /// Nose 2's pin on its level screw, a link's length right of the frame's.
    fn pusher2(&self, alpha: f64, hang: f64) -> f64 {
        let q = self.pin2(alpha, hang);
        q.0 + (self.l2 * self.l2 - (q.1 - self.p2_z).powi(2)).sqrt()
    }

    fn axis(&self, alpha: f64, hang: f64) -> P2 {
        add(self.hinge2(alpha), rot(hang, (0.0, -self.drop)))
    }

    /// From a row's three numbers to where the cutter's tip is in the stick's section.
    fn pose(&self, lift: f64, swing: f64, turn: f64) -> Pose {
        let (c1, c2) = (self.c1_home - lift, self.c2_home - swing);
        let (mut lo, mut hi) = (-0.6, 0.6);
        for _ in 0..70 {
            let mid = (lo + hi) / 2.0;
            if self.pusher1(mid) > c1 {
                lo = mid
            } else {
                hi = mid
            }
        }
        let alpha = (lo + hi) / 2.0;
        let (mut lo, mut hi) = (-0.7, 0.7);
        for _ in 0..70 {
            let mid = (lo + hi) / 2.0;
            if self.pusher2(alpha, mid) < c2 {
                lo = mid
            } else {
                hi = mid
            }
        }
        let hang = (lo + hi) / 2.0;
        let axis = self.axis(alpha, hang);
        let psi = turn.to_radians() + hang;
        Pose {
            alpha,
            hang,
            axis,
            across: self.cutter_y - axis.0,
            tip: axis.1 - self.bit_up,
            t: (-psi - PI / 2.0).rem_euclid(TAU),
        }
    }
}

struct Row {
    n: u64,
    kind: String,
    tool: String,
    t: f64,
    across: f64,
    tip: f64,
    lift: f64,
    swing: f64,
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
                lift: f(&r["lift"]),
                swing: f(&r["swing"]),
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

/// Each row's three numbers, worked forward through the hinges, put
/// the cutter where the plan says, and the cutter is never in the tile.
#[test]
fn every_row_puts_the_cutter_where_the_plan_wants_it_and_never_in_the_tile() {
    let n = numbers();
    let (sled, rows, tile) = (Sled::new(&n), rows(&n), outline(&n));
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
        let p = sled.pose(r.lift, r.swing, r.turn);
        let miss = (p.across - r.across)
            .abs()
            .max((p.tip - r.tip).abs())
            .max((p.t - r.t).sin().abs() * 30.0);
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
                assert!(r.across.abs() < 1e-9 && (r.tip - master).abs() < 1e-9);
            }
            _ => {}
        }
    }
    println!(
        "{} rows, the worst {worst:.1e} mm from its cut; the finishing bit, flank and shank too, at most {gouge:.1e} mm into the tile",
        rows.len()
    );
    assert!(gouge < 1e-6);
    let count = |k: &str| rows.iter().filter(|r| r.kind == k).count() as u64;
    for k in ["set", "rough", "finish"] {
        assert_eq!(count(k), n["count"][k].as_u64().unwrap());
    }
    // Two settings of the cutter, each on the master's radius: the straight bit before roughing, the ball-nose before the finish.
    let sets: Vec<&Row> = rows.iter().filter(|r| r.kind == "set").collect();
    assert_eq!(sets.len(), 2);
    assert!(sets[0].tool == "straight" && sets[1].tool == "ball");
    // The master is smaller than anything a pass comes near, so a cutter left under it is never touched by it in a cut.
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
/// cutter never further from the axis than the frame swings.
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

/// At every row the blank clears the table, and so does the pulley,
/// which stands lower than the hangers; each link is a link's length
/// from pin to pin and stays near its screw's line; and the axis is
/// where the tile is fattest, the master inside that and the spur
/// inside the master, so a block lying under both touches only the
/// master.
#[test]
fn the_stick_clears_the_table_and_the_links_stay_true_at_every_row() {
    let n = numbers();
    let (sled, rows, tile) = (Sled::new(&n), rows(&n), outline(&n));
    let circle = f(&n["tile"]["circle"]);
    let nearest = tile.iter().map(|q| q.0.hypot(q.1)).fold(f64::MAX, f64::min);
    let (master, spur) = (f(&n["sled"]["master_r"]), f(&n["sled"]["spur_r"]));
    println!(
        "a circle of {circle:.2} fits about the axis; the master is {:.0} round, the spur {:.0}",
        2.0 * master,
        2.0 * spur
    );
    assert!(circle > 9.5 && nearest >= circle - 1e-9 && master <= circle - 1.0);
    assert!(spur <= master - 1.0);
    let stock = blank(&n);
    let (mut table, mut pulley) = (f64::MAX, f64::MAX);
    let (mut lean1, mut lean2) = (0.0f64, 0.0f64);
    for r in &rows {
        let p = sled.pose(r.lift, r.swing, r.turn);
        pulley = pulley.min(p.axis.1 - f(&n["pulley_r"]));
        // Each link: its two pins a link apart, and how far it leans off its screw.
        let (q1, q2) = (sled.pin1(p.alpha), sled.pin2(p.alpha, p.hang));
        let (c1, c2) = (sled.c1_home - r.lift, sled.c2_home - r.swing);
        assert!(((q1.0 - sled.p1_y).hypot(q1.1 - c1) - sled.l1).abs() < 1e-3);
        assert!(((q2.0 - c2).hypot(q2.1 - sled.p2_z) - sled.l2).abs() < 1e-3);
        assert!(
            q1.1 > c1 && q2.0 < c2,
            "the noses are under the plate and right of the frame"
        );
        lean1 = lean1.max(((q1.0 - sled.p1_y) / sled.l1).asin().abs().to_degrees());
        lean2 = lean2.max(((q2.1 - sled.p2_z) / sled.l2).asin().abs().to_degrees());
        if r.kind != "rough" && r.kind != "finish" {
            continue;
        }
        // The blank as it was sawn, whatever has been cut off it since: its lowest corner.
        let reach = stock
            .iter()
            .map(|q| seen(*q, r.t).1)
            .fold(f64::MIN, f64::max);
        table = table.min(p.axis.1 - reach);
    }
    println!(
        "the blank {table:.2} over the table at its lowest, the pulley {pulley:.2}; the links lean at most {lean1:.1} and {lean2:.1} deg"
    );
    assert!(table >= f(&n["table_clear"]) - 0.01 && table < f(&n["table_clear"]) + 1.5);
    assert!(pulley >= 1.0 && f(&n["hanger_r"]) < f(&n["pulley_r"]));
    assert!(lean1.max(lean2) <= f(&n["sled"]["lean_most"]));
    assert!(
        (lean1 - f(&n["sled"]["lean"][0])).abs() < 0.01
            && (lean2 - f(&n["sled"]["lean"][1])).abs() < 0.01
    );
}

/// Between passes the controller goes up to the safe height, unless
/// the table marks the move as one that may go straight: swing, then
/// turn, then lift, from where the last row left it. Every move so
/// marked is followed here along that path: the master, which is over
/// the cutter with the sled back, passes it clear, and the blank
/// passes the table clear at every angle it turns through.
#[test]
fn every_move_marked_straight_passes_the_cutter_and_the_table_clear() {
    let n = numbers();
    let (sled, rows) = (Sled::new(&n), rows(&n));
    let ball = f(&n["bit"]) / 2.0;
    let (taper, shank) = (f(&n["taper"]).to_radians(), f(&n["shank"]) / 2.0);
    let flat = f(&n["rough"]["bit"]) / 2.0;
    let master = f(&n["sled"]["master_r"]);
    let stock = blank(&n);
    let past = f(&n["turn_past"]);
    // The cutter's outline over the table, as points: the ball-nose with its flank and shank, and the straight bit.
    let mut edge_ball: Vec<P2> = Vec::new();
    let zc = sled.bit_up - ball;
    for k in 0..=80 {
        let off = shank * k as f64 / 80.0;
        if let Some(up) = flank(off, ball, taper, shank) {
            edge_ball.push((off, zc - up));
            edge_ball.push((-off, zc - up));
        }
    }
    let low = edge_ball.iter().map(|q| q.1).fold(f64::MAX, f64::min);
    let mut z = 0.0;
    while z < low {
        edge_ball.push((shank, z));
        edge_ball.push((-shank, z));
        z += 0.5;
    }
    let mut edge_flat: Vec<P2> = (-12..=12)
        .map(|k| (flat * k as f64 / 12.0, sled.bit_up))
        .collect();
    let mut z = 0.0;
    while z < sled.bit_up {
        edge_flat.push((flat, z));
        edge_flat.push((-flat, z));
        z += 0.5;
    }
    let reach = |t: f64| stock.iter().map(|q| seen(*q, t).1).fold(f64::MIN, f64::max);
    let t_of = |turn: f64, hang: f64| -(turn.to_radians() + hang) - PI / 2.0;
    let (mut straight, mut least_gap, mut least_table) = (0, f64::MAX, f64::MAX);
    for i in 1..rows.len() {
        let (a, b) = (&rows[i - 1], &rows[i]);
        if !b.straight {
            continue;
        }
        straight += 1;
        assert!(
            a.kind != "set" && b.kind != "set",
            "a setting of the bit is never gone to or left straight"
        );
        let edge = if b.tool == "ball" {
            &edge_ball
        } else {
            &edge_flat
        };
        let mut look = |lift: f64, swing: f64, turns: &[f64]| {
            let p = sled.pose(lift, swing, 0.0);
            let gap = edge
                .iter()
                .map(|q| (p.axis.0 - q.0).hypot(p.axis.1 - q.1))
                .fold(f64::MAX, f64::min)
                - master;
            least_gap = least_gap.min(gap);
            for g in turns {
                least_table = least_table.min(p.axis.1 - reach(t_of(*g, p.hang)));
            }
        };
        // The swing, at the old lift and angle.
        let k = ((b.swing - a.swing).abs() / 0.25).ceil().max(1.0) as usize;
        for j in 0..=k {
            look(
                a.lift,
                a.swing + (b.swing - a.swing) * j as f64 / k as f64,
                &[a.turn],
            );
        }
        // The turn, there: forwards, or back a little past its angle and up to it.
        let ahead = (b.turn - a.turn).rem_euclid(360.0);
        let legs = if ahead <= 180.0 {
            vec![(a.turn, a.turn + ahead)]
        } else {
            vec![
                (a.turn, a.turn + ahead - 360.0 - past),
                (b.turn - past, b.turn),
            ]
        };
        let mut through: Vec<f64> = Vec::new();
        for (g0, g1) in legs {
            let m = ((g1 - g0).abs() / 0.5).ceil().max(1.0) as usize;
            through.extend((0..=m).map(|j| g0 + (g1 - g0) * j as f64 / m as f64));
        }
        look(a.lift, b.swing, &through);
        // The lift, at the new swing and angle.
        let k = ((b.lift - a.lift).abs() / 0.25).ceil().max(1.0) as usize;
        for j in 0..=k {
            look(
                a.lift + (b.lift - a.lift) * j as f64 / k as f64,
                b.swing,
                &[b.turn],
            );
        }
    }
    println!(
        "{straight} of {} moves go straight: the master never nearer the cutter than {least_gap:.2}, the blank never nearer the table than {least_table:.2}",
        rows.len()
    );
    assert_eq!(straight as u64, n["straight"].as_u64().unwrap());
    assert!(
        straight * 2 > rows.len(),
        "most moves need no trip to the safe height"
    );
    assert!(least_gap >= f(&n["move_clear"][0]) - 0.05);
    assert!(least_table >= f(&n["move_clear"][1]) - 0.05);
}

/// The controller's program, run on pins that do nothing, ends every
/// row on the table's numbers to the step, and with the sled back
/// never swings or turns the stick below the height at which the
/// spindle clears the cutter.
#[test]
fn the_controller_ends_every_row_on_the_table_and_crosses_only_at_the_safe_height() {
    let n = numbers();
    let (sled, rows) = (Sled::new(&n), rows(&n));
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    let per = [f(&sim["per"][0]), f(&sim["per"][1]), f(&sim["per"][2])];
    let turn_steps = (360.0 * per[2]).round();
    let done = sim["rows"].as_array().unwrap();
    assert_eq!(done.len(), rows.len());
    for (r, s) in rows.iter().zip(done) {
        assert_eq!(s[0].as_u64().unwrap(), r.n);
        assert!(
            (f(&s[1]) - r.lift * per[0]).abs() <= 0.5 + 1e-6,
            "row {} lift",
            r.n
        );
        assert!(
            (f(&s[2]) - r.swing * per[1]).abs() <= 0.5 + 1e-6,
            "row {} swing",
            r.n
        );
        let off = (f(&s[3]) - r.turn * per[2]).rem_euclid(turn_steps);
        assert!(
            off <= 0.5 + 1e-6 || off >= turn_steps - 0.5 - 1e-6,
            "row {} turn",
            r.n
        );
    }
    // The rows go round the stick in the order of the spindle's angle: it turns little, and comes up to every angle forwards.
    let turned = f(&sim["turned"]) / per[2];
    println!(
        "the spindle turns {turned:.0} deg in all, coming up to its {} angles",
        rows.len()
    );
    assert!(turned < 4.0 * 360.0);
    let safe = f(&n["sled"]["safe"]);
    assert!((f(&sim["safe_steps"]) - (safe * per[0]).round()).abs() < 1e-9);
    assert!(f(&sim["deepest_crossing"]) <= f(&sim["safe_steps"]));
    // Left half way and taken up again, it homes and ends every remaining row on the same steps; its first move is
    // from home, so that one goes by the safe height whatever the table says.
    let again = &sim["resumed"];
    let after = again["after"].as_u64().unwrap();
    assert!(after > 100 && (after as usize) < rows.len() - 100);
    let rest = again["rows"].as_array().unwrap();
    assert_eq!(rest.len(), rows.len() - after as usize);
    for (a, b) in rest.iter().zip(&done[after as usize..]) {
        assert_eq!(a, b);
    }
    assert_eq!(
        again["went_straight"].as_u64().unwrap(),
        rows.iter()
            .filter(|r| r.straight && r.n > after + 1)
            .count() as u64
    );
    assert!(f(&again["deepest_crossing"]) <= f(&sim["safe_steps"]));
    // It went straight exactly where the table said it might.
    assert_eq!(
        sim["went_straight"].as_u64().unwrap(),
        rows.iter().filter(|r| r.straight).count() as u64
    );
    // At the safe lift the master, which is what is over the cutter with the sled back, clears it wherever the frame swings.
    let need = sled.bit_up + f(&n["sled"]["master_r"]) + f(&n["spindle_clear"]);
    let (lo, hi) = rows.iter().fold((f64::MAX, 0.0f64), |a, r| {
        (a.0.min(r.swing), a.1.max(r.swing))
    });
    let mut least = f64::MAX;
    for k in 0..=20 {
        let p = sled.pose(
            safe,
            lo * (1.0 - k as f64 / 20.0) + hi * k as f64 / 20.0,
            0.0,
        );
        least = least.min(p.axis.1);
    }
    println!(
        "safe at lift {safe} mm: the axis no lower than {least:.2}, the master over the cutter needs {need:.2}; \
         the pushers a step is {:.4} and {:.4} mm, the spindle's {:.4} deg",
        1.0 / per[0],
        1.0 / per[1],
        1.0 / per[2]
    );
    assert!(least >= need - 0.3);
    assert!(sled.pose(safe + 4.0, hi, 0.0).axis.1 < need + 4.0);
    // Travel: both pushers start a little short of their home switches and stay on their screws.
    let deep = rows.iter().map(|r| r.lift).fold(0.0, f64::max);
    assert!(rows.iter().all(|r| r.lift >= 2.0 && r.swing >= 1.5));
    // Each screw's nose is a block on a linear rail's carriage: at home and at the furthest any row sends it the
    // carriage is wholly on its rail. The lift's first.
    let l = &n["sled"]["lift"];
    let (z0, z1) = (f(&l["rail_z"][0]), f(&l["rail_z"][1]));
    let (tall, top) = (f(&l["block"][1]), f(&l["block"][3]));
    for lift in [0.0, deep] {
        let mid = sled.c1_home - lift + top - tall / 2.0;
        let c = f(&n["sled"]["swing"]["carriage"][1]);
        assert!(
            mid - c / 2.0 >= z0 + 2.0 && mid + c / 2.0 <= z1 - 2.0,
            "the lift's carriage is off its rail at lift {lift}"
        );
    }
    // The lift screw ends just past its nut with the block at home, and short of the plate's underside, which is
    // 15 over link 1's upper pin, with the plate at its lowest.
    let nut_home = sled.c1_home + top - tall;
    assert!(f(&l["screw_top"]) >= nut_home + 3.0);
    assert!(f(&l["screw_top"]) < sled.c1_home - deep + sled.l1 * 0.95 + 15.0 - 5.0);
    // The swing's: the motor stands past the rail's end, clear of the block at home.
    let w = &n["sled"]["swing"];
    let (rail0, rail1) = (f(&w["rail_y"][0]), f(&w["rail_y"][1]));
    let (carriage, block, short) = (f(&w["carriage"][1]), f(&w["block"][1]), f(&w["block"][3]));
    assert!((rail1 - rail0 - f(&w["rail"][2])).abs() < 1e-9);
    for swing in [0.0, hi] {
        let mid = sled.c2_home - swing - short + block / 2.0;
        assert!(
            mid - carriage / 2.0 >= rail0 + 2.0 && mid + carriage / 2.0 <= rail1 - 2.0,
            "the carriage is off its rail at swing {swing}"
        );
    }
    assert!(f(&w["motor_y"]) >= rail1 && f(&w["motor_y"]) >= sled.c2_home - short + block + 5.0);
    // The pin is half way up the block, on the screw's line.
    assert!(
        (f(&w["rail_z"]) + f(&w["carriage"][2]) + f(&w["block"][2]) / 2.0 - sled.p2_z).abs() < 1e-9
    );
    assert!(1.0 / per[0] < 0.005 && 1.0 / per[2] < 0.05);
}

/// The document: every part regenerates, and it is the sled at one
/// finishing row, the cutter's tip on the stick.
#[test]
fn the_model_is_the_sled_at_one_row_with_the_cutter_on_the_stick() {
    let (mut doc, n) = (load(), numbers());
    let (sled, rows) = (Sled::new(&n), rows(&n));
    let d = &n["drawn"];
    let row = rows
        .iter()
        .find(|r| r.n == d["row"].as_u64().unwrap())
        .unwrap();
    assert_eq!(row.kind, "finish");
    let p = sled.pose(row.lift, row.swing, row.turn);
    let axis = pair(&d["axis"]);
    assert!((p.axis.0 - axis.0).abs() < 1e-3 && (p.axis.1 - axis.1).abs() < 1e-3);
    assert!(
        (p.alpha - f(&d["alpha"])).abs() < 1e-5
            && (p.hang - f(&d["alpha"]) - f(&d["beta"])).abs() < 1e-5
    );
    // The stick: the tile's section the whole of its length, about the drawn axis.
    let area = f(&n["tile"]["area"]);
    let stick = bodies_of(&mut doc, "Stick").remove(0);
    let expect = area * f(&n["stick"]);
    println!(
        "stick {:.0} mm3, the tile's section gives {expect:.0}",
        stick.volume()
    );
    assert!((stick.volume() - expect).abs() < 0.002 * expect);
    let (lo, hi) = stick.bounds().unwrap();
    assert!(lo.x.abs() < 1e-6 && (hi.x - f(&n["stick"])).abs() < 1e-6);
    // The cutter: up through the table to its height, its tip on the stick and not in it.
    let cutter = bodies_of(&mut doc, "Cutter").remove(0);
    let (_, top) = cutter.bounds().unwrap();
    assert!((top.z - sled.bit_up).abs() < 1e-6);
    let bite = overlap(&cutter, &stick);
    println!(
        "cutter into the stick: {bite:.3} mm3; the stick's lowest point {:.2} over the table",
        lo.z
    );
    assert!(bite < 1.0 && lo.z > 0.0 && lo.z <= sled.bit_up + 1e-6);
    // Everything else stays out of everything else.
    let mut all: Vec<(String, Solid)> = Vec::new();
    for t in [
        "Table and fence",
        "Sled",
        "Riser",
        "Hinge 1 shaft",
        "Hinge 1 supports SK20",
        "Hinge 1 blocks SC20UU",
        "Lift plate",
        "Hinge 2 supports SK20",
        "Hinge 2 shaft",
        "Hinge 2 blocks SC20UU",
        "Beam",
        "Head hangers",
        "Tail hanger",
        "Bearings 6201",
        "Spindle and centre",
        "Belt drive",
        "Master",
        "Lift pusher",
        "Links",
        "Lift rail",
        "Lift block",
        "Swing pusher",
        "Swing rail",
        "Swing block",
        "Swing base",
        "Controller",
    ] {
        for (k, b) in bodies_of(&mut doc, t).into_iter().enumerate() {
            all.push((format!("{t} {}", k + 1), b));
        }
    }
    all.push(("Stick".to_string(), stick));
    all.push(("Cutter".to_string(), cutter));
    let boxes: Vec<_> = all.iter().map(|(_, b)| b.bounds().unwrap()).collect();
    let mut checked = 0;
    for i in 0..all.len() {
        for k in i + 1..all.len() {
            let ((a0, a1), (b0, b1)) = (boxes[i], boxes[k]);
            if a0.x >= b1.x
                || b0.x >= a1.x
                || a0.y >= b1.y
                || b0.y >= a1.y
                || a0.z >= b1.z
                || b0.z >= a1.z
            {
                continue;
            }
            checked += 1;
            let v = overlap(&all[i].1, &all[k].1);
            assert!(v < 1.0, "{} is {v:.1} mm3 into {}", all[i].0, all[k].0);
        }
    }
    println!(
        "{} bodies, {checked} pairs near enough to try, none through another",
        all.len()
    );
    assert!(all.len() >= 30);
    // The links as drawn run from the noses' pins to the plate's and the frame's.
    let (q1, q2) = (pair(&d["pin1"]), pair(&d["pin2"]));
    let (w1, w2) = (sled.pin1(p.alpha), sled.pin2(p.alpha, p.hang));
    assert!((q1.0 - w1.0).hypot(q1.1 - w1.1) < 1e-3 && (q2.0 - w2.0).hypot(q2.1 - w2.1) < 1e-3);
    let c1 = f(&d["c1"]);
    assert!(
        (sled.pusher1(p.alpha) - c1).abs() < 1e-3 && (sled.c1_home - row.lift - c1).abs() < 1e-3
    );
    assert!((sled.pusher2(p.alpha, p.hang) - f(&d["c2"])).abs() < 1e-3);
    assert!(!dir().join("out/guide.stl").exists());
    assert!(!dir().join("out/tower.stl").exists());
    // the master is bought, a ground sleeve: a printed one could not be probed with
    assert!(!dir().join("out/master.stl").exists());
    for name in ["lift_block", "swing_block", "swing_base"] {
        assert!(!dir().join("out/steady.stl").exists());
        assert!(dir().join(format!("out/{name}.stl")).exists(), "{name}.stl");
    }
    assert!(dir().join("out/passes.csv").exists() && dir().join("firmware/main.py").exists());
    // The build tries the sled at a couple of hundred poses, the rows and the ways between them, and stops if any part
    // is in another or in the cutter's lane; this is one pose of those, tried again here with the kernel's own solids.
    assert!(n["sweep"]["poses"].as_u64().unwrap() >= 100);
}

/// A sled off the drawing by `v`, in the order calibrate.py fits them.
fn built(d: &Value, v: &[f64]) -> Sled {
    Sled {
        h1: (f(&d["h1"][0]) + v[0], f(&d["h1"][1]) + v[1]),
        arm: (f(&d["arm"][0]) + v[2], f(&d["arm"][1]) + v[3]),
        drop: f(&d["drop"]) + v[4],
        p1_y: f(&d["p1_y"]) + v[5],
        e1: (f(&d["e1"][0]) + v[6], f(&d["e1"][1]) + v[7]),
        l1: f(&d["l1"]) + v[8],
        p2_z: f(&d["p2_z"]) + v[9],
        e2: (f(&d["e2"][0]) + v[10], f(&d["e2"][1]) + v[11]),
        l2: f(&d["l2"]) + v[12],
        c1_home: f(&d["c1_home"]) + v[13],
        c2_home: f(&d["c2_home"]) + v[14],
        cutter_y: v[15],
        bit_up: f(&d["bit_up"]),
    }
}

/// What the probing touches: the pin, and the block lying, on edge and on end.
struct Things<'a> {
    drawn: &'a Value,
    pin_r: f64,
    block: [f64; 3],
    post_face: f64,
    sizes: Option<&'a Value>,
}

impl Things<'_> {
    fn size(&self, key: &str, default: f64) -> f64 {
        self.sizes
            .and_then(|s| s.get(key))
            .map(f)
            .unwrap_or(default)
    }

    /// One of them as the master meets it end on: from, to, and its top over the table.
    fn section(&self, what: &str, v: &[f64]) -> (f64, f64, f64) {
        let b = self.block;
        match what {
            "pin" => {
                let r = self.size("pin_r", self.pin_r);
                (v[15] - r, v[15] + r, f(&self.drawn["bit_up"]) + v[16])
            }
            "flat" => {
                let y = self.size("flat_y", 0.0);
                (y - b[2] / 2.0, y + b[2] / 2.0, self.size("flat_top", b[0]))
            }
            "edge" => {
                let y = f(&self.drawn["edge_y"]) + v[19];
                let half = self.size("edge_half", b[0] / 2.0);
                (y - half, y + half, self.size("edge_top", b[1]))
            }
            "post_far" => {
                let y = -self.post_face + v[20];
                (y - b[0], y, self.size("post_top", b[2]))
            }
            "post_near" => {
                let y = self.post_face + v[21];
                (y, y + b[0], self.size("post_top", b[2]))
            }
            other => panic!("nothing called {other}"),
        }
    }
}

/// The master's middle with the screws at `at`: the axis, and the master's runout turned with the spindle.
fn master(sled: &Sled, v: &[f64], at: (f64, f64, f64)) -> P2 {
    let p = sled.pose(at.0, at.1, at.2);
    let (m, t) = ((v[17], v[18]), p.t);
    (
        p.axis.0 - m.0 * t.sin() + m.1 * t.cos(),
        p.axis.1 - m.0 * t.cos() - m.1 * t.sin(),
    )
}

/// Between the master, `r` in radius, and a thing: nothing at a touch.
fn gap(c: P2, r: f64, thing: (f64, f64, f64)) -> f64 {
    let dy = (thing.0 - c.0).max(c.0 - thing.1).max(0.0);
    let dz = (c.1 - thing.2).max(0.0);
    dy.hypot(dz) - r
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(f).collect()
}

/// The table that finds out what a built sled is, on the sled as drawn:
/// every touch is a touch, its screw has a window's room each side of
/// it, and the master is never brought past a thing nearer than the
/// planner says.
#[test]
fn the_probing_table_touches_what_it_says_and_comes_at_nothing_blind() {
    let n = numbers();
    let d = &n["sled"]["as_drawn"];
    let probe = &n["probe"];
    let things = Things {
        drawn: d,
        pin_r: f(&probe["pin_r"]),
        block: [
            f(&probe["block"][0]),
            f(&probe["block"][1]),
            f(&probe["block"][2]),
        ],
        post_face: f(&probe["post_face"]),
        sizes: None,
    };
    let zero = vec![0.0; 22];
    let sled = built(d, &zero);
    let r = f(&d["master_r"]);
    let (lift_w, swing_w) = (f(&probe["windows"][0]), f(&probe["windows"][1]));
    let (lift_most, swing_most) = (f(&d["lift_most"]), f(&d["swing_most"]));
    let rows = probe["rows"].as_array().unwrap();
    let csv = std::fs::read_to_string(dir().join("out/probe.csv")).unwrap();
    assert_eq!(
        csv.lines()
            .filter(|l| l.as_bytes()[0].is_ascii_digit())
            .count(),
        rows.len()
    );
    let mut count = std::collections::BTreeMap::new();
    let mut last: Option<(f64, f64)> = None; // where the last probe left the screws
    let mut nearest = (f64::MAX, f64::MAX); // the pin, a block
    for row in rows {
        let kind = row["kind"].as_str().unwrap();
        let what = row["tool"].as_str().unwrap();
        let at = (f(&row["lift"]), f(&row["swing"]), f(&row["turn"]));
        match kind {
            "down" => {
                let g = gap(master(&sled, &zero, at), r, things.section(what, &zero));
                assert!(g.abs() < 2e-3, "row {} is {g} from a touch", row["n"]);
                assert!(at.0 - lift_w >= 0.0 && at.0 + lift_w <= lift_most);
                // the master's lowest point is on the thing's top, not off its edge
                let (y0, y1, _) = things.section(what, &zero);
                let c = master(&sled, &zero, at);
                assert!(c.0 > y0 + 2.0 && c.0 < y1 - 2.0, "row {}", row["n"]);
                *count.entry(what.to_string()).or_insert(0) += 1;
                last = None;
            }
            "sw+" | "sw-" | "br+" | "br-" => {
                let way = if kind.ends_with('+') { 1.0 } else { -1.0 };
                let thing = things.section(what, &zero);
                let g = gap(master(&sled, &zero, at), r, thing);
                assert!(g.abs() < 2e-3, "row {} is {g} from a touch", row["n"]);
                // against a side, not a top corner: the axis is under the thing's top
                assert!(master(&sled, &zero, at).1 < thing.2 - 2.0);
                let (start, far) = (at.1 - way * swing_w, at.1 + way * swing_w);
                assert!(start >= 0.5 && start <= swing_most && far >= 0.5 && far <= swing_most);
                assert!(at.0 <= lift_most);
                if kind.starts_with("br") {
                    // the block is not there yet: the master is put where its face will be, and backs off
                    last = Some((at.0, start));
                    continue;
                }
                // the way there: down from the top of the lift at the starting swing, or the swing first and then
                // the lift from where the last probe on this face left the screws
                let via = row["via"].as_str().unwrap();
                let path: Vec<(f64, f64)> = if via == "h" {
                    (0..=(at.0 as usize)).map(|k| (k as f64, start)).collect()
                } else {
                    let from = last.expect("a straight move with nothing before it");
                    (0..=12)
                        .map(|k| (from.0 + (at.0 - from.0) * k as f64 / 12.0, start))
                        .collect()
                };
                let clear = path
                    .iter()
                    .map(|&(lift, swing)| gap(master(&sled, &zero, (lift, swing, at.2)), r, thing))
                    .fold(f64::MAX, f64::min);
                if what == "pin" {
                    nearest.0 = nearest.0.min(clear);
                } else {
                    nearest.1 = nearest.1.min(clear);
                }
                *count.entry(what.to_string()).or_insert(0) += 1;
                last = Some((at.0, start));
            }
            _ => {}
        }
    }
    println!(
        "probing: {count:?}; on its way the master passes the pin no nearer than {:.1} mm, a block than {:.1}",
        nearest.0, nearest.1
    );
    let want = [
        ("edge", 11),
        ("flat", 3),
        ("pin", 18),
        ("post_far", 4),
        ("post_near", 4),
    ];
    assert_eq!(
        count,
        want.iter().map(|&(k, v)| (k.to_string(), v)).collect()
    );
    // The pin is wherever the router is, and a sled a few millimetres out must still pass it; a block was slid up
    // to the master where the sled held it, so it is where the sled thinks it is.
    assert!(nearest.0 >= 6.0 && nearest.1 >= 4.0);
    // A block on end is taller than the lift can carry the master over: it is only ever touched from the side it
    // was brought up on. The block on edge is crossed, at the top of the lift, with room.
    let top = sled.pose(0.0, f(&d["swing_most"]) / 2.0, 0.0).axis.1 - r;
    assert!(top > things.block[1] + 5.0 && top < things.block[2]);
}

/// One sled built wrong, the whole way: the controller's own program
/// probing it, the fit, and the table planned for what was found run
/// on the sled as it really is. All of it worked again here from the
/// recorded touches and tables, by this file's own hinges.
#[test]
fn a_sled_built_wrong_is_found_by_touching_and_the_table_planned_for_it_cuts_true() {
    let c: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/calibration.json")).unwrap())
            .unwrap();
    let (d, e) = (&c["drawn"], &c["example"]);
    let block = [f(&c["block"][0]), f(&c["block"][1]), f(&c["block"][2])];
    let real_things = Things {
        drawn: d,
        pin_r: f(&c["pin_r"]),
        block,
        post_face: f(&c["post_face"]),
        sizes: Some(&e["sizes"]),
    };
    let sold = Things {
        sizes: None,
        ..real_things
    };
    let (truth, fitted) = (floats(&e["true"]), floats(&e["fitted"]));
    let zero = vec![0.0; truth.len()];
    let (real, found, drawn) = (built(d, &truth), built(d, &fitted), built(d, &zero));
    let (r_real, r) = (f(&e["sizes"]["master_r"]), f(&d["master_r"]));
    // It was built wrong: up to half a millimetre in its dimensions, more in its switches.
    let out = truth[..13].iter().fold(0.0f64, |m, x| m.max(x.abs()));
    assert!(out > 0.3 && out <= 0.5 && truth[13].abs().max(truth[14].abs()) > 1.0);

    let touches = e["touches"].as_array().unwrap();
    assert_eq!(touches.len(), 40);
    let (mut sum, mut worst, mut off_drawing) = (0.0, 0.0f64, 0.0f64);
    for t in touches {
        let what = t[2].as_str().unwrap();
        let at = (f(&t[3]), f(&t[4]), f(&t[5]));
        // a touch on the sled as it is: the three taken there, a hundredth of a millimetre of scatter each
        let g = gap(
            master(&real, &truth, at),
            r_real,
            real_things.section(what, &truth),
        );
        assert!(g.abs() < 0.03, "row {} is {g} from the real sled", t[0]);
        assert!(
            f(&t[6]) <= 0.08,
            "row {}'s touches are {} apart",
            t[0],
            t[6]
        );
        // and on the sled fitted, with everything the size it is sold as
        let g = gap(master(&found, &fitted, at), r, sold.section(what, &fitted));
        sum += g * g;
        worst = worst.max(g.abs());
        // the pin and the lying block are where they are, whatever the sled: the drawn sled is nowhere near them
        if what == "pin" || what == "flat" {
            let g = gap(master(&drawn, &zero, at), r, sold.section(what, &zero));
            off_drawing = off_drawing.max(g.abs());
        }
    }
    let rms = (sum / touches.len() as f64).sqrt();
    println!(
        "{} touches: the fitted sled leaves them {:.0} um out, the worst {:.0}; the drawn sled up to {off_drawing:.2} mm",
        touches.len(),
        1000.0 * rms,
        1000.0 * worst
    );
    assert!(rms < 0.012 && worst < 0.03 && off_drawing > 0.5);
    assert!((rms - f(&e["rms"])).abs() < 1e-4);

    // The table for the sled found, run on the real one. A bit is set by bringing its tip up to the master where
    // the table's setting row holds it; then the cuts.
    let table = e["table"].as_array().unwrap();
    let ball = f(&c["ball_r"]);
    assert_eq!(table[0][0].as_str(), Some("set"));
    let at = (f(&table[0][4]), f(&table[0][5]), f(&table[0][6]));
    let m = master(&real, &truth, at);
    let tip_z = m.1 - ((r_real + ball).powi(2) - (truth[15] - m.0).powi(2)).sqrt() + ball;
    assert!((tip_z - found.bit_up).abs() < 1.5);
    let turn = |a: f64| (a + PI).rem_euclid(TAU) - PI;
    let (mut got, mut want) = (Vec::new(), Vec::new());
    for row in &table[1..] {
        let (t, across, tip) = (f(&row[1]), f(&row[2]), f(&row[3]));
        let at = (f(&row[4]), f(&row[5]), f(&row[6]));
        // the table is the found sled's: there, the row is the cut
        let q = found.pose(at.0, at.1, at.2);
        assert!((q.across - across).abs() < 1e-6 && (q.tip - tip).abs() < 1e-6);
        assert!(turn(q.t - t).abs() < 1e-7);
        let p = real.pose(at.0, at.1, at.2);
        got.push((p.across, p.axis.1 - tip_z + ball, p.t));
        want.push((across, tip + ball, t));
    }
    assert!(got.len() > 200);
    // A tile turned about its axis is the same tile: take the common turn out.
    let twist = got
        .iter()
        .zip(&want)
        .map(|(g, w)| turn(g.2 - w.2))
        .sum::<f64>()
        / got.len() as f64;
    let mut error = 0.0f64;
    for (g, w) in got.iter().zip(&want) {
        let u = g.2 - twist;
        let q = (
            -g.0 * u.sin() + g.1 * u.cos(),
            g.0 * u.cos() + g.1 * u.sin(),
        );
        let p = (
            -w.0 * w.2.sin() + w.1 * w.2.cos(),
            w.0 * w.2.cos() + w.1 * w.2.sin(),
        );
        error = error.max((q.0 - p.0).hypot(q.1 - p.1));
    }
    println!(
        "the finishing cuts land within {error:.3} mm of where they were wanted; from the drawing's table they would be {:.2} out",
        f(&e["error_as_built"])
    );
    assert!(error < 0.05 && (error - f(&e["error"])).abs() < 1e-6);
    assert!(f(&e["error_as_built"]) > 1.0);

    // And over three hundred such sleds, as calibrate.py's study recorded it.
    let s = &c["study"];
    assert!(s["trials"].as_u64().unwrap() >= 300);
    assert_eq!(s["stopped"].as_u64(), Some(0));
    assert_eq!(s["touches"].as_u64(), Some(40));
    assert!(f(&s["as_built"][0]) > 1.0);
    assert!(f(&s["fitted"][0]) < 0.03 && f(&s["fitted"][1]) < 0.045 && f(&s["fitted"][2]) < 0.06);
}

/// The bands that show whether the stick runs true: turned with the
/// cutter straight under the axis, where a band's radius is the axis's
/// height over the bit, and with the bit's side level with the axis on
/// the fence's side, where it is the axis's distance from the bit.
#[test]
fn the_bands_are_turned_under_the_axis_and_beside_it() {
    let n = numbers();
    let sled = Sled::new(&n);
    let bit_r = f(&n["rough"]["bit"]) / 2.0;
    let radius = f(&n["bands"]["r"]);
    let rows = n["bands"]["rows"].as_array().unwrap();
    let sim: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/steps.json")).unwrap())
            .unwrap();
    let per = [f(&sim["per"][0]), f(&sim["per"][1]), f(&sim["per"][2])];
    let done = sim["bands"].as_array().unwrap();
    assert_eq!(done.len(), rows.len());
    let (mut under, mut beside, mut rounds) = (Vec::new(), Vec::new(), 0);
    let mut at = (0.0, 0.0);
    for (row, s) in rows.iter().zip(done) {
        match row["kind"].as_str().unwrap() {
            "place" => {
                let p = sled.pose(f(&row["lift"]), f(&row["swing"]), f(&row["turn"]));
                if p.across.abs() < 1e-3 {
                    under.push(p.tip);
                } else {
                    // the bit's tip over the axis's height, its side on the band
                    assert!((p.tip + bit_r).abs() < 1e-3 && p.across > 0.0);
                    beside.push(p.across - bit_r);
                }
                at = (f(&row["lift"]) * per[0], f(&row["swing"]) * per[1]);
                assert!(
                    (f(&s[1]) - at.0).abs() <= 0.5 + 1e-6 && (f(&s[2]) - at.1).abs() <= 0.5 + 1e-6
                );
            }
            "round" => {
                // a whole turn, nothing else moved
                rounds += 1;
                assert!(
                    (f(&s[1]) - at.0).abs() <= 0.5 + 1e-6 && (f(&s[2]) - at.1).abs() <= 0.5 + 1e-6
                );
            }
            _ => {}
        }
    }
    println!("bands under the axis at {under:?}, beside it at {beside:?}; {rounds} turns");
    assert_eq!(rounds, 12);
    for bands in [&under, &beside] {
        assert_eq!(bands.len(), 3);
        assert!(bands
            .windows(2)
            .all(|w| w[0] > w[1] && w[0] - w[1] <= 4.0 + 1e-3));
        assert!((bands[2] - radius).abs() < 1e-3);
    }
    // A full turn of the spindle is a whole number of steps, so a band closes on itself.
    assert!((360.0 * per[2] - (360.0 * per[2]).round()).abs() < 1e-9);
}

/// The blank is a thing a saw and a planer make: a rectangle, whole
/// millimetres each way, with the tile inside it and 2 mm to spare on
/// every side. The table's first row turns the spindle to where it
/// goes on with a wide face level and underneath, and says where the
/// centre is from that face and from the one toward the fence.
#[test]
fn the_blank_is_a_rectangle_about_the_tile_and_goes_on_a_wide_face_down() {
    let n = numbers();
    let (sled, rows, tile, stock) = (Sled::new(&n), rows(&n), outline(&n), blank(&n));
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
    // every point of the tile is inside, at least `over` from every face; and not much more than that from the
    // nearest: it is the least such rectangle, rounded up to the millimetre
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
    let swing = stock.iter().map(|q| q.0.hypot(q.1)).fold(0.0, f64::max);
    assert!((swing - f(&n["blank_swing"])).abs() < 1e-6);

    // The first row: nothing is cut, the stick is as high as it is ever cut, over the cutter's line.
    let load = &rows[0];
    assert_eq!(load.kind, "load");
    assert!(rows.iter().skip(1).all(|r| r.kind != "load"));
    let p = sled.pose(load.lift, load.swing, load.turn);
    assert!(p.across.abs() < 2e-3 && (p.tip - swing).abs() < 2e-3);
    // The blank as it hangs there: across the sled and down toward the router.
    let hung: Vec<P2> = stock.iter().map(|q| seen(*q, p.t)).collect();
    let mut order: Vec<usize> = (0..4).collect();
    order.sort_by(|&i, &k| hung[k].1.partial_cmp(&hung[i].1).unwrap());
    let (a, c) = (hung[order[0]], hung[order[1]]);
    assert!((a.1 - c.1).abs() < 1e-3, "the face underneath is level");
    assert!(
        ((a.0 - c.0).abs() - size.0.max(size.1)).abs() < 1e-3,
        "and it is a wide one"
    );
    let (under, fence) = (f(&b["load"]["under"]), f(&b["load"]["fence"]));
    assert!(
        (a.1 - under).abs() < 1e-3,
        "the centre is that far up from it"
    );
    let toward_fence = hung.iter().map(|q| q.0).fold(f64::MIN, f64::max);
    assert!(
        (toward_fence - fence).abs() < 1e-3,
        "and that far in from the face toward the fence"
    );
    assert!(under > 15.0 && fence > 15.0);
}

/// A stop is clamped to the fence at each end of the sled's travel.
/// Against the back one the cutter is under the master, clear of the
/// hanger and of the stick's end, and the switch on the sled's end is
/// closed; against the forward one the cutter is past the stick's far
/// end.
#[test]
fn the_stops_leave_the_cutter_under_the_master_and_past_the_stick() {
    let n = numbers();
    let s = &n["sled"];
    let (ends, stops, drawn) = (pair(&s["x"]), pair(&s["stops_x"]), f(&s["drawn_x"]));
    // The model has the cutter `drawn` along the sled. Slid until an end meets its stop, the cutter is here along it:
    let back = drawn - (stops.1 - ends.1);
    let forward = drawn + (ends.0 - stops.0);
    let (master, bit_r) = (pair(&s["master_x"]), f(&n["rough"]["bit"]) / 2.0);
    println!(
        "sled back, the cutter is {back:.0} along it, the master from {:.0} to {:.0}; forward, {forward:.0}, the stick {:.0} long; {:.0} of travel",
        master.0,
        master.1,
        f(&n["stick"]),
        forward - back
    );
    assert!((back - f(&s["back_x"])).abs() < 1e-9);
    // the widest bit, whole, under the master: not under the hanger behind it nor the spur and the stick's end before it
    assert!(back - bit_r > master.0 + 5.0 && back + bit_r < master.1 - 2.0);
    assert!((forward - (f(&n["stick"]) + f(&s["run"]))).abs() < 1e-9);
    assert!(forward - bit_r > f(&n["stick"]) + 1.0);
    // The stops are in the right runner's way, on the fence; the switch is on that runner's end, where the back stop
    // meets it.
    let (stop, right) = (&s["stop"], pair(&s["right_y"]));
    let (sw_y, sw) = (pair(&s["switch"]["y"]), &s["switch"]["size"]);
    assert!(sw_y.0 > right.1 - f(&stop[1]) && sw_y.1 < right.1);
    assert!(f(&s["ply"]) + f(&sw[2]) < f(&stop[2]));
    let mut doc = load();
    let blocks = bodies_of(&mut doc, "Stops");
    assert_eq!(blocks.len(), 2);
    let block = f(&stop[0]) * f(&stop[1]) * f(&stop[2]);
    assert!(blocks
        .iter()
        .all(|b| (b.volume() - block).abs() < 1e-3 * block));
    let switch = bodies_of(&mut doc, "Sled-back switch");
    assert_eq!(switch.len(), 1);
    let size = f(&sw[0]) * f(&sw[1]) * f(&sw[2]);
    assert!((switch[0].volume() - size).abs() < 1e-3 * size);
    // Drawn mid-pass, the sled is between its stops and touches neither.
    assert!(stops.0 < ends.0 - 50.0 && stops.1 > ends.1 + 50.0);
}
