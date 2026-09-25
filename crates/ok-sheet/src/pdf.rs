//! A small vector PDF writer: one page in millimetres with lines, filled
//! polygons, circles and Helvetica text. Enough for a drawing sheet, with
//! no dependencies and no embedded fonts (the standard 14 are assumed).

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

    /// The finished file.
    pub fn finish(self) -> Vec<u8> {
        let content = format!("{} 0 0 {} 0 0 cm\n{}", num(PT), num(PT), self.ops);
        let objects: Vec<String> = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>",
                num(self.width * PT),
                num(self.height * PT)
            ),
            format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .into(),
        ];
        let mut out = String::from("%PDF-1.4\n%\u{e2}\u{e3}\u{cf}\u{d3}\n");
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, body));
        }
        let xref = out.len();
        out.push_str(&format!(
            "xref\n0 {}\n0000000000 65535 f \n",
            objects.len() + 1
        ));
        for o in &offsets {
            out.push_str(&format!("{:010} 00000 n \n", o));
        }
        out.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref
        ));
        out.into_bytes()
    }
}
