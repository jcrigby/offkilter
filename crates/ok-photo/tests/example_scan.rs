//! The committed records of the example's scans through real video
//! files (`examples/turntable-scan/out/`, written by its `build.py`
//! with ffmpeg): the gourd, the mushroom standing and merged with
//! itself on its side, and the bucky ball, each checked against what
//! it was rendered from.

use ok_photo::simulate::{mesh_volume, Gourd, Mushroom, Solid};

fn read(rel: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/turntable-scan/out")
        .join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_example_scan_through_a_video_reads_the_gourd() {
    let s = read("scan.json");
    let num = |k: &str| s[k].as_f64().unwrap_or_else(|| panic!("no {k}"));
    let gourd = Gourd {
        base: [210.0, 150.0],
        height: 120.0,
        lobes: 5,
        lobe: 0.06,
    };
    assert!(num("frames_used") >= 30.0, "{}", num("frames_used"));
    assert!(
        (num("focal") - 1300.0).abs() / 1300.0 < 0.02,
        "focal {}",
        num("focal")
    );
    let axis = s["axis"].as_array().unwrap();
    assert!(
        (axis[0].as_f64().unwrap() - 210.0).abs() < 1.5
            && (axis[1].as_f64().unwrap() - 150.0).abs() < 1.5
    );
    assert!(num("aim") < 3.0, "aim {}", num("aim"));
    assert!(num("laser_angle") > 25.0 && num("laser_angle") < 40.0);
    assert!(num("point_count") > 5000.0);
    assert!(
        (num("volume") - gourd.volume()).abs() / gourd.volume() < 0.04,
        "volume {}",
        num("volume")
    );
    assert!(
        s["warnings"].as_array().unwrap().is_empty(),
        "{}",
        s["warnings"]
    );
}

#[test]
fn the_example_merge_of_the_mushroom_on_its_side_reads_under_the_cap() {
    let truth = Mushroom {
        base: [210.0, 150.0],
        height: 100.0,
        stem: 18.0,
        cap: 48.0,
    }
    .volume();
    // One scan bridges the stem to the cap's rim with a cone.
    let one = read("mushroom/scan.json")["volume"].as_f64().unwrap();
    assert!(
        one > 1.06 * truth,
        "one scan volume {one} of {truth}: the cap's underside was seen"
    );
    let s = read("merged/merged.json");
    let regs = s["registrations"].as_array().unwrap();
    assert_eq!(regs.len(), 1);
    let r = &regs[0];
    assert_eq!(r["placement"], "OnSide");
    assert!(
        r["residual"].as_f64().unwrap() < 0.6,
        "residual {}",
        r["residual"]
    );
    assert!(
        r["overlap"].as_f64().unwrap() > 0.5,
        "overlap {}",
        r["overlap"]
    );
    let volume = s["volume"].as_f64().unwrap();
    assert!(
        (volume - truth).abs() / truth < 0.04,
        "merged volume {volume} vs {truth}"
    );
    assert!(
        s["warnings"].as_array().unwrap().is_empty(),
        "{}",
        s["warnings"]
    );
}

#[test]
fn the_example_scan_of_the_bucky_ball_reads_it() {
    let s = read("bucky/scan.json");
    let truth = mesh_volume(&Solid::Bucky.standing(100.0, [210.0, 150.0], 13.0));
    let volume = s["volume"].as_f64().unwrap();
    assert!(s["frames_used"].as_f64().unwrap() >= 20.0);
    assert!(
        volume > 0.92 * truth && volume < 1.02 * truth,
        "bucky volume {volume} vs {truth}"
    );
    assert!(
        s["warnings"].as_array().unwrap().is_empty(),
        "{}",
        s["warnings"]
    );
}
