//! The overview (Super+Tab): every window of the workspace laid out side by side on the dimmed
//! desktop, so that one can be picked at a glance. The windows glide from where they are to their
//! places and back. Click a window to go to it, click elsewhere or press Escape to leave.

use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Rectangle};

use crate::state::Fde;

/// How long the windows take to glide to their places (and back).
pub const EXPO_SECONDS: f32 = 0.35;

/// A place on the overview: left, top, width, height.
pub type Cell = (f64, f64, f64, f64);

/// Places for windows of `sizes` (width, height) on a screen of `w` x `h`: a grid with as many
/// columns as the square root of their number asks for, each window as big as its cell allows
/// (never more than 1.2 times its size) and centred in it; the last row is centred too.
/// Returns, for each window, the place of its picture.
pub fn layout(sizes: &[(f64, f64)], w: f64, h: f64) -> Vec<Cell> {
    let n = sizes.len();
    if n == 0 {
        return Vec::new();
    }
    let (side, top, bottom, gap) = (0.06 * w, 0.09 * h, 0.12 * h, 0.03 * w);
    let cols = ((n as f64 * (w / h).max(1.0)).sqrt().ceil() as usize).clamp(1, n);
    let rows = n.div_ceil(cols);
    let cell_w = (w - 2.0 * side - (cols as f64 - 1.0) * gap) / cols as f64;
    let cell_h = (h - top - bottom - (rows as f64 - 1.0) * gap) / rows as f64;
    let mut out = Vec::with_capacity(n);
    for (i, &(pw, ph)) in sizes.iter().enumerate() {
        let (row, col) = (i / cols, i % cols);
        let in_row = if row == rows - 1 { n - row * cols } else { cols };
        // a short last row is centred
        let row_w = in_row as f64 * cell_w + (in_row as f64 - 1.0) * gap;
        let x0 = (w - row_w) / 2.0 + col as f64 * (cell_w + gap);
        let y0 = top + row as f64 * (cell_h + gap);
        let k = (cell_w / pw.max(1.0)).min(cell_h / ph.max(1.0)).min(1.2);
        let (dw, dh) = (pw * k, ph * k);
        out.push((x0 + (cell_w - dw) / 2.0, y0 + (cell_h - dh) / 2.0, dw, dh));
    }
    out
}

/// `a` to `b` by `t` (0 .. 1), cell by cell.
pub fn mix(a: Cell, b: Cell, t: f64) -> Cell {
    let l = |x: f64, y: f64| x + (y - x) * t;
    (l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3))
}

/// The overview while it is open.
pub struct Expo {
    pub since: std::time::Instant,
    /// leaving: since when (the windows glide back)
    pub closing: Option<std::time::Instant>,
    pub windows: Vec<Window>,
    pub hover: Option<usize>,
}

impl Expo {
    /// How far the windows are on their way to their places: 0 (where they were) .. 1 (laid out).
    pub fn progress(&self) -> f64 {
        let ease = |t: f32| {
            let t = t.clamp(0.0, 1.0) as f64;
            t * t * (3.0 - 2.0 * t)
        };
        match self.closing {
            Some(c) => 1.0 - ease(c.elapsed().as_secs_f32() / EXPO_SECONDS),
            None => ease(self.since.elapsed().as_secs_f32() / EXPO_SECONDS),
        }
    }

    pub fn finished(&self) -> bool {
        self.closing.is_some_and(|c| c.elapsed().as_secs_f32() >= EXPO_SECONDS)
    }
}

impl Fde {
    /// Super+Tab: open the overview, or leave it.
    pub fn toggle_expo(&mut self) {
        if self.expo.is_some() {
            self.close_expo(None);
            return;
        }
        if self.locked || self.greeter || self.window_menu.is_some() {
            return;
        }
        // front to back, then the minimized ones of this workspace (as the switcher lists them)
        let mut windows: Vec<Window> = self
            .space
            .elements()
            .rev()
            .filter(|w| self.minimize_progress(w).is_none() && !self.is_override_redirect(w) && self.parent_window(w).is_none())
            .cloned()
            .collect();
        windows.extend(self.minimized.iter().rev().filter(|w| self.on_current_workspace(w)).cloned());
        if windows.is_empty() {
            return;
        }
        self.switcher = None;
        self.expo = Some(Expo { since: std::time::Instant::now(), closing: None, windows, hover: None });
        self.request_redraw();
    }

    /// Where the windows go, in the shared space (on the first monitor).
    pub fn expo_cells(&self) -> Vec<Rectangle<f64, Logical>> {
        let Some(e) = &self.expo else { return Vec::new() };
        let r = self.output_rect(&self.output);
        let sizes: Vec<(f64, f64)> = e.windows.iter().map(|w| (w.geometry().size.w as f64, w.geometry().size.h as f64)).collect();
        layout(&sizes, r.size.w as f64, r.size.h as f64)
            .into_iter()
            .map(|(x, y, w, h)| Rectangle::new((x + r.loc.x as f64, y + r.loc.y as f64).into(), (w, h).into()))
            .collect()
    }

    /// Leave the overview, going to `chosen` if there is one (back from the task bar too).
    pub fn close_expo(&mut self, chosen: Option<Window>) {
        let Some(e) = self.expo.as_mut() else { return };
        if e.closing.is_some() {
            return;
        }
        // start the way back from where the windows are laid out now
        // continue from where the windows are now: the time at which the way back has this progress
        let x = (1.0 - e.progress()).clamp(0.0, 1.0);
        let t = 0.5 - ((1.0 - 2.0 * x).clamp(-1.0, 1.0).asin() / 3.0).sin();
        e.closing = Some(std::time::Instant::now() - std::time::Duration::from_secs_f32(t as f32 * EXPO_SECONDS));
        e.hover = None;
        if let Some(w) = chosen {
            if self.minimized.contains(&w) {
                self.restore_specific(&w);
            }
            self.focus_window(&w);
        }
        self.request_redraw();
    }

    /// The pointer moved: light the window under it. True if the overview took the motion.
    pub fn expo_motion(&mut self, p: Point<f64, Logical>) -> bool {
        if self.expo.as_ref().is_none_or(|e| e.closing.is_some()) {
            return self.expo.is_some();
        }
        let hover = self.expo_cells().iter().position(|c| c.contains(p));
        if let Some(e) = self.expo.as_mut() {
            if e.hover != hover {
                e.hover = hover;
                self.request_redraw();
            }
        }
        true
    }

    /// A button went down in the overview: a window is chosen, anywhere else leaves.
    pub fn expo_click(&mut self, p: Point<f64, Logical>) {
        let cells = self.expo_cells();
        let chosen = cells.iter().position(|c| c.contains(p)).and_then(|i| self.expo.as_ref().and_then(|e| e.windows.get(i).cloned()));
        self.close_expo(chosen);
    }

    /// Housekeeping each frame: the overview ends when its way back is done (or the screen locks).
    pub fn tick_expo(&mut self) {
        if self.expo.as_ref().is_some_and(|e| e.finished()) || (self.locked && self.expo.is_some()) {
            self.expo = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(c: Cell, w: f64, h: f64) -> bool {
        c.0 >= 0.0 && c.1 >= 0.0 && c.0 + c.2 <= w + 1e-6 && c.1 + c.3 <= h + 1e-6
    }

    fn overlap(a: Cell, b: Cell) -> bool {
        a.0 < b.0 + b.2 - 1e-6 && b.0 < a.0 + a.2 - 1e-6 && a.1 < b.1 + b.3 - 1e-6 && b.1 < a.1 + a.3 - 1e-6
    }

    #[test]
    fn windows_get_places_that_fit_and_never_overlap() {
        let (w, h) = (1280.0, 800.0);
        for n in 1..=12 {
            let sizes: Vec<(f64, f64)> = (0..n).map(|i| (400.0 + 60.0 * i as f64, 300.0 + 20.0 * i as f64)).collect();
            let cells = layout(&sizes, w, h);
            assert_eq!(cells.len(), n);
            for (i, c) in cells.iter().enumerate() {
                assert!(inside(*c, w, h), "n={n} cell {i} {c:?} is on the screen");
                // the shape of the window is kept
                let (pw, ph) = sizes[i];
                assert!((c.2 / c.3 - pw / ph).abs() < 1e-6);
                for d in &cells[i + 1..] {
                    assert!(!overlap(*c, *d), "n={n}: {c:?} and {d:?} overlap");
                }
            }
        }
        assert!(layout(&[], w, h).is_empty());
    }

    #[test]
    fn a_small_window_is_not_blown_up_much_and_a_big_one_shrinks() {
        let cells = layout(&[(100.0, 80.0)], 1280.0, 800.0);
        assert!((cells[0].2 - 120.0).abs() < 1e-6, "at most 1.2 times: {:?}", cells[0]);
        let cells = layout(&[(4000.0, 3000.0)], 1280.0, 800.0);
        assert!(cells[0].2 < 1280.0 && cells[0].3 < 800.0);
        // one window is in the middle
        let c = layout(&[(500.0, 400.0)], 1280.0, 800.0)[0];
        assert!((c.0 + c.2 / 2.0 - 640.0).abs() < 1.0);
    }

    #[test]
    fn a_short_last_row_is_centred() {
        let sizes = vec![(400.0, 300.0); 5]; // 3 columns: 3 + 2
        let cells = layout(&sizes, 1280.0, 800.0);
        let mid = |c: Cell| c.0 + c.2 / 2.0;
        assert!((mid(cells[3]) + mid(cells[4]) - 1280.0).abs() < 1.0, "the two below are centred as a pair");
    }

    #[test]
    fn mixing_goes_from_one_place_to_the_other() {
        let (a, b) = ((0.0, 0.0, 100.0, 100.0), (200.0, 100.0, 50.0, 50.0));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), (100.0, 50.0, 75.0, 75.0));
    }
}
