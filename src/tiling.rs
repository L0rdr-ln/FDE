//! Tiling: Alt+T switches the workspace into a tiling mode. Its windows are laid out by a layout
//! (master and stack, grid or monocle) and stay laid out: a new window takes the master place, a
//! closed one leaves a gap that the others fill, a minimized one steps out. Alt+T again leaves the
//! mode and puts every window back where it was. Dialogs, windows of a fixed size, maximized and
//! full-screen windows and windows set to float stay as they are.
//!
//! Keys (Alt is the modifier everywhere in FDE): J / K focus the next / previous window,
//! Shift+J / Shift+K move the focused window down / up the order, Z makes it the master,
//! Shift+Left / Right change the width of the master area, Shift+Up / Down the number of master
//! windows, Y changes the layout, Shift+F makes a window float (or tile again). Dragging a tiled
//! window by its title bar onto another one swaps them; dragging an edge of the master area
//! changes its width.

use smithay::desktop::Window;
use smithay::utils::IsAlive;
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


/// How the tiled windows of a workspace are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// the master windows in a column on the left, the others in a column on the right
    MasterStack,
    /// as many columns as rows
    Grid,
    /// every window as big as the area (the front one is seen)
    Monocle,
}

impl Layout {
    pub fn next(self) -> Layout {
        match self {
            Layout::MasterStack => Layout::Grid,
            Layout::Grid => Layout::Monocle,
            Layout::Monocle => Layout::MasterStack,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::MasterStack => "master and stack",
            Layout::Grid => "grid",
            Layout::Monocle => "monocle",
        }
    }
}

/// What a workspace in tiling mode is set to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileState {
    pub on: bool,
    pub layout: Layout,
    /// the share of the width the master column takes
    pub ratio: f64,
    /// how many windows are masters
    pub masters: usize,
}

impl Default for TileState {
    fn default() -> Self {
        TileState { on: false, layout: Layout::MasterStack, ratio: 0.55, masters: 1 }
    }
}

pub const MIN_RATIO: f64 = 0.15;
pub const MAX_RATIO: f64 = 0.85;

/// `count` cells one above the other in a column at `x` that is `w` wide, filling the height of
/// `area` (no rounding gaps).
fn column(area: Rectangle<i32, Logical>, x: i32, w: i32, count: usize) -> Vec<Rectangle<i32, Logical>> {
    let inner = (area.size.h - GAP * (count as i32 + 1)).max(count as i32);
    (0..count as i32)
        .map(|i| {
            let (top, bottom) = (inner * i / count as i32, inner * (i + 1) / count as i32);
            Rectangle::new((x, area.loc.y + GAP + i * GAP + top).into(), (w.max(1), (bottom - top).max(1)).into())
        })
        .collect()
}

/// Master and stack: `masters` windows in a column at the left taking `ratio` of the width, the
/// rest in a column at the right. With no more windows than masters there is only the one column.
pub fn master_stack(area: Rectangle<i32, Logical>, n: usize, masters: usize, ratio: f64) -> Vec<Rectangle<i32, Logical>> {
    if n == 0 {
        return Vec::new();
    }
    let m = masters.clamp(1, n);
    if n == m {
        return column(area, area.loc.x + GAP, area.size.w - 2 * GAP, n);
    }
    let inner_w = area.size.w - 3 * GAP;
    let master_w = (inner_w as f64 * ratio.clamp(MIN_RATIO, MAX_RATIO)).round() as i32;
    let stack_w = inner_w - master_w;
    let mut out = column(area, area.loc.x + GAP, master_w, m);
    out.extend(column(area, area.loc.x + 2 * GAP + master_w, stack_w, n - m));
    out
}

/// Every window as big as the area.
pub fn monocle(area: Rectangle<i32, Logical>, n: usize) -> Vec<Rectangle<i32, Logical>> {
    let cell = Rectangle::new((area.loc.x + GAP, area.loc.y + GAP).into(), ((area.size.w - 2 * GAP).max(1), (area.size.h - 2 * GAP).max(1)).into());
    vec![cell; n]
}

/// The cells of `layout` for `n` windows.
pub fn cells(state: &TileState, area: Rectangle<i32, Logical>, n: usize) -> Vec<Rectangle<i32, Logical>> {
    match state.layout {
        Layout::MasterStack => master_stack(area, n, state.masters, state.ratio),
        Layout::Grid => grid(area, n),
        Layout::Monocle => monocle(area, n),
    }
}

/// The share of the width a master column of outer width `master_w` has, in an area `area_w` wide.
pub fn ratio_for_master_width(master_w: i32, area_w: i32) -> f64 {
    (master_w as f64 / (area_w - 3 * GAP).max(1) as f64).clamp(MIN_RATIO, MAX_RATIO)
}

impl Fde {
    /// Is the workspace being shown in tiling mode?
    pub fn tiling_on(&self) -> bool {
        self.tiling.get(&self.workspace).is_some_and(|s| s.on)
    }

    fn tile_state(&self) -> TileState {
        self.tiling.get(&self.workspace).copied().unwrap_or_default()
    }

    fn say(&mut self, text: String) {
        self.hud = Some((std::time::Instant::now(), text));
    }

    /// A window that cannot change its size (min and max size the same): it floats.
    fn fixed_size(&self, window: &Window) -> bool {
        use smithay::wayland::compositor::with_states;
        use smithay::wayland::shell::xdg::SurfaceCachedState;
        if let Some(top) = window.toplevel() {
            let (min, max) = with_states(top.wl_surface(), |s| {
                let mut c = s.cached_state.get::<SurfaceCachedState>();
                let c = c.current();
                (c.min_size, c.max_size)
            });
            return min.w > 0 && min.h > 0 && min == max;
        }
        if let Some(x) = window.x11_surface() {
            return x.min_size().is_some() && x.min_size() == x.max_size();
        }
        false
    }

    /// May this window be tiled (it is on the screen, and nothing about it says it floats)?
    pub(crate) fn tileable(&self, w: &Window) -> bool {
        w.alive()
            && self.space.elements().any(|e| e == w)
            && !self.is_override_redirect(w)
            && !self.is_fullscreen(w)
            && !self.is_maximized(w)
            && self.minimize_progress(w).is_none()
            && !self.minimized.contains(w)
            && !crate::windows::info(w).lock().unwrap().floating
            && self.parent_window(w).is_none()
            && !self.fixed_size(w)
    }

    /// The windows being tiled on this workspace, in the order of the layout.
    pub fn tiled_now(&self) -> Vec<Window> {
        self.tile_order.iter().filter(|w| self.tileable(w)).cloned().collect()
    }

    /// Is `window` one of the tiled ones?
    pub fn is_tiled(&self, window: &Window) -> bool {
        self.tiling_on() && self.tile_applied.get(&self.workspace).is_some_and(|v| v.iter().any(|(w, _)| w == window))
    }

    /// Alt+T: this workspace goes into tiling mode, or out of it (every window back where it was).
    pub fn toggle_tiling(&mut self) {
        if self.tiling_on() {
            self.tiling.entry(self.workspace).or_default().on = false;
            self.tile_applied.remove(&self.workspace);
            self.restore_before_tiling();
            self.say("Tiling: off".into());
        } else {
            self.begin_tiling();
            self.tiling.entry(self.workspace).or_default().on = true;
            // the order starts as the stacking order: the front window is the master
            let front_first: Vec<Window> = self.space.elements().rev().filter(|w| self.tileable(w)).cloned().collect();
            self.tile_order.retain(|w| !front_first.contains(w));
            for (i, w) in front_first.into_iter().enumerate() {
                self.tile_order.insert(i, w);
            }
            self.retile();
            let name = self.tile_state().layout.name();
            self.say(format!("Tiling: {name}"));
        }
        self.request_redraw();
    }

    /// Remember where the windows are, so that leaving the mode can put them back.
    fn begin_tiling(&mut self) {
        let mut before = Vec::new();
        let windows: Vec<Window> = self.space.elements().rev().filter(|w| !self.is_override_redirect(w) && !self.is_fullscreen(w) && self.minimize_progress(w).is_none()).cloned().collect();
        for w in windows {
            let maximized = self.is_maximized(&w);
            let snapped = self.snapped_rect(&w);
            let Some(now) = self.space.element_geometry(&w) else { continue };
            let back_to = if maximized { self.restore_rect(&w).unwrap_or(now) } else { now };
            before.push((w.clone(), back_to, maximized, snapped));
            {
                let mut i = crate::windows::info(&w).lock().unwrap();
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
        }
        self.tiled_before = Some(before);
    }

    /// Put the windows that were there when the mode began back (those still there).
    fn restore_before_tiling(&mut self) {
        let order: Vec<Window> = self.space.elements().cloned().collect();
        if let Some(before) = self.tiled_before.take() {
            for (w, rect, maximized, snapped) in before {
                if !self.space.elements().any(|e| *e == w) {
                    continue;
                }
                self.place_client(&w, rect);
                if maximized {
                    self.set_maximized(&w, true);
                    crate::windows::info(&w).lock().unwrap().restore = Some(rect);
                }
                // a snapped window is back in its half, and still knows its place before that
                crate::windows::info(&w).lock().unwrap().snapped = snapped;
            }
        }
        // moving a window brings it forward: the order front to back is put back at the end
        for w in &order {
            if self.space.elements().any(|e| e == w) {
                self.space.raise_element(w, false);
            }
        }
        self.raise_on_top_windows();
        self.request_redraw();
    }

    /// Keep the tiled workspace laid out: called every frame, and only does something when the
    /// windows, the layout or the room for them changed.
    pub fn retile(&mut self) {
        if self.locked || self.greeter || self.drag.is_some() {
            return;
        }
        let ws = self.workspace;
        // a workspace the mode was never set for follows `shell.tiling_default`
        if !self.tiling.contains_key(&ws) && self.config.shell.tiling_default {
            self.tiling.insert(ws, TileState { on: true, ..TileState::default() });
        }
        let state = self.tile_state();
        if !state.on {
            return;
        }
        self.tile_order.retain(|w| w.alive());
        // a window that is new to the mode is the master
        let fresh: Vec<Window> = self.space.elements().rev().filter(|w| self.tileable(w) && !self.tile_order.contains(w)).cloned().collect();
        for (i, w) in fresh.into_iter().enumerate() {
            self.tile_order.insert(i, w);
        }
        let mut wanted: Vec<(Window, Rectangle<i32, Logical>)> = Vec::new();
        for out in self.all_outputs() {
            let on_it: Vec<Window> = self.tile_order.iter().filter(|w| self.tileable(w) && self.output_for_window(w) == out).cloned().collect();
            let area = self.usable_area(&out);
            for (w, cell) in on_it.iter().zip(cells(&state, area, on_it.len())) {
                wanted.push((w.clone(), cell));
            }
        }
        if self.tile_applied.get(&ws).is_some_and(|applied| *applied == wanted) {
            return;
        }
        // placing windows brings them forward: the stacking order is put back at the end
        let order: Vec<Window> = self.space.elements().cloned().collect();
        for (w, cell) in &wanted {
            {
                let mut i = crate::windows::info(w).lock().unwrap();
                i.restore = None;
                i.snapped = None;
            }
            let rect = crate::layout::client_rect(*cell, &self.frame_style(w));
            self.place_client(w, rect);
        }
        for w in &order {
            if self.space.elements().any(|e| e == w) {
                self.space.raise_element(w, false);
            }
        }
        self.raise_on_top_windows();
        self.tile_applied.insert(ws, wanted);
        self.request_redraw();
    }

    /// Alt+J / Alt+K: focus the next or previous tiled window.
    pub fn tile_focus(&mut self, delta: i32) {
        if !self.tiling_on() {
            return;
        }
        let list = self.tiled_now();
        if list.len() < 2 {
            return;
        }
        let current = self.space.elements().last().and_then(|f| list.iter().position(|w| w == f)).unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(list.len() as i32) as usize;
        let w = list[next].clone();
        self.focus_window(&w);
    }

    /// Alt+Shift+J / K: the focused window changes places with the next or previous one.
    pub fn tile_swap(&mut self, delta: i32) {
        if !self.tiling_on() {
            return;
        }
        let list = self.tiled_now();
        let Some(f) = self.space.elements().last().cloned() else { return };
        let Some(i) = list.iter().position(|w| *w == f) else { return };
        let j = (i as i32 + delta).rem_euclid(list.len() as i32) as usize;
        self.swap_in_order(&list[i].clone(), &list[j].clone());
    }

    /// Alt+Z: the focused window becomes the master (if it is, it changes with the second one).
    pub fn tile_promote(&mut self) {
        if !self.tiling_on() {
            return;
        }
        let list = self.tiled_now();
        let Some(f) = self.space.elements().last().cloned() else { return };
        let Some(i) = list.iter().position(|w| *w == f) else { return };
        let other = if i == 0 { list.get(1).cloned() } else { Some(list[0].clone()) };
        if let Some(o) = other {
            self.swap_in_order(&f, &o);
        }
    }

    fn swap_in_order(&mut self, a: &Window, b: &Window) {
        let (Some(i), Some(j)) = (self.tile_order.iter().position(|w| w == a), self.tile_order.iter().position(|w| w == b)) else { return };
        self.tile_order.swap(i, j);
        self.request_redraw();
    }

    /// Alt+Shift+Left / Right: the master column wider or narrower.
    pub fn tile_ratio(&mut self, delta: f64) {
        if !self.tiling_on() {
            return;
        }
        let st = self.tiling.entry(self.workspace).or_default();
        st.ratio = (st.ratio + delta).clamp(MIN_RATIO, MAX_RATIO);
        let pct = (st.ratio * 100.0).round() as i32;
        self.say(format!("Master width: {pct} %"));
    }

    /// Alt+Shift+Up / Down: one more or fewer master window.
    pub fn tile_masters(&mut self, delta: i32) {
        if !self.tiling_on() {
            return;
        }
        let st = self.tiling.entry(self.workspace).or_default();
        st.masters = (st.masters as i32 + delta).clamp(1, 8) as usize;
        let m = st.masters;
        self.say(format!("Master windows: {m}"));
    }

    /// Alt+Y: the next layout.
    pub fn tile_next_layout(&mut self) {
        if !self.tiling_on() {
            return;
        }
        let st = self.tiling.entry(self.workspace).or_default();
        st.layout = st.layout.next();
        let name = st.layout.name();
        self.say(format!("Layout: {name}"));
    }

    /// Alt+Shift+F: the focused window floats where it can be moved and sized freely, or goes back
    /// into the layout.
    pub fn tile_toggle_float(&mut self) {
        let Some(w) = self.space.elements().last().cloned() else { return };
        let floating = {
            let mut i = crate::windows::info(&w).lock().unwrap();
            i.floating = !i.floating;
            i.floating
        };
        if floating && self.tiling_on() {
            // it comes out of the layout as a window of its own in the middle of the monitor
            let area = self.usable_area(&self.output_for_window(&w));
            let (cw, ch) = (area.size.w * 7 / 10, area.size.h * 7 / 10);
            let outer = Rectangle::new((area.loc.x + (area.size.w - cw) / 2, area.loc.y + (area.size.h - ch) / 2).into(), (cw, ch).into());
            let rect = crate::layout::client_rect(outer, &self.frame_style(&w));
            self.place_client(&w, rect);
            self.space.raise_element(&w, true);
        }
        self.say(if floating { "Window floats".into() } else { "Window is tiled".into() });
        self.tile_applied.remove(&self.workspace);
        self.request_redraw();
    }

    /// A tiled window was let go after being dragged: onto another one it changes places with it,
    /// elsewhere it goes back to its cell.
    pub fn tile_drop(&mut self, window: &Window) {
        if !self.is_tiled(window) {
            return;
        }
        let ws = self.workspace;
        let p = self.pointer_location;
        let target = self
            .tile_applied
            .get(&ws)
            .and_then(|v| v.iter().find(|(w, cell)| w != window && cell.to_f64().contains(p)).map(|(w, _)| w.clone()));
        if let Some(t) = target {
            self.swap_in_order(window, &t);
        }
        self.tile_applied.remove(&ws);
        self.request_redraw();
    }

    /// A tiled window was resized by its edge: if it is a master (or a stack window) of the master
    /// and stack layout, the master column takes the width it was given.
    pub fn tile_after_resize(&mut self, window: &Window) {
        if !self.is_tiled(window) {
            return;
        }
        let ws = self.workspace;
        let st = self.tile_state();
        if st.layout == Layout::MasterStack {
            let list = self.tiled_now();
            let n = list.len();
            if let (Some(i), Some(outer)) = (list.iter().position(|w| w == window), self.outer_of(window)) {
                if n > st.masters.clamp(1, n.max(1)) {
                    let area = self.usable_area(&self.output_for_window(window));
                    let master_w = if i < st.masters { outer.size.w } else { area.size.w - 3 * GAP - outer.size.w };
                    self.tiling.entry(ws).or_default().ratio = ratio_for_master_width(master_w, area.size.w);
                }
            }
        }
        self.tile_applied.remove(&ws);
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

    #[test]
    fn master_and_stack_fills_the_area_in_two_columns() {
        let area = Rectangle::new((0, 30).into(), (1000, 700).into());
        for n in 1..=9 {
            for masters in 1..=3 {
                let cells = master_stack(area, n, masters, 0.55);
                assert_eq!(cells.len(), n);
                for (i, a) in cells.iter().enumerate() {
                    assert!(area.contains_rect(*a), "n={n} m={masters}: {a:?} is inside");
                    for b in &cells[i + 1..] {
                        assert!(!a.overlaps(*b), "n={n} m={masters}: {a:?} {b:?}");
                    }
                }
                // the whole height is used in each column
                let bottom = cells.iter().map(|c| c.loc.y + c.size.h).max().unwrap();
                assert_eq!(bottom, area.loc.y + area.size.h - GAP, "n={n} m={masters}: no gap at the bottom");
            }
        }
        // one window fills; two are side by side with the master wider; the master is on the left
        assert_eq!(master_stack(area, 1, 1, 0.55)[0], Rectangle::new((GAP, 30 + GAP).into(), (1000 - 2 * GAP, 700 - 2 * GAP).into()));
        let two = master_stack(area, 2, 1, 0.6);
        assert!(two[0].loc.x < two[1].loc.x && two[0].size.w > two[1].size.w);
        assert_eq!(two[0].size.h, two[1].size.h);
        // three windows, one master: the stack has two, one above the other
        let three = master_stack(area, 3, 1, 0.5);
        assert_eq!(three[0].size.h, 700 - 2 * GAP);
        assert!(three[1].loc.y < three[2].loc.y && three[1].loc.x == three[2].loc.x);
        // more masters than windows: they share the one column
        let all_masters = master_stack(area, 2, 5, 0.5);
        assert_eq!(all_masters[0].size.w, all_masters[1].size.w);
        assert!(all_masters[0].loc.y < all_masters[1].loc.y);
        assert!(master_stack(area, 0, 1, 0.5).is_empty());
    }

    #[test]
    fn the_ratio_moves_the_divider_within_limits() {
        let area = Rectangle::new((0, 0).into(), (1000, 700).into());
        let narrow = master_stack(area, 2, 1, 0.3)[0].size.w;
        let wide = master_stack(area, 2, 1, 0.7)[0].size.w;
        assert!(narrow < wide);
        assert_eq!(master_stack(area, 2, 1, 0.0)[0].size.w, master_stack(area, 2, 1, MIN_RATIO)[0].size.w, "clamped");
        assert_eq!(master_stack(area, 2, 1, 5.0)[0].size.w, master_stack(area, 2, 1, MAX_RATIO)[0].size.w);
        // the ratio of a width is the inverse of the layout
        let r = ratio_for_master_width(wide, 1000);
        assert!((r - 0.7).abs() < 0.01, "{r}");
        assert_eq!(ratio_for_master_width(1, 1000), MIN_RATIO);
    }

    #[test]
    fn monocle_gives_every_window_the_whole_area_and_layouts_cycle() {
        let area = Rectangle::new((0, 30).into(), (1000, 700).into());
        let m = monocle(area, 3);
        assert!(m.iter().all(|c| *c == m[0]) && m.len() == 3);
        assert_eq!(Layout::MasterStack.next(), Layout::Grid);
        assert_eq!(Layout::Grid.next().next(), Layout::MasterStack);
        let st = TileState { layout: Layout::Monocle, ..TileState::default() };
        assert_eq!(cells(&st, area, 2), monocle(area, 2));
    }
}
