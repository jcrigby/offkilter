//! The Spectre sticks (examples/spectre-sticks) as a regression suite:
//! the plan the build script asked for has five sticks whose every face
//! is a through rip and whose bumps are two roundover passes on a gable,
//! and the document's fixtures regenerate closed with each stick and
//! offcut sitting in its nest, clear of the base and the lid.

use ok_brep::{boolean, BoolOp};
use ok_model::{Document, TabKind};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-sticks")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_sticks.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/sticks.json")).unwrap()).unwrap()
}

#[test]
fn the_plan_saws_every_face_through_and_rounds_the_bumps_from_their_gables() {
    let n = numbers();
    let plan = &n["plan"];
    let sticks = plan["sticks"].as_array().unwrap();
    assert_eq!(sticks.len(), 5);
    assert!((plan["theta"].as_f64().unwrap() - 56.26).abs() < 0.05);
    assert!((plan["radius"].as_f64().unwrap() - 6.35).abs() < 0.02);
    assert!((plan["ridge"].as_f64().unwrap() - 7.89).abs() < 0.02);
    let bits = plan["bits"].as_str().unwrap();
    assert!(
        bits.contains("roundover") && bits.contains("core box") && !bits.contains("bullnose"),
        "{bits}"
    );
    // the same plan from the kernel, for the shapes the json does not carry
    let s = ok_sketch::spectre::sticks::Settings {
        edge: plan["edge"].as_f64().unwrap(),
        gap: plan["gap"].as_f64().unwrap(),
        kerf: plan["kerf"].as_f64().unwrap(),
        ..ok_sketch::spectre::sticks::Settings::default()
    };
    let p = ok_sketch::spectre::sticks::plan(&s).unwrap();
    assert!((p.height - plan["height"].as_f64().unwrap()).abs() < 1e-6);
    for (st, js) in p.sticks.iter().zip(sticks) {
        // a cut per face but the datum, all through: the blade at one height clears the stock
        assert_eq!(st.saw.len(), st.section.len() - 1);
        assert_eq!(st.saw.len(), js["saw"].as_array().unwrap().len());
        for c in &st.saw {
            let top = c.before.iter().map(|q| q.y).fold(0.0, f64::max);
            assert!(
                top <= p.height + 1e-9,
                "the blade clears cut {} of prism {}",
                c.face,
                st.prism
            );
            assert!(c.offcuts.len() <= 1);
        }
        // every bump two passes, the second with the stick end for end; every chord one
        let bumps = st
            .faces
            .iter()
            .filter(|f| f.kind == ok_sketch::spectre::sticks::SawnKind::Gable)
            .count()
            / 2;
        let chords = st
            .faces
            .iter()
            .filter(|f| f.kind == ok_sketch::spectre::sticks::SawnKind::Chord)
            .count();
        let (mut b, mut f) = (0, 0);
        for q in &st.router {
            match q.kind {
                ok_sketch::spectre::sticks::PassKind::Bump => {
                    b += 1;
                    assert_eq!(q.mirrored, q.side == Some(1));
                    assert!(
                        (q.height - p.bump_height).abs() < 1e-9,
                        "one roundover height"
                    );
                    assert!(q.height > p.radius, "the flat clears the table");
                }
                ok_sketch::spectre::sticks::PassKind::Flute => {
                    f += 1;
                    assert!((q.height - p.flute_depth).abs() < 1e-9);
                }
            }
        }
        assert_eq!((b, f), (2 * bumps, chords), "prism {}", st.prism);
        // and the stick comes out as the prism, the flutes the gap deeper
        let target = ok_sketch::signed_area(&st.target);
        assert!(
            st.off < 0.03 * target,
            "prism {}: {} off {target}",
            st.prism,
            st.off
        );
        assert!((st.off - js["off"].as_f64().unwrap()).abs() < 1e-6);
    }
}

/// Every fixture regenerates closed, and in each the stick and its
/// offcut sit in the nest without meeting the base or the lid.
#[test]
fn the_fixtures_hold_their_sticks_clear() {
    let mut doc = load();
    let tabs: Vec<_> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string()))
        .collect();
    assert_eq!(tabs.len(), 6, "a sequence and five sticks");
    let mut checked = 0;
    for (tab, name) in tabs {
        let r = doc.regenerate_studio(tab, None).unwrap();
        let errors: Vec<_> = r.errors().collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        for b in &r.bodies {
            b.solid
                .validate()
                .unwrap_or_else(|e| panic!("{name} {}: {e}", b.name));
        }
        if !name.starts_with("Stick") {
            assert!(r.bodies.len() >= 5 * 4, "{name}: {} stages", r.bodies.len());
            continue;
        }
        if let TabKind::PartStudio(ps) = &doc.tab(tab).unwrap().kind {
            assert!(!ps.features().is_empty());
        }
        // group the bodies by fixture: "Saw 1 base", "Saw 1 lid", "Saw 1 stick", "Saw 1 offcut 1", "Pass 2 hold-down" ...
        let label = |s: &str| s.split(' ').take(2).collect::<Vec<_>>().join(" ");
        let mut labels: Vec<String> = r.bodies.iter().map(|b| label(&b.name)).collect();
        labels.sort();
        labels.dedup();
        for l in &labels {
            let of = |suffix: &str| {
                r.bodies
                    .iter()
                    .filter(|b| b.name == format!("{l} {suffix}"))
                    .collect::<Vec<_>>()
            };
            let holders: Vec<_> = ["base", "lid", "hold-down"]
                .iter()
                .flat_map(|s| of(s))
                .collect();
            assert!(!holders.is_empty(), "{name} {l} has a fixture");
            let stick = of("stick");
            assert_eq!(stick.len(), 1, "{name} {l}");
            let loose: Vec<_> = r
                .bodies
                .iter()
                .filter(|b| b.name.starts_with(&format!("{l} offcut")))
                .collect();
            for held in stick.iter().chain(&loose) {
                for h in &holders {
                    let meet = boolean(&held.solid, &h.solid, BoolOp::Intersection).unwrap();
                    assert!(
                        meet.volume() < 1e-3,
                        "{name}: {} meets {} by {} mm3",
                        held.name,
                        h.name,
                        meet.volume()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 60, "{checked} held bodies checked");
}
