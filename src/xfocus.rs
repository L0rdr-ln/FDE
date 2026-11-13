//! Which X11 window has the keyboard, told to the X server. FDE follows keyboard focus as Wayland
//! surfaces; X11 programs also need the X input focus set, and those that take part in focus
//! (WM_TAKE_FOCUS: Qt, Java, ...) a "take focus" message, or they ignore what is typed. Without
//! this only programs under the pointer got keys (xterm did, FeatherPad did not).
//!
//! FDE keeps its own small connection to XWayland for it. As in the ICCCM (and Smithay's own
//! window manager), the window's WM_HINTS and WM_PROTOCOLS decide: the input focus for windows
//! that accept input, "take focus" only for windows that list it (xterm closes on any other
//! WM_PROTOCOLS message).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, InputFocus};
use x11rb::rust_connection::RustConnection;

pub struct XFocus {
    conn: RustConnection,
    wm_protocols: u32,
    wm_take_focus: u32,
    /// the window that has the X input focus now
    focused: Option<u32>,
}

impl XFocus {
    pub fn connect(display: u32) -> Option<XFocus> {
        let (conn, _) = x11rb::connect(Some(&format!(":{display}"))).ok()?;
        let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS").ok()?.reply().ok()?.atom;
        let wm_take_focus = conn.intern_atom(false, b"WM_TAKE_FOCUS").ok()?.reply().ok()?.atom;
        Some(XFocus { conn, wm_protocols, wm_take_focus, focused: None })
    }

    /// `window` has the keyboard now.
    pub fn focus(&mut self, window: u32) {
        if self.focused == Some(window) {
            return;
        }
        self.focused = Some(window);
        let (accepts_input, takes_focus) = self.input_model(window);
        if accepts_input {
            let _ = self.conn.set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME);
        }
        if takes_focus {
            let msg = ClientMessageEvent::new(32, window, self.wm_protocols, [self.wm_take_focus, x11rb::CURRENT_TIME, 0, 0, 0]);
            let _ = self.conn.send_event(false, window, EventMask::NO_EVENT, msg);
        }
        let _ = self.conn.flush();
    }

    /// Does the window want the input focus set (WM_HINTS input, true when not said), and does
    /// it take part in focus (WM_TAKE_FOCUS in WM_PROTOCOLS)?
    fn input_model(&self, window: u32) -> (bool, bool) {
        let hints = self
            .conn
            .get_property(false, window, AtomEnum::WM_HINTS, AtomEnum::WM_HINTS, 0, 9)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect::<Vec<u32>>()));
        let accepts_input = match hints.as_deref() {
            Some([flags, input, ..]) if flags & 1 != 0 => *input != 0,
            _ => true,
        };
        let takes_focus = self
            .conn
            .get_property(false, window, self.wm_protocols, AtomEnum::ATOM, 0, 32)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|mut v| v.any(|a| a == self.wm_take_focus)))
            .unwrap_or(false);
        (accepts_input, takes_focus)
    }

    /// No X11 window has the keyboard (a Wayland window has it).
    pub fn unfocus(&mut self) {
        if self.focused.take().is_some() {
            let _ = self.conn.set_input_focus(InputFocus::POINTER_ROOT, x11rb::NONE, x11rb::CURRENT_TIME);
            let _ = self.conn.flush();
        }
    }
}
