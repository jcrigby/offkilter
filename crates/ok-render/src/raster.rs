//! The rasteriser: orthographic projection, depth buffer, Lambert
//! shading, edges and outlines.

use crate::{Item, Options};
use ok_math::Vec3;

/// A colour, 8 bits a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn scaled(self, k: f64) -> Rgb {
        let f = |c: u8| ((c as f64) * k).round().clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }
}

pub const BACKGROUND: Rgb = Rgb(0xff, 0xff, 0xff);
const EDGE: Rgb = Rgb(0x2c, 0x30, 0x3a);
const TRIAD: [Rgb; 3] = [
    Rgb(0xd0, 0x40, 0x40),
    Rgb(0x40, 0xa0, 0x40),
    Rgb(0x40, 0x60, 0xd0),
];
/// Margin left around the fitted bodies, as a fraction of each side.
const MARGIN: f64 = 0.06;

/// An RGB image, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// `width * height * 3` bytes.
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize, fill: Rgb) -> Image {
        let mut data = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            data.extend([fill.0, fill.1, fill.2]);
        }
        Image {
            width,
            height,
            data,
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Rgb {
        let i = (y * self.width + x) * 3;
        Rgb(self.data[i], self.data[i + 1], self.data[i + 2])
    }

    pub fn set(&mut self, x: usize, y: usize, c: Rgb) {
        let i = (y * self.width + x) * 3;
        self.data[i] = c.0;
        self.data[i + 1] = c.1;
        self.data[i + 2] = c.2;
    }
}

/// The camera: an orthonormal frame and a fit of world units to pixels.
struct Camera {
    x: Vec3,
    y: Vec3,
    z: Vec3,
    centre: Vec3,
    scale: f64,
    half_w: f64,
    half_h: f64,
    /// Extent of the bodies along the view, for depth tolerances.
    depth_range: f64,
}

impl Camera {
    fn new(options: &Options, points: impl Iterator<Item = Vec3>) -> Camera {
        let (eye, up) = options.view.frame();
        let z = eye;
        let x = up
            .cross(z)
            .normalized()
            .or_else(|| Vec3::Y.cross(z).normalized())
            .or_else(|| Vec3::X.cross(z).normalized())
            .unwrap_or(Vec3::X);
        let y = z.cross(x);
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        let mut any = false;
        for p in points {
            any = true;
            let s = Vec3::new(p.dot(x), p.dot(y), p.dot(z));
            lo = Vec3::new(lo.x.min(s.x), lo.y.min(s.y), lo.z.min(s.z));
            hi = Vec3::new(hi.x.max(s.x), hi.y.max(s.y), hi.z.max(s.z));
        }
        if !any {
            lo = Vec3::ZERO;
            hi = Vec3::ZERO;
        }
        let w = options.width as f64;
        let h = options.height as f64;
        let dx = (hi.x - lo.x).max(1e-9);
        let dy = (hi.y - lo.y).max(1e-9);
        let fit = ((w * (1.0 - 2.0 * MARGIN)) / dx).min((h * (1.0 - 2.0 * MARGIN)) / dy);
        let scale = if any && fit.is_finite() { fit } else { 1.0 };
        let mid = (lo + hi) * 0.5;
        Camera {
            x,
            y,
            z,
            centre: x * mid.x + y * mid.y + z * mid.z,
            scale,
            half_w: w / 2.0,
            half_h: h / 2.0,
            depth_range: (hi.z - lo.z).max(1e-9),
        }
    }

    /// Screen position (pixels, y down) and depth (larger is nearer).
    fn project(&self, p: Vec3) -> (f64, f64, f64) {
        let d = p - self.centre;
        (
            self.half_w + d.dot(self.x) * self.scale,
            self.half_h - d.dot(self.y) * self.scale,
            d.dot(self.z),
        )
    }
}

struct Buffers {
    image: Image,
    depth: Vec<f64>,
    /// Item index + 1 of the nearest fragment, 0 for none.
    owner: Vec<u32>,
}

pub(crate) fn render(items: &[Item], options: &Options) -> Image {
    let camera = match options.fit {
        Some((lo, hi)) => Camera::new(
            options,
            (0..8)
                .map(|k| {
                    Vec3::new(
                        if k & 1 == 0 { lo.x } else { hi.x },
                        if k & 2 == 0 { lo.y } else { hi.y },
                        if k & 4 == 0 { lo.z } else { hi.z },
                    )
                })
                .collect::<Vec<_>>()
                .into_iter(),
        ),
        None => Camera::new(
            options,
            items.iter().flat_map(|it| {
                it.mesh
                    .positions
                    .chunks_exact(3)
                    .chain(it.cap.positions.chunks_exact(3))
                    .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
            }),
        ),
    };
    let (w, h) = (options.width, options.height);
    let mut buf = Buffers {
        image: Image::new(w, h, BACKGROUND),
        depth: vec![f64::NEG_INFINITY; w * h],
        owner: vec![0; w * h],
    };
    let light = (camera.x * 0.2 + camera.y * 0.4 + camera.z * 0.9)
        .normalized()
        .unwrap_or(camera.z);
    for (index, item) in items.iter().enumerate() {
        shade_mesh(
            &mut buf,
            &camera,
            &item.mesh,
            item.colour,
            false,
            index as u32 + 1,
            light,
            options,
        );
        shade_mesh(
            &mut buf,
            &camera,
            &item.cap,
            item.colour,
            true,
            index as u32 + 1,
            light,
            options,
        );
    }
    if options.edges {
        outline(&mut buf, &camera);
        for item in items {
            for edge in &item.edges {
                draw_edge(&mut buf, &camera, edge.points, options);
            }
        }
    }
    if options.triad {
        triad(&mut buf.image, &camera);
    }
    buf.image
}

fn vertex(mesh: &ok_mesh::TriMesh, i: u32) -> (Vec3, Vec3) {
    let k = i as usize * 3;
    let p = Vec3::new(
        mesh.positions[k] as f64,
        mesh.positions[k + 1] as f64,
        mesh.positions[k + 2] as f64,
    );
    let n = Vec3::new(
        mesh.normals[k] as f64,
        mesh.normals[k + 1] as f64,
        mesh.normals[k + 2] as f64,
    );
    (p, n)
}

/// Rasterises one mesh. A `hatched` mesh (the caps of a section) is
/// drawn in diagonal stripes of two tones so the cut reads as a cut.
#[allow(clippy::too_many_arguments)]
fn shade_mesh(
    buf: &mut Buffers,
    camera: &Camera,
    mesh: &ok_mesh::TriMesh,
    colour: Rgb,
    hatched: bool,
    owner: u32,
    light: Vec3,
    options: &Options,
) {
    let (w, h) = (buf.image.width, buf.image.height);
    for tri in mesh.indices.chunks_exact(3) {
        let (pa, na) = vertex(mesh, tri[0]);
        let (pb, nb) = vertex(mesh, tri[1]);
        let (pc, nc) = vertex(mesh, tri[2]);
        if let Some(s) = &options.section {
            if !s.keeps(pa) && !s.keeps(pb) && !s.keeps(pc) {
                continue;
            }
        }
        let geometric = (pb - pa).cross(pc - pa);
        let facing = geometric.dot(camera.z);
        if facing == 0.0 {
            continue;
        }
        let back = facing < 0.0;
        if back && options.section.is_none() {
            continue;
        }
        let (ax, ay, az) = camera.project(pa);
        let (bx, by, bz) = camera.project(pb);
        let (cx, cy, cz) = camera.project(pc);
        let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        if area.abs() < 1e-12 {
            continue;
        }
        let x0 = ax.min(bx).min(cx).floor().max(0.0) as usize;
        let x1 = (ax.max(bx).max(cx).ceil() as isize).min(w as isize - 1);
        let y0 = ay.min(by).min(cy).floor().max(0.0) as usize;
        let y1 = (ay.max(by).max(cy).ceil() as isize).min(h as isize - 1);
        if x1 < 0 || y1 < 0 {
            continue;
        }
        for py in y0..=y1 as usize {
            let sy = py as f64 + 0.5;
            for px in x0..=x1 as usize {
                let sx = px as f64 + 0.5;
                let wa = ((bx - sx) * (cy - sy) - (by - sy) * (cx - sx)) / area;
                let wb = ((cx - sx) * (ay - sy) - (cy - sy) * (ax - sx)) / area;
                let wc = 1.0 - wa - wb;
                if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                    continue;
                }
                let depth = wa * az + wb * bz + wc * cz;
                let i = py * w + px;
                if depth <= buf.depth[i] {
                    continue;
                }
                if let Some(s) = &options.section {
                    let p = pa * wa + pb * wb + pc * wc;
                    if !s.keeps(p) {
                        continue;
                    }
                }
                let mut n = (na * wa + nb * wb + nc * wc)
                    .normalized()
                    .unwrap_or(geometric.normalized().unwrap_or(camera.z));
                if back {
                    n = -n;
                }
                let lambert = n.dot(light).max(0.0);
                let mut intensity = 0.35 + 0.65 * lambert;
                if back {
                    intensity *= 0.6;
                }
                if hatched {
                    intensity = if (px + py) / 4 % 2 == 0 { 0.55 } else { 0.85 };
                }
                buf.depth[i] = depth;
                buf.owner[i] = owner;
                buf.image.set(px, py, colour.scaled(intensity));
            }
        }
    }
}

/// Darkens pixels where the nearest body changes or the depth jumps, so
/// silhouettes (a cylinder's rim, one body in front of another) read
/// without relying on the display edges.
fn outline(buf: &mut Buffers, camera: &Camera) {
    let (w, h) = (buf.image.width, buf.image.height);
    let jump = camera.depth_range * 0.03;
    let mut marks = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if buf.owner[i] == 0 {
                continue;
            }
            let neighbours = [
                (x + 1 < w).then(|| i + 1),
                (y + 1 < h).then(|| i + w),
                (x > 0).then(|| i - 1),
                (y > 0).then(|| i - w),
            ];
            for j in neighbours.into_iter().flatten() {
                let other_body = buf.owner[j] != buf.owner[i];
                let nearer = buf.depth[i] > buf.depth[j] + jump;
                if (other_body && buf.depth[i] >= buf.depth[j]) || nearer {
                    marks.push(i);
                    break;
                }
            }
        }
    }
    for i in marks {
        buf.image.set(i % w, i / w, EDGE);
    }
}

fn draw_edge(buf: &mut Buffers, camera: &Camera, points: [Vec3; 2], options: &Options) {
    let (mut a, mut b) = (points[0], points[1]);
    if let Some(s) = &options.section {
        let (ka, kb) = (s.keeps(a), s.keeps(b));
        match (ka, kb) {
            (false, false) => return,
            (true, true) => {}
            _ => {
                let da = s.axis.dot(a) - s.offset;
                let db = s.axis.dot(b) - s.offset;
                let t = da / (da - db);
                let cut = a + (b - a) * t;
                if ka {
                    b = cut;
                } else {
                    a = cut;
                }
            }
        }
    }
    let (ax, ay, az) = camera.project(a);
    let (bx, by, bz) = camera.project(b);
    let bias = camera.depth_range * 0.004;
    let steps = (bx - ax).abs().max((by - ay).abs()).ceil().max(1.0) as usize;
    let (w, h) = (buf.image.width as f64, buf.image.height as f64);
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let x = ax + (bx - ax) * t;
        let y = ay + (by - ay) * t;
        let z = az + (bz - az) * t;
        if x < 0.0 || y < 0.0 || x >= w || y >= h {
            continue;
        }
        let (px, py) = (x as usize, y as usize);
        let i = py * buf.image.width + px;
        if z + bias >= buf.depth[i] {
            buf.image.set(px, py, EDGE);
        }
    }
}

fn line(image: &mut Image, from: (f64, f64), to: (f64, f64), colour: Rgb) {
    let steps = (to.0 - from.0)
        .abs()
        .max((to.1 - from.1).abs())
        .ceil()
        .max(1.0) as usize;
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let x = from.0 + (to.0 - from.0) * t;
        let y = from.1 + (to.1 - from.1) * t;
        if x >= 0.0 && y >= 0.0 && x < image.width as f64 && y < image.height as f64 {
            image.set(x as usize, y as usize, colour);
        }
    }
}

/// The world axes projected into the lower left corner.
fn triad(image: &mut Image, camera: &Camera) {
    let size = (image.width.min(image.height) as f64 * 0.06).clamp(10.0, 28.0);
    let origin = (size + 8.0, image.height as f64 - size - 8.0);
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    // Draw the axis pointing away from the eye first so nearer ones win.
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by(|&a, &b| {
        axes[a]
            .dot(camera.z)
            .partial_cmp(&axes[b].dot(camera.z))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for k in order {
        let axis = axes[k];
        let dx = axis.dot(camera.x) * size;
        let dy = -axis.dot(camera.y) * size;
        let tip = (origin.0 + dx, origin.1 + dy);
        line(image, origin, tip, TRIAD[k]);
        // A second pixel of width so the triad survives scaling down.
        line(
            image,
            (origin.0 + 1.0, origin.1),
            (tip.0 + 1.0, tip.1),
            TRIAD[k],
        );
    }
}
