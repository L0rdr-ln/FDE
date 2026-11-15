//! The window menu: right click on a title bar (FDE's own or a program's, which asks for it), or
//! Alt+Space. Minimize, maximize, always on top, send to another workspace, close.

use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Rectangle};

use crate::state::Fde;

/// Size of one row and the menu's width, in logical pixels.
pub const ROW_H: i32 = 28;
pub const WIDTH: i32 = 230;
pub const PAD: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Minimize,
    Maximize,
    Restore,
    OnTop(bool),
    /// pin the window's program to the task bar (true: it is pinned now, so this unpins)
    Pin(bool),
    ToWorkspace(usize),
    Close,
}

impl MenuItem {
    pub fn label(&self) -> String {
        match self {
            MenuItem::Minimize => "Minimize".into(),
            MenuItem::Maximize => "Maximize".into(),
            MenuItem::Restore => "Restore".into(),
            MenuItem::OnTop(true) => "✓ Always on top".into(),
            MenuItem::OnTop(false) => "Always on top".into(),
            MenuItem::Pin(false) => "Pin to task bar".into(),
            MenuItem::Pin(true) => "Unpin from task bar".into(),
            MenuItem::ToWorkspace(n) => format!("Move to workspace {}", n + 1),
            MenuItem::Close => "Close".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WindowMenu {
    pub window: Window,
    /// top left corner, in the shared space
    pub at: Point<i32, Logical>,
    pub items: Vec<MenuItem>,
    /// the desktop file name of the window's program, for Pin
    pub pin_id: Option<String>,
    /// the row under the pointer or chosen with the arrow keys
    pub hover: Option<usize>,
}

/// The menu's outline with its corner at `at` and `rows` rows.
pub fn menu_rect(at: Point<i32, Logical>, rows: usize) -> Rectangle<i32, Logical> {
    Rectangle::new(at, (WIDTH, rows as i32 * ROW_H + 2 * PAD).into())
}

/// The row at `p` of a menu at `at` with `rows` rows, if `p` is on one.
pub fn row_at(at: Point<i32, Logical>, rows: usize, p: Point<f64, Logical>) -> Option<usize> {
    let r = menu_rect(at, rows).to_f64();
    if !r.contains(p) {
        return None;
    }
    let i = ((p.y - r.loc.y - PAD as f64) / ROW_H as f64).floor();
    (i >= 0.0 && (i as usize) < rows).then_some(i as usize)
}

impl WindowMenu {
    pub fn rect(&self) -> Rectangle<i32, Logical> {
        menu_rect(self.at, self.items.len())
    }

    pub fn row_at(&self, p: Point<f64, Logical>) -> Option<usize> {
        row_at(self.at, self.items.len(), p)
    }
}

impl Fde {
    /// What the menu offers for `window` now.
    pub fn window_menu_items(&self, window: &Window) -> Vec<MenuItem> {
        let mut items = vec![MenuItem::Minimize];
        items.push(if self.is_maximized(window) { MenuItem::Restore } else { MenuItem::Maximize });
        items.push(MenuItem::OnTop(self.is_on_top(window)));
        if let Some((_, pinned)) = self.pin_target(window) {
            items.push(MenuItem::Pin(pinned));
        }
        let here = self.window_workspace_of(window);
        for n in 0..self.workspace_count() {
            if n != here {
                items.push(MenuItem::ToWorkspace(n));
            }
        }
        items.push(MenuItem::Close);
        items
    }

    /// Open the menu for `window` with its corner at `p` (kept on the monitor).
    pub fn open_window_menu(&mut self, window: &Window, p: Point<f64, Logical>) {
        if self.locked {
            return;
        }
        let items = self.window_menu_items(window);
        let pin_id = self.pin_target(window).map(|(id, _)| id);
        let mut menu = WindowMenu { window: window.clone(), at: p.to_i32_round(), items, pin_id, hover: None };
        if let Some(out) = self.output_at(p) {
            let r = self.output_rect(&out);
            let size = menu.rect().size;
            menu.at.x = menu.at.x.min(r.loc.x + r.size.w - size.w).max(r.loc.x);
            menu.at.y = menu.at.y.min(r.loc.y + r.size.h - size.h).max(r.loc.y);
        }
        self.window_menu = Some(menu);
        self.request_redraw();
    }

    pub fn close_window_menu(&mut self) {
        if self.window_menu.take().is_some() {
            self.request_redraw();
        }
    }

    /// Do what row `i` of the open menu says, and close the menu.
    pub fn choose_window_menu(&mut self, i: usize) {
        let Some(menu) = self.window_menu.take() else { return };
        self.request_redraw();
        let Some(item) = menu.items.get(i).copied() else { return };
        if let (MenuItem::Pin(was), Some(id)) = (item, menu.pin_id.as_deref()) {
            self.set_pinned(id, !was);
            return;
        }
        let w = menu.window;
        if !self.space.elements().any(|e| *e == w) {
            return; // gone meanwhile
        }
        match item {
            MenuItem::Minimize => self.minimize_window(&w),
            MenuItem::Maximize => self.set_maximized(&w, true),
            MenuItem::Restore => self.set_maximized(&w, false),
            MenuItem::OnTop(on) => self.set_on_top(&w, !on),
            MenuItem::Pin(_) => {}
            MenuItem::ToWorkspace(n) => self.move_window_to_workspace(&w, n),
            MenuItem::Close => self.close_window(&w),
        }
    }

    /// The pointer moved: light the row under it. True if the menu took the motion.
    pub fn window_menu_motion(&mut self, p: Point<f64, Logical>) -> bool {
        let Some(menu) = self.window_menu.as_mut() else { return false };
        let row = menu.row_at(p);
        if row != menu.hover {
            menu.hover = row;
            self.request_redraw();
        }
        true
    }

    /// A button went down while the menu is open: a row is chosen, anywhere else closes it.
    pub fn window_menu_click(&mut self, p: Point<f64, Logical>) {
        let row = self.window_menu.as_ref().and_then(|m| m.row_at(p));
        match row {
            Some(i) => self.choose_window_menu(i),
            None => self.close_window_menu(),
        }
    }

    /// Up/Down move the light, Enter chooses, Escape closes.
    pub fn window_menu_key(&mut self, step: i32, enter: bool, escape: bool) {
        if escape {
            self.close_window_menu();
            return;
        }
        let Some(menu) = self.window_menu.as_mut() else { return };
        if enter {
            if let Some(i) = menu.hover {
                self.choose_window_menu(i);
            }
            return;
        }
        let n = menu.items.len() as i32;
        let cur = menu.hover.map(|h| h as i32).unwrap_or(if step > 0 { -1 } else { n });
        menu.hover = Some((cur + step).rem_euclid(n) as usize);
        self.request_redraw();
    }

    /// The desktop file name of the window's program, and whether it is pinned to the task bar now
    /// (None when it is not an installed program).
    pub fn pin_target(&self, window: &Window) -> Option<(String, bool)> {
        let app_id = crate::placement::app_id(window)?;
        let apps = crate::apps::installed();
        let i = crate::apps::index_for_window(&apps, &app_id)?;
        let pinned = self.config.shell.pinned.iter().any(|n| crate::apps::index_for_pinned(&apps, n) == Some(i));
        Some((apps[i].id.clone(), pinned))
    }

    /// Pin a program to the task bar, or take it off, in the user's configuration file (the task
    /// bar looks at it every couple of seconds).
    pub fn set_pinned(&mut self, id: &str, pin: bool) {
        let apps = crate::apps::installed();
        let target = crate::apps::index_for_pinned(&apps, id);
        let mut list: Vec<String> = self.config.shell.pinned.clone();
        list.retain(|n| crate::apps::index_for_pinned(&apps, n) != target || target.is_none() && n != id);
        if pin {
            list.push(id.to_string());
        }
        let Some(path) = crate::config::user_path() else { return };
        match crate::config::set(&path, &[("shell.pinned", crate::config::Setting::List(list.clone()))]) {
            Ok(()) => self.config.shell.pinned = list,
            Err(e) => tracing::warn!("cannot pin: {e}"),
        }
        self.request_redraw();
    }

    pub fn is_on_top(&self, window: &Window) -> bool {
        crate::windows::info(window).lock().unwrap().on_top
    }

    /// Keep `window` above the others (or not).
    pub fn set_on_top(&mut self, window: &Window, on: bool) {
        crate::windows::info(window).lock().unwrap().on_top = on;
        self.raise_on_top_windows();
    }

    /// Windows that are always on top go back above the rest (after another one came forward).
    pub fn raise_on_top_windows(&mut self) {
        let tops: Vec<Window> = self.space.elements().filter(|w| self.is_on_top(w)).cloned().collect();
        for w in tops {
            self.space.raise_element(&w, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_found_by_height() {
        let at = (100, 100).into();
        assert_eq!(row_at(at, 3, (110.0, 100.0 + PAD as f64 + 1.0).into()), Some(0));
        assert_eq!(row_at(at, 3, (110.0, 100.0 + PAD as f64 + ROW_H as f64 * 2.5).into()), Some(2));
        assert_eq!(row_at(at, 3, (99.0, 120.0).into()), None);
        assert_eq!(row_at(at, 3, (110.0, 101.0).into()), None, "the padding above the first row");
        assert_eq!(MenuItem::ToWorkspace(2).label(), "Move to workspace 3");
    }
}
