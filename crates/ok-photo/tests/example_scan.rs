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

#[test]
fn the_example_bucky_ball_fits_thirty_two_faces() {
    // Through the video (a 1080p phone's worth of colour resolution)
    // most faces are found; from the frames straight (a 4K phone's),
    // all of them, and they close into the solid.
    let v = read("bucky/faces.json");
    let found = v["planes"].as_array().unwrap().len();
    assert!(found >= 20, "video: {found} planes");
    let f = read("bucky-frames/faces.json");
    let planes = f["planes"].as_array().unwrap();
    assert_eq!(planes.len(), 32, "planes");
    let p = &f["polyhedron"];
    assert!(!p.is_null(), "the planes did not close: {}", f["warnings"]);
    assert_eq!(p["faces"].as_array().unwrap().len(), 32);
    assert_eq!(p["vertices"].as_array().unwrap().len(), 60);
    let edges = p["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 90);
    // Every edge the same length, every dihedral one of the two the
    // truncated icosahedron has.
    let truth = Solid::Bucky.standing(100.0, [210.0, 150.0], 13.0);
    let mut corners: Vec<[f64; 3]> = Vec::new();
    for q in truth.iter().flatten() {
        if !corners
            .iter()
            .any(|c| (c[0] - q[0]).abs() + (c[1] - q[1]).abs() + (c[2] - q[2]).abs() < 1e-6)
        {
            corners.push(*q);
        }
    }
    let edge = corners
        .iter()
        .enumerate()
        .flat_map(|(i, a)| {
            corners.iter().skip(i + 1).map(move |b| {
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            })
        })
        .fold(f64::MAX, f64::min);
    for e in edges {
        let l = e["length"].as_f64().unwrap();
        let d = e["dihedral"].as_f64().unwrap();
        assert!((l - edge).abs() < 1.0, "edge {l} vs {edge}");
        assert!(
            (d - 138.19).abs() < 1.0 || (d - 142.62).abs() < 1.0,
            "dihedral {d}"
        );
    }
    let volume = p["volume"].as_f64().unwrap();
    let true_volume = mesh_volume(&truth);
    assert!(
        (volume - true_volume).abs() / true_volume < 0.01,
        "volume {volume} vs {true_volume}"
    );
}
