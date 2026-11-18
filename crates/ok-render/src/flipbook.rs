//! Flip books: flat animations of filled and outlined polygons, written
//! as an animated GIF.
//!
//! A book is a list of frames. Each frame paints some shapes onto the
//! page itself (`keep`: there in that frame and in every later one) and
//! some over it for that frame only (`draw`). That is enough to show a
//! thing being made: what has been done is kept, what is being done is
//! drawn. Shapes are opaque and painted in order.
//!
//! The GIF is written with each frame's own colours (up to 255 of them,
//! the commonest if there are more) and only the pixels that changed
//! since the frame before, so a long animation of small changes stays
//! small. [`read_gif`] reads such a file back, for tests.

use crate::raster::{Image, Rgb};
use std::collections::BTreeMap;

/// One polygon or polyline, in the book's own units, y up.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub points: Vec<(f64, f64)>,
    /// Filled with this, by the even-odd rule.
    pub fill: Option<Rgb>,
    /// Outlined with this.
    pub stroke: Option<Rgb>,
    /// The outline's width, in pixels of the finished frame.
    pub width: f64,
    /// Whether the outline goes back from the last point to the first.
    pub closed: bool,
}

/// One frame of a book.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    /// Painted onto the page: in this frame and in every later one.
    pub keep: Vec<Shape>,
    /// Painted over the page for this frame only.
    pub draw: Vec<Shape>,
    /// How long the frame is shown, in milliseconds.
    pub ms: u32,
}

/// A flat animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    pub width: usize,
    pub height: usize,
    /// What a frame shows: x0, y0, x1, y1 in the shapes' units. It is
    /// scaled the same both ways and centred in the frame.
    pub view: [f64; 4],
    pub background: Rgb,
    pub frames: Vec<Frame>,
    /// Whether the animation starts again when it ends.
    pub repeat: bool,
}

/// Every pixel is this many samples each way: edges are smooth.
const SAMPLES: usize = 3;
/// The largest frame, each way, and the most frames.
pub const MOST: (usize, usize) = (2000, 5000);

/// The page being painted, at `SAMPLES` to a pixel.
struct Page {
    w: usize,
    h: usize,
    data: Vec<u8>,
    scale: f64,
    left: f64,
    top: f64,
}

impl Page {
    fn new(book: &Book) -> Result<Page, String> {
        let [x0, y0, x1, y1] = book.view;
        if !(book.view.iter().all(|v| v.is_finite()) && x1 > x0 && y1 > y0) {
            return Err("a flip book's view is x0, y0, x1, y1 with x1 > x0 and y1 > y0".into());
        }
        if book.width == 0 || book.height == 0 || book.width > MOST.0 || book.height > MOST.0 {
            return Err(format!(
                "a flip book's frame is 1 to {} pixels each way",
                MOST.0
            ));
        }
        if book.frames.is_empty() || book.frames.len() > MOST.1 {
            return Err(format!("a flip book has 1 to {} frames", MOST.1));
        }
        let (w, h) = (book.width * SAMPLES, book.height * SAMPLES);
        let scale = (w as f64 / (x1 - x0)).min(h as f64 / (y1 - y0));
        let mut data = Vec::with_capacity(w * h * 3);
        for _ in 0..w * h {
            data.extend([book.background.0, book.background.1, book.background.2]);
        }
        Ok(Page {
            w,
            h,
            data,
            scale,
            left: (w as f64 - scale * (x1 - x0)) / 2.0 - scale * x0,
            top: (h as f64 - scale * (y1 - y0)) / 2.0 + scale * y1,
        })
    }

    /// Fills a polygon given in samples, by the even-odd rule: a
    /// sample is in it if its centre is.
    fn fill(&mut self, pts: &[(f64, f64)], c: Rgb) {
        if pts.len() < 3 {
            return;
        }
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
        let first = (lo - 0.5).ceil().max(0.0) as usize;
        let last = ((hi - 0.5).ceil().max(0.0) as usize).min(self.h);
        let mut xs: Vec<f64> = Vec::new();
        for row in first..last {
            let y = row as f64 + 0.5;
            xs.clear();
            for (i, a) in pts.iter().enumerate() {
                let b = pts[(i + 1) % pts.len()];
                if (a.1 <= y) != (b.1 <= y) {
                    xs.push(a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
            for pair in xs.chunks_exact(2) {
                let from = (pair[0] - 0.5).ceil().max(0.0) as usize;
                let to = ((pair[1] - 0.5).ceil().max(0.0) as usize).min(self.w);
                for cell in self.data[(row * self.w + from.min(to)) * 3..(row * self.w + to) * 3]
                    .chunks_exact_mut(3)
                {
                    cell[0] = c.0;
                    cell[1] = c.1;
                    cell[2] = c.2;
                }
            }
        }
    }

    fn paint(&mut self, shape: &Shape) {
        let pts: Vec<(f64, f64)> = shape
            .points
            .iter()
            .map(|p| (self.left + self.scale * p.0, self.top - self.scale * p.1))
            .collect();
        if let Some(c) = shape.fill {
            self.fill(&pts, c);
        }
        let Some(c) = shape.stroke else { return };
        let half = shape.width.max(0.0) * SAMPLES as f64 / 2.0;
        if half <= 0.0 || pts.is_empty() {
            return;
        }
        // an outline is a strip along every side and a round at every corner
        let sides = if shape.closed && pts.len() > 2 {
            pts.len()
        } else {
            pts.len() - 1
        };
        for i in 0..sides {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let long = (b.0 - a.0).hypot(b.1 - a.1);
            if long < 1e-9 {
                continue;
            }
            let n = (-(b.1 - a.1) / long * half, (b.0 - a.0) / long * half);
            self.fill(
                &[
                    (a.0 + n.0, a.1 + n.1),
                    (b.0 + n.0, b.1 + n.1),
                    (b.0 - n.0, b.1 - n.1),
                    (a.0 - n.0, a.1 - n.1),
                ],
                c,
            );
        }
        for p in &pts {
            let round: Vec<(f64, f64)> = (0..12)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 12.0;
                    (p.0 + half * a.cos(), p.1 + half * a.sin())
                })
                .collect();
            self.fill(&round, c);
        }
    }

    /// The page at a pixel to a pixel: each the mean of its samples.
    fn image(&self) -> Image {
        let (w, h) = (self.w / SAMPLES, self.h / SAMPLES);
        let mut data = Vec::with_capacity(w * h * 3);
        let per = (SAMPLES * SAMPLES) as u32;
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; 3];
                for dy in 0..SAMPLES {
                    let at = ((y * SAMPLES + dy) * self.w + x * SAMPLES) * 3;
                    for cell in self.data[at..at + SAMPLES * 3].chunks_exact(3) {
                        sum[0] += cell[0] as u32;
                        sum[1] += cell[1] as u32;
                        sum[2] += cell[2] as u32;
                    }
                }
                data.extend(sum.map(|s| ((s + per / 2) / per) as u8));
            }
        }
        Image {
            width: w,
            height: h,
            data,
        }
    }
}

/// Renders the frames one after another and hands each to `each` with
/// its number and how long it is shown.
pub fn play(
    book: &Book,
    mut each: impl FnMut(usize, &Image, u32) -> Result<(), String>,
) -> Result<(), String> {
    let mut page = Page::new(book)?;
    for (n, frame) in book.frames.iter().enumerate() {
        for shape in &frame.keep {
            page.paint(shape);
        }
        if frame.draw.is_empty() {
            each(n, &page.image(), frame.ms)?;
            continue;
        }
        let kept = page.data.clone();
        for shape in &frame.draw {
            page.paint(shape);
        }
        each(n, &page.image(), frame.ms)?;
        page.data = kept;
    }
    Ok(())
}

/// One frame of the book, as an image.
pub fn still(book: &Book, index: usize) -> Result<Image, String> {
    if index >= book.frames.len() {
        return Err(format!("frame {index} of a book of {}", book.frames.len()));
    }
    let mut cut = book.clone();
    cut.frames.truncate(index + 1);
    let mut found = None;
    play(&cut, |n, image, _| {
        if n == index {
            found = Some(image.clone());
        }
        Ok(())
    })?;
    found.ok_or_else(|| "no such frame".into())
}

/// The book as an animated GIF.
pub fn gif(book: &Book) -> Result<Vec<u8>, String> {
    gif_each(book, |_, _| Ok(()))
}

/// The book as an animated GIF, each frame also handed to `seen` as it
/// is rendered: for stills from the same pass.
pub fn gif_each(
    book: &Book,
    mut seen: impl FnMut(usize, &Image) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend(b"GIF89a");
    out.extend((book.width as u16).to_le_bytes());
    out.extend((book.height as u16).to_le_bytes());
    out.extend([0u8, 0, 0]); // no colours of its own: every frame brings its own
    if book.repeat {
        out.extend([0x21, 0xff, 0x0b]);
        out.extend(b"NETSCAPE2.0");
        out.extend([0x03, 0x01, 0x00, 0x00, 0x00]);
    }
    let mut shown: Option<Vec<u8>> = None;
    play(book, |n, image, ms| {
        seen(n, image)?;
        write_frame(&mut out, &mut shown, image, ms);
        Ok(())
    })?;
    out.push(0x3b);
    Ok(out)
}

/// The colours a frame is written in: the ones it has, or, if it has
/// more than 255, those gathered into 255 boxes by median cut (the
/// widest box split in two at the middle of its pixels, again and
/// again) and each box's mean.
fn palette(counts: &BTreeMap<[u8; 3], u32>) -> Vec<[u8; 3]> {
    if counts.len() <= 255 {
        return counts.keys().copied().collect();
    }
    let mut boxes: Vec<Vec<([u8; 3], u32)>> = vec![counts.iter().map(|(c, n)| (*c, *n)).collect()];
    while boxes.len() < 255 {
        let mut widest: Option<(u8, usize, usize)> = None;
        for (i, b) in boxes.iter().enumerate().filter(|(_, b)| b.len() > 1) {
            for k in 0..3 {
                let lo = b.iter().map(|e| e.0[k]).min().unwrap();
                let hi = b.iter().map(|e| e.0[k]).max().unwrap();
                if widest.is_none_or(|w| hi - lo > w.0) {
                    widest = Some((hi - lo, i, k));
                }
            }
        }
        let Some((_, i, k)) = widest else { break };
        let mut b = boxes.swap_remove(i);
        b.sort_by_key(|e| (e.0[k], e.0));
        let total: u64 = b.iter().map(|e| e.1 as u64).sum();
        let (mut run, mut at) = (0u64, 1usize);
        for (j, e) in b.iter().enumerate() {
            run += e.1 as u64;
            if run * 2 >= total {
                at = (j + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(at);
        boxes.push(b);
        boxes.push(rest);
    }
    boxes
        .iter()
        .map(|b| {
            let total: u64 = b.iter().map(|e| e.1 as u64).sum();
            [0, 1, 2].map(|k| {
                let sum: u64 = b.iter().map(|e| e.0[k] as u64 * e.1 as u64).sum();
                ((sum + total / 2) / total) as u8
            })
        })
        .collect()
}

fn nearest(table: &[[u8; 3]], c: [u8; 3]) -> u8 {
    let mut best = (u32::MAX, 0usize);
    for (i, p) in table.iter().enumerate() {
        let d: u32 = (0..3)
            .map(|k| {
                let e = p[k] as i32 - c[k] as i32;
                (e * e) as u32
            })
            .sum();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1 as u8
}

/// Appends one frame: only the box of pixels that differ from what is
/// already showing, the unchanged ones inside it see-through.
fn write_frame(out: &mut Vec<u8>, shown: &mut Option<Vec<u8>>, image: &Image, ms: u32) {
    let (w, h) = (image.width, image.height);
    let before = shown.as_deref();
    let differs = |i: usize| match before {
        Some(old) => old[i * 3..i * 3 + 3] != image.data[i * 3..i * 3 + 3],
        None => true,
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if differs(y * w + x) {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    let nothing = x1 <= x0;
    if nothing {
        (x0, y0, x1, y1) = (0, 0, 1, 1); // a frame all the same: one see-through pixel, for its time
    }
    let mut counts: BTreeMap<[u8; 3], u32> = BTreeMap::new();
    if !nothing {
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * w + x;
                if differs(i) {
                    let c = [
                        image.data[i * 3],
                        image.data[i * 3 + 1],
                        image.data[i * 3 + 2],
                    ];
                    *counts.entry(c).or_insert(0) += 1;
                }
            }
        }
    }
    let table = palette(&counts);
    let index: BTreeMap<[u8; 3], u8> = counts.keys().map(|c| (*c, nearest(&table, *c))).collect();
    let clear = table.len() as u8; // the see-through colour is the one after the last
    let mut pixels = Vec::with_capacity((x1 - x0) * (y1 - y0));
    let mut any_clear = false;
    let was_blank = shown.is_none();
    let canvas = shown.get_or_insert_with(|| vec![0u8; w * h * 3]);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = y * w + x;
            let c = [
                image.data[i * 3],
                image.data[i * 3 + 1],
                image.data[i * 3 + 2],
            ];
            let same = !was_blank && canvas[i * 3..i * 3 + 3] == c;
            let k = if same || nothing {
                None
            } else {
                let k = index[&c];
                // brought to its nearest colour it may be what is showing already
                (was_blank || canvas[i * 3..i * 3 + 3] != table[k as usize]).then_some(k)
            };
            match k {
                Some(k) => {
                    canvas[i * 3..i * 3 + 3].copy_from_slice(&table[k as usize]);
                    pixels.push(k);
                }
                None => {
                    any_clear = true;
                    pixels.push(clear);
                }
            }
        }
    }
    let mut bits = 1u8;
    while (1usize << bits) < table.len() + 1 {
        bits += 1;
    }
    let delay = ((ms + 5) / 10).clamp(2, 65535) as u16;
    out.extend([0x21, 0xf9, 0x04, 0x04 | any_clear as u8]);
    out.extend(delay.to_le_bytes());
    out.extend([clear, 0x00]);
    out.push(0x2c);
    for v in [x0, y0, x1 - x0, y1 - y0] {
        out.extend((v as u16).to_le_bytes());
    }
    out.push(0x80 | (bits - 1));
    for k in 0..1usize << bits {
        out.extend(table.get(k).copied().unwrap_or([0, 0, 0]));
    }
    let min = bits.max(2);
    out.push(min);
    for block in lzw(min, &pixels).chunks(255) {
        out.push(block.len() as u8);
        out.extend(block);
    }
    out.push(0x00);
}

/// GIF's LZW: codes of growing width, least significant bit first.
fn lzw(min: u8, data: &[u8]) -> Vec<u8> {
    const SLOTS: usize = 8192;
    let clear = 1u16 << min;
    let end = clear + 1;
    let mut out = Vec::new();
    let (mut acc, mut held) = (0u32, 0u32);
    let mut put = |out: &mut Vec<u8>, code: u16, size: u32| {
        acc |= (code as u32) << held;
        held += size;
        while held >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            held -= 8;
        }
    };
    // what follows a code: (code, byte) -> the code for the two together
    let mut keys = vec![0u32; SLOTS];
    let mut vals = vec![0u16; SLOTS];
    let slot = |key: u32| (key.wrapping_mul(2654435761) >> 19) as usize & (SLOTS - 1);
    let mut size = min as u32 + 1;
    let mut next = end + 1;
    put(&mut out, clear, size);
    let mut prefix: Option<u16> = None;
    for &k in data {
        let Some(p) = prefix else {
            prefix = Some(k as u16);
            continue;
        };
        let key = ((p as u32) << 8 | k as u32) + 1;
        let mut at = slot(key);
        while keys[at] != 0 && keys[at] != key {
            at = (at + 1) & (SLOTS - 1);
        }
        if keys[at] == key {
            prefix = Some(vals[at]);
            continue;
        }
        put(&mut out, p, size);
        if next < 4096 {
            keys[at] = key;
            vals[at] = next;
            if next == 1 << size {
                size += 1;
            }
            next += 1;
        } else {
            put(&mut out, clear, size);
            keys.iter_mut().for_each(|v| *v = 0);
            size = min as u32 + 1;
            next = end + 1;
        }
        prefix = Some(k as u16);
    }
    if let Some(p) = prefix {
        put(&mut out, p, size);
        // The reader adds a code for every one it reads, this last one too, and widens when its table says so:
        // the end has to be written as wide as it will be read.
        if next == 1 << size && size < 12 {
            size += 1;
        }
    }
    put(&mut out, end, size);
    if held > 0 {
        out.push(acc as u8);
    }
    out
}

fn unlzw(min: u8, data: &[u8], want: usize) -> Result<Vec<u8>, String> {
    let clear = 1u16 << min;
    let end = clear + 1;
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut out = Vec::with_capacity(want);
    let (mut acc, mut held, mut at) = (0u32, 0u32, 0usize);
    let mut size = min as u32 + 1;
    let mut next = end + 1;
    let mut prev: Option<u16> = None;
    let mut stack = Vec::new();
    loop {
        while held < size {
            let Some(b) = data.get(at) else {
                return Err("the picture's data ends early".into());
            };
            acc |= (*b as u32) << held;
            held += 8;
            at += 1;
        }
        let code = (acc & ((1 << size) - 1)) as u16;
        acc >>= size;
        held -= size;
        if code == clear {
            size = min as u32 + 1;
            next = end + 1;
            prev = None;
            continue;
        }
        if code == end {
            break;
        }
        let Some(p) = prev else {
            if code >= clear {
                return Err("a code before its time".into());
            }
            out.push(code as u8);
            prev = Some(code);
            continue;
        };
        if code > next {
            return Err("a code before its time".into());
        }
        // the string for `code`, or for the one before it and its own first byte again
        let mut c = if code < next { code } else { p };
        stack.clear();
        while c > end {
            stack.push(suffix[c as usize]);
            c = prefix[c as usize];
        }
        let first = c as u8;
        out.push(first);
        out.extend(stack.iter().rev());
        if code == next {
            out.push(first);
        }
        if next < 4096 {
            prefix[next as usize] = p;
            suffix[next as usize] = first;
            next += 1;
            if next == 1 << size && size < 12 {
                size += 1;
            }
        }
        prev = Some(code);
    }
    Ok(out)
}

/// Reads a GIF written by [`gif`] (that subset: no interlace, frames
/// left in place) and hands every frame as it would be seen to `each`,
/// with its number and its time in milliseconds. Returns the frame's
/// width and height and how many frames there were.
pub fn read_gif(
    bytes: &[u8],
    mut each: impl FnMut(usize, &Image, u32),
) -> Result<(usize, usize, usize), String> {
    let short = || "the file ends early".to_string();
    if bytes.len() < 13 || &bytes[..6] != b"GIF89a" {
        return Err("not a GIF89a".into());
    }
    let word = |at: usize| -> Result<usize, String> {
        Ok(u16::from_le_bytes([
            *bytes.get(at).ok_or_else(short)?,
            *bytes.get(at + 1).ok_or_else(short)?,
        ]) as usize)
    };
    let (w, h) = (word(6)?, word(8)?);
    let mut at = 13;
    let mut global: Vec<[u8; 3]> = Vec::new();
    if bytes[10] & 0x80 != 0 {
        let n = 1usize << ((bytes[10] & 7) + 1);
        let table = bytes.get(at..at + n * 3).ok_or_else(short)?;
        global = table.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        at += n * 3;
    }
    let mut canvas = Image::new(w, h, Rgb(0, 0, 0));
    let (mut frames, mut delay, mut clear) = (0usize, 0u32, None);
    loop {
        match *bytes.get(at).ok_or_else(short)? {
            0x3b => break,
            0x21 => {
                let label = *bytes.get(at + 1).ok_or_else(short)?;
                at += 2;
                if label == 0xf9 {
                    let b = bytes.get(at..at + 6).ok_or_else(short)?;
                    delay = u16::from_le_bytes([b[2], b[3]]) as u32 * 10;
                    clear = (b[1] & 1 != 0).then_some(b[4]);
                    if (b[1] >> 2) & 7 > 1 {
                        return Err("a frame that is not left in place".into());
                    }
                }
                loop {
                    let n = *bytes.get(at).ok_or_else(short)? as usize;
                    at += 1 + n;
                    if n == 0 {
                        break;
                    }
                }
            }
            0x2c => {
                let (x0, y0, fw, fh) = (word(at + 1)?, word(at + 3)?, word(at + 5)?, word(at + 7)?);
                let packed = *bytes.get(at + 9).ok_or_else(short)?;
                at += 10;
                if packed & 0x40 != 0 || x0 + fw > w || y0 + fh > h {
                    return Err("an interlaced frame, or one outside the picture".into());
                }
                let table: Vec<[u8; 3]> = if packed & 0x80 != 0 {
                    let n = 1usize << ((packed & 7) + 1);
                    let t = bytes.get(at..at + n * 3).ok_or_else(short)?;
                    at += n * 3;
                    t.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
                } else {
                    global.clone()
                };
                let min = *bytes.get(at).ok_or_else(short)?;
                at += 1;
                if !(2..=8).contains(&min) {
                    return Err("a code size that is not 2 to 8".into());
                }
                let mut data = Vec::new();
                loop {
                    let n = *bytes.get(at).ok_or_else(short)? as usize;
                    data.extend(bytes.get(at + 1..at + 1 + n).ok_or_else(short)?);
                    at += 1 + n;
                    if n == 0 {
                        break;
                    }
                }
                let pixels = unlzw(min, &data, fw * fh)?;
                if pixels.len() != fw * fh {
                    return Err("a frame with the wrong number of pixels".into());
                }
                for (i, k) in pixels.iter().enumerate() {
                    if Some(*k) == clear {
                        continue;
                    }
                    let c = table.get(*k as usize).ok_or("a colour off its table")?;
                    canvas.set(x0 + i % fw, y0 + i / fw, Rgb(c[0], c[1], c[2]));
                }
                each(frames, &canvas, delay);
                frames += 1;
                (delay, clear) = (0, None);
            }
            other => return Err(format!("an unknown block, {other:#x}")),
        }
    }
    Ok((w, h, frames))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(points: &[(f64, f64)], fill: Option<Rgb>) -> Shape {
        Shape {
            points: points.to_vec(),
            fill,
            stroke: None,
            width: 0.0,
            closed: true,
        }
    }

    fn book(frames: Vec<Frame>) -> Book {
        Book {
            width: 200,
            height: 200,
            view: [0.0, 0.0, 100.0, 100.0],
            background: Rgb(255, 255, 255),
            frames,
            repeat: true,
        }
    }

    /// How much of the image is ink, in pixels: black is 1, white 0.
    fn ink(image: &Image) -> f64 {
        image
            .data
            .chunks_exact(3)
            .map(|c| 1.0 - c[0] as f64 / 255.0)
            .sum()
    }

    #[test]
    fn a_filled_polygon_covers_its_area() {
        let black = Some(Rgb(0, 0, 0));
        // a triangle of 30 x 40 units, and a 96-gon of radius 25: two pixels to a unit
        let triangle = [(10.3, 12.7), (40.3, 12.7), (10.3, 52.7)];
        let round: Vec<(f64, f64)> = (0..96)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 96.0;
                (60.1 + 25.0 * a.cos(), 61.3 + 25.0 * a.sin())
            })
            .collect();
        for (points, area) in [
            (triangle.to_vec(), 600.0),
            (
                round,
                0.5 * 96.0 * 25.0 * 25.0 * (std::f64::consts::TAU / 96.0).sin(),
            ),
        ] {
            let b = book(vec![Frame {
                keep: vec![],
                draw: vec![shape(&points, black)],
                ms: 100,
            }]);
            let got = ink(&still(&b, 0).unwrap()) / 4.0;
            assert!(
                (got - area).abs() < 0.003 * area,
                "{got} units of ink for {area}"
            );
        }
    }

    #[test]
    fn the_view_is_centred_and_y_is_up() {
        // a view twice as wide as it is high, in a square frame: a band across the middle
        let mut b = book(vec![Frame {
            keep: vec![],
            draw: vec![shape(
                &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                Some(Rgb(0, 0, 0)),
            )],
            ms: 100,
        }]);
        b.view = [0.0, 0.0, 200.0, 100.0];
        let image = still(&b, 0).unwrap();
        // the square is at the left, at the bottom of the band: pixels 0..10 across, 140..150 down
        assert_eq!(image.pixel(5, 145), Rgb(0, 0, 0));
        assert_eq!(image.pixel(5, 135), Rgb(255, 255, 255));
        assert_eq!(image.pixel(15, 145), Rgb(255, 255, 255));
        assert!((ink(&image) - 100.0).abs() < 1.0);
    }

    #[test]
    fn an_outline_is_as_wide_as_it_says() {
        let line = Shape {
            points: vec![(20.0, 50.0), (80.0, 50.0)],
            fill: None,
            stroke: Some(Rgb(0, 0, 0)),
            width: 6.0,
            closed: false,
        };
        let b = book(vec![Frame {
            keep: vec![],
            draw: vec![line],
            ms: 100,
        }]);
        let image = still(&b, 0).unwrap();
        // 120 pixels long and 6 wide, with a round end of radius 3 at each end
        let want = 120.0 * 6.0 + std::f64::consts::PI * 9.0;
        let got = ink(&image);
        assert!((got - want).abs() < 0.03 * want, "{got} for {want}");
        assert_eq!(image.pixel(100, 100), Rgb(0, 0, 0));
        assert_eq!(image.pixel(100, 104), Rgb(255, 255, 255));
    }

    #[test]
    fn what_is_kept_stays_and_what_is_drawn_does_not() {
        let (red, blue) = (Rgb(200, 30, 30), Rgb(30, 30, 200));
        let left = shape(
            &[(10.0, 10.0), (40.0, 10.0), (40.0, 40.0), (10.0, 40.0)],
            Some(red),
        );
        let right = shape(
            &[(60.0, 10.0), (90.0, 10.0), (90.0, 40.0), (60.0, 40.0)],
            Some(blue),
        );
        let b = book(vec![
            Frame {
                keep: vec![left],
                draw: vec![right],
                ms: 100,
            },
            Frame::default(),
        ]);
        let mut seen = Vec::new();
        play(&b, |_, image, _| {
            seen.push((image.pixel(50, 150), image.pixel(150, 150)));
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, vec![(red, blue), (red, Rgb(255, 255, 255))]);
    }

    #[test]
    fn the_codes_read_back_as_the_bytes_that_were_written() {
        // runs, noise, and enough of both to fill the table of 4096 codes several times over
        let mut state = 12345u32;
        let mut noise = move |most: u32| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            ((state >> 16) % most) as u8
        };
        for (min, most, long) in [
            (2u8, 4u32, 3usize),
            (2, 3, 70_000),
            (5, 32, 50_000),
            (8, 256, 120_000),
            (8, 2, 90_000),
        ] {
            let data: Vec<u8> = (0..long).map(|_| noise(most)).collect();
            let packed = lzw(min, &data);
            assert_eq!(
                unlzw(min, &packed, long).unwrap(),
                data,
                "{min} {most} {long}"
            );
        }
        // Every length, so that some end exactly where the codes grow a bit wider: there the end's own code is
        // read a bit wider than the code before it, and has to be written so.
        for min in 2u8..=8 {
            let data: Vec<u8> = (0..700).map(|_| noise(1 << min.min(3))).collect();
            for long in 0..=data.len() {
                let packed = lzw(min, &data[..long]);
                assert_eq!(
                    unlzw(min, &packed, long).unwrap(),
                    data[..long],
                    "{min} {long}"
                );
            }
        }
        assert_eq!(unlzw(4, &lzw(4, &[]), 0).unwrap(), Vec::<u8>::new());
        assert!(
            lzw(8, &vec![7u8; 100_000]).len() < 1000,
            "a run packs small"
        );
    }

    #[test]
    fn a_gif_reads_back_as_the_frames_that_were_rendered() {
        let colours = [Rgb(200, 30, 30), Rgb(30, 160, 60), Rgb(30, 30, 200)];
        let mut frames = Vec::new();
        for (k, c) in colours.iter().enumerate() {
            let x = 10.0 + 25.0 * k as f64;
            frames.push(Frame {
                keep: vec![shape(
                    &[(x, 10.0), (x + 20.0, 10.0), (x + 20.0, 30.0)],
                    Some(*c),
                )],
                draw: vec![Shape {
                    points: vec![(x, 60.0), (x + 20.0, 80.0)],
                    fill: None,
                    stroke: Some(*c),
                    width: 3.0,
                    closed: false,
                }],
                ms: 70 + 30 * k as u32,
            });
        }
        frames.push(Frame {
            keep: vec![],
            draw: vec![],
            ms: 500,
        });
        frames.push(Frame {
            keep: vec![],
            draw: vec![],
            ms: 250,
        }); // the same picture again: a frame all the same
        let b = book(frames);
        let mut rendered = Vec::new();
        play(&b, |_, image, ms| {
            rendered.push((image.clone(), ms));
            Ok(())
        })
        .unwrap();
        let bytes = gif(&b).unwrap();
        let mut read = Vec::new();
        let (w, h, n) = read_gif(&bytes, |_, image, ms| read.push((image.clone(), ms))).unwrap();
        assert_eq!((w, h, n), (200, 200, 5));
        assert_eq!(read.len(), rendered.len());
        for (k, (a, b)) in rendered.iter().zip(&read).enumerate() {
            assert!(a.0 == b.0, "frame {k} is not what was rendered");
            assert_eq!(
                b.1,
                (a.1 + 5) / 10 * 10,
                "frame {k}'s time, to a hundredth of a second"
            );
        }
        // only what changed is written: five frames of 200 x 200 in a few kilobytes
        println!("{} bytes for 5 frames", bytes.len());
        assert!(bytes.len() < 6000);
        // and the same book gives the same bytes
        assert_eq!(gif(&b).unwrap(), bytes);
    }

    #[test]
    fn more_colours_than_a_frame_holds_are_brought_to_the_nearest() {
        // 40 x 40 squares, every one its own colour: 1600 colours in one frame
        let mut draw = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let (x, y) = (i as f64 * 2.5, j as f64 * 2.5);
                draw.push(shape(
                    &[(x, y), (x + 2.5, y), (x + 2.5, y + 2.5), (x, y + 2.5)],
                    Some(Rgb(60 + 4 * i as u8, 60 + 4 * j as u8, 128)),
                ));
            }
        }
        let b = book(vec![Frame {
            keep: vec![],
            draw,
            ms: 100,
        }]);
        let want = still(&b, 0).unwrap();
        let mut worst = 0i32;
        read_gif(&gif(&b).unwrap(), |_, image, _| {
            for (a, b) in image.data.iter().zip(&want.data) {
                worst = worst.max((*a as i32 - *b as i32).abs());
            }
        })
        .unwrap();
        println!("the worst channel is {worst} out");
        assert!(worst > 0 && worst <= 10);
    }

    #[test]
    fn a_book_that_cannot_be_drawn_says_why() {
        let mut b = book(vec![Frame::default()]);
        b.view = [0.0, 0.0, 0.0, 10.0];
        assert!(gif(&b).unwrap_err().contains("view"));
        let b = book(vec![]);
        assert!(gif(&b).unwrap_err().contains("frames"));
        assert!(still(&book(vec![Frame::default()]), 3).is_err());
        assert!(read_gif(b"GIF87a", |_, _, _| {}).is_err());
    }
}
