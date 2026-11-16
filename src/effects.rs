//! The big animations: a window sucked into the task bar like a genie into a lamp, folded away
//! like an old television, or shattered into pieces of glass. This file is only the geometry (it
//! says where each point of the window's picture goes at time `p`); `render.rs` draws it with the
//! mesh program. Everything is a pure function, so it is tested without a screen.

use std::f64::consts::PI;

/// How a minimized window goes to the task bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimizeEffect {
    /// shrinks and fades towards its button
    Shrink,
    /// poured into its button like a genie into a lamp (and out again when restored)
    Lamp,
}

/// How a closed window goes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseEffect {
    Fade,
    /// collapses to a line and then a dot, like a television switched off
    Fold,
    /// breaks into pieces of glass that fall
    Shatter,
}

impl MinimizeEffect {
    pub fn parse(s: &str) -> MinimizeEffect {
        match s {
            "lamp" | "genie" => MinimizeEffect::Lamp,
            _ => MinimizeEffect::Shrink,
        }
    }

    pub fn seconds(self) -> f32 {
        match self {
            MinimizeEffect::Shrink => 0.22,
            MinimizeEffect::Lamp => 0.5,
        }
    }
}

impl CloseEffect {
    pub fn parse(s: &str) -> CloseEffect {
        match s {
            "shatter" | "glass" => CloseEffect::Shatter,
            "fold" | "tv" => CloseEffect::Fold,
            _ => CloseEffect::Fade,
        }
    }

    pub fn seconds(self) -> f32 {
        match self {
            CloseEffect::Fade => 0.18,
            CloseEffect::Fold => 0.4,
            CloseEffect::Shatter => 0.8,
        }
    }
}

/// A rectangle: left, top, width, height.
pub type Rect = (f64, f64, f64, f64);

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// The grid of `n` x `n` points (row by row, top to bottom) of `rect` as it is poured into the
/// point `target`, `p` (0 .. 1) of the way. The bottom of the window leaves first and the top
/// follows, so the window narrows into a funnel that bends towards the target. Returns the points
/// and how opaque it still is.
pub fn lamp(p: f64, rect: Rect, target: (f64, f64), n: usize) -> (Vec<(f32, f32)>, f32) {
    let (x, y, w, h) = rect;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let p = p.clamp(0.0, 1.0);
    // the pull speeds up, and the whole thing is a bit slower at first
    let q = p * p * (3.0 - 2.0 * p);
    const LAG: f64 = 0.9; // how much later the top starts than the bottom
    let mut pts = Vec::with_capacity(n * n);
    for i in 0..n {
        let v = i as f64 / (n - 1) as f64; // 0 top .. 1 bottom
        // the bottom row starts at once, the top row when the rest has gone most of the way
        let s = ((q * (1.0 + LAG) - (1.0 - v) * LAG) / 1.0).clamp(0.0, 1.0);
        let e = s * s * (3.0 - 2.0 * s);
        let row_y = lerp(y + h * v, target.1, e);
        // the sideways bend is slow at first and fast late, like a genie's neck
        let bend = smooth(s * 1.15);
        let row_cx = lerp(cx, target.0, bend);
        let row_w = lerp(w, 6.0, smooth(s * 1.05));
        for j in 0..n {
            let u = j as f64 / (n - 1) as f64 - 0.5;
            pts.push(((row_cx + u * row_w) as f32, row_y as f32));
        }
    }
    let alpha = (1.0 - smooth((p - 0.8) / 0.2)) as f32;
    let _ = cy;
    (pts, alpha)
}

/// The same grid for a window switched off like a television: it flattens into a line (the first
/// 60 %), the line shrinks to a dot, and the dot is gone.
pub fn fold(p: f64, rect: Rect, n: usize) -> (Vec<(f32, f32)>, f32) {
    let (x, y, w, h) = rect;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let p = p.clamp(0.0, 1.0);
    let squash = 1.0 - smooth(p / 0.6) * 0.99; // height factor
    let narrow = 1.0 - smooth((p - 0.55) / 0.45) * 0.97; // width factor
    let mut pts = Vec::with_capacity(n * n);
    for i in 0..n {
        let v = i as f64 / (n - 1) as f64 - 0.5;
        for j in 0..n {
            let u = j as f64 / (n - 1) as f64 - 0.5;
            pts.push(((cx + u * w * narrow) as f32, (cy + v * h * squash) as f32));
        }
    }
    let alpha = (1.0 - smooth((p - 0.9) / 0.1)) as f32;
    (pts, alpha)
}

/// One torn-off piece of a window: its four corners (top left, top right, bottom left, bottom
/// right), the part of the picture it carries (left, top, right, bottom, each 0 .. 1) and how
/// opaque it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub corners: [(f32, f32); 4],
    pub uv: (f32, f32, f32, f32),
    pub alpha: f32,
}

/// A number 0 .. 1 from a tile's place: the same every frame (no random state to keep).
fn noise(i: usize, j: usize, k: u32) -> f64 {
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (k as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h & 0xFFFF) as f64 / 65535.0
}

/// A window broken into `cols` x `rows` pieces of glass `p` (0 .. 1) of the way through its end.
/// The pieces nearest the middle go first and fly outwards, tumble and fall, shrinking and fading.
pub fn shatter(p: f64, rect: Rect, cols: usize, rows: usize) -> Vec<Piece> {
    let (x, y, w, h) = rect;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let p = p.clamp(0.0, 1.0);
    let (tw, th) = (w / cols as f64, h / rows as f64);
    let mut out = Vec::with_capacity(cols * rows);
    for i in 0..rows {
        for j in 0..cols {
            let (px, py) = (x + (j as f64 + 0.5) * tw, y + (i as f64 + 0.5) * th);
            let (dx, dy) = ((px - cx) / (w / 2.0), (py - cy) / (h / 2.0));
            let dist = (dx * dx + dy * dy).sqrt() / 2f64.sqrt();
            // the pieces by the middle start first
            let delay = 0.25 * dist + 0.08 * noise(i, j, 1);
            let t = ((p - delay) / (1.0 - delay)).clamp(0.0, 1.0);
            let alpha = if t <= 0.0 { 1.0 } else { 1.0 - smooth((t - 0.45) / 0.55) };
            let spin = (noise(i, j, 2) - 0.5) * 2.0 * 3.0 * PI * t;
            let speed = 0.35 + 0.55 * noise(i, j, 3);
            // outwards from the middle, a little sideways, then down
            let ox = (dx + (noise(i, j, 4) - 0.5) * 0.6) * w * 0.55 * speed * t;
            let oy = (dy - 0.25 + (noise(i, j, 5) - 0.5) * 0.4) * h * 0.35 * speed * t + h * 1.1 * t * t;
            let scale = 1.0 - 0.45 * t;
            let (c, s) = (spin.cos(), spin.sin());
            let corner = |sx: f64, sy: f64| -> (f32, f32) {
                let (lx, ly) = (sx * tw / 2.0 * scale, sy * th / 2.0 * scale);
                ((px + ox + lx * c - ly * s) as f32, (py + oy + lx * s + ly * c) as f32)
            };
            out.push(Piece {
                corners: [corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0)],
                uv: (j as f32 / cols as f32, i as f32 / rows as f32, (j + 1) as f32 / cols as f32, (i + 1) as f32 / rows as f32),
                alpha: alpha as f32,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = (100.0, 80.0, 400.0, 300.0);

    #[test]
    fn effect_names_are_read_and_unknown_ones_are_plain() {
        assert_eq!(MinimizeEffect::parse("lamp"), MinimizeEffect::Lamp);
        assert_eq!(MinimizeEffect::parse("genie"), MinimizeEffect::Lamp);
        assert_eq!(MinimizeEffect::parse("whatever"), MinimizeEffect::Shrink);
        assert_eq!(CloseEffect::parse("shatter"), CloseEffect::Shatter);
        assert_eq!(CloseEffect::parse("fold"), CloseEffect::Fold);
        assert_eq!(CloseEffect::parse(""), CloseEffect::Fade);
    }

    #[test]
    fn the_lamp_starts_as_the_window_and_ends_in_the_target() {
        let n = 9;
        let (start, a0) = lamp(0.0, R, (300.0, 600.0), n);
        assert_eq!(a0, 1.0);
        assert!((start[0].0 - 100.0).abs() < 1e-3 && (start[0].1 - 80.0).abs() < 1e-3, "top left corner");
        assert!((start[n * n - 1].0 - 500.0).abs() < 1e-3 && (start[n * n - 1].1 - 380.0).abs() < 1e-3, "bottom right corner");
        let (end, a1) = lamp(1.0, R, (300.0, 600.0), n);
        assert_eq!(a1, 0.0);
        assert!(end.iter().all(|&(x, y)| (x - 300.0).abs() < 4.0 && (y - 600.0).abs() < 0.01), "everything is at the button");
    }

    #[test]
    fn the_lamp_pours_bottom_first_and_never_folds_over() {
        let n = 9;
        for k in 0..=20 {
            let (pts, _) = lamp(k as f64 / 20.0, R, (300.0, 600.0), n);
            for j in 0..n {
                for i in 1..n {
                    assert!(pts[i * n + j].1 + 1e-3 >= pts[(i - 1) * n + j].1, "rows keep their order at p={}", k as f64 / 20.0);
                }
            }
            for i in 0..n {
                for j in 1..n {
                    assert!(pts[i * n + j].0 >= pts[i * n + j - 1].0 - 1e-3, "columns keep their order");
                }
            }
        }
        // halfway the bottom of the window is much closer to the button than its top
        let (mid, _) = lamp(0.5, R, (300.0, 600.0), n);
        let (top, bottom) = (mid[4].1, mid[(n - 1) * n + 4].1);
        assert!(600.0 - bottom < 600.0 - top - 60.0, "the bottom leads: top {top}, bottom {bottom}");
        // and the funnel is narrower at the bottom than at the top
        let width = |i: usize| mid[i * n + n - 1].0 - mid[i * n].0;
        assert!(width(n - 1) < width(0), "narrower at the bottom");
    }

    #[test]
    fn the_fold_goes_to_a_line_then_a_dot() {
        let n = 5;
        let (start, _) = fold(0.0, R, n);
        assert!((start[0].0 - 100.0).abs() < 1e-3 && (start[0].1 - 80.0).abs() < 1e-3);
        let (line, a) = fold(0.58, R, n);
        let height = line[n * n - 1].1 - line[0].1;
        let width = line[n - 1].0 - line[0].0;
        assert!(height < 10.0 && width > 300.0 && a == 1.0, "a thin line: {height} x {width}");
        let (dot, a) = fold(1.0, R, n);
        assert!(dot[n - 1].0 - dot[0].0 < 15.0 && a == 0.0);
    }

    #[test]
    fn the_glass_is_whole_at_first_and_gone_at_last() {
        let (cols, rows) = (8, 6);
        let start = shatter(0.0, R, cols, rows);
        assert_eq!(start.len(), cols * rows);
        assert!(start.iter().all(|p| p.alpha == 1.0));
        // the pieces tile the window exactly: the first one's corner is the window's corner and
        // neighbours share edges
        assert!((start[0].corners[0].0 - 100.0).abs() < 1e-3 && (start[0].corners[0].1 - 80.0).abs() < 1e-3);
        assert!((start[0].corners[1].0 - start[1].corners[0].0).abs() < 1e-3);
        assert_eq!(start[0].uv, (0.0, 0.0, 1.0 / cols as f32, 1.0 / rows as f32));
        let last = start.last().unwrap();
        assert_eq!(last.uv.2, 1.0);
        assert_eq!(last.uv.3, 1.0);
        let end = shatter(1.0, R, cols, rows);
        assert!(end.iter().all(|p| p.alpha < 0.01), "everything has faded");
    }

    #[test]
    fn the_pieces_by_the_middle_go_first_and_everything_falls() {
        let (cols, rows) = (8, 6);
        let moved = |ps: &[Piece], i: usize| {
            let a = ps[i].corners[0];
            let b = shatter(0.0, R, cols, rows)[i].corners[0];
            ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
        };
        let early = shatter(0.2, R, cols, rows);
        let middle = 2 * cols + 3; // near the centre
        let corner = 0;
        assert!(moved(&early, middle) > moved(&early, corner) + 1.0, "the middle piece has already left, the corner has not");
        // later the whole thing is lower than it started
        let late = shatter(0.8, R, cols, rows);
        let mean_y = |ps: &[Piece]| ps.iter().map(|p| p.corners[0].1 as f64).sum::<f64>() / ps.len() as f64;
        assert!(mean_y(&late) > mean_y(&shatter(0.0, R, cols, rows)) + 100.0, "it fell");
    }

    #[test]
    fn the_same_time_gives_the_same_picture() {
        assert_eq!(shatter(0.37, R, 8, 6), shatter(0.37, R, 8, 6));
    }
}
