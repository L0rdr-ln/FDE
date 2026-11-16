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
    /// burns away from the bottom, edges glowing
    Burn,
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
            "burn" | "fire" => CloseEffect::Burn,
            _ => CloseEffect::Fade,
        }
    }

    pub fn seconds(self) -> f32 {
        match self {
            CloseEffect::Fade => 0.18,
            CloseEffect::Fold => 0.4,
            CloseEffect::Shatter => 0.8,
            CloseEffect::Burn => 1.2,
        }
    }
}

/// How a new window appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenEffect {
    /// fades in and grows a little
    Pop,
    /// unrolls downwards from its title bar, with a small overshoot
    Unfold,
}

impl OpenEffect {
    pub fn parse(s: &str) -> OpenEffect {
        match s {
            "unfold" => OpenEffect::Unfold,
            _ => OpenEffect::Pop,
        }
    }

    pub fn seconds(self) -> f32 {
        match self {
            OpenEffect::Pop => 0.18,
            OpenEffect::Unfold => 0.38,
        }
    }
}

/// How long the workspace cube takes to turn.
pub const CUBE_SECONDS: f32 = 0.55;

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

/// A window unrolling: the grid of `rect` `p` (0 .. 1) of the way, its top edge fixed and its
/// bottom edge running down past the end a little and coming back. Returns the points and how
/// opaque it is (it is fully visible after the first quarter).
pub fn unfold(p: f64, rect: Rect, n: usize) -> (Vec<(f32, f32)>, f32) {
    let (x, y, w, h) = rect;
    let p = p.clamp(0.0, 1.0);
    // ease-out with overshoot ("back"): 0 -> about 1.06 -> 1
    let c1 = 1.2;
    let t = p - 1.0;
    let grow = (1.0 + (c1 + 1.0) * t * t * t + c1 * t * t).max(0.02);
    let wide = 0.94 + 0.06 * smooth(p * 1.5); // the width opens a little too
    let cx = x + w / 2.0;
    let mut pts = Vec::with_capacity(n * n);
    for i in 0..n {
        let v = i as f64 / (n - 1) as f64;
        for j in 0..n {
            let u = j as f64 / (n - 1) as f64 - 0.5;
            pts.push(((cx + u * w * wide) as f32, (y + v * h * grow) as f32));
        }
    }
    (pts, smooth(p * 4.0) as f32)
}

/// The two faces of the workspace cube while it turns: the one that leaves (`.0`) and the one
/// that arrives (`.1`), each as `n` x `n` points on the screen of `w` x `h` pixels and how bright it
/// is (a face turned away is darker). `dir` is +1 when the cube turns to the left (the next
/// workspace comes in from the right) and -1 the other way. `p` is 0 .. 1.
pub fn cube(p: f64, dir: i32, w: f64, h: f64, n: usize) -> [(Vec<(f32, f32)>, f32); 2] {
    let p = smooth(p);
    let theta = p * PI / 2.0;
    let sign = if dir >= 0 { 1.0 } else { -1.0 };
    let face = |phi: f64| -> (Vec<(f32, f32)>, f32) {
        let r = w / 2.0;
        let camera = 1.8 * w;
        let (cx, cy) = (w / 2.0, h / 2.0);
        let mut pts = Vec::with_capacity(n * n);
        for i in 0..n {
            let v = i as f64 / (n - 1) as f64 - 0.5;
            for j in 0..n {
                let x = (j as f64 / (n - 1) as f64 - 0.5) * w;
                let xr = x * phi.cos() + r * phi.sin();
                let zr = -x * phi.sin() + r * phi.cos();
                let s = camera / (camera - (zr - r));
                pts.push(((cx + xr * s) as f32, (cy + v * h * s) as f32));
            }
        }
        // a face turned too far away to be seen from the camera (its back would show) is not drawn
        let seen = phi.cos() > r / (camera + r);
        (pts, if seen { (1.0 - 0.6 * phi.abs() / (PI / 2.0)) as f32 } else { 0.0 })
    };
    [face(-sign * theta), face(sign * (PI / 2.0 - theta))]
}

/// One window of the window stack (Flip 3D), `d` places from the chosen one (0 = the chosen one at
/// the front; 1, 2, ... behind it; a fraction while the stack turns), as `n` x `n` points on a
/// screen of `w` x `h` pixels, for a picture `pw` x `ph` pixels (shown at most that big), and how
/// opaque it is. The windows are turned towards the middle and recede to the right.
pub fn flip(d: f64, w: f64, h: f64, pw: f64, ph: f64, n: usize) -> (Vec<(f32, f32)>, f32) {
    let k = (0.56 * w / pw.max(1.0)).min(0.56 * h / ph.max(1.0)).min(1.5);
    let (hw, hh) = (pw * k / 2.0, ph * k / 2.0);
    let yaw: f64 = 0.62; // about 35 degrees
    let camera = 1.7 * w;
    let ad = d.abs();
    let centre = (w * (0.30 + 0.13 * d), h * (0.52 - 0.045 * d), -0.28 * w * ad);
    let mut pts = Vec::with_capacity(n * n);
    for i in 0..n {
        let y = (i as f64 / (n - 1) as f64 - 0.5) * 2.0 * hh;
        for j in 0..n {
            let x = (j as f64 / (n - 1) as f64 - 0.5) * 2.0 * hw;
            let xr = x * yaw.cos();
            let zr = -x * yaw.sin(); // the right edge is farther away
            let s = camera / (camera - (centre.2 + zr));
            pts.push(((w / 2.0 + (centre.0 - w / 2.0 + xr) * s) as f32, (h / 2.0 + (centre.1 - h / 2.0 + y) * s) as f32));
        }
    }
    let alpha = (1.0 - 0.08 * ad).clamp(0.35, 1.0) * (1.0 - (ad - 5.0).clamp(0.0, 1.0));
    (pts, alpha as f32)
}

/// A soap bubble: a see-through ball with a bright rim and a shine, `n` x `n` pixels,
/// premultiplied RGBA.
pub fn bubble_sprite(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    let r = n as f64 / 2.0 - 1.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x as f64 + 0.5 - n as f64 / 2.0) / r, (y as f64 + 0.5 - n as f64 / 2.0) / r);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 1.0 {
                continue;
            }
            let inside = 1.0 - smooth((d - 0.97) / 0.03);
            // faint body, bright rim
            let mut a = (0.04 + 0.10 * d * d) * inside + 0.55 * smooth((d - 0.78) / 0.19) * inside;
            let mut c = [0.85, 0.95, 1.0];
            // lower right: light coming through, a little warmer
            let lr = ((dx + dy) / 2.0).max(0.0) * smooth((d - 0.55) / 0.4);
            a += 0.25 * lr;
            c = [c[0] + 0.1 * lr, c[1] + 0.03 * lr, c[2] - 0.08 * lr];
            // the shine, up left
            let (sx, sy) = (dx + 0.38, dy + 0.42);
            let sd = (sx * sx / 0.8 + sy * sy * 1.4).sqrt();
            let shine = (1.0 - smooth(sd / 0.2)) * 0.9;
            a = a + shine * (1.0 - a);
            let a = a.clamp(0.0, 1.0);
            let i = (y * n + x) * 4;
            for k in 0..3 {
                out[i + k] = (c[k].clamp(0.0, 1.0) * a * 255.0).round() as u8;
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// A thin soft ring (for a ripple), `n` x `n` pixels, premultiplied RGBA.
pub fn ring_sprite(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    let r = n as f64 / 2.0 - 1.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x as f64 + 0.5 - n as f64 / 2.0) / r, (y as f64 + 0.5 - n as f64 / 2.0) / r);
            let d = (dx * dx + dy * dy).sqrt();
            // a band around 0.9, soft on both sides
            let a = (smooth((d - 0.72) / 0.18) * (1.0 - smooth((d - 0.92) / 0.08))).clamp(0.0, 1.0) * 0.9;
            let i = (y * n + x) * 4;
            out[i] = (0.9 * a * 255.0).round() as u8;
            out[i + 1] = (0.97 * a * 255.0).round() as u8;
            out[i + 2] = (a * 255.0).round() as u8;
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// Where a bubble is: its middle, its width and how opaque it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bubble {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub alpha: f64,
}

/// `count` bubbles rising through a screen of `w` x `h` at time `t` seconds: each has its own
/// size, speed and sway, comes up from below the bottom edge and leaves above the top, fading at
/// both ends. The same time gives the same picture.
pub fn bubbles(t: f64, w: f64, h: f64, count: usize) -> Vec<Bubble> {
    (0..count)
        .map(|i| {
            let period = 16.0 + 16.0 * noise(i, 0, 11);
            let u = (t / period + noise(i, 1, 12)).rem_euclid(1.0);
            let size = 16.0 + 84.0 * noise(i, 5, 16).powi(2);
            let sway = 18.0 + 34.0 * noise(i, 3, 14);
            let x = w * (0.04 + 0.92 * noise(i, 2, 13)) + sway * (t * (0.35 + 0.4 * noise(i, 4, 15)) + i as f64 * 1.7).sin();
            let y = h + size / 2.0 - u * (h + size);
            let fade = smooth(u / 0.08) * (1.0 - smooth((u - 0.9) / 0.1));
            Bubble { x, y, size, alpha: 0.85 * fade }
        })
        .collect()
}

/// A ripple `t` (0 .. 1) of the way: the radius of its ring and how opaque it is.
pub fn ripple(t: f64) -> (f64, f64) {
    let t = t.clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - t).powi(3);
    (10.0 + 70.0 * e, 0.85 * (1.0 - t).powf(1.4))
}

/// How long a click ripple lasts.
pub const RIPPLE_SECONDS: f32 = 0.7;

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
        assert_eq!(CloseEffect::parse("burn"), CloseEffect::Burn);
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

    #[test]
    fn a_window_unrolls_from_its_top_edge() {
        let n = 7;
        let (start, a0) = unfold(0.0, R, n);
        assert!(start[n * n - 1].1 - start[0].1 < 20.0, "almost flat at the start");
        assert!(a0 < 0.01);
        assert!((start[0].1 - 80.0).abs() < 1e-3, "the top edge stays");
        let (mid, _) = unfold(0.5, R, n);
        assert!(mid[n * n - 1].1 > start[n * n - 1].1);
        // the bottom runs a little past the final size and comes back
        let bottom = |p: f64| unfold(p, R, n).0[n * n - 1].1;
        assert!((0..=100).any(|k| bottom(k as f64 / 100.0) > 380.5), "overshoots");
        let (end, a1) = unfold(1.0, R, n);
        assert!((end[n * n - 1].1 - 380.0).abs() < 1e-2 && (end[n * n - 1].0 - 500.0).abs() < 1e-2 && a1 == 1.0);
        assert_eq!(OpenEffect::parse("unfold"), OpenEffect::Unfold);
        assert_eq!(OpenEffect::parse("pop"), OpenEffect::Pop);
    }

    #[test]
    fn the_cube_turns_one_face_away_and_the_next_in() {
        let (w, h, n) = (800.0, 600.0, 9);
        let [out, inc] = cube(0.0, 1, w, h, n);
        assert!((out.0[0].0).abs() < 1e-2 && (out.0[0].1).abs() < 1e-2, "the old face is the screen");
        assert!((out.0[n * n - 1].0 - 800.0).abs() < 1e-2 && (out.0[n * n - 1].1 - 600.0).abs() < 1e-2);
        assert_eq!(out.1, 1.0);
        // the new face is still behind the corner: not seen
        assert_eq!(inc.1, 0.0);
        let [out, inc] = cube(1.0, 1, w, h, n);
        assert_eq!(out.1, 0.0, "the old one has turned out of sight");
        assert!((inc.0[0].0).abs() < 1e-2 && (inc.0[n * n - 1].0 - 800.0).abs() < 1e-2, "the new one is the screen");
        assert_eq!(inc.1, 1.0);
        // halfway the two meet in the middle and are equally bright
        let [out, inc] = cube(0.5, 1, w, h, n);
        assert!((out.1 - inc.1).abs() < 1e-3);
        let right_of_out = out.0[n - 1].0; // top right of the leaving face
        let left_of_in = inc.0[0].0; // top left of the arriving one
        assert!((right_of_out - left_of_in).abs() < 1.0, "they share an edge: {right_of_out} {left_of_in}");
        // the other way round mirrors it
        // turning the other way mirrors it: the leaving face moves to the right
        let [out_l, _] = cube(0.5, -1, w, h, n);
        let [out_r, _] = cube(0.5, 1, w, h, n);
        assert!(out_l.0[0].0 > out_r.0[0].0 + 100.0);
    }

    #[test]
    fn the_cube_edge_in_the_middle_is_taller_near_the_viewer() {
        let [out, inc] = cube(0.5, 1, 800.0, 600.0, 9);
        let height = |f: &(Vec<(f32, f32)>, f32), col: usize| f.0[8 * 9 + col].1 - f.0[col].1;
        // the shared edge is the leaving face's right side and the arriving face's left side
        assert!((height(&out, 8) - height(&inc, 0)).abs() < 1.0);
        assert!(height(&out, 8) < height(&out, 0) + 1.0 || height(&out, 8) > 0.0);
    }

    #[test]
    fn the_stack_recedes_to_the_right_and_gets_smaller() {
        let (w, h, n) = (1280.0, 800.0, 6);
        let width = |p: &(Vec<(f32, f32)>, f32)| p.0[n - 1].0 - p.0[0].0;
        let left = |p: &(Vec<(f32, f32)>, f32)| p.0[0].0;
        let front = flip(0.0, w, h, 700.0, 500.0, n);
        let second = flip(1.0, w, h, 700.0, 500.0, n);
        let fifth = flip(4.0, w, h, 700.0, 500.0, n);
        assert!(width(&front) > width(&second) && width(&second) > width(&fifth), "smaller behind");
        assert!(left(&front) < left(&second) && left(&second) < left(&fifth), "to the right behind");
        assert_eq!(front.1, 1.0);
        assert!(fifth.1 < front.1, "dimmer behind");
        // each window's right edge is farther away than its left: it is shorter
        let edge = |p: &(Vec<(f32, f32)>, f32), col: usize| p.0[(n - 1) * n + col].1 - p.0[col].1;
        assert!(edge(&front, n - 1) < edge(&front, 0), "turned: the right edge is shorter");
        // a window of any size stays on the screen
        let big = flip(0.0, w, h, 4000.0, 3000.0, n);
        assert!(big.0.iter().all(|&(x, y)| x >= 0.0 && x <= 1280.0 && y >= 0.0 && y <= 800.0));
        // very far back is invisible
        assert_eq!(flip(9.0, w, h, 700.0, 500.0, n).1, 0.0);
        // fractions in between are continuous
        let a = flip(0.5, w, h, 700.0, 500.0, n).0[0].0;
        assert!(a > left(&front) && a < left(&second));
    }

    #[test]
    fn a_bubble_is_clear_inside_bright_at_the_rim_with_a_shine() {
        let n = 64;
        let px = bubble_sprite(n);
        let alpha = |x: usize, y: usize| px[(y * n + x) * 4 + 3] as f64 / 255.0;
        assert!(alpha(32, 32) < 0.25, "see-through in the middle: {}", alpha(32, 32));
        assert!(alpha(32, 3) > 0.35 && alpha(60, 32) > 0.35, "the rim shows");
        assert!(alpha(0, 0) == 0.0 && alpha(63, 63) == 0.0, "nothing outside the circle");
        let shine = alpha(32 - 12, 32 - 14);
        assert!(shine > 0.6, "a bright shine up left: {shine}");
        // premultiplied: colour never exceeds alpha
        assert!(px.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
    }

    #[test]
    fn a_ring_is_hollow() {
        let n = 64;
        let px = ring_sprite(n);
        let alpha = |x: usize, y: usize| px[(y * n + x) * 4 + 3];
        assert_eq!(alpha(32, 32), 0);
        assert!(alpha(32 + 28, 32) > 150, "the ring is near the edge");
        assert_eq!(alpha(0, 0), 0);
    }

    #[test]
    fn bubbles_rise_sway_and_fade_at_the_ends() {
        let (w, h) = (1280.0, 800.0);
        let a = bubbles(10.0, w, h, 12);
        assert_eq!(a.len(), 12);
        assert_eq!(a, bubbles(10.0, w, h, 12), "the same time, the same picture");
        let b = bubbles(10.5, w, h, 12);
        let rising = a.iter().zip(&b).filter(|(p, q)| q.y < p.y).count();
        assert!(rising >= 11, "they go up ({rising} of 12)");
        // always inside the screen's width (with the sway) and never out of the 0..1 opacity
        for t in 0..400 {
            for bub in bubbles(t as f64 * 0.37, w, h, 12) {
                assert!(bub.x > -60.0 && bub.x < w + 60.0, "x {}", bub.x);
                assert!((0.0..=1.0).contains(&bub.alpha));
                assert!(bub.size >= 16.0 && bub.size <= 100.0);
                if bub.y > h + bub.size / 2.0 - 1.0 || bub.y < -bub.size / 2.0 + 1.0 {
                    assert!(bub.alpha < 0.05, "invisible when it is off the screen");
                }
            }
        }
        // different bubbles differ
        assert!(a.windows(2).any(|p| (p[0].size - p[1].size).abs() > 1.0));
    }

    #[test]
    fn a_ripple_spreads_and_fades() {
        let (r0, a0) = ripple(0.0);
        let (r1, a1) = ripple(0.5);
        let (r2, a2) = ripple(1.0);
        assert!(r0 < r1 && r1 < r2);
        assert!(a0 > a1 && a1 > a2 && a2 < 0.01);
    }
}
