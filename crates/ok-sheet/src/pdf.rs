//! A small vector PDF writer: one page in millimetres with lines, filled
//! polygons, circles and Helvetica text. Enough for a drawing sheet, with
//! no embedded fonts (the standard 14 are assumed). The content stream
//! is deflated, which makes a sheet a quarter of its size; [`inflated`]
//! gives it back as text for anything that greps the file.

/// Where text sits relative to its anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Left,
    Middle,
    Right,
}

/// One page, built up in millimetres with y up from the bottom-left.
pub struct Page {
    width: f64,
    height: f64,
    ops: String,
}

/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;

fn num(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// Approximate advance of Helvetica text, in units of the font size.
pub fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | 't' | 'f' | 'I' | '.' | ',' | ':' | ';' | '\'' | '(' | ')' | '·' => {
                0.28
            }
            'm' | 'w' | 'M' | 'W' => 0.83,
            ' ' => 0.28,
            c if c.is_ascii_digit() => 0.556,
            c if c.is_ascii_uppercase() => 0.68,
            _ => 0.55,
        })
        .sum::<f64>()
        * size
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // WinAnsi has Ø, ×, · and the accented Latin letters; anything
            // else becomes a question mark rather than garbage.
            c if (c as u32) < 128 => out.push(c),
            'Ø' => out.push_str("\\330"),
            '×' => out.push_str("\\327"),
            '·' => out.push_str("\\267"),
            '°' => out.push_str("\\260"),
            c if (c as u32) < 256 => out.push_str(&format!("\\{:03o}", c as u32)),
            _ => out.push('?'),
        }
    }
    out
}

impl Page {
    pub fn new(width: f64, height: f64) -> Page {
        Page {
            width,
            height,
            ops: String::new(),
        }
    }

    /// The grey every later stroke and fill uses (0 black, 1 white)
    /// until the next call; every drawing call saves and restores the
    /// state around itself, so this is the state they inherit.
    pub fn gray(&mut self, g: f64) {
        self.ops.push_str(&format!("{} G {} g\n", num(g), num(g)));
    }

    /// Straight segments as one path; `dash` is (on, off) in mm.
    pub fn lines(&mut self, segments: &[[(f64, f64); 2]], width: f64, dash: Option<(f64, f64)>) {
        if segments.is_empty() {
            return;
        }
        self.ops.push_str(&format!("q {} w 1 J ", num(width)));
        if let Some((on, off)) = dash {
            self.ops
                .push_str(&format!("[{} {}] 0 d ", num(on), num(off)));
        }
        for [a, b] in segments {
            self.ops.push_str(&format!(
                "{} {} m {} {} l ",
                num(a.0),
                num(a.1),
                num(b.0),
                num(b.1)
            ));
        }
        self.ops.push_str("S Q\n");
    }

    /// A closed polygon, filled black.
    pub fn polygon(&mut self, points: &[(f64, f64)]) {
        if points.len() < 3 {
            return;
        }
        self.ops.push_str("q ");
        for (i, p) in points.iter().enumerate() {
            self.ops.push_str(&format!(
                "{} {} {} ",
                num(p.0),
                num(p.1),
                if i == 0 { "m" } else { "l" }
            ));
        }
        self.ops.push_str("h f Q\n");
    }

    /// A rectangle outline, or filled white when `fill` (to blank what is under it).
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, width: f64, fill: bool) {
        self.ops.push_str(&format!(
            "q {} w {} {} {} {} re {} Q\n",
            num(width),
            num(x),
            num(y),
            num(w),
            num(h),
            if fill { "1 g B 0 g" } else { "S" }
        ));
    }

    /// A circle outline (four Bézier quarters), filled white when `fill`.
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, width: f64, fill: bool) {
        let k = 0.5523 * r;
        self.ops.push_str(&format!(
            "q {} w {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h {} Q\n",
            num(width),
            num(cx + r), num(cy),
            num(cx + r), num(cy + k), num(cx + k), num(cy + r), num(cx), num(cy + r),
            num(cx - k), num(cy + r), num(cx - r), num(cy + k), num(cx - r), num(cy),
            num(cx - r), num(cy - k), num(cx - k), num(cy - r), num(cx), num(cy - r),
            num(cx + k), num(cy - r), num(cx + r), num(cy - k), num(cx + r), num(cy),
            if fill { "1 g B 0 g" } else { "S" }
        ));
    }

    /// A filled dot.
    pub fn dot(&mut self, cx: f64, cy: f64, r: f64) {
        let k = 0.5523 * r;
        self.ops.push_str(&format!(
            "q {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h f Q\n",
            num(cx + r), num(cy),
            num(cx + r), num(cy + k), num(cx + k), num(cy + r), num(cx), num(cy + r),
            num(cx - k), num(cy + r), num(cx - r), num(cy + k), num(cx - r), num(cy),
            num(cx - r), num(cy - k), num(cx - k), num(cy - r), num(cx), num(cy - r),
            num(cx + k), num(cy - r), num(cx + r), num(cy - k), num(cx + r), num(cy),
        ));
    }

    /// Text at `(x, y)` (the baseline's middle height), `size` mm high,
    /// turned `angle` degrees counter-clockwise about the anchor.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        size: f64,
        text: &str,
        anchor: Anchor,
        angle: f64,
        bold: bool,
    ) {
        let w = text_width(text, size);
        let dx = match anchor {
            Anchor::Left => 0.0,
            Anchor::Middle => -w / 2.0,
            Anchor::Right => -w,
        };
        // Centre the cap height on y.
        let dy = -0.36 * size;
        let (c, s) = (angle.to_radians().cos(), angle.to_radians().sin());
        self.ops.push_str(&format!(
            "q {} {} {} {} {} {} cm BT /{} {} Tf {} {} Td ({}) Tj ET Q\n",
            num(c),
            num(s),
            num(-s),
            num(c),
            num(x),
            num(y),
            if bold { "F2" } else { "F1" },
            num(size),
            num(dx),
            num(dy),
            escape(text)
        ));
    }

    /// The finished file: the content stream deflated.
    pub fn finish(self) -> Vec<u8> {
        let content = format!("{} 0 0 {} 0 0 cm\n{}", num(PT), num(PT), self.ops);
        let deflated = miniz_oxide::deflate::compress_to_vec_zlib(content.as_bytes(), 6);
        let mut stream = format!(
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            deflated.len()
        )
        .into_bytes();
        stream.extend_from_slice(&deflated);
        stream.extend_from_slice(b"\nendstream");
        let objects: Vec<Vec<u8>> = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>",
                num(self.width * PT),
                num(self.height * PT)
            )
            .into_bytes(),
            stream,
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        ];
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for o in &offsets {
            out.extend_from_slice(format!("{:010} 00000 n \n", o).as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                objects.len() + 1,
                xref
            )
            .as_bytes(),
        );
        out
    }
}

/// Where `needle` first occurs in `hay` at or after `from`.
fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The file as text with every deflated stream inflated in place, for
/// tests and tools that grep a sheet for what it draws. Bytes that are
/// not UTF-8 come out as replacement characters, so offsets in the
/// result mean nothing; a stream that does not inflate is left as it is.
pub fn inflated(pdf: &[u8]) -> String {
    let mut out = Vec::with_capacity(pdf.len() * 4);
    let mut at = 0;
    while let Some(s) = find(pdf, b">>\nstream\n", at) {
        let start = s + b">>\nstream\n".len();
        // The stream's dictionary runs back to the last "<<" before it.
        let dict_at = pdf[at..s]
            .windows(2)
            .rposition(|w| w == b"<<")
            .map_or(at, |p| p + at);
        let dict = String::from_utf8_lossy(&pdf[dict_at..s]).into_owned();
        let len: usize = dict
            .split("/Length ")
            .nth(1)
            .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        let end = (start + len).min(pdf.len());
        out.extend_from_slice(&pdf[at..start]);
        let data = &pdf[start..end];
        match dict.contains("/FlateDecode") {
            true => match miniz_oxide::inflate::decompress_to_vec_zlib(data) {
                Ok(raw) => out.extend_from_slice(&raw),
                Err(_) => out.extend_from_slice(data),
            },
            false => out.extend_from_slice(data),
        }
        at = end;
    }
    out.extend_from_slice(&pdf[at..]);
    String::from_utf8_lossy(&out).into_owned()
}
