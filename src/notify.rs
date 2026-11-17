//! Desktop notifications: the picture of the bubbles (pure drawing, for tests) and the state a
//! `org.freedesktop.Notifications` server keeps. `bin/fde-notify.rs` is the program.

use crate::panel::{blend, blit};
use crate::text::TextRenderer;

pub const W: i32 = 340;
pub const BUBBLE_H: i32 = 92;
pub const ACTION_W: i32 = 96;
pub const ACTION_H: i32 = 22;
pub const GAP: i32 = 8;
pub const MAX_SHOWN: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    /// when it goes away; None = stays until dismissed
    pub expires: Option<std::time::Instant>,
    pub critical: bool,
    /// the program's icon: a name from the icon themes or the path of a PNG or SVG (may be empty)
    pub icon: String,
    /// (key, label) pairs the program offered; "default" is what clicking the bubble does
    pub actions: Vec<(String, String)>,
}

/// What the server knows: the notes in order of arrival and the next id.
#[derive(Default)]
pub struct Store {
    pub notes: Vec<Note>,
    next_id: u32,
}

impl Store {
    /// Add a note (or replace the one with `replaces` as id); returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn notify(
        &mut self,
        app: &str,
        replaces: u32,
        summary: &str,
        body: &str,
        timeout_ms: i32,
        critical: bool,
        actions: &[String],
        now: std::time::Instant,
    ) -> u32 {
        let expires = if critical || timeout_ms == 0 {
            None
        } else {
            let ms = if timeout_ms < 0 { 6000 } else { timeout_ms as u64 };
            Some(now + std::time::Duration::from_millis(ms))
        };
        let strip = |s: &str| strip_markup(s);
        let pairs: Vec<(String, String)> = actions.chunks_exact(2).map(|c| (c[0].clone(), strip(&c[1]))).collect();
        let note = |id| Note { id, app: app.to_string(), summary: strip(summary), body: strip(body), expires, critical, icon: String::new(), actions: pairs.clone() };
        if replaces != 0 {
            if let Some(n) = self.notes.iter_mut().find(|n| n.id == replaces) {
                *n = note(replaces);
                return replaces;
            }
        }
        self.next_id += 1;
        let id = self.next_id;
        self.notes.push(note(id));
        id
    }

    /// Remove one note; true if it existed.
    pub fn close(&mut self, id: u32) -> bool {
        let before = self.notes.len();
        self.notes.retain(|n| n.id != id);
        self.notes.len() != before
    }

    /// Remove expired notes and return their ids.
    pub fn expire(&mut self, now: std::time::Instant) -> Vec<u32> {
        let gone: Vec<u32> = self.notes.iter().filter(|n| n.expires.map(|e| e <= now).unwrap_or(false)).map(|n| n.id).collect();
        self.notes.retain(|n| !gone.contains(&n.id));
        gone
    }

    /// The notes on screen, newest first.
    pub fn shown(&self) -> Vec<&Note> {
        self.notes.iter().rev().take(MAX_SHOWN).collect()
    }
}

impl Note {
    /// The buttons drawn on the bubble: the actions except the default one, at most two.
    pub fn buttons(&self) -> Vec<&(String, String)> {
        self.actions.iter().filter(|(k, _)| k != "default").take(2).collect()
    }

    pub fn has_default(&self) -> bool {
        self.actions.iter().any(|(k, _)| k == "default")
    }
}

/// The rectangle (x0, y0, x1, y1), in bubble pixels, of button `i` of a bubble.
pub fn button_rect(i: usize) -> (i32, i32, i32, i32) {
    let x1 = W - 16 - i as i32 * (ACTION_W + 8);
    (x1 - ACTION_W, BUBBLE_H - ACTION_H - 10, x1, BUBBLE_H - 10)
}

/// Which button of which bubble (counted from the top) is at this point?
pub fn action_at(x: f64, y: f64, notes: &[&Note]) -> Option<(usize, usize)> {
    let row = row_at(y, notes.len())?;
    let local_y = y - (row as i32 * (BUBBLE_H + GAP)) as f64;
    (0..notes[row].buttons().len()).find_map(|i| {
        let (x0, y0, x1, y1) = button_rect(i);
        (x >= x0 as f64 && x < x1 as f64 && local_y >= y0 as f64 && local_y < y1 as f64).then_some((row, i))
    })
}

/// Where "do not disturb" is kept: `$XDG_RUNTIME_DIR/fde/dnd` ("1" when on).
pub fn dnd_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("fde/dnd"))
}

pub fn dnd_on() -> bool {
    dnd_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim() == "1").unwrap_or(false)
}

pub fn set_dnd(on: bool) {
    if let Some(p) = dnd_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, if on { "1\n" } else { "0\n" });
    }
}

/// With do-not-disturb on only critical notes are shown; the others wait quietly until it is
/// off (or their time runs out).
// ---------------------------------------------------------------------------- the history

/// Notifications kept in the history.
pub const HISTORY_MAX: usize = 50;

/// One remembered notification.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    /// seconds since 1970
    pub time: u64,
    pub app: String,
    pub summary: String,
    pub body: String,
}

fn runtime_file(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("fde").join(name))
}

pub fn history_path() -> Option<std::path::PathBuf> {
    runtime_file("notification-history")
}

fn seen_path() -> Option<std::path::PathBuf> {
    runtime_file("notification-seen")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A history line: `time TAB app TAB summary TAB body`, tabs and line breaks escaped.
pub fn encode_entry(e: &HistoryEntry) -> String {
    format!("{}\t{}\t{}\t{}", e.time, escape(&e.app), escape(&e.summary), escape(&e.body))
}

pub fn decode_entry(line: &str) -> Option<HistoryEntry> {
    let f: Vec<&str> = line.splitn(4, '\t').collect();
    if f.len() != 4 {
        return None;
    }
    Some(HistoryEntry { time: f[0].parse().ok()?, app: unescape(f[1]), summary: unescape(f[2]), body: unescape(f[3]) })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// What an icon name from a program may be: a plain name, or the path of a PNG or SVG file.
pub fn clean_icon(name: &str) -> String {
    let n: String = name.chars().take(200).collect();
    let ok_name = !n.is_empty() && !n.contains('/') && !n.contains("..") && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'));
    let ok_path = n.starts_with('/') && !n.contains("..") && (n.ends_with(".png") || n.ends_with(".svg"));
    if ok_name || ok_path { n } else { String::new() }
}

impl Store {
    /// Say which icon a note shows (after `notify` gave its id).
    pub fn set_icon(&mut self, id: u32, icon: &str) {
        if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
            n.icon = clean_icon(icon);
        }
    }
}

/// Size of the icon in a bubble (logical pixels).
pub const ICON: i32 = 32;

/// `text` cut to at most `max` characters.
pub fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Remember a notification (the last `HISTORY_MAX` are kept).
pub fn add_history(app: &str, summary: &str, body: &str) {
    let Some(path) = history_path() else { return };
    let mut lines: Vec<String> = std::fs::read_to_string(&path).unwrap_or_default().lines().map(String::from).collect();
    lines.push(encode_entry(&HistoryEntry { time: now_secs(), app: app.to_string(), summary: summary.to_string(), body: body.to_string() }));
    if lines.len() > HISTORY_MAX {
        lines.drain(..lines.len() - HISTORY_MAX);
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, lines.join("\n") + "\n").is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// The remembered notifications, newest first.
pub fn read_history() -> Vec<HistoryEntry> {
    let text = history_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let mut v: Vec<HistoryEntry> = text.lines().filter_map(decode_entry).collect();
    v.reverse();
    v
}

pub fn clear_history() {
    if let Some(p) = history_path() {
        let _ = std::fs::remove_file(p);
    }
    mark_seen();
}

/// The user has looked at the history now.
pub fn mark_seen() {
    if let Some(p) = seen_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, now_secs().to_string());
    }
}

/// How many notifications came after the history was last looked at.
pub fn unread_count() -> usize {
    let seen: u64 = seen_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| t.trim().parse().ok()).unwrap_or(0);
    read_history().iter().filter(|e| e.time > seen).count()
}

pub fn visible<'a>(notes: Vec<&'a Note>, dnd: bool) -> Vec<&'a Note> {
    if dnd {
        notes.into_iter().filter(|n| n.critical).collect()
    } else {
        notes
    }
}

/// Notification bodies may carry a little HTML (`<b>`, `<i>`, `<a>`); show the text only.
pub fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

pub fn size(n: usize) -> (i32, i32) {
    let n = n.min(MAX_SHOWN) as i32;
    (W, n * BUBBLE_H + (n - 1).max(0) * GAP)
}

/// Which bubble is at height `y` (0 = the top one)?
pub fn row_at(y: f64, n: usize) -> Option<usize> {
    let step = (BUBBLE_H + GAP) as f64;
    let i = (y / step).floor();
    let within = y - i * step;
    (i >= 0.0 && (i as usize) < n.min(MAX_SHOWN) && within < BUBBLE_H as f64).then_some(i as usize)
}

/// The bubbles, newest on top, as one premultiplied RGBA picture of `size(n)` times `s`.
pub fn draw(text: &mut TextRenderer, s: i32, notes: &[&Note]) -> Vec<u8> {
    let (lw, lh) = size(notes.len());
    let (w, h) = (lw * s, lh * s);
    let sf = s as f32;
    let mut buf = vec![0u8; (w * h * 4).max(0) as usize];
    let pal = crate::theme::palette();
    let rad = (12.0 - 12.0 * pal.solid) * sf;
    let bubble_h = BUBBLE_H * s;
    for (i, note) in notes.iter().take(MAX_SHOWN).enumerate() {
        let y0 = i as i32 * (BUBBLE_H + GAP) * s;
        for y in 0..bubble_h {
            for x in 0..w {
                let dx = (x as f32 + 0.5 - w as f32 / 2.0).abs() - (w as f32 / 2.0 - rad);
                let dy = (y as f32 + 0.5 - bubble_h as f32 / 2.0).abs() - (bubble_h as f32 / 2.0 - rad);
                let d = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - rad;
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let t = y as f32 / bubble_h as f32;
                let idx = (((y0 + y) * w + x) * 4) as usize;
                let px = &mut buf[idx..idx + 4];
                // the light glass of the task bar and the windows (a red tint for critical ones), a
                // sheen, a bright rim with a thin shade inside it
                let base = if note.critical { [0.98, 0.58, 0.52 - 0.1 * t] } else { pal.light(0.42 + 0.18 * (1.0 - t)) };
                blend(px, base, (0.88 + 0.06 * t + 0.06 * pal.solid) * cover);
                if t < 0.4 {
                    blend(px, [1.0, 1.0, 1.0], 0.30 * (1.0 - t / 0.4) * cover * pal.shine.max(0.25));
                }
                if d > -1.5 * sf {
                    blend(px, [1.0, 1.0, 1.0], 0.85 * cover);
                } else if d > -2.5 * sf {
                    blend(px, pal.dark(0.5), 0.28 * cover);
                }
            }
        }
        // the program's icon at the left (a dialog icon when it sent none or it cannot be found)
        let fallback: &[&str] = if note.critical { &["dialog-error"] } else { &["dialog-information"] };
        let icon = crate::icons::cached(&note.icon, ICON * s, fallback);
        let indent = if icon.is_some() { ICON + 10 } else { 0 };
        if let Some(i) = &icon {
            crate::icons::blit_icon(&mut buf, w, h, i, 14 * s, y0 + (BUBBLE_H - ICON) * s / 2 - 2 * s, 1.0);
        }
        let ink = if note.critical { [95, 12, 6] } else { pal.ink };
        let soft = [(ink[0] as u32 * 3 / 4 + 40) as u8, (ink[1] as u32 * 3 / 4 + 45) as u8, (ink[2] as u32 * 3 / 4 + 55) as u8];
        if let Some(img) = text.line(&note.summary, w - (32 + indent) * s, 15.0 * sf, ink, Some(([255, 255, 255], 0.9)), 2 * s) {
            blit(&mut buf, w, h, &img, (16 + indent) * s, y0 + 12 * s);
        }
        // the message: on two lines when there are no buttons below it
        let lines = if note.buttons().is_empty() { wrap(text, &note.body, w - (32 + indent) * s, 13.0 * sf, 2) } else { vec![note.body.clone()] };
        for (li, line) in lines.iter().enumerate() {
            if let Some(img) = text.line(line, w - (32 + indent) * s, 13.0 * sf, soft, Some(([255, 255, 255], 0.8)), 2 * s) {
                blit(&mut buf, w, h, &img, (16 + indent) * s, y0 + (38 + 18 * li as i32) * s);
            }
        }
        // the buttons the program asked for
        for (bi, (_, label)) in note.buttons().into_iter().enumerate() {
            let (bx0, by0, bx1, by1) = button_rect(bi);
            let (bx0, by0, bx1, by1) = (bx0 * s, y0 + by0 * s, bx1 * s, y0 + by1 * s);
            let brad = 7.0 * sf;
            for y in by0..by1 {
                for x in bx0..bx1 {
                    let dx = (x as f32 + 0.5 - (bx0 + bx1) as f32 / 2.0).abs() - ((bx1 - bx0) as f32 / 2.0 - brad);
                    let dy = (y as f32 + 0.5 - (by0 + by1) as f32 / 2.0).abs() - ((by1 - by0) as f32 / 2.0 - brad);
                    let d = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - brad;
                    let cover = (0.5 - d).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        let idx = ((y * w + x) * 4) as usize;
                        blend(&mut buf[idx..idx + 4], [1.0, 1.0, 1.0], 0.45 * cover);
                        if d > -1.2 * sf {
                            blend(&mut buf[idx..idx + 4], [1.0, 1.0, 1.0], 0.85 * cover);
                        } else if d > -2.2 * sf {
                            blend(&mut buf[idx..idx + 4], pal.dark(0.55), 0.22 * cover);
                        }
                    }
                }
            }
            if let Some(img) = text.line(label, bx1 - bx0 - 8 * s, 12.0 * sf, ink, Some(([255, 255, 255], 0.8)), s) {
                blit(&mut buf, w, h, &img, bx0 + (bx1 - bx0 - img.width) / 2, by0 + (by1 - by0 - img.height) / 2);
            }
        }
    }
    buf
}

/// `text` broken into at most `max_lines` lines of whole words that fit `width` pixels; the
/// last line keeps the rest (and is cut with "…" when drawn).
pub fn wrap(renderer: &mut crate::text::TextRenderer, text: &str, width: i32, px: f32, max_lines: usize) -> Vec<String> {
    let fits = |r: &mut crate::text::TextRenderer, s: &str| r.line(s, 100_000, px, [255, 255, 255], None, 0).map(|img| img.width <= width).unwrap_or(true);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if lines.len() + 1 == max_lines {
            lines.push(words[i..].join(" "));
            break;
        }
        let mut line = words[i].to_string();
        i += 1;
        while i < words.len() {
            let longer = format!("{line} {}", words[i]);
            if !fits(renderer, &longer) {
                break;
            }
            line = longer;
            i += 1;
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// The tests that point XDG_RUNTIME_DIR at a folder of their own take turns.
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn the_history_remembers_the_latest_notifications_newest_first() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        assert!(read_history().is_empty() && unread_count() == 0);
        add_history("Mail", "Hello\tthere", "line one\nline two \\ end");
        add_history("Chat", "Second", "");
        let h = read_history();
        assert_eq!((h[0].app.as_str(), h[0].summary.as_str()), ("Chat", "Second"), "newest first");
        assert_eq!((h[1].summary.as_str(), h[1].body.as_str()), ("Hello\tthere", "line one\nline two \\ end"), "tabs, line breaks and backslashes survive");
        assert_eq!(unread_count(), 2, "nothing looked at yet");
        std::fs::write(seen_path().unwrap(), "99999999999").unwrap();
        assert_eq!(unread_count(), 0, "all looked at");
        std::fs::write(seen_path().unwrap(), "0").unwrap();
        assert_eq!(unread_count(), 2);
        for i in 0..(HISTORY_MAX + 10) {
            add_history("x", &format!("n{i}"), "");
        }
        assert_eq!(read_history().len(), HISTORY_MAX, "the oldest are dropped");
        assert_eq!(read_history()[0].summary, format!("n{}", HISTORY_MAX + 9));
        clear_history();
        assert!(read_history().is_empty());
        assert!(decode_entry("broken").is_none() && decode_entry("x\ta\tb\tc").is_none());
    }

    #[test]
    fn notes_get_ids_replace_expire_and_close() {
        let t0 = Instant::now();
        let mut s = Store::default();
        let a = s.notify("app", 0, "Hi", "there", -1, false, &[], t0);
        let b = s.notify("app", 0, "Second", "", 1000, false, &[], t0);
        let c = s.notify("app", 0, "Sticky", "", 0, false, &[], t0);
        assert_eq!((a, b, c), (1, 2, 3));
        assert_eq!(s.notify("app", a, "Hi again", "", -1, false, &[], t0), a, "replacing keeps the id");
        assert_eq!(s.notes.len(), 3);
        assert!(s.expire(t0 + Duration::from_millis(500)).is_empty());
        assert_eq!(s.expire(t0 + Duration::from_millis(1500)), vec![b]);
        assert_eq!(s.expire(t0 + Duration::from_secs(60)), vec![a], "the default lifetime ends; sticky stays");
        assert!(s.close(c) && !s.close(c));
    }

    #[test]
    fn markup_is_stripped_and_the_newest_is_on_top() {
        assert_eq!(strip_markup("<b>Bold</b> &amp; <a href=\"x\">link</a>"), "Bold & link");
        let t0 = Instant::now();
        let mut s = Store::default();
        for i in 0..7 {
            s.notify("a", 0, &format!("n{i}"), "", -1, false, &[], t0);
        }
        let shown = s.shown();
        assert_eq!(shown.len(), MAX_SHOWN);
        assert_eq!(shown[0].summary, "n6");
    }

    #[test]
    fn actions_become_buttons_and_the_default_one_is_the_bubble_itself() {
        let t0 = Instant::now();
        let mut s = Store::default();
        let actions: Vec<String> = ["default", "Open", "reply", "Reply", "mute", "Mute"].iter().map(|x| x.to_string()).collect();
        s.notify("chat", 0, "Anna", "hello", -1, false, &actions, t0);
        s.notify("mail", 0, "Plain", "no buttons", -1, false, &[], t0);
        let shown = s.shown(); // newest first: the plain one, then Anna
        assert!(shown[1].has_default() && !shown[0].has_default());
        let labels: Vec<&str> = shown[1].buttons().iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(labels, ["Reply", "Mute"], "the default action is not a button; at most two buttons");
        // a click on a button finds it; a click on the plain bubble's corner or an empty spot finds none
        let (x0, y0, x1, y1) = button_rect(0);
        let row_y = (BUBBLE_H + GAP) as f64;
        assert_eq!(action_at(((x0 + x1) / 2) as f64, row_y + ((y0 + y1) / 2) as f64, &shown), Some((1, 0)));
        let (x0, y0, x1, y1) = button_rect(1);
        assert_eq!(action_at(((x0 + x1) / 2) as f64, row_y + ((y0 + y1) / 2) as f64, &shown), Some((1, 1)));
        assert_eq!(action_at(((x0 + x1) / 2) as f64, ((y0 + y1) / 2) as f64, &shown), None, "the first bubble has no buttons");
        assert_eq!(action_at(10.0, row_y + 10.0, &shown), None);
        // the buttons are drawn
        let mut text = TextRenderer::default();
        let with = draw(&mut text, 1, &shown);
        let plain_only = draw(&mut text, 1, &shown[..1]);
        assert_ne!(&with[(BUBBLE_H + GAP) as usize * W as usize * 4..], &plain_only[..plain_only.len().min(with.len() - (BUBBLE_H + GAP) as usize * W as usize * 4)]);
    }

    #[test]
    fn do_not_disturb_hides_everything_but_critical_notes_and_is_remembered_in_a_file() {
        let t0 = Instant::now();
        let mut s = Store::default();
        s.notify("a", 0, "Chat", "hi", -1, false, &[], t0);
        s.notify("a", 0, "Disk full", "!", -1, true, &[], t0);
        let all = s.shown();
        assert_eq!(visible(all.clone(), false).len(), 2);
        let quiet = visible(all, true);
        assert_eq!(quiet.len(), 1);
        assert_eq!(quiet[0].summary, "Disk full");
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        assert!(!dnd_on());
        set_dnd(true);
        assert!(dnd_on());
        set_dnd(false);
        assert!(!dnd_on());
    }

    #[test]
    fn bubbles_are_drawn_with_gaps_and_hit_testing_matches() {
        let t0 = Instant::now();
        let mut s = Store::default();
        s.notify("a", 0, "One", "first", -1, false, &[], t0);
        s.notify("a", 0, "Two", "second", -1, false, &[], t0);
        let mut text = TextRenderer::default();
        let shown = s.shown();
        let buf = draw(&mut text, 1, &shown);
        let (w, h) = size(2);
        assert_eq!(h, 2 * BUBBLE_H + GAP);
        let alpha = |x: i32, y: i32| buf[((y * w + x) * 4 + 3) as usize];
        assert!(alpha(w / 2, 5) > 100 && alpha(w / 2, BUBBLE_H + GAP + 5) > 100, "both bubbles are on");
        assert_eq!(alpha(w / 2, BUBBLE_H + GAP / 2), 0, "and the gap between them is clear");
        assert_eq!(row_at(10.0, 2), Some(0));
        assert_eq!(row_at((BUBBLE_H + GAP + 10) as f64, 2), Some(1));
        assert_eq!(row_at((BUBBLE_H + 2) as f64, 2), None);
    }

    #[test]
    fn long_text_is_cut_by_characters() {
        assert_eq!(clip("häßlich", 3), "häß");
        assert_eq!(clip("ok", 200), "ok");
        assert_eq!(clip(&"x".repeat(5000), 1000).len(), 1000);
    }

    #[test]
    fn icon_names_from_programs_are_cleaned() {
        assert_eq!(clean_icon("firefox"), "firefox");
        assert_eq!(clean_icon("dialog-information"), "dialog-information");
        assert_eq!(clean_icon("/usr/share/pixmaps/x.png"), "/usr/share/pixmaps/x.png");
        assert_eq!(clean_icon("/etc/passwd"), "", "only pictures");
        assert_eq!(clean_icon("../../etc/x.png"), "");
        assert_eq!(clean_icon("a/b"), "");
        assert_eq!(clean_icon(""), "");
        assert_eq!(clean_icon("naïve"), "");
        assert_eq!(clean_icon(&"a".repeat(500)).len(), 200);
    }

    #[test]
    fn a_bubble_shows_its_icon_and_the_text_moves_over() {
        let mut store = Store::default();
        let now = std::time::Instant::now();
        let id = store.notify("App", 0, "Hello", "A message that is long enough to need the width it has", 5000, false, &[], now);
        let plain = store.notes[0].clone();
        assert_eq!(plain.icon, "");
        store.set_icon(id, "dialog-warning");
        assert_eq!(store.notes[0].icon, "dialog-warning");
        store.set_icon(id, "/etc/passwd");
        assert_eq!(store.notes[0].icon, "", "a path that is not a picture is dropped");
        store.set_icon(id, "dialog-warning");
        let mut text = TextRenderer::default();
        let with = draw(&mut text, 1, &[&store.notes[0]]);
        let (w, _) = size(1);
        // the warning triangle is amber: some pixel in the icon's place is orange
        let at = |x: i32, y: i32| {
            let i = ((y * w + x) * 4) as usize;
            [with[i] as i32, with[i + 1] as i32, with[i + 2] as i32, with[i + 3] as i32]
        };
        let amber = (14..14 + ICON).any(|x| (20..20 + ICON).any(|y| { let p = at(x, y); p[0] > 200 && p[1] > 120 && p[1] < 190 && p[2] < 90 }));
        assert!(amber, "the icon is drawn at the left");
    }
}
