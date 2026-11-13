//! Draw only when something changed. A picture is needed again when a program committed, input
//! arrived, a monitor or the configuration changed (`request_redraw`), while an animation runs
//! (`animating`), and at least every half second (the clock of night light, safety net).

use std::time::{Duration, Instant};

use smithay::reexports::wayland_server::Resource;
use smithay::utils::IsAlive;

use crate::state::Fde;

/// The longest a monitor goes without being drawn.
pub const IDLE_REDRAW: Duration = Duration::from_millis(500);

impl Fde {
    /// Tell the locking program the screen is locked, once every monitor shows its lock surface
    /// (called after a locked frame was drawn).
    pub fn confirm_lock(&mut self) {
        if self.lock_confirmation.is_some() && self.all_outputs().iter().all(|o| self.lock_surfaces.iter().any(|(lo, _)| lo == o)) {
            if let Some(confirmation) = self.lock_confirmation.take() {
                confirmation.lock();
            }
        }
    }

    /// An unlock was asked for: it counts only if the lock object of the program that locked is
    /// gone now (its unlock destroys it). Any other request (a program that never held the lock)
    /// leaves the screen locked.
    /// After the programs' requests of a round are handled: forget closed menus (which also ends
    /// a menu's hold on the keyboard and pointer), and finish an unlock.
    pub fn after_dispatch(&mut self) {
        self.popups.cleanup();
        self.end_finished_popup_grab();
        self.finish_unlock();
    }

    /// The program's menu is gone (closed by a choice, by Escape, or by the program): give the
    /// keyboard and pointer back at once. Smithay's grab would only notice at the next key press,
    /// and until then the program's next menu would be refused and close straight away.
    pub fn end_finished_popup_grab(&mut self) {
        let ended = self.popup_grab.as_ref().is_some_and(|g| g.has_ended());
        if !ended {
            return;
        }
        // the window the menu belonged to (once the menu is gone, the grab names it)
        let root = self.popup_grab.take().and_then(|g| g.current_grab());
        if let Some(kb) = self.seat.get_keyboard() {
            kb.unset_grab(self);
            // the keyboard goes back to that window, unless it went elsewhere meanwhile
            let on_dead_or_menu = kb.current_focus().is_none_or(|f| !f.alive() || f.client() == root.as_ref().and_then(|r| r.client()));
            if on_dead_or_menu && root.as_ref().is_some_and(|r| r.alive()) && !self.locked {
                kb.set_focus(self, root, smithay::utils::SERIAL_COUNTER.next_serial());
            }
        }
        if let Some(ptr) = self.seat.get_pointer() {
            let time = self.start_time.elapsed().as_millis() as u32;
            ptr.unset_grab(self, smithay::utils::SERIAL_COUNTER.next_serial(), time);
        }
    }

    pub fn finish_unlock(&mut self) {
        if !self.unlock_pending {
            return;
        }
        self.unlock_pending = false;
        use smithay::reexports::wayland_server::Resource;
        if self.lock_owner.as_ref().map(|o| o.is_alive()).unwrap_or(false) {
            tracing::warn!("an unlock from a program that did not lock the session: refused");
            return;
        }
        self.lock_owner = None;
        self.locked = false;
        self.lock_surfaces.clear();
        self.lock_confirmation = None;
        self.focus_top_window();
        self.request_redraw();
        tracing::info!("session unlocked");
    }

    /// Fast mode: one blur pass, no drop shadows, no wobble. Chosen with `appearance.performance`.
    pub fn fast_mode(&self) -> bool {
        match self.config.appearance.performance.as_str() {
            "fast" => true,
            "quality" => false,
            _ => self.software_renderer,
        }
    }

    /// Look at which graphics driver is drawing: software ones ("llvmpipe", "softpipe", "swrast")
    /// are slow with blur, so `auto` turns fast mode on.
    pub fn detect_renderer(&mut self, renderer: &mut smithay::backend::renderer::gles::GlesRenderer) {
        let name = renderer
            .with_context(|gl| unsafe {
                let p = gl.GetString(smithay::backend::renderer::gles::ffi::RENDERER);
                if p.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(p as *const _).to_string_lossy().to_lowercase()
                }
            })
            .unwrap_or_default();
        self.software_renderer = ["llvmpipe", "softpipe", "swrast", "software"].iter().any(|k| name.contains(k));
        tracing::info!("graphics: {name}{}", if self.software_renderer { " (software: fast mode when appearance.performance is auto)" } else { "" });
    }

    /// Something on screen may have changed.
    pub fn request_redraw(&mut self) {
        self.redraw_serial = self.redraw_serial.wrapping_add(1);
    }

    /// Is something moving that needs a new picture every frame?
    pub fn animating(&self) -> bool {
        !self.ghosts.is_empty()
            || !self.wobbles.is_empty()
            || self.hud.is_some()
            || self.space.elements().any(|w| self.open_progress(w) < 1.0 || self.minimize_progress(w).is_some())
    }

    /// Does a monitor that last drew `drawn` (see `redraw_serial`) at `last` need a new picture?
    pub fn needs_redraw(&self, drawn: u64, last: Instant) -> bool {
        drawn != self.redraw_serial || last.elapsed() >= IDLE_REDRAW || self.animating()
    }
}

/// How long frames take, logged every five seconds (`RUST_LOG=info`): the first thing to look at
/// when the desktop feels slow.
pub struct FrameStats {
    since: Instant,
    frames: u32,
    render: Duration,
    present: Duration,
    worst: Duration,
}

impl Default for FrameStats {
    fn default() -> Self {
        FrameStats { since: Instant::now(), frames: 0, render: Duration::ZERO, present: Duration::ZERO, worst: Duration::ZERO }
    }
}

impl FrameStats {
    /// A frame took `render` to draw and `present` to hand to the screen.
    pub fn add(&mut self, render: Duration, present: Duration) {
        self.frames += 1;
        self.render += render;
        self.present += present;
        self.worst = self.worst.max(render + present);
        if self.since.elapsed() >= Duration::from_secs(5) {
            let n = self.frames.max(1) as f64;
            tracing::info!(
                "frames: {} in {:.1}s, drawing {:.1} ms and handing over {:.1} ms on average, slowest {:.0} ms",
                self.frames,
                self.since.elapsed().as_secs_f64(),
                self.render.as_secs_f64() * 1000.0 / n,
                self.present.as_secs_f64() * 1000.0 / n,
                self.worst.as_secs_f64() * 1000.0
            );
            *self = FrameStats::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_desktop_is_drawn_only_when_asked_or_every_half_second() {
        let display = smithay::reexports::wayland_server::Display::<Fde>::new().unwrap();
        let mut s = Fde::new(&display.handle(), (800, 600).into());
        let (mut drawn, mut last) = (s.redraw_serial, Instant::now());
        assert!(!s.needs_redraw(drawn, last), "nothing happened: no new picture");
        s.request_redraw();
        assert!(s.needs_redraw(drawn, last), "something changed");
        drawn = s.redraw_serial;
        last = Instant::now();
        assert!(!s.needs_redraw(drawn, last));
        assert!(s.needs_redraw(drawn, Instant::now() - Duration::from_millis(600)), "the safety net");
        s.hud = Some((Instant::now(), "x".into()));
        assert!(s.needs_redraw(drawn, last), "an announcement is animating");
    }
}
