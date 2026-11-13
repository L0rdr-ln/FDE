//! Several keyboard layouts (`input.keyboard_layout = "us,de"`): Super+Space or a click on the
//! task bar's label switches to the next. The compositor writes the one in use to
//! `$XDG_RUNTIME_DIR/fde/layout` for the task bar; the task bar asks for the next one with SIGUSR2.

use smithay::input::keyboard::Layout;

use crate::state::Fde;

static NEXT_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_next_layout(_: libc::c_int) {
    NEXT_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Listen for the task bar's SIGUSR2.
pub fn watch_signal() {
    unsafe { libc::signal(libc::SIGUSR2, on_next_layout as extern "C" fn(libc::c_int) as libc::sighandler_t) };
}

/// Did the task bar ask for the next layout since the last call?
pub fn take_request() -> bool {
    NEXT_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// The short name of layout `index` of the configured list ("us,de(nodeadkeys)" → "US", "DE").
pub fn short_name(config_layouts: &str, index: usize) -> String {
    config_layouts
        .split(',')
        .nth(index)
        .map(|l| l.trim().split('(').next().unwrap_or("").to_uppercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("#{}", index + 1))
}

fn state_file() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("fde").join("layout"))
}

/// What the task bar shows: the layout in use, if there is more than one.
pub fn read_state() -> Option<String> {
    let s = std::fs::read_to_string(state_file()?).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Ask the running compositor for the next layout (from the task bar).
pub fn request_next() {
    if let Some(pid) = std::env::var("FDE_PID").ok().and_then(|p| p.parse::<libc::pid_t>().ok()) {
        unsafe { libc::kill(pid, libc::SIGUSR2) };
    }
}

impl Fde {
    /// How many layouts the keyboard has, and which one is in use.
    pub fn layouts(&mut self) -> (usize, usize) {
        let Some(kb) = self.seat.get_keyboard() else { return (1, 0) };
        kb.with_xkb_state(self, |ctx| {
            let xkb = ctx.xkb().lock().unwrap();
            (xkb.layouts().count(), xkb.active_layout().0 as usize)
        })
    }

    /// Switch to the next layout (back to the first after the last).
    pub fn next_layout(&mut self) {
        let Some(kb) = self.seat.get_keyboard() else { return };
        let (count, current) = self.layouts();
        if count < 2 {
            return;
        }
        let next = (current + 1) % count;
        kb.with_xkb_state(self, |mut ctx| ctx.set_layout(Layout(next as u32)));
        self.write_layout_state();
        self.hud = Some((std::time::Instant::now(), format!("Keyboard: {}", short_name(&self.config.input.keyboard_layout, next))));
        self.request_redraw();
    }

    /// Tell the task bar which layout is in use (nothing to show with a single layout).
    pub fn write_layout_state(&mut self) {
        let Some(f) = state_file() else { return };
        let (count, current) = self.layouts();
        let text = if count > 1 { short_name(&self.config.input.keyboard_layout, current) } else { String::new() };
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(f, text + "\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_come_from_the_configured_list() {
        assert_eq!(short_name("us,de(nodeadkeys)", 0), "US");
        assert_eq!(short_name("us,de(nodeadkeys)", 1), "DE");
        assert_eq!(short_name("us", 3), "#4");
        assert_eq!(short_name("", 0), "#1");
    }
}
