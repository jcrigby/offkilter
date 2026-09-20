//! Headless renders of a tab, so a script or a language model can look
//! at what it built without a browser.
//!
//! The renderer is a plain software rasteriser: an orthographic camera
//! fitted to the bodies, a depth buffer, Lambert shading with the light
//! over the viewer's shoulder, the display edges (between distinct
//! surfaces) drawn on top, and an axis triad in the corner. An optional
//! section plane discards everything on one side of an axis-aligned
//! plane, the same clip the client's section view applies, and shades
//! the interior it exposes darker.

use ok_brep::DisplayEdge;
use ok_math::Vec3;
use ok_mesh::TriMesh;
use ok_model::{Body, Document, TabId};

mod png;
mod raster;

pub use raster::{Image, Rgb};

/// The standard viewpoints of the client, plus any direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    /// Looking down the Z axis, Y up the page.
    Top,
    /// Looking along +Y, Z up the page.
    Front,
    /// Looking along -X, Z up the page.
    Right,
    /// The client's isometric-style view from +X, -Y, +Z.
    Iso,
    /// Any eye direction (from the model towards the eye); Z is up unless
    /// the direction is vertical.
    Direction(Vec3),
}

impl View {
    /// Parses the names the client uses; `"x,y,z"` gives a direction.
    pub fn parse(name: &str) -> Result<View, String> {
        match name.trim().to_lowercase().as_str() {
            "" | "iso" | "isometric" => Ok(View::Iso),
            "top" => Ok(View::Top),
            "front" => Ok(View::Front),
            "right" => Ok(View::Right),
            other => {
                let parts: Vec<f64> = other
                    .split(',')
                    .map(|p| p.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| {
                        format!("unknown view {name:?}: use top, front, right, iso or x,y,z")
                    })?;
                if parts.len() != 3 {
                    return Err(format!("view direction {name:?} needs three numbers"));
                }
                let d = Vec3::new(parts[0], parts[1], parts[2]);
                d.normalized()
                    .map(View::Direction)
                    .ok_or_else(|| "view direction must not be zero".to_string())
            }
        }
    }

    /// Unit vector from the model towards the eye, and the page-up vector.
    pub fn frame(self) -> (Vec3, Vec3) {
        match self {
            View::Top => (Vec3::Z, Vec3::Y),
            View::Front => (Vec3::new(0.0, -1.0, 0.0), Vec3::Z),
            View::Right => (Vec3::X, Vec3::Z),
            View::Iso => (
                Vec3::new(0.6, -0.7, 0.5).normalized().unwrap_or(Vec3::Z),
                Vec3::Z,
            ),
            View::Direction(d) => {
                let up = if d.cross(Vec3::Z).length() < 1e-6 {
                    Vec3::Y
                } else {
                    Vec3::Z
                };
                (d, up)
            }
        }
    }
}

/// An axis-aligned clipping plane: keeps the side where `axis · p >= offset`,
/// or the other side when `flip` is set (the client's section view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub axis: Vec3,
    pub offset: f64,
    pub flip: bool,
}

impl Section {
    /// Parses `"z:10"` or `"x:-5:flip"`.
    pub fn parse(text: &str) -> Result<Section, String> {
        let parts: Vec<&str> = text.split(':').map(str::trim).collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(format!(
                "section {text:?}: use axis:offset[:flip], e.g. z:10"
            ));
        }
        let axis = match parts[0].to_lowercase().as_str() {
            "x" => Vec3::X,
            "y" => Vec3::Y,
            "z" => Vec3::Z,
            other => return Err(format!("section axis {other:?}: use x, y or z")),
        };
        let offset: f64 = parts[1]
            .parse()
            .map_err(|_| format!("section offset {:?} is not a number", parts[1]))?;
        let flip = match parts.get(2).map(|f| f.to_lowercase()) {
            None => false,
            Some(f) if f == "flip" || f == "true" => true,
            Some(f) if f == "false" => false,
            Some(f) => return Err(format!("section flag {f:?}: use flip")),
        };
        Ok(Section { axis, offset, flip })
    }

    fn keeps(&self, p: Vec3) -> bool {
        let d = self.axis.dot(p) - self.offset;
        if self.flip {
            d <= 0.0
        } else {
            d >= 0.0
        }
    }
}

/// What to render and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub width: usize,
    pub height: usize,
    pub view: View,
    pub section: Option<Section>,
    /// Draw the display edges of the bodies.
    pub edges: bool,
    /// Draw the axis triad in the lower left corner.
    pub triad: bool,
    /// Fit the camera to this box instead of the items' extent.
    pub fit: Option<(Vec3, Vec3)>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            width: 800,
            height: 600,
            view: View::Iso,
            section: None,
            edges: true,
            triad: true,
            fit: None,
        }
    }
}

/// Largest image either side may be.
pub const MAX_SIZE: usize = 2048;

/// One body to draw: its mesh, its edges and its colour, plus the faces
/// a section plane cut through it (drawn hatched).
#[derive(Debug, Clone, Default)]
pub struct Item {
    pub mesh: TriMesh,
    pub edges: Vec<DisplayEdge>,
    pub colour: Rgb,
    pub cap: TriMesh,
}

impl Default for Rgb {
    fn default() -> Self {
        PALETTE[0]
    }
}

/// The body colours, cycled in body order.
pub const PALETTE: [Rgb; 6] = [
    Rgb(0x9a, 0xb4, 0xd0),
    Rgb(0xd0, 0xb0, 0x8a),
    Rgb(0x9c, 0xc8, 0xa4),
    Rgb(0xc8, 0x9c, 0xb8),
    Rgb(0xd4, 0xc4, 0x8c),
    Rgb(0x9c, 0xb8, 0xc0),
];

/// The body as drawn whole.
pub fn item_of(body: &Body, index: usize) -> Item {
    Item {
        mesh: body.mesh.clone(),
        edges: body.edges.clone(),
        colour: PALETTE[index % PALETTE.len()],
        cap: TriMesh::new(),
    }
}

/// The body with the section applied: the kept half is a closed solid
/// again, so the cut faces show as hatched caps rather than a hollow
/// interior. A body the cut fails on is drawn whole; one entirely on the
/// removed side is left out.
pub fn cut_item(body: &Body, index: usize, section: &Section) -> Option<Item> {
    let plane = ok_math::Plane::from_origin_normal(section.axis * section.offset, section.axis)?;
    let level = section.offset;
    let half = match ok_brep::split(&body.solid, &plane) {
        Ok(Some((below, above))) => {
            if section.flip {
                below
            } else {
                above
            }
        }
        Ok(None) => {
            let inside = body.solid.vertices.iter().any(|v| section.keeps(*v));
            return inside.then(|| item_of(body, index));
        }
        Err(_) => return Some(item_of(body, index)),
    };
    let (mesh, faces) = ok_brep::tessellate_with_faces(&half);
    let extent = half
        .bounds()
        .map_or(1.0, |(lo, hi)| (hi - lo).length().max(1.0));
    let on_cut: Vec<bool> = half
        .faces
        .iter()
        .map(|f| {
            f.plane.normal.dot(section.axis).abs() > 1.0 - 1e-6
                && (f.plane.normal.dot(f.plane.origin) * f.plane.normal.dot(section.axis) - level)
                    .abs()
                    <= 1e-6 * extent
        })
        .collect();
    let mut body_mesh = TriMesh::new();
    let mut cap = TriMesh::new();
    for (t, tri) in mesh.indices.chunks_exact(3).enumerate() {
        let target = if on_cut[faces[t] as usize] {
            &mut cap
        } else {
            &mut body_mesh
        };
        let p = |i: u32| {
            let k = i as usize * 3;
            Vec3::new(
                mesh.positions[k] as f64,
                mesh.positions[k + 1] as f64,
                mesh.positions[k + 2] as f64,
            )
        };
        let n = |i: u32| {
            let k = i as usize * 3;
            [mesh.normals[k], mesh.normals[k + 1], mesh.normals[k + 2]]
        };
        let base = target.vertex_count() as u32;
        for &i in tri {
            let q = p(i);
            target
                .positions
                .extend([q.x as f32, q.y as f32, q.z as f32]);
            target.normals.extend(n(i));
        }
        target.indices.extend([base, base + 1, base + 2]);
    }
    Some(Item {
        mesh: body_mesh,
        edges: ok_brep::display_edges(&half),
        colour: PALETTE[index % PALETTE.len()],
        cap,
    })
}

/// Renders the items into an image.
pub fn render(items: &[Item], options: &Options) -> Result<Image, String> {
    if options.width == 0 || options.height == 0 {
        return Err("image size must be positive".into());
    }
    if options.width > MAX_SIZE || options.height > MAX_SIZE {
        return Err(format!("image size is limited to {MAX_SIZE} pixels a side"));
    }
    Ok(raster::render(items, options))
}

/// Regenerates a tab and renders its bodies to a PNG.
pub fn screenshot(doc: &mut Document, tab: TabId, options: &Options) -> Result<Vec<u8>, String> {
    let kind = doc.tab(tab).map(|t| t.kind_name()).ok_or("no such tab")?;
    let bodies = if kind == "assembly" {
        doc.regenerate_assembly(tab)
            .map_err(|e| e.to_string())?
            .bodies
    } else {
        doc.regenerate_studio(tab, None)
            .map_err(|e| e.to_string())?
            .bodies
    };
    let (items, options) = match &options.section {
        Some(section) => {
            let items = bodies
                .iter()
                .enumerate()
                .filter_map(|(i, b)| cut_item(b, i, section))
                .collect();
            // The bodies are cut already; the fit still covers what was removed
            // so the picture does not jump as the plane moves.
            let mut o = options.clone();
            o.section = None;
            o.fit = bodies.iter().flat_map(|b| b.mesh.bounds()).fold(
                None,
                |acc: Option<(Vec3, Vec3)>, (lo, hi)| {
                    Some(match acc {
                        None => (lo, hi),
                        Some((a, b)) => (
                            Vec3::new(a.x.min(lo.x), a.y.min(lo.y), a.z.min(lo.z)),
                            Vec3::new(b.x.max(hi.x), b.y.max(hi.y), b.z.max(hi.z)),
                        ),
                    })
                },
            );
            (items, o)
        }
        None => (
            bodies
                .iter()
                .enumerate()
                .map(|(i, b)| item_of(b, i))
                .collect::<Vec<Item>>(),
            options.clone(),
        ),
    };
    let image = render(&items, &options)?;
    Ok(png::encode(&image))
}

/// Encodes an image as PNG.
pub fn to_png(image: &Image) -> Vec<u8> {
    png::encode(image)
}

/// Decodes a PNG produced by [`to_png`] (only that subset: 8-bit RGB,
/// no interlace), for tests and round trips.
pub fn from_png(bytes: &[u8]) -> Result<Image, String> {
    png::decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(size: f64) -> TriMesh {
        let s = size;
        let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        let mut m = TriMesh::new();
        // bottom (z=0), top (z=s), and four sides, CCW from outside.
        m.push_quad(
            p(0.0, 0.0, 0.0),
            p(0.0, s, 0.0),
            p(s, s, 0.0),
            p(s, 0.0, 0.0),
        );
        m.push_quad(p(0.0, 0.0, s), p(s, 0.0, s), p(s, s, s), p(0.0, s, s));
        m.push_quad(
            p(0.0, 0.0, 0.0),
            p(s, 0.0, 0.0),
            p(s, 0.0, s),
            p(0.0, 0.0, s),
        );
        m.push_quad(p(s, 0.0, 0.0), p(s, s, 0.0), p(s, s, s), p(s, 0.0, s));
        m.push_quad(p(s, s, 0.0), p(0.0, s, 0.0), p(0.0, s, s), p(s, s, s));
        m.push_quad(
            p(0.0, s, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, s),
            p(0.0, s, s),
        );
        assert!(m.signed_volume() > 0.0);
        m
    }

    fn opts(view: View) -> Options {
        Options {
            width: 200,
            height: 100,
            view,
            section: None,
            edges: false,
            triad: false,
            fit: None,
        }
    }

    fn item(mesh: TriMesh, colour: Rgb) -> Item {
        Item {
            mesh,
            colour,
            ..Item::default()
        }
    }

    fn painted(img: &Image) -> usize {
        (0..img.width * img.height)
            .filter(|&i| img.pixel(i % img.width, i / img.width) != raster::BACKGROUND)
            .count()
    }

    #[test]
    fn a_cube_from_the_top_fills_the_fitted_square() {
        let m = cube(10.0);
        let items = [item(m, PALETTE[0])];
        let img = render(&items, &opts(View::Top)).unwrap();
        // Fitted to the shorter side with a 6% margin each side: about 88 px square.
        let n = painted(&img);
        let side = (n as f64).sqrt();
        assert!((side - 88.0).abs() < 3.0, "painted {n} px, side {side}");
        // One face is seen, so one shade.
        let mut shades = std::collections::BTreeSet::new();
        for y in 0..img.height {
            for x in 0..img.width {
                let c = img.pixel(x, y);
                if c != raster::BACKGROUND {
                    shades.insert(c);
                }
            }
        }
        assert_eq!(shades.len(), 1, "{shades:?}");
    }

    #[test]
    fn the_iso_view_shows_three_faces_in_three_shades() {
        let m = cube(10.0);
        let items = [item(m, PALETTE[0])];
        let img = render(&items, &opts(View::Iso)).unwrap();
        let mut shades = std::collections::BTreeMap::new();
        for y in 0..img.height {
            for x in 0..img.width {
                let c = img.pixel(x, y);
                if c != raster::BACKGROUND {
                    *shades.entry(c).or_insert(0usize) += 1;
                }
            }
        }
        assert_eq!(shades.len(), 3, "{shades:?}");
        // The top face is the brightest and, from (0.6, -0.7, 0.5), the
        // three visible faces have areas in proportion to |dir · normal|.
        let total: usize = shades.values().sum();
        let brightest = shades
            .keys()
            .max_by_key(|c| c.0 as u32 + c.1 as u32 + c.2 as u32)
            .unwrap();
        let top_share = shades[brightest] as f64 / total as f64;
        assert!(
            (top_share - 0.5 / 1.8).abs() < 0.03,
            "top share {top_share}"
        );
    }

    #[test]
    fn a_section_removes_one_side_and_shades_the_inside() {
        let m = cube(10.0);
        let items = [item(m, PALETTE[0])];
        let whole = painted(&render(&items, &opts(View::Front)).unwrap());
        let mut o = opts(View::Front);
        o.section = Some(Section::parse("z:5").unwrap());
        let half = painted(&render(&items, &o).unwrap());
        // The fit is computed before clipping, so the kept half is half the pixels.
        assert!(
            (half as f64 / whole as f64 - 0.5).abs() < 0.03,
            "{half} of {whole}"
        );
        o.section = Some(Section::parse("y:5").unwrap());
        let img = render(&items, &o).unwrap();
        // Looking along +Y with the near half removed, the far wall's inside shows.
        assert!((painted(&img) as f64 / whole as f64 - 1.0).abs() < 0.03);
        let c = img.pixel(img.width / 2, img.height / 2);
        assert!(
            c != raster::BACKGROUND && c.0 < PALETTE[0].0,
            "inside shade {c:?}"
        );
    }

    #[test]
    fn png_round_trips() {
        let m = cube(10.0);
        let edges = vec![DisplayEdge {
            points: [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
            faces: [0, 1],
        }];
        let items = [Item {
            mesh: m,
            edges,
            colour: PALETTE[1],
            cap: TriMesh::new(),
        }];
        let mut o = opts(View::Iso);
        o.edges = true;
        o.triad = true;
        let img = render(&items, &o).unwrap();
        let bytes = to_png(&img);
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let back = from_png(&bytes).unwrap();
        assert_eq!(back, img);
    }

    #[test]
    fn views_and_sections_parse() {
        assert_eq!(View::parse("TOP").unwrap(), View::Top);
        assert_eq!(View::parse("").unwrap(), View::Iso);
        assert!(
            matches!(View::parse("1,0,0").unwrap(), View::Direction(d) if (d.x - 1.0).abs() < 1e-12)
        );
        assert!(View::parse("back").is_err());
        let s = Section::parse("X:-2.5:flip").unwrap();
        assert_eq!(
            s,
            Section {
                axis: Vec3::X,
                offset: -2.5,
                flip: true
            }
        );
        assert!(Section::parse("w:1").is_err());
        assert!(!s.keeps(Vec3::X));
        assert!(s.keeps(Vec3::X * -3.0));
    }

    fn block(depth: f64) -> (Document, TabId) {
        let mut doc = Document::new("shot");
        let tab = doc.tabs[0].id;
        let ops = serde_json::json!([
            { "type": "studio", "tab": tab.0, "op": { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": null } },
            { "type": "studio", "tab": tab.0, "op": { "type": "sketch", "id": 1, "op": { "type": "add_rectangle", "a": { "x": 0, "y": 0 }, "b": { "x": 20, "y": 10 } } } },
            { "type": "studio", "tab": tab.0, "op": { "type": "add_extrude", "sketch": 1, "depth": depth, "name": null } }
        ]);
        let ops: Vec<ok_model::DocOp> = serde_json::from_value(ops).unwrap();
        doc.apply_all(ops).unwrap();
        (doc, tab)
    }

    #[test]
    fn a_document_tab_renders_to_a_png() {
        let (mut doc, tab) = block(5.0);
        let png = screenshot(
            &mut doc,
            tab,
            &Options {
                width: 120,
                height: 90,
                ..Options::default()
            },
        )
        .unwrap();
        let img = from_png(&png).unwrap();
        assert_eq!((img.width, img.height), (120, 90));
        assert!(painted(&img) > 1000);
    }

    #[test]
    fn a_sectioned_tab_caps_the_cut_with_hatching() {
        let (mut doc, tab) = block(10.0);
        let mut o = Options {
            width: 200,
            height: 100,
            edges: false,
            triad: false,
            ..Options::default()
        };
        o.view = View::Top;
        let whole = painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap());
        // Keep the lower part so the cap faces the camera.
        o.section = Some(Section::parse("z:4:flip").unwrap());
        let img = from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap();
        // From above, the cap covers the same footprint, in the two hatch shades.
        assert!((painted(&img) as f64 / whole as f64 - 1.0).abs() < 0.02);
        let mut shades = std::collections::BTreeSet::new();
        for y in 0..img.height {
            for x in 0..img.width {
                let c = img.pixel(x, y);
                if c != raster::BACKGROUND {
                    shades.insert(c);
                }
            }
        }
        assert_eq!(shades.len(), 2, "{shades:?}");
        // The kept half is above the plane: from the front it is 60% as tall.
        o.view = View::Front;
        o.section = None;
        let whole = painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap());
        o.section = Some(Section::parse("z:4").unwrap());
        let kept = painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap());
        assert!(
            (kept as f64 / whole as f64 - 0.6).abs() < 0.03,
            "{kept} of {whole}"
        );
        // Flipped, the lower 40% stays; a plane clear of the body keeps all or nothing.
        o.section = Some(Section::parse("z:4:flip").unwrap());
        let kept = painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap());
        assert!(
            (kept as f64 / whole as f64 - 0.4).abs() < 0.03,
            "{kept} of {whole}"
        );
        o.section = Some(Section::parse("z:-1").unwrap());
        assert_eq!(
            painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap()),
            whole
        );
        o.section = Some(Section::parse("z:-1:flip").unwrap());
        assert_eq!(
            painted(&from_png(&screenshot(&mut doc, tab, &o).unwrap()).unwrap()),
            0
        );
    }
}
