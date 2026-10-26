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

/// A mushroom: a stem up to six tenths of the height, then a cap of
/// `cap` radius whose underside is flat and whose top is half an
/// ellipsoid. The underside faces straight down, which no camera
/// above the table sees from any angle, so one scan bridges the stem
/// to the cap's rim with a cone and only a second pose, the mushroom
/// on its side, shows what is under the cap.
#[derive(Debug, Clone, Copy)]
pub struct Mushroom {
    pub base: [f64; 2],
    pub height: f64,
    pub stem: f64,
    pub cap: f64,
}

impl Mushroom {
    pub fn gill_height(&self) -> f64 {
        0.6 * self.height
    }

    /// The rings of the profile from the bottom up: (z, radius).
    fn rings(&self, nz: usize) -> Vec<(f64, f64)> {
        let g = self.gill_height();
        let mut rings = Vec::new();
        for i in 0..=nz {
            rings.push((g * i as f64 / nz as f64, self.stem));
        }
        for i in 0..=nz {
            // The cap: an ellipse quarter from the rim up to the top.
            let t = i as f64 / nz as f64;
            let z = g + (self.height - g) * t;
            let r = self.cap * (1.0 - t * t).max(0.0).sqrt();
            rings.push((z, r.max(0.0)));
        }
        rings
    }

    /// Its surface as triangles, counter-clockwise from outside.
    pub fn mesh(&self, nz: usize, nphi: usize) -> Vec<[[f64; 3]; 3]> {
        let rings = self.rings(nz);
        let at = |ring: &(f64, f64), ip: usize| -> [f64; 3] {
            let phi = 2.0 * PI * ip as f64 / nphi as f64;
            [
                self.base[0] + ring.1 * phi.cos(),
                self.base[1] + ring.1 * phi.sin(),
                ring.0,
            ]
        };
        let mut tris = Vec::new();
        for w in rings.windows(2) {
            for ip in 0..nphi {
                let (a, b, c, d) = (
                    at(&w[0], ip),
                    at(&w[0], (ip + 1) % nphi),
                    at(&w[1], (ip + 1) % nphi),
                    at(&w[1], ip),
                );
                if (w[0].1 - w[1].1).abs() > 1e-12 || (w[0].0 - w[1].0).abs() > 1e-12 {
                    tris.push([a, b, c]);
                    tris.push([a, c, d]);
                }
            }
        }
        // The last ring is the apex (radius 0), so the side already
        // closes the top; the bottom is a fan.
        let bottom = [self.base[0], self.base[1], 0.0];
        for ip in 0..nphi {
            tris.push([bottom, at(&rings[0], (ip + 1) % nphi), at(&rings[0], ip)]);
        }
        tris.retain(|t| {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            dot(n, n) > 1e-18
        });
        tris
    }

    /// The stem's cylinder and the cap's half ellipsoid.
    pub fn volume(&self) -> f64 {
        let g = self.gill_height();
        PI * self.stem * self.stem * g + 2.0 / 3.0 * PI * self.cap * self.cap * (self.height - g)
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

/// A convex solid of known shape to scan: the five Platonic solids and
/// the truncated icosahedron (the bucky ball), each as a vertex list,
/// its faces found as the planes that support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solid {
    Tetrahedron,
    Cube,
    Octahedron,
    Dodecahedron,
    Icosahedron,
    Bucky,
}

impl Solid {
    pub const ALL: [Solid; 6] = [
        Solid::Tetrahedron,
        Solid::Cube,
        Solid::Octahedron,
        Solid::Dodecahedron,
        Solid::Icosahedron,
        Solid::Bucky,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Solid::Tetrahedron => "tetrahedron",
            Solid::Cube => "cube",
            Solid::Octahedron => "octahedron",
            Solid::Dodecahedron => "dodecahedron",
            Solid::Icosahedron => "icosahedron",
            Solid::Bucky => "bucky ball",
        }
    }

    /// The vertices about the origin, at the textbook coordinates.
    pub fn vertices(&self) -> Vec<[f64; 3]> {
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        let signs = |f: &dyn Fn(f64, f64, f64) -> [f64; 3]| -> Vec<[f64; 3]> {
            let mut v = Vec::new();
            for sa in [-1.0, 1.0] {
                for sb in [-1.0, 1.0] {
                    for sc in [-1.0, 1.0] {
                        v.push(f(sa, sb, sc));
                    }
                }
            }
            v
        };
        let dedup = |mut v: Vec<[f64; 3]>| -> Vec<[f64; 3]> {
            let mut out: Vec<[f64; 3]> = Vec::new();
            for p in v.drain(..) {
                if !out.iter().any(|q| dist2(*q, p) < 1e-12) {
                    out.push(p);
                }
            }
            out
        };
        match self {
            Solid::Tetrahedron => vec![
                [1.0, 1.0, 1.0],
                [1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
            ],
            Solid::Cube => signs(&|a, b, c| [a, b, c]),
            Solid::Octahedron => vec![
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ],
            Solid::Icosahedron => dedup(
                [
                    signs(&|_, b, c| [0.0, b, c * phi]),
                    signs(&|a, _, c| [a, c * phi, 0.0]),
                    signs(&|a, b, _| [a * phi, 0.0, b]),
                ]
                .concat(),
            ),
            Solid::Dodecahedron => dedup(
                [
                    signs(&|a, b, c| [a, b, c]),
                    signs(&|_, b, c| [0.0, b / phi, c * phi]),
                    signs(&|a, b, _| [a / phi, b * phi, 0.0]),
                    signs(&|a, _, c| [a * phi, 0.0, c / phi]),
                ]
                .concat(),
            ),
            Solid::Bucky => {
                // A third of the way along every edge of the icosahedron,
                // from both ends.
                let ico = Solid::Icosahedron.vertices();
                let mut v = Vec::new();
                for (i, a) in ico.iter().enumerate() {
                    for b in ico.iter().skip(i + 1) {
                        if (dist2(*a, *b) - 4.0).abs() < 1e-9 {
                            v.push(lerp(*a, *b, 1.0 / 3.0));
                            v.push(lerp(*b, *a, 1.0 / 3.0));
                        }
                    }
                }
                v
            }
        }
    }

    /// The faces as triangles, counter-clockwise from outside.
    pub fn triangles(&self) -> Vec<[[f64; 3]; 3]> {
        hull_triangles(&self.vertices())
    }

    /// The solid standing on one face, `height` mm tall, its centre
    /// over `at` on the field; `turn` degrees about its vertical axis.
    pub fn standing(&self, height: f64, at: [f64; 2], turn: f64) -> Vec<[[f64; 3]; 3]> {
        let tris = self.triangles();
        // Rest on the first face: rotate its outward normal to -z.
        let n = norm(cross(
            sub(tris[0][1], tris[0][0]),
            sub(tris[0][2], tris[0][0]),
        ));
        let r = rotation_between(n, [0.0, 0.0, -1.0]);
        let tris = transform(&tris, r, [0.0; 3]);
        let tris = transform(&tris, rotation([0.0, 0.0, 1.0], turn), [0.0; 3]);
        stand(&tris, height, at)
    }
}

fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    dot(sub(a, b), sub(a, b))
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The faces of the convex hull of `pts` as triangles: every plane
/// through three of them that has all the rest on one side is a face,
/// its vertices ordered round its normal and fanned.
pub fn hull_triangles(pts: &[[f64; 3]]) -> Vec<[[f64; 3]; 3]> {
    let n = pts.len();
    let mut planes: Vec<([f64; 3], f64)> = Vec::new();
    let mut tris = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let nn = cross(sub(pts[j], pts[i]), sub(pts[k], pts[i]));
                if dot(nn, nn).sqrt() < 1e-9 {
                    continue;
                }
                let mut nn = norm(nn);
                let mut d = dot(nn, pts[i]);
                let side: Vec<f64> = pts.iter().map(|p| dot(nn, *p) - d).collect();
                let (above, below) = (
                    side.iter().filter(|s| **s > 1e-9).count(),
                    side.iter().filter(|s| **s < -1e-9).count(),
                );
                if above > 0 && below > 0 {
                    continue;
                }
                if above > 0 {
                    nn = [-nn[0], -nn[1], -nn[2]];
                    d = -d;
                }
                if planes
                    .iter()
                    .any(|(m, e)| dist2(*m, nn) < 1e-9 && (e - d).abs() < 1e-9)
                {
                    continue;
                }
                planes.push((nn, d));
                let on: Vec<[f64; 3]> = pts
                    .iter()
                    .filter(|p| (dot(nn, **p) - d).abs() < 1e-9)
                    .copied()
                    .collect();
                let c = on.iter().fold([0.0; 3], |c, p| {
                    [
                        c[0] + p[0] / on.len() as f64,
                        c[1] + p[1] / on.len() as f64,
                        c[2] + p[2] / on.len() as f64,
                    ]
                });
                let u = norm(sub(on[0], c));
                let v = cross(nn, u);
                let mut ordered: Vec<(f64, [f64; 3])> = on
                    .iter()
                    .map(|p| {
                        let d = sub(*p, c);
                        (dot(d, v).atan2(dot(d, u)), *p)
                    })
                    .collect();
                ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
                for w in 1..ordered.len() - 1 {
                    tris.push([ordered[0].1, ordered[w].1, ordered[w + 1].1]);
                }
            }
        }
    }
    tris
}

/// The rotation of `degrees` about the unit `axis` (Rodrigues).
pub fn rotation(axis: [f64; 3], degrees: f64) -> [[f64; 3]; 3] {
    let a = norm(axis);
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

/// The rotation taking unit `from` to unit `to`.
pub fn rotation_between(from: [f64; 3], to: [f64; 3]) -> [[f64; 3]; 3] {
    let axis = cross(from, to);
    let s = dot(axis, axis).sqrt();
    let c = dot(from, to);
    if s < 1e-12 {
        if c > 0.0 {
            return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        }
        // Opposite: half a turn about any perpendicular.
        let perp = if from[0].abs() < 0.9 {
            norm(cross(from, [1.0, 0.0, 0.0]))
        } else {
            norm(cross(from, [0.0, 1.0, 0.0]))
        };
        return rotation(perp, 180.0);
    }
    rotation(axis, s.atan2(c).to_degrees())
}

pub fn rotate(r: [[f64; 3]; 3], p: [f64; 3]) -> [f64; 3] {
    [dot(r[0], p), dot(r[1], p), dot(r[2], p)]
}

/// Every vertex rotated by `r` then moved by `t`.
pub fn transform(tris: &[[[f64; 3]; 3]], r: [[f64; 3]; 3], t: [f64; 3]) -> Vec<[[f64; 3]; 3]> {
    tris.iter()
        .map(|tri| {
            let mut out = *tri;
            for p in out.iter_mut() {
                let q = rotate(r, *p);
                *p = [q[0] + t[0], q[1] + t[1], q[2] + t[2]];
            }
            out
        })
        .collect()
}

/// The centroid of a mesh's surface, each triangle's centre weighted
/// by its area: where a person centres a thing by eye.
pub fn surface_centroid(tris: &[[[f64; 3]; 3]]) -> [f64; 3] {
    let (mut c, mut total) = ([0.0; 3], 0.0);
    for t in tris {
        let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let area = dot(n, n).sqrt() / 2.0;
        for a in 0..3 {
            c[a] += area * (t[0][a] + t[1][a] + t[2][a]) / 3.0;
        }
        total += area;
    }
    [c[0] / total, c[1] / total, c[2] / total]
}

/// The mesh scaled to `height` tall and set on the field's plane with
/// its centroid over `at`.
pub fn stand(tris: &[[[f64; 3]; 3]], height: f64, at: [f64; 2]) -> Vec<[[f64; 3]; 3]> {
    let (lo, hi) = tris
        .iter()
        .flatten()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p[2]), h.max(p[2])));
    let s = height / (hi - lo);
    let scale = [[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]];
    settle(&transform(tris, scale, [0.0; 3]), at)
}

/// The mesh turned upside down (half a turn about a horizontal axis at
/// `azimuth` degrees), then turned `turn` degrees about the vertical,
/// and set back on the field with its middle over `at`.
pub fn upside_down(
    tris: &[[[f64; 3]; 3]],
    azimuth: f64,
    turn: f64,
    at: [f64; 2],
) -> Vec<[[f64; 3]; 3]> {
    let a = azimuth.to_radians();
    let flipped = transform(tris, rotation([a.cos(), a.sin(), 0.0], 180.0), [0.0; 3]);
    let turned = transform(&flipped, rotation([0.0, 0.0, 1.0], turn), [0.0; 3]);
    settle(&turned, at)
}

/// The mesh rolled a quarter turn onto its side about the horizontal
/// axis at `azimuth` degrees, turned `turn` about the vertical, and set
/// on the field with its middle over `at`.
pub fn on_side(
    tris: &[[[f64; 3]; 3]],
    azimuth: f64,
    turn: f64,
    at: [f64; 2],
) -> Vec<[[f64; 3]; 3]> {
    let a = azimuth.to_radians();
    let rolled = transform(tris, rotation([a.cos(), a.sin(), 0.0], 90.0), [0.0; 3]);
    let turned = transform(&rolled, rotation([0.0, 0.0, 1.0], turn), [0.0; 3]);
    settle(&turned, at)
}

/// The mesh moved so that it rests on z = 0 with its centroid over
/// `at`, its size unchanged.
pub fn settle(tris: &[[[f64; 3]; 3]], at: [f64; 2]) -> Vec<[[f64; 3]; 3]> {
    let lo = tris.iter().flatten().map(|p| p[2]).fold(f64::MAX, f64::min);
    let c = surface_centroid(tris);
    let eye = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    transform(tris, eye, [at[0] - c[0], at[1] - c[1], -lo])
}

/// The volume a closed mesh encloses (divergence theorem).
pub fn mesh_volume(tris: &[[[f64; 3]; 3]]) -> f64 {
    tris.iter()
        .map(|t| dot(t[0], cross(t[1], t[2])) / 6.0)
        .sum::<f64>()
        .abs()
}

/// The distance from a point to the nearest triangle of a mesh.
pub fn distance_to_mesh(p: [f64; 3], tris: &[[[f64; 3]; 3]]) -> f64 {
    tris.iter()
        .map(|t| dist2(p, closest_on_triangle(p, t)).sqrt())
        .fold(f64::INFINITY, f64::min)
}

/// Where a horizontal ray from `(centre, z)` at `azimuth` degrees
/// last leaves the mesh: the radius of the section at that height and
/// direction, as the scanner's profiles see it; None when the ray
/// meets nothing.
pub fn radius_at(tris: &[[[f64; 3]; 3]], centre: [f64; 2], z: f64, azimuth: f64) -> Option<f64> {
    let o = [centre[0], centre[1], z];
    let a = azimuth.to_radians();
    let d = [a.cos(), a.sin(), 0.0];
    tris.iter()
        .filter_map(|t| ray_triangle(o, d, t))
        .fold(None, |best: Option<f64>, hit| {
            Some(best.map_or(hit, |b| b.max(hit)))
        })
}

/// The distance along the ray to the triangle (Möller and Trumbore).
fn ray_triangle(o: [f64; 3], d: [f64; 3], t: &[[f64; 3]; 3]) -> Option<f64> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, t[0]);
    let u = dot(s, p) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let dist = dot(e2, q) * inv;
    (dist > 0.0).then_some(dist)
}

/// The closest point to `p` on the triangle (Ericson, Real-Time
/// Collision Detection 5.1.5).
fn closest_on_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> [f64; 3] {
    let (a, b, c) = (t[0], t[1], t[2]);
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return lerp(a, b, v);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return lerp(a, c, w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return lerp(b, c, w);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mushroom_closes_and_its_mesh_volume_is_its_formula() {
        let m = Mushroom {
            base: [210.0, 150.0],
            height: 100.0,
            stem: 18.0,
            cap: 48.0,
        };
        let tris = m.mesh(40, 72);
        let v = mesh_volume(&tris);
        assert!(
            (v - m.volume()).abs() / m.volume() < 0.005,
            "mesh {v} vs {}",
            m.volume()
        );
        let mut edges: std::collections::HashMap<(String, String), i32> = Default::default();
        let key = |p: [f64; 3]| format!("{:.6},{:.6},{:.6}", p[0], p[1], p[2]);
        for t in &tris {
            for e in 0..3 {
                let (a, b) = (key(t[e]), key(t[(e + 1) % 3]));
                *edges.entry((a.clone(), b.clone())).or_insert(0) += 1;
                *edges.entry((b, a)).or_insert(0) -= 1;
            }
        }
        assert!(
            edges.values().all(|v| *v == 0),
            "the mushroom is not closed"
        );
    }

    #[test]
    fn the_solids_close_with_their_textbook_face_counts_and_volumes() {
        // (faces, triangles, volume at the textbook coordinates)
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        let expect = [
            (Solid::Tetrahedron, 4, 4, 8.0 / 3.0),
            (Solid::Cube, 6, 12, 8.0),
            (Solid::Octahedron, 8, 8, 4.0 / 3.0),
            // Edge 2/phi: (15 + 7 sqrt5) / 4 · a^3.
            (
                Solid::Dodecahedron,
                12,
                36,
                (15.0 + 7.0 * 5f64.sqrt()) / 4.0 * (2.0 / phi).powi(3),
            ),
            // Edge 2: 5 (3 + sqrt5) / 12 · a^3.
            (
                Solid::Icosahedron,
                20,
                20,
                5.0 * (3.0 + 5f64.sqrt()) / 12.0 * 8.0,
            ),
            // Edge 2/3: (125 + 43 sqrt5) / 4 · a^3.
            (
                Solid::Bucky,
                32,
                12 * 3 + 20 * 4,
                (125.0 + 43.0 * 5f64.sqrt()) / 4.0 * (2.0f64 / 3.0).powi(3),
            ),
        ];
        for (solid, faces, ntris, volume) in expect {
            let tris = solid.triangles();
            assert_eq!(tris.len(), ntris, "{}", solid.name());
            let v = mesh_volume(&tris);
            assert!(
                (v - volume).abs() < 1e-9 * volume.max(1.0),
                "{} volume {v} vs {volume}",
                solid.name()
            );
            // Closed: every edge shared by two triangles in opposite directions.
            let mut edges: std::collections::HashMap<(String, String), i32> = Default::default();
            let key = |p: [f64; 3]| format!("{:.6},{:.6},{:.6}", p[0], p[1], p[2]);
            for t in &tris {
                for e in 0..3 {
                    let (a, b) = (key(t[e]), key(t[(e + 1) % 3]));
                    *edges.entry((a.clone(), b.clone())).or_insert(0) += 1;
                    *edges.entry((b, a)).or_insert(0) -= 1;
                }
            }
            assert!(
                edges.values().all(|v| *v == 0),
                "{} is not closed",
                solid.name()
            );
            let _ = faces;
            // Standing: on the table, the asked height, centred.
            let s = solid.standing(100.0, [210.0, 150.0], 17.0);
            let zs: Vec<f64> = s.iter().flatten().map(|p| p[2]).collect();
            let (lo, hi) = zs
                .iter()
                .fold((f64::MAX, f64::MIN), |(l, h), z| (l.min(*z), h.max(*z)));
            assert!(
                lo.abs() < 1e-9 && (hi - 100.0).abs() < 1e-9,
                "{} stands {lo} to {hi}",
                solid.name()
            );
            // On a face: at least three vertices on the table.
            let on_table = s.iter().flatten().filter(|p| p[2].abs() < 1e-9).count();
            assert!(
                on_table >= 3,
                "{} rests on {on_table} vertex slots",
                solid.name()
            );
            let c = surface_centroid(&s);
            assert!(
                (c[0] - 210.0).abs() < 1e-9 && (c[1] - 150.0).abs() < 1e-9,
                "{} centred at {c:?}",
                solid.name()
            );
            let d = distance_to_mesh([210.0, 150.0, 50.0], &s);
            assert!(
                d > 3.0 && d < 60.0,
                "{} centre to surface {d}",
                solid.name()
            );
        }
    }
}
