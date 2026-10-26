//! A synthetic turntable, for building the scanner against before any
//! video exists and for keeping it honest after: an object of known
//! shape on the measuring field, a camera and a line laser fixed to
//! each other going round it (the same thing as the field turning
//! under them), one frame per step. The object's triangles are
//! rasterised into a depth buffer, the field's plane is shaded behind
//! them, and the laser paints a green line where its plane meets what
//! the camera sees and the laser itself can reach, by a shadow map
//! rendered from the laser's own point of view.

use super::field::{field_colour, Field};
use ok_render::{Image, Rgb};
use std::f64::consts::PI;

/// A lobed gourd: a bottle-shaped profile of revolution, `lobes`
/// ridges round it `lobe` deep as a fraction of the radius, standing on
/// the field at `base` with its axis up z.
#[derive(Debug, Clone, Copy)]
pub struct Gourd {
    pub base: [f64; 2],
    pub height: f64,
    pub lobes: usize,
    pub lobe: f64,
}

impl Gourd {
    /// The radius at height `z` and azimuth `phi` (radians from +x).
    pub fn radius(&self, z: f64, phi: f64) -> f64 {
        let h = self.height / 120.0;
        let z = z / h;
        let profile = 45.0 * (-((z - 35.0) / 28.0).powi(2)).exp()
            + 22.0 * (-((z - 95.0) / 18.0).powi(2)).exp()
            + 4.0;
        profile * h * (1.0 + self.lobe * (self.lobes as f64 * phi).cos())
    }

    /// Its surface as triangles, counter-clockwise from outside, with a
    /// flat top and bottom.
    pub fn mesh(&self, nz: usize, nphi: usize) -> Vec<[[f64; 3]; 3]> {
        let at = |iz: usize, ip: usize| -> [f64; 3] {
            let z = self.height * iz as f64 / nz as f64;
            let phi = 2.0 * PI * ip as f64 / nphi as f64;
            let r = self.radius(z, phi);
            [
                self.base[0] + r * phi.cos(),
                self.base[1] + r * phi.sin(),
                z,
            ]
        };
        let mut tris = Vec::new();
        for iz in 0..nz {
            for ip in 0..nphi {
                let (a, b, c, d) = (
                    at(iz, ip),
                    at(iz, (ip + 1) % nphi),
                    at(iz + 1, (ip + 1) % nphi),
                    at(iz + 1, ip),
                );
                tris.push([a, b, c]);
                tris.push([a, c, d]);
            }
        }
        let (bottom, top) = (
            [self.base[0], self.base[1], 0.0],
            [self.base[0], self.base[1], self.height],
        );
        for ip in 0..nphi {
            tris.push([bottom, at(0, (ip + 1) % nphi), at(0, ip)]);
            tris.push([top, at(nz, ip), at(nz, (ip + 1) % nphi)]);
        }
        tris
    }

    /// Volume by integrating the profile.
    pub fn volume(&self) -> f64 {
        let (nz, nphi) = (600, 360);
        let mut v = 0.0;
        for iz in 0..nz {
            let z = self.height * (iz as f64 + 0.5) / nz as f64;
            for ip in 0..nphi {
                let phi = 2.0 * PI * (ip as f64 + 0.5) / nphi as f64;
                v += 0.5
                    * self.radius(z, phi).powi(2)
                    * (2.0 * PI / nphi as f64)
                    * (self.height / nz as f64);
            }
        }
        v
    }
}

/// A pinhole camera: focal length in pixels, principal point, position
/// in world millimetres, and the rotation taking world to camera axes
/// (x right, y down, z forward).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub f: f64,
    pub cx: f64,
    pub cy: f64,
    pub pos: [f64; 3],
    pub r: [[f64; 3]; 3],
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

impl Camera {
    /// At `pos` looking at `target`, with world z as up.
    pub fn look_at(f: f64, width: usize, height: usize, pos: [f64; 3], target: [f64; 3]) -> Camera {
        let forward = norm(sub(target, pos));
        let right = norm(cross(forward, [0.0, 0.0, 1.0]));
        let down = cross(forward, right);
        Camera {
            f,
            cx: width as f64 / 2.0,
            cy: height as f64 / 2.0,
            pos,
            r: [right, down, forward],
        }
    }

    /// World to camera coordinates.
    pub fn to_camera(&self, p: [f64; 3]) -> [f64; 3] {
        let d = sub(p, self.pos);
        [dot(self.r[0], d), dot(self.r[1], d), dot(self.r[2], d)]
    }

    /// Where a world point lands: pixel x, y and depth.
    pub fn project(&self, p: [f64; 3]) -> Option<[f64; 3]> {
        let c = self.to_camera(p);
        (c[2] > 1.0).then(|| {
            [
                self.cx + self.f * c[0] / c[2],
                self.cy + self.f * c[1] / c[2],
                c[2],
            ]
        })
    }

    /// The world direction of the ray through a pixel.
    pub fn ray(&self, u: f64, v: f64) -> [f64; 3] {
        let d = [(u - self.cx) / self.f, (v - self.cy) / self.f, 1.0];
        norm([
            self.r[0][0] * d[0] + self.r[1][0] * d[1] + self.r[2][0] * d[2],
            self.r[0][1] * d[0] + self.r[1][1] * d[1] + self.r[2][1] * d[2],
            self.r[0][2] * d[0] + self.r[1][2] * d[1] + self.r[2][2] * d[2],
        ])
    }
}

/// A depth buffer with the world point and the surface normal at each
/// pixel that a triangle covers.
struct Depth {
    z: Vec<f64>,
    point: Vec<[f64; 3]>,
    normal: Vec<[f64; 3]>,
}

fn rasterise(cam: &Camera, tris: &[[[f64; 3]; 3]], w: usize, h: usize) -> Depth {
    let mut d = Depth {
        z: vec![f64::INFINITY; w * h],
        point: vec![[0.0; 3]; w * h],
        normal: vec![[0.0; 3]; w * h],
    };
    for t in tris {
        let (Some(a), Some(b), Some(c)) = (cam.project(t[0]), cam.project(t[1]), cam.project(t[2]))
        else {
            continue;
        };
        let n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
        let (x0, x1) = (
            a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize,
            (a[0].max(b[0]).max(c[0]).ceil() as isize).min(w as isize - 1),
        );
        let (y0, y1) = (
            a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize,
            (a[1].max(b[1]).max(c[1]).ceil() as isize).min(h as isize - 1),
        );
        if x1 < 0 || y1 < 0 {
            continue;
        }
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-9 {
            continue;
        }
        for j in y0..=y1 as usize {
            for i in x0..=x1 as usize {
                let (px, py) = (i as f64 + 0.5, j as f64 + 0.5);
                let wa = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
                let wb = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
                let wc = 1.0 - wa - wb;
                if wa < -1e-6 || wb < -1e-6 || wc < -1e-6 {
                    continue;
                }
                // Perspective-correct: interpolate 1/z and point/z.
                let iz = wa / a[2] + wb / b[2] + wc / c[2];
                let z = 1.0 / iz;
                let k = j * w + i;
                if z < d.z[k] {
                    d.z[k] = z;
                    let mut p = [0.0; 3];
                    for (axis, pp) in p.iter_mut().enumerate() {
                        *pp = (wa * t[0][axis] / a[2]
                            + wb * t[1][axis] / b[2]
                            + wc * t[2][axis] / c[2])
                            * z;
                    }
                    d.point[k] = p;
                    d.normal[k] = n;
                }
            }
        }
    }
    d
}

/// The rig: how far the camera stands from the turntable's axis, how
/// high above the table's plane in degrees, and where the laser is,
/// as an azimuth offset from the camera about the axis and its own
/// distance and elevation. The laser's plane holds the axis.
#[derive(Debug, Clone, Copy)]
pub struct Rig {
    pub focal: f64,
    pub width: usize,
    pub height: usize,
    pub distance: f64,
    pub elevation: f64,
    pub laser_azimuth: f64,
    pub laser_distance: f64,
    pub laser_elevation: f64,
    /// The field point the turntable's axis passes through, and the
    /// height on it the camera looks at.
    pub axis: [f64; 2],
    pub look_height: f64,
}

/// Where the camera and the laser stand for the turn angle `theta`
/// (degrees): the camera, and the laser's origin and plane normal.
pub fn rig_at(rig: &Rig, theta: f64) -> (Camera, [f64; 3], [f64; 3]) {
    let place = |az: f64, dist: f64, el: f64| -> [f64; 3] {
        let (a, e) = (az.to_radians(), el.to_radians());
        [
            rig.axis[0] + dist * a.cos() * e.cos(),
            rig.axis[1] + dist * a.sin() * e.cos(),
            rig.look_height + dist * e.sin(),
        ]
    };
    let cam = Camera::look_at(
        rig.focal,
        rig.width,
        rig.height,
        place(theta, rig.distance, rig.elevation),
        [rig.axis[0], rig.axis[1], rig.look_height],
    );
    let laser = place(
        theta + rig.laser_azimuth,
        rig.laser_distance,
        rig.laser_elevation,
    );
    // A vertical plane through the axis and the laser.
    let along = sub(laser, [rig.axis[0], rig.axis[1], laser[2]]);
    let normal = norm(cross(along, [0.0, 0.0, 1.0]));
    (cam, laser, normal)
}

/// One frame: the field (its paper at `paper` grey, the exposure being
/// locked for the laser), the object in `colour` lit from the camera's
/// side, and the laser line where its plane meets the scene within
/// `sigma` mm, mixed in as a green that does not saturate to white.
#[allow(clippy::too_many_arguments)]
pub fn render(
    field: &Field,
    tris: &[[[f64; 3]; 3]],
    cam: &Camera,
    laser: [f64; 3],
    laser_normal: [f64; 3],
    colour: Rgb,
    paper: u8,
    seed: u64,
) -> Image {
    let (w, h) = (cam.cx as usize * 2, cam.cy as usize * 2);
    let depth = rasterise(cam, tris, w, h);
    // The shadow map: the scene from the laser, looking at the axis.
    let look = {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for t in tris {
            for p in t {
                for a in 0..3 {
                    lo[a] = lo[a].min(p[a]);
                    hi[a] = hi[a].max(p[a]);
                }
            }
        }
        [
            (lo[0] + hi[0]) / 2.0,
            (lo[1] + hi[1]) / 2.0,
            (lo[2] + hi[2]) / 2.0,
        ]
    };
    let shadow_cam = Camera::look_at(1200.0, 1200, 1200, laser, look);
    let shadow = rasterise(&shadow_cam, tris, 1200, 1200);
    let lit = |p: [f64; 3]| -> bool {
        let Some(q) = shadow_cam.project(p) else {
            return false;
        };
        let (i, j) = (q[0].round() as isize, q[1].round() as isize);
        if i < 0 || j < 0 || i >= 1200 || j >= 1200 {
            return true;
        }
        // The bias keeps a surface from shadowing itself where it lies
        // at a grazing angle to the laser.
        q[2] <= shadow.z[j as usize * 1200 + i as usize] + 3.0
    };
    let sigma = 0.45;
    let mut img = Image::new(w, h, Rgb(60, 60, 60));
    let mut rng = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let light = norm(sub(cam.pos, look));
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let (mut c, p);
            if depth.z[k].is_finite() {
                let n = depth.normal[k];
                let shade = 0.35 + 0.65 * dot(n, light).max(0.0);
                c = Rgb(
                    (colour.0 as f64 * shade) as u8,
                    (colour.1 as f64 * shade) as u8,
                    (colour.2 as f64 * shade) as u8,
                );
                p = Some(depth.point[k]);
            } else {
                let d = cam.ray(i as f64 + 0.5, j as f64 + 0.5);
                if d[2] < -1e-6 {
                    let t = -cam.pos[2] / d[2];
                    let hit = [cam.pos[0] + t * d[0], cam.pos[1] + t * d[1], 0.0];
                    let b = field.bounds();
                    if hit[0] >= b[0] && hit[0] <= b[2] && hit[1] >= b[1] && hit[1] <= b[3] {
                        c = match field_colour(field, hit[0], hit[1]) {
                            Some(Rgb(0, 0, 0)) => Rgb(20, 20, 20),
                            Some(Rgb(255, 255, 255)) => Rgb(paper, paper, paper),
                            Some(Rgb(200, 200, 200)) => {
                                let g = (paper as f64 * 0.8) as u8;
                                Rgb(g, g, g)
                            }
                            Some(x) => x,
                            None => Rgb(paper, paper, paper),
                        };
                        p = Some(hit);
                    } else {
                        c = Rgb(70, 65, 60);
                        p = Some(hit);
                    }
                } else {
                    c = Rgb(60, 60, 60);
                    p = None;
                }
            }
            if let Some(p) = p {
                let s = dot(laser_normal, sub(p, laser));
                let intensity = (-(s / sigma).powi(2)).exp();
                if intensity > 0.02 && lit(p) {
                    let mix =
                        |a: u8, b: u8| (a as f64 * (1.0 - intensity) + b as f64 * intensity) as u8;
                    c = Rgb(mix(c.0, 90), mix(c.1, 255), mix(c.2, 110));
                }
            }
            // A little sensor noise.
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let noise = (rng % 7) as i32 - 3;
            let n = |v: u8| (v as i32 + noise).clamp(0, 255) as u8;
            img.set(i, j, Rgb(n(c.0), n(c.1), n(c.2)));
        }
    }
    img
}

/// A whole turn: `frames` steps of the rig round the object.
pub fn turntable(
    field: &Field,
    tris: &[[[f64; 3]; 3]],
    rig: &Rig,
    frames: usize,
    colour: Rgb,
) -> Vec<Image> {
    (0..frames)
        .map(|k| {
            let theta = 360.0 * k as f64 / frames as f64;
            let (cam, laser, normal) = rig_at(rig, theta);
            render(field, tris, &cam, laser, normal, colour, 185, k as u64 + 1)
        })
        .collect()
}
