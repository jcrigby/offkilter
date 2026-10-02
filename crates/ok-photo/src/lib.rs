//! Measuring from photographs. `sheet_pdf` prints a reference sheet: a
//! light millimetre grid with four black bullseye fiducials in the
//! corners (the origin's has an extra ring, so the sheet's orientation
//! is known) and a 100 mm bar to check the print scale. `measure` takes
//! a photograph of parts lying on that sheet, finds the fiducials,
//! solves the homography that maps the picture onto the sheet's
//! millimetres, rectifies the picture, and segments the dark shapes on
//! the grid into parts with a bounding box, area, outline and any holes,
//! all in millimetres from the origin fiducial. Good enough to rough out
//! a part and to point at when asking for a real measurement: the
//! silhouette is of the top face, and anything with height is shifted
//! by parallax unless the camera looked straight down.
//!
//! Pure Rust: bytes in, numbers and a PNG out.

use ok_render::{Image, Rgb};
use ok_sheet::pdf::{Anchor, Page};
pub use ok_sheet::SheetSize;
use serde::Serialize;
use std::f64::consts::PI;

/// Fiducial centres sit this far in from the page edges.
pub const INSET: f64 = 25.0;
/// The bullseye: outer ring radius, its hole, the dot; the origin's
/// extra ring outside it.
const RING_R: f64 = 8.0;
const RING_HOLE_R: f64 = 5.0;
const DOT_R: f64 = 2.5;
const ORIGIN_RING_R: f64 = 11.5;
const ORIGIN_RING_HOLE_R: f64 = 10.0;
/// The size code: a row of dots below the x axis by the origin mark,
/// one per step in `SheetSize::code`, so a photograph says which sheet
/// it is of.
const CODE_DOT_R: f64 = 1.5;
const CODE_X0: f64 = 18.0;
const CODE_PITCH: f64 = 5.0;
const CODE_Y: f64 = -9.0;
/// Pixels per millimetre of the rectified picture.
const SCALE: f64 = 4.0;
/// The rectified picture reaches this far outside the fiducials.
const BORDER: f64 = 15.0;

/// Fiducial spacing on a page: the sheet frame's x and y extent.
pub fn span(size: SheetSize) -> (f64, f64) {
    let (w, h) = size.size();
    (w - 2.0 * INSET, h - 2.0 * INSET)
}

/// The sizes in the order the code dots count them: one dot is A4.
const CODED: [SheetSize; 5] = [
    SheetSize::A4,
    SheetSize::Letter,
    SheetSize::A3,
    SheetSize::A2,
    SheetSize::Tabloid,
];

/// How many code dots the sheet of this size carries.
pub fn code(size: SheetSize) -> usize {
    CODED.iter().position(|&s| s == size).map_or(1, |i| i + 1)
}

/// The size a count of code dots names.
pub fn size_of_code(dots: usize) -> Option<SheetSize> {
    (dots >= 1).then(|| CODED.get(dots - 1).copied()).flatten()
}

/// A thing of known size on the sheet, for reading the print scale.
#[derive(Debug, Clone, PartialEq)]
pub enum Reference {
    /// A round object of this diameter in millimetres: a coin, a bearing.
    Disc(f64),
    /// A photo scale with alternating dark and light blocks of this
    /// length in millimetres (a forensic ABFO No. 2 scale has 10 mm
    /// bars), so dark blocks repeat every twice that.
    Bars(f64),
    /// A steel rule with millimetre graduations: its ticks are read.
    /// Without a sheet in the picture, this makes a flat scan
    /// measurable on its own.
    Rule,
}

impl Reference {
    /// `"disc 24.26"`, `"bars 10"`, or a US coin by name.
    pub fn parse(text: &str) -> Result<Reference, String> {
        let t = text.trim().to_lowercase();
        let coin = match t.as_str() {
            "quarter" => Some(24.26),
            "nickel" => Some(21.21),
            "dime" => Some(17.91),
            "penny" | "cent" => Some(19.05),
            _ => None,
        };
        if let Some(d) = coin {
            return Ok(Reference::Disc(d));
        }
        if matches!(t.as_str(), "rule" | "ruler" | "steel rule" | "rule mm") {
            return Ok(Reference::Rule);
        }
        let mut words = t.split_whitespace();
        let kind = words.next().unwrap_or("");
        let value: f64 = words
            .next()
            .unwrap_or("")
            .trim_end_matches("mm")
            .parse()
            .map_err(|_| {
                format!("reference {text:?}: give a size, like \"disc 24.26\" or \"bars 10\"")
            })?;
        if value <= 0.0 {
            return Err(format!("reference {text:?}: the size must be positive"));
        }
        match kind {
            "disc" | "coin" | "round" => Ok(Reference::Disc(value)),
            "bars" | "bar" | "scale" => Ok(Reference::Bars(value)),
            _ => Err(format!(
                "reference {text:?}: say \"rule\" for a steel rule with millimetre graduations, \"bars <mm>\" for a photo scale's alternating blocks, \"disc <mm>\" for a round object, or a US coin by name"
            )),
        }
    }

    fn describe(&self) -> String {
        match self {
            Reference::Disc(d) => format!("a {d:.2} mm disc"),
            Reference::Bars(b) => format!("a scale with {b:.0} mm bars"),
            Reference::Rule => "a rule's millimetre graduations".to_string(),
        }
    }
}

/// The reference sheet as a PDF at true size.
pub fn sheet_pdf(size: SheetSize) -> Vec<u8> {
    let (w, h) = size.size();
    let (lx, ly) = span(size);
    let (x0, y0) = (INSET, INSET);
    let mut page = Page::new(w, h);
    // The grid, light so photographs threshold it away: 10 mm lines,
    // heavier every 50.
    let mut fine = Vec::new();
    let mut coarse = Vec::new();
    let mut x = 0.0;
    while x <= lx + 1e-9 {
        let seg = [(x0 + x, y0), (x0 + x, y0 + ly)];
        if (x % 50.0).abs() < 1e-9 {
            coarse.push(seg);
        } else {
            fine.push(seg);
        }
        x += 10.0;
    }
    let mut y = 0.0;
    while y <= ly + 1e-9 {
        let seg = [(x0, y0 + y), (x0 + lx, y0 + y)];
        if (y % 50.0).abs() < 1e-9 {
            coarse.push(seg);
        } else {
            fine.push(seg);
        }
        y += 10.0;
    }
    page.gray(0.78);
    page.lines(&fine, 0.15, None);
    page.gray(0.55);
    page.lines(&coarse, 0.3, None);
    // Millimetre labels every 50 along the bottom and the left.
    page.gray(0.35);
    let mut v = 50.0;
    while v < lx - 10.0 {
        page.text(
            x0 + v,
            y0 - 4.5,
            2.5,
            &format!("{v:.0}"),
            Anchor::Middle,
            0.0,
            false,
        );
        v += 50.0;
    }
    let mut v = 50.0;
    while v < ly - 10.0 {
        page.text(
            x0 - 3.0,
            y0 + v - 0.9,
            2.5,
            &format!("{v:.0}"),
            Anchor::Right,
            0.0,
            false,
        );
        v += 50.0;
    }
    // The fiducials: bullseyes, the origin's with an extra ring.
    let corners = [(x0, y0), (x0 + lx, y0), (x0 + lx, y0 + ly), (x0, y0 + ly)];
    for (k, (cx, cy)) in corners.into_iter().enumerate() {
        if k == 0 {
            page.gray(0.0);
            page.dot(cx, cy, ORIGIN_RING_R);
            page.gray(1.0);
            page.dot(cx, cy, ORIGIN_RING_HOLE_R);
        }
        page.gray(0.0);
        page.dot(cx, cy, RING_R);
        page.gray(1.0);
        page.dot(cx, cy, RING_HOLE_R);
        page.gray(0.0);
        page.dot(cx, cy, DOT_R);
    }
    // The size code: dots below the axis by the origin mark.
    page.gray(0.0);
    for k in 0..code(size) {
        page.dot(
            x0 + CODE_X0 + k as f64 * CODE_PITCH,
            y0 + CODE_Y,
            CODE_DOT_R,
        );
    }
    // Title and the print-scale bar in the bottom margin.
    page.gray(0.0);
    // The bar sits under the text, its end ticks clear of the letters.
    page.text(
        x0 + 45.0,
        10.5,
        3.0,
        &format!(
            "offkilter measuring sheet · {} · print at 100 % · the bar below is 100 mm · origin at the double-ringed mark, x right, y up · the dots by it say which sheet",
            size.name()
        ),
        Anchor::Left,
        0.0,
        false,
    );
    page.lines(
        &[
            [(x0 + 45.0, 5.5), (x0 + 145.0, 5.5)],
            [(x0 + 45.0, 4.0), (x0 + 45.0, 7.0)],
            [(x0 + 145.0, 4.0), (x0 + 145.0, 7.0)],
        ],
        0.4,
        None,
    );
    page.finish()
}

/// A part found on the sheet, in millimetres of the sheet frame.
#[derive(Debug, Clone, Serialize)]
pub struct Part {
    /// x0, y0, x1, y1.
    pub bbox: [f64; 4],
    /// Of the dark silhouette, holes excluded.
    pub area: f64,
    pub centroid: [f64; 2],
    /// Diameter of the circle with the silhouette's area (holes
    /// filled): the part's size, if it is round.
    pub diameter: f64,
    /// 1 for a disc (holes or not), less for anything else.
    pub circularity: f64,
    /// The silhouette, simplified, counter-clockwise in the sheet frame.
    pub outline: Vec<[f64; 2]>,
    /// The outline as fitted edges, counter-clockwise, meeting at
    /// shared vertices: straight runs and arcs where the traced boundary
    /// lies within 0.3 mm of them, or one circle when the whole outline
    /// is round. Empty when nothing fits.
    pub edges: Vec<Edge>,
    pub holes: Vec<Hole>,
}

/// An edge fitted to a run of a part's outline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Edge {
    Line {
        a: [f64; 2],
        b: [f64; 2],
    },
    /// From `start` to `end` about `centre`, counter-clockwise when
    /// `ccw` (a convex corner of the part), clockwise otherwise (a
    /// notch).
    Arc {
        centre: [f64; 2],
        radius: f64,
        start: [f64; 2],
        end: [f64; 2],
        ccw: bool,
    },
    Circle {
        centre: [f64; 2],
        radius: f64,
    },
}

/// A hole through a part, seen as paper showing through it.
#[derive(Debug, Clone, Serialize)]
pub struct Hole {
    pub centre: [f64; 2],
    /// Diameter of the circle with the hole's area.
    pub diameter: f64,
    /// 1 for a circle, less for anything else.
    pub circularity: f64,
    pub area: f64,
}

/// The print scale read from a reference of known size.
#[derive(Debug, Clone, Serialize)]
pub struct Calibration {
    pub reference: String,
    /// What the reference measured before the correction, in the
    /// millimetres the sheet's marks implied.
    pub measured: f64,
    pub nominal: f64,
    /// True millimetres per printed millimetre: 0.97 for a sheet
    /// printed at 97 %.
    pub factor: f64,
    /// Where the reference lies on the sheet (true mm), for keeping
    /// it out of the parts and drawing it.
    pub bbox: [f64; 4],
    /// A disc reference's width over its height as measured on the
    /// sheet: 1 when the print is true, else the print was stretched
    /// one way (1 for a scale's bars and a rule, which read one way).
    pub aspect: f64,
}

fn sheet_name<S: serde::Serializer>(size: &Option<SheetSize>, s: S) -> Result<S::Ok, S::Error> {
    match size {
        Some(size) => s.serialize_str(size.name()),
        None => s.serialize_none(),
    }
}

/// What reading a rule's graduations gave.
#[derive(Debug, Clone, Serialize)]
pub struct RuleReading {
    /// Ticks used on the edge read, and the length they span.
    pub ticks: usize,
    pub length: f64,
    /// Photograph pixels per millimetre at the rule.
    pub px_per_mm: f64,
    /// How the millimetre edge was told from an inch edge: by the
    /// ratio between the two edges' pitches, or assumed because only
    /// one edge read.
    pub edge: &'static str,
    /// The rule's centre in the photograph, in pixels, and the unit
    /// direction its graduations run along there.
    pub centre: [f64; 2],
    pub direction: [f64; 2],
    /// The ticks' box in the photograph, in pixels.
    pub bbox: [f64; 4],
}

/// What `measure` found.
#[derive(Debug, Clone, Serialize)]
pub struct Measurement {
    /// Which sheet the picture is of, and whether the picture said so
    /// (the size code by the origin mark) or the caller did. None for
    /// a flat scan measured from a rule alone: then the frame is the
    /// picture's own, origin at its bottom-left corner.
    #[serde(serialize_with = "sheet_name")]
    pub sheet: Option<SheetSize>,
    pub sheet_read: bool,
    /// The rule, when one was read: how many ticks over how many mm.
    pub rule: Option<RuleReading>,
    pub calibration: Option<Calibration>,
    /// Fiducial centres in the photograph's pixels: origin, x, xy, y.
    pub fiducials: [[f64; 2]; 4],
    /// Millimetres per photograph pixel at the sheet's middle.
    pub mm_per_pixel: f64,
    /// Fit of the four fiducials to the homography, in photograph pixels.
    pub residual: f64,
    /// The resolution the file claims (a PNG's pHYs chunk, a JPEG's JFIF
    /// density), in dots per inch; a scanner writes it, a phone does not.
    pub dpi: Option<f64>,
    /// What looks wrong with the picture or the print: a stretched
    /// print (a reference disc wider than tall), the file's dpi
    /// disagreeing with the rule or the marks.
    pub warnings: Vec<String>,
    pub parts: Vec<Part>,
    /// The rectified picture with the grid and the parts drawn on it,
    /// as a PNG, `SCALE` pixels per millimetre; the sheet frame's origin
    /// sits `BORDER` mm in from its bottom-left corner.
    #[serde(skip)]
    pub picture: Vec<u8>,
    /// Pixels per millimetre and origin (x, y from the top-left) of the
    /// rectified picture.
    pub picture_scale: f64,
    pub picture_origin: [f64; 2],
}

/// The resolution a PNG (pHYs chunk) or JPEG (JFIF APP0 density) file
/// claims, as dots per inch along x and y. None when the file does not
/// say, or says it in no unit.
pub fn dpi_of(bytes: &[u8]) -> Option<(f64, f64)> {
    let be32 = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64;
    let be16 = |b: &[u8]| u16::from_be_bytes([b[0], b[1]]) as f64;
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut at = 8;
        while at + 8 <= bytes.len() {
            let len = be32(&bytes[at..at + 4]) as usize;
            let kind = &bytes[at + 4..at + 8];
            if kind == b"pHYs" && at + 17 <= bytes.len() {
                let d = &bytes[at + 8..at + 17];
                let (x, y) = (be32(&d[0..4]), be32(&d[4..8]));
                return (d[8] == 1 && x > 0.0 && y > 0.0).then_some((x * 0.0254, y * 0.0254));
            }
            if kind == b"IDAT" || kind == b"IEND" {
                return None;
            }
            at += 12 + len;
        }
        return None;
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut at = 2;
        while at + 4 <= bytes.len() && bytes[at] == 0xFF {
            let marker = bytes[at + 1];
            if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
                at += 2;
                continue;
            }
            let len = be16(&bytes[at + 2..at + 4]) as usize;
            if marker == 0xE0 && at + 16 <= bytes.len() && &bytes[at + 4..at + 9] == b"JFIF\0" {
                let units = bytes[at + 11];
                let (x, y) = (
                    be16(&bytes[at + 12..at + 14]),
                    be16(&bytes[at + 14..at + 16]),
                );
                return match units {
                    1 if x > 0.0 && y > 0.0 => Some((x, y)),
                    2 if x > 0.0 && y > 0.0 => Some((x * 2.54, y * 2.54)),
                    _ => None,
                };
            }
            if marker == 0xDA {
                return None;
            }
            at += 2 + len;
        }
    }
    None
}

/// Decodes a PNG or JPEG into RGB.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return ok_render::from_png(bytes);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut d = jpeg_decoder::Decoder::new(bytes);
        let pixels = d.decode().map_err(|e| format!("JPEG: {e}"))?;
        let info = d.info().ok_or("JPEG: no image info")?;
        let (width, height) = (info.width as usize, info.height as usize);
        let data = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => pixels,
            jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&g| [g, g, g]).collect(),
            jpeg_decoder::PixelFormat::L16 => pixels
                .chunks(2)
                .flat_map(|c| {
                    let g = c[0];
                    [g, g, g]
                })
                .collect(),
            jpeg_decoder::PixelFormat::CMYK32 => pixels
                .chunks(4)
                .flat_map(|c| {
                    let k = c[3] as u32;
                    let f = |v: u8| ((v as u32) * k / 255) as u8;
                    [f(c[0]), f(c[1]), f(c[2])]
                })
                .collect(),
        };
        return Ok(Image {
            width,
            height,
            data,
        });
    }
    Err("not a PNG or a JPEG".into())
}

/// Measures a photograph of parts on the reference sheet. `sheet` is
/// the fallback when the picture's size code cannot be read;
/// `reference` names a thing of known size on the sheet, from which
/// the print scale is read and everything corrected.
pub fn measure(
    bytes: &[u8],
    sheet: Option<SheetSize>,
    reference: Option<&Reference>,
) -> Result<Measurement, String> {
    let photo = decode(bytes)?;
    measure_image_at(&photo, dpi_of(bytes), sheet, reference)
}

/// `measure` on a decoded picture that claims no resolution.
pub fn measure_image(
    photo: &Image,
    sheet: Option<SheetSize>,
    reference: Option<&Reference>,
) -> Result<Measurement, String> {
    measure_image_at(photo, None, sheet, reference)
}

/// `measure` on a decoded picture, with the resolution its file
/// claimed (dots per inch along x and y) as a cross-check.
pub fn measure_image_at(
    photo: &Image,
    dpi: Option<(f64, f64)>,
    sheet: Option<SheetSize>,
    reference: Option<&Reference>,
) -> Result<Measurement, String> {
    let mut warnings = Vec::new();
    let dpi = dpi.map(|(x, y)| {
        if ((x - y) / x).abs() > 0.005 {
            warnings.push(format!(
                "the file claims {x:.0} dpi along x and {y:.0} along y: a scan at different resolutions each way; sizes are taken from the marks or the rule, not from it"
            ));
        }
        (x + y) / 2.0
    });
    // Work at most 1800 px across for the fiducials.
    let factor = (photo.width.max(photo.height) as f64 / 1800.0)
        .ceil()
        .max(1.0) as usize;
    let small = shrink(photo, factor);
    let gray = luma(&small);
    let (w, h) = (small.width, small.height);
    let threshold = otsu(&gray);
    let dark: Vec<bool> = gray.iter().map(|&g| (g as u32) < threshold).collect();
    let rule = match reference {
        Some(Reference::Rule) => Some(read_rule(photo)?),
        _ => None,
    };
    let f = factor as f64;
    let (found, dots) = match find_fiducials(&dark, w, h) {
        Ok(found) => found,
        // No sheet: with a rule read, the picture is a flat scan
        // measured in its own frame.
        Err(_) if rule.is_some() => {
            let r = rule.as_ref().unwrap();
            let (pw, ph) = (photo.width as f64, photo.height as f64);
            let corners = [[0.0, ph], [pw, ph], [pw, 0.0], [0.0, 0.0]];
            let (lx, ly) = (pw / r.px_per_mm, ph / r.px_per_mm);
            let bbox = [
                r.bbox[0] / r.px_per_mm,
                ly - r.bbox[3] / r.px_per_mm,
                r.bbox[2] / r.px_per_mm,
                ly - r.bbox[1] / r.px_per_mm,
            ];
            let pass = rectify(photo, &corners, lx, ly, 0.0, Some((1.0, 1.0, bbox)))?;
            if let Some(dpi) = dpi {
                let file = dpi / 25.4;
                if ((r.px_per_mm - file) / file).abs() > 0.02 {
                    warnings.push(format!(
                        "the file claims {dpi:.0} dpi ({file:.3} px/mm) but the rule reads {:.3} px/mm, {:.1} % apart; the rule is trusted, so check that it lay flat on the glass and that the scanner's resolution is what the file says",
                        r.px_per_mm,
                        ((r.px_per_mm - file) / file).abs() * 100.0
                    ));
                }
            }
            return Ok(Measurement {
                sheet: None,
                sheet_read: false,
                rule,
                calibration: None,
                fiducials: corners,
                mm_per_pixel: pass.mm_per_pixel,
                residual: 0.0,
                dpi,
                warnings,
                parts: pass.parts,
                picture: ok_render::to_png(&pass.picture),
                picture_scale: SCALE,
                picture_origin: [BORDER * SCALE, (pass.ly + BORDER) * SCALE],
            });
        }
        Err(e) => return Err(e),
    };
    let fiducials = [
        [found[0].0 * f, found[0].1 * f],
        [found[1].0 * f, found[1].1 * f],
        [found[2].0 * f, found[2].1 * f],
        [found[3].0 * f, found[3].1 * f],
    ];
    let coded = size_of_code(dots);
    let (size, sheet_read) = match (coded, sheet) {
        (Some(c), _) => (c, true),
        (None, Some(s)) => (s, false),
        (None, None) => {
            return Err(format!(
                "no size code by the origin mark ({dots} dots found where 1 to {} name the sheet); say which sheet it is",
                CODED.len()
            ))
        }
    };
    let (lx, ly) = span(size);
    let keep_out = (ORIGIN_RING_R + 3.0) * SCALE;
    // A rule is read in the photograph; its pitch there against the
    // marks' scale at the same spot gives the print scale directly.
    let preset = |k: f64| -> Result<Option<(f64, f64, [f64; 4])>, String> {
        let Some(r) = rule.as_ref() else {
            return Ok(None);
        };
        let sheet = [[0.0, 0.0], [lx * k, 0.0], [lx * k, ly * k], [0.0, ly * k]];
        let to_sheet = homography(&fiducials, &sheet)?;
        // One true millimetre of the rule, in the sheet's millimetres,
        // taken along the rule: a tilted picture is foreshortened one
        // way, and the rule's pitch was read along its own direction.
        let at = apply(&to_sheet, r.centre);
        let step = apply(
            &to_sheet,
            [
                r.centre[0] + r.px_per_mm * r.direction[0],
                r.centre[1] + r.px_per_mm * r.direction[1],
            ],
        );
        let measured = dist2(at, step).sqrt();
        let corners = [
            apply(&to_sheet, [r.bbox[0], r.bbox[1]]),
            apply(&to_sheet, [r.bbox[2], r.bbox[1]]),
            apply(&to_sheet, [r.bbox[2], r.bbox[3]]),
            apply(&to_sheet, [r.bbox[0], r.bbox[3]]),
        ];
        let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for c in corners {
            bbox[0] = bbox[0].min(c[0]);
            bbox[1] = bbox[1].min(c[1]);
            bbox[2] = bbox[2].max(c[0]);
            bbox[3] = bbox[3].max(c[1]);
        }
        Ok(Some((measured, 1.0, bbox)))
    };
    let mut pass = rectify(photo, &fiducials, lx, ly, keep_out, preset(1.0)?)?;
    if rule.is_none() {
        pass.reference = find_reference(reference, &pass);
    }
    let mut calibration = None;
    if let Some(r) = reference {
        let (measured, nominal, _) = pass
            .reference
            .ok_or_else(|| format!("{} not found on the sheet", r.describe()))?;
        let k = nominal / measured;
        // Measure again in true millimetres: the marks are k times
        // their nominal spacing apart.
        pass = rectify(photo, &fiducials, lx * k, ly * k, keep_out, preset(k)?)?;
        if rule.is_none() {
            pass.reference = find_reference(reference, &pass);
        }
        let (_, _, bbox) = pass
            .reference
            .ok_or_else(|| format!("{} not found on the sheet", r.describe()))?;
        // A disc shows a print stretched one way: it comes out wider
        // than tall, or the reverse, on the sheet's millimetres.
        let aspect = match r {
            Reference::Disc(_) => pass
                .parts
                .iter()
                .find(|p| p.bbox == bbox)
                .map(|p| aspect_of(&p.outline))
                .unwrap_or(1.0),
            _ => 1.0,
        };
        exclude_reference(&mut pass, bbox);
        if (aspect - 1.0).abs() > 0.01 {
            warnings.push(format!(
                "the print seems stretched one way: the reference disc measures {:.2} x {:.2} mm on the sheet ({:.1} % {} than tall), so sizes along x and y are scaled differently and the correction by its area holds only on average; print the sheet again without fitting it to the page",
                bbox[2] - bbox[0],
                bbox[3] - bbox[1],
                (aspect - 1.0).abs() * 100.0,
                if aspect > 1.0 { "wider" } else { "narrower" }
            ));
        }
        calibration = Some(Calibration {
            reference: r.describe(),
            measured,
            nominal,
            factor: k,
            bbox,
            aspect,
        });
    }
    // A scanned sheet: the file's resolution says how far apart the
    // marks really are, which is the print scale, with or without a
    // reference to agree with.
    if let Some(dpi) = dpi {
        let from_file = (25.4 / dpi) / pass.mm_per_pixel;
        let k = calibration.as_ref().map(|c| c.factor).unwrap_or(1.0);
        if ((from_file - k) / k).abs() > 0.01 {
            warnings.push(match &calibration {
                Some(c) => format!(
                    "the file claims {dpi:.0} dpi, by which the sheet was printed at {:.1} %, but the {} says {:.1} %; the reference is trusted",
                    from_file * 100.0,
                    c.reference,
                    k * 100.0
                ),
                None => format!(
                    "the file claims {dpi:.0} dpi, by which the marks are {:.1} % of their nominal spacing apart: the sheet seems printed at {:.1} %, and sizes trust 100 %; name a reference to correct for it",
                    from_file * 100.0,
                    from_file * 100.0
                ),
            });
        }
    }
    Ok(Measurement {
        sheet: Some(size),
        sheet_read,
        rule,
        calibration,
        fiducials,
        mm_per_pixel: pass.mm_per_pixel,
        residual: pass.residual,
        dpi,
        warnings,
        parts: pass.parts,
        picture: ok_render::to_png(&pass.picture),
        picture_scale: SCALE,
        picture_origin: [BORDER * SCALE, (pass.ly + BORDER) * SCALE],
    })
}

/// One rectification of the photograph onto a sheet frame whose marks
/// are `lx` x `ly` mm apart.
struct Pass {
    #[allow(dead_code)]
    lx: f64,
    ly: f64,
    mm_per_pixel: f64,
    residual: f64,
    parts: Vec<Part>,
    /// Every dark block of 3 mm2 or more, for finding a scale's bars.
    blocks: Vec<[f64; 4]>,
    /// The reference, when asked for and found: what it measured, its
    /// nominal size, and its box.
    reference: Option<(f64, f64, [f64; 4])>,
    picture: Image,
}

fn rectify(
    photo: &Image,
    fiducials: &[[f64; 2]; 4],
    lx: f64,
    ly: f64,
    keep_out: f64,
    preset: Option<(f64, f64, [f64; 4])>,
) -> Result<Pass, String> {
    let sheet = [[0.0, 0.0], [lx, 0.0], [lx, ly], [0.0, ly]];
    let to_sheet = homography(fiducials, &sheet)?;
    let to_photo = invert(&to_sheet)?;
    let residual = {
        let mut sum = 0.0;
        for k in 0..4 {
            let p = apply(&to_photo, sheet[k]);
            sum += (p[0] - fiducials[k][0]).powi(2) + (p[1] - fiducials[k][1]).powi(2);
        }
        (sum / 4.0).sqrt()
    };
    let mm_per_pixel = {
        let c = [lx / 2.0, ly / 2.0];
        let a = apply(&to_photo, c);
        let b = apply(&to_photo, [c[0] + 1.0, c[1]]);
        1.0 / ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
    };
    // Rectify: the sheet frame at SCALE px/mm, BORDER mm beyond the
    // fiducials, y up on the sheet and down in the picture.
    let pw = ((lx + 2.0 * BORDER) * SCALE).round() as usize;
    let ph = ((ly + 2.0 * BORDER) * SCALE).round() as usize;
    let mut picture = Image::new(pw, ph, Rgb(255, 255, 255));
    let mut rect_gray = vec![255u8; pw * ph];
    for j in 0..ph {
        for i in 0..pw {
            let x = -BORDER + i as f64 / SCALE;
            let y = ly + BORDER - j as f64 / SCALE;
            let p = apply(&to_photo, [x, y]);
            if let Some(c) = sample(photo, p[0], p[1]) {
                picture.set(i, j, c);
                rect_gray[j * pw + i] =
                    (0.299 * c.0 as f64 + 0.587 * c.1 as f64 + 0.114 * c.2 as f64) as u8;
            }
        }
    }
    let px = |x: f64| ((x + BORDER) * SCALE) as isize;
    let py = |y: f64| ((ly + BORDER - y) * SCALE) as isize;
    // Segment the dark shapes on the grid, keeping clear of the
    // fiducials and the margins.
    let t2 = otsu(&rect_gray);
    let mut mask = vec![false; pw * ph];
    for j in 0..ph {
        for i in 0..pw {
            let x = -BORDER + i as f64 / SCALE;
            let y = ly + BORDER - j as f64 / SCALE;
            if x < 0.0 || y < 0.0 || x > lx || y > ly {
                continue;
            }
            let near_fiducial = sheet
                .iter()
                .any(|s| ((x - s[0]).powi(2) + (y - s[1]).powi(2)).sqrt() * SCALE < keep_out);
            if near_fiducial {
                continue;
            }
            mask[j * pw + i] = (rect_gray[j * pw + i] as u32) < t2;
        }
    }
    let mask = open(&mask, pw, ph, 2);
    let comps = components(&mask, pw, ph);
    let light: Vec<bool> = mask.iter().map(|m| !m).collect();
    let light_comps = components(&light, pw, ph);
    let min_area = (20.0 * SCALE * SCALE) as usize;
    let mm = |i: usize, j: usize| -> [f64; 2] {
        [-BORDER + i as f64 / SCALE, ly + BORDER - j as f64 / SCALE]
    };
    let mut parts = Vec::new();
    for c in comps.iter().filter(|c| c.area >= min_area) {
        let lo = mm(c.x0, c.y1 + 1);
        let hi = mm(c.x1 + 1, c.y0);
        let centroid = [-BORDER + c.cx / SCALE, ly + BORDER - c.cy / SCALE];
        let mut traced = trace(&c.labels_of(&mask, pw), pw, ph, c);
        // Counter-clockwise in the sheet frame (y up), whichever way the
        // tracer went round in the picture.
        let signed = (0..traced.len())
            .map(|i| {
                let (a, b) = (mm(traced[i].0, traced[i].1), {
                    let q = traced[(i + 1) % traced.len()];
                    mm(q.0, q.1)
                });
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>();
        if signed < 0.0 {
            traced.reverse();
        }
        let corners = simplify_indices(&traced, 0.3 * SCALE);
        let dense: Vec<[f64; 2]> = traced.iter().map(|&(i, j)| mm(i, j)).collect();
        let outline: Vec<[f64; 2]> = corners.iter().map(|&k| dense[k]).collect();
        let edges = fit_edges(&dense, &corners, 0.3);
        let mut holes = Vec::new();
        for l in &light_comps {
            let inside = l.x0 > c.x0 && l.y0 > c.y0 && l.x1 < c.x1 && l.y1 < c.y1;
            let touches_edge = l.x0 == 0 || l.y0 == 0 || l.x1 + 1 == pw || l.y1 + 1 == ph;
            if !inside || touches_edge || l.area < (2.0 * SCALE * SCALE) as usize {
                continue;
            }
            let area = l.area as f64 / (SCALE * SCALE);
            let perimeter = l.perimeter as f64 / SCALE;
            holes.push(Hole {
                centre: [-BORDER + l.cx / SCALE, ly + BORDER - l.cy / SCALE],
                diameter: 2.0 * (area / PI).sqrt(),
                circularity: (4.0 * PI * area / (perimeter * perimeter)).min(1.0),
                area,
            });
        }
        // Roundness is of the silhouette with its holes filled: a washer
        // is round. The component's own perimeter counts the holes' rims.
        let area = c.area as f64 / (SCALE * SCALE);
        let filled = area + holes.iter().map(|h| h.area).sum::<f64>();
        let perimeter: f64 = (0..outline.len())
            .map(|i| dist2(outline[i], outline[(i + 1) % outline.len()]).sqrt())
            .sum();
        parts.push(Part {
            bbox: [lo[0], lo[1], hi[0], hi[1]],
            area,
            centroid,
            diameter: 2.0 * (filled / PI).sqrt(),
            circularity: (4.0 * PI * filled / (perimeter * perimeter)).min(1.0),
            outline,
            edges,
            holes,
        });
    }
    parts.sort_by(|a, b| b.area.total_cmp(&a.area));
    // The smaller dark blocks too, for a photo scale's bars.
    let blocks: Vec<[f64; 4]> = comps
        .iter()
        .filter(|c| c.area >= (3.0 * SCALE * SCALE) as usize && c.fill() > 0.6)
        .map(|c| {
            let lo = mm(c.x0, c.y1 + 1);
            let hi = mm(c.x1 + 1, c.y0);
            [lo[0], lo[1], hi[0], hi[1]]
        })
        .collect();
    // The overlay: the grid, the fiducials, each part's box and holes.
    let mut x = 0.0;
    while x <= lx + 1e-9 {
        let major = (x % 50.0).abs() < 1e-9;
        vline(
            &mut picture,
            px(x),
            py(ly),
            py(0.0),
            if major {
                Rgb(90, 140, 220)
            } else {
                Rgb(190, 210, 240)
            },
        );
        x += 10.0;
    }
    let mut y = 0.0;
    while y <= ly + 1e-9 {
        let major = (y % 50.0).abs() < 1e-9;
        hline(
            &mut picture,
            px(0.0),
            px(lx),
            py(y),
            if major {
                Rgb(90, 140, 220)
            } else {
                Rgb(190, 210, 240)
            },
        );
        y += 10.0;
    }
    if keep_out > 0.0 {
        for s in &sheet {
            cross(&mut picture, px(s[0]), py(s[1]), 8, Rgb(40, 90, 200));
        }
    }
    let mut pass = Pass {
        lx,
        ly,
        mm_per_pixel,
        residual,
        parts,
        blocks,
        reference: preset,
        picture,
    };
    if let Some((_, _, bbox)) = preset {
        exclude_reference(&mut pass, bbox);
    }
    draw_parts(&mut pass);
    Ok(pass)
}

/// The reference a pass was asked for, found among its parts (a disc)
/// or its smaller dark blocks (a scale's bars): what it measured, its
/// nominal size, and its box.
fn find_reference(reference: Option<&Reference>, pass: &Pass) -> Option<(f64, f64, [f64; 4])> {
    match reference {
        None | Some(Reference::Rule) => None,
        Some(Reference::Disc(d)) => pass
            .parts
            .iter()
            .filter(|p| p.circularity > 0.85 && p.holes.is_empty())
            .filter(|p| (p.diameter / d - 1.0).abs() < 0.25)
            .min_by(|a, b| (a.diameter - d).abs().total_cmp(&(b.diameter - d).abs()))
            .map(|p| (p.diameter, *d, p.bbox)),
        Some(Reference::Bars(b)) => {
            bar_run(&pass.blocks, 2.0 * b).map(|(pitch, bbox)| (pitch, 2.0 * b, bbox))
        }
    }
}

/// A closed outline's width over its height, from the second moments
/// of the polygon it bounds (an ellipse's semi-axes are twice the
/// standard deviations along x and y): finer than its pixel box.
fn aspect_of(outline: &[[f64; 2]]) -> f64 {
    let n = outline.len();
    if n < 3 {
        return 1.0;
    }
    let (mut a, mut cx, mut cy, mut sxx, mut syy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        let cross = p[0] * q[1] - q[0] * p[1];
        a += cross;
        cx += (p[0] + q[0]) * cross;
        cy += (p[1] + q[1]) * cross;
        sxx += (p[0] * p[0] + p[0] * q[0] + q[0] * q[0]) * cross;
        syy += (p[1] * p[1] + p[1] * q[1] + q[1] * q[1]) * cross;
    }
    a /= 2.0;
    if a.abs() < 1e-12 {
        return 1.0;
    }
    cx /= 6.0 * a;
    cy /= 6.0 * a;
    let var_x = sxx / (12.0 * a) - cx * cx;
    let var_y = syy / (12.0 * a) - cy * cy;
    if var_x <= 0.0 || var_y <= 0.0 {
        return 1.0;
    }
    (var_x / var_y).sqrt()
}

/// Takes the reference out of the parts, along with whatever else lies
/// within its box (a scale's numerals, a rule's body), and boxes it on
/// the picture.
fn exclude_reference(pass: &mut Pass, bbox: [f64; 4]) {
    let margin = 3.0;
    pass.parts.retain(|p| {
        !(p.centroid[0] > bbox[0] - margin
            && p.centroid[0] < bbox[2] + margin
            && p.centroid[1] > bbox[1] - margin
            && p.centroid[1] < bbox[3] + margin)
    });
    let ly = pass.ly;
    let px = |x: f64| ((x + BORDER) * SCALE) as isize;
    let py = |y: f64| ((ly + BORDER - y) * SCALE) as isize;
    let (x0, y0, x1, y1) = (
        px(bbox[0]) - 4,
        py(bbox[3]) - 4,
        px(bbox[2]) + 4,
        py(bbox[1]) + 4,
    );
    hline(&mut pass.picture, x0, x1, y0, Rgb(240, 150, 30));
    hline(&mut pass.picture, x0, x1, y1, Rgb(240, 150, 30));
    vline(&mut pass.picture, x0, y0, y1, Rgb(240, 150, 30));
    vline(&mut pass.picture, x1, y0, y1, Rgb(240, 150, 30));
}

/// Boxes each part and crosses each hole on the picture.
fn draw_parts(pass: &mut Pass) {
    let ly = pass.ly;
    let px = |x: f64| ((x + BORDER) * SCALE) as isize;
    let py = |y: f64| ((ly + BORDER - y) * SCALE) as isize;
    let picture = &mut pass.picture;
    for p in &pass.parts {
        let (x0, y0, x1, y1) = (px(p.bbox[0]), py(p.bbox[3]), px(p.bbox[2]), py(p.bbox[1]));
        hline(picture, x0, x1, y0, Rgb(220, 40, 40));
        hline(picture, x0, x1, y1, Rgb(220, 40, 40));
        vline(picture, x0, y0, y1, Rgb(220, 40, 40));
        vline(picture, x1, y0, y1, Rgb(220, 40, 40));
        for h in &p.holes {
            cross(
                picture,
                px(h.centre[0]),
                py(h.centre[1]),
                5,
                Rgb(30, 170, 60),
            );
        }
    }
}

/// Reads a steel rule's millimetre graduations in the photograph: the
/// ticks are the thin marks darker than their surroundings, the rule's
/// direction is the one most of them are perpendicular to, the ticks
/// whose bases line up are one edge, and a straight-line fit of tick
/// position against tick index over that edge gives pixels per
/// millimetre. A second edge with the pitch of sixteenths, thirty-
/// seconds or sixty-fourths of an inch tells which edge is metric;
/// with one edge the millimetre one is assumed.
fn read_rule(photo: &Image) -> Result<RuleReading, String> {
    // Down to at most 4800 px across: a 12 megapixel phone picture of
    // a sheet, or a 300 dpi scan of one, keeps every pixel, and a
    // 0.25 mm tick is three of them; a 600 dpi scan halves.
    let factor = (photo.width.max(photo.height) as f64 / 4800.0)
        .ceil()
        .max(1.0) as usize;
    let small = shrink(photo, factor);
    let gray = luma(&small);
    let (w, h) = (small.width, small.height);
    // Darker than the local mean: ticks on any body.
    let r = 12usize;
    let mut integral = vec![0u64; (w + 1) * (h + 1)];
    for j in 0..h {
        let mut row = 0u64;
        for i in 0..w {
            row += gray[j * w + i] as u64;
            integral[(j + 1) * (w + 1) + i + 1] = integral[j * (w + 1) + i + 1] + row;
        }
    }
    let mut mask = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            let (x0, y0) = (i.saturating_sub(r), j.saturating_sub(r));
            let (x1, y1) = ((i + r + 1).min(w), (j + r + 1).min(h));
            let sum = integral[y1 * (w + 1) + x1] + integral[y0 * (w + 1) + x0]
                - integral[y0 * (w + 1) + x1]
                - integral[y1 * (w + 1) + x0];
            let mean = sum as f64 / ((x1 - x0) * (y1 - y0)) as f64;
            // Well below the local mean in proportion, not just by a
            // margin: at the rule's edge the mean is pulled up by the
            // paper, and the body must not read as a mark there.
            mask[j * w + i] = (gray[j * w + i] as f64) < (mean - 20.0).min(0.72 * mean);
        }
    }
    // Ticks: small, thin, and their angle.
    struct Tick {
        c: [f64; 2],
        angle: f64,
        length: f64,
        thickness: f64,
    }
    let ticks: Vec<Tick> = components(&mask, w, h)
        .iter()
        .filter(|c| c.area >= 3 && c.area <= (w * h) / 2000)
        .filter_map(|c| {
            let (big, little, angle) = c.axes();
            let length = (12.0 * big).sqrt();
            let elongation = (big / little.max(1.0 / 12.0)).sqrt();
            (elongation >= 2.5).then_some(Tick {
                c: [c.cx, c.cy],
                angle,
                length,
                thickness: c.area as f64 / length.max(1.0),
            })
        })
        .collect();
    if ticks.len() < 20 {
        return Err(format!(
            "no rule read: {} tick-like marks in the picture, 20 or more are needed",
            ticks.len()
        ));
    }
    // The rule runs across the direction most ticks share.
    let bins = 90usize;
    let mut hist = vec![0usize; bins];
    for t in &ticks {
        let b = ((t.angle.rem_euclid(PI)) / PI * bins as f64) as usize % bins;
        hist[b] += 1;
    }
    // The fullest bin, its neighbours breaking ties, so the angle is
    // within a bin of most ticks.
    let peak = (0..bins)
        .max_by_key(|&b| (hist[b], hist[(b + bins - 1) % bins] + hist[(b + 1) % bins]))
        .unwrap();
    let tick_angle = (peak as f64 + 0.5) / bins as f64 * PI;
    let aligned: Vec<&Tick> = ticks
        .iter()
        .filter(|t| {
            let d = (t.angle - tick_angle).rem_euclid(PI);
            d.min(PI - d) < 3.0 * PI / 180.0
        })
        .collect();
    // Refine the direction as the mean of the aligned ticks' angles.
    let (sx, sy) = aligned.iter().fold((0.0, 0.0), |(sx, sy), t| {
        (sx + (2.0 * t.angle).cos(), sy + (2.0 * t.angle).sin())
    });
    let tick_angle = 0.5 * sy.atan2(sx);
    let along_dir = [-(tick_angle).sin(), tick_angle.cos()];
    let across_dir = [tick_angle.cos(), tick_angle.sin()];
    let along = |t: &Tick| t.c[0] * along_dir[0] + t.c[1] * along_dir[1];
    let across = |t: &Tick| t.c[0] * across_dir[0] + t.c[1] * across_dir[1];
    // One edge: the ticks whose bases line up. A tick's base is one of
    // its ends; try both.
    let mut thick: Vec<f64> = aligned.iter().map(|t| t.thickness).collect();
    thick.sort_by(f64::total_cmp);
    let window = (3.0 * thick[thick.len() / 2]).max(2.0);
    let ends: Vec<(f64, f64)> = aligned
        .iter()
        .map(|t| (across(t) - t.length / 2.0, across(t) + t.length / 2.0))
        .collect();
    // Cluster the ticks by that end: a run of ends with no gap wider
    // than the tolerance is one edge (or the tips of one tick length,
    // which fit just as well).
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for side in 0..2 {
        let mut order: Vec<usize> = (0..aligned.len()).collect();
        let key = |k: usize| if side == 0 { ends[k].0 } else { ends[k].1 };
        order.sort_by(|&a, &b| key(a).total_cmp(&key(b)));
        let mut start = 0;
        for end in 1..=order.len() {
            if end == order.len() || key(order[end]) - key(order[end - 1]) > window {
                if end - start >= 20 {
                    groups.push(order[start..end].to_vec());
                }
                start = end;
            }
        }
    }
    // Fit each group: index against position.
    struct Edge {
        pitch: f64,
        ticks: usize,
        first: f64,
        last: f64,
        members: Vec<usize>,
    }
    let fit = |members: &[usize]| -> Option<Edge> {
        let mut pos: Vec<f64> = members.iter().map(|&k| along(aligned[k])).collect();
        pos.sort_by(f64::total_cmp);
        pos.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        if pos.len() < 20 {
            return None;
        }
        let mut gaps: Vec<f64> = pos.windows(2).map(|g| g[1] - g[0]).collect();
        gaps.sort_by(f64::total_cmp);
        let mut pitch = gaps[gaps.len() / 2];
        if pitch < 2.0 {
            return None;
        }
        // Index each tick from its neighbour, so a rough pitch cannot
        // drift a whole tick over a long rule, then fit the pitch and
        // index again with the fitted one.
        let n = pos.len() as f64;
        let mut rms = 0.0;
        for _ in 0..2 {
            let mut idx = vec![0.0; pos.len()];
            for k in 1..pos.len() {
                idx[k] = idx[k - 1] + ((pos[k] - pos[k - 1]) / pitch).round().max(1.0);
            }
            let (mi, mp) = (idx.iter().sum::<f64>() / n, pos.iter().sum::<f64>() / n);
            let sxx: f64 = idx.iter().map(|i| (i - mi).powi(2)).sum();
            let sxy: f64 = idx.iter().zip(&pos).map(|(i, p)| (i - mi) * (p - mp)).sum();
            if sxx <= 0.0 {
                return None;
            }
            pitch = sxy / sxx;
            rms = (idx
                .iter()
                .zip(&pos)
                .map(|(i, p)| (mp + pitch * (i - mi) - p).powi(2))
                .sum::<f64>()
                / n)
                .sqrt();
        }
        if rms > 0.15 * pitch {
            return None;
        }
        let steps = gaps
            .iter()
            .filter(|g| ((*g / pitch) - 1.0).abs() < 0.2)
            .count();
        if (steps as f64) < 0.6 * gaps.len() as f64 {
            return None;
        }
        Some(Edge {
            pitch,
            ticks: pos.len(),
            first: pos[0],
            last: pos[pos.len() - 1],
            members: members.to_vec(),
        })
    };
    let mut edges: Vec<Edge> = groups.iter().filter_map(|g| fit(g)).collect();
    edges.sort_by(|a, b| b.ticks.cmp(&a.ticks));
    if edges.is_empty() {
        return Err(
            "no rule read: tick-like marks were found but none line up evenly along an edge".into(),
        );
    }
    // Which edge is millimetres: the best edge, unless a second edge of
    // a clearly different pitch says by its ratio that it is inch
    // graduations (or that the best one is).
    let best = &edges[0];
    let other = edges
        .iter()
        .skip(1)
        .find(|e| (e.pitch / best.pitch - 1.0).abs() > 0.05);
    let (mm, edge_how) = match other {
        Some(b) => {
            let (small_, large) = if best.pitch < b.pitch {
                (best, b)
            } else {
                (b, best)
            };
            let ratio = large.pitch / small_.pitch;
            if (ratio - 25.4 / 16.0).abs() < 0.04 {
                (small_, "sixteenths on the other edge")
            } else if (ratio - 32.0 / 25.4).abs() < 0.03 {
                (large, "thirty-seconds on the other edge")
            } else if (ratio - 64.0 / 25.4).abs() < 0.05 {
                (large, "sixty-fourths on the other edge")
            } else {
                (best, "assumed: the two edges' pitches are not inch and mm")
            }
        }
        None => (best, "assumed: one edge read"),
    };
    let f = factor as f64;
    let members: Vec<&Tick> = mm.members.iter().map(|&k| aligned[k]).collect();
    let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for t in &members {
        bbox[0] = bbox[0].min(t.c[0] - t.length / 2.0);
        bbox[1] = bbox[1].min(t.c[1] - t.length / 2.0);
        bbox[2] = bbox[2].max(t.c[0] + t.length / 2.0);
        bbox[3] = bbox[3].max(t.c[1] + t.length / 2.0);
    }
    let n = members.len() as f64;
    let centre = [
        members.iter().map(|t| t.c[0]).sum::<f64>() / n * f,
        members.iter().map(|t| t.c[1]).sum::<f64>() / n * f,
    ];
    Ok(RuleReading {
        ticks: mm.ticks,
        length: (mm.last - mm.first) / mm.pitch,
        px_per_mm: mm.pitch * f,
        edge: edge_how,
        centre,
        direction: along_dir,
        bbox: [bbox[0] * f, bbox[1] * f, bbox[2] * f, bbox[3] * f],
    })
}

/// The longest straight, evenly spaced run of at least three alike
/// blocks whose pitch is within a quarter of `pitch`: a photo scale's
/// bars. Returns the measured pitch (end to end over the run) and the
/// run's box.
fn bar_run(blocks: &[[f64; 4]], pitch: f64) -> Option<(f64, [f64; 4])> {
    let centre = |b: &[f64; 4]| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
    let area = |b: &[f64; 4]| (b[2] - b[0]) * (b[3] - b[1]);
    let mut best: Option<(usize, f64, [f64; 4])> = None;
    for (i, a) in blocks.iter().enumerate() {
        for (j, b) in blocks.iter().enumerate() {
            if i == j || !(0.7..=1.4).contains(&(area(a) / area(b))) {
                continue;
            }
            let (ca, cb) = (centre(a), centre(b));
            let step = [cb[0] - ca[0], cb[1] - ca[1]];
            let len = dist2(ca, cb).sqrt();
            if (len / pitch - 1.0).abs() > 0.25 {
                continue;
            }
            // Walk on from b while a block of a's size sits at each step.
            let mut run = vec![i, j];
            let mut last = cb;
            loop {
                let want = [last[0] + step[0], last[1] + step[1]];
                let next = blocks.iter().enumerate().find(|(k, c)| {
                    !run.contains(k)
                        && (0.7..=1.4).contains(&(area(a) / area(c)))
                        && dist2(centre(c), want).sqrt() < 0.12 * pitch
                });
                match next {
                    Some((k, c)) => {
                        run.push(k);
                        last = centre(c);
                    }
                    None => break,
                }
            }
            if run.len() < 3 {
                continue;
            }
            let measured = dist2(ca, last).sqrt() / (run.len() - 1) as f64;
            let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for &k in &run {
                bbox[0] = bbox[0].min(blocks[k][0]);
                bbox[1] = bbox[1].min(blocks[k][1]);
                bbox[2] = bbox[2].max(blocks[k][2]);
                bbox[3] = bbox[3].max(blocks[k][3]);
            }
            if best.is_none_or(|(n, _, _)| run.len() > n) {
                best = Some((run.len(), measured, bbox));
            }
        }
    }
    best.map(|(_, m, b)| (m, b))
}

// ---- raster helpers ------------------------------------------------

/// What lies on a synthetic sheet, in true millimetres of the sheet
/// frame: `rects` (x0, y0, x1, y1), `discs` (cx, cy, r), and `holes`
/// (cx, cy, r) drilled through whatever they land on. `print` is the
/// scale the sheet was printed at (0.97 for a sheet that came out 3 %
/// small), which shrinks the marks, the grid and the code but not the
/// parts.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub rects: Vec<[f64; 4]>,
    pub discs: Vec<[f64; 3]>,
    pub holes: Vec<[f64; 3]>,
    pub print: f64,
    /// How much wider than tall the print came out (1 when true): the
    /// sheet's own features are scaled by `print * stretch` along x and
    /// `print` along y.
    pub stretch: f64,
    /// A steel rule: (x, y) of its lower-left corner, its length, and
    /// its angle in degrees. Metric ticks along its lower edge, inch
    /// sixteenths along its upper.
    pub rule: Option<(f64, f64, f64, f64)>,
}

impl Default for Scene {
    fn default() -> Scene {
        Scene {
            rects: Vec::new(),
            discs: Vec::new(),
            holes: Vec::new(),
            print: 1.0,
            stretch: 1.0,
            rule: None,
        }
    }
}

/// Shades one point of the scene, in true millimetres: a part, or the
/// rule's body and ticks; None where the background shows.
fn scene_at(scene: &Scene, xt: f64, yt: f64) -> Option<Rgb> {
    if let Some((rx, ry, len, deg)) = scene.rule {
        let (c, s) = ((deg * PI / 180.0).cos(), (deg * PI / 180.0).sin());
        let (dx, dy) = (xt - rx, yt - ry);
        let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
        let width = 15.0;
        if (0.0..=len + 6.0).contains(&u) && (0.0..=width).contains(&v) {
            // Ticks: every mm on the bottom edge (2, 3 and 4.5 mm long),
            // every sixteenth on the top (1.5, 2.5 and 4), 0.25 mm wide,
            // starting 3 mm in from the end.
            let tick = |pitch: f64, up: f64| -> Option<i64> {
                let k = (up / pitch).round();
                ((up - k * pitch).abs() <= 0.125 && k >= 0.0 && k * pitch <= len)
                    .then_some(k as i64)
            };
            let mm_len = |k: i64| {
                if k % 10 == 0 {
                    4.5
                } else if k % 5 == 0 {
                    3.0
                } else {
                    2.0
                }
            };
            let inch = 25.4 / 16.0;
            let in_len = |k: i64| {
                if k % 16 == 0 {
                    4.0
                } else if k % 4 == 0 {
                    2.5
                } else {
                    1.5
                }
            };
            let on_mm = tick(1.0, u - 3.0).is_some_and(|k| v <= mm_len(k));
            let on_in = tick(inch, u - 3.0).is_some_and(|k| v >= width - in_len(k));
            return Some(if on_mm || on_in {
                Rgb(25, 25, 25)
            } else {
                Rgb(175, 178, 180)
            });
        }
    }
    let on_part = scene
        .rects
        .iter()
        .any(|r| (r[0]..=r[2]).contains(&xt) && (r[1]..=r[3]).contains(&yt))
        || scene
            .discs
            .iter()
            .any(|d| ((xt - d[0]).powi(2) + (yt - d[1]).powi(2)).sqrt() <= d[2]);
    let in_hole = scene
        .holes
        .iter()
        .any(|d| ((xt - d[0]).powi(2) + (yt - d[1]).powi(2)).sqrt() <= d[2]);
    (on_part && !in_hole).then_some(Rgb(40, 40, 40))
}

/// A flatbed scan of the scene, `w` x `h` mm at `s` px/mm, white
/// background, no sheet: origin at the bottom-left, y up.
pub fn scan_image(w: f64, h: f64, s: f64, scene: &Scene) -> Image {
    let (pw, ph) = ((w * s) as usize, (h * s) as usize);
    let mut img = Image::new(pw, ph, Rgb(255, 255, 255));
    for j in 0..ph {
        for i in 0..pw {
            let xt = (i as f64 + 0.5) / s;
            let yt = h - (j as f64 + 0.5) / s;
            if let Some(c) = scene_at(scene, xt, yt) {
                img.set(i, j, c);
            }
        }
    }
    img
}

impl Scene {
    /// A photo scale's alternating bars: `n` dark blocks of `block` mm,
    /// 3 mm tall, starting at (x, y) and repeating every two blocks.
    pub fn bars(mut self, x: f64, y: f64, block: f64, n: usize) -> Scene {
        for k in 0..n {
            let x0 = x + 2.0 * block * k as f64;
            self.rects.push([x0, y, x0 + block, y + 3.0]);
        }
        self
    }
}

/// The printed sheet as a raster at `s` px per true mm, y down, as a
/// scanner would see it, with the scene lying on it. Tests in other
/// crates photograph it.
pub fn sheet_image(size: SheetSize, s: f64, scene: &Scene) -> Image {
    let (w, h) = size.size();
    let (lx, ly) = span(size);
    let (px, py) = (scene.print * scene.stretch, scene.print);
    let (pw, ph) = ((w * px * s) as usize, (h * py * s) as usize);
    let mut img = Image::new(pw, ph, Rgb(255, 255, 255));
    let corners = [(0.0, 0.0), (lx, 0.0), (lx, ly), (0.0, ly)];
    let dots = code(size);
    for j in 0..ph {
        for i in 0..pw {
            // Sheet frame: origin at the origin fiducial, y up. The
            // sheet's own features are in printed mm, the parts in true.
            let xt = (i as f64 + 0.5) / s - INSET * px;
            let yt = h * py - (j as f64 + 0.5) / s - INSET * py;
            let (x, y) = (xt / px, yt / py);
            let mut c = Rgb(255, 255, 255);
            let on_sheet = (0.0..=lx).contains(&x) && (0.0..=ly).contains(&y);
            let on_grid = on_sheet
                && (((x / 10.0).round() * 10.0 - x).abs() < 0.12
                    || ((y / 10.0).round() * 10.0 - y).abs() < 0.12);
            if on_grid {
                c = Rgb(200, 200, 200);
            }
            for (k, (fx, fy)) in corners.iter().enumerate() {
                let r = ((x - fx).powi(2) + (y - fy).powi(2)).sqrt();
                let black = r <= DOT_R
                    || (RING_HOLE_R..=RING_R).contains(&r)
                    || (k == 0 && (ORIGIN_RING_HOLE_R..=ORIGIN_RING_R).contains(&r));
                let white = r < RING_HOLE_R && r > DOT_R
                    || (k == 0 && r > RING_R && r < ORIGIN_RING_HOLE_R);
                if black {
                    c = Rgb(0, 0, 0);
                } else if white {
                    c = Rgb(255, 255, 255);
                }
            }
            for k in 0..dots {
                let cx = CODE_X0 + k as f64 * CODE_PITCH;
                if ((x - cx).powi(2) + (y - CODE_Y).powi(2)).sqrt() <= CODE_DOT_R {
                    c = Rgb(0, 0, 0);
                }
            }
            if let Some(part) = scene_at(scene, xt, yt) {
                c = part;
            }
            img.set(i, j, c);
        }
    }
    img
}

/// The printed sheet (or any picture) photographed askew: its corners
/// land on the given picture points, in order top-left, top-right,
/// bottom-right, bottom-left, in a `pw` x `ph` picture. Tests in other
/// crates use it with `sheet_image` to make a photograph.
pub fn photograph(sheet: &Image, corners: [[f64; 2]; 4], pw: usize, ph: usize) -> Image {
    let from = [
        [0.0, 0.0],
        [sheet.width as f64, 0.0],
        [sheet.width as f64, sheet.height as f64],
        [0.0, sheet.height as f64],
    ];
    let h = homography(&corners, &from).expect("four distinct corners");
    let mut out = Image::new(pw, ph, Rgb(235, 232, 225));
    for j in 0..ph {
        for i in 0..pw {
            let p = apply(&h, [i as f64 + 0.5, j as f64 + 0.5]);
            if let Some(c) = sample(sheet, p[0], p[1]) {
                out.set(i, j, c);
            }
        }
    }
    out
}

fn shrink(img: &Image, factor: usize) -> Image {
    if factor <= 1 {
        return img.clone();
    }
    let (w, h) = (img.width / factor, img.height / factor);
    let mut out = Image::new(w, h, Rgb(0, 0, 0));
    for j in 0..h {
        for i in 0..w {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for dj in 0..factor {
                for di in 0..factor {
                    let c = img.pixel(i * factor + di, j * factor + dj);
                    r += c.0 as u32;
                    g += c.1 as u32;
                    b += c.2 as u32;
                }
            }
            let n = (factor * factor) as u32;
            out.set(i, j, Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8));
        }
    }
    out
}

fn luma(img: &Image) -> Vec<u8> {
    img.data
        .chunks(3)
        .map(|c| (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) as u8)
        .collect()
}

/// Otsu's threshold: the grey level that best splits the histogram.
fn otsu(gray: &[u8]) -> u32 {
    let mut hist = [0u64; 256];
    for &g in gray {
        hist[g as usize] += 1;
    }
    let total = gray.len() as f64;
    let sum: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &n)| i as f64 * n as f64)
        .sum();
    let (mut sum_b, mut w_b, mut best, mut at) = (0.0, 0.0, 0.0, 128u32);
    for (t, &n) in hist.iter().enumerate() {
        w_b += n as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * n as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f).powi(2);
        if between > best {
            best = between;
            at = t as u32 + 1;
        }
    }
    at
}

/// Bilinear sample of a photograph at a fractional pixel position.
fn sample(img: &Image, u: f64, v: f64) -> Option<Rgb> {
    if u < 0.0 || v < 0.0 || u >= (img.width - 1) as f64 || v >= (img.height - 1) as f64 {
        return None;
    }
    let (i, j) = (u.floor() as usize, v.floor() as usize);
    let (fu, fv) = (u - i as f64, v - j as f64);
    let ch = |c: Rgb, k: usize| match k {
        0 => c.0 as f64,
        1 => c.1 as f64,
        _ => c.2 as f64,
    };
    let (a, b, c, d) = (
        img.pixel(i, j),
        img.pixel(i + 1, j),
        img.pixel(i, j + 1),
        img.pixel(i + 1, j + 1),
    );
    let mut out = [0u8; 3];
    for (k, o) in out.iter_mut().enumerate() {
        let top = ch(a, k) * (1.0 - fu) + ch(b, k) * fu;
        let bottom = ch(c, k) * (1.0 - fu) + ch(d, k) * fu;
        *o = (top * (1.0 - fv) + bottom * fv).round() as u8;
    }
    Some(Rgb(out[0], out[1], out[2]))
}

fn erode(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut out = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if !mask[j * w + i] {
                continue;
            }
            let mut all = true;
            'n: for dj in 0..=2 * r {
                for di in 0..=2 * r {
                    let (x, y) = (
                        i as isize + di as isize - r as isize,
                        j as isize + dj as isize - r as isize,
                    );
                    if x < 0
                        || y < 0
                        || x >= w as isize
                        || y >= h as isize
                        || !mask[y as usize * w + x as usize]
                    {
                        all = false;
                        break 'n;
                    }
                }
            }
            out[j * w + i] = all;
        }
    }
    out
}

fn dilate(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut out = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if !mask[j * w + i] {
                continue;
            }
            for dj in 0..=2 * r {
                for di in 0..=2 * r {
                    let (x, y) = (
                        i as isize + di as isize - r as isize,
                        j as isize + dj as isize - r as isize,
                    );
                    if x >= 0 && y >= 0 && x < w as isize && y < h as isize {
                        out[y as usize * w + x as usize] = true;
                    }
                }
            }
        }
    }
    out
}

/// Morphological opening: specks and lines thinner than `2r + 1` go.
fn open(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    dilate(&erode(mask, w, h, r), w, h, r)
}

/// A connected component of a mask, 4-connected.
#[derive(Debug, Clone)]
struct Comp {
    area: usize,
    perimeter: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    cx: f64,
    cy: f64,
    /// Sums of i*i, j*j and i*j, for the second moments.
    sxx: f64,
    syy: f64,
    sxy: f64,
}

impl Comp {
    /// Principal axes from the second central moments: the larger and
    /// smaller variance and the angle of the larger, in radians from
    /// the x axis.
    fn axes(&self) -> (f64, f64, f64) {
        let n = self.area as f64;
        let (mx, my) = (self.cx - 0.5, self.cy - 0.5);
        let vxx = self.sxx / n - mx * mx;
        let vyy = self.syy / n - my * my;
        let vxy = self.sxy / n - mx * my;
        let mean = (vxx + vyy) / 2.0;
        let d = (((vxx - vyy) / 2.0).powi(2) + vxy * vxy).sqrt();
        let angle = 0.5 * (2.0 * vxy).atan2(vxx - vyy);
        ((mean + d).max(0.0), (mean - d).max(0.0), angle)
    }
    fn width(&self) -> usize {
        self.x1 - self.x0 + 1
    }
    fn height(&self) -> usize {
        self.y1 - self.y0 + 1
    }
    fn fill(&self) -> f64 {
        self.area as f64 / (self.width() * self.height()) as f64
    }
    fn contains(&self, o: &Comp) -> bool {
        o.x0 > self.x0 && o.y0 > self.y0 && o.x1 < self.x1 && o.y1 < self.y1
    }
    /// The mask of this component alone.
    fn labels_of(&self, mask: &[bool], w: usize) -> Vec<bool> {
        // Rebuilt by a flood from the bounding box: cheaper than keeping
        // every component's pixels.
        let h = mask.len() / w;
        let mut out = vec![false; mask.len()];
        let mut seen = vec![false; mask.len()];
        let mut stack = Vec::new();
        // Any pixel of the component: the centroid may fall in a hole,
        // so scan the box for the first set pixel on the label's rows.
        'find: for j in self.y0..=self.y1 {
            for i in self.x0..=self.x1 {
                if mask[j * w + i] {
                    stack.push((i, j));
                    break 'find;
                }
            }
        }
        while let Some((i, j)) = stack.pop() {
            let k = j * w + i;
            if seen[k] || !mask[k] {
                continue;
            }
            seen[k] = true;
            out[k] = true;
            if i > 0 {
                stack.push((i - 1, j));
            }
            if j > 0 {
                stack.push((i, j - 1));
            }
            if i + 1 < w {
                stack.push((i + 1, j));
            }
            if j + 1 < h {
                stack.push((i, j + 1));
            }
        }
        out
    }
}

fn components(mask: &[bool], w: usize, h: usize) -> Vec<Comp> {
    let mut label = vec![0u32; w * h];
    let mut out = Vec::new();
    let mut next = 1u32;
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || label[start] != 0 {
            continue;
        }
        let mut c = Comp {
            area: 0,
            perimeter: 0,
            x0: usize::MAX,
            y0: usize::MAX,
            x1: 0,
            y1: 0,
            cx: 0.0,
            cy: 0.0,
            sxx: 0.0,
            syy: 0.0,
            sxy: 0.0,
        };
        stack.push(start);
        label[start] = next;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % w, k / w);
            c.area += 1;
            c.x0 = c.x0.min(i);
            c.y0 = c.y0.min(j);
            c.x1 = c.x1.max(i);
            c.y1 = c.y1.max(j);
            c.cx += i as f64;
            c.cy += j as f64;
            c.sxx += (i * i) as f64;
            c.syy += (j * j) as f64;
            c.sxy += (i * j) as f64;
            let mut edge = false;
            let mut visit = |x: isize, y: isize| {
                if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                    edge = true;
                    return;
                }
                let n = y as usize * w + x as usize;
                if !mask[n] {
                    edge = true;
                } else if label[n] == 0 {
                    label[n] = next;
                    stack.push(n);
                }
            };
            visit(i as isize - 1, j as isize);
            visit(i as isize + 1, j as isize);
            visit(i as isize, j as isize - 1);
            visit(i as isize, j as isize + 1);
            if edge {
                c.perimeter += 1;
            }
        }
        c.cx = c.cx / c.area as f64 + 0.5;
        c.cy = c.cy / c.area as f64 + 0.5;
        out.push(c);
        next += 1;
    }
    out
}

/// The four fiducials' centres in pixels (origin, x, xy, y) and the
/// number of size-code dots by the origin.
#[allow(clippy::type_complexity)]
fn find_fiducials(dark: &[bool], w: usize, h: usize) -> Result<([(f64, f64); 4], usize), String> {
    let comps = components(dark, w, h);
    let is_ring = |c: &Comp| {
        c.area >= 40 && {
            let aspect = c.width() as f64 / c.height() as f64;
            (0.6..=1.7).contains(&aspect) && (0.08..=0.8).contains(&c.fill())
        }
    };
    let is_dot = |c: &Comp| c.area >= 4 && c.fill() > 0.55;
    let concentric = |a: &Comp, b: &Comp| {
        a.contains(b)
            && ((a.cx - b.cx).powi(2) + (a.cy - b.cy).powi(2)).sqrt() < 0.2 * a.width() as f64
    };
    // Bullseyes: a ring with a dot at its centre; the origin's ring has
    // a bullseye inside it instead.
    let mut bullseyes: Vec<(usize, f64, f64)> = Vec::new(); // (ring index, dot cx, dot cy)
    for (ai, a) in comps.iter().enumerate() {
        if !is_ring(a) {
            continue;
        }
        if let Some(d) = comps
            .iter()
            .filter(|b| is_dot(b) && concentric(a, b) && b.area * 2 < a.area)
            .max_by_key(|b| b.area)
        {
            bullseyes.push((ai, d.cx, d.cy));
        }
    }
    let mut origins: Vec<(usize, usize)> = Vec::new(); // (outer ring index, bullseye index)
    for (ai, a) in comps.iter().enumerate() {
        if !is_ring(a) {
            continue;
        }
        for (bi, &(ring, _, _)) in bullseyes.iter().enumerate() {
            if ring != ai && concentric(a, &comps[ring]) {
                origins.push((ai, bi));
            }
        }
    }
    let origin = origins
        .iter()
        .max_by_key(|(ai, _)| comps[*ai].area)
        .ok_or("no origin mark (the double-ringed bullseye) in the picture")?;
    let origin_ring = &comps[origin.0];
    let mut others: Vec<&(usize, f64, f64)> = bullseyes
        .iter()
        .enumerate()
        .filter(|(bi, (ring, _, _))| *bi != origin.1 && !origin_ring.contains(&comps[*ring]))
        .map(|(_, b)| b)
        .collect();
    others.sort_by(|a, b| comps[b.0].area.cmp(&comps[a.0].area));
    if others.len() < 3 {
        return Err(format!(
            "found the origin mark and {} other bullseye(s); all four corner marks must be in the picture",
            others.len()
        ));
    }
    // Keep bullseyes of about the origin's bullseye's size (the same
    // marks at the same distance), then the three largest.
    let ref_area = comps[bullseyes[origin.1].0].area as f64;
    let mut like: Vec<&(usize, f64, f64)> = others
        .iter()
        .copied()
        .filter(|b| {
            let r = comps[b.0].area as f64 / ref_area;
            (0.3..=3.0).contains(&r)
        })
        .collect();
    if like.len() < 3 {
        like = others.clone();
    }
    like.truncate(3);
    let o = (bullseyes[origin.1].1, bullseyes[origin.1].2);
    let pts: Vec<(f64, f64)> = like.iter().map(|b| (b.1, b.2)).collect();
    // Order the other three by angle from the origin, going the way the
    // sheet's x axis leads to its y axis: in a picture (y down) the
    // sheet's counter-clockwise reads as decreasing angle.
    let cx = (o.0 + pts.iter().map(|p| p.0).sum::<f64>()) / 4.0;
    let cy = (o.1 + pts.iter().map(|p| p.1).sum::<f64>()) / 4.0;
    let angle = |p: (f64, f64)| (p.1 - cy).atan2(p.0 - cx);
    let a0 = angle(o);
    let mut ordered: Vec<((f64, f64), f64)> = pts
        .iter()
        .map(|&p| (p, (angle(p) - a0).rem_euclid(2.0 * PI)))
        .collect();
    ordered.sort_by(|a, b| b.1.total_cmp(&a.1));
    let corners = [o, ordered[0].0, ordered[1].0, ordered[2].0];
    // The size code: small dots in a row below the x axis, just past
    // the origin ring. Positions are taken in the origin's local frame
    // (x towards the x mark, y towards the y mark, scaled by the
    // origin ring), which perspective barely bends over 50 mm.
    let s = origin_ring.width() as f64 / (2.0 * ORIGIN_RING_R);
    let unit = |p: (f64, f64)| {
        let d = ((p.0 - o.0).powi(2) + (p.1 - o.1).powi(2)).sqrt();
        [(p.0 - o.0) / d, (p.1 - o.1) / d]
    };
    let (ex, ey) = (unit(corners[1]), unit(corners[3]));
    let dot_area = PI * (CODE_DOT_R * s).powi(2);
    let mut along: Vec<f64> = comps
        .iter()
        .filter(|c| c.fill() > 0.5 && (0.3..=3.0).contains(&(c.area as f64 / dot_area)))
        .filter_map(|c| {
            let (dx, dy) = (c.cx - o.0, c.cy - o.1);
            let a = (dx * ex[0] + dy * ex[1]) / s;
            let b = (dx * ey[0] + dy * ey[1]) / s;
            let first = CODE_X0 - CODE_PITCH;
            let last = CODE_X0 + CODED.len() as f64 * CODE_PITCH;
            ((first..=last).contains(&a) && (CODE_Y - 4.0..=CODE_Y + 4.0).contains(&b)).then_some(a)
        })
        .collect();
    along.sort_by(f64::total_cmp);
    // Evenly spaced: every gap near the pitch, else it is not the code.
    let even = along
        .windows(2)
        .all(|g| ((g[1] - g[0]) / CODE_PITCH - 1.0).abs() < 0.35);
    let dots = if even { along.len() } else { 0 };
    Ok((corners, dots))
}

// ---- projective geometry -----------------------------------------------

/// Row-major 3x3.
type H = [f64; 9];

/// The homography taking each `from` point to its `to` point (direct
/// linear transform on four correspondences).
pub fn homography(from: &[[f64; 2]; 4], to: &[[f64; 2]; 4]) -> Result<H, String> {
    let mut a = [[0.0f64; 9]; 8];
    for k in 0..4 {
        let ([u, v], [x, y]) = (from[k], to[k]);
        a[2 * k] = [u, v, 1.0, 0.0, 0.0, 0.0, -x * u, -x * v, x];
        a[2 * k + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -y * u, -y * v, y];
    }
    // Gaussian elimination with partial pivoting on the 8x9 system.
    for col in 0..8 {
        let pivot = (col..8)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        if a[pivot][col].abs() < 1e-12 {
            return Err("degenerate fiducial layout".into());
        }
        a.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let f = a[row][col] / a[col][col];
                let pivot_row = a[col];
                for (x, p) in a[row].iter_mut().zip(pivot_row).skip(col) {
                    *x -= f * p;
                }
            }
        }
    }
    let mut h = [0.0; 9];
    for (k, row) in a.iter().enumerate() {
        h[k] = row[8] / row[k];
    }
    h[8] = 1.0;
    Ok(h)
}

fn invert(h: &H) -> Result<H, String> {
    let [a, b, c, d, e, f, g, hh, i] = *h;
    let det = a * (e * i - f * hh) - b * (d * i - f * g) + c * (d * hh - e * g);
    if det.abs() < 1e-15 {
        return Err("singular homography".into());
    }
    let m = [
        e * i - f * hh,
        c * hh - b * i,
        b * f - c * e,
        f * g - d * i,
        a * i - c * g,
        c * d - a * f,
        d * hh - e * g,
        b * g - a * hh,
        a * e - b * d,
    ];
    Ok(m.map(|v| v / det))
}

fn apply(h: &H, p: [f64; 2]) -> [f64; 2] {
    let w = h[6] * p[0] + h[7] * p[1] + h[8];
    [
        (h[0] * p[0] + h[1] * p[1] + h[2]) / w,
        (h[3] * p[0] + h[4] * p[1] + h[5]) / w,
    ]
}

// ---- outlines ---------------------------------------------------------

/// The boundary of a component as pixel positions, traced around it
/// clockwise (Moore neighbourhood) from its top-left pixel, stopping
/// when the start is re-entered the way it was first left.
fn trace(mask: &[bool], w: usize, h: usize, c: &Comp) -> Vec<(usize, usize)> {
    let at = |q: (isize, isize)| {
        q.0 >= 0
            && q.1 >= 0
            && q.0 < w as isize
            && q.1 < h as isize
            && mask[q.1 as usize * w + q.0 as usize]
    };
    let mut start = None;
    'find: for j in c.y0..=c.y1 {
        for i in c.x0..=c.x1 {
            if mask[j * w + i] {
                start = Some((i as isize, j as isize));
                break 'find;
            }
        }
    }
    let Some(start) = start else {
        return Vec::new();
    };
    // Eight neighbours clockwise from the west.
    const N: [(isize, isize); 8] = [
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
    ];
    let index_of = |d: (isize, isize)| N.iter().position(|n| *n == d).unwrap_or(0);
    let mut out = vec![start];
    let mut p = start;
    let mut back = (start.0 - 1, start.1); // the background we came from
    let mut first: Option<usize> = None;
    let limit = 8 * (c.width() + c.height()) + 2 * c.area;
    loop {
        let bd = index_of((back.0 - p.0, back.1 - p.1));
        let mut step = None;
        for k in 0..8 {
            let d = (bd + k) % 8;
            let q = (p.0 + N[d].0, p.1 + N[d].1);
            if at(q) {
                step = Some((q, d, (bd + k + 7) % 8));
                break;
            }
        }
        let Some((q, d, before)) = step else { break }; // an isolated pixel
        if p == start {
            match first {
                None => first = Some(d),
                Some(f) if f == d => break,
                _ => {}
            }
        }
        back = (p.0 + N[before].0, p.1 + N[before].1);
        p = q;
        out.push(q);
        if out.len() > limit {
            break;
        }
    }
    if out.len() > 1 && out[out.len() - 1] == start {
        out.pop();
    }
    out.into_iter()
        .map(|(x, y)| (x as usize, y as usize))
        .collect()
}

/// Douglas–Peucker on a closed polyline, tolerance in pixels: the loop
/// is split at the point farthest from its start into two open runs.
/// Returns the indices of the points kept, in order.
fn simplify_indices(points: &[(usize, usize)], tol: f64) -> Vec<usize> {
    let n = points.len();
    if n < 4 {
        return (0..n).collect();
    }
    let mut pts: Vec<[f64; 2]> = points.iter().map(|&(x, y)| [x as f64, y as f64]).collect();
    pts.push(pts[0]);
    let far = (1..n)
        .max_by(|&a, &b| dist2(pts[a], pts[0]).total_cmp(&dist2(pts[b], pts[0])))
        .unwrap();
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    keep[far] = true;
    keep[n] = true;
    dp(&pts, 0, far, tol, &mut keep);
    dp(&pts, far, n, tol, &mut keep);
    keep[..n]
        .iter()
        .enumerate()
        .filter(|(_, k)| **k)
        .map(|(i, _)| i)
        .collect()
}

/// A least-squares circle through `pts` (Kåsa's algebraic fit): centre,
/// radius, and the largest distance of a point from the circle. None
/// when the points are too few or collinear.
fn circle_fit(pts: &[[f64; 2]]) -> Option<([f64; 2], f64, f64)> {
    if pts.len() < 3 {
        return None;
    }
    // Centre the data for conditioning.
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p[0]).sum::<f64>() / n;
    let my = pts.iter().map(|p| p[1]).sum::<f64>() / n;
    let (mut suu, mut suv, mut svv, mut suuu, mut svvv, mut suvv, mut svuu) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in pts {
        let (u, v) = (p[0] - mx, p[1] - my);
        suu += u * u;
        suv += u * v;
        svv += v * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 * (suu + svv).powi(2).max(1e-12) {
        return None;
    }
    let bu = (suuu + suvv) / 2.0;
    let bv = (svvv + svuu) / 2.0;
    let uc = (bu * svv - bv * suv) / det;
    let vc = (bv * suu - bu * suv) / det;
    let r = (uc * uc + vc * vc + (suu + svv) / n).sqrt();
    let centre = [uc + mx, vc + my];
    if !r.is_finite() || !centre[0].is_finite() || !centre[1].is_finite() {
        return None;
    }
    let dev = pts
        .iter()
        .map(|p| (dist2(*p, centre).sqrt() - r).abs())
        .fold(0.0, f64::max);
    Some((centre, r, dev))
}

/// Straight runs and arcs fitted to a closed outline (`dense`, every
/// boundary pixel in millimetres, counter-clockwise) between its
/// simplified corners (`corners`, indices into `dense`): from the
/// sharpest corner, each run is extended over the following corners as
/// far as a line or a circle stays within `tol` of every boundary point,
/// the circle taking over where it reaches further; a run of the whole
/// outline that is one circle is a `Circle`. Edges meet at corner
/// points, so they chain into a closed loop.
fn fit_edges(dense: &[[f64; 2]], corners: &[usize], tol: f64) -> Vec<Edge> {
    let (n, m) = (dense.len(), corners.len());
    if n < 8 || m < 2 {
        return Vec::new();
    }
    if let Some((centre, radius, dev)) = circle_fit(dense) {
        if dev <= tol {
            return vec![Edge::Circle { centre, radius }];
        }
    }
    // The boundary points from corner k to corner l (cyclic), inclusive.
    let run = |k: usize, l: usize| -> Vec<[f64; 2]> {
        let (s, e) = (corners[k % m], corners[l % m]);
        if e > s {
            dense[s..=e].to_vec()
        } else {
            dense[s..].iter().chain(&dense[..=e]).copied().collect()
        }
    };
    let line_dev = |pts: &[[f64; 2]]| -> f64 {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        pts.iter().map(|p| seg_dist(*p, a, b)).fold(0.0, f64::max)
    };
    // Start at the sharpest corner, so no run straddles a true corner.
    let turn = |k: usize| -> f64 {
        let (p, c, q) = (
            dense[corners[(k + m - 1) % m]],
            dense[corners[k]],
            dense[corners[(k + 1) % m]],
        );
        let (ax, ay) = (c[0] - p[0], c[1] - p[1]);
        let (bx, by) = (q[0] - c[0], q[1] - c[1]);
        let la = (ax * ax + ay * ay).sqrt();
        let lb = (bx * bx + by * by).sqrt();
        if la < 1e-12 || lb < 1e-12 {
            0.0
        } else {
            ((ax * bx + ay * by) / (la * lb)).clamp(-1.0, 1.0).acos()
        }
    };
    let start = (0..m)
        .max_by(|&a, &b| turn(a).total_cmp(&turn(b)))
        .unwrap_or(0);
    let mut edges = Vec::new();
    let mut k = start;
    let mut covered = 0;
    while covered < m {
        let max_l = k + (m - covered);
        let mut line_to = k + 1;
        let mut j = k + 2;
        while j <= max_l && line_dev(&run(k, j)) <= tol {
            line_to = j;
            j += 1;
        }
        let mut arc_to: Option<(usize, [f64; 2], f64)> = None;
        let mut j = k + 2;
        while j <= max_l {
            match circle_fit(&run(k, j)) {
                Some((c, r, dev)) if dev <= tol && r < 1000.0 => {
                    arc_to = Some((j, c, r));
                    j += 1;
                }
                _ => break,
            }
        }
        let (edge, next) = match arc_to {
            Some((j, centre, radius)) if j > line_to => {
                let pts = run(k, j);
                // The fitted circle, its ends the corner points moved onto
                // it (by at most `tol`); the neighbouring edges follow.
                let onto = |p: [f64; 2]| -> [f64; 2] {
                    let d = dist2(p, centre).sqrt();
                    if d < 1e-9 {
                        p
                    } else {
                        [
                            centre[0] + (p[0] - centre[0]) * radius / d,
                            centre[1] + (p[1] - centre[1]) * radius / d,
                        ]
                    }
                };
                let (start, end) = (onto(pts[0]), onto(pts[pts.len() - 1]));
                let through = pts[pts.len() / 2];
                let ccw = (start[0] - centre[0]) * (through[1] - centre[1])
                    - (start[1] - centre[1]) * (through[0] - centre[0])
                    > 0.0;
                (
                    Edge::Arc {
                        centre,
                        radius,
                        start,
                        end,
                        ccw,
                    },
                    j,
                )
            }
            _ => (
                Edge::Line {
                    a: dense[corners[k % m]],
                    b: dense[corners[line_to % m]],
                },
                line_to,
            ),
        };
        edges.push(edge);
        covered += next - k;
        k = next;
    }
    // Chain the edges: an arc's ends were moved onto its circle, so the
    // edge before ends and the edge after begins where the arc does.
    let count = edges.len();
    for i in 0..count {
        let next = (i + 1) % count;
        if let Some(start) = arc_start(&edges[next]) {
            match &mut edges[i] {
                Edge::Line { b, .. } => *b = start,
                Edge::Arc { end, .. } => *end = start,
                Edge::Circle { .. } => {}
            }
        } else if let Some(end) = arc_end(&edges[i]) {
            if let Edge::Line { a, .. } = &mut edges[next] {
                *a = end;
            }
        }
    }
    edges
}

fn arc_start(e: &Edge) -> Option<[f64; 2]> {
    match e {
        Edge::Arc { start, .. } => Some(*start),
        _ => None,
    }
}

fn arc_end(e: &Edge) -> Option<[f64; 2]> {
    match e {
        Edge::Arc { end, .. } => Some(*end),
        _ => None,
    }
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let l2 = dist2(a, b);
    if l2 < 1e-12 {
        return dist2(p, a).sqrt();
    }
    let t = (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / l2).clamp(0.0, 1.0);
    dist2(p, [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]).sqrt()
}

fn dp(pts: &[[f64; 2]], s: usize, e: usize, tol: f64, keep: &mut [bool]) {
    if e <= s + 1 {
        return;
    }
    let (mut worst, mut at) = (0.0, s);
    for k in s + 1..e {
        let d = seg_dist(pts[k], pts[s], pts[e]);
        if d > worst {
            worst = d;
            at = k;
        }
    }
    if worst > tol {
        keep[at] = true;
        dp(pts, s, at, tol, keep);
        dp(pts, at, e, tol, keep);
    }
}

// ---- overlay drawing ----------------------------------------------------

fn put(img: &mut Image, x: isize, y: isize, c: Rgb) {
    if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
        img.set(x as usize, y as usize, c);
    }
}

fn hline(img: &mut Image, x0: isize, x1: isize, y: isize, c: Rgb) {
    for x in x0.min(x1)..=x0.max(x1) {
        put(img, x, y, c);
    }
}

fn vline(img: &mut Image, x: isize, y0: isize, y1: isize, c: Rgb) {
    for y in y0.min(y1)..=y0.max(y1) {
        put(img, x, y, c);
    }
}

fn cross(img: &mut Image, x: isize, y: isize, r: isize, c: Rgb) {
    hline(img, x - r, x + r, y, c);
    vline(img, x, y - r, y + r, c);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_is_a_pdf_with_four_marks_and_a_grid() {
        let pdf = sheet_pdf(SheetSize::A4);
        let text = ok_sheet::pdf::inflated(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 841.89 595.276]"));
        // Filled discs: origin ring, its hole, three per bullseye, and
        // the one code dot of A4.
        assert_eq!(text.matches("c h f Q").count(), 2 + 4 * 3 + 1);
        assert_eq!(code(SheetSize::A4), 1);
        let letter = ok_sheet::pdf::inflated(&sheet_pdf(SheetSize::Letter));
        assert_eq!(letter.matches("c h f Q").count(), 2 + 4 * 3 + 2);
        assert_eq!(size_of_code(2), Some(SheetSize::Letter));
        assert_eq!(size_of_code(5), Some(SheetSize::Tabloid));
        assert_eq!(size_of_code(0), None);
        assert_eq!(size_of_code(6), None);
        assert!(text.contains("(100) Tj") && text.contains("(150) Tj"));
        assert!(text.contains("the bar below is 100 mm"));
        let (lx, ly) = span(SheetSize::A4);
        assert!((lx - 247.0).abs() < 1e-9 && (ly - 160.0).abs() < 1e-9);
    }

    #[test]
    fn a_photograph_of_the_sheet_measures_what_lies_on_it() {
        // A 40 x 25 rectangle at (60, 50) with a 6 mm hole in its middle,
        // and a 30 mm disc at (150, 100).
        let sheet = sheet_image(
            SheetSize::A4,
            6.0,
            &Scene {
                rects: vec![[60.0, 50.0, 100.0, 75.0]],
                discs: vec![[150.0, 100.0, 15.0]],
                holes: vec![[80.0, 62.5, 3.0]],
                ..Scene::default()
            },
        );
        let photo = photograph(
            &sheet,
            [
                [130.0, 95.0],
                [1470.0, 150.0],
                [1400.0, 1120.0],
                [180.0, 1060.0],
            ],
            1600,
            1200,
        );
        let m = measure_image(&photo, None, None).unwrap();
        assert!(
            m.sheet == Some(SheetSize::A4) && m.sheet_read,
            "{:?}",
            m.sheet
        );
        assert!(m.calibration.is_none());
        assert!(m.residual < 1.5, "fiducial fit {} px", m.residual);
        assert!((m.mm_per_pixel - 0.23).abs() < 0.03, "{}", m.mm_per_pixel);
        // The origin mark is near the picture's bottom-left, x runs right.
        assert!(
            m.fiducials[0][0] < 300.0 && m.fiducials[0][1] > 900.0,
            "{:?}",
            m.fiducials
        );
        assert!(m.fiducials[1][0] > 1200.0, "{:?}", m.fiducials);
        assert_eq!(
            m.parts.len(),
            2,
            "{:?}",
            m.parts.iter().map(|p| p.bbox).collect::<Vec<_>>()
        );
        // Largest first: the rectangle, then the disc.
        let rect = &m.parts[0];
        let disc = &m.parts[1];
        for (got, want) in disc.bbox.iter().zip([135.0, 85.0, 165.0, 115.0]) {
            assert!((got - want).abs() < 0.6, "disc box {:?}", disc.bbox);
        }
        assert!(
            (disc.area - PI * 225.0).abs() / (PI * 225.0) < 0.03,
            "disc area {}",
            disc.area
        );
        assert!(disc.holes.is_empty());
        for (got, want) in rect.bbox.iter().zip([60.0, 50.0, 100.0, 75.0]) {
            assert!((got - want).abs() < 0.6, "rect box {:?}", rect.bbox);
        }
        assert!(
            (rect.area - (1000.0 - PI * 9.0)).abs() / 1000.0 < 0.03,
            "rect area {}",
            rect.area
        );
        assert_eq!(rect.holes.len(), 1, "{:?}", rect.holes);
        let hole = &rect.holes[0];
        assert!((hole.diameter - 6.0).abs() < 0.4, "hole {hole:?}");
        assert!(
            (hole.centre[0] - 80.0).abs() < 0.5 && (hole.centre[1] - 62.5).abs() < 0.5,
            "hole {hole:?}"
        );
        assert!(hole.circularity > 0.8, "hole {hole:?}");
        assert!(rect.circularity < 0.85, "rect {}", rect.circularity);
        assert!(disc.circularity > 0.85, "disc {}", disc.circularity);
        assert!((disc.diameter - 30.0).abs() < 0.5, "disc {}", disc.diameter);
        // The rectangle's outline simplifies to about its four corners.
        assert!(
            rect.outline.len() >= 4 && rect.outline.len() <= 12,
            "{} outline points",
            rect.outline.len()
        );
        // Fitted edges: the rectangle is four straight runs meeting at its
        // corners, the disc one circle.
        assert_eq!(rect.edges.len(), 4, "{:?}", rect.edges);
        for e in &rect.edges {
            let Edge::Line { a, b } = e else {
                panic!("{e:?}")
            };
            for p in [a, b] {
                let near_x = (p[0] - 60.0).abs() < 0.6 || (p[0] - 100.0).abs() < 0.6;
                let near_y = (p[1] - 50.0).abs() < 0.6 || (p[1] - 75.0).abs() < 0.6;
                assert!(near_x && near_y, "corner {p:?}");
            }
        }
        match disc.edges.as_slice() {
            [Edge::Circle { centre, radius }] => {
                assert!(
                    (centre[0] - 150.0).abs() < 0.5 && (centre[1] - 100.0).abs() < 0.5,
                    "{centre:?}"
                );
                assert!((radius - 15.0).abs() < 0.4, "{radius}");
            }
            other => panic!("{other:?}"),
        }
        let picture = ok_render::from_png(&m.picture).unwrap();
        assert_eq!(picture.width, ((247.0 + 30.0) * SCALE) as usize);
        assert_eq!(picture.height, ((160.0 + 30.0) * SCALE) as usize);
    }

    #[test]
    fn a_tab_with_a_rounded_end_fits_three_lines_and_an_arc() {
        // A 40 x 25 bar whose right end is a half disc of radius 12.5,
        // and a notch: a 5 mm hole broken into the bottom edge.
        let scan = sheet_image(
            SheetSize::A4,
            6.0,
            &Scene {
                rects: vec![[60.0, 50.0, 100.0, 75.0]],
                discs: vec![[100.0, 62.5, 12.5]],
                holes: vec![[75.0, 50.0, 2.5]],
                ..Scene::default()
            },
        );
        let m = measure_image(&scan, None, None).unwrap();
        assert_eq!(m.parts.len(), 1, "{:?}", m.parts);
        let tab = &m.parts[0];
        let lines: Vec<_> = tab
            .edges
            .iter()
            .filter(|e| matches!(e, Edge::Line { .. }))
            .collect();
        let arcs: Vec<_> = tab
            .edges
            .iter()
            .filter_map(|e| match e {
                Edge::Arc {
                    centre,
                    radius,
                    ccw,
                    ..
                } => Some((*centre, *radius, *ccw)),
                _ => None,
            })
            .collect();
        // Left, top and bottom (the bottom in two pieces either side of the notch).
        assert_eq!(lines.len(), 4, "{:?}", tab.edges);
        assert_eq!(arcs.len(), 2, "{:?}", tab.edges);
        let (end, notch) = if arcs[0].1 > arcs[1].1 {
            (arcs[0], arcs[1])
        } else {
            (arcs[1], arcs[0])
        };
        assert!(
            (end.0[0] - 100.0).abs() < 0.7 && (end.0[1] - 62.5).abs() < 0.7,
            "end centre {:?}",
            end.0
        );
        assert!((end.1 - 12.5).abs() < 0.5, "end radius {}", end.1);
        assert!(end.2, "the rounded end turns counter-clockwise");
        assert!(
            (notch.0[0] - 75.0).abs() < 0.7 && (notch.0[1] - 50.0).abs() < 0.7,
            "notch centre {:?}",
            notch.0
        );
        assert!((notch.1 - 2.5).abs() < 0.5, "notch radius {}", notch.1);
        assert!(!notch.2, "the notch turns clockwise");
        // The edges chain: each ends where the next begins.
        let ends = |e: &Edge| match e {
            Edge::Line { a, b } => (*a, *b),
            Edge::Arc { start, end, .. } => (*start, *end),
            Edge::Circle { centre, .. } => (*centre, *centre),
        };
        for (i, e) in tab.edges.iter().enumerate() {
            let next = &tab.edges[(i + 1) % tab.edges.len()];
            assert!(
                dist2(ends(e).1, ends(next).0) < 1e-12,
                "{e:?} then {next:?}"
            );
        }
    }

    #[test]
    fn a_short_print_is_read_from_its_dots_and_corrected_by_its_bars() {
        // A Letter sheet printed at 96 %, with a 10 mm bar scale, a
        // quarter and a 40 x 20 plate on it, photographed askew.
        let scene = Scene {
            rects: vec![[100.0, 30.0, 140.0, 50.0]],
            discs: vec![[60.0, 100.0, 24.26 / 2.0]],
            print: 0.96,
            ..Scene::default()
        }
        .bars(120.0, 110.0, 10.0, 5);
        let sheet = sheet_image(SheetSize::Letter, 6.0, &scene);
        let photo = photograph(
            &sheet,
            [
                [120.0, 110.0],
                [1500.0, 160.0],
                [1440.0, 1120.0],
                [170.0, 1060.0],
            ],
            1600,
            1200,
        );
        // Uncorrected, the sheet reads as Letter from its dots and
        // everything comes out 1/0.96 too big.
        let m = measure_image(&photo, Some(SheetSize::A4), None).unwrap();
        assert!(
            m.sheet == Some(SheetSize::Letter) && m.sheet_read,
            "{:?}",
            m.sheet
        );
        let plate = &m.parts[0];
        assert!(
            (plate.bbox[2] - plate.bbox[0] - 40.0 / 0.96).abs() < 0.7,
            "{:?}",
            plate.bbox
        );
        // With the bars named, the print scale is read and corrected.
        let m = measure_image(&photo, None, Some(&Reference::Bars(10.0))).unwrap();
        let cal = m.calibration.as_ref().expect("calibrated");
        assert!((cal.factor - 0.96).abs() < 0.01, "{cal:?}");
        assert!(
            (cal.nominal - 20.0).abs() < 1e-9 && (cal.measured - 20.0 / 0.96).abs() < 0.3,
            "{cal:?}"
        );
        assert_eq!(m.parts.len(), 2, "the bars are not parts: {:?}", m.parts);
        let plate = &m.parts[0];
        for (got, want) in plate.bbox.iter().zip([100.0, 30.0, 140.0, 50.0]) {
            assert!((got - want).abs() < 0.6, "plate {:?}", plate.bbox);
        }
        let coin = &m.parts[1];
        assert!(
            coin.circularity > 0.85 && (coin.diameter - 24.26).abs() < 0.4,
            "{coin:?}"
        );
        // The coin as the reference instead: the same answer, without the coin.
        let m = measure_image(&photo, None, Some(&Reference::parse("quarter").unwrap())).unwrap();
        let cal = m.calibration.as_ref().expect("calibrated");
        assert!((cal.factor - 0.96).abs() < 0.015, "{cal:?}");
        // The plate and the five bars, which are only parts this time.
        assert_eq!(m.parts.len(), 6, "{:?}", m.parts);
        assert!(
            m.parts.iter().all(|p| p.circularity < 0.85),
            "the coin stays out: {:?}",
            m.parts
        );
        let err = measure_image(&photo, None, Some(&Reference::Disc(50.0))).unwrap_err();
        assert!(err.contains("not found"), "{err}");
        assert_eq!(
            Reference::parse("bars 10mm").unwrap(),
            Reference::Bars(10.0)
        );
        assert_eq!(Reference::parse("disc 22").unwrap(), Reference::Disc(22.0));
        assert!(Reference::parse("tape").is_err());
    }

    #[test]
    fn a_flat_scan_is_measured_from_a_steel_rule() {
        // A 300 dpi scan (0.0847 mm/px) of a 30 x 20 plate with a 5 mm
        // hole, a 12 mm disc, and a 150 mm rule laid at 4 degrees.
        let scene = Scene {
            rects: vec![[20.0, 20.0, 50.0, 40.0]],
            discs: vec![[80.0, 30.0, 6.0]],
            holes: vec![[35.0, 30.0, 2.5]],
            rule: Some((15.0, 60.0, 150.0, 4.0)),
            ..Scene::default()
        };
        let scan = scan_image(200.0, 120.0, 300.0 / 25.4, &scene);
        let err = measure_image(&scan, None, None).unwrap_err();
        assert!(err.contains("origin mark"), "{err}");
        let m = measure_image(&scan, None, Some(&Reference::Rule)).unwrap();
        assert!(m.sheet.is_none() && !m.sheet_read);
        let rule = m.rule.as_ref().expect("rule read");
        assert!((rule.px_per_mm - 300.0 / 25.4).abs() < 0.02, "{rule:?}");
        assert!(rule.ticks >= 140 && rule.length > 140.0, "{rule:?}");
        assert!(rule.edge.starts_with("sixteenths"), "{rule:?}");
        assert!(
            (m.mm_per_pixel - 25.4 / 300.0).abs() < 1e-4,
            "{}",
            m.mm_per_pixel
        );
        assert_eq!(m.parts.len(), 2, "the rule is not a part: {:?}", m.parts);
        let plate = &m.parts[0];
        for (got, want) in plate.bbox.iter().zip([20.0, 20.0, 50.0, 40.0]) {
            assert!((got - want).abs() < 0.5, "plate {:?}", plate.bbox);
        }
        assert!(
            (plate.holes[0].diameter - 5.0).abs() < 0.4,
            "{:?}",
            plate.holes
        );
        let disc = &m.parts[1];
        assert!(
            disc.circularity > 0.85 && (disc.diameter - 12.0).abs() < 0.4,
            "{disc:?}"
        );
        assert!((disc.centroid[0] - 80.0).abs() < 0.5 && (disc.centroid[1] - 30.0).abs() < 0.5);
        // The picture spans the scan in millimetres.
        let picture = ok_render::from_png(&m.picture).unwrap();
        assert!((picture.width as f64 - (200.0 + 30.0) * SCALE).abs() < 4.0);
    }

    #[test]
    fn a_rule_on_a_short_print_gives_the_print_scale() {
        // The Letter sheet printed at 95 %, scanned at 300 dpi, with a
        // rule and a 40 x 20 plate on it.
        let scene = Scene {
            rects: vec![[100.0, 30.0, 140.0, 50.0]],
            rule: Some((30.0, 90.0, 150.0, -2.0)),
            print: 0.95,
            ..Scene::default()
        };
        let scan = sheet_image(SheetSize::Letter, 300.0 / 25.4, &scene);
        let m = measure_image(&scan, None, Some(&Reference::Rule)).unwrap();
        assert!(
            m.sheet == Some(SheetSize::Letter) && m.sheet_read,
            "{:?}",
            m.sheet
        );
        let cal = m.calibration.as_ref().expect("calibrated");
        assert!((cal.factor - 0.95).abs() < 0.005, "{cal:?}");
        assert_eq!(m.parts.len(), 1, "the rule is not a part: {:?}", m.parts);
        let plate = &m.parts[0];
        for (got, want) in plate.bbox.iter().zip([100.0, 30.0, 140.0, 50.0]) {
            assert!((got - want).abs() < 0.5, "plate {:?}", plate.bbox);
        }
        assert_eq!(Reference::parse("rule").unwrap(), Reference::Rule);
    }

    #[test]
    fn a_phone_photograph_of_the_sheet_reads_the_rule_on_it() {
        // A 12 megapixel phone picture (4000 x 3000) of the Letter sheet
        // printed at 96 %, filling most of the frame and a little askew,
        // with a 150 mm rule at 5 degrees and a 40 x 20 plate on it.
        let scene = Scene {
            rects: vec![[100.0, 30.0, 140.0, 50.0]],
            rule: Some((30.0, 90.0, 150.0, 5.0)),
            print: 0.96,
            ..Scene::default()
        };
        let sheet = sheet_image(SheetSize::Letter, 16.0, &scene);
        let photo = photograph(
            &sheet,
            [
                [180.0, 160.0],
                [3850.0, 240.0],
                [3780.0, 2860.0],
                [240.0, 2760.0],
            ],
            4000,
            3000,
        );
        let m = measure_image(&photo, None, Some(&Reference::Rule)).unwrap();
        let rule = m.rule.as_ref().expect("rule read");
        assert!(rule.ticks >= 120, "{rule:?}");
        let cal = m.calibration.as_ref().expect("calibrated");
        assert!(
            (cal.factor - 0.96).abs() < 0.005,
            "{cal:?} rule {rule:?} mm/px {}",
            m.mm_per_pixel
        );
        let plate = &m.parts[0];
        for (got, want) in plate.bbox.iter().zip([100.0, 30.0, 140.0, 50.0]) {
            assert!((got - want).abs() < 0.6, "plate {:?}", plate.bbox);
        }
    }

    /// A PNG with a pHYs chunk claiming `dpi` added after its header.
    fn with_dpi(png: &[u8], dpi_x: f64, dpi_y: f64) -> Vec<u8> {
        let crc = |bytes: &[u8]| -> u32 {
            let mut c = 0xFFFF_FFFFu32;
            for &b in bytes {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 == 1 {
                        0xEDB8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
            }
            !c
        };
        let ihdr_end = 8 + 4 + 4 + 13 + 4;
        let mut data = b"pHYs".to_vec();
        data.extend_from_slice(&((dpi_x / 0.0254).round() as u32).to_be_bytes());
        data.extend_from_slice(&((dpi_y / 0.0254).round() as u32).to_be_bytes());
        data.push(1);
        let mut out = png[..ihdr_end].to_vec();
        out.extend_from_slice(&9u32.to_be_bytes());
        out.extend_from_slice(&data);
        out.extend_from_slice(&crc(&data).to_be_bytes());
        out.extend_from_slice(&png[ihdr_end..]);
        out
    }

    #[test]
    fn the_resolution_a_file_claims_is_read() {
        let png = ok_render::to_png(&Image::new(4, 3, Rgb(0, 0, 0)));
        assert_eq!(dpi_of(&png), None);
        let (x, y) = dpi_of(&with_dpi(&png, 300.0, 300.0)).unwrap();
        assert!(
            (x - 300.0).abs() < 0.5 && (y - 300.0).abs() < 0.5,
            "{x} {y}"
        );
        assert_eq!(decode(&with_dpi(&png, 300.0, 300.0)).unwrap().width, 4);
        // A JFIF header: version 1.1, units 1 (dpi), 300 x 200.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 1, 0x01, 0x2C,
            0x00, 0xC8, 0, 0,
        ];
        assert_eq!(dpi_of(&jpeg), Some((300.0, 200.0)));
        let mut dpcm = jpeg;
        dpcm[13] = 2;
        let (x, _) = dpi_of(&dpcm).unwrap();
        assert!((x - 300.0 * 2.54).abs() < 1e-9, "{x}");
        dpcm[13] = 0;
        assert_eq!(dpi_of(&dpcm), None);
    }

    #[test]
    fn a_scan_whose_dpi_disagrees_with_the_rule_is_warned_about() {
        let scene = Scene {
            rects: vec![[20.0, 20.0, 50.0, 40.0]],
            rule: Some((15.0, 60.0, 150.0, 4.0)),
            ..Scene::default()
        };
        let scan = ok_render::to_png(&scan_image(200.0, 120.0, 300.0 / 25.4, &scene));
        let agree = measure(&with_dpi(&scan, 300.0, 300.0), None, Some(&Reference::Rule)).unwrap();
        assert!((agree.dpi.unwrap() - 300.0).abs() < 0.5, "{:?}", agree.dpi);
        assert!(agree.warnings.is_empty(), "{:?}", agree.warnings);
        let off = measure(&with_dpi(&scan, 400.0, 400.0), None, Some(&Reference::Rule)).unwrap();
        assert_eq!(off.warnings.len(), 1, "{:?}", off.warnings);
        assert!(
            off.warnings[0].contains("400 dpi") && off.warnings[0].contains("the rule is trusted"),
            "{:?}",
            off.warnings
        );
        // Sizes still come from the rule.
        assert!((off.parts[0].bbox[2] - off.parts[0].bbox[0] - 30.0).abs() < 0.5);
        let aniso = measure(&with_dpi(&scan, 300.0, 310.0), None, Some(&Reference::Rule)).unwrap();
        assert!(
            aniso
                .warnings
                .iter()
                .any(|w| w.contains("different resolutions each way")),
            "{:?}",
            aniso.warnings
        );
    }

    #[test]
    fn a_print_stretched_one_way_is_warned_about_by_a_disc() {
        // A sheet printed 3 % wider than tall, with a quarter on it.
        let scan = sheet_image(
            SheetSize::A4,
            6.0,
            &Scene {
                rects: vec![[60.0, 50.0, 100.0, 75.0]],
                discs: vec![[150.0, 100.0, 24.26 / 2.0]],
                stretch: 1.03,
                ..Scene::default()
            },
        );
        let m = measure_image(&scan, None, Some(&Reference::Disc(24.26))).unwrap();
        let c = m.calibration.as_ref().unwrap();
        // On the sheet's (stretched) millimetres the true disc reads narrower.
        assert!((c.aspect - 1.0 / 1.03).abs() < 0.006, "aspect {}", c.aspect);
        assert_eq!(m.warnings.len(), 1, "{:?}", m.warnings);
        assert!(
            m.warnings[0].contains("stretched") && m.warnings[0].contains("narrower than tall"),
            "{:?}",
            m.warnings
        );
        let plain = sheet_image(
            SheetSize::A4,
            6.0,
            &Scene {
                discs: vec![[150.0, 100.0, 24.26 / 2.0]],
                ..Scene::default()
            },
        );
        let m = measure_image(&plain, None, Some(&Reference::Disc(24.26))).unwrap();
        assert!((m.calibration.unwrap().aspect - 1.0).abs() < 0.01);
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
        // A scanned sheet whose file says 300 dpi while the marks say it was printed short.
        let short = sheet_image(
            SheetSize::A4,
            300.0 / 25.4,
            &Scene {
                print: 0.97,
                ..Scene::default()
            },
        );
        let m = measure(
            &with_dpi(&ok_render::to_png(&short), 300.0, 300.0),
            None,
            None,
        )
        .unwrap();
        assert_eq!(m.warnings.len(), 1, "{:?}", m.warnings);
        assert!(
            m.warnings[0].contains("printed at 97.") && m.warnings[0].contains("name a reference"),
            "{:?}",
            m.warnings
        );
    }

    #[test]
    fn a_picture_without_the_marks_is_refused() {
        let blank = Image::new(400, 300, Rgb(255, 255, 255));
        let err = measure_image(&blank, Some(SheetSize::A4), None).unwrap_err();
        assert!(err.contains("origin mark"), "{err}");
    }
}
