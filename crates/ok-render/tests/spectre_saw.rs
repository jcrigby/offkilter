//! The Spectre saw station (examples/spectre-saw) as a regression
//! suite: the kernel's program for the example's settings is ten rips,
//! two through and four pairs of rabbets, on a blank that already has
//! three faces, every turn on the 30° grid, every cassette's floor at
//! least the thinnest, and the section it leaves is the tile less the
//! nicks past the corners and nothing else; the first cassette takes
//! the blank either way round and no other does; each cassette in the
//! document is two halves, each one piece, standing on the plate with
//! its tongue and cross key in the plate's grooves and only its own,
//! holding its stick and offcuts with nothing overlapping, the lid and
//! the wood clear of the blade's path and the halves in it only by the
//! trim at their inner edges, each flange at the clamps' height where
//! the clamps press; the station's bed and plate stay one piece each
//! with the blade's path stopping short of their rear edges, the plate
//! in the bed's pocket on the saw table, the clamps clear of everything
//! and pressing on the flanges; and the last stick is the tile. Every
//! number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid};
use ok_math::{Plane, Vec2};
use ok_model::{Document, RegenResult, TabId};
use ok_sketch::spectre::saw::{program, Kind, Settings};
use ok_sketch::{signed_area, Loop, Profile};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-saw")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_saw.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/saw.json")).unwrap()).unwrap()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no {k}"))
}

fn settings(n: &serde_json::Value) -> Settings {
    let s = &n["program"]["settings"];
    Settings {
        edge: num(s, "edge"),
        gap: num(s, "gap"),
        kerf: num(s, "kerf"),
        margin: num(s, "margin"),
        floor: num(s, "floor"),
        bias: num(s, "bias"),
    }
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn regen(doc: &mut Document, name: &str) -> RegenResult {
    let tab = tab_named(doc, name);
    let r = doc.regenerate_studio(tab, None).unwrap();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
    for b in &r.bodies {
        b.solid
            .validate()
            .unwrap_or_else(|e| panic!("{name} / {}: {e}", b.name));
    }
    r
}

fn body<'a>(r: &'a RegenResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
        .solid
}

fn rot(q: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(q.x * c - q.y * s, q.x * s + q.y * c)
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

/// A section in the station's frame (across the sled, up) extruded
/// along the feed, as the build script makes every body: world y is
/// across, z up, x along the feed.
fn prism(pts: &[Vec2], x0: f64, x1: f64) -> Solid {
    let p = Profile {
        outer: Loop::polygon(pts.to_vec()),
        holes: Vec::new(),
    };
    ok_brep::extrude(&p, &Plane::YZ, x0, x1, 1).unwrap()
}

#[test]
fn the_program_is_ten_rips_that_leave_the_tile_and_nothing_else() {
    let n = numbers();
    let s = settings(&n);
    let p = program(&s).unwrap();
    let stored = &n["program"];
    assert_eq!(p.cuts.len(), 10);
    assert_eq!(stored["cuts"].as_array().unwrap().len(), 10);
    assert!((p.height - num(stored, "height")).abs() < 1e-9);
    let through: Vec<usize> = p
        .cuts
        .iter()
        .filter(|c| c.kind == Kind::Through)
        .map(|c| c.face)
        .collect();
    assert_eq!(through, vec![2, 9]);
    let mut corners: Vec<usize> = p.cuts.iter().filter_map(|c| c.corner).collect();
    corners.dedup();
    assert_eq!(corners, vec![1, 4, 8, 13], "each corner's pair together");
    // The blank: the tile's thickness between faces 5 and 11 and its
    // width to face 6, plus the margin past the notched end.
    let unit = 1.5 * (1.0 + 3f64.sqrt());
    assert!(
        (p.thickness - (unit * s.edge - s.gap)).abs() < 1e-9,
        "{}",
        p.thickness
    );
    let reach = 3f64.sqrt() + 1.5; // vertex 6 along the 60° axis
                                   // Less the erosion: half the gap at the end face, more at the notched
                                   // end's convex corners.
    assert!(
        (p.width - (reach * s.edge + s.margin)).abs() < 2.0 * s.gap,
        "{}",
        p.width
    );
    let xs: Vec<f64> = p.tile.iter().map(|q| q.x).collect();
    assert!((xs.iter().cloned().fold(f64::INFINITY, f64::min) - s.margin).abs() < 1e-9);
    assert!((xs.iter().cloned().fold(0.0, f64::max) - p.width).abs() < 1e-9);
    let ys: Vec<f64> = p.tile.iter().map(|q| q.y).collect();
    assert!(ys.iter().cloned().fold(f64::INFINITY, f64::min).abs() < 1e-9);
    assert!((ys.iter().cloned().fold(0.0, f64::max) - p.thickness).abs() < 1e-9);
    for (i, c) in p.cuts.iter().enumerate() {
        let stored_cut = &stored["cuts"][i];
        let name = format!("cut {}", i + 1);
        println!(
            "{:2}. face {:2} {:?} turn {:3.0} bed {:5.2} offcuts {} thin {:.2}{}",
            i + 1,
            c.face,
            c.kind,
            c.turn,
            c.bed,
            c.offcuts.len(),
            c.thin,
            if c.symmetric { " symmetric" } else { "" }
        );
        assert_eq!(stored_cut["face"].as_u64().unwrap() as usize, c.face);
        assert!((num(stored_cut, "bed") - c.bed).abs() < 1e-9);
        assert!((c.turn / 30.0 - (c.turn / 30.0).round()).abs() < 1e-9);
        assert!(c.bed >= s.floor - 1e-9);
        assert!((c.top - p.height).abs() < 1e-9);
        // A rabbet's corner sits on the blade's near face, the bias under
        // its top, and the stock stands above it; a through cut's stock is
        // wholly under the blade's top.
        match c.corner {
            Some(v) => {
                let corner = rot(p.tile[v], c.turn) + c.shift;
                assert!(
                    corner.x.abs() < 1e-9,
                    "{name}: corner off the blade by {}",
                    corner.x
                );
                assert!((corner.y + s.bias - p.height).abs() < 1e-9, "{}", corner.y);
                let top = c.before.iter().map(|q| q.y).fold(0.0, f64::max);
                assert!(top > p.height, "a rabbet's stock stands above the blade");
                assert_eq!(c.offcuts.len(), if i % 2 == 0 { 0 } else { 1 });
            }
            None => {
                let top = c.before.iter().map(|q| q.y).fold(0.0, f64::max);
                assert!(top <= p.height + 1e-9);
                assert_eq!(c.offcuts.len(), 1);
            }
        }
        // Nothing comes off thinner than a sliver, and nothing is lost.
        assert!(c.thin > 2.0, "{}", c.thin);
        let taken: f64 = c.offcuts.iter().map(|o| signed_area(o)).sum();
        let kerf = signed_area(&c.before) - signed_area(&c.after) - taken;
        assert!(
            kerf > 0.0 && kerf < s.kerf * (p.width + p.thickness) * 2.0,
            "{kerf}"
        );
    }
    assert!(p.cuts[0].symmetric, "the blank is a rectangle");
    assert!(p.cuts[1..].iter().all(|c| !c.symmetric));
    // The section: the tile, less a kerf-wide nick the bias deep at
    // each cut of the two square corners (the 120° corners' overshoot
    // runs under the far face), and nothing left over.
    let nicks = 4.0 * s.kerf * s.bias;
    assert!(p.left < 1e-6, "{}", p.left);
    assert!(
        p.nicked > nicks && p.nicked < 1.25 * nicks,
        "{} vs {nicks}",
        p.nicked
    );
    let last = p.cuts.last().unwrap();
    assert!((signed_area(&last.after) - signed_area(&p.tile) + p.nicked).abs() < 1e-6);
    println!(
        "blank {:.2} x {:.2}, blade {:.2} over the plate, {:.2} mm2 nicked",
        p.width, p.thickness, p.height, p.nicked
    );
    // Short of the corner, the sliver appears.
    let short = program(&Settings { bias: 0.0, ..s }).unwrap();
    assert!(short.nicked < 1e-6 && short.left < 1e-6);
}

/// A box in the station's frame: across, up, along the feed.
fn boxed(x: (f64, f64), y: (f64, f64), z: (f64, f64)) -> Solid {
    prism(
        &[
            Vec2::new(x.0, y.0),
            Vec2::new(x.1, y.0),
            Vec2::new(x.1, y.1),
            Vec2::new(x.0, y.1),
        ],
        z.0,
        z.1,
    )
}

fn pair(v: &serde_json::Value) -> (f64, f64) {
    (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
}

/// The interface the build script lays out, read back from its numbers.
struct Station {
    trim: f64,
    key_w: f64,
    key_h: f64,
    key_clear: f64,
    cross_at: f64,
    cross_w: f64,
    cross_len: f64,
    ledge_h: f64,
    plate_t: f64,
    blade_r: f64,
    end: f64,
    z: (f64, f64),
    opening_z: (f64, f64),
    tongue: [f64; 2],
    pads: [Vec<[f64; 4]>; 2],
    plate: [f64; 4],
    bed: [f64; 4],
}

const SIDES: [&str; 2] = ["left", "right"];

impl Station {
    fn read(n: &serde_json::Value) -> Station {
        let s = &n["station"];
        let pads = |side: &str| {
            s["pads"][side]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let v: Vec<f64> = p
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|q| q.as_f64().unwrap())
                        .collect();
                    [v[0], v[1], v[2], v[3]]
                })
                .collect::<Vec<_>>()
        };
        let four = |k: &str| {
            let v: Vec<f64> = s[k]
                .as_array()
                .unwrap()
                .iter()
                .map(|q| q.as_f64().unwrap())
                .collect();
            [v[0], v[1], v[2], v[3]]
        };
        let opening = four("opening");
        Station {
            trim: num(s, "trim"),
            key_w: num(s, "key_w"),
            key_h: num(s, "key_h"),
            key_clear: num(s, "key_clear"),
            cross_at: num(s, "cross_at"),
            cross_w: num(s, "cross_w"),
            cross_len: num(s, "cross_len"),
            ledge_h: num(s, "ledge_h"),
            plate_t: num(s, "plate_t"),
            blade_r: num(s, "blade_d") / 2.0,
            end: num(s, "end"),
            z: pair(&s["z"]),
            opening_z: (opening[2], opening[3]),
            tongue: [num(&s["tongue"], "left"), num(&s["tongue"], "right")],
            pads: [pads("left"), pads("right")],
            plate: four("plate"),
            bed: four("bed"),
        }
    }

    /// The air of a side's grooves in the plate: the tongue's groove
    /// along the opening, the cross key's at the front, with their
    /// clearance and the 0.2 under the keys.
    fn grooves(&self, side: usize) -> (Solid, Solid) {
        let (t, o) = (self.tongue[side], if side == 0 { -1.0 } else { 1.0 });
        let c = self.key_clear;
        let y = (-self.key_h - 0.2, 0.0);
        let tongue = boxed(
            (t - self.key_w / 2.0 - c, t + self.key_w / 2.0 + c),
            y,
            self.opening_z,
        );
        let e = t + o * self.key_w / 2.0;
        let (a, b) = (e - o * c, e + o * (self.cross_len + c));
        let cross = boxed(
            (a.min(b), a.max(b)),
            y,
            (
                self.z.0 + self.cross_at - c,
                self.z.0 + self.cross_at + self.cross_w + c,
            ),
        );
        (tongue, cross)
    }

    /// The keys' own volume: the tongue along the half, the cross key.
    fn key_volume(&self) -> f64 {
        self.key_w * self.key_h * (self.z.1 - self.z.0) + self.cross_len * self.key_h * self.cross_w
    }

    /// The blade's path over the whole stroke: from far ahead to the
    /// edge of its circle behind the top at the sled's stop, down to
    /// the saw table, the kerf wide across.
    fn blade_path(&self, kerf: f64, height: f64) -> Solid {
        let r = self.blade_r;
        let bottom = -self.plate_t;
        let mut pts = vec![Vec2::new(-1000.0, bottom), Vec2::new(-1000.0, height)];
        let mut y = height;
        while y > bottom {
            let c = (r * r - (y - (height - r)).powi(2)).max(0.0).sqrt();
            pts.push(Vec2::new(self.end + c, y));
            y -= 1.0;
        }
        let c = (r * r - (bottom - (height - r)).powi(2)).max(0.0).sqrt();
        pts.push(Vec2::new(self.end + c, bottom));
        // Anticlockwise, as a profile's outer loop goes.
        pts.reverse();
        assert!(signed_area(&pts) > 0.0);
        let p = Profile {
            outer: Loop::polygon(pts),
            holes: Vec::new(),
        };
        // The front plane: x along the feed, y up, its normal -y across,
        // so heights -kerf to 0 are y = 0 to kerf across the bed.
        ok_brep::extrude(&p, &Plane::XZ, -kerf, 0.0, 1).unwrap()
    }

    /// How far the blade's path runs along the feed at the saw table.
    fn path_end(&self, height: f64) -> f64 {
        let r = self.blade_r;
        self.end
            + (r * r - (-self.plate_t - (height - r)).powi(2))
                .max(0.0)
                .sqrt()
    }
}

fn axis(v: ok_math::Vec3, i: usize) -> f64 {
    [v.x, v.y, v.z][i]
}

#[test]
fn every_cassette_stands_keyed_on_the_plate_with_its_halves_clear_of_the_blade() {
    let mut doc = load();
    let n = numbers();
    let s = settings(&n);
    let p = program(&s).unwrap();
    let st = Station::read(&n);
    let length = num(&n, "length");
    let heights = n["heights"].as_array().unwrap();
    let key = st.key_volume();
    let mut plate_volume = None;
    for (i, c) in p.cuts.iter().enumerate() {
        let name = format!("Cut {}", i + 1);
        let r = regen(&mut doc, &name);
        assert_eq!(r.bodies.len(), 5 + c.offcuts.len(), "{name}");
        let halves = [body(&r, "Base left"), body(&r, "Base right")];
        let lid = body(&r, "Lid");
        let plate = body(&r, "Plate");
        let stick = body(&r, "Stick");
        let offcuts: Vec<&Solid> = (1..=c.offcuts.len())
            .map(|k| body(&r, &format!("Offcut {k}")))
            .collect();
        // The plate is the same in every tab.
        let pv = plate.volume();
        assert!(
            (pv - *plate_volume.get_or_insert(pv)).abs() < 1e-6 * pv,
            "{name}"
        );
        // The wood is the program's sections over the stick's length, on its bed.
        assert!(
            (stick.volume() - signed_area(&c.after) * length).abs() < 1e-6 * stick.volume(),
            "{name}"
        );
        for (o, poly) in offcuts.iter().zip(&c.offcuts) {
            assert!(
                (o.volume() - signed_area(poly) * length).abs() < 1e-6 * o.volume(),
                "{name}"
            );
        }
        let (slo, _) = stick.bounds().unwrap();
        assert!(
            (slo.z - c.bed).abs() < 1e-6,
            "{name}: bed {} vs {}",
            slo.z,
            c.bed
        );
        // Nothing overlaps anything.
        let mut all: Vec<(&str, &Solid)> = vec![
            ("left half", halves[0]),
            ("right half", halves[1]),
            ("lid", lid),
            ("plate", plate),
            ("stick", stick),
        ];
        all.extend(offcuts.iter().map(|o| ("offcut", *o)));
        for (a, (an, asol)) in all.iter().enumerate() {
            for (bn, bsol) in &all[a + 1..] {
                assert!(
                    overlap(asol, bsol) < 1e-6,
                    "{name}: the {an} is in the {bn}"
                );
            }
        }
        for (k, h) in halves.iter().enumerate() {
            let side = SIDES[k];
            // One piece each: the blade only trims them.
            assert_eq!(h.shells().len(), 1, "{name}: the {side} half is in pieces");
            let (lo, hi) = h.bounds().unwrap();
            // Standing on the plate's top on its keys, as tall as the cassette.
            assert!((lo.z + st.key_h).abs() < 1e-6, "{name} {side}: {}", lo.z);
            assert!((hi.z - heights[i].as_f64().unwrap()).abs() < 1e-6);
            assert!((lo.x - st.z.0).abs() < 1e-6 && (hi.x - st.z.1).abs() < 1e-6);
            // Its keys in its own grooves, filling the keys' volume, and
            // nothing in the other side's.
            let (tongue, cross) = st.grooves(k);
            // The two grooves meet at the tongue's outer side; count that
            // corner once.
            let both = ok_brep::boolean(&tongue, &cross, BoolOp::Intersection)
                .map(|b| overlap(h, &b))
                .unwrap_or(0.0);
            let mine = overlap(h, &tongue) + overlap(h, &cross) - both;
            assert!(
                (mine - key).abs() < 1e-4 * key,
                "{name} {side}: keys {mine} vs {key}"
            );
            let (t2, c2) = st.grooves(1 - k);
            assert!(
                overlap(h, &t2) + overlap(h, &c2) < 1e-6,
                "{name}: the {side} half fits the other side"
            );
            // The flange's top at the clamps' height under every pad, and
            // nothing over it there.
            for pad in &st.pads[k] {
                let slab = boxed(
                    (pad[0], pad[1]),
                    (st.ledge_h - 0.05, st.ledge_h),
                    (pad[2], pad[3]),
                );
                let area = (pad[1] - pad[0]) * (pad[3] - pad[2]);
                assert!(
                    (overlap(h, &slab) - area * 0.05).abs() < 1e-3 * area * 0.05,
                    "{name} {side}: no flange under the clamp"
                );
                let over = boxed(
                    (pad[0], pad[1]),
                    (st.ledge_h, st.ledge_h + 40.0),
                    (pad[2], pad[3]),
                );
                assert!(
                    overlap(h, &over) < 1e-6,
                    "{name} {side}: the half stands over the flange"
                );
            }
        }
        // The blade's path: never in the lid, the wood or the plate past
        // its slot, and in the halves only by the trim at their edges.
        let path = st.blade_path(s.kerf, p.height);
        let (plo, phi) = path.bounds().unwrap();
        assert!(
            plo.y.abs() < 1e-9 && (phi.y - s.kerf).abs() < 1e-9,
            "{plo:?} {phi:?}"
        );
        assert!((phi.z - p.height).abs() < 1e-9 && (plo.z + st.plate_t).abs() < 1e-9);
        assert!(
            (path.volume() - s.kerf * (phi.x - plo.x) * (phi.z - plo.z)).abs()
                < 0.2 * path.volume()
        );
        for (what, sol) in [("lid", lid), ("stick", stick), ("plate", plate)] {
            assert!(
                overlap(&path, sol) < 1e-6,
                "{name}: the blade meets the {what}"
            );
        }
        for o in &offcuts {
            assert!(
                overlap(&path, o) < 1e-6,
                "{name}: the blade meets an offcut"
            );
        }
        let z = (st.z.0 - 1.0, st.z.1 + 1.0);
        let y = (-st.plate_t, p.height);
        let (lt, rt) = (st.trim, s.kerf - st.trim);
        assert!(
            overlap(halves[0], &boxed((lt, s.kerf + 1.0), y, z)) < 1e-6,
            "{name}: the left half past its trim"
        );
        assert!(
            overlap(halves[1], &boxed((-1.0, rt), y, z)) < 1e-6,
            "{name}: the right half past its trim"
        );
        let trimmed = [overlap(halves[0], &path), overlap(halves[1], &path)];
        assert!(
            trimmed.iter().all(|&v| v > 0.0),
            "{name}: a half stands clear of the blade: no zero clearance"
        );
        let most = st.trim * p.height * (st.z.1 - st.z.0);
        assert!(
            trimmed.iter().all(|&v| v <= most + 1e-6),
            "{name}: {trimmed:?}"
        );
        println!(
            "{name}: halves trimmed {:.0} and {:.0} mm3, keys {key:.0} mm3 each in their grooves",
            trimmed[0], trimmed[1]
        );
    }
    // The last stick is the tile.
    let r = regen(&mut doc, "Cut 10");
    let stick = body(&r, "Stick");
    let tile = signed_area(&p.tile);
    assert!(
        (stick.volume() - (tile - p.nicked) * length).abs() < 1e-6 * stick.volume(),
        "{} vs {}",
        stick.volume(),
        (tile - p.nicked) * length
    );
    let seq = regen(&mut doc, "Sequence");
    assert_eq!(
        seq.bodies
            .iter()
            .filter(|b| b.name.starts_with("Stick "))
            .count(),
        10
    );
    assert_eq!(
        seq.bodies
            .iter()
            .filter(|b| b.name.starts_with("Blade "))
            .count(),
        10
    );
}

#[test]
fn the_station_holds_the_plate_in_the_bed_and_the_clamps_on_the_flanges() {
    let mut doc = load();
    let n = numbers();
    let s = settings(&n);
    let p = program(&s).unwrap();
    let st = Station::read(&n);
    let r = regen(&mut doc, "Station");
    let bed = body(&r, "Bed");
    let plate = body(&r, "Plate");
    let halves = [body(&r, "Base left"), body(&r, "Base right")];
    let clamps: Vec<(&str, &Solid)> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("Clamp "))
        .map(|b| (b.name.as_str(), &b.solid))
        .collect();
    assert_eq!(clamps.len(), 4);
    // The bed and the plate stay one piece each: the blade's path stops
    // short of their rear edges, by a bridge.
    assert_eq!(bed.shells().len(), 1, "the bed is in pieces");
    assert_eq!(plate.shells().len(), 1, "the plate is in pieces");
    let reach = st.path_end(p.height);
    let plate_bridge = st.plate[3] - reach;
    let bed_bridge = st.bed[3] - reach;
    println!(
        "blade {:.1} over the table; its path ends {reach:.0} along the feed; bridges: plate {plate_bridge:.0}, bed {bed_bridge:.0}",
        p.height + st.plate_t
    );
    assert!(plate_bridge >= 20.0 && bed_bridge >= 20.0);
    // A 10 in saw raises its blade about 79 mm.
    assert!(p.height + st.plate_t < 79.0);
    let path = st.blade_path(s.kerf, p.height);
    assert!(
        overlap(&path, bed) < 1e-6,
        "the blade meets the bed past its slot"
    );
    assert!(
        overlap(&path, plate) < 1e-6,
        "the blade meets the plate past its slot"
    );
    // The plate in the bed's pocket, on the saw table with it.
    assert!(overlap(plate, bed) < 1e-6);
    let (plo, phi) = plate.bounds().unwrap();
    let (blo, bhi) = bed.bounds().unwrap();
    assert!((plo.z - blo.z).abs() < 1e-6 && (plo.z + st.plate_t).abs() < 1e-6);
    assert!(phi.z.abs() < 1e-6 && bhi.z > phi.z);
    // Under the bed all round: the bed reaches past the plate both ways
    // across and along the feed.
    for i in 0..2 {
        assert!(axis(plo, i) > axis(blo, i) && axis(phi, i) < axis(bhi, i));
    }
    // The clamps: clear of everything else and of the blade's path, each
    // pad down on its flange.
    let mut others: Vec<&Solid> = r
        .bodies
        .iter()
        .filter(|b| !b.name.starts_with("Clamp "))
        .map(|b| &b.solid)
        .collect();
    others.push(&path);
    for (name, c) in &clamps {
        for o in &others {
            assert!(overlap(c, o) < 1e-6, "{name} meets something");
        }
    }
    for (k, pads) in st.pads.iter().enumerate() {
        for (j, pad) in pads.iter().enumerate() {
            let name = format!("Clamp {} {}", SIDES[k], j + 1);
            let c = clamps.iter().find(|(n, _)| *n == name).unwrap().1;
            let area = (pad[1] - pad[0]) * (pad[3] - pad[2]);
            let above = boxed(
                (pad[0], pad[1]),
                (st.ledge_h, st.ledge_h + 0.05),
                (pad[2], pad[3]),
            );
            let below = boxed(
                (pad[0], pad[1]),
                (st.ledge_h - 0.05, st.ledge_h),
                (pad[2], pad[3]),
            );
            assert!(
                (overlap(c, &above) - area * 0.05).abs() < 1e-3 * area * 0.05,
                "{name}: its pad is off the flange: {} vs {}",
                overlap(c, &above),
                area * 0.05
            );
            assert!(
                (overlap(halves[k], &below) - area * 0.05).abs() < 1e-3 * area * 0.05,
                "{name}: no flange under it"
            );
        }
    }
}
