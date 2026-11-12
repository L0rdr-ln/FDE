//! Alt+T: the windows of the monitor the pointer is on (this workspace) side by side in a grid,
//! the front one at the top left. Alt+T again puts them back where they were.

use smithay::desktop::Window;
use smithay::utils::{Logical, Rectangle};

use crate::state::Fde;

/// Room between the windows, and around them, in logical pixels.
pub const GAP: i32 = 8;

/// `n` cells filling `area`: as many columns as rows, or one more; the last row may be shorter
/// and then shares its width among fewer windows.
pub fn grid(area: Rectangle<i32, Logical>, n: usize) -> Vec<Rectangle<i32, Logical>> {
    if n == 0 {
        return Vec::new();
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let inner_h = area.size.h - GAP * (rows as i32 + 1);
    let cell_h = inner_h / rows as i32;
    let mut out = Vec::with_capacity(n);
    for r in 0..rows {
        let in_row = if r + 1 == rows { n - cols * (rows - 1) } else { cols };
        let cell_w = (area.size.w - GAP * (in_row as i32 + 1)) / in_row as i32;
        for c in 0..in_row {
            let x = area.loc.x + GAP + c as i32 * (cell_w + GAP);
            let y = area.loc.y + GAP + r as i32 * (cell_h + GAP);
            out.push(Rectangle::new((x, y).into(), (cell_w.max(1), cell_h.max(1)).into()));
        }
    }
    out
}

impl Fde {
    /// The windows Alt+T arranges: on the pointer's monitor and this workspace, not minimized,
    /// not full screen, front first.
    pub fn tileable_windows(&self) -> Vec<Window> {
        let out = self.output_at(self.pointer_location).unwrap_or_else(|| self.output.clone());
        self.space
            .elements()
            .rev()
            .filter(|w| !self.is_override_redirect(w) && !self.is_fullscreen(w) && self.minimize_progress(w).is_none())
            .filter(|w| self.output_for_window(w) == out)
            .cloned()
            .collect()
    }

    /// Alt+T: tile, or put back what the last Alt+T tiled.
    pub fn toggle_tiling(&mut self) {
        // moving a window brings it forward: the order front to back is put back at the end
        let order: Vec<Window> = self.space.elements().cloned().collect();
        self.tile_or_untile();
        for w in &order {
            if self.space.elements().any(|e| e == w) {
                self.space.raise_element(w, false);
            }
        }
        self.raise_on_top_windows();
    }

    fn tile_or_untile(&mut self) {
        let windows = self.tileable_windows();
        if let Some(before) = self.tiled_before.take() {
            // the same windows, still where tiling put them: back to where they were
            let same = before.len() == windows.len() && before.iter().all(|(w, _, _, _)| windows.contains(w));
            if same {
                for (w, rect, maximized, snapped) in before {
                    self.place_client(&w, rect);
                    if maximized {
                        self.set_maximized(&w, true);
                        crate::windows::info(&w).lock().unwrap().restore = Some(rect);
                    }
                    // a snapped window is back in its half, and still knows its place before that
                    crate::windows::info(&w).lock().unwrap().snapped = snapped;
                }
                self.request_redraw();
                return;
            }
        }
        if windows.is_empty() {
            return;
        }
        let out = self.output_for_window(&windows[0]);
        let area = self.usable_area(&out);
        let cells = grid(area, windows.len());
        let mut before = Vec::new();
        for (w, cell) in windows.iter().zip(cells) {
            let maximized = self.is_maximized(w);
            let snapped = self.snapped_rect(w);
            let Some(now) = self.space.element_geometry(w) else { continue };
            let back_to = if maximized { self.restore_rect(w).unwrap_or(now) } else { now };
            before.push((w.clone(), back_to, maximized, snapped));
            {
                let mut i = crate::windows::info(w).lock().unwrap();
                i.restore = None;
                i.snapped = None;
            }
            if maximized {
                if let Some(top) = w.toplevel() {
                    top.with_pending_state(|s| {
                        s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized)
                    });
                }
                if let Some(x) = w.x11_surface() {
                    let _ = x.set_maximized(false);
                }
            }
            let rect = crate::layout::client_rect(cell, &self.frame_style(w));
            self.place_client(w, rect);
        }
        self.tiled_before = Some(before);
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_fills_the_area_without_overlap() {
        let area = Rectangle::new((0, 30).into(), (1000, 700).into());
        for n in 1..=10 {
            let cells = grid(area, n);
            assert_eq!(cells.len(), n);
            for (i, a) in cells.iter().enumerate() {
                assert!(area.contains_rect(*a), "{n}: cell {i} {a:?} is inside");
                for b in &cells[i + 1..] {
                    assert!(!a.overlaps(*b), "{n}: {a:?} and {b:?} do not overlap");
                }
            }
        }
        // three windows: two on top, one as wide as both below
        let c = grid(area, 3);
        assert_eq!(c[0].size.w, c[1].size.w);
        assert!(c[2].size.w > c[0].size.w * 2 - 10 && c[2].loc.y > c[0].loc.y);
        assert_eq!(grid(area, 1)[0], Rectangle::new((GAP, 30 + GAP).into(), (1000 - 2 * GAP, 700 - 2 * GAP).into()));
    }
}
