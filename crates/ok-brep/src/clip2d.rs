//! Clipping oriented 2D loops to a rectangle and closing the pieces
//! along its boundary.
//!
//! Loops are counter-clockwise around material (holes clockwise), and
//! open chains have material on their left. Inside the rectangle the
//! result is the same region, bounded by the pieces of the loops and
//! chains that fall inside plus the parts of the rectangle's boundary
//! between them: from where a piece exits, the boundary is followed
//! counter-clockwise to the next entry. Vertices lying exactly on the
//! rectangle's edges are kept in the pieces on both sides, so two
//! neighbouring rectangles agree on the points they share.

/// A closed or open polyline, `[x, y]` per point.
pub type Contour = Vec<[f64; 2]>;

/// A rectangle, `[x0, y0, x1, y1]`.
pub type Rect = [f64; 4];

/// The region within `rect`, as closed contours (counter-clockwise
/// material, clockwise holes). `corner_inside` says whether the
/// rectangle's lower-left corner lies in material, needed when no piece
/// touches the boundary; `None` when the pieces are inconsistent (their
/// ends along the boundary do not alternate between exits and entries).
pub fn clip_and_close(
    loops: &[Contour],
    chains: &[Contour],
    rect: Rect,
    tol: f64,
    corner_inside: impl Fn() -> Option<bool>,
) -> Option<Vec<Contour>> {
    let mut closed: Vec<Contour> = Vec::new();
    let mut open: Vec<Contour> = Vec::new();
    for l in loops {
        clip_loop(l, rect, &mut closed, &mut open);
    }
    for c in chains {
        clip_polyline(c, rect, &mut open);
    }
    // A piece that is one point (a loop touching the boundary from
    // outside) bounds nothing.
    open.retain(|p| p.len() >= 2 && dist(p[0], p[p.len() - 1]) > tol || p.len() > 2);
    if open.is_empty() {
        let mut out = closed;
        if corner_inside()? {
            out.push(vec![
                [rect[0], rect[1]],
                [rect[2], rect[1]],
                [rect[2], rect[3]],
                [rect[0], rect[3]],
            ]);
        }
        return Some(out);
    }
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    let perimeter = 2.0 * (w + h);
    // Position along the boundary, counter-clockwise from the lower left.
    let along = |p: [f64; 2]| -> f64 {
        let d = [
            (p[1] - rect[1]).abs(),
            (p[0] - rect[2]).abs(),
            (p[1] - rect[3]).abs(),
            (p[0] - rect[0]).abs(),
        ];
        let side = (0..4)
            .min_by(|&i, &j| d[i].partial_cmp(&d[j]).unwrap())
            .unwrap();
        match side {
            0 => p[0] - rect[0],
            1 => w + (p[1] - rect[1]),
            2 => w + h + (rect[2] - p[0]),
            _ => 2.0 * w + h + (rect[3] - p[1]),
        }
    };
    let corner_at = |s: f64| -> [f64; 2] {
        if s < w {
            [rect[0] + s, rect[1]]
        } else if s < w + h {
            [rect[2], rect[1] + (s - w)]
        } else if s < 2.0 * w + h {
            [rect[2] - (s - w - h), rect[3]]
        } else {
            [rect[0], rect[3] - (s - 2.0 * w - h)]
        }
    };
    let corner_positions = [0.0, w, w + h, 2.0 * w + h];
    // Every endpoint on the boundary, sorted; entries and exits must
    // alternate for the material bands along the boundary to make sense.
    // An exit and an entry at one point (a loop crossing the boundary
    // there and coming back through the same point) are ordered exit
    // first so the walk passes straight through.
    let mut events: Vec<(f64, usize, bool)> = Vec::new(); // (position, piece, is_start)
    for (i, piece) in open.iter().enumerate() {
        events.push((along(piece[0]), i, true));
        events.push((along(*piece.last().unwrap()), i, false));
    }
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.2.cmp(&b.2)));
    for pair in events.windows(2) {
        if pair[0].2 == pair[1].2 {
            return None;
        }
    }
    if events.len() >= 2 && events[0].2 == events[events.len() - 1].2 {
        return None;
    }
    let mut used = vec![false; open.len()];
    let mut out: Vec<Contour> = closed;
    for first in 0..open.len() {
        if used[first] {
            continue;
        }
        let mut poly: Contour = Vec::new();
        let mut current = first;
        loop {
            used[current] = true;
            poly.extend(open[current].iter().copied());
            let exit_index = events.iter().position(|e| e.1 == current && !e.2).unwrap();
            // The next event counter-clockwise from the exit is an entry.
            let next = events[(exit_index + 1) % events.len()];
            if !next.2 {
                return None;
            }
            let (exit, entry) = (events[exit_index].0, next.0);
            // Corners passed on the way.
            let passed: Vec<f64> = if entry >= exit {
                corner_positions
                    .iter()
                    .copied()
                    .filter(|&c| c > exit && c < entry)
                    .collect()
            } else {
                corner_positions
                    .iter()
                    .copied()
                    .filter(|&c| c > exit)
                    .chain(corner_positions.iter().copied().filter(|&c| c < entry))
                    .collect()
            };
            for c in passed {
                poly.push(corner_at(c.min(perimeter - 1e-12)));
            }
            current = next.1;
            if current == first {
                break;
            }
            if used[current] {
                return None;
            }
        }
        poly.dedup_by(|a, b| dist(*a, *b) <= tol);
        while poly.len() > 1 && dist(poly[0], poly[poly.len() - 1]) <= tol {
            poly.pop();
        }
        if poly.len() >= 3 {
            out.push(poly);
        }
    }
    Some(out)
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Inside or on the rectangle.
fn inside_rect(p: [f64; 2], rect: Rect) -> bool {
    p[0] >= rect[0] && p[0] <= rect[2] && p[1] >= rect[1] && p[1] <= rect[3]
}

/// The parameter interval of segment `a b` inside `rect` (Liang–Barsky).
fn clip_segment(a: [f64; 2], b: [f64; 2], rect: Rect) -> Option<(f64, f64)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-d[0], a[0] - rect[0]),
        (d[0], rect[2] - a[0]),
        (-d[1], a[1] - rect[1]),
        (d[1], rect[3] - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Cuts an open polyline into the pieces inside `rect`; a piece's ends
/// are the polyline's own ends or points on the boundary (a vertex on
/// the boundary is its own crossing point).
pub fn clip_polyline(points: &[[f64; 2]], rect: Rect, out: &mut Vec<Contour>) {
    let mut current: Option<Contour> = None;
    if points.len() < 2 {
        return;
    }
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (a_in, b_in) = (inside_rect(a, rect), inside_rect(b, rect));
        match (a_in, b_in) {
            (true, true) => current.get_or_insert_with(|| vec![a]).push(b),
            (true, false) => {
                let mut piece = current.take().unwrap_or_else(|| vec![a]);
                if let Some((_, t1)) = clip_segment(a, b, rect) {
                    let exit = lerp(a, b, t1);
                    if dist(exit, *piece.last().unwrap()) > 0.0 {
                        piece.push(exit);
                    }
                }
                out.push(piece);
            }
            (false, true) => {
                if let Some(piece) = current.take() {
                    out.push(piece);
                }
                let t0 = clip_segment(a, b, rect).map_or(0.0, |(t0, _)| t0);
                let entry = lerp(a, b, t0);
                current = Some(if dist(entry, b) > 0.0 {
                    vec![entry, b]
                } else {
                    vec![b]
                });
            }
            (false, false) => {
                if let Some(piece) = current.take() {
                    out.push(piece);
                }
                if let Some((t0, t1)) = clip_segment(a, b, rect) {
                    if t1 > t0 {
                        out.push(vec![lerp(a, b, t0), lerp(a, b, t1)]);
                    }
                }
            }
        }
    }
    if let Some(piece) = current.take() {
        out.push(piece);
    }
}

/// Cuts a closed loop to `rect`: kept whole when it lies inside,
/// otherwise in open pieces (none when it stays clear of the rectangle,
/// enclosing it or not).
pub fn clip_loop(l: &[[f64; 2]], rect: Rect, closed: &mut Vec<Contour>, open: &mut Vec<Contour>) {
    let strictly_inside =
        |p: [f64; 2]| p[0] > rect[0] && p[0] < rect[2] && p[1] > rect[1] && p[1] < rect[3];
    if l.iter().all(|p| inside_rect(*p, rect)) {
        closed.push(l.to_vec());
        return;
    }
    // Start at a vertex outside, or failing that one on the boundary.
    let start = l
        .iter()
        .position(|p| !inside_rect(*p, rect))
        .or_else(|| l.iter().position(|p| !strictly_inside(*p)));
    let Some(start) = start else {
        return;
    };
    let mut rotated: Contour = Vec::with_capacity(l.len() + 1);
    rotated.extend(l[start..].iter().copied());
    rotated.extend(l[..start].iter().copied());
    rotated.push(l[start]);
    clip_polyline(&rotated, rect, open);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(c: &Contour) -> f64 {
        let n = c.len();
        (0..n)
            .map(|i| {
                let (a, b) = (c[i], c[(i + 1) % n]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            / 2.0
    }

    #[test]
    fn a_square_cut_by_a_strip_keeps_its_area_in_pieces() {
        let square: Contour = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let strip = [3.0, -1.0, 6.0, 11.0];
        let pieces = clip_and_close(std::slice::from_ref(&square), &[], strip, 1e-9, || {
            Some(false)
        })
        .unwrap();
        assert_eq!(pieces.len(), 1);
        assert!((area(&pieces[0]) - 30.0).abs() < 1e-9);
        // A hole inside the strip stays a hole; one crossing the strip's
        // edge is cut with it.
        let hole: Contour = vec![[4.0, 4.0], [4.0, 6.0], [5.0, 6.0], [5.0, 4.0]];
        let pieces =
            clip_and_close(&[square.clone(), hole], &[], strip, 1e-9, || Some(false)).unwrap();
        let total: f64 = pieces.iter().map(area).sum();
        assert!((total - 28.0).abs() < 1e-9, "{total}");
        let hole: Contour = vec![[5.0, 4.0], [5.0, 6.0], [8.0, 6.0], [8.0, 4.0]];
        let pieces = clip_and_close(&[square, hole], &[], strip, 1e-9, || Some(false)).unwrap();
        let total: f64 = pieces.iter().map(area).sum();
        assert!((total - 28.0).abs() < 1e-9, "{total}");
    }

    #[test]
    fn vertices_on_the_strip_edges_are_kept_on_both_sides() {
        // A loop with vertices exactly on x = 3 and x = 6.
        let l: Contour = vec![
            [0.0, 0.0],
            [3.0, 0.0],
            [6.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [6.0, 10.0],
            [3.0, 10.0],
            [0.0, 10.0],
        ];
        let left = clip_and_close(
            std::slice::from_ref(&l),
            &[],
            [0.0, -1.0, 3.0, 11.0],
            1e-9,
            || Some(false),
        )
        .unwrap();
        let mid = clip_and_close(
            std::slice::from_ref(&l),
            &[],
            [3.0, -1.0, 6.0, 11.0],
            1e-9,
            || Some(false),
        )
        .unwrap();
        let right =
            clip_and_close(&[l], &[], [6.0, -1.0, 10.0, 11.0], 1e-9, || Some(false)).unwrap();
        for (piece, expected) in [(&left, 30.0), (&mid, 30.0), (&right, 40.0)] {
            assert_eq!(piece.len(), 1);
            assert!((area(&piece[0]) - expected).abs() < 1e-9);
        }
        assert!(mid[0].contains(&[3.0, 0.0]) && mid[0].contains(&[6.0, 10.0]));
        assert!(left[0].contains(&[3.0, 0.0]) && right[0].contains(&[6.0, 10.0]));
    }
}
