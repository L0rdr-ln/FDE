//! Touchpad gestures. A three-finger swipe left or right goes to the next or previous workspace
//! (the fingers move the workspaces, as on other desktops: left brings the one on the right).
//! Every other gesture (pinch to zoom, two- and four-finger swipes, holds) goes to the program
//! under the pointer through zwp_pointer_gestures_v1.

use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
    GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
};
use smithay::utils::SERIAL_COUNTER;

use crate::state::Fde;

/// How far (touchpad units, about pixels) the fingers must travel sideways for a workspace switch.
pub const SWITCH_DISTANCE: f64 = 100.0;

/// A three-finger swipe that FDE keeps for itself, and how far it went.
#[derive(Debug, Clone, Copy, Default)]
pub struct Swipe {
    pub dx: f64,
    pub dy: f64,
}

/// What a finished workspace swipe does: -1 (previous), +1 (next) or nothing.
pub fn swipe_step(s: Swipe) -> i32 {
    if s.dx.abs() < SWITCH_DISTANCE || s.dx.abs() < s.dy.abs() {
        0
    } else if s.dx < 0.0 {
        1
    } else {
        -1
    }
}

impl Fde {
    pub fn swipe_begin(&mut self, fingers: u32, time: u32) {
        if fingers == 3 && !self.locked {
            self.swipe = Some(Swipe::default());
            return;
        }
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_swipe_begin(self, &GestureSwipeBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers });
        }
    }

    pub fn swipe_update(&mut self, dx: f64, dy: f64, time: u32) {
        if let Some(s) = self.swipe.as_mut() {
            s.dx += dx;
            s.dy += dy;
            return;
        }
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_swipe_update(self, &GestureSwipeUpdateEvent { time, delta: (dx, dy).into() });
        }
    }

    pub fn swipe_end(&mut self, cancelled: bool, time: u32) {
        if let Some(s) = self.swipe.take() {
            let step = swipe_step(s);
            if !cancelled && step != 0 {
                self.switch_workspace_by(step);
            }
            return;
        }
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_swipe_end(self, &GestureSwipeEndEvent { serial: SERIAL_COUNTER.next_serial(), time, cancelled });
        }
    }

    pub fn pinch_begin(&mut self, fingers: u32, time: u32) {
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_pinch_begin(self, &GesturePinchBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers });
        }
    }

    pub fn pinch_update(&mut self, dx: f64, dy: f64, scale: f64, rotation: f64, time: u32) {
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_pinch_update(self, &GesturePinchUpdateEvent { time, delta: (dx, dy).into(), scale, rotation });
        }
    }

    pub fn pinch_end(&mut self, cancelled: bool, time: u32) {
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_pinch_end(self, &GesturePinchEndEvent { serial: SERIAL_COUNTER.next_serial(), time, cancelled });
        }
    }

    pub fn hold_begin(&mut self, fingers: u32, time: u32) {
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_hold_begin(self, &GestureHoldBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers });
        }
    }

    pub fn hold_end(&mut self, cancelled: bool, time: u32) {
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.gesture_hold_end(self, &GestureHoldEndEvent { serial: SERIAL_COUNTER.next_serial(), time, cancelled });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_clear_sideways_swipe_switches() {
        assert_eq!(swipe_step(Swipe { dx: -250.0, dy: 20.0 }), 1, "fingers to the left: the next workspace");
        assert_eq!(swipe_step(Swipe { dx: 250.0, dy: -20.0 }), -1);
        assert_eq!(swipe_step(Swipe { dx: -40.0, dy: 0.0 }), 0, "too short");
        assert_eq!(swipe_step(Swipe { dx: -150.0, dy: 300.0 }), 0, "mostly up or down");
    }
}
