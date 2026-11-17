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
//! the model is the sled at one of the rows.

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
            across: -axis.0,
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

fn near(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy)
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
    assert!(rows[0].kind == "set" && rows.last().unwrap().kind == "finish");
    assert!(rows
        .iter()
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
        apart * 10 > last.len() * 8,
        "the passes do not share a dozen stops"
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
    let hull: Vec<P2> = n["rough"]["hull"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect();
    let over = f(&n["rough"]["blank"]);
    let cell = 0.25;
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for q in &hull {
        lo = (lo.0.min(q.0 - over), lo.1.min(q.1 - over));
        hi = (hi.0.max(q.0 + over), hi.1.max(q.1 + over));
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
            if inside(q, &tile) {
                continue;
            }
            let near_hull = (0..hull.len())
                .map(|m| near(q, hull[m], hull[(m + 1) % hull.len()]))
                .fold(f64::MAX, f64::min);
            if inside(q, &hull) || near_hull <= over {
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
/// where the tile is fattest, the master and spur inside that.
#[test]
fn the_stick_clears_the_table_and_the_links_stay_true_at_every_row() {
    let n = numbers();
    let (sled, rows, tile) = (Sled::new(&n), rows(&n), outline(&n));
    let circle = f(&n["tile"]["circle"]);
    let nearest = tile.iter().map(|q| q.0.hypot(q.1)).fold(f64::MAX, f64::min);
    let master = f(&n["sled"]["master_r"]);
    println!(
        "a circle of {circle:.2} fits about the axis; the master and spur are {:.0} round",
        2.0 * master
    );
    assert!(circle > 9.5 && nearest >= circle - 1e-9 && master <= circle - 1.0);
    let blank = f(&n["rough"]["blank"]);
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
        // The blank: the tile's reach toward the router and what the saw left on it.
        let reach = tile
            .iter()
            .map(|q| seen(*q, r.t).1)
            .fold(f64::MIN, f64::max)
            + blank;
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
    assert!(deep < f(&n["sled"]["nose"]) - 25.0 && hi < f(&n["sled"]["nose2"]) - 20.0);
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
        "Tower",
        "Swing pusher",
        "Guide",
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
    for name in ["master", "tower", "guide"] {
        assert!(!dir().join("out/steady.stl").exists());
        assert!(dir().join(format!("out/{name}.stl")).exists(), "{name}.stl");
    }
    assert!(dir().join("out/passes.csv").exists() && dir().join("firmware/main.py").exists());
    // The build tries the sled at a couple of hundred poses, the rows and the ways between them, and stops if any part
    // is in another or in the cutter's lane; this is one pose of those, tried again here with the kernel's own solids.
    assert!(n["sweep"]["poses"].as_u64().unwrap() >= 100);
}
