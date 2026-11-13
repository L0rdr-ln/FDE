//! The small glass panels that open above the task bar: a volume slider, the Wi-Fi networks, a
//! calendar, the notification history, and the tooltip with a window's title. Pure drawing and
//! hit testing (premultiplied RGBA, top row first) so they can be tested;
//! `bin/fde-panel.rs` puts them on screen as layer-shell surfaces.

use crate::panel::{blend, blit};
use crate::status::{Network, WifiNet};
use crate::text::TextRenderer;

/// One line of the notification history.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryItem {
    pub app: String,
    pub summary: String,
    pub body: String,
    /// "3 min ago"
    pub age: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Flyout {
    /// the titles of a program's windows
    Tooltip(Vec<String>),
    Volume { percent: u32, muted: bool },
    /// `networks` is None while the list is being fetched
    Network { current: Network, networks: Option<Vec<WifiNet>> },
    /// the month shown, and today's date (year, month, day)
    Calendar { year: i32, month: u32, today: (i32, u32, u32) },
    Notifications { items: Vec<HistoryItem>, dnd: bool },
}

/// What is under a point of a flyout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    None,
    /// the volume slider, at this fraction (0 .. 1) of its length
    Slider(f64),
    Mute,
    Network(usize),
    NetworkSettings,
    CalPrev,
    CalNext,
    CalToday,
    ClearAll,
    Dnd,
}

const PAD: i32 = 14;
const ROW: i32 = 28;
const BTN_H: i32 = 24;
pub const MAX_NETWORKS: usize = 7;
pub const MAX_HISTORY: usize = 6;
const HISTORY_ROW: i32 = 46;
const CAL_CELL: (i32, i32) = (32, 26);

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// The day of the week, 0 = Monday .. 6 = Sunday.
pub fn weekday(year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let sunday_first = (y + y / 4 - y / 100 + y / 400 + T[month as usize - 1] + day as i32).rem_euclid(7) as u32;
    (sunday_first + 6) % 7
}

/// The month before or after (`delta` -1 or 1).
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// Today's date in local time.
pub fn today() -> (i32, u32, u32) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_year + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32)
    }
}

/// "3 min ago" for a note that arrived `secs` seconds ago.
pub fn age_label(secs: u64) -> String {
    match secs {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}

fn shown_rows(f: &Flyout) -> usize {
    match f {
        Flyout::Network { networks, .. } => networks.as_ref().map(|n| n.len().min(MAX_NETWORKS)).unwrap_or(0).max(1),
        Flyout::Notifications { items, .. } => items.len().min(MAX_HISTORY).max(1),
        _ => 0,
    }
}

/// The size of the flyout in logical pixels (`text` measures a tooltip's lines at scale `s`).
pub fn size(f: &Flyout, text: &mut TextRenderer, s: i32) -> (i32, i32) {
    match f {
        Flyout::Tooltip(lines) => {
            let widest = lines.iter().filter_map(|l| text.line(l, 420 * s, 13.0 * s as f32, [0, 0, 0], None, 0)).map(|i| i.width).max().unwrap_or(40 * s);
            (widest / s + 2 * PAD + 10, lines.len().max(1) as i32 * 20 + 14)
        }
        Flyout::Volume { .. } => (250, 96),
        Flyout::Network { .. } => (270, 44 + shown_rows(f) as i32 * ROW + 12 + BTN_H + 12),
        Flyout::Calendar { .. } => (252, 62 + 6 * CAL_CELL.1 + 12),
        Flyout::Notifications { .. } => (330, 44 + shown_rows(f) as i32 * HISTORY_ROW + 12 + BTN_H + 12),
    }
}

type Rect = (i32, i32, i32, i32);

fn inside(r: Rect, x: f64, y: f64) -> bool {
    x >= r.0 as f64 && x < r.2 as f64 && y >= r.1 as f64 && y < r.3 as f64
}

pub fn slider_rect(w: i32) -> Rect {
    (PAD, 44, w - PAD, 58)
}

pub fn mute_rect(w: i32) -> Rect {
    (PAD, 66, w - PAD, 66 + BTN_H)
}

fn footer_rect(w: i32, rows_end: i32) -> Rect {
    (PAD, rows_end + 12, w - PAD, rows_end + 12 + BTN_H)
}

fn network_rows_end(f: &Flyout) -> i32 {
    44 + shown_rows(f) as i32 * ROW
}

fn history_rows_end(f: &Flyout) -> i32 {
    44 + shown_rows(f) as i32 * HISTORY_ROW
}

pub fn cal_prev_rect() -> Rect {
    (10, 8, 40, 36)
}

pub fn cal_next_rect(w: i32) -> Rect {
    (w - 40, 8, w - 10, 36)
}

pub fn dnd_rect(w: i32) -> Rect {
    (w - 98, 10, w - 12, 10 + BTN_H)
}

/// The rectangle of day `d` (1-based) of a month that starts on weekday `first` (0 = Monday).
pub fn cal_day_rect(w: i32, first: u32, d: u32) -> Rect {
    let i = first + d - 1;
    let (col, row) = ((i % 7) as i32, (i / 7) as i32);
    let x0 = (w - 7 * CAL_CELL.0) / 2 + col * CAL_CELL.0;
    let y0 = 62 + row * CAL_CELL.1;
    (x0, y0, x0 + CAL_CELL.0, y0 + CAL_CELL.1)
}

/// What is at (x, y) of the flyout, in logical pixels.
pub fn hit(f: &Flyout, x: f64, y: f64) -> Hit {
    let (w, _) = {
        // the hit test needs only the width, which does not depend on the text
        match f {
            Flyout::Volume { .. } => (250, 0),
            Flyout::Network { .. } => (270, 0),
            Flyout::Calendar { .. } => (252, 0),
            Flyout::Notifications { .. } => (330, 0),
            Flyout::Tooltip(_) => return Hit::None,
        }
    };
    match f {
        Flyout::Volume { .. } => {
            let r = slider_rect(w);
            if inside((r.0 - 6, r.1 - 8, r.2 + 6, r.3 + 8), x, y) {
                return Hit::Slider(((x - r.0 as f64) / (r.2 - r.0) as f64).clamp(0.0, 1.0));
            }
            if inside(mute_rect(w), x, y) {
                return Hit::Mute;
            }
            Hit::None
        }
        Flyout::Network { networks, .. } => {
            let n = shown_rows(f);
            if networks.as_ref().is_some_and(|l| !l.is_empty()) && x >= PAD as f64 && x < (w - PAD) as f64 && y >= 44.0 && y < (44 + n as i32 * ROW) as f64 {
                return Hit::Network(((y - 44.0) / ROW as f64) as usize);
            }
            if inside(footer_rect(w, network_rows_end(f)), x, y) {
                return Hit::NetworkSettings;
            }
            Hit::None
        }
        Flyout::Calendar { .. } => {
            if inside(cal_prev_rect(), x, y) {
                Hit::CalPrev
            } else if inside(cal_next_rect(w), x, y) {
                Hit::CalNext
            } else if inside((44, 8, w - 44, 36), x, y) {
                Hit::CalToday
            } else {
                Hit::None
            }
        }
        Flyout::Notifications { .. } => {
            if inside(dnd_rect(w), x, y) {
                Hit::Dnd
            } else if inside(footer_rect(w, history_rows_end(f)), x, y) {
                Hit::ClearAll
            } else {
                Hit::None
            }
        }
        Flyout::Tooltip(_) => Hit::None,
    }
}

// ---------------------------------------------------------------------------- drawing

/// Coverage (0 .. 1) of a rounded box and how far inside its edge a point is.
fn round_cover(x: f32, y: f32, r: (f32, f32, f32, f32), rad: f32) -> (f32, f32) {
    let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
    let (qx, qy) = ((x - cx).abs() - ((r.2 - r.0) / 2.0 - rad), (y - cy).abs() - ((r.3 - r.1) / 2.0 - rad));
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - rad;
    ((0.5 - d).clamp(0.0, 1.0), -d)
}

/// Fill a rounded box (pixels) with `fill(t)` = (colour, alpha) down its height, a bright rim and
/// a thin shade inside it.
#[allow(clippy::too_many_arguments)]
fn fill_box(buf: &mut [u8], w: i32, h: i32, r: Rect, rad: f32, s: i32, fill: impl Fn(f32) -> ([f32; 3], f32), rim: f32, shade: [f32; 3]) {
    let sf = s as f32;
    let rf = (r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32);
    for y in r.1.max(0)..r.3.min(h) {
        for x in r.0.max(0)..r.2.min(w) {
            let (cover, inner) = round_cover(x as f32 + 0.5, y as f32 + 0.5, rf, rad);
            if cover <= 0.0 {
                continue;
            }
            let t = (y - r.1) as f32 / (r.3 - r.1).max(1) as f32;
            let i = ((y * w + x) * 4) as usize;
            let (c, a) = fill(t);
            blend(&mut buf[i..i + 4], c, a * cover);
            if inner < 1.2 * sf {
                blend(&mut buf[i..i + 4], [1.0, 1.0, 1.0], rim * cover);
            } else if inner < 2.2 * sf {
                blend(&mut buf[i..i + 4], shade, 0.22 * cover);
            }
        }
    }
}

/// The light glass of the task bar and the windows, as a rounded sheet.
fn glass(buf: &mut [u8], w: i32, h: i32, s: i32) {
    let pal = crate::theme::palette();
    let rad = (10.0 - 10.0 * pal.solid) * s as f32;
    fill_box(
        buf,
        w,
        h,
        (0, 0, w, h),
        rad,
        s,
        |t| (pal.light(0.42 + 0.18 * (1.0 - t)), 0.88 + 0.06 * t + 0.06 * pal.solid),
        0.85,
        pal.dark(0.5),
    );
    // the sheen over the top
    for y in 0..(h / 5).max(1) {
        let k = 0.26 * (1.0 - y as f32 / (h / 5).max(1) as f32) * pal.shine.max(0.25);
        for x in 0..w {
            let (cover, _) = round_cover(x as f32 + 0.5, y as f32 + 0.5, (0.0, 0.0, w as f32, h as f32), rad);
            if cover > 0.0 {
                let i = ((y * w + x) * 4) as usize;
                blend(&mut buf[i..i + 4], [1.0, 1.0, 1.0], k * cover);
            }
        }
    }
}

fn scaled(r: Rect, s: i32) -> Rect {
    (r.0 * s, r.1 * s, r.2 * s, r.3 * s)
}

fn tile(buf: &mut [u8], w: i32, h: i32, r: Rect, s: i32, hot: bool) {
    let pal = crate::theme::palette();
    fill_box(
        buf,
        w,
        h,
        scaled(r, s),
        7.0 * s as f32,
        s,
        |t| if hot { (pal.light(0.2), 0.75 - 0.2 * t) } else { ([1.0, 1.0, 1.0], 0.42 - 0.16 * t) },
        0.85,
        pal.dark(0.55),
    );
}

#[allow(clippy::too_many_arguments)]
fn label(text: &mut TextRenderer, buf: &mut [u8], w: i32, h: i32, s: i32, s_text: &str, x: i32, y_mid: i32, px: f32, color: [u8; 3], max_w: i32) -> i32 {
    match text.line(s_text, max_w * s, px * s as f32, color, Some(([255, 255, 255], 0.8)), 2 * s) {
        Some(img) => {
            let (iw, ih) = (img.width, img.height);
            blit(buf, w, h, &img, x * s, y_mid * s - ih / 2);
            iw / s
        }
        None => 0,
    }
}

/// The text of a button, centred in it.
fn button_label(text: &mut TextRenderer, buf: &mut [u8], w: i32, h: i32, s: i32, r: Rect, s_text: &str, color: [u8; 3]) {
    if let Some(img) = text.line(s_text, (r.2 - r.0 - 8) * s, 12.5 * s as f32, color, Some(([255, 255, 255], 0.8)), s) {
        blit(buf, w, h, &img, (r.0 + r.2) * s / 2 - img.width / 2, (r.1 + r.3) * s / 2 - img.height / 2);
    }
}

/// The flyout as a picture of `size(..) * s` pixels. `hover` lights what the pointer is over.
pub fn draw(f: &Flyout, text: &mut TextRenderer, s: i32, hover: Hit) -> Vec<u8> {
    let (lw, lh) = size(f, text, s);
    let (w, h) = (lw * s, lh * s);
    let mut buf = vec![0u8; (w * h * 4).max(0) as usize];
    let pal = crate::theme::palette();
    let ink = pal.ink;
    let soft = [(ink[0] as u32 / 2 + 60).min(255) as u8, (ink[1] as u32 / 2 + 70).min(255) as u8, (ink[2] as u32 / 2 + 80).min(255) as u8];
    glass(&mut buf, w, h, s);
    match f {
        Flyout::Tooltip(lines) => {
            for (i, l) in lines.iter().enumerate() {
                label(text, &mut buf, w, h, s, l, PAD, 17 + i as i32 * 20, 13.0, ink, lw - 2 * PAD);
            }
        }
        Flyout::Volume { percent, muted } => {
            let title = if *muted { "Volume: muted".to_string() } else { format!("Volume: {percent}%") };
            label(text, &mut buf, w, h, s, &title, PAD, 24, 14.0, ink, lw - 2 * PAD);
            // the slider: a groove, the part to the knob in the accent, a round knob
            let r = slider_rect(lw);
            let (x0, x1, cy) = (r.0 as f32 * s as f32, r.2 as f32 * s as f32, (r.1 + r.3) as f32 / 2.0 * s as f32);
            let level = if *muted { 0.0 } else { (*percent as f32 / 100.0).clamp(0.0, 1.0) };
            let knob_x = x0 + (x1 - x0) * level;
            let groove = (x0 as i32, (cy - 4.0 * s as f32) as i32, x1 as i32, (cy + 4.0 * s as f32) as i32);
            fill_box(&mut buf, w, h, groove, 4.0 * s as f32, s, |_| (pal.dark(0.55), 0.35), 0.25, pal.dark(0.7));
            if knob_x > x0 + 2.0 {
                fill_box(&mut buf, w, h, (x0 as i32, groove.1, knob_x as i32, groove.3), 4.0 * s as f32, s, |t| (pal.light(0.1 + 0.2 * (1.0 - t)), 0.95), 0.4, pal.dark(0.7));
            }
            let kr = 8.0 * s as f32;
            for y in (cy - kr - 2.0) as i32..(cy + kr + 2.0) as i32 {
                for x in (knob_x - kr - 2.0) as i32..(knob_x + kr + 2.0) as i32 {
                    if x < 0 || y < 0 || x >= w || y >= h {
                        continue;
                    }
                    let d = ((x as f32 + 0.5 - knob_x).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                    let cover = (kr + 0.5 - d).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        let i = ((y * w + x) * 4) as usize;
                        let t = ((y as f32 - (cy - kr)) / (2.0 * kr)).clamp(0.0, 1.0);
                        blend(&mut buf[i..i + 4], [0.98 - 0.12 * t, 0.99 - 0.08 * t, 1.0], cover);
                        if d > kr - 1.3 * s as f32 {
                            blend(&mut buf[i..i + 4], pal.dark(0.6), 0.7 * cover);
                        }
                    }
                }
            }
            let m = mute_rect(lw);
            tile(&mut buf, w, h, m, s, hover == Hit::Mute);
            button_label(text, &mut buf, w, h, s, m, if *muted { "Unmute" } else { "Mute" }, ink);
        }
        Flyout::Network { current, networks } => {
            label(text, &mut buf, w, h, s, "Wi-Fi", PAD, 22, 14.0, ink, lw - 2 * PAD);
            let now = crate::status::network_label(current);
            label(text, &mut buf, w, h, s, &format!("now: {now}"), lw / 2 - 10, 22, 12.0, soft, lw / 2 - PAD);
            match networks {
                None => {
                    label(text, &mut buf, w, h, s, "Looking for networks…", PAD, 44 + ROW / 2, 13.0, soft, lw - 2 * PAD);
                }
                Some(list) if list.is_empty() => {
                    label(text, &mut buf, w, h, s, "No Wi-Fi networks found", PAD, 44 + ROW / 2, 13.0, soft, lw - 2 * PAD);
                }
                Some(list) => {
                    for (i, net) in list.iter().take(MAX_NETWORKS).enumerate() {
                        let row = (PAD - 4, 44 + i as i32 * ROW, lw - PAD + 4, 44 + (i as i32 + 1) * ROW);
                        if hover == Hit::Network(i) {
                            fill_box(&mut buf, w, h, scaled((row.0, row.1 + 1, row.2, row.3 - 1), s), 4.0 * s as f32, s, |t| ([0.86 - 0.06 * t, 0.93 - 0.04 * t, 1.0], 0.95), 0.0, [0.45, 0.65, 0.9]);
                        }
                        let mid = (row.1 + row.3) / 2;
                        if net.active {
                            // a dot in the accent for the network in use
                            fill_box(&mut buf, w, h, scaled((PAD, mid - 4, PAD + 9, mid + 5), s), 4.5 * s as f32, s, |t| (pal.light(0.05 + 0.25 * (1.0 - t)), 0.95), 0.5, pal.dark(0.7));
                        }
                        label(text, &mut buf, w, h, s, &net.ssid, PAD + 18, mid, 13.0, ink, lw - 2 * PAD - 18 - 74);
                        let info = format!("{}{}%", if net.secure { "🔒 " } else { "" }, net.signal);
                        label(text, &mut buf, w, h, s, &info, lw - PAD - 64, mid, 12.0, soft, 70);
                    }
                }
            }
            let fr = footer_rect(lw, network_rows_end(f));
            tile(&mut buf, w, h, fr, s, hover == Hit::NetworkSettings);
            button_label(text, &mut buf, w, h, s, fr, "Network settings…", ink);
        }
        Flyout::Calendar { year, month, today } => {
            let title = format!("{} {}", MONTHS[*month as usize - 1], year);
            let (p, n) = (cal_prev_rect(), cal_next_rect(lw));
            tile(&mut buf, w, h, p, s, hover == Hit::CalPrev);
            button_label(text, &mut buf, w, h, s, p, "‹", ink);
            tile(&mut buf, w, h, n, s, hover == Hit::CalNext);
            button_label(text, &mut buf, w, h, s, n, "›", ink);
            if let Some(img) = text.line(&title, (lw - 90) * s, 14.0 * s as f32, ink, Some(([255, 255, 255], 0.9)), 2 * s) {
                blit(&mut buf, w, h, &img, lw * s / 2 - img.width / 2, 22 * s - img.height / 2);
            }
            let x0 = (lw - 7 * CAL_CELL.0) / 2;
            for (c, name) in WEEKDAYS.iter().enumerate() {
                let cx = x0 + c as i32 * CAL_CELL.0 + CAL_CELL.0 / 2;
                if let Some(img) = text.line(name, CAL_CELL.0 * s, 11.0 * s as f32, soft, Some(([255, 255, 255], 0.7)), s) {
                    blit(&mut buf, w, h, &img, cx * s - img.width / 2, 50 * s - img.height / 2);
                }
            }
            let first = weekday(*year, *month, 1);
            for d in 1..=days_in_month(*year, *month) {
                let r = cal_day_rect(lw, first, d);
                let is_today = (*year, *month, d) == *today;
                if is_today {
                    fill_box(&mut buf, w, h, scaled((r.0 + 2, r.1 + 1, r.2 - 2, r.3 - 1), s), 6.0 * s as f32, s, |t| (pal.light(0.0 + 0.1 * t), 0.95), 0.6, pal.dark(0.7));
                }
                let color = if is_today { [255, 255, 255] } else if (first + d - 1) % 7 >= 5 { [150, 60, 40] } else { ink };
                if let Some(img) = text.line(&d.to_string(), CAL_CELL.0 * s, 13.0 * s as f32, color, if is_today { Some(([0, 40, 90], 0.6)) } else { Some(([255, 255, 255], 0.7)) }, s) {
                    blit(&mut buf, w, h, &img, (r.0 + r.2) * s / 2 - img.width / 2, (r.1 + r.3) * s / 2 - img.height / 2);
                }
            }
        }
        Flyout::Notifications { items, dnd } => {
            label(text, &mut buf, w, h, s, "Notifications", PAD, 22, 14.0, ink, 150);
            let d = dnd_rect(lw);
            tile(&mut buf, w, h, d, s, hover == Hit::Dnd);
            button_label(text, &mut buf, w, h, s, d, if *dnd { "Quiet: on" } else { "Quiet: off" }, ink);
            if items.is_empty() {
                label(text, &mut buf, w, h, s, "Nothing new", PAD, 44 + HISTORY_ROW / 2, 13.0, soft, lw - 2 * PAD);
            }
            for (i, it) in items.iter().take(MAX_HISTORY).enumerate() {
                let y = 44 + i as i32 * HISTORY_ROW;
                label(text, &mut buf, w, h, s, &it.summary, PAD, y + 13, 13.0, ink, lw - 2 * PAD - 70);
                label(text, &mut buf, w, h, s, &it.age, lw - PAD - 64, y + 13, 11.0, soft, 70);
                let line = if it.app.is_empty() { it.body.clone() } else if it.body.is_empty() { it.app.clone() } else { format!("{}: {}", it.app, it.body) };
                label(text, &mut buf, w, h, s, &line, PAD, y + 31, 12.0, soft, lw - 2 * PAD);
                if i + 1 < items.len().min(MAX_HISTORY) {
                    for x in PAD * s..(lw - PAD) * s {
                        let idx = (((y + HISTORY_ROW - 1) * s * w + x) * 4) as usize;
                        blend(&mut buf[idx..idx + 4], pal.dark(0.5), 0.14);
                    }
                }
            }
            let fr = footer_rect(lw, history_rows_end(f));
            tile(&mut buf, w, h, fr, s, hover == Hit::ClearAll);
            button_label(text, &mut buf, w, h, s, fr, "Clear all", ink);
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_knows_the_days() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 11), 30);
        // 13 November 2026 is a Friday, 1 January 2000 a Saturday, 29 February 2024 a Thursday
        assert_eq!(weekday(2026, 11, 13), 4);
        assert_eq!(weekday(2000, 1, 1), 5);
        assert_eq!(weekday(2024, 2, 29), 3);
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 6, 0), (2026, 6));
        let (y, m, d) = today();
        assert!((2024..2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d));
        assert_eq!(age_label(5), "now");
        assert_eq!(age_label(125), "2 min ago");
        assert_eq!(age_label(7300), "2 h ago");
        assert_eq!(age_label(200_000), "2 days ago");
    }

    #[test]
    fn the_flyouts_have_sizes_and_things_to_click() {
        let mut text = TextRenderer::default();
        let vol = Flyout::Volume { percent: 40, muted: false };
        assert_eq!(size(&vol, &mut text, 1), (250, 96));
        assert_eq!(hit(&vol, 14.0, 51.0), Hit::Slider(0.0));
        assert!(matches!(hit(&vol, 125.0, 51.0), Hit::Slider(f) if (f - 0.5).abs() < 0.01));
        assert_eq!(hit(&vol, 120.0, 75.0), Hit::Mute);
        assert_eq!(hit(&vol, 5.0, 5.0), Hit::None);

        let nets = vec![
            WifiNet { ssid: "Home".into(), signal: 80, active: true, secure: true },
            WifiNet { ssid: "Cafe".into(), signal: 40, active: false, secure: false },
        ];
        let net = Flyout::Network { current: Network::Offline, networks: Some(nets) };
        assert_eq!(hit(&net, 100.0, 44.0 + 30.0), Hit::Network(1));
        let (_, h) = size(&net, &mut text, 1);
        assert_eq!(hit(&net, 100.0, (h - 20) as f64), Hit::NetworkSettings);
        let waiting = Flyout::Network { current: Network::Offline, networks: None };
        assert_eq!(hit(&waiting, 100.0, 50.0), Hit::None, "no rows to click while looking");

        let cal = Flyout::Calendar { year: 2026, month: 11, today: (2026, 11, 13) };
        assert_eq!(hit(&cal, 20.0, 20.0), Hit::CalPrev);
        assert_eq!(hit(&cal, 230.0, 20.0), Hit::CalNext);
        assert_eq!(hit(&cal, 126.0, 20.0), Hit::CalToday);
        // the 1st of November 2026 is a Sunday (last column of the first row), so the 13th, a
        // Friday, is in the fifth column of the third row
        let r = cal_day_rect(252, weekday(2026, 11, 1), 13);
        let first_row = cal_day_rect(252, weekday(2026, 11, 1), 1);
        assert_eq!(r.1 - first_row.1, CAL_CELL.1 * 2, "third row");
        assert_eq!(r.0 - cal_day_rect(252, 0, 1).0, 4 * CAL_CELL.0, "fifth column");
        let n = Flyout::Notifications { items: vec![], dnd: false };
        assert_eq!(hit(&n, 300.0, 20.0), Hit::Dnd);
        assert_eq!(hit(&Flyout::Tooltip(vec!["x".into()]), 5.0, 5.0), Hit::None);
    }

    #[test]
    fn every_flyout_draws_something_the_size_it_says() {
        let mut text = TextRenderer::default();
        let flyouts = [
            Flyout::Tooltip(vec!["Untitled - FeatherPad".into(), "Notes".into()]),
            Flyout::Volume { percent: 70, muted: false },
            Flyout::Volume { percent: 70, muted: true },
            Flyout::Network { current: Network::Wired, networks: Some(vec![WifiNet { ssid: "Home".into(), signal: 70, active: true, secure: true }]) },
            Flyout::Network { current: Network::Offline, networks: None },
            Flyout::Calendar { year: 2026, month: 2, today: (2026, 2, 3) },
            Flyout::Notifications { items: vec![HistoryItem { app: "Mail".into(), summary: "Hello".into(), body: "Lunch?".into(), age: "now".into() }], dnd: true },
        ];
        for f in &flyouts {
            for s in [1, 2] {
                let (w, h) = size(f, &mut text, s);
                let buf = draw(f, &mut text, s, Hit::None);
                assert_eq!(buf.len() as i32, w * s * h * s * 4, "{f:?} at {s}");
                assert!(buf.chunks(4).any(|p| p[3] > 100), "{f:?} has glass");
                assert_eq!(buf[3], 0, "{f:?}: the corner is transparent");
            }
        }
        // a lit slider fills with the accent up to the level
        let low = draw(&Flyout::Volume { percent: 10, muted: false }, &mut text, 1, Hit::None);
        let high = draw(&Flyout::Volume { percent: 90, muted: false }, &mut text, 1, Hit::None);
        assert_ne!(low, high);
    }
}
