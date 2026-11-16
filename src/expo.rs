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

/// The buttons of the workspaces along the top of the overview (`count` of them, centred): each is
/// about 110 x 56 pixels, smaller on a small screen.
pub fn strip(count: usize, w: f64, h: f64) -> Vec<Cell> {
    if count == 0 {
        return Vec::new();
    }
    let bh = (0.07 * h).clamp(30.0, 56.0);
    let bw = (bh * 1.9).min((w - 24.0) / count as f64 - 10.0).max(24.0);
    let gap = 10.0;
    let total = count as f64 * bw + (count as f64 - 1.0) * gap;
    let x0 = (w - total) / 2.0;
    (0..count).map(|i| (x0 + i as f64 * (bw + gap), 10.0, bw, bh)).collect()
}

/// `a` to `b` by `t` (0 .. 1), cell by cell.
pub fn mix(a: Cell, b: Cell, t: f64) -> Cell {
    let l = |x: f64, y: f64| x + (y - x) * t;
    (l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3))
}

/// The overview while it is open.
/// A window being carried across the overview with the button down.
#[derive(Debug, Clone)]
pub struct ExpoDrag {
    pub index: usize,
    pub start: Point<f64, Logical>,
    pub at: Point<f64, Logical>,
    /// the pointer went far enough for this to be a drag and not a click
    pub moved: bool,
}

pub struct Expo {
    pub drag: Option<ExpoDrag>,
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
        self.expo = Some(Expo { drag: None, since: std::time::Instant::now(), closing: None, windows, hover: None });
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
        // a window that went away while the overview was open is not a choice
        let chosen = chosen.filter(|w| smithay::utils::IsAlive::alive(w) && (self.space.elements().any(|e| e == w) || self.minimized.contains(w)));
        if let Some(w) = chosen {
            if self.minimized.contains(&w) {
                self.restore_specific(&w);
            }
            self.focus_window(&w);
        }
        self.request_redraw();
    }

    /// Where the workspace buttons are, in the shared space (on the first monitor).
    pub fn expo_strip(&self) -> Vec<Rectangle<f64, Logical>> {
        let r = self.output_rect(&self.output);
        strip(self.workspace_count(), r.size.w as f64, r.size.h as f64)
            .into_iter()
            .map(|(x, y, w, h)| Rectangle::new((x + r.loc.x as f64, y + r.loc.y as f64).into(), (w, h).into()))
            .collect()
    }

    /// The pointer moved: light the window under it, or carry the window being dragged. True if
    /// the overview took the motion.
    pub fn expo_motion(&mut self, p: Point<f64, Logical>) -> bool {
        if self.expo.as_ref().is_none_or(|e| e.closing.is_some()) {
            return self.expo.is_some();
        }
        let hover = self.expo_cells().iter().position(|c| c.contains(p));
        if let Some(e) = self.expo.as_mut() {
            if let Some(d) = e.drag.as_mut() {
                d.at = p;
                if !d.moved && ((p.x - d.start.x).powi(2) + (p.y - d.start.y).powi(2)).sqrt() > 8.0 {
                    d.moved = true;
                }
                self.request_redraw();
                return true;
            }
            if e.hover != hover {
                e.hover = hover;
                self.request_redraw();
            }
        }
        true
    }

    /// A button went down in the overview: a workspace button goes there, a window may be picked
    /// up (a click chooses it, a drag onto a workspace button sends it there), anywhere else leaves.
    pub fn expo_press(&mut self, p: Point<f64, Logical>) {
        if self.expo.as_ref().is_none_or(|e| e.closing.is_some()) {
            return;
        }
        if let Some(n) = self.expo_strip().iter().position(|b| b.contains(p)) {
            if n != self.workspace {
                self.switch_workspace(n);
            }
            self.close_expo(None);
            return;
        }
        let cells = self.expo_cells();
        match cells.iter().position(|c| c.contains(p)) {
            Some(i) => {
                if let Some(e) = self.expo.as_mut() {
                    e.drag = Some(ExpoDrag { index: i, start: p, at: p, moved: false });
                }
                self.request_redraw();
            }
            None => self.close_expo(None),
        }
    }

    /// The button went up: the carried window is dropped on a workspace button (and goes there) or
    /// was only clicked (and is chosen).
    pub fn expo_release(&mut self, p: Point<f64, Logical>) {
        let Some(drag) = self.expo.as_mut().and_then(|e| e.drag.take()) else { return };
        let window = self.expo.as_ref().and_then(|e| e.windows.get(drag.index).cloned());
        let Some(window) = window else { return };
        if !drag.moved {
            self.close_expo(Some(window));
            return;
        }
        if let Some(n) = self.expo_strip().iter().position(|b| b.contains(p)) {
            if n != self.window_workspace_of(&window) {
                self.move_window_to_workspace(&window, n);
                if let Some(e) = self.expo.as_mut() {
                    e.windows.retain(|w| *w != window);
                    if e.windows.is_empty() {
                        self.expo = None;
                    }
                }
                self.hud = Some((std::time::Instant::now(), format!("Moved to workspace {}", n + 1)));
            }
        }
        self.request_redraw();
    }

    /// The hot corner: the pointer in the top left corner of the first monitor for a quarter of a
    /// second opens the overview (once; it takes the pointer leaving the corner to arm it again).
    pub fn check_hot_corner(&mut self) {
        if !self.config.shell.hot_corner || self.locked || self.greeter || self.expo.is_some() {
            self.corner_since = None;
            return;
        }
        let r = self.output_rect(&self.output);
        let p = self.pointer_location;
        let in_corner = p.x >= r.loc.x as f64 && p.y >= r.loc.y as f64 && p.x < r.loc.x as f64 + 2.0 && p.y < r.loc.y as f64 + 2.0;
        if !in_corner {
            self.corner_since = None;
            self.corner_armed = true;
            return;
        }
        if !self.corner_armed {
            return;
        }
        match self.corner_since {
            None => {
                self.corner_since = Some(std::time::Instant::now());
                self.request_redraw();
            }
            Some(t) if t.elapsed().as_secs_f32() >= 0.25 => {
                self.corner_since = None;
                self.corner_armed = false;
                // not over a full-screen window (a game's menu bar is up there)
                let out = self.output.clone();
                if !self.fullscreen_on(&out) {
                    self.toggle_expo();
                }
            }
            Some(_) => {}
        }
    }

    /// Housekeeping each frame: the overview ends when its way back is done (or the screen locks).
    pub fn tick_expo(&mut self) {
        // windows that closed meanwhile leave the overview (it ends with the last one)
        if let Some(e) = self.expo.as_mut() {
            e.windows.retain(|w| smithay::utils::IsAlive::alive(w));
            if e.windows.is_empty() {
                self.expo = None;
                return;
            }
        }
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

    #[test]
    fn the_workspace_buttons_are_centred_along_the_top_and_fit() {
        for (w, h) in [(1920.0, 1080.0), (1280.0, 800.0), (320.0, 200.0)] {
            for count in 1..=9 {
                let b = strip(count, w, h);
                assert_eq!(b.len(), count);
                for (i, c) in b.iter().enumerate() {
                    assert!(c.0 >= 0.0 && c.0 + c.2 <= w + 1e-6 || w < 400.0, "{count} buttons on {w}: {c:?}");
                    assert!(c.1 >= 0.0 && c.1 + c.3 < h * 0.15 + 30.0);
                    assert!(b[i + 1..].iter().all(|d| !overlap(*c, *d)));
                }
                let mid = (b[0].0 + b[count - 1].0 + b[count - 1].2) / 2.0;
                assert!((mid - w / 2.0).abs() < 1.0, "centred");
            }
        }
        assert!(strip(0, 800.0, 600.0).is_empty());
        // the buttons are above the windows (the layout leaves room for them)
        let top = layout(&[(400.0, 300.0)], 1280.0, 800.0)[0].1;
        let bottom_of_strip = strip(4, 1280.0, 800.0)[0].1 + strip(4, 1280.0, 800.0)[0].3;
        assert!(top >= bottom_of_strip, "the windows start below the buttons");
    }
}
