//! Windows open where they were last closed: the place and size of each program's window,
//! by its app id, kept in `$XDG_STATE_HOME/fde/windows` (`~/.local/state/fde/windows`).
//! `shell.remember_windows = false` turns it off.

use std::collections::HashMap;
use std::path::PathBuf;

use smithay::desktop::Window;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

use crate::state::Fde;

/// At most this many programs are remembered (the least recently closed are forgotten).
const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// the window without its frame, in the shared space
    pub rect: Rectangle<i32, Logical>,
    pub maximized: bool,
}

/// The remembered places, most recently closed last.
#[derive(Debug, Default, Clone)]
pub struct Placements {
    pub entries: Vec<(String, Placement)>,
}

impl Placements {
    pub fn get(&self, app_id: &str) -> Option<Placement> {
        self.entries.iter().find(|(id, _)| id == app_id).map(|(_, p)| *p)
    }

    pub fn put(&mut self, app_id: &str, p: Placement) {
        self.entries.retain(|(id, _)| id != app_id);
        self.entries.push((app_id.to_string(), p));
        if self.entries.len() > MAX_ENTRIES {
            let extra = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..extra);
        }
    }

    /// One program per line: `app_id x y width height maximized`, separated by tabs.
    pub fn parse(text: &str) -> Placements {
        let mut out = Placements::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 6 || f[0].is_empty() {
                continue;
            }
            let n: Vec<i32> = f[1..5].iter().filter_map(|v| v.parse().ok()).collect();
            if n.len() != 4 || n[2] < 1 || n[3] < 1 {
                continue;
            }
            let rect = Rectangle::new((n[0], n[1]).into(), (n[2], n[3]).into());
            out.put(f[0], Placement { rect, maximized: f[5] == "1" });
        }
        out
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for (id, p) in &self.entries {
            let r = p.rect;
            s.push_str(&format!("{id}\t{}\t{}\t{}\t{}\t{}\n", r.loc.x, r.loc.y, r.size.w, r.size.h, p.maximized as u8));
        }
        s
    }
}

pub fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("fde").join("windows"))
}

pub fn load() -> Placements {
    state_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| Placements::parse(&t)).unwrap_or_default()
}

/// The program's name for itself: the Wayland app id or the X11 class.
pub fn app_id(window: &Window) -> Option<String> {
    let id = if let Some(x) = window.x11_surface() {
        x.class()
    } else {
        let top = window.toplevel()?;
        with_states(top.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|d| d.lock().unwrap().app_id.clone())
        })
        .unwrap_or_default()
    };
    // a name with tabs or line breaks would break the file
    (!id.is_empty() && !id.contains(['\t', '\n'])).then_some(id)
}

impl Fde {
    /// A window is about to close: remember where it was (its normal place if it is maximized
    /// or snapped; nothing if it is full screen or not on the screen).
    pub fn remember_placement(&mut self, window: &Window) {
        if !self.config.shell.remember_windows || self.is_fullscreen(window) {
            return;
        }
        let Some(id) = app_id(window) else { return };
        let Some(current) = self.space.element_geometry(window) else { return };
        let maximized = self.is_maximized(window);
        let rect = if maximized {
            self.restore_rect(window).unwrap_or(current)
        } else {
            self.snapped_rect(window).unwrap_or(current)
        };
        if rect.size.w < 1 || rect.size.h < 1 {
            return;
        }
        self.placements.put(&id, Placement { rect, maximized });
        if let Some(f) = state_file() {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = f.with_extension("tmp");
            if std::fs::write(&tmp, self.placements.to_text()).is_ok() {
                let _ = std::fs::rename(&tmp, &f);
            }
        }
    }

    /// A window is about to be shown for the first time: put it where this program's
    /// window was last, if that is still on a monitor. True if it was placed.
    pub fn restore_placement(&mut self, window: &Window) -> bool {
        if !self.config.shell.remember_windows {
            return false;
        }
        let Some(id) = app_id(window) else { return false };
        let Some(p) = self.placements.get(&id) else { return false };
        // still on a monitor (one that may have been unplugged since does not count), with the
        // title bar reachable
        let on_screen = self.all_outputs().iter().any(|o| {
            let area = self.usable_area(o);
            area.contains(p.rect.loc) && p.rect.size.w <= area.size.w && p.rect.size.h <= area.size.h
        });
        if !on_screen {
            return false;
        }
        if let Some(top) = window.toplevel() {
            top.with_pending_state(|s| s.size = Some(p.rect.size));
        }
        if let Some(x) = window.x11_surface() {
            let _ = x.configure(Some(p.rect));
        }
        self.space.map_element(window.clone(), p.rect.loc, false);
        if p.maximized {
            self.set_maximized(window, true);
            crate::windows::info(window).lock().unwrap().restore = Some(p.rect);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_survive_the_file_and_bad_lines_are_skipped() {
        let mut p = Placements::default();
        p.put("foot", Placement { rect: Rectangle::new((10, 20).into(), (640, 480).into()), maximized: false });
        p.put("org.fde.Settings", Placement { rect: Rectangle::new((0, 0).into(), (900, 600).into()), maximized: true });
        let text = p.to_text() + "broken line\nfoot\tx\t1\t2\t3\t0\nzero\t1\t1\t0\t5\t0\n";
        let q = Placements::parse(&text);
        assert_eq!(q.entries.len(), 2);
        assert_eq!(q.get("foot"), p.get("foot"));
        assert!(q.get("org.fde.Settings").unwrap().maximized);
        // putting again moves it to the end (most recent)
        let mut q = q;
        q.put("foot", Placement { rect: Rectangle::new((1, 1).into(), (2, 2).into()), maximized: false });
        assert_eq!(q.entries.last().unwrap().0, "foot");
    }

    #[test]
    fn only_so_many_programs_are_remembered() {
        let mut p = Placements::default();
        for i in 0..(MAX_ENTRIES + 5) {
            p.put(&format!("app{i}"), Placement { rect: Rectangle::new((0, 0).into(), (1, 1).into()), maximized: false });
        }
        assert_eq!(p.entries.len(), MAX_ENTRIES);
        assert!(p.get("app0").is_none() && p.get(&format!("app{}", MAX_ENTRIES + 4)).is_some());
    }
}
