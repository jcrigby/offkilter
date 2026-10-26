//! The turntable scanner: frames of an object turning on the measuring
//! field under a camera and a line laser fixed to each other, into a
//! point cloud and a mesh in the field's millimetres.
//!
//! Each frame's marks give the camera's pose over the field (the
//! homography factors into a rotation and a translation once the
//! focal length is known, and the focal length comes from the
//! homographies themselves). The laser line is the green ridge in the
//! picture. Where it lies on the paper it is straight, and each of its
//! pixels is a ray meeting the known plane of the field, so those
//! points are known in three dimensions. With the camera and the laser
//! fixed to each other and the table turning, that line is the same
//! line in the camera's frame in every frame, so it gives the laser's
//! plane only one of its two directions; the other is the turntable's
//! axis, which the camera's centres describe a circle about and which
//! the laser is aimed through. Then each pixel of the line on the
//! object is one ray meeting that plane, and the pose takes the point
//! into the field's frame, which turns with the object. Consecutive
//! profiles stitch into a mesh.

use super::field;
use super::{apply, cross, invert, luma, otsu, put, shrink, H};
use ok_render::{Image, Rgb};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// The least green (green over the larger of red and blue, 0 to
    /// 255) a pixel is ever taken as the laser line at; the frame's own
    /// ridges set a higher bar when the line is strong.
    pub laser_min: f64,
    /// Points this close to the table (field z) are the table.
    pub min_height: f64,
    /// Profiles are resampled to bins this tall for meshing.
    pub bin: f64,
}

impl Default for ScanOptions {
    fn default() -> ScanOptions {
        ScanOptions {
            laser_min: 12.0,
            min_height: 1.5,
            bin: 1.0,
        }
    }
}

/// What one frame gave.
#[derive(Debug, Clone, Serialize)]
pub struct FrameScan {
    pub index: usize,
    /// Marks read and used, and how well they fitted (photo pixels, rms).
    pub marks: usize,
    pub fit: f64,
    /// The focal length this frame's homography alone implies.
    pub focal: Option<f64>,
    /// The camera's centre in field millimetres, and its azimuth about
    /// the turntable's axis in degrees.
    pub camera: [f64; 3],
    pub azimuth: f64,
    pub line_pixels: usize,
    pub on_paper: usize,
    pub points: usize,
    /// Why the frame was left out, if it was.
    pub skipped: Option<String>,
}

/// The scan.
#[derive(Debug, Clone, Serialize)]
pub struct Scan {
    pub frames: Vec<FrameScan>,
    pub frames_used: usize,
    /// The focal length in pixels, the median over the frames.
    pub focal: f64,
    /// The laser's plane in the camera's frame, n·p + d = 0 with n unit.
    pub laser_plane: [f64; 4],
    /// The angle between the laser's plane and the camera's optical
    /// axis, in degrees: the triangulation angle.
    pub laser_angle: f64,
    /// How well the line on the paper fitted the plane (mm, rms) and
    /// how many of its pixels did.
    pub laser_fit: f64,
    pub laser_points: usize,
    /// The turntable's axis in field millimetres, from the circle the
    /// camera's centres describe.
    pub axis: [f64; 2],
    /// The object's centre line in field millimetres, the mean of the
    /// points, which the mesh is built about.
    pub centre: [f64; 2],
    /// How far the laser's line on the table passes from the axis, in
    /// millimetres: the laser is aimed through the axis, and the plane
    /// is taken to hold it.
    pub aim: f64,
    /// The point cloud in field millimetres.
    #[serde(skip)]
    pub points: Vec<[f64; 3]>,
    pub point_count: usize,
    /// For each point, the frame (its index in `frames`) and the pixel
    /// it came from.
    #[serde(skip)]
    pub sources: Vec<(usize, [f64; 2])>,
    /// One profile per frame used, in azimuth order: points resampled
    /// into height bins.
    #[serde(skip)]
    pub profiles: Vec<Vec<[f64; 3]>>,
    #[serde(skip)]
    pub triangles: Vec<[[f64; 3]; 3]>,
    pub triangle_count: usize,
    /// x0, y0, z0, x1, y1, z1 of the points.
    pub bounds: [f64; 6],
    /// The mesh's volume, in cubic millimetres.
    pub volume: f64,
    /// The share of the mesh's profile points filled in from the
    /// frames either side rather than seen.
    pub filled: f64,
    pub warnings: Vec<String>,
    /// The first frame used, with the marks crossed in blue, the line's
    /// pixels on the paper in yellow and on the object in red.
    #[serde(skip)]
    pub picture: Vec<u8>,
}

/// A camera pose: camera = r · world + t.
#[derive(Debug, Clone, Copy)]
struct Pose {
    r: [[f64; 3]; 3],
    t: [f64; 3],
}

impl Pose {
    fn to_camera(self, p: [f64; 3]) -> [f64; 3] {
        let mut out = [self.t[0], self.t[1], self.t[2]];
        for (k, o) in out.iter_mut().enumerate() {
            *o += self.r[k][0] * p[0] + self.r[k][1] * p[1] + self.r[k][2] * p[2];
        }
        out
    }
    fn to_world(self, c: [f64; 3]) -> [f64; 3] {
        let d = [c[0] - self.t[0], c[1] - self.t[1], c[2] - self.t[2]];
        [
            self.r[0][0] * d[0] + self.r[1][0] * d[1] + self.r[2][0] * d[2],
            self.r[0][1] * d[0] + self.r[1][1] * d[1] + self.r[2][1] * d[2],
            self.r[0][2] * d[0] + self.r[1][2] * d[1] + self.r[2][2] * d[2],
        ]
    }
    fn centre(self) -> [f64; 3] {
        self.to_world([0.0; 3])
    }
}

fn norm3(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The columns of the field-to-image homography with the principal
/// point taken out: (gx - cx gz, gy - cy gz, gz) for each.
fn columns(g: &H, cx: f64, cy: f64) -> [[f64; 3]; 3] {
    // Normalised so the perspective terms are of a known size: a
    // homography is only defined up to scale, and an inverted one comes
    // at whatever scale the inversion left it.
    let scale = if g[8].abs() > 1e-12 {
        g[8]
    } else {
        g.iter().map(|v| v * v).sum::<f64>().sqrt()
    };
    let g: Vec<f64> = g.iter().map(|v| v / scale).collect();
    let col = |k: usize| [g[k] - cx * g[6 + k], g[3 + k] - cy * g[6 + k], g[6 + k]];
    [col(0), col(1), col(2)]
}

/// The focal length a field-to-image homography implies, from the
/// first two columns being images of orthonormal directions: two
/// constraints, used where each is well conditioned.
pub fn focal_from(g: &H, cx: f64, cy: f64) -> Option<f64> {
    let [a, b, _] = columns(g, cx, cy);
    let mut estimates = Vec::new();
    let ortho = a[0] * b[0] + a[1] * b[1];
    let zz = a[2] * b[2];
    if zz.abs() > 1e-9 {
        let f2 = -ortho / zz;
        if f2 > 0.0 {
            estimates.push((f2.sqrt(), zz.abs()));
        }
    }
    let dz = a[2] * a[2] - b[2] * b[2];
    if dz.abs() > 1e-9 {
        let f2 = (b[0] * b[0] + b[1] * b[1] - a[0] * a[0] - a[1] * a[1]) / dz;
        if f2 > 0.0 {
            estimates.push((f2.sqrt(), dz.abs()));
        }
    }
    // Weighted by how far from degenerate each constraint is.
    let w: f64 = estimates.iter().map(|e| e.1).sum();
    (w > 0.0).then(|| estimates.iter().map(|e| e.0 * e.1).sum::<f64>() / w)
}

/// The pose from a field-to-image homography at a known focal length,
/// the rotation made orthonormal and the field put in front of the
/// camera.
fn pose_from(g: &H, f: f64, cx: f64, cy: f64) -> Pose {
    let [a, b, c] = columns(g, cx, cy);
    let mut h1 = [a[0] / f, a[1] / f, a[2]];
    let mut h2 = [b[0] / f, b[1] / f, b[2]];
    let mut h3 = [c[0] / f, c[1] / f, c[2]];
    let l1 = dot3(h1, h1).sqrt();
    let l2 = dot3(h2, h2).sqrt();
    let lambda = 2.0 / (l1 + l2);
    for v in [&mut h1, &mut h2, &mut h3] {
        for x in v.iter_mut() {
            *x *= lambda;
        }
    }
    if h3[2] < 0.0 {
        for v in [&mut h1, &mut h2, &mut h3] {
            for x in v.iter_mut() {
                *x = -*x;
            }
        }
    }
    let r1 = norm3(h1);
    let d = dot3(h2, r1);
    let r2 = norm3([h2[0] - d * r1[0], h2[1] - d * r1[1], h2[2] - d * r1[2]]);
    let r3 = cross3(r1, r2);
    // Rows of R are the camera axes in world coordinates; its columns
    // are the world axes in camera coordinates, which r1, r2, r3 are.
    Pose {
        r: [
            [r1[0], r2[0], r3[0]],
            [r1[1], r2[1], r3[1]],
            [r1[2], r2[2], r3[2]],
        ],
        t: h3,
    }
}

/// The laser line's pixels: in each row, every ridge of green (green
/// over the larger of red and blue), located to a fraction of a pixel
/// by its centroid over the half-height run. A ridge counts when it is
/// at least `floor` high and at least three tenths as high as the
/// frame's typical ridge (the median over a first pass), so a line
/// that compression or a dark surface has dimmed still reads, and
/// then only where it continues through neighbouring rows: a line is
/// a line, noise is a speck.
pub fn laser_line(img: &Image, floor: f64) -> Vec<[f64; 2]> {
    let (w, h) = (img.width, img.height);
    let mut green: Vec<f64> = vec![0.0; w * h];
    for v in 0..h {
        for u in 0..w {
            let c = img.pixel(u, v);
            green[v * w + u] = (c.1 as f64 - c.0.max(c.2) as f64).max(0.0);
        }
    }
    // Ridges per row at a threshold: (row, centroid u, peak height).
    let ridges = |min: f64| -> Vec<Vec<(f64, f64)>> {
        (0..h)
            .map(|v| {
                let g = &green[v * w..(v + 1) * w];
                let mut out = Vec::new();
                let mut u = 1;
                while u + 1 < w {
                    if g[u] >= min && g[u] >= g[u - 1] && g[u] >= g[u + 1] {
                        let half = (g[u] / 2.0).max(min / 2.0);
                        let (mut a, mut b) = (u, u);
                        while a > 0 && g[a - 1] >= half {
                            a -= 1;
                        }
                        while b + 1 < w && g[b + 1] >= half {
                            b += 1;
                        }
                        let (mut sum, mut wsum) = (0.0, 0.0);
                        for (k, &gk) in g.iter().enumerate().take(b + 1).skip(a) {
                            sum += (k as f64 + 0.5) * gk;
                            wsum += gk;
                        }
                        out.push((sum / wsum, g[u]));
                        u = b + 2;
                    } else {
                        u += 1;
                    }
                }
                out
            })
            .collect()
    };
    let first = ridges(floor);
    let mut peaks: Vec<f64> = first.iter().flatten().map(|r| r.1).collect();
    if peaks.is_empty() {
        return Vec::new();
    }
    peaks.sort_by(f64::total_cmp);
    let min = floor.max(0.3 * peaks[peaks.len() / 2]);
    let rows = ridges(min);
    // Continuity: a ridge with a ridge within 3 px in the row above or
    // below, in a run of at least four rows.
    let near = |a: &[(f64, f64)], u: f64| a.iter().any(|r| (r.0 - u).abs() <= 3.0);
    let mut keep: Vec<Vec<bool>> = rows
        .iter()
        .enumerate()
        .map(|(v, r)| {
            r.iter()
                .map(|(u, _)| {
                    (v > 0 && near(&rows[v - 1], *u)) || (v + 1 < h && near(&rows[v + 1], *u))
                })
                .collect()
        })
        .collect();
    // Runs: walk down from each kept ridge that has none above it.
    for v in 0..h {
        for i in 0..rows[v].len() {
            if !keep[v][i] || (v > 0 && near(&rows[v - 1], rows[v][i].0)) {
                continue;
            }
            let (mut run, mut u, mut vv) = (1, rows[v][i].0, v);
            while vv + 1 < h {
                match rows[vv + 1].iter().position(|r| (r.0 - u).abs() <= 3.0) {
                    Some(j) => {
                        u = rows[vv + 1][j].0;
                        vv += 1;
                        run += 1;
                    }
                    None => break,
                }
            }
            if run < 4 {
                let (mut u, mut vv) = (rows[v][i].0, v);
                keep[v][i] = false;
                while vv + 1 < h {
                    match rows[vv + 1].iter().position(|r| (r.0 - u).abs() <= 3.0) {
                        Some(j) => {
                            keep[vv + 1][j] = false;
                            u = rows[vv + 1][j].0;
                            vv += 1;
                        }
                        None => break,
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for (v, r) in rows.iter().enumerate() {
        for (i, (u, _)) in r.iter().enumerate() {
            if keep[v][i] {
                out.push([*u, v as f64 + 0.5]);
            }
        }
    }
    out
}

/// The scan of a turn's frames.
pub fn scan(frames: &[Image], opts: &ScanOptions) -> Result<Scan, String> {
    if frames.is_empty() {
        return Err("no frames".into());
    }
    let (w, h) = (frames[0].width as f64, frames[0].height as f64);
    let (cx, cy) = (w / 2.0, h / 2.0);
    struct Read {
        index: usize,
        to_field: H,
        to_image: H,
        marks: usize,
        marks_px: Vec<[f64; 2]>,
        fit: f64,
        focal: Option<f64>,
        line: Vec<[f64; 2]>,
    }
    let mut reads: Vec<Read> = Vec::new();
    let mut report: Vec<FrameScan> = Vec::new();
    let blank = |index: usize| FrameScan {
        index,
        marks: 0,
        fit: 0.0,
        focal: None,
        camera: [0.0; 3],
        azimuth: 0.0,
        line_pixels: 0,
        on_paper: 0,
        points: 0,
        skipped: None,
    };
    for (index, img) in frames.iter().enumerate() {
        if img.width as f64 != w || img.height as f64 != h {
            let mut f = blank(index);
            f.skipped = Some("not the size of the first frame".into());
            report.push(f);
            continue;
        }
        let factor = (img.width.max(img.height) as f64 / 1800.0).ceil().max(1.0) as usize;
        let small = shrink(img, factor);
        let gray = luma(&small);
        let threshold = otsu(&gray);
        let dark: Vec<bool> = gray.iter().map(|&g| (g as u32) < threshold).collect();
        let marks = field::find_marks(&dark, small.width, small.height);
        let mut f = blank(index);
        let fitted = if marks.len() >= 4 {
            field::fit_marks(&marks, factor as f64)
        } else {
            Err(format!("{} marks read", marks.len()))
        };
        match fitted {
            Ok((to_field, kept, rms)) => {
                let to_image = invert(&to_field)?;
                f.marks = kept.len();
                f.fit = rms;
                f.focal = focal_from(&to_image, cx, cy);
                let line = laser_line(img, opts.laser_min);
                f.line_pixels = line.len();
                reads.push(Read {
                    index,
                    to_field,
                    to_image,
                    marks: kept.len(),
                    marks_px: kept.iter().map(|m| m.pixel).collect(),
                    fit: rms,
                    focal: f.focal,
                    line,
                });
            }
            Err(e) => f.skipped = Some(e),
        }
        report.push(f);
    }
    if reads.len() < 3 {
        return Err(format!(
            "only {} of {} frames read the field; the scanner needs three or more, with four marks in two columns and two rows in each",
            reads.len(),
            frames.len()
        ));
    }
    let mut warnings = Vec::new();
    // The focal length: the median of what the frames imply.
    let mut focals: Vec<f64> = reads.iter().filter_map(|r| r.focal).collect();
    if focals.is_empty() {
        return Err("no frame gave a focal length: the camera looked straight down on the field, which leaves the focal length undetermined; tilt it".into());
    }
    focals.sort_by(f64::total_cmp);
    let focal = focals[focals.len() / 2];
    let spread = (focals[focals.len() * 9 / 10] - focals[focals.len() / 10]) / focal;
    if spread > 0.1 {
        warnings.push(format!(
            "the frames disagree on the focal length by {:.0} %; a zoom that changed, or the field not flat",
            spread * 100.0
        ));
    }
    let poses: Vec<Pose> = reads
        .iter()
        .map(|r| pose_from(&r.to_image, focal, cx, cy))
        .collect();
    // The axis: the circle the camera's centres describe.
    let centres: Vec<[f64; 3]> = poses.iter().map(|p| p.centre()).collect();
    let axis = circle_centre(&centres.iter().map(|c| [c[0], c[1]]).collect::<Vec<_>>())
        .ok_or("the camera's centres do not lie on a circle: the frames are not of one turn")?;
    let azimuth = |c: [f64; 3]| {
        (c[1] - axis[1])
            .atan2(c[0] - axis[0])
            .to_degrees()
            .rem_euclid(360.0)
    };
    // The line on the paper: in each frame the straight line most of
    // the laser's pixels lie on in the picture; the rest are the
    // object. Each paper pixel is a point on the field's plane.
    let mut paper_cam: Vec<[f64; 3]> = Vec::new();
    let mut aims: Vec<f64> = Vec::new();
    let mut on_paper: Vec<Vec<bool>> = Vec::new();
    for (k, r) in reads.iter().enumerate() {
        let inliers = straight_line_inliers(&r.line, 1.5);
        let mut field_pts: Vec<[f64; 2]> = Vec::new();
        for (px, &on) in r.line.iter().zip(&inliers) {
            if on {
                let [x, y] = apply(&r.to_field, *px);
                field_pts.push([x, y]);
                paper_cam.push(poses[k].to_camera([x, y, 0.0]));
            }
        }
        if let Some(d) = line_distance(&field_pts, axis) {
            aims.push(d);
        }
        on_paper.push(inliers);
    }
    if paper_cam.len() < 100 {
        return Err(format!(
            "only {} laser pixels lie on the paper over the frames; the line is not green enough, laser_min ({}) is too high, or the line does not cross the field",
            paper_cam.len(),
            opts.laser_min
        ));
    }
    aims.sort_by(f64::total_cmp);
    let aim = aims.get(aims.len() / 2).copied().unwrap_or(f64::NAN);
    if aim > 5.0 {
        warnings.push(format!(
            "the laser's line on the table passes {aim:.0} mm from the turntable's axis; aim it through the centre, since the scan takes its plane to hold the axis"
        ));
    }
    // The laser's plane: through the paper line and the axis, in the
    // camera's frame, the axis sampled through every frame's pose.
    let mut plane_pts = paper_cam.clone();
    let per_frame_axis = (paper_cam.len() / 5 / reads.len()).max(4);
    for pose in &poses {
        for n in 0..per_frame_axis {
            let z = 250.0 * n as f64 / (per_frame_axis - 1).max(1) as f64;
            plane_pts.push(pose.to_camera([axis[0], axis[1], z]));
        }
    }
    let plane = fit_plane(&plane_pts)?;
    let dist = |p: [f64; 3]| (dot3([plane[0], plane[1], plane[2]], p) + plane[3]).abs();
    let paper_count = paper_cam.len();
    let laser_fit =
        (paper_cam.iter().map(|p| dist(*p).powi(2)).sum::<f64>() / paper_count as f64).sqrt();
    let laser_angle = plane[2].abs().asin().to_degrees();
    if laser_angle < 10.0 {
        warnings.push(format!(
            "the laser's plane is only {laser_angle:.0} degrees off the camera's axis: depth will be coarse; put the laser further round"
        ));
    }
    // Triangulate the rest: a ray through the pixel meets the plane.
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut sources: Vec<(usize, [f64; 2])> = Vec::new();
    let mut per_frame: Vec<Vec<[f64; 3]>> = vec![Vec::new(); reads.len()];
    let mut paper_per_frame = vec![0usize; reads.len()];
    let mut kind: Vec<Vec<u8>> = Vec::new(); // 1 paper, 2 object, 0 neither
    for (k, r) in reads.iter().enumerate() {
        let mut kinds = vec![0u8; r.line.len()];
        for (n, px) in r.line.iter().enumerate() {
            if on_paper[k][n] {
                paper_per_frame[k] += 1;
                kinds[n] = 1;
                continue;
            }
            let [u, v] = *px;
            let d = [(u - cx) / focal, (v - cy) / focal, 1.0];
            let nd = dot3([plane[0], plane[1], plane[2]], d);
            if nd.abs() < 1e-9 {
                continue;
            }
            let s = -plane[3] / nd;
            if s <= 0.0 {
                continue;
            }
            let p = poses[k].to_world([d[0] * s, d[1] * s, d[2] * s]);
            if p[2] < opts.min_height || p[2] > 1000.0 {
                continue;
            }
            per_frame[k].push(p);
            kinds[n] = 2;
        }
        // Where the line jumps between surfaces at different depths
        // (from a stem to the rim of the cap over it), the ridge in
        // the rows between is a blend of the two, and its rays meet
        // the plane along a chain strung between the surfaces. Along
        // the line the points step a fraction of a millimetre; a point
        // whose steps to both neighbours are many times that is on
        // such a chain.
        let frame_pts = &per_frame[k];
        let mut steps: Vec<f64> = frame_pts.windows(2).map(|w| dist3(w[0], w[1])).collect();
        steps.sort_by(f64::total_cmp);
        let typical = steps.get(steps.len() / 2).copied().unwrap_or(0.0);
        let far = (5.0 * typical).max(3.0);
        let keep: Vec<bool> = (0..frame_pts.len())
            .map(|i| {
                let before = i > 0 && dist3(frame_pts[i - 1], frame_pts[i]) > far;
                let after = i + 1 < frame_pts.len() && dist3(frame_pts[i], frame_pts[i + 1]) > far;
                !(before && after)
            })
            .collect();
        let mut i = 0;
        let mut kept = Vec::with_capacity(frame_pts.len());
        for (n, px) in r.line.iter().enumerate() {
            if kinds[n] != 2 {
                continue;
            }
            if keep[i] {
                kept.push(frame_pts[i]);
                points.push(frame_pts[i]);
                sources.push((r.index, *px));
            } else {
                kinds[n] = 0;
            }
            i += 1;
        }
        per_frame[k] = kept;
        kind.push(kinds);
    }
    if points.is_empty() {
        return Err("the laser line never left the paper: nothing stood on the field, or the line missed it".into());
    }
    // Strays: a paper pixel the line fit missed triangulates to a point
    // near the table far from the object. Points much further from the
    // axis than the object's bulk go.
    let mut radii: Vec<f64> = points
        .iter()
        .map(|p| ((p[0] - axis[0]).powi(2) + (p[1] - axis[1]).powi(2)).sqrt())
        .collect();
    radii.sort_by(f64::total_cmp);
    let reach = 1.25 * radii[radii.len() * 95 / 100] + 5.0;
    let keep = |p: &[f64; 3]| ((p[0] - axis[0]).powi(2) + (p[1] - axis[1]).powi(2)).sqrt() <= reach;
    let before = points.len();
    let kept: Vec<bool> = points.iter().map(keep).collect();
    let mut it = kept.iter();
    points.retain(|_| *it.next().unwrap());
    let mut it = kept.iter();
    sources.retain(|_| *it.next().unwrap());
    for f in per_frame.iter_mut() {
        f.retain(keep);
    }
    let strays = before - points.len();
    if strays > before / 10 {
        warnings.push(format!(
            "{strays} of {before} points lay far from the object and were dropped: the line on the paper is noisy, so the video may be compressed hard or the exposure not locked"
        ));
    }
    // The report, and the frames in azimuth order.
    let mut order: Vec<usize> = (0..reads.len()).collect();
    order.sort_by(|&a, &b| azimuth(centres[a]).total_cmp(&azimuth(centres[b])));
    for (k, r) in reads.iter().enumerate() {
        let f = &mut report[r.index];
        f.camera = centres[k];
        f.azimuth = azimuth(centres[k]);
        f.on_paper = paper_per_frame[k];
        f.points = per_frame[k].len();
        f.marks = r.marks;
        f.fit = r.fit;
    }
    // The mesh: sectors centred on the frames' azimuths, one per frame
    // that had the line on the object.
    let with_points = per_frame.iter().filter(|f| !f.is_empty()).count();
    let n = points.len() as f64;
    let mean = points
        .iter()
        .fold([0.0; 2], |c, p| [c[0] + p[0] / n, c[1] + p[1] / n]);
    let phase = order
        .iter()
        .find(|&&k| !per_frame[k].is_empty())
        .map(|&k| {
            let (sx, sy) = per_frame[k].iter().fold((0.0, 0.0), |(sx, sy), p| {
                let a = (p[1] - mean[1]).atan2(p[0] - mean[0]);
                (sx + a.cos(), sy + a.sin())
            });
            sy.atan2(sx).to_degrees()
        })
        .unwrap_or(0.0);
    let Mesh {
        centre,
        profiles,
        measured,
        triangles,
        volume,
    } = cloud_mesh(&points, with_points, phase, opts.bin)?;
    let cells = measured.iter().map(|m| m.len()).sum::<usize>().max(1);
    let filled = measured.iter().flatten().filter(|m| !**m).count() as f64 / cells as f64;
    if filled > 0.25 {
        warnings.push(format!(
            "{:.0} % of the mesh is filled in from neighbouring frames rather than seen: the line was lost on much of the object, or faces turned from the camera while the laser lit them",
            100.0 * filled
        ));
    }
    let mut bounds = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for p in &points {
        for a in 0..3 {
            bounds[a] = bounds[a].min(p[a]);
            bounds[3 + a] = bounds[3 + a].max(p[a]);
        }
    }
    // The picture: the first frame used, annotated.
    let first = &reads[0];
    let mut picture = frames[first.index].clone();
    for (n, px) in first.line.iter().enumerate() {
        let [u, v] = *px;
        let colour = match kind[0][n] {
            1 => Rgb(255, 220, 40),
            2 => Rgb(240, 40, 40),
            _ => Rgb(120, 120, 120),
        };
        put(&mut picture, u as isize, v as isize, colour);
        put(&mut picture, u as isize + 1, v as isize, colour);
    }
    for m in &first.marks_px {
        cross(
            &mut picture,
            m[0] as isize,
            m[1] as isize,
            6,
            Rgb(40, 90, 255),
        );
    }
    let triangle_count = triangles.len();
    let point_count = points.len();
    Ok(Scan {
        frames_used: reads.len(),
        frames: report,
        focal,
        laser_plane: plane,
        laser_angle,
        laser_fit,
        laser_points: paper_count,
        axis,
        centre,
        aim,
        points,
        point_count,
        sources,
        profiles,
        triangles,
        triangle_count,
        bounds,
        volume,
        filled,
        warnings,
        picture: ok_render::to_png(&picture),
    })
}

/// The plane through points by least squares: an initial normal from
/// the spread, then the normal's largest component fixed at 1 and the
/// rest solved. Returned as [nx, ny, nz, d] with the normal unit.
fn fit_plane(points: &[[f64; 3]]) -> Result<[f64; 4], String> {
    let n = points.len();
    if n < 3 {
        return Err("too few points for a plane".into());
    }
    let mut c = [0.0; 3];
    for p in points {
        for (ca, pa) in c.iter_mut().zip(p) {
            *ca += pa / n as f64;
        }
    }
    // The normal: the cross product of the two most spread directions,
    // found as the longest offset and the offset least aligned with it.
    let far = points
        .iter()
        .max_by(|a, b| {
            let da = (0..3).map(|k| (a[k] - c[k]).powi(2)).sum::<f64>();
            let db = (0..3).map(|k| (b[k] - c[k]).powi(2)).sum::<f64>();
            da.total_cmp(&db)
        })
        .unwrap();
    let u = norm3([far[0] - c[0], far[1] - c[1], far[2] - c[2]]);
    let side = points
        .iter()
        .max_by(|a, b| {
            let ra = cross3(u, [a[0] - c[0], a[1] - c[1], a[2] - c[2]]);
            let rb = cross3(u, [b[0] - c[0], b[1] - c[1], b[2] - c[2]]);
            dot3(ra, ra).total_cmp(&dot3(rb, rb))
        })
        .unwrap();
    let v = cross3(u, [side[0] - c[0], side[1] - c[1], side[2] - c[2]]);
    if dot3(v, v).sqrt() < 1e-6 {
        return Err(
            "the laser's points lie along a line, not a plane: the axis and the paper line coincide"
                .into(),
        );
    }
    let mut plane = {
        let nrm = norm3(v);
        [nrm[0], nrm[1], nrm[2], -dot3(nrm, c)]
    };
    for _ in 0..2 {
        let big = (0..3)
            .max_by(|&i, &j| plane[i].abs().total_cmp(&plane[j].abs()))
            .unwrap();
        let others: Vec<usize> = (0..3).filter(|&i| i != big).collect();
        // p[big] + a p[o0] + b p[o1] + c = 0
        let mut ata = [[0.0f64; 3]; 3];
        let mut atb = [0.0f64; 3];
        for p in points {
            let row = [p[others[0]], p[others[1]], 1.0];
            let rhs = -p[big];
            for r in 0..3 {
                for cc in 0..3 {
                    ata[r][cc] += row[r] * row[cc];
                }
                atb[r] += row[r] * rhs;
            }
        }
        let Some(sol) = solve3(ata, atb) else {
            break;
        };
        let mut nrm = [0.0; 3];
        nrm[big] = 1.0;
        nrm[others[0]] = sol[0];
        nrm[others[1]] = sol[1];
        let l = dot3(nrm, nrm).sqrt();
        plane = [nrm[0] / l, nrm[1] / l, nrm[2] / l, sol[2] / l];
    }
    Ok(plane)
}

/// Which of the points lie within `tol` of the straight line most of
/// them lie on: pairs sampled, the line with the most points near it,
/// refined by least squares on those.
fn straight_line_inliers(pts: &[[f64; 2]], tol: f64) -> Vec<bool> {
    let n = pts.len();
    if n < 2 {
        return vec![false; n];
    }
    let mut rng: u64 = 0x2545F4914F6CDD1D;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };
    let line_of = |a: [f64; 2], b: [f64; 2]| -> Option<([f64; 2], f64)> {
        let d = [b[0] - a[0], b[1] - a[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        (l > 1e-9).then(|| {
            let nrm = [-d[1] / l, d[0] / l];
            (nrm, -(nrm[0] * a[0] + nrm[1] * a[1]))
        })
    };
    let count = |line: &([f64; 2], f64)| {
        pts.iter()
            .filter(|p| (line.0[0] * p[0] + line.0[1] * p[1] + line.1).abs() < tol)
            .count()
    };
    let mut best: Option<(([f64; 2], f64), usize)> = None;
    for _ in 0..120 {
        if let Some(line) = line_of(pts[next()], pts[next()]) {
            let k = count(&line);
            if best.is_none_or(|(_, bk)| k > bk) {
                best = Some((line, k));
            }
        }
    }
    let Some((mut line, _)) = best else {
        return vec![false; n];
    };
    // Refit on the inliers: the direction from the spread.
    for _ in 0..2 {
        let inl: Vec<[f64; 2]> = pts
            .iter()
            .filter(|p| (line.0[0] * p[0] + line.0[1] * p[1] + line.1).abs() < tol)
            .copied()
            .collect();
        if inl.len() < 2 {
            break;
        }
        let m = inl.len() as f64;
        let (cx, cy) = (
            inl.iter().map(|p| p[0]).sum::<f64>() / m,
            inl.iter().map(|p| p[1]).sum::<f64>() / m,
        );
        let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
        for p in &inl {
            sxx += (p[0] - cx).powi(2);
            syy += (p[1] - cy).powi(2);
            sxy += (p[0] - cx) * (p[1] - cy);
        }
        let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let dir = [angle.cos(), angle.sin()];
        let nrm = [-dir[1], dir[0]];
        line = (nrm, -(nrm[0] * cx + nrm[1] * cy));
    }
    pts.iter()
        .map(|p| (line.0[0] * p[0] + line.0[1] * p[1] + line.1).abs() < tol)
        .collect()
}

/// How far a point lies from the straight line through points, by the
/// spread of the points.
fn line_distance(pts: &[[f64; 2]], q: [f64; 2]) -> Option<f64> {
    if pts.len() < 2 {
        return None;
    }
    let m = pts.len() as f64;
    let (cx, cy) = (
        pts.iter().map(|p| p[0]).sum::<f64>() / m,
        pts.iter().map(|p| p[1]).sum::<f64>() / m,
    );
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in pts {
        sxx += (p[0] - cx).powi(2);
        syy += (p[1] - cy).powi(2);
        sxy += (p[0] - cx) * (p[1] - cy);
    }
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let nrm = [-angle.sin(), angle.cos()];
    Some((nrm[0] * (q[0] - cx) + nrm[1] * (q[1] - cy)).abs())
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let mut m = [
        [a[0][0], a[0][1], a[0][2], b[0]],
        [a[1][0], a[1][1], a[1][2], b[1]],
        [a[2][0], a[2][1], a[2][2], b[2]],
    ];
    for col in 0..3 {
        let pivot = (col..3).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        let pivot_row = m[col];
        for (row, r) in m.iter_mut().enumerate() {
            if row != col {
                let f = r[col] / pivot_row[col];
                for (x, p) in r.iter_mut().zip(pivot_row).skip(col) {
                    *x -= f * p;
                }
            }
        }
    }
    Some([m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]])
}

/// The centre of the circle through points, algebraically.
fn circle_centre(pts: &[[f64; 2]]) -> Option<[f64; 2]> {
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy, mut sxxx, mut syyy, mut sxyy, mut sxxy) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in pts {
        let (x, y) = (p[0], p[1]);
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        sxxx += x * x * x;
        syyy += y * y * y;
        sxyy += x * y * y;
        sxxy += x * x * y;
    }
    let sol = solve3(
        [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]],
        [-(sxxx + sxyy), -(sxxy + syyy), -(sxx + syy)],
    )?;
    Some([-sol[0] / 2.0, -sol[1] / 2.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulate::{rig_at, Rig};

    /// The field-to-image homography of a camera: K [r1 r2 t].
    fn homography_of(cam: &crate::simulate::Camera) -> H {
        let r = cam.r;
        let t = cam.to_camera([0.0, 0.0, 0.0]);
        let m = [
            [r[0][0], r[0][1], t[0]],
            [r[1][0], r[1][1], t[1]],
            [r[2][0], r[2][1], t[2]],
        ];
        let k = [[cam.f, 0.0, cam.cx], [0.0, cam.f, cam.cy], [0.0, 0.0, 1.0]];
        let mut g = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                g[i * 3 + j] = (0..3).map(|x| k[i][x] * m[x][j]).sum();
            }
        }
        g.map(|v| v / g[8])
    }

    #[test]
    fn the_focal_length_and_pose_come_back_from_a_homography() {
        let rig = Rig {
            focal: 1300.0,
            width: 1400,
            height: 1050,
            distance: 650.0,
            elevation: 22.0,
            laser_azimuth: 35.0,
            laser_distance: 650.0,
            laser_elevation: 30.0,
            axis: [210.0, 150.0],
            look_height: 55.0,
        };
        for theta in [0.0, 37.0, 200.0] {
            let (cam, _, _) = rig_at(&rig, theta);
            let g = homography_of(&cam);
            let p = apply(&g, [210.0, 150.0]);
            let q = cam.project([210.0, 150.0, 0.0]).unwrap();
            assert!(
                (p[0] - q[0]).abs() < 1e-6 && (p[1] - q[1]).abs() < 1e-6,
                "{p:?} {q:?}"
            );
            let f = focal_from(&g, cam.cx, cam.cy).expect("a focal length");
            assert!((f - 1300.0).abs() < 1.0, "focal {f} at {theta}");
            let pose = pose_from(&g, f, cam.cx, cam.cy);
            let c = pose.centre();
            for a in 0..3 {
                assert!(
                    (c[a] - cam.pos[a]).abs() < 0.5,
                    "centre {c:?} vs {:?}",
                    cam.pos
                );
            }
        }
    }
}

/// A mesh from a point cloud: profiles round the axis, the triangles
/// between them, and the volume by slices.
#[derive(Debug, Clone)]
pub struct Mesh {
    /// The vertical line the profiles are taken about: the mean of the
    /// points.
    pub centre: [f64; 2],
    /// One profile per sector in azimuth order, a point per height bin
    /// over the range the sectors share.
    pub profiles: Vec<Vec<[f64; 3]>>,
    /// For each profile point, whether it came from points in its own
    /// cell (true) or was filled from the sectors either side.
    pub measured: Vec<Vec<bool>>,
    pub triangles: Vec<[[f64; 3]; 3]>,
    pub volume: f64,
}

/// The surface through a point cloud about the vertical line through
/// the cloud's mean, for an object a horizontal ray from that line
/// leaves once at every height (anything convex, and most things that
/// stand on a turntable; not a torus). The points go into `sectors` of
/// azimuth about it, the first centred at `phase` degrees, and bins
/// `bin` tall; each sector's bins are a
/// profile, a bin the sectors either side disagree with dropped as a
/// stray, gaps bridged, the range cut to the bins a third of the
/// sectors have, and the profiles stitched into quads round the turn
/// with a fan at each end. The volume is by slices, which a hole in
/// the mesh does not reach.
pub fn cloud_mesh(
    points: &[[f64; 3]],
    sectors: usize,
    phase: f64,
    bin: f64,
) -> Result<Mesh, String> {
    if sectors < 3 {
        return Err("fewer than three sectors have points on the object".into());
    }
    if points.is_empty() {
        return Err("no points to mesh".into());
    }
    let n = points.len() as f64;
    let axis = points
        .iter()
        .fold([0.0; 2], |c, p| [c[0] + p[0] / n, c[1] + p[1] / n]);
    let bin_of = |p: &[f64; 3]| (p[2] / bin).floor() as i64;
    let axis_r = |p: &[f64; 3]| ((p[0] - axis[0]).powi(2) + (p[1] - axis[1]).powi(2)).sqrt();
    let width = 360.0 / sectors as f64;
    let sector_of = |p: &[f64; 3]| -> usize {
        let a = (p[1] - axis[1]).atan2(p[0] - axis[0]).to_degrees();
        (((a - phase) / width + 0.5).floor() as i64).rem_euclid(sectors as i64) as usize
    };
    // One point per sector and bin: the mean of the cell's points. A
    // scan's points lie on the laser's lines, one azimuth per frame,
    // and a cell that holds lines at two azimuths (sectors not centred
    // on the frames, or two scans merged) keeps only its fullest line,
    // since the mean of points on two lines lies inside the surface
    // by the chord between them, deepest at a corner.
    let mut cells: Vec<std::collections::BTreeMap<i64, Vec<[f64; 3]>>> =
        vec![Default::default(); sectors];
    for p in points {
        cells[sector_of(p)].entry(bin_of(p)).or_default().push(*p);
    }
    let az_of = |p: &[f64; 3]| (p[1] - axis[1]).atan2(p[0] - axis[0]).to_degrees();
    let means: Vec<std::collections::BTreeMap<i64, [f64; 3]>> = cells
        .iter()
        .map(|bins| {
            bins.iter()
                .map(|(b, pts)| {
                    let mut pts: Vec<(f64, [f64; 3])> =
                        pts.iter().map(|p| (az_of(p), *p)).collect();
                    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
                    // Lines: runs of points within a degree of each other.
                    let mut best: Option<(usize, usize)> = None;
                    let mut start = 0;
                    for i in 1..=pts.len() {
                        if i == pts.len() || pts[i].0 - pts[i - 1].0 > 1.0 {
                            if best.is_none_or(|(a, b)| i - start > b - a) {
                                best = Some((start, i));
                            }
                            start = i;
                        }
                    }
                    let (lo, hi) = best.unwrap();
                    let mut line: Vec<[f64; 3]> = pts[lo..hi].iter().map(|(_, p)| *p).collect();
                    line.sort_by(|a, b| axis_r(a).total_cmp(&axis_r(b)));
                    // A line that runs in and out over more than two
                    // bins' worth of radius within one bin of height,
                    // with its points spread along that run rather
                    // than bunched at one end, is on a face near flat,
                    // the top of a cube or the underside of a cap: the
                    // slice there reaches the outermost point. Else the
                    // point of median radius, which a few points off a
                    // neighbouring face that strayed into the bin by
                    // their height do not move.
                    let (rmin, rmax) = (axis_r(&line[0]), axis_r(&line[line.len() - 1]));
                    let beyond = line
                        .iter()
                        .filter(|p| axis_r(p) > (rmin + rmax) / 2.0)
                        .count();
                    if rmax - rmin > 2.0 * bin && beyond * 3 >= line.len() {
                        return (*b, line[line.len() - 1]);
                    }
                    (*b, line[line.len() / 2])
                })
                .collect()
        })
        .collect();
    // Short gaps in a profile (the line lost for a few rows) are
    // bridged by interpolation between the bins either side. A long
    // gap is a surface the camera could not see, the stem under a cap,
    // and is left for the sectors that saw it to fill.
    let bridge = |bins: &[(i64, [f64; 3])]| -> Vec<(i64, [f64; 3])> {
        let mut out = Vec::new();
        for w in bins.windows(2) {
            let ((b0, p0), (b1, p1)) = (w[0], w[1]);
            out.push((b0, p0));
            if b1 - b0 > 4 {
                continue;
            }
            for g in 1..(b1 - b0) {
                let t = g as f64 / (b1 - b0) as f64;
                out.push((
                    b0 + g,
                    [
                        p0[0] + (p1[0] - p0[0]) * t,
                        p0[1] + (p1[1] - p0[1]) * t,
                        p0[2] + (p1[2] - p0[2]) * t,
                    ],
                ));
            }
        }
        if let Some(last) = bins.last() {
            out.push(*last);
        }
        out
    };
    // A bin one sector has at a radius the sectors round it do not,
    // and its own sector does not a little higher and lower, is a
    // stray that survived the reach test: a false ridge near the
    // object's foot, where the line is smeared, put a few millimetres
    // up and far out. Each sector's bins are held against the median
    // of the bridged profiles of the two sectors either side, where
    // three or more of them reach that height, and against the median
    // of its own profile four to eight bins away on both sides: a bin
    // further than 15 % and 3 mm from both goes. A feature on the
    // object shows in the sectors beside it or runs up its own, so it
    // stays; a corner of a cube is far from the faces either side but
    // is the same corner all the way up.
    let nf = sectors;
    let bridged: Vec<std::collections::HashMap<i64, f64>> = means
        .iter()
        .map(|m| {
            let bins: Vec<(i64, [f64; 3])> = m.iter().map(|(b, p)| (*b, *p)).collect();
            bridge(&bins).iter().map(|(b, p)| (*b, axis_r(p))).collect()
        })
        .collect();
    let mut seen: Vec<std::collections::HashSet<i64>> = vec![Default::default(); nf];
    let raw: Vec<Vec<(i64, [f64; 3])>> = (0..nf)
        .map(|k| {
            let kept: Vec<(i64, [f64; 3])> = means[k]
                .iter()
                .filter(|(b, p)| {
                    let mut near: Vec<f64> = (1..=2)
                        .flat_map(|d| [(k + nf - d) % nf, (k + d) % nf])
                        .filter(|&j| j != k)
                        .filter_map(|j| bridged[j].get(b).copied())
                        .collect();
                    if near.len() < 3 {
                        return true;
                    }
                    near.sort_by(f64::total_cmp);
                    let m = near[near.len() / 2];
                    let r = axis_r(p);
                    if (r - m).abs() <= 0.15 * m + 3.0 {
                        return true;
                    }
                    let mut own: Vec<f64> = (4..=8)
                        .flat_map(|d| [**b - d, **b + d])
                        .filter_map(|bb| bridged[k].get(&bb).copied())
                        .collect();
                    if own.len() < 3 {
                        return false;
                    }
                    own.sort_by(f64::total_cmp);
                    let om = own[own.len() / 2];
                    (r - om).abs() <= 0.15 * om + 3.0
                })
                .map(|(b, p)| (*b, *p))
                .collect();
            seen[k] = kept.iter().map(|(b, _)| *b).collect();
            bridge(&kept)
        })
        .collect();
    if raw.iter().filter(|prof| !prof.is_empty()).count() < 3 {
        return Err("fewer than three sectors have points on the object".into());
    }
    // The height range: the bins a third of the sectors or more have.
    // A sector missing a bin in that range (its line lost there, or on
    // a face the camera could not see) takes the point where its own
    // azimuth crosses the chord between the nearest sectors either
    // side that have it: on a flat face that chord lies in the face,
    // and on a curve it is the same chord the mesh makes anyway.
    let mut count: std::collections::BTreeMap<i64, usize> = Default::default();
    for prof in &raw {
        for (b, _) in prof {
            *count.entry(*b).or_insert(0) += 1;
        }
    }
    let enough = (nf / 3).max(2);
    let in_range: Vec<i64> = count
        .iter()
        .filter(|(_, n)| **n >= enough)
        .map(|(b, _)| *b)
        .collect();
    let (lo, hi) =
        match (in_range.first(), in_range.last()) {
            (Some(&lo), Some(&hi)) if hi > lo => (lo, hi),
            _ => return Err(
                "the profiles share no height range; the object or the laser moved during the turn"
                    .into(),
            ),
        };
    let polar = |p: [f64; 3]| {
        let (dx, dy) = (p[0] - axis[0], p[1] - axis[1]);
        ((dx * dx + dy * dy).sqrt(), dy.atan2(dx))
    };
    let azimuths: Vec<f64> = (0..nf)
        .map(|k| (phase + k as f64 * width).to_radians())
        .collect();
    let own: Vec<std::collections::BTreeMap<i64, [f64; 3]>> = raw
        .iter()
        .map(|prof| prof.iter().copied().collect())
        .collect();
    let measured: Vec<Vec<bool>> = (0..nf)
        .map(|k| (lo..=hi).map(|b| seen[k].contains(&b)).collect())
        .collect();
    let profiles: Vec<Vec<[f64; 3]>> = (0..nf)
        .map(|k| {
            (lo..=hi)
                .map(|b| {
                    if let Some(p) = own[k].get(&b) {
                        return *p;
                    }
                    let find = |step: isize| -> Option<(usize, [f64; 3])> {
                        for d in 1..nf as isize {
                            let j = (k as isize + step * d).rem_euclid(nf as isize) as usize;
                            if let Some(p) = own[j].get(&b) {
                                return Some((d as usize, *p));
                            }
                        }
                        None
                    };
                    let (ca, sa) = (azimuths[k].cos(), azimuths[k].sin());
                    let r = match (find(-1), find(1)) {
                        (Some((da, pa)), Some((db, pb))) => {
                            // Across a short gap, the ray at this
                            // azimuth meets the chord from pa to pb
                            // (on a flat face that chord lies in the
                            // face); across a long one, where a chord
                            // would cut deep inside anything round, the
                            // radii either side interpolated by distance.
                            let (ax, ay) = (pa[0] - axis[0], pa[1] - axis[1]);
                            let (bx, by) = (pb[0] - axis[0], pb[1] - axis[1]);
                            let (ex, ey) = (bx - ax, by - ay);
                            let det = ca * ey - sa * ex;
                            let along = if da + db <= 4 && det.abs() > 1e-9 {
                                let t = (ax * ey - ay * ex) / det;
                                let u = (ax * sa - ay * ca) / det;
                                (t > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
                            } else {
                                None
                            };
                            along.unwrap_or_else(|| {
                                (polar(pa).0 * db as f64 + polar(pb).0 * da as f64)
                                    / (da + db) as f64
                            })
                        }
                        (Some((_, p)), None) | (None, Some((_, p))) => polar(p).0,
                        // No sector saw this height: bridge straight
                        // between this sector's own nearest bins below
                        // and above, the honest cone across what no
                        // camera reached.
                        (None, None) => {
                            let below = own[k]
                                .range(..b)
                                .next_back()
                                .map(|(bb, p)| (*bb, polar(*p).0));
                            let above = own[k]
                                .range(b + 1..)
                                .next()
                                .map(|(bb, p)| (*bb, polar(*p).0));
                            match (below, above) {
                                (Some((b0, r0)), Some((b1, r1))) => {
                                    r0 + (r1 - r0) * (b - b0) as f64 / (b1 - b0) as f64
                                }
                                (Some((_, r)), None) | (None, Some((_, r))) => r,
                                (None, None) => 0.0,
                            }
                        }
                    };
                    let z = (b as f64 + 0.5) * bin;
                    [axis[0] + r * ca, axis[1] + r * sa, z]
                })
                .collect()
        })
        .collect();
    let n_bins = (hi - lo + 1) as usize;
    let mut triangles: Vec<[[f64; 3]; 3]> = Vec::new();
    for a in 0..nf {
        let b = (a + 1) % nf;
        let (pa, pb) = (&profiles[a], &profiles[b]);
        for i in 0..n_bins - 1 {
            let (p, q, p1, q1) = (pa[i], pb[i], pa[i + 1], pb[i + 1]);
            triangles.push([p, q, q1]);
            triangles.push([p, q1, p1]);
        }
    }
    // The caps: fans from the axis to the bottom ring and the top ring.
    let zb = profiles.iter().map(|p| p[0][2]).sum::<f64>() / nf as f64;
    let zt = profiles.iter().map(|p| p[n_bins - 1][2]).sum::<f64>() / nf as f64;
    for a in 0..nf {
        let b = (a + 1) % nf;
        triangles.push([[axis[0], axis[1], zb], profiles[b][0], profiles[a][0]]);
        triangles.push([
            [axis[0], axis[1], zt],
            profiles[a][n_bins - 1],
            profiles[b][n_bins - 1],
        ]);
    }
    // The volume by slices: at each height bin, the polygon through the
    // profiles in azimuth order, integrated by the trapezoid rule over
    // the rings' own mean heights (a ring on a flat rim sits at the
    // rim's height, not the bin's middle), and half a bin of the end
    // rings' area past each end.
    let areas: Vec<f64> = (0..n_bins)
        .map(|i| {
            (0..nf)
                .map(|k| {
                    let (a, b) = (profiles[k][i], profiles[(k + 1) % nf][i]);
                    (a[0] - axis[0]) * (b[1] - axis[1]) - (b[0] - axis[0]) * (a[1] - axis[1])
                })
                .sum::<f64>()
                .abs()
                / 2.0
        })
        .collect();
    let heights: Vec<f64> = (0..n_bins)
        .map(|i| profiles.iter().map(|p| p[i][2]).sum::<f64>() / nf as f64)
        .collect();
    let mut volume = 0.0;
    for i in 0..n_bins - 1 {
        volume += (areas[i] + areas[i + 1]) / 2.0 * (heights[i + 1] - heights[i]).max(0.0);
    }
    volume += areas[0] * (heights[0] - lo as f64 * bin).max(0.0);
    volume += areas[n_bins - 1] * ((hi + 1) as f64 * bin - heights[n_bins - 1]).max(0.0);
    Ok(Mesh {
        centre: axis,
        profiles,
        measured,
        triangles,
        volume,
    })
}

/// The points as an ASCII PLY file.
pub fn ply(points: &[[f64; 3]]) -> String {
    let mut out = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nend_header\n",
        points.len()
    );
    for p in points {
        out.push_str(&format!("{:.3} {:.3} {:.3}\n", p[0], p[1], p[2]));
    }
    out
}

/// The points of an ASCII PLY file (the first three properties of
/// each vertex).
pub fn read_ply(text: &str) -> Result<Vec<[f64; 3]>, String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err("not a PLY file".into());
    }
    let mut count = None;
    for line in lines.by_ref() {
        let line = line.trim();
        if let Some(n) = line.strip_prefix("element vertex ") {
            count = Some(n.trim().parse::<usize>().map_err(|e| e.to_string())?);
        } else if line.starts_with("format") && !line.contains("ascii") {
            return Err("only ASCII PLY is read".into());
        } else if line == "end_header" {
            break;
        }
    }
    let count = count.ok_or("the PLY file has no vertex element")?;
    let mut points = Vec::with_capacity(count);
    for line in lines.take(count) {
        let v: Vec<f64> = line
            .split_whitespace()
            .take(3)
            .map(|s| s.parse::<f64>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        if v.len() < 3 {
            return Err("a vertex line has fewer than three numbers".into());
        }
        points.push([v[0], v[1], v[2]]);
    }
    Ok(points)
}
