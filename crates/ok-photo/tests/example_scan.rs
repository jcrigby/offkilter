//! The committed record of the example's scan through a real video
//! file (`examples/turntable-scan/out/scan.json`, written by its
//! `build.py` with ffmpeg), checked against the gourd it was rendered
//! from.

use ok_photo::simulate::Gourd;

#[test]
fn the_example_scan_through_a_video_reads_the_gourd() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/turntable-scan/out/scan.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let s: serde_json::Value = serde_json::from_str(&text).unwrap();
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
