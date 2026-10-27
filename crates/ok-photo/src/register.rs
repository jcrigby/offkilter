//! Registering scans of one object taken in different poses: the
//! object upside down, on its side, or set down again elsewhere on
//! the field, or the same pose under a different camera and laser. A
//! turntable scan sees nothing under an overhang, so a second pose
//! sees what the first could not, and the two clouds go into one
//! mesh once the rigid motion between them is known. The motion is
//! found by iterative closest point, seeded by what was done to the
//! object (a flip, a roll, a move), since a scan of a cube from every
//! side looks the same from every side and a blind search would be
//! as likely to find the wrong one.

/// A rigid motion, p' = r · p + t.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Transform {
    pub r: [[f64; 3]; 3],
    pub t: [f64; 3],
}

impl Transform {
    pub fn identity() -> Transform {
        Transform {
            r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: [0.0; 3],
        }
    }

    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        [
            self.r[0][0] * p[0] + self.r[0][1] * p[1] + self.r[0][2] * p[2] + self.t[0],
            self.r[1][0] * p[0] + self.r[1][1] * p[1] + self.r[1][2] * p[2] + self.t[1],
            self.r[2][0] * p[0] + self.r[2][1] * p[1] + self.r[2][2] * p[2] + self.t[2],
        ]
    }

    /// `self` after `other`: (self ∘ other)(p) = self(other(p)).
    pub fn then_after(&self, other: &Transform) -> Transform {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.r[i][k] * other.r[k][j]).sum();
            }
        }
        let t = self.apply(other.t);
        Transform { r, t }
    }

    /// The angle this rotation turns through, in degrees.
    pub fn angle(&self) -> f64 {
        let tr = self.r[0][0] + self.r[1][1] + self.r[2][2];
        ((tr - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
    }
}

/// The rotation of `degrees` about the unit `axis`.
fn rotation(axis: [f64; 3], degrees: f64) -> [[f64; 3]; 3] {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let a = [axis[0] / l, axis[1] / l, axis[2] / l];
    let (s, c) = degrees.to_radians().sin_cos();
    let t = 1.0 - c;
    [
        [
            c + a[0] * a[0] * t,
            a[0] * a[1] * t - a[2] * s,
            a[0] * a[2] * t + a[1] * s,
        ],
        [
            a[1] * a[0] * t + a[2] * s,
            c + a[1] * a[1] * t,
            a[1] * a[2] * t - a[0] * s,
        ],
        [
            a[2] * a[0] * t - a[1] * s,
            a[2] * a[1] * t + a[0] * s,
            c + a[2] * a[2] * t,
        ],
    ]
}

fn mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// What was done to the object between the scan being registered and
/// the one it is registered to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Placement {
    /// Not touched: only the camera or the laser moved.
    Same,
    /// Set down again the same way up, anywhere, turned any amount.
    Moved,
    /// Turned over, then set down anywhere, turned any amount.
    UpsideDown,
    /// Rolled a quarter turn onto a side, then set down anywhere.
    OnSide,
}

impl Placement {
    pub fn parse(s: &str) -> Result<Placement, String> {
        let k = s.trim().to_lowercase().replace(['-', '_'], " ");
        match k.as_str() {
            "same" | "" => Ok(Placement::Same),
            "moved" => Ok(Placement::Moved),
            "upside down" | "flipped" | "inverted" => Ok(Placement::UpsideDown),
            "on side" | "on its side" | "side" | "rolled" => Ok(Placement::OnSide),
            _ => Err(format!(
                "placed `{s}` is not one of same, moved, upside down, on its side"
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Placement::Same => "same",
            Placement::Moved => "moved",
            Placement::UpsideDown => "upside down",
            Placement::OnSide => "on its side",
        }
    }
}

/// The result of a registration.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Registration {
    pub placement: Placement,
    /// Takes the registered cloud's points into the reference's frame.
    pub transform: Transform,
    /// The median distance from the registered cloud's points to the
    /// reference's surface, over the points that found a neighbour
    /// within reach, in mm.
    pub residual: f64,
    /// The share of the registered cloud's points within 2 mm of the
    /// reference's surface: the two scans' overlap.
    pub overlap: f64,
    pub seeds: usize,
}

/// A k-d tree over points for nearest neighbours.
pub(crate) struct Tree {
    pub(crate) pts: Vec<[f64; 3]>,
    /// Node k splits `idx[lo..hi]` at its middle on axis `depth % 3`.
    idx: Vec<usize>,
}

impl Tree {
    pub(crate) fn new(pts: &[[f64; 3]]) -> Tree {
        let mut idx: Vec<usize> = (0..pts.len()).collect();
        fn build(pts: &[[f64; 3]], idx: &mut [usize], depth: usize) {
            if idx.len() <= 1 {
                return;
            }
            let a = depth % 3;
            let mid = idx.len() / 2;
            idx.select_nth_unstable_by(mid, |&i, &j| pts[i][a].total_cmp(&pts[j][a]));
            let (l, r) = idx.split_at_mut(mid);
            build(pts, l, depth + 1);
            build(pts, &mut r[1..], depth + 1);
        }
        build(pts, &mut idx, 0);
        Tree {
            pts: pts.to_vec(),
            idx,
        }
    }

    /// The nearest point within `max` of `q`, as (distance, index).
    pub(crate) fn nearest(&self, q: [f64; 3], max: f64) -> Option<(f64, usize)> {
        let mut best: Option<(f64, usize)> = None;
        let mut best_d2 = max * max;
        fn search(
            t: &Tree,
            lo: usize,
            hi: usize,
            depth: usize,
            q: [f64; 3],
            best: &mut Option<(f64, usize)>,
            best_d2: &mut f64,
        ) {
            if lo >= hi {
                return;
            }
            let a = depth % 3;
            let mid = lo + (hi - lo) / 2;
            let i = t.idx[mid];
            let p = t.pts[i];
            let d2 = (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>();
            if d2 < *best_d2 {
                *best_d2 = d2;
                *best = Some((d2, i));
            }
            let diff = q[a] - p[a];
            let (near, far) = if diff < 0.0 {
                ((lo, mid), (mid + 1, hi))
            } else {
                ((mid + 1, hi), (lo, mid))
            };
            search(t, near.0, near.1, depth + 1, q, best, best_d2);
            if diff * diff < *best_d2 {
                search(t, far.0, far.1, depth + 1, q, best, best_d2);
            }
        }
        search(self, 0, self.idx.len(), 0, q, &mut best, &mut best_d2);
        best.map(|(d2, i)| (d2.sqrt(), i))
    }
}

impl Tree {
    /// The indices of the points within `r` of `q`.
    pub(crate) fn within(&self, q: [f64; 3], r: f64) -> Vec<usize> {
        let mut out = Vec::new();
        fn search(
            t: &Tree,
            lo: usize,
            hi: usize,
            depth: usize,
            q: [f64; 3],
            r: f64,
            out: &mut Vec<usize>,
        ) {
            if lo >= hi {
                return;
            }
            let a = depth % 3;
            let mid = lo + (hi - lo) / 2;
            let i = t.idx[mid];
            let p = t.pts[i];
            if (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>() <= r * r {
                out.push(i);
            }
            let diff = q[a] - p[a];
            if diff - r <= 0.0 {
                search(t, lo, mid, depth + 1, q, r, out);
            }
            if diff + r >= 0.0 {
                search(t, mid + 1, hi, depth + 1, q, r, out);
            }
        }
        search(self, 0, self.idx.len(), 0, q, r, &mut out);
        out
    }

    /// The typical spacing: the median distance from a point to its
    /// nearest other point, over a sample.
    pub(crate) fn spacing(&self) -> f64 {
        let step = (self.pts.len() / 500).max(1);
        let mut ds: Vec<f64> = self
            .pts
            .iter()
            .step_by(step)
            .filter_map(|p| {
                let mut near: Vec<f64> = self
                    .within(*p, 20.0)
                    .iter()
                    .map(|&i| {
                        (0..3)
                            .map(|k| (self.pts[i][k] - p[k]).powi(2))
                            .sum::<f64>()
                            .sqrt()
                    })
                    .filter(|d| *d > 1e-9)
                    .collect();
                near.sort_by(f64::total_cmp);
                near.first().copied()
            })
            .collect();
        ds.sort_by(f64::total_cmp);
        ds.get(ds.len() / 2).copied().unwrap_or(1.0)
    }

    /// A unit normal at every point: the least-spread direction of
    /// the points round it, by the covariance's smallest eigenvector.
    /// A scan's points are dense along each laser line and sparse
    /// across, so the neighbourhood starts at a few spacings and
    /// grows until it spreads two ways, not just along one line; zero
    /// where it never does.
    pub(crate) fn normals(&self) -> Vec<[f64; 3]> {
        let base = (4.0 * self.spacing()).max(1.0);
        self.pts
            .iter()
            .map(|p| {
                let mut radius = base;
                for _ in 0..5 {
                    let near = self.within(*p, radius);
                    if near.len() >= 6 {
                        let n = near.len() as f64;
                        let c = near.iter().fold([0.0; 3], |c, &i| {
                            [
                                c[0] + self.pts[i][0] / n,
                                c[1] + self.pts[i][1] / n,
                                c[2] + self.pts[i][2] / n,
                            ]
                        });
                        let mut cov = [[0.0; 3]; 3];
                        for &i in &near {
                            let d = [
                                self.pts[i][0] - c[0],
                                self.pts[i][1] - c[1],
                                self.pts[i][2] - c[2],
                            ];
                            for (r, row) in cov.iter_mut().enumerate() {
                                for (k, v) in row.iter_mut().enumerate() {
                                    *v += d[r] * d[k];
                                }
                            }
                        }
                        let (values, normal) = eigen(cov);
                        // Spread two ways: the middle eigenvalue a tenth
                        // of the largest or more.
                        if values[1] >= 0.1 * values[2] {
                            return normal;
                        }
                    }
                    radius *= 2.0;
                }
                [0.0; 3]
            })
            .collect()
    }

    /// Every point's line tangent: the longest spread of the points
    /// within `close` of it, or None where there are too few.
    pub(crate) fn tangents(&self, close: f64) -> Vec<Option<[f64; 3]>> {
        self.pts
            .iter()
            .map(|p| {
                // A line the camera sees at a slant is sparse along
                // its length: look up to four times as far for the
                // three points a tangent needs.
                let mut near = self.within(*p, close);
                let mut reach = close;
                while near.len() < 3 && reach < 4.0 * close {
                    reach *= 2.0;
                    near = self.within(*p, reach);
                }
                if near.len() < 3 {
                    return None;
                }
                let n = near.len() as f64;
                let c = near.iter().fold([0.0; 3], |c, &i| {
                    [
                        c[0] + self.pts[i][0] / n,
                        c[1] + self.pts[i][1] / n,
                        c[2] + self.pts[i][2] / n,
                    ]
                });
                let mut cov = [[0.0; 3]; 3];
                for &i in &near {
                    let d = [
                        self.pts[i][0] - c[0],
                        self.pts[i][1] - c[1],
                        self.pts[i][2] - c[2],
                    ];
                    for (r, row) in cov.iter_mut().enumerate() {
                        for (k, v) in row.iter_mut().enumerate() {
                            *v += d[r] * d[k];
                        }
                    }
                }
                Some(eigen_full(cov).1[2])
            })
            .collect()
    }

    /// The noise along a scan's laser lines: the typical distance of a
    /// point from the straight line through the points close along
    /// its own, over a sample. Zero for a cloud without lines.
    pub(crate) fn line_noise(&self) -> f64 {
        let close = (3.0 * self.spacing()).max(1.5);
        let step = (self.pts.len() / 500).max(1);
        let mut ds: Vec<f64> = Vec::new();
        for p in self.pts.iter().step_by(step) {
            let near = self.within(*p, close);
            if near.len() < 4 {
                continue;
            }
            let n = near.len() as f64;
            let c = near.iter().fold([0.0; 3], |c, &i| {
                [
                    c[0] + self.pts[i][0] / n,
                    c[1] + self.pts[i][1] / n,
                    c[2] + self.pts[i][2] / n,
                ]
            });
            let mut cov = [[0.0; 3]; 3];
            for &i in &near {
                let d = [
                    self.pts[i][0] - c[0],
                    self.pts[i][1] - c[1],
                    self.pts[i][2] - c[2],
                ];
                for (r, row) in cov.iter_mut().enumerate() {
                    for (k, v) in row.iter_mut().enumerate() {
                        *v += d[r] * d[k];
                    }
                }
            }
            let (values, vectors) = eigen_full(cov);
            // Only where the neighbourhood is one line: where a second
            // scan's line crosses, the spread is two-way and says
            // nothing about the noise.
            if values[1] > 0.05 * values[2] {
                continue;
            }
            let t = vectors[2];
            let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
            let along = d[0] * t[0] + d[1] * t[1] + d[2] * t[2];
            let off = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - along * along)
                .max(0.0)
                .sqrt();
            ds.push(off);
        }
        ds.sort_by(f64::total_cmp);
        ds.get(ds.len() / 2).copied().unwrap_or(0.0)
    }

    /// The pitch of a scan's laser lines: the typical distance from a
    /// point to the nearest point of a line running parallel to its
    /// own (the offset mostly across the local tangent, the tangents
    /// alike), over a sample. A cloud without lines gives back zero.
    pub(crate) fn line_pitch(&self, tangents: &[Option<[f64; 3]>], close: f64) -> f64 {
        let step = (self.pts.len() / 300).max(1);
        let mut pitches: Vec<f64> = Vec::new();
        for (pi, p) in self.pts.iter().enumerate().step_by(step) {
            let Some(t) = tangents[pi] else {
                continue;
            };
            let across = self
                .within(*p, 30.0_f64.max(10.0 * close))
                .iter()
                .filter_map(|&i| {
                    let d = [
                        self.pts[i][0] - p[0],
                        self.pts[i][1] - p[1],
                        self.pts[i][2] - p[2],
                    ];
                    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    let along = (d[0] * t[0] + d[1] * t[1] + d[2] * t[2]).abs();
                    let parallel = tangents[i].is_some_and(|tq| {
                        (tq[0] * t[0] + tq[1] * t[1] + tq[2] * t[2]).abs()
                            > 25f64.to_radians().cos()
                    });
                    (l > close && along < 0.4 * l && parallel).then_some(l)
                })
                .fold(f64::INFINITY, f64::min);
            if across.is_finite() {
                pitches.push(across);
            }
        }
        pitches.sort_by(f64::total_cmp);
        pitches.get(pitches.len() / 2).copied().unwrap_or(0.0)
    }

    /// A unit normal at every point (zero where none could be taken):
    /// the least-spread direction of the points round it, over a
    /// neighbourhood that grows until it spreads two ways, since a
    /// scan's points are dense along each laser line and sparse
    /// across. Smooth across an edge, which suits registration.
    ///
    /// The normals for face fitting, with the radius each was taken
    /// over and its support, the share of the neighbourhood lying on
    /// its plane (an interior point's is high, an edge's low); a zero
    /// normal carries the radius tried. A scan's points lie on laser lines,
    /// and one line is perfectly planar on its own (it lies in the
    /// laser's plane, bent or not), so a normal is never taken from one
    /// line: it is the line's own tangent crossed with the step to the
    /// nearest point of a neighbouring line, which lies in the surface,
    /// then refined on the neighbours within `tol` of that first plane
    /// so that a point near an edge keeps its own face's normal rather
    /// than a blend. Where a neighbouring line lies either side, the
    /// side with more points on its plane wins. A cloud without lines
    /// falls back to the plane through the neighbourhood.
    pub(crate) fn normals_lined(&self, tol: f64) -> Vec<([f64; 3], f64, f64)> {
        let spacing = self.spacing();
        let close = (3.0 * spacing).max(1.5);
        let tangents = self.tangents(close);
        let pitch = self.line_pitch(&tangents, close);
        let lined = pitch > close;
        let radius = if lined {
            1.5 * pitch
        } else {
            (4.0 * spacing).max(1.0)
        };
        let fit = |idx: &[usize]| -> ([f64; 3], [f64; 3]) {
            let n = idx.len() as f64;
            let c = idx.iter().fold([0.0; 3], |c, &i| {
                [
                    c[0] + self.pts[i][0] / n,
                    c[1] + self.pts[i][1] / n,
                    c[2] + self.pts[i][2] / n,
                ]
            });
            let mut cov = [[0.0; 3]; 3];
            for &i in idx {
                let d = [
                    self.pts[i][0] - c[0],
                    self.pts[i][1] - c[1],
                    self.pts[i][2] - c[2],
                ];
                for (r, row) in cov.iter_mut().enumerate() {
                    for (k, v) in row.iter_mut().enumerate() {
                        *v += d[r] * d[k];
                    }
                }
            }
            let (_, vectors) = eigen_full(cov);
            (vectors[0], vectors[2])
        };
        self.pts
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                let near = self.within(*p, radius);
                if near.len() < 6 {
                    return ([0.0; 3], radius, 0.0);
                }
                if !lined {
                    let (n, _) = fit(&near);
                    let on = near
                        .iter()
                        .filter(|&&i| {
                            let d = [
                                self.pts[i][0] - p[0],
                                self.pts[i][1] - p[1],
                                self.pts[i][2] - p[2],
                            ];
                            (d[0] * n[0] + d[1] * n[1] + d[2] * n[2]).abs() <= tol
                        })
                        .count();
                    return (n, radius, on as f64 / near.len() as f64);
                }
                let Some(t) = tangents[pi] else {
                    return ([0.0; 3], radius, 0.0);
                };
                // The nearest point of a neighbouring line, on either
                // side: offset across this line's tangent, and running
                // parallel to it. The same line bent over an edge also
                // offers points across the tangent, but their own
                // tangent has turned with the fold.
                let candidates = |idx: &[usize]| -> Vec<(f64, [f64; 3])> {
                    let mut out: Vec<(f64, [f64; 3])> = idx
                        .iter()
                        .filter_map(|&i| {
                            let d = [
                                self.pts[i][0] - p[0],
                                self.pts[i][1] - p[1],
                                self.pts[i][2] - p[2],
                            ];
                            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                            let a = d[0] * t[0] + d[1] * t[1] + d[2] * t[2];
                            let parallel = tangents[i].is_some_and(|tq| {
                                (tq[0] * t[0] + tq[1] * t[1] + tq[2] * t[2]).abs()
                                    > 25f64.to_radians().cos()
                            });
                            (l > close && a.abs() < 0.4 * l && parallel).then_some((l, d))
                        })
                        .collect();
                    out.sort_by(|a, b| a.0.total_cmp(&b.0));
                    out
                };
                // Lines lie further apart out at the rim than in near
                // the axis, and further still on a face the laser
                // strikes at a slant, so the search widens, up to four
                // times, until a neighbouring line appears.
                let mut across = candidates(&near);
                let mut near = near;
                let mut reach = radius;
                while across.is_empty() && reach < 4.0 * radius {
                    reach *= 2.0;
                    near = self.within(*p, reach);
                    across = candidates(&near);
                }
                let Some(first) = across.first().copied() else {
                    return ([0.0; 3], radius, 0.0);
                };
                let other = across
                    .iter()
                    .find(|(_, d)| d[0] * first.1[0] + d[1] * first.1[1] + d[2] * first.1[2] < 0.0);
                let mut best: Option<(usize, [f64; 3])> = None;
                for (_, d) in std::iter::once(&first).chain(other) {
                    let n0 = [
                        t[1] * d[2] - t[2] * d[1],
                        t[2] * d[0] - t[0] * d[2],
                        t[0] * d[1] - t[1] * d[0],
                    ];
                    let l = (n0[0] * n0[0] + n0[1] * n0[1] + n0[2] * n0[2]).sqrt();
                    if l < 1e-12 {
                        continue;
                    }
                    let n0 = [n0[0] / l, n0[1] / l, n0[2] / l];
                    let inliers: Vec<usize> = near
                        .iter()
                        .copied()
                        .filter(|&i| {
                            let d = [
                                self.pts[i][0] - p[0],
                                self.pts[i][1] - p[1],
                                self.pts[i][2] - p[2],
                            ];
                            (d[0] * n0[0] + d[1] * n0[1] + d[2] * n0[2]).abs() <= tol
                        })
                        .collect();
                    if inliers.len() >= 6 && best.is_none_or(|(c, _)| inliers.len() > c) {
                        let (n, _) = fit(&inliers);
                        best = Some((inliers.len(), n));
                    }
                }
                match best {
                    Some((count, n)) => (n, radius, count as f64 / near.len() as f64),
                    None => ([0.0; 3], radius, 0.0),
                }
            })
            .collect()
    }
}

/// The eigenvalues of a symmetric 3x3 matrix in ascending order and
/// the unit eigenvector of the smallest, by Jacobi rotations.
pub(crate) fn eigen(a: [[f64; 3]; 3]) -> ([f64; 3], [f64; 3]) {
    let (values, vectors) = eigen_full(a);
    (values, vectors[0])
}

/// The eigenvalues in ascending order with their unit eigenvectors.
fn eigen_full(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..30 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > big {
                big = a[i][j].abs();
                p = i;
                q = j;
            }
        }
        if big < 1e-12 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let t = if theta == 0.0 { 1.0 } else { t };
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        for row in a.iter_mut() {
            let (akp, akq) = (row[p], row[q]);
            row[p] = c * akp - s * akq;
            row[q] = s * akp + c * akq;
        }
        let (rp, rq) = (a[p], a[q]);
        for k in 0..3 {
            a[p][k] = c * rp[k] - s * rq[k];
            a[q][k] = s * rp[k] + c * rq[k];
        }
        for row in v.iter_mut() {
            let (vp, vq) = (row[p], row[q]);
            row[p] = c * vp - s * vq;
            row[q] = s * vp + c * vq;
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]));
    let mut values = [0.0; 3];
    let mut vectors = [[0.0; 3]; 3];
    for (slot, &k) in order.iter().enumerate() {
        values[slot] = a[k][k];
        let n = [v[0][k], v[1][k], v[2][k]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        vectors[slot] = if l < 1e-12 {
            [0.0; 3]
        } else {
            [n[0] / l, n[1] / l, n[2] / l]
        };
    }
    (values, vectors)
}

fn extent(pts: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in pts {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    (lo, hi)
}

fn solve6(mut a: [[f64; 6]; 6], mut b: [f64; 6]) -> Option<[f64; 6]> {
    for c in 0..6 {
        let p = (c..6).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        let pivot = a[c];
        for r in c + 1..6 {
            let f = a[r][c] / pivot[c];
            for (ark, pk) in a[r].iter_mut().zip(&pivot).skip(c) {
                *ark -= f * pk;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; 6];
    for c in (0..6).rev() {
        let s: f64 = (c + 1..6).map(|k| a[c][k] * x[k]).sum();
        x[c] = (b[c] - s) / a[c][c];
    }
    Some(x)
}

/// One run of iterative closest point from a seed: the source sample
/// moved by the current transform, each point paired with its nearest
/// reference point within reach, and the small rotation and
/// translation solved that best bring each point onto the plane
/// through its pair (point to plane, which lets the source slide
/// along flat faces instead of snapping to the reference's sampling).
/// The reach stays wide and nothing is trimmed while the pose is
/// still turning, since the far pairs are what pull it round; after
/// that the furthest fifth of the pairs goes and the reach shrinks
/// to three times the median. Returns the transform and the median
/// point-to-point distance.
fn icp(
    sample: &[[f64; 3]],
    tree: &Tree,
    normals: &[[f64; 3]],
    seed: Transform,
    reach: f64,
) -> (Transform, f64) {
    let mut tf = seed;
    let mut max = reach;
    let mut last = f64::INFINITY;
    for it in 0..80 {
        let moved: Vec<[f64; 3]> = sample.iter().map(|p| tf.apply(*p)).collect();
        let mut pairs: Vec<(f64, [f64; 3], usize)> = moved
            .iter()
            .filter_map(|p| tree.nearest(*p, max).map(|(d, q)| (d, *p, q)))
            .collect();
        if pairs.len() < 6 {
            return (tf, f64::INFINITY);
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let keep = if it < 20 {
            pairs.len()
        } else {
            (pairs.len() * 4 / 5).max(6)
        };
        pairs.truncate(keep);
        let median = pairs[pairs.len() / 2].0;
        let c = pairs.iter().fold([0.0; 3], |c, (_, p, _)| {
            [
                c[0] + p[0] / keep as f64,
                c[1] + p[1] / keep as f64,
                c[2] + p[2] / keep as f64,
            ]
        });
        let mut a = [[0.0; 6]; 6];
        let mut b = [0.0; 6];
        for (_, p, qi) in &pairs {
            let q = tree.pts[*qi];
            let n = normals[*qi];
            let x = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
            // n · (p + ω × x + δ - q): the row is [x × n | n].
            let xn = [
                x[1] * n[2] - x[2] * n[1],
                x[2] * n[0] - x[0] * n[2],
                x[0] * n[1] - x[1] * n[0],
            ];
            let j = [xn[0], xn[1], xn[2], n[0], n[1], n[2]];
            let r = n[0] * (q[0] - p[0]) + n[1] * (q[1] - p[1]) + n[2] * (q[2] - p[2]);
            for i in 0..6 {
                b[i] += j[i] * r;
                for k in 0..6 {
                    a[i][k] += j[i] * j[k];
                }
            }
        }
        let Some(x) = solve6(a, b) else {
            return (tf, median);
        };
        let w = [x[0], x[1], x[2]];
        let angle = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
        let rw = if angle < 1e-12 {
            Transform::identity().r
        } else {
            rotation(w, angle.to_degrees())
        };
        // p' = c + Rw (p - c) + δ, with p = R p0 + t.
        let r = mul(rw, tf.r);
        let tc = [tf.t[0] - c[0], tf.t[1] - c[1], tf.t[2] - c[2]];
        let rt = [
            rw[0][0] * tc[0] + rw[0][1] * tc[1] + rw[0][2] * tc[2],
            rw[1][0] * tc[0] + rw[1][1] * tc[1] + rw[1][2] * tc[2],
            rw[2][0] * tc[0] + rw[2][1] * tc[1] + rw[2][2] * tc[2],
        ];
        tf = Transform {
            r,
            t: [
                rt[0] + c[0] + x[3],
                rt[1] + c[1] + x[4],
                rt[2] + c[2] + x[5],
            ],
        };
        if it >= 20 {
            max = (3.0 * median).clamp(1.5, reach).max(max * 0.8);
        }
        let step = angle.to_degrees() + (x[3] * x[3] + x[4] * x[4] + x[5] * x[5]).sqrt();
        if step < 1e-4 && (last - median).abs() < 1e-5 {
            break;
        }
        last = median;
    }
    (tf, surface_distances(sample, tree, normals, &tf, reach).0)
}

/// How far the source's points lie from the reference's surface once
/// moved: the median distance to the plane through each point's
/// nearest reference point (the scans' laser lines interleave, so the
/// nearest point itself can be millimetres along the surface from a
/// perfect fit), and the share of points within 2 mm of it. The
/// median is infinite when fewer than a tenth of the points find a
/// neighbour within reach.
fn surface_distances(
    sample: &[[f64; 3]],
    tree: &Tree,
    normals: &[[f64; 3]],
    tf: &Transform,
    reach: f64,
) -> (f64, f64) {
    let mut ds: Vec<f64> = sample
        .iter()
        .filter_map(|p| {
            let p = tf.apply(*p);
            tree.nearest(p, reach).map(|(d, qi)| {
                let (q, n) = (tree.pts[qi], normals[qi]);
                if n == [0.0; 3] {
                    d
                } else {
                    (n[0] * (q[0] - p[0]) + n[1] * (q[1] - p[1]) + n[2] * (q[2] - p[2])).abs()
                }
            })
        })
        .collect();
    if ds.len() < sample.len() / 10 {
        return (f64::INFINITY, 0.0);
    }
    let within = ds.iter().filter(|d| **d <= 2.0).count();
    ds.sort_by(f64::total_cmp);
    (ds[ds.len() / 2], within as f64 / sample.len() as f64)
}

/// Registers `src` onto `dst`: the rigid motion taking the points of
/// `src` into the frame of `dst`, found from seeds the placement
/// gives (the turn about the vertical, and for a roll the axis rolled
/// about, every 30 degrees), each scored by how close the source then
/// lies to the reference, the best few refined by iterative closest
/// point, and the closest kept.
pub fn register(
    src: &[[f64; 3]],
    dst: &[[f64; 3]],
    placement: Placement,
) -> Result<Registration, String> {
    if src.len() < 50 || dst.len() < 50 {
        return Err(format!(
            "too few points to register ({} and {})",
            src.len(),
            dst.len()
        ));
    }
    let tree = Tree::new(dst);
    let normals = tree.normals();
    let step = (src.len() / 1500).max(1);
    let sample: Vec<[f64; 3]> = src.iter().step_by(step).copied().collect();
    let coarse: Vec<[f64; 3]> = sample.iter().step_by(4).copied().collect();
    let (slo, shi) = extent(src);
    let (dlo, dhi) = extent(dst);
    let cs = [
        (slo[0] + shi[0]) / 2.0,
        (slo[1] + shi[1]) / 2.0,
        (slo[2] + shi[2]) / 2.0,
    ];
    let cd = [
        (dlo[0] + dhi[0]) / 2.0,
        (dlo[1] + dhi[1]) / 2.0,
        (dlo[2] + dhi[2]) / 2.0,
    ];
    let size = (0..3)
        .map(|a| (shi[a] - slo[a]).max(dhi[a] - dlo[a]))
        .fold(0.0, f64::max);
    let reach = (size * 0.15).max(5.0);
    // The seeds: rotations about the source's centre that put it on
    // the reference's.
    let about = |r: [[f64; 3]; 3]| -> Transform {
        let rc = [
            r[0][0] * cs[0] + r[0][1] * cs[1] + r[0][2] * cs[2],
            r[1][0] * cs[0] + r[1][1] * cs[1] + r[1][2] * cs[2],
            r[2][0] * cs[0] + r[2][1] * cs[1] + r[2][2] * cs[2],
        ];
        Transform {
            r,
            t: [cd[0] - rc[0], cd[1] - rc[1], cd[2] - rc[2]],
        }
    };
    let z = [0.0, 0.0, 1.0];
    let x = [1.0, 0.0, 0.0];
    let turns: Vec<f64> = (0..12).map(|k| 30.0 * k as f64).collect();
    let seeds: Vec<Transform> = match placement {
        Placement::Same => vec![Transform::identity()],
        Placement::Moved => turns.iter().map(|&g| about(rotation(z, g))).collect(),
        Placement::UpsideDown => turns
            .iter()
            .map(|&g| about(mul(rotation(z, g), rotation(x, 180.0))))
            .collect(),
        Placement::OnSide => turns
            .iter()
            .flat_map(|&g| {
                turns.iter().map(move |&a| {
                    about(mul(mul(rotation(z, g), rotation(x, 90.0)), rotation(z, -a)))
                })
            })
            .collect(),
    };
    // Score every seed coarsely, refine the best few.
    let mut scored: Vec<(f64, Transform)> = seeds
        .iter()
        .map(|s| {
            let mut ds: Vec<f64> = coarse
                .iter()
                .map(|p| {
                    tree.nearest(s.apply(*p), reach)
                        .map(|(d, _)| d)
                        .unwrap_or(reach)
                })
                .collect();
            ds.sort_by(f64::total_cmp);
            (ds[ds.len() / 2], *s)
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    let refine = scored.len().min(6);
    let mut best: Option<(f64, Transform)> = None;
    for (_, seed) in scored.iter().take(refine) {
        let (tf, residual) = icp(&sample, &tree, &normals, *seed, reach);
        if best.is_none_or(|(r, _)| residual < r) {
            best = Some((residual, tf));
        }
    }
    let (residual, transform) = best.unwrap();
    if !residual.is_finite() {
        return Err(format!(
            "the scans do not register as `{}`: too little of one lies within reach of the other",
            placement.name()
        ));
    }
    let (residual, overlap) = surface_distances(src, &tree, &normals, &transform, reach);
    Ok(Registration {
        placement,
        transform,
        residual,
        overlap,
        seeds: seeds.len(),
    })
}

/// Several scans of one object into one cloud: each after the first
/// is registered onto everything merged so far, by how the object was
/// placed between them, and its points carried over. Returns the
/// cloud, in the first scan's frame, and the registrations.
pub fn merge(
    scans: &[(Vec<[f64; 3]>, Placement)],
) -> Result<(Vec<[f64; 3]>, Vec<Registration>), String> {
    let Some((first, _)) = scans.first() else {
        return Err("no scans to merge".into());
    };
    let mut cloud = first.clone();
    let mut regs = Vec::new();
    for (points, placement) in scans.iter().skip(1) {
        let reg = register(points, &cloud, *placement)?;
        cloud.extend(points.iter().map(|p| reg.transform.apply(*p)));
        regs.push(reg);
    }
    Ok((cloud, regs))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points over a box's faces, the top and one side left out of
    /// `partial` so the clouds only overlap in part.
    pub fn box_points(partial: bool) -> Vec<[f64; 3]> {
        let (w, d, h) = (60.0, 40.0, 80.0);
        let mut pts = Vec::new();
        let n = 24;
        for i in 0..=n {
            for j in 0..=n {
                let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                pts.push([u * w, 0.0, v * h]);
                pts.push([u * w, d, v * h]);
                pts.push([0.0, u * d, v * h]);
                if !partial {
                    pts.push([w, u * d, v * h]);
                    pts.push([u * w, v * d, h]);
                }
                pts.push([u * w, v * d, 0.0]);
            }
        }
        pts
    }

    fn moved(pts: &[[f64; 3]], tf: &Transform) -> Vec<[f64; 3]> {
        pts.iter().map(|p| tf.apply(*p)).collect()
    }

    fn check(placement: Placement, truth: Transform) {
        let dst = box_points(false);
        // The source: the box in the other pose, part of it unseen.
        let src = moved(&box_points(true), &truth);
        let reg = register(&src, &dst, placement).unwrap();
        // Every source point lands on the reference's surface.
        let tree = Tree::new(&dst);
        let mut ds: Vec<f64> = src
            .iter()
            .map(|p| {
                tree.nearest(reg.transform.apply(*p), 10.0)
                    .map(|(d, _)| d)
                    .unwrap_or(10.0)
            })
            .collect();
        ds.sort_by(f64::total_cmp);
        let p90 = ds[ds.len() * 9 / 10];
        assert!(
            p90 < 0.05,
            "{}: 90 % of points within {p90} mm, residual {}, overlap {}",
            placement.name(),
            reg.residual,
            reg.overlap
        );
        assert!(
            reg.overlap > 0.95,
            "{}: overlap {}",
            placement.name(),
            reg.overlap
        );
    }

    #[test]
    fn a_box_registers_from_each_placement() {
        let z = [0.0, 0.0, 1.0];
        let x = [1.0, 0.0, 0.0];
        let about = |r: [[f64; 3]; 3], t: [f64; 3]| Transform { r, t };
        check(Placement::Same, Transform::identity());
        check(
            Placement::Moved,
            about(rotation(z, 50.0), [25.0, -40.0, 0.0]),
        );
        // Upside down: the box flipped lands with its top at z = 0.
        check(
            Placement::UpsideDown,
            about(
                mul(rotation(z, 110.0), rotation(x, 180.0)),
                [20.0, 70.0, 80.0],
            ),
        );
        // On its side: rolled about an axis at 40 degrees, then turned.
        let a = 40f64.to_radians();
        let roll = rotation([a.cos(), a.sin(), 0.0], 90.0);
        let r = mul(rotation(z, 25.0), roll);
        let tf = about(r, [0.0; 3]);
        let lo = box_points(false)
            .iter()
            .map(|p| tf.apply(*p)[2])
            .fold(f64::MAX, f64::min);
        check(Placement::OnSide, about(r, [30.0, 10.0, -lo]));
    }

    #[test]
    fn the_placement_names_parse() {
        assert_eq!(
            Placement::parse("upside-down").unwrap(),
            Placement::UpsideDown
        );
        assert_eq!(Placement::parse("On its side").unwrap(), Placement::OnSide);
        assert_eq!(Placement::parse("").unwrap(), Placement::Same);
        assert!(Placement::parse("sideways").is_err());
    }
}
