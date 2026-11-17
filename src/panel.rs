//! The task bar's picture: a glass strip along the bottom with the start orb on the left and
//! the clock on the right. Pure drawing (premultiplied RGBA, top row first) so it can be tested;
//! `bin/fde-panel.rs` puts it on screen as a layer-shell client.

use crate::text::TextRenderer;

pub const HEIGHT: i32 = 40;
pub const ORB_SIZE: i32 = 32;
pub const ORB_MARGIN: i32 = 6;

/// Is the point on the start button (a rounded square)?
pub fn on_orb(x: f64, y: f64, height: i32) -> bool {
    let (cx, cy) = orb_center(height);
    let r = ORB_SIZE as f64 / 2.0;
    (x - cx).abs() <= r && (y - cy).abs() <= r
}

/// One window in the task list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    pub title: String,
    pub active: bool,
    pub minimized: bool,
    /// the program's icon, `TASK_ICON` logical pixels at the bar's scale
    pub icon: Option<crate::icons::Icon>,
    /// how many windows the program has (a number on the button when more than one)
    pub windows: usize,
    /// pinned to the bar and not running: only its icon, no tile
    pub idle: bool,
}

/// What the task bar knows of one window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WinInfo {
    pub app_id: String,
    pub title: String,
    pub active: bool,
    pub minimized: bool,
}

/// One button of the bar: a program with its windows (none: pinned and not running).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// the installed program (an index into the list of apps), if it is one
    pub app: Option<usize>,
    /// the windows of it, as indexes into the list given to `group_windows`
    pub windows: Vec<usize>,
    pub pinned: bool,
}

/// The buttons of the bar: the pinned programs first, in the order pinned (running or not), then
/// the other programs in the order their first window came. Windows of one program share a button.
pub fn group_windows(windows: &[WinInfo], apps: &[crate::apps::App], pinned: &[String]) -> Vec<Group> {
    // a window that is not an installed program (or names no program) is a button of its own
    let mut groups: Vec<(Group, Option<String>)> = Vec::new();
    for name in pinned {
        if let Some(i) = crate::apps::index_for_pinned(apps, name) {
            if !groups.iter().any(|(g, _)| g.app == Some(i)) {
                groups.push((Group { app: Some(i), windows: Vec::new(), pinned: true }, None));
            }
        }
    }
    for (w, win) in windows.iter().enumerate() {
        let app = crate::apps::index_for_window(apps, &win.app_id);
        let by_name = if app.is_none() && !win.app_id.is_empty() { Some(win.app_id.to_lowercase()) } else { None };
        let found = groups.iter_mut().find(|(g, name)| match (app, &by_name) {
            (Some(i), _) => g.app == Some(i),
            (None, Some(n)) => g.app.is_none() && name.as_ref() == Some(n),
            _ => false,
        });
        match found {
            Some((g, _)) => g.windows.push(w),
            None => groups.push((Group { app, windows: vec![w], pinned: false }, by_name)),
        }
    }
    groups.into_iter().map(|(g, _)| g).collect()
}

impl Group {
    /// The button as `draw` wants it (without its icon).
    pub fn task(&self, windows: &[WinInfo], apps: &[crate::apps::App]) -> Task {
        let mine: Vec<&WinInfo> = self.windows.iter().filter_map(|&w| windows.get(w)).collect();
        let title = mine.first().map(|w| w.title.clone()).filter(|t| !t.is_empty()).or_else(|| self.app.and_then(|i| apps.get(i)).map(|a| a.name.clone())).unwrap_or_default();
        Task {
            title,
            active: mine.iter().any(|w| w.active),
            minimized: !mine.is_empty() && mine.iter().all(|w| w.minimized),
            icon: None,
            windows: mine.len(),
            idle: mine.is_empty(),
        }
    }

    /// The lines of the tooltip: the title of each window, or the program's name when none is open.
    pub fn tooltip(&self, windows: &[WinInfo], apps: &[crate::apps::App]) -> Vec<String> {
        let titles: Vec<String> = self.windows.iter().filter_map(|&w| windows.get(w)).map(|w| if w.title.is_empty() { "(untitled)".to_string() } else { w.title.clone() }).collect();
        if titles.is_empty() {
            vec![self.app.and_then(|i| apps.get(i)).map(|a| a.name.clone()).unwrap_or_default()]
        } else {
            titles
        }
    }
}

/// Size of the icons on the task buttons (logical pixels).
pub const TASK_ICON: i32 = 24;

const TASKS_LEFT: i32 = ORB_MARGIN * 2 + ORB_SIZE + 6;
const TASK_GAP: i32 = 4;
const TASK_MAX_W: i32 = 46;
const CLOCK_ROOM: i32 = 90;
/// The clock (and the date under it) at the right end of the bar can be clicked this far in.
pub const CLOCK_HIT_WIDTH: i32 = 100;

// Room at the right end of the bar for the clock, workspace dots and status labels (logical
// pixels): set each time the status labels are drawn, so the window buttons stop before them.
// (The task bar draws on one thread.)
thread_local! {
    static RIGHT_ROOM: std::cell::Cell<i32> = const { std::cell::Cell::new(CLOCK_ROOM) };
}

pub fn right_room() -> i32 {
    RIGHT_ROOM.with(|r| r.get())
}

/// The x range and the y range of task button `i` of `n` in a panel `width` wide.
pub fn task_rect(i: usize, n: usize, width: i32, height: i32) -> (i32, i32, i32, i32) {
    let avail = (width - TASKS_LEFT - right_room()).max(0);
    let w = ((avail - TASK_GAP * (n as i32 - 1).max(0)) / n.max(1) as i32).clamp(0, TASK_MAX_W);
    let x0 = TASKS_LEFT + i as i32 * (w + TASK_GAP);
    (x0, x0 + w, 5, height - 5)
}

/// Which task button is at (x, y)?
pub fn task_at(x: f64, y: f64, width: i32, height: i32, n: usize) -> Option<usize> {
    (0..n).find(|&i| {
        let (x0, x1, y0, y1) = task_rect(i, n, width, height);
        x >= x0 as f64 && x < x1 as f64 && y >= y0 as f64 && y < y1 as f64
    })
}

fn orb_center(height: i32) -> (f64, f64) {
    ((ORB_MARGIN + ORB_SIZE / 2) as f64, height as f64 / 2.0)
}

pub(crate) fn blend(px: &mut [u8], rgb: [f32; 3], a: f32) {
    // source-over of a premultiplied colour (rgb * a, a)
    let inv = 1.0 - a;
    for c in 0..3 {
        px[c] = (rgb[c] * 255.0 * a + px[c] as f32 * inv).round().clamp(0.0, 255.0) as u8;
    }
    px[3] = (255.0 * a + px[3] as f32 * inv).round().clamp(0.0, 255.0) as u8;
}

/// Composite a rendered line of text (premultiplied) onto `buf` at (x0, y0).
pub(crate) fn blit(buf: &mut [u8], width: i32, height: i32, img: &crate::text::TextImage, x0: i32, y0: i32) {
    for yy in 0..img.height {
        for xx in 0..img.width {
            let (px_, py_) = (x0 + xx, y0 + yy);
            if px_ < 0 || py_ < 0 || px_ >= width || py_ >= height {
                continue;
            }
            let s = ((yy * img.width + xx) * 4) as usize;
            let d = ((py_ * width + px_) * 4) as usize;
            let inv = 255 - img.rgba[s + 3] as u32;
            for c in 0..4 {
                buf[d + c] = (img.rgba[s + c] as u32 + buf[d + c] as u32 * inv / 255).min(255) as u8;
            }
        }
    }
}

/// `width` x `height` RGBA, premultiplied, in pixels: the bar is `width / s` x `height / s`
/// logical pixels drawn at `s` pixels per logical one. `hover` brightens the orb, `clock` is
/// e.g. "14:05".
#[allow(clippy::too_many_arguments)]
pub fn draw(text: &mut TextRenderer, width: i32, height: i32, s: i32, hover: bool, clock: &str, tasks: &[Task]) -> Vec<u8> {
    let mut buf = vec![0u8; (width * height * 4) as usize];
    let sf = s as f32;
    let pal = crate::theme::palette();
    for y in 0..height {
        let t = y as f32 / (height - 1).max(1) as f32;
        // the glass of the window frames: the accent lifted toward white, a sheen over the upper
        // half, a bright edge on top with a thin shade under it
        let base = pal.light(0.42 + 0.18 * (1.0 - t));
        let alpha = 0.82 + 0.08 * t + 0.10 * pal.solid;
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let px = &mut buf[i..i + 4];
            blend(px, base, alpha);
            if t < 0.5 {
                blend(px, [1.0, 1.0, 1.0], 0.34 * (1.0 - t / 0.5) * pal.shine.max(0.25));
            }
            if y < s {
                blend(px, [1.0, 1.0, 1.0], 0.85);
            } else if y < 2 * s {
                blend(px, pal.dark(0.5), 0.28);
            }
        }
    }
    // the start button: a glass rounded square with the FDE mark (four coloured bubbles)
    {
        let (cx, cy) = orb_center(height / s);
        let (cx, cy) = (cx as f32 * sf, cy as f32 * sf);
        let r = ORB_SIZE as f32 / 2.0 * sf;
        let rad = 9.0 * sf;
        for y in 0..height {
            for x in 0..(ORB_SIZE + 2 * ORB_MARGIN) * s {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let (qx, qy) = (dx.abs() - (r - rad), dy.abs() - (r - rad));
                let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - rad;
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let t = ((dy + r) / (2.0 * r)).clamp(0.0, 1.0);
                let i = ((y * width + x) * 4) as usize;
                let px = &mut buf[i..i + 4];
                // pale glass, a little bluer toward the bottom
                let k = if hover { 0.10 } else { 0.0 };
                blend(px, [0.95 - 0.16 * t + k, 0.98 - 0.10 * t + k, 1.0], 0.97 * cover);
                if t < 0.5 {
                    blend(px, [1.0, 1.0, 1.0], 0.55 * (1.0 - t / 0.5) * cover);
                }
                // outline: dark outside, light inside
                if d > -1.3 * sf {
                    blend(px, pal.dark(0.65), 0.75 * cover);
                } else if d > -2.5 * sf {
                    blend(px, [1.0, 1.0, 1.0], 0.65 * cover);
                }
            }
        }
        // the mark
        let colors = [[0.95, 0.33, 0.20], [0.30, 0.72, 0.25], [0.20, 0.55, 0.92], [0.98, 0.76, 0.12]];
        for (n, (bx, by)) in [(-0.27f32, -0.27f32), (0.27, -0.27), (-0.27, 0.27), (0.27, 0.27)].into_iter().enumerate() {
            let (bcx, bcy, br) = (cx + bx * r, cy + by * r, 0.20 * r);
            for y in (bcy - br - 2.0) as i32..(bcy + br + 2.0) as i32 {
                for x in (bcx - br - 2.0) as i32..(bcx + br + 2.0) as i32 {
                    if x < 0 || y < 0 || x >= width || y >= height {
                        continue;
                    }
                    let (ddx, ddy) = (x as f32 + 0.5 - bcx, y as f32 + 0.5 - bcy);
                    let d = (ddx * ddx + ddy * ddy).sqrt();
                    let cover = (br + 0.5 - d).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        let i = ((y * width + x) * 4) as usize;
                        let lit = (1.0 - (((ddx + 0.3 * br) / br).powi(2) + ((ddy + 0.4 * br) / br).powi(2)).sqrt() * 0.7).clamp(0.0, 1.0);
                        let c = colors[n];
                        let shade = 0.78 + 0.35 * lit;
                        blend(&mut buf[i..i + 4], [(c[0] * shade).min(1.0), (c[1] * shade).min(1.0), (c[2] * shade).min(1.0)], cover);
                    }
                }
            }
        }
    }
    // the task buttons: a glass tile each with only the program's icon (a letter without one);
    // the active window's is lit from below, a minimized one is faint
    let ink = pal.ink;
    for (i, task) in tasks.iter().enumerate() {
        let (lx0, lx1, ly0, ly1) = task_rect(i, tasks.len(), width / s, height / s);
        if lx1 - lx0 < 24 {
            continue;
        }
        let (x0, x1, y0, y1) = (lx0 * s, lx1 * s, ly0 * s, ly1 * s);
        let rad = 7.0f32 * sf;
        for y in if task.idle { 0..0 } else { y0..y1 } {
            for x in x0..x1 {
                let dx = (x as f32 + 0.5 - (x0 + x1) as f32 / 2.0).abs() - ((x1 - x0) as f32 / 2.0 - rad);
                let dy = (y as f32 + 0.5 - (y0 + y1) as f32 / 2.0).abs() - ((y1 - y0) as f32 / 2.0 - rad);
                let d = dx.max(0.0).hypot(dy.max(0.0)) - rad;
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let t = (y - y0) as f32 / (y1 - y0) as f32;
                let i = ((y * width + x) * 4) as usize;
                let px = &mut buf[i..i + 4];
                if task.active {
                    blend(px, [1.0, 1.0, 1.0], (0.50 - 0.18 * t) * cover);
                    let cxn = ((x as f32 + 0.5 - (x0 + x1) as f32 / 2.0) / ((x1 - x0) as f32 / 2.0)).abs();
                    let glow = (t - 0.4).max(0.0) / 0.6 * (1.0 - cxn * cxn).max(0.0);
                    blend(px, pal.accent, 0.45 * glow * cover);
                } else if !task.minimized {
                    blend(px, [1.0, 1.0, 1.0], (0.26 - 0.12 * t) * cover);
                } else {
                    blend(px, [1.0, 1.0, 1.0], 0.10 * cover);
                }
                if d > -1.2 * sf {
                    let edge = if task.active { 0.95 } else if task.minimized { 0.30 } else { 0.60 };
                    blend(px, [1.0, 1.0, 1.0], edge * cover);
                } else if d > -2.2 * sf {
                    blend(px, pal.dark(0.55), 0.22 * cover);
                }
            }
        }
        match &task.icon {
            Some(icon) => {
                crate::icons::blit_icon(&mut buf, width, height, icon, x0 + ((x1 - x0) - icon.size) / 2, (height - icon.size) / 2, if task.minimized { 0.55 } else { 1.0 });
            }
            None => {
                let letter: String = task.title.trim_start_matches('*').chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                if let Some(img) = text.line(&letter, x1 - x0, 17.0 * sf, ink, Some(([255, 255, 255], 0.8)), 2 * s) {
                    blit(&mut buf, width, height, &img, x0 + ((x1 - x0) - img.width) / 2, (height - img.height) / 2);
                }
            }
        }
    }
    // a number on the button of a program with more than one window
    for (i, task) in tasks.iter().enumerate() {
        if task.windows < 2 {
            continue;
        }
        let (_, lx1, _, ly1) = task_rect(i, tasks.len(), width / s, height / s);
        let (cx, cy, r) = ((lx1 - 9) as f32 * sf, (ly1 - 9) as f32 * sf, 7.0 * sf);
        for y in (cy - r - 2.0) as i32..(cy + r + 2.0) as i32 {
            for x in (cx - r - 2.0) as i32..(cx + r + 2.0) as i32 {
                if x < 0 || y < 0 || x >= width || y >= height {
                    continue;
                }
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cover = (r + 0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let idx = ((y * width + x) * 4) as usize;
                    blend(&mut buf[idx..idx + 4], pal.light(0.1), 0.97 * cover);
                    if d > r - 1.2 * sf {
                        blend(&mut buf[idx..idx + 4], pal.dark(0.7), 0.8 * cover);
                    }
                }
            }
        }
        if let Some(img) = text.line(&task.windows.to_string(), 24 * s, 10.0 * sf, [255, 255, 255], Some(([0, 30, 70], 0.7)), s) {
            blit(&mut buf, width, height, &img, cx as i32 - img.width / 2, cy as i32 - img.height / 2);
        }
    }
    // the clock (and under it the date), in the dark ink with a white glow, at the right
    {
        let mut lines = clock.splitn(2, '\n');
        let (time, date) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
        let glow = Some(([255, 255, 255], 0.9));
        let t_img = text.line(time, 120 * s, 16.0 * sf, ink, glow, 3 * s);
        let d_img = if date.is_empty() { None } else { text.line(date, 120 * s, 10.5 * sf, ink, Some(([255, 255, 255], 0.8)), 2 * s) };
        let col_w = t_img.as_ref().map(|i| i.width).unwrap_or(0).max(d_img.as_ref().map(|i| i.width).unwrap_or(0));
        let total = t_img.as_ref().map(|i| i.height).unwrap_or(0) + d_img.as_ref().map(|i| i.height - 5 * s).unwrap_or(0);
        let mut y = (height - total) / 2;
        let cx = width - 14 * s - col_w / 2;
        if let Some(img) = &t_img {
            blit(&mut buf, width, height, img, cx - img.width / 2, y);
            y += img.height - 3 * s;
        }
        if let Some(img) = &d_img {
            blit(&mut buf, width, height, img, cx - img.width / 2, y);
        }
    }
    buf
}

/// The workspace dots left of the clock: `current` of `count` (1-based). Drawn over the bar.
pub fn draw_workspaces(buf: &mut [u8], width: i32, height: i32, s: i32, current: usize, count: usize) {
    if count < 2 {
        return;
    }
    let sf = s as f32;
    let r = 4.5 * sf;
    let gap = 8.0 * sf;
    let total = count as f32 * (2.0 * r) + (count as f32 - 1.0) * gap;
    let x0 = width as f32 - 112.0 * sf - total;
    let cy = height as f32 / 2.0;
    for i in 0..count {
        let cx = x0 + r + i as f32 * (2.0 * r + gap);
        let on = i + 1 == current;
        for y in (cy - r - 2.0) as i32..=(cy + r + 2.0) as i32 {
            for x in (cx - r - 2.0) as i32..=(cx + r + 2.0) as i32 {
                if x < 0 || y < 0 || x >= width || y >= height {
                    continue;
                }
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cover = (r + 0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let idx = ((y * width + x) * 4) as usize;
                let ink = crate::theme::palette().ink;
                let ink = [ink[0] as f32 / 255.0, ink[1] as f32 / 255.0, ink[2] as f32 / 255.0];
                if on {
                    blend(&mut buf[idx..idx + 4], ink, 0.85 * cover);
                } else {
                    // a ring
                    let ring = (1.0 - ((r - d) / (1.4 * sf)).clamp(0.0, 1.0)).max(0.0);
                    blend(&mut buf[idx..idx + 4], ink, (0.08 + 0.55 * ring) * cover);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusKind {
    Notifications,
    Cpu,
    Memory,
    Volume,
    Network,
    /// shown only when the machine has a Bluetooth controller
    Bluetooth,
    KeyboardLayout,
    Battery,
    DoNotDisturb,
    /// an item of the system tray (its index in `tray::read_entries`); its label is "icon|title"
    Tray(usize),
}

thread_local! {
    static CHROME_ICONS: std::cell::RefCell<crate::icons::IconCache> = std::cell::RefCell::new(crate::icons::IconCache::default());
}

/// A small icon for the shell's own buttons and labels (looked up once per size).
fn chrome_icon(name: &str, px: i32) -> Option<crate::icons::Icon> {
    CHROME_ICONS.with(|c| c.borrow_mut().get(name, px, &[]).cloned())
}

/// The icon that goes with a status label, from what the label says.
fn status_icon_name(kind: StatusKind, label: &str) -> Option<&'static str> {
    match kind {
        StatusKind::Volume => {
            let l = label.to_lowercase();
            if l.contains("mute") {
                return Some("audio-volume-muted");
            }
            let pct: i32 = l.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(50);
            Some(if pct >= 66 { "audio-volume-high" } else if pct >= 33 { "audio-volume-medium" } else { "audio-volume-low" })
        }
        StatusKind::Network => Some(if label.to_lowercase().starts_with("wired") { "network-wired" } else { "network-wireless" }),
        StatusKind::Bluetooth => Some("bluetooth"),
        StatusKind::Battery => Some("battery"),
        _ => None,
    }
}

/// Size of the icons next to status labels and in the power buttons (logical pixels).
pub const CHROME_ICON: i32 = 16;
/// Size of the icons in the power buttons (they are narrow).
pub const POWER_ICON: i32 = 12;
/// Size of a system tray icon (logical pixels).
pub const TRAY_ICON: i32 = 20;

/// Width of the workspace dots (logical pixels), 0 with a single workspace.
fn workspace_dots_width(count: usize) -> f32 {
    if count < 2 {
        0.0
    } else {
        count as f32 * 9.0 + (count as f32 - 1.0) * 8.0 + 14.0
    }
}

/// The status labels (volume, battery) right of the task buttons and left of the workspace dots,
/// drawn over the bar. Returns where each one is, in logical x pixels, for hit testing.
pub fn draw_status(
    text: &mut TextRenderer,
    buf: &mut [u8],
    width: i32,
    height: i32,
    s: i32,
    items: &[(StatusKind, String)],
    workspaces: usize,
) -> Vec<(StatusKind, i32, i32)> {
    let sf = s as f32;
    let pal = crate::theme::palette();
    let mut right = width as f32 - (112.0 + workspace_dots_width(workspaces)) * sf;
    let mut rects = Vec::new();
    for (kind, label) in items {
        // a tray item is only its picture
        if let StatusKind::Tray(_) = kind {
            let key = label.split('|').next().unwrap_or("");
            let px = TRAY_ICON * s;
            let icon = chrome_icon(key, px).or_else(|| chrome_icon("application-x-executable", px));
            let x0 = right as i32 - px;
            if let Some(i) = &icon {
                crate::icons::blit_icon(buf, width, height, i, x0, (height - i.size) / 2, 1.0);
            }
            rects.push((*kind, x0 / s - 2, (x0 + px) / s + 2));
            right = x0 as f32 - 6.0 * sf;
            continue;
        }
        let Some(img) = text.line(label, 220 * s, 12.5 * sf, pal.ink, Some(([255, 255, 255], 0.8)), 2 * s) else { continue };
        // a small icon before the words, where there is one
        let icon = status_icon_name(*kind, label).and_then(|n| chrome_icon(n, CHROME_ICON * s));
        let icon_w = icon.as_ref().map(|i| i.size + 3 * s).unwrap_or(0);
        let x0 = right as i32 - img.width - icon_w;
        if let Some(i) = &icon {
            crate::icons::blit_icon(buf, width, height, i, x0, (height - i.size) / 2, 1.0);
        }
        blit(buf, width, height, &img, x0 + icon_w, (height - img.height) / 2);
        rects.push((*kind, x0 / s, (x0 + icon_w + img.width) / s));
        right = x0 as f32 - 12.0 * sf;
    }
    // the window buttons end a little before the leftmost label
    let room = (width as f32 - right) / sf + 4.0;
    RIGHT_ROOM.with(|r| r.set((room.ceil() as i32).max(CLOCK_ROOM)));
    rects
}

/// The icon of a power button.
fn power_icon_name(id: &str) -> &'static str {
    match id {
        "lock" => "system-lock-screen",
        "logout" => "system-log-out",
        "sleep" => "system-suspend",
        "restart" => "system-reboot",
        _ => "system-shutdown",
    }
}

pub const MENU_W: i32 = 420;
pub const MENU_ROW: i32 = 30;
/// Size of the icons in the start menu (logical pixels).
pub const MENU_ICON: i32 = 22;
const MENU_PAD: i32 = 8;
pub const MENU_MAX_ROWS: usize = 16;
const SEARCH_H: i32 = 38;
/// the power row at the bottom of the menu
pub const POWER_H: i32 = 40;

// How many programs the start menu shows at most: `MENU_MAX_ROWS`, fewer on a short screen. The
// task bar draws on one thread.
thread_local! {
    static MENU_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(MENU_MAX_ROWS) };
}

pub fn menu_max_rows() -> usize {
    MENU_ROWS.with(|r| r.get())
}

/// How many rows fit above the task bar on a monitor `logical_height` high (and at least 3).
pub fn rows_for_height(logical_height: i32) -> usize {
    let fixed = HEIGHT + 4 + 8 + SEARCH_H + 2 * MENU_PAD + POWER_H;
    (((logical_height - fixed) / MENU_ROW).max(3) as usize).min(MENU_MAX_ROWS)
}

/// Fit the start menu to a monitor `logical_height` high.
pub fn fit_menu_to_height(logical_height: i32) {
    MENU_ROWS.with(|r| r.set(rows_for_height(logical_height)));
}

/// Size of the start menu: a search field and room for `menu_max_rows()` entries, always.
pub fn menu_size() -> (i32, i32) {
    (MENU_W, SEARCH_H + menu_max_rows() as i32 * MENU_ROW + 2 * MENU_PAD + POWER_H)
}

/// The rectangle of power button `i` (x0, y0, x1, y1), in menu pixels.
pub fn power_rect(i: usize) -> (i32, i32, i32, i32) {
    let n = crate::power::ACTIONS.len() as i32;
    let gap = 4;
    let w = (MENU_W - 2 * MENU_PAD - (n - 1) * gap) / n;
    let (_, h) = menu_size();
    let x0 = MENU_PAD + i as i32 * (w + gap);
    (x0, h - POWER_H + 4, x0 + w, h - MENU_PAD)
}

/// Which power button is at this point of the menu?
pub fn power_at(x: f64, y: f64) -> Option<usize> {
    (0..crate::power::ACTIONS.len()).find(|&i| {
        let (x0, y0, x1, y1) = power_rect(i);
        x >= x0 as f64 && x < x1 as f64 && y >= y0 as f64 && y < y1 as f64
    })
}

/// What the power row looks like: the button under the pointer and the one waiting for a second click.
#[derive(Default, Clone, Copy)]
pub struct PowerView {
    pub hover: Option<usize>,
    pub armed: Option<usize>,
}

/// Which menu row (of `n` shown) is at height `y`?
pub fn menu_row_at(y: f64, n: usize) -> Option<usize> {
    let row = ((y - (MENU_PAD + SEARCH_H) as f64) / MENU_ROW as f64).floor();
    (row >= 0.0 && (row as usize) < n.min(menu_max_rows())).then_some(row as usize)
}

/// The start menu at `s` pixels per logical pixel: a glass sheet with rounded corners and one
/// row per entry. The picture is `menu_size()` times `s`.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu(text: &mut TextRenderer, s: i32, query: &str, names: &[String], icons: &[Option<&crate::icons::Icon>], hover: Option<usize>, power: &PowerView) -> Vec<u8> {
    let (lw, lh) = menu_size();
    let (w, h) = (lw * s, lh * s);
    let sf = s as f32;
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let pal = crate::theme::palette();
    let rad = (12.0 - 12.0 * pal.solid) * sf;
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - w as f32 / 2.0).abs() - (w as f32 / 2.0 - rad);
            let dy = (y as f32 + 0.5 - h as f32 / 2.0).abs() - (h as f32 / 2.0 - rad);
            let d = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - rad;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let t = y as f32 / h as f32;
            let i = ((y * w + x) * 4) as usize;
            let px = &mut buf[i..i + 4];
            // the same light glass as the task bar and the window frames: a sheen at the top, a
            // bright rim with a thin shade inside it
            blend(px, pal.light(0.42 + 0.18 * (1.0 - t)), (0.82 + 0.08 * t + 0.10 * pal.solid) * cover);
            if t < 0.2 {
                blend(px, [1.0, 1.0, 1.0], 0.30 * (1.0 - t * 5.0) * cover * pal.shine.max(0.25));
            }
            if d > -1.5 * sf {
                blend(px, [1.0, 1.0, 1.0], 0.85 * cover);
            } else if d > -2.5 * sf {
                blend(px, pal.dark(0.5), 0.28 * cover);
            }
        }
    }
    // the search field: a pale pill with what has been typed (or a hint)
    let (sx0, sy0, sw, sh) = (MENU_PAD * s, MENU_PAD * s, w - 2 * MENU_PAD * s, (SEARCH_H - 6) * s);
    for y in 0..sh {
        for x in 0..sw {
            let dx = (x as f32 + 0.5 - sw as f32 / 2.0).abs() - (sw as f32 / 2.0 - sh as f32 / 2.0);
            let dy = (y as f32 + 0.5 - sh as f32 / 2.0).abs();
            let d = dx.max(0.0).hypot(dy) - sh as f32 / 2.0;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover > 0.0 {
                let i = (((sy0 + y) * w + sx0 + x) * 4) as usize;
                blend(&mut buf[i..i + 4], [1.0, 1.0, 1.0], 0.78 * cover);
                if d > -1.2 * sf {
                    blend(&mut buf[i..i + 4], [0.1, 0.3, 0.6], 0.45 * cover);
                }
            }
        }
    }
    let (shown_text, color) = if query.is_empty() { ("Search programs", [90, 120, 160]) } else { (query, [10, 40, 90]) };
    if let Some(img) = text.line(shown_text, sw - 28 * s, 14.0 * sf, color, None, 0) {
        blit(&mut buf, w, h, &img, sx0 + 14 * s, sy0 + (sh - img.height) / 2);
    }
    // the list: a light, almost white sheet, as in the Aero start menu
    {
        let (_, mh) = menu_size();
        let (lx0, ly0, lx1, ly1) = (MENU_PAD * s, (MENU_PAD + SEARCH_H - 2) * s, w - MENU_PAD * s, (mh - POWER_H - 2) * s);
        let lrad = 6.0 * sf;
        for y in ly0..ly1 {
            for x in lx0..lx1 {
                let dx = (x as f32 + 0.5 - (lx0 + lx1) as f32 / 2.0).abs() - ((lx1 - lx0) as f32 / 2.0 - lrad);
                let dy = (y as f32 + 0.5 - (ly0 + ly1) as f32 / 2.0).abs() - ((ly1 - ly0) as f32 / 2.0 - lrad);
                let d = dx.max(0.0).hypot(dy.max(0.0)) - lrad;
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let idx = ((y * w + x) * 4) as usize;
                let t = (y - ly0) as f32 / (ly1 - ly0) as f32;
                blend(&mut buf[idx..idx + 4], [0.985 - 0.03 * t, 0.99 - 0.02 * t, 1.0], 0.96 * cover);
                if d > -1.2 * sf {
                    blend(&mut buf[idx..idx + 4], [0.55, 0.65, 0.78], 0.6 * cover);
                }
            }
        }
    }
    for (i, name) in names.iter().take(menu_max_rows()).enumerate() {
        let y0 = (MENU_PAD + SEARCH_H + i as i32 * MENU_ROW) * s;
        if hover == Some(i) {
            // the Aero hover: a pale blue gradient with a blue rim and round corners
            let (hx0, hx1, hy0, hy1) = ((MENU_PAD + 4) * s, w - (MENU_PAD + 4) * s, y0 + s, y0 + (MENU_ROW - 1) * s);
            let hrad = 3.0 * sf;
            for y in hy0..hy1 {
                for x in hx0..hx1 {
                    let dx = (x as f32 + 0.5 - (hx0 + hx1) as f32 / 2.0).abs() - ((hx1 - hx0) as f32 / 2.0 - hrad);
                    let dy = (y as f32 + 0.5 - (hy0 + hy1) as f32 / 2.0).abs() - ((hy1 - hy0) as f32 / 2.0 - hrad);
                    let d = dx.max(0.0).hypot(dy.max(0.0)) - hrad;
                    let cover = (0.5 - d).clamp(0.0, 1.0);
                    if cover <= 0.0 {
                        continue;
                    }
                    let t = (y - hy0) as f32 / (hy1 - hy0) as f32;
                    let idx = ((y * w + x) * 4) as usize;
                    blend(&mut buf[idx..idx + 4], [0.90 - 0.08 * t, 0.95 - 0.05 * t, 1.0], cover);
                    if d > -1.2 * sf {
                        blend(&mut buf[idx..idx + 4], [0.45, 0.65, 0.9], 0.8 * cover);
                    }
                }
            }
        }
        let mut tx = (MENU_PAD + 12) * s;
        if let Some(Some(icon)) = icons.get(i) {
            crate::icons::blit_icon(&mut buf, w, h, icon, tx, y0 + (MENU_ROW * s - icon.size) / 2, 1.0);
            tx += icon.size + 10 * s;
        }
        if let Some(img) = text.line(name, w - tx - (MENU_PAD + 8) * s, 14.0 * sf, [24, 34, 52], None, 0) {
            blit(&mut buf, w, h, &img, tx, y0 + (MENU_ROW * s - img.height) / 2);
        }
    }
    // the power row: a thin line and five small glass buttons
    {
        let (_, mh) = menu_size();
        let ly = (mh - POWER_H) * s;
        for x in MENU_PAD * s..w - MENU_PAD * s {
            let idx = ((ly * w + x) * 4) as usize;
            blend(&mut buf[idx..idx + 4], pal.dark(0.5), 0.30);
        }
        // icons in the buttons only if every label still fits next to its icon (all or none)
        let fits = crate::power::ACTIONS.iter().enumerate().all(|(i, a)| {
            let (x0, _, x1, _) = power_rect(i);
            let room = (x1 - x0) * s - 2 * s;
            let ico = chrome_icon(power_icon_name(a.id), POWER_ICON * s).map(|i| i.size + 2 * s);
            let wide = ["Sure?", a.label].iter().map(|l| text.line(l, room, 10.0 * sf, [0, 0, 0], None, 0).map(|t| t.width).unwrap_or(0)).max().unwrap_or(0);
            ico.is_some_and(|w| w + wide <= room)
        });
        for (i, action) in crate::power::ACTIONS.iter().enumerate() {
            let (x0, y0, x1, y1) = power_rect(i);
            let (x0, y0, x1, y1) = (x0 * s, y0 * s, x1 * s, y1 * s);
            let rad = 7.0f32 * sf;
            let armed = power.armed == Some(i);
            let hot = power.hover == Some(i);
            for y in y0..y1 {
                for x in x0..x1 {
                    let dx = (x as f32 + 0.5 - (x0 + x1) as f32 / 2.0).abs() - ((x1 - x0) as f32 / 2.0 - rad);
                    let dy = (y as f32 + 0.5 - (y0 + y1) as f32 / 2.0).abs() - ((y1 - y0) as f32 / 2.0 - rad);
                    let d = dx.max(0.0).hypot(dy.max(0.0)) - rad;
                    let cover = (0.5 - d).clamp(0.0, 1.0);
                    if cover <= 0.0 {
                        continue;
                    }
                    let t = (y - y0) as f32 / (y1 - y0) as f32;
                    let idx = ((y * w + x) * 4) as usize;
                    let (base, a) = if armed {
                        ([0.85, 0.22, 0.18], 0.90 - 0.15 * t)
                    } else if hot {
                        (pal.light(0.2), 0.75 - 0.2 * t)
                    } else {
                        ([1.0, 1.0, 1.0], 0.40 - 0.16 * t)
                    };
                    blend(&mut buf[idx..idx + 4], base, a * cover);
                    if t < 0.45 {
                        blend(&mut buf[idx..idx + 4], [1.0, 1.0, 1.0], 0.20 * (1.0 - t / 0.45) * cover * pal.shine.max(0.25));
                    }
                    if d > -1.2 * sf {
                        blend(&mut buf[idx..idx + 4], [1.0, 1.0, 1.0], 0.85 * cover);
                    } else if d > -2.2 * sf {
                        blend(&mut buf[idx..idx + 4], pal.dark(0.55), 0.22 * cover);
                    }
                }
            }
            let label = if armed { "Sure?" } else { action.label };
            let color = if armed { [255, 255, 255] } else { pal.ink };
            let glow = if armed { Some(([90, 10, 5], 0.6)) } else { Some(([255, 255, 255], 0.8)) };
            // a small icon before the word, when the two fit side by side in the button
            let room = x1 - x0 - 2 * s;
            let icon = chrome_icon(power_icon_name(action.id), POWER_ICON * s);
            if let Some(img) = text.line(label, room, 10.0 * sf, color, glow, s) {
                let with_icon = icon.as_ref().filter(|_| fits);
                let icon_w = with_icon.map(|i| i.size + 2 * s).unwrap_or(0);
                let gx = x0 + (x1 - x0 - icon_w - img.width) / 2;
                if let Some(i) = with_icon {
                    crate::icons::blit_icon(&mut buf, w, h, i, gx, y0 + (y1 - y0 - i.size) / 2, 1.0);
                }
                blit(&mut buf, w, h, &img, gx + icon_w, y0 + (y1 - y0 - img.height) / 2);
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_tile_is_pale_with_colour_and_the_strip_is_light_blue_with_a_clock() {
        let mut text = TextRenderer::default();
        let (w, h) = (400, HEIGHT);
        let buf = draw(&mut text, w, h, 1, false, "12:34", &[Task { title: "Files".into(), active: true, ..Default::default() }, Task { title: "Terminal".into(), ..Default::default() }]);
        let at = |x: i32, y: i32| {
            let i = ((y * w + x) * 4) as usize;
            [buf[i] as i32, buf[i + 1] as i32, buf[i + 2] as i32, buf[i + 3] as i32]
        };
        let (cx, cy) = (ORB_MARGIN + ORB_SIZE / 2, h / 2);
        let tile = at(cx, cy + 12);
        assert!(tile[0] > 150 && tile[1] > 170 && tile[2] > 200, "a pale glass tile: {tile:?}");
        // the red bubble at the upper left of the mark
        let bubble = at(cx - (0.27 * ORB_SIZE as f32 / 2.0) as i32, cy - (0.27 * ORB_SIZE as f32 / 2.0) as i32);
        assert!(bubble[0] > bubble[1] + 60, "the mark has a red bubble: {bubble:?}");
        let strip = at(150, h - 4);
        assert!(strip[2] > strip[0] && strip[0] * 255 / strip[3].max(1) > 120 && strip[3] > 100, "a light blue translucent strip: {strip:?}");
        // dark ink where the clock is (if the machine has fonts at all)
        let mut ink = 0;
        for y in 0..h {
            for x in (w - 90)..(w - 10) {
                if at(x, y)[0] < 110 {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0 || text.line("x", 50, 16.0, [0; 3], None, 0).is_none(), "the clock leaves ink");
        // two task buttons: the first is the active, lighter one
        let (a, b) = (at(TASKS_LEFT + 6, h - 9), at(TASKS_LEFT + 6 + TASK_MAX_W + TASK_GAP, h - 9));
        assert!(a[0] > b[0], "the active task is lighter: {a:?} {b:?}");
        assert_eq!(task_at((TASKS_LEFT + 5) as f64, 20.0, w, h, 2), Some(0));
        assert_eq!(task_at((TASKS_LEFT + TASK_MAX_W + TASK_GAP + 5) as f64, 20.0, w, h, 2), Some(1));
        assert_eq!(task_at(300.0, 2.0, w, h, 2), None);
        assert!(on_orb(22.0, 20.0, h) && !on_orb(100.0, 20.0, h));
    }

    #[test]
    fn the_menu_has_rows_a_highlight_and_rounded_corners() {
        let mut text = TextRenderer::default();
        let names: Vec<String> = ["Files", "Terminal", "Settings"].iter().map(|s| s.to_string()).collect();
        let (w, h) = menu_size();
        let buf = draw_menu(&mut text, 1, "", &names, &[], Some(1), &PowerView::default());
        let at = |x: i32, y: i32| {
            let i = ((y * w + x) * 4) as usize;
            [buf[i] as i32, buf[i + 1] as i32, buf[i + 2] as i32, buf[i + 3] as i32]
        };
        assert_eq!(at(0, 0)[3], 0, "the corner is transparent");
        assert!(at(w / 2, h / 2)[3] > 100, "the sheet is translucent, not empty");
        let (hi, plain) = (at(w - 20, MENU_PAD + SEARCH_H + MENU_ROW + 14), at(w - 20, MENU_PAD + SEARCH_H + 14));
        assert!(plain[0] > 230 && plain[1] > 230, "the list is a light sheet: {plain:?}");
        assert!(hi[2] - hi[0] > plain[2] - plain[0] + 15, "the hovered row is pale blue: {hi:?} {plain:?}");
        assert_eq!(menu_row_at((MENU_PAD + SEARCH_H + MENU_ROW + 3) as f64, 3), Some(1));
        assert_eq!(menu_row_at(2.0, 3), None, "the search field is not a row");
        assert_eq!(menu_row_at((MENU_PAD + SEARCH_H + 3 * MENU_ROW + 3) as f64, 3), None, "below the last row");
    }

    #[test]
    fn the_status_labels_are_drawn_left_of_the_dots_and_can_be_found_again() {
        let mut text = TextRenderer::default();
        let (w, h) = (600, HEIGHT);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let items = vec![(StatusKind::Volume, "Vol 45%".to_string()), (StatusKind::Battery, "Bat 87%".to_string())];
        let rects = draw_status(&mut text, &mut buf, w, h, 1, &items, 4);
        // the window buttons now stop before the labels
        let (_, x1, _, _) = task_rect(0, 1, w, h);
        assert!(rects.iter().all(|r| x1 < r.1) || rects.is_empty(), "buttons end at {x1}, labels {rects:?}");
        if text.line("x", 50, 12.0, [0, 0, 0], None, 0).is_none() {
            return; // no fonts on this machine
        }
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].0, StatusKind::Volume);
        assert!(rects[0].1 > rects[1].2 && rects[0].2 <= w - 112 - 60, "volume sits left of battery, both left of the dots: {rects:?}");
        assert!(buf.iter().any(|&b| b > 0), "something was drawn");
    }

    #[test]
    fn the_workspace_dots_show_which_one_is_on_and_stay_away_from_a_single_workspace() {
        let (w, h) = (400, HEIGHT);
        let mut plain = vec![0u8; (w * h * 4) as usize];
        draw_workspaces(&mut plain, w, h, 1, 1, 1);
        assert!(plain.iter().all(|&b| b == 0), "one workspace: nothing to show");
        let mut a = vec![0u8; (w * h * 4) as usize];
        let mut b = a.clone();
        draw_workspaces(&mut a, w, h, 1, 1, 4);
        draw_workspaces(&mut b, w, h, 1, 3, 4);
        assert!(a.iter().any(|&p| p > 0) && a != b, "the lit dot moves");
        // the first dot is brighter when it is the current one
        let r = 4.5f32;
        let total = 4.0 * (2.0 * r) + 3.0 * 8.0;
        let cx = (w as f32 - 112.0 - total + r) as i32;
        let at = |buf: &Vec<u8>| buf[(((h / 2) * w + cx) * 4 + 3) as usize];
        assert!(at(&a) > at(&b) + 100, "lit {} vs ring {}", at(&a), at(&b));
    }

    #[test]
    fn the_power_row_has_five_buttons_that_can_be_hit_and_do_not_overlap_the_rows() {
        let (w, h) = menu_size();
        assert_eq!(w, MENU_W);
        let mut seen = Vec::new();
        for i in 0..crate::power::ACTIONS.len() {
            let (x0, y0, x1, y1) = power_rect(i);
            assert!(x1 > x0 + 40 && y1 > y0 + 20 && x1 <= w && y1 <= h, "button {i} fits: {:?}", (x0, y0, x1, y1));
            assert_eq!(power_at((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0), Some(i));
            seen.push(x0);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 5);
        assert_eq!(power_at(5.0, 5.0), None);
        assert_eq!(menu_row_at((h - 20) as f64, menu_max_rows()), None, "the power row is not a program row");
    }

    #[test]
    fn windows_share_a_button_with_their_program_and_pinned_programs_come_first() {
        use crate::apps::App;
        let app = |id: &str, name: &str, exec: &str| App { id: id.into(), name: name.into(), exec: exec.into(), ..Default::default() };
        let apps = vec![app("firefox", "Firefox", "firefox"), app("foot", "Foot", "foot"), app("org.gnome.Files", "Files", "nautilus")];
        let win = |id: &str, title: &str, active: bool, minimized: bool| WinInfo { app_id: id.into(), title: title.into(), active, minimized };
        let windows = vec![
            win("foot", "~", false, true),
            win("firefox", "Home", false, false),
            win("foot", "htop", true, false),
            win("mystery", "X", false, false),
            win("mystery", "Y", false, false),
            win("", "a", false, false),
            win("", "b", false, false),
        ];
        let pinned = vec!["files".to_string(), "firefox".to_string(), "not-installed".to_string(), "FILES".to_string()];
        let groups = group_windows(&windows, &apps, &pinned);
        let shape: Vec<(Option<usize>, Vec<usize>, bool)> = groups.iter().map(|g| (g.app, g.windows.clone(), g.pinned)).collect();
        assert_eq!(
            shape,
            vec![
                (Some(2), vec![], true),      // Files: pinned, not running
                (Some(0), vec![1], true),     // Firefox: pinned and running
                (Some(1), vec![0, 2], false), // Foot: two windows, one button
                (None, vec![3, 4], false),    // an unknown program's windows share one by its name
                (None, vec![5], false),       // windows that name nothing are a button each
                (None, vec![6], false),
            ],
            "a program not installed is not pinned, and one pinned twice is shown once"
        );
        let foot = groups[2].task(&windows, &apps);
        assert_eq!((foot.windows, foot.active, foot.minimized, foot.idle, foot.title.as_str()), (2, true, false, false, "~"), "lit if any window is, faint only if all are minimized");
        let files = groups[0].task(&windows, &apps);
        assert_eq!((files.windows, files.idle, files.title.as_str()), (0, true, "Files"), "a pinned program that is not running shows its name");
        assert_eq!(groups[2].tooltip(&windows, &apps), vec!["~", "htop"]);
        assert_eq!(groups[0].tooltip(&windows, &apps), vec!["Files"]);
        assert_eq!(groups[4].tooltip(&windows, &apps), vec!["a"]);
    }

    #[test]
    fn a_pinned_idle_program_is_only_an_icon_and_a_second_window_adds_a_number() {
        let mut text = TextRenderer::default();
        let (w, h) = (400, HEIGHT);
        let icon = crate::icons::Icon { size: TASK_ICON, rgba: vec![255; (TASK_ICON * TASK_ICON * 4) as usize] };
        let tasks = [
            Task { title: "A".into(), icon: Some(icon.clone()), idle: true, ..Default::default() },
            Task { title: "B".into(), icon: Some(icon.clone()), windows: 1, ..Default::default() },
            Task { title: "C".into(), icon: Some(icon), windows: 3, ..Default::default() },
        ];
        let buf = draw(&mut text, w, h, 1, false, "12:34", &tasks);
        let at = |x: i32, y: i32| {
            let i = ((y * w + x) * 4) as usize;
            [buf[i] as i32, buf[i + 1] as i32, buf[i + 2] as i32]
        };
        let corner = |i: usize| {
            let (x0, _, y0, _) = task_rect(i, 3, w, h);
            at(x0 + 2, y0 + 9) // inside the tile's rounded corner area, away from the icon
        };
        let bar_at = |i: usize| {
            let (_, _, y0, _) = task_rect(i, 3, w, h);
            at(300, y0 + 9)
        };
        let bar = bar_at(0);
        assert!((corner(0)[0] - bar[0]).abs() < 8, "an idle pinned program has no tile: {:?} {:?}", corner(0), bar);
        assert!(corner(1)[0] > bar[0] + 8, "a running one does: {:?} {:?}", corner(1), bar);
        // the badge of the third sits at its lower right corner and is lighter than the tile there
        let (_, x1, _, y1) = task_rect(2, 3, w, h);
        let badge = at(x1 - 9, y1 - 9);
        let plain = {
            let (_, x1b, _, y1b) = task_rect(1, 3, w, h);
            at(x1b - 9, y1b - 9)
        };
        assert_ne!(badge, plain, "a number marks a program with several windows");
    }

    #[test]
    fn the_menu_has_fewer_rows_on_a_short_screen() {
        assert_eq!(rows_for_height(1080), MENU_MAX_ROWS, "a tall screen shows them all");
        assert!(rows_for_height(400) < MENU_MAX_ROWS && rows_for_height(400) >= 3, "{}", rows_for_height(400));
        assert_eq!(rows_for_height(100), 3, "never fewer than three");
        // the menu plus the bar and the gap above it always fit
        for h in [320, 400, 500, 600, 768, 800] {
            fit_menu_to_height(h);
            let (_, mh) = menu_size();
            assert!(mh + HEIGHT + 12 <= h || menu_max_rows() == 3, "height {h}: menu {mh}");
        }
        fit_menu_to_height(2000);
    }

    #[test]
    fn at_scale_two_the_bar_and_menu_are_twice_as_many_pixels_and_the_orb_is_twice_as_wide() {
        let mut text = TextRenderer::default();
        // the start button's width in pixels along the middle row: from its first to its last
        // dark outline pixel
        let mut orb_width = |s: i32| {
            let (w, h) = (400 * s, HEIGHT * s);
            let buf = draw(&mut text, w, h, s, false, "", &[]);
            let y = h / 2;
            let dark: Vec<i32> = (0..(ORB_SIZE + 2 * ORB_MARGIN) * s)
                .filter(|&x| {
                    let i = ((y * w + x) * 4) as usize;
                    buf[i] as i32 + buf[i + 1] as i32 + buf[i + 2] as i32 > 0 && (buf[i] as i32 + buf[i + 1] as i32 + buf[i + 2] as i32) < 330
                })
                .collect();
            dark.last().unwrap_or(&0) - dark.first().unwrap_or(&0) + 1
        };
        let (one, two) = (orb_width(1), orb_width(2));
        assert!((one - ORB_SIZE).abs() <= 4 && (two - 2 * ORB_SIZE).abs() <= 6, "the orb is about 32 px wide at 1x and 64 px at 2x: {one} {two}");
        let (mw, mh) = menu_size();
        assert_eq!(draw_menu(&mut text, 2, "", &[], &[], None, &PowerView::default()).len() as i32, mw * 2 * mh * 2 * 4);
    }

    #[test]
    fn status_items_and_power_buttons_get_icons_that_fit_what_they_say() {
        assert_eq!(status_icon_name(StatusKind::Volume, "Vol muted"), Some("audio-volume-muted"));
        assert_eq!(status_icon_name(StatusKind::Volume, "Vol 80%"), Some("audio-volume-high"));
        assert_eq!(status_icon_name(StatusKind::Volume, "Vol 45%"), Some("audio-volume-medium"));
        assert_eq!(status_icon_name(StatusKind::Volume, "Vol 10%"), Some("audio-volume-low"));
        assert_eq!(status_icon_name(StatusKind::Network, "Wired"), Some("network-wired"));
        assert_eq!(status_icon_name(StatusKind::Network, "HomeWifi 80%"), Some("network-wireless"));
        assert_eq!(status_icon_name(StatusKind::Cpu, "CPU 3%"), None);
        for action in crate::power::ACTIONS {
            assert!(chrome_icon(power_icon_name(action.id), POWER_ICON).is_some(), "an icon for {}", action.id);
        }
        // an item with an icon is wider than the same label without one
        let mut text = TextRenderer::default();
        let (w, h) = (900, HEIGHT);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let items = vec![(StatusKind::Cpu, "CPU 3%".to_string()), (StatusKind::Volume, "CPU 3%".to_string())];
        let rects = draw_status(&mut text, &mut buf, w, h, 1, &items, 1);
        if rects.len() == 2 {
            let (cpu, vol) = (rects[0].2 - rects[0].1, rects[1].2 - rects[1].1);
            assert!(vol >= cpu + CHROME_ICON, "the icon makes room: {cpu} vs {vol}");
        }
    }

    #[test]
    fn a_tray_item_is_drawn_as_its_picture_and_can_be_hit() {
        let dir = tempfile::tempdir().unwrap();
        let icon = dir.path().join("red.png");
        crate::tray::write_png(&icon, 32, 32, &[255u8, 0, 0, 255].repeat(32 * 32)).unwrap();
        let mut text = TextRenderer::default();
        let (w, h) = (900, HEIGHT);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let key = format!("{}|My program", icon.display());
        let items = vec![(StatusKind::Tray(0), key.clone()), (StatusKind::Tray(1), "no-such-icon-anywhere|Other".to_string()), (StatusKind::Cpu, "CPU 3%".to_string())];
        let rects = draw_status(&mut text, &mut buf, w, h, 1, &items, 1);
        assert_eq!(rects.iter().map(|r| r.0).collect::<Vec<_>>(), vec![StatusKind::Tray(0), StatusKind::Tray(1), StatusKind::Cpu], "in order, from the right");
        let (_, x0, x1) = rects[0];
        assert!(x1 - x0 >= TRAY_ICON && x1 - x0 <= TRAY_ICON + 8, "an icon's width: {}", x1 - x0);
        assert!(rects[0].1 > rects[1].1 && rects[1].1 > rects[2].1, "each further left");
        // red pixels where the first icon is
        let red = (x0..x1).flat_map(|x| (0..h).map(move |y| (x, y))).filter(|&(x, y)| {
            let i = ((y * w + x) * 4) as usize;
            buf[i] > 200 && buf[i + 1] < 40 && buf[i + 3] > 200
        }).count();
        assert!(red > 150, "the picture is drawn: {red} red pixels");
        // an icon that cannot be found is replaced by a stand-in, not left out
        let (_, y0, y1) = rects[1];
        let any = (y0..y1).flat_map(|x| (0..h).map(move |y| (x, y))).any(|(x, y)| buf[((y * w + x) * 4) as usize + 3] > 100);
        assert!(any, "a stand-in icon");
    }
}
