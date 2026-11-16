//! Openbox themes. A theme is a folder `NAME/openbox-3/` with a text file `themerc` (colours,
//! gradients, bevels, border and padding sizes) and small `.xbm` bitmaps for the buttons. FDE reads
//! it and draws the window frames in that style (title bar and borders in the theme's textures,
//! the title in its colour, the buttons from its bitmaps). The rest of the desktop (task bar, menus)
//! takes its accent colour from the theme's title bar.
//!
//! Not supported: the bottom handle with its grips (its height is not added to the frame), the
//! window icon and the shade, desktop and sticky buttons, menu and on-screen-display colours,
//! pressed and hover looks of the buttons, and fonts (FDE's own font is used).

use std::path::{Path, PathBuf};

/// A colour, 0 .. 255 per channel.
pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relief {
    Flat,
    Raised,
    Sunken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
    Diagonal,
    CrossDiagonal,
    Pyramid,
    MirrorHorizontal,
    SplitVertical,
}

/// One `*.bg` entry of a theme: how a part of the frame is filled.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    /// "Parentrelative": no fill of its own, the part shows what is under it
    pub parent_relative: bool,
    pub relief: Relief,
    /// None: a solid colour
    pub gradient: Option<Direction>,
    /// 0 none, 1 or 2: a one or two pixel light and dark edge (for raised and sunken)
    pub bevel: u8,
    pub interlaced: bool,
    pub border: bool,
    pub color: Rgb,
    pub color_to: Rgb,
    pub color_split: Rgb,
    pub color_to_split: Rgb,
    pub border_color: Rgb,
}

impl Texture {
    pub fn solid(color: Rgb) -> Texture {
        Texture {
            parent_relative: false,
            relief: Relief::Flat,
            gradient: None,
            bevel: 0,
            interlaced: false,
            border: false,
            color,
            color_to: color,
            color_split: color,
            color_to_split: color,
            border_color: color,
        }
    }
}

/// The looks of the frame of a window that has the focus, or one that has not.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub title: Texture,
    pub label: Texture,
    pub label_text: Rgb,
    pub border_color: Rgb,
    pub client_color: Rgb,
    pub button_bg: Texture,
    pub button_image: Rgb,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObTheme {
    pub name: String,
    pub dir: PathBuf,
    pub border_width: u32,
    pub client_padding: u32,
    pub padding_w: u32,
    pub padding_h: u32,
    pub justify: Justify,
    pub active: Look,
    pub inactive: Look,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Left,
    Center,
    Right,
}

/// The height of the label (the text line and the buttons beside it), in pixels at scale 1.
pub const LABEL_HEIGHT: u32 = 14;

impl ObTheme {
    /// Height of the title bar at scale 1.
    pub fn title_height(&self) -> u32 {
        LABEL_HEIGHT + 2 * self.padding_h
    }

    /// Thickness of the frame at the sides and bottom (and above the title) at scale 1.
    pub fn frame_width(&self) -> u32 {
        self.border_width + self.client_padding
    }
}

/// A colour: `#rrggbb`, `#rgb`, `rgb:rr/gg/bb` or a common name.
pub fn parse_rgb(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        return match h.len() {
            6 => Some([u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?]),
            3 => {
                let v = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|x| x * 17);
                Some([v(0)?, v(1)?, v(2)?])
            }
            _ => None,
        };
    }
    if let Some(r) = s.strip_prefix("rgb:") {
        let parts: Vec<&str> = r.split('/').collect();
        if parts.len() == 3 {
            let v = |p: &str| -> Option<u8> {
                let n = u32::from_str_radix(p, 16).ok()?;
                // 1 to 4 hex digits scale to 0 .. 255
                let max = 16u32.pow(p.len() as u32) - 1;
                Some((n * 255 / max.max(1)) as u8)
            };
            return Some([v(parts[0])?, v(parts[1])?, v(parts[2])?]);
        }
        return None;
    }
    match s.to_ascii_lowercase().as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([255, 0, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 0, 255]),
        "gray" | "grey" => Some([190, 190, 190]),
        "darkgray" | "darkgrey" => Some([169, 169, 169]),
        "lightgray" | "lightgrey" => Some([211, 211, 211]),
        "yellow" => Some([255, 255, 0]),
        "orange" => Some([255, 165, 0]),
        _ => None,
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

fn scale_rgb(a: Rgb, k: f32) -> Rgb {
    a.map(|v| (v as f32 * k).round().clamp(0.0, 255.0) as u8)
}

/// The words of a texture line ("Raised Gradient Diagonal Bevel1 Border").
fn parse_texture_words(value: &str, tex: &mut Texture) {
    tex.parent_relative = false;
    tex.gradient = None;
    tex.relief = Relief::Flat;
    tex.bevel = 0;
    tex.interlaced = false;
    tex.border = false;
    for w in value.split_whitespace().map(|w| w.to_ascii_lowercase()) {
        match w.as_str() {
            "parentrelative" => tex.parent_relative = true,
            "flat" => tex.relief = Relief::Flat,
            "raised" => tex.relief = Relief::Raised,
            "sunken" => tex.relief = Relief::Sunken,
            "solid" => tex.gradient = None,
            "gradient" => tex.gradient = tex.gradient.or(Some(Direction::Vertical)),
            "horizontal" => tex.gradient = Some(Direction::Horizontal),
            "vertical" => tex.gradient = Some(Direction::Vertical),
            "diagonal" => tex.gradient = Some(Direction::Diagonal),
            "crossdiagonal" => tex.gradient = Some(Direction::CrossDiagonal),
            "pyramid" => tex.gradient = Some(Direction::Pyramid),
            "mirrorhorizontal" => tex.gradient = Some(Direction::MirrorHorizontal),
            "splitvertical" => tex.gradient = Some(Direction::SplitVertical),
            "bevel1" => tex.bevel = 1,
            "bevel2" => tex.bevel = 2,
            "interlaced" => tex.interlaced = true,
            "border" => tex.border = true,
            _ => {}
        }
    }
}

/// The `key: value` lines of a themerc (`!` and `#` at the start of a line begin a comment),
/// keys in lower case.
pub fn parse_pairs(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('!') || l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

struct Pairs(Vec<(String, String)>);

impl Pairs {
    /// The last value of the first of `keys` that is set (themes repeat keys; the later wins).
    fn get(&self, keys: &[&str]) -> Option<&str> {
        for k in keys {
            if let Some((_, v)) = self.0.iter().rev().find(|(key, _)| key == k) {
                return Some(v.as_str());
            }
        }
        None
    }

    fn color(&self, keys: &[&str]) -> Option<Rgb> {
        self.get(keys).and_then(parse_rgb)
    }

    fn number(&self, keys: &[&str]) -> Option<u32> {
        self.get(keys).and_then(|v| v.trim().parse().ok())
    }

    /// The texture `prefix` ("window.active.title.bg"): its words and its colours. `fallback` is
    /// what it looks like when the theme says nothing.
    fn texture(&self, prefix: &str, fallback: &Texture) -> Texture {
        let mut t = fallback.clone();
        if let Some(words) = self.get(&[prefix]) {
            parse_texture_words(words, &mut t);
        }
        if let Some(c) = self.color(&[&format!("{prefix}.color")]) {
            t.color = c;
        }
        t.color_to = self.color(&[&format!("{prefix}.colorto")]).unwrap_or(t.color);
        t.color_split = self.color(&[&format!("{prefix}.color.splitto")]).unwrap_or_else(|| scale_rgb(t.color, 0.9));
        t.color_to_split = self.color(&[&format!("{prefix}.colorto.splitto")]).unwrap_or_else(|| scale_rgb(t.color_to, 0.9));
        t.border_color = self.color(&[&format!("{prefix}.border.color")]).unwrap_or(t.border_color);
        t
    }
}

impl ObTheme {
    /// Read the text of a themerc. `name` and `dir` say where the theme lives (the buttons'
    /// bitmaps are read from `dir` when the frame is drawn).
    pub fn parse(text: &str, name: &str, dir: &Path) -> ObTheme {
        let p = Pairs(parse_pairs(text));
        let border_width = p.number(&["window.border.width", "border.width"]).unwrap_or(1).min(16);
        let padding_w = p.number(&["padding.width", "window.padding.width"]).unwrap_or(3).min(32);
        let padding_h = p.number(&["padding.height", "window.padding.height"]).unwrap_or(3).min(32);
        let client_padding = p.number(&["window.client.padding.width"]).unwrap_or(padding_w).min(16);
        let justify = match p.get(&["window.label.text.justify"]).map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("center") => Justify::Center,
            Some("right") => Justify::Right,
            _ => Justify::Left,
        };
        let global_border = p.color(&["border.color"]).unwrap_or([0, 0, 0]);
        let look = |which: &str, default_text: Rgb| -> Look {
            let base = Texture::solid([0x80, 0x80, 0x80]);
            let title = p.texture(&format!("window.{which}.title.bg"), &base);
            let label = {
                let mut parent = title.clone();
                parent.parent_relative = true;
                p.texture(&format!("window.{which}.label.bg"), &parent)
            };
            let button_bg = {
                let mut parent = title.clone();
                parent.parent_relative = true;
                p.texture(&format!("window.{which}.button.unpressed.bg"), &parent)
            };
            let label_text = p.color(&[&format!("window.{which}.label.text.color")]).unwrap_or(default_text);
            Look {
                client_color: p.color(&[&format!("window.{which}.client.color"), &format!("window.{which}.frame.color")]).unwrap_or(title.color),
                border_color: p.color(&[&format!("window.{which}.border.color")]).unwrap_or(global_border),
                button_image: p.color(&[&format!("window.{which}.button.unpressed.image.color")]).unwrap_or(label_text),
                title,
                label,
                label_text,
                button_bg,
            }
        };
        ObTheme {
            name: name.to_string(),
            dir: dir.to_path_buf(),
            border_width,
            client_padding,
            padding_w,
            padding_h,
            justify,
            active: look("active", [255, 255, 255]),
            inactive: look("inactive", [200, 200, 200]),
        }
    }

    /// Load the theme in the folder `dir` (which holds `openbox-3/themerc`).
    pub fn load(dir: &Path) -> Result<ObTheme, String> {
        let rc = dir.join("openbox-3").join("themerc");
        let text = std::fs::read_to_string(&rc).map_err(|e| format!("{}: {e}", rc.display()))?;
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("openbox").to_string();
        Ok(ObTheme::parse(&text, &name, dir))
    }

    /// The accent colour of the whole desktop: the middle of the active title bar.
    pub fn accent(&self) -> Rgb {
        let t = &self.active.title;
        mix(t.color, t.color_to, 0.5)
    }
}

/// Where Openbox themes are looked for, best first.
pub fn theme_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = std::env::var_os("FDE_OPENBOX_THEMES") {
        v.push(PathBuf::from(p));
    }
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        v.push(h.join(".themes"));
        v.push(h.join(".local/share/themes"));
    }
    v.push(PathBuf::from("/usr/local/share/themes"));
    v.push(PathBuf::from("/usr/share/themes"));
    v
}

/// A theme name that is only a folder name (no way out of the theme folders).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && name.len() < 100 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.' | '+'))
}

/// The folder of the Openbox theme called `name`, if there is one.
pub fn find_dir(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    dirs.iter().map(|d| d.join(name)).find(|d| d.join("openbox-3").join("themerc").is_file())
}

/// The names of all Openbox themes in `dirs` (each once, in order).
pub fn list_names(dirs: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        let mut names: Vec<String> = rd
            .flatten()
            .filter(|e| e.path().join("openbox-3").join("themerc").is_file())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|n| valid_name(n))
            .collect();
        names.sort();
        for n in names {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- bitmaps

/// A 1-bit picture from an `.xbm` file: width, height and the bits (row by row, 1 = drawn).
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub bits: Vec<bool>,
}

/// Read an X bitmap (C source: `#define x_width 8`, `#define x_height 8`, `static char x_bits[] = { 0x.., ... }`).
pub fn parse_xbm(text: &str) -> Option<Bitmap> {
    let mut width = None;
    let mut height = None;
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("#define") {
            let mut it = rest.split_whitespace();
            let (name, val) = (it.next()?, it.next()?);
            if name.ends_with("_width") {
                width = val.parse::<usize>().ok();
            } else if name.ends_with("_height") {
                height = val.parse::<usize>().ok();
            }
        }
    }
    let (width, height) = (width?, height?);
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return None;
    }
    let body = text.split_once('{')?.1.split_once('}')?.0;
    let bytes: Vec<u8> = body
        .split(',')
        .filter_map(|t| {
            let t = t.trim();
            let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))?;
            u8::from_str_radix(t, 16).ok()
        })
        .collect();
    let row_bytes = width.div_ceil(8);
    if bytes.len() < row_bytes * height {
        return None;
    }
    let mut bits = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            // least significant bit first
            bits.push(bytes[y * row_bytes + x / 8] >> (x % 8) & 1 == 1);
        }
    }
    Some(Bitmap { width, height, bits })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Close,
    Maximize,
    Restore,
    Iconify,
}

impl Glyph {
    fn file(self) -> &'static str {
        match self {
            Glyph::Close => "close.xbm",
            Glyph::Maximize => "max.xbm",
            Glyph::Restore => "max_toggled.xbm",
            Glyph::Iconify => "iconify.xbm",
        }
    }
}

/// A plain 8 x 8 stand-in for a button bitmap the theme does not have.
fn builtin_glyph(g: Glyph) -> Bitmap {
    let pic: [&str; 8] = match g {
        Glyph::Close => ["........", ".##..##.", "..####..", "...##...", "..####..", ".##..##.", "........", "........"],
        Glyph::Maximize => ["........", ".######.", ".#....#.", ".#....#.", ".#....#.", ".######.", "........", "........"],
        Glyph::Restore => ["........", "..#####.", "..#...#.", ".#####..", ".#...#..", ".#####..", "........", "........"],
        Glyph::Iconify => ["........", "........", "........", "........", "........", ".######.", ".######.", "........"],
    };
    Bitmap { width: 8, height: 8, bits: pic.iter().flat_map(|r| r.chars().map(|c| c == '#')).collect() }
}

/// The bitmap of a button: the theme's own, or a plain one.
pub fn glyph_bitmap(theme: &ObTheme, g: Glyph) -> Bitmap {
    let path = theme.dir.join("openbox-3").join(g.file());
    std::fs::read_to_string(path).ok().and_then(|t| parse_xbm(&t)).unwrap_or_else(|| builtin_glyph(g))
}

// ---------------------------------------------------------------- drawing

/// A picture of RGBA8 pixels, premultiplied, top row first.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![0; w * h * 4] }
    }

    fn set(&mut self, x: usize, y: usize, c: Rgb) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 4;
            self.px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.w + x) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    fn clear_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                let i = (yy * self.w + xx) * 4;
                self.px[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    /// `src` (premultiplied) over the canvas at (x, y).
    pub fn over(&mut self, src: &[u8], sw: usize, sh: usize, x: i32, y: i32) {
        for sy in 0..sh {
            for sx in 0..sw {
                let (dx, dy) = (x + sx as i32, y + sy as i32);
                if dx < 0 || dy < 0 || dx as usize >= self.w || dy as usize >= self.h {
                    continue;
                }
                let s = &src[(sy * sw + sx) * 4..(sy * sw + sx) * 4 + 4];
                if s[3] == 0 {
                    continue;
                }
                let i = (dy as usize * self.w + dx as usize) * 4;
                let inv = 255 - s[3] as u32;
                for c in 0..4 {
                    self.px[i + c] = (s[c] as u32 + self.px[i + c] as u32 * inv / 255).min(255) as u8;
                }
            }
        }
    }
}

/// The colour at (x, y) of a `w` x `h` rectangle filled with `t` (without edges).
fn texture_color(t: &Texture, x: usize, y: usize, w: usize, h: usize) -> Rgb {
    let (fx, fy) = (x as f32 / (w.max(2) - 1) as f32, y as f32 / (h.max(2) - 1) as f32);
    let mut c = match t.gradient {
        None => t.color,
        Some(Direction::Vertical) => mix(t.color, t.color_to, fy),
        Some(Direction::Horizontal) => mix(t.color, t.color_to, fx),
        Some(Direction::Diagonal) => mix(t.color, t.color_to, (fx + fy) / 2.0),
        Some(Direction::CrossDiagonal) => mix(t.color, t.color_to, ((1.0 - fx) + fy) / 2.0),
        Some(Direction::Pyramid) => mix(t.color, t.color_to, ((fx - 0.5).abs() + (fy - 0.5).abs()).min(1.0)),
        Some(Direction::MirrorHorizontal) => mix(t.color, t.color_to, 1.0 - (2.0 * fx - 1.0).abs()),
        Some(Direction::SplitVertical) => {
            if fy < 0.5 {
                mix(t.color, t.color_to, fy * 2.0)
            } else {
                mix(t.color_split, t.color_to_split, (fy - 0.5) * 2.0)
            }
        }
    };
    if t.interlaced && y % 2 == 1 {
        c = scale_rgb(c, 0.88);
    }
    c
}

/// Fill the rectangle (x, y, w, h) of `cv` with `t`: its colours, bevel and border.
pub fn fill(cv: &mut Canvas, x: usize, y: usize, w: usize, h: usize, t: &Texture) {
    if w == 0 || h == 0 {
        return;
    }
    for yy in 0..h {
        for xx in 0..w {
            let c = texture_color(t, xx, yy, w, h);
            cv.set(x + xx, y + yy, c);
        }
    }
    // the light and dark edges of a raised (or sunken) texture
    if t.relief != Relief::Flat && t.bevel > 0 {
        let base = t.color;
        let (light, dark) = (mix(base, [255, 255, 255], 0.4), mix(base, [0, 0, 0], 0.4));
        let (top_left, bottom_right) = if t.relief == Relief::Raised { (light, dark) } else { (dark, light) };
        for xx in 0..w {
            cv.set(x + xx, y, top_left);
            cv.set(x + xx, y + h - 1, bottom_right);
        }
        for yy in 0..h {
            cv.set(x, y + yy, top_left);
            cv.set(x + w - 1, y + yy, bottom_right);
        }
        if t.bevel == 2 && w > 4 && h > 4 {
            let (tl2, br2) = (mix(top_left, texture_color(t, 1, 1, w, h), 0.5), mix(bottom_right, texture_color(t, 1, 1, w, h), 0.5));
            for xx in 1..w - 1 {
                cv.set(x + xx, y + 1, tl2);
                cv.set(x + xx, y + h - 2, br2);
            }
            for yy in 1..h - 1 {
                cv.set(x + 1, y + yy, tl2);
                cv.set(x + w - 2, y + yy, br2);
            }
        }
    }
    if t.border {
        for xx in 0..w {
            cv.set(x + xx, y, t.border_color);
            cv.set(x + xx, y + h - 1, t.border_color);
        }
        for yy in 0..h {
            cv.set(x, y + yy, t.border_color);
            cv.set(x + w - 1, y + yy, t.border_color);
        }
    }
}

/// What a button is and where (a rectangle inside the frame picture).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonSpot {
    pub glyph: Glyph,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The title of a window drawn by the caller: premultiplied RGBA, with its size.
pub struct Label<'a> {
    pub rgba: &'a [u8],
    pub w: usize,
    pub h: usize,
}

/// What the frame is drawn for.
pub struct FrameSpec<'a> {
    /// the whole window with its frame, in pixels
    pub w: usize,
    pub h: usize,
    /// pixels per logical pixel (the sizes below are scaled by it)
    pub scale: f32,
    pub focused: bool,
    pub label: Option<Label<'a>>,
    pub buttons: &'a [ButtonSpot],
}

/// Draw the frame of a window: the border, the title bar, the label and the buttons; the place of
/// the client is left see-through.
pub fn render_frame(theme: &ObTheme, spec: &FrameSpec) -> Canvas {
    let s = spec.scale.max(0.5);
    let px = |v: u32| ((v as f32 * s).round() as usize).max(if v > 0 { 1 } else { 0 });
    let look = if spec.focused { &theme.active } else { &theme.inactive };
    let (w, h) = (spec.w, spec.h);
    let mut cv = Canvas::new(w, h);
    let (bw, frame) = (px(theme.border_width), px(theme.frame_width()));
    let title_h = px(theme.title_height());
    // the border, then the client colour inside it
    fill(&mut cv, 0, 0, w, h, &Texture::solid(look.border_color));
    if w > 2 * bw && h > 2 * bw {
        fill(&mut cv, bw, bw, w - 2 * bw, h - 2 * bw, &Texture::solid(look.client_color));
    }
    // the title bar
    let (tx, ty) = (frame, frame);
    let tw = w.saturating_sub(2 * frame);
    fill(&mut cv, tx, ty, tw, title_h, &look.title);
    // the label: its own texture unless it is parent relative
    let (pad_w, pad_h) = (px(theme.padding_w), px(theme.padding_h));
    let buttons_left = spec.buttons.iter().map(|b| b.x).min().unwrap_or((tx + tw) as i32);
    let (lx, ly) = (tx + pad_w, ty + pad_h);
    let lw = ((buttons_left - pad_w as i32) - lx as i32).max(0) as usize;
    let lh = title_h.saturating_sub(2 * pad_h);
    if !look.label.parent_relative {
        fill(&mut cv, lx, ly, lw, lh, &look.label);
    }
    if let Some(label) = &spec.label {
        let x = match theme.justify {
            Justify::Left => lx as i32,
            Justify::Center => lx as i32 + (lw as i32 - label.w as i32) / 2,
            Justify::Right => lx as i32 + lw as i32 - label.w as i32,
        };
        let y = ly as i32 + (lh as i32 - label.h as i32) / 2;
        cv.over(label.rgba, label.w, label.h, x, y);
    }
    // the buttons
    for b in spec.buttons {
        if b.w <= 0 || b.h <= 0 {
            continue;
        }
        if !look.button_bg.parent_relative {
            fill(&mut cv, b.x.max(0) as usize, b.y.max(0) as usize, b.w as usize, b.h as usize, &look.button_bg);
        }
        let bm = glyph_bitmap(theme, b.glyph);
        let k = ((s.round() as i32).max(1)).min(((b.w / bm.width as i32).max(1)).min((b.h / bm.height as i32).max(1)));
        let (gw, gh) = (bm.width as i32 * k, bm.height as i32 * k);
        let (gx, gy) = (b.x + (b.w - gw) / 2, b.y + (b.h - gh) / 2);
        for yy in 0..bm.height {
            for xx in 0..bm.width {
                if bm.bits[yy * bm.width + xx] {
                    for dy in 0..k {
                        for dx in 0..k {
                            let (x, y) = (gx + xx as i32 * k + dx, gy + yy as i32 * k + dy);
                            if x >= 0 && y >= 0 {
                                cv.set(x as usize, y as usize, look.button_image);
                            }
                        }
                    }
                }
            }
        }
    }
    // the client's place is empty (the program draws there)
    if w > 2 * frame && h > 2 * frame + title_h {
        cv.clear_rect(frame, frame + title_h, w - 2 * frame, h - 2 * frame - title_h);
    }
    cv
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEARLOOKS_LIKE: &str = "
! a comment
# another
border.width: 1
padding.width: 3
padding.height: 2
window.client.padding.width: 2
window.active.title.bg: Raised Gradient Vertical Bevel1
window.active.title.bg.color: #4a6ea9
window.active.title.bg.colorTo: #2a3e69
window.inactive.title.bg: Flat Solid
window.inactive.title.bg.color: #bbbbbb
window.active.label.bg: Parentrelative
window.active.label.text.color: #ffffff
window.inactive.label.text.color: rgb:44/44/44
window.active.border.color: #112233
window.inactive.border.color: #999999
window.active.client.color: #cccccc
window.label.text.justify: Center
window.active.button.unpressed.image.color: #ffeedd
";

    fn theme() -> ObTheme {
        ObTheme::parse(CLEARLOOKS_LIKE, "Test", Path::new("/nonexistent"))
    }

    #[test]
    fn colours_in_every_notation_are_read() {
        assert_eq!(parse_rgb("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_rgb("#f80"), Some([255, 136, 0]));
        assert_eq!(parse_rgb("rgb:ff/80/00"), Some([255, 128, 0]));
        assert_eq!(parse_rgb("rgb:f/8/0"), Some([255, 136, 0]));
        assert_eq!(parse_rgb("rgb:ffff/8000/0000"), Some([255, 127, 0]));
        assert_eq!(parse_rgb("White"), Some([255, 255, 255]));
        assert_eq!(parse_rgb("#12"), None);
        assert_eq!(parse_rgb("nonsense"), None);
    }

    #[test]
    fn a_themerc_gives_sizes_colours_and_textures() {
        let t = theme();
        assert_eq!((t.border_width, t.padding_w, t.padding_h, t.client_padding), (1, 3, 2, 2));
        assert_eq!(t.frame_width(), 3);
        assert_eq!(t.title_height(), LABEL_HEIGHT + 4);
        assert_eq!(t.justify, Justify::Center);
        let a = &t.active.title;
        assert_eq!((a.relief, a.gradient, a.bevel), (Relief::Raised, Some(Direction::Vertical), 1));
        assert_eq!((a.color, a.color_to), ([0x4a, 0x6e, 0xa9], [0x2a, 0x3e, 0x69]));
        let i = &t.inactive.title;
        assert_eq!((i.relief, i.gradient), (Relief::Flat, None));
        assert!(t.active.label.parent_relative, "the label has no fill of its own");
        assert_eq!(t.inactive.label_text, [0x44, 0x44, 0x44]);
        assert_eq!(t.active.border_color, [0x11, 0x22, 0x33]);
        assert_eq!(t.active.client_color, [0xcc, 0xcc, 0xcc]);
        assert_eq!(t.active.button_image, [0xff, 0xee, 0xdd]);
        // an empty theme still gives something usable
        let e = ObTheme::parse("", "Empty", Path::new("/x"));
        assert_eq!((e.border_width, e.padding_w), (1, 3));
        assert!(e.title_height() > 10);
        // the accent is the middle of the active title bar
        assert_eq!(t.accent(), [0x3a, 0x56, 0x89]);
    }

    #[test]
    fn the_later_of_two_equal_keys_wins_and_unknown_ones_are_ignored() {
        let t = ObTheme::parse("border.width: 2\nborder.width: 4\nsomething.else: x\nwindow.active.title.bg.color: nonsense\n", "T", Path::new("/x"));
        assert_eq!(t.border_width, 4);
        assert_eq!(t.active.title.color, [0x80, 0x80, 0x80], "a colour that is not one leaves the default");
    }

    #[test]
    fn xbm_bitmaps_are_read_least_significant_bit_first() {
        let text = "#define close_width 8\n#define close_height 2\nstatic unsigned char close_bits[] = {\n 0x81, 0x03 };\n";
        let b = parse_xbm(text).unwrap();
        assert_eq!((b.width, b.height), (8, 2));
        assert_eq!(&b.bits[..8], &[true, false, false, false, false, false, false, true]);
        assert_eq!(&b.bits[8..], &[true, true, false, false, false, false, false, false]);
        // a wider one takes two bytes per row
        let wide = "#define x_width 10\n#define x_height 1\nstatic char x_bits[] = { 0xff, 0x01 };";
        let w = parse_xbm(wide).unwrap();
        assert_eq!(w.bits.iter().filter(|b| **b).count(), 9);
        assert!(parse_xbm("nothing").is_none());
        assert!(parse_xbm("#define a_width 8\n#define a_height 8\nstatic char a[] = { 0x00 };").is_none(), "too few bytes");
    }

    #[test]
    fn gradients_run_between_their_two_colours() {
        let mut t = Texture::solid([0, 0, 0]);
        t.color_to = [200, 100, 50];
        t.gradient = Some(Direction::Vertical);
        assert_eq!(texture_color(&t, 3, 0, 10, 20), [0, 0, 0]);
        assert_eq!(texture_color(&t, 3, 19, 10, 20), [200, 100, 50]);
        t.gradient = Some(Direction::Horizontal);
        assert_eq!(texture_color(&t, 0, 7, 10, 20), [0, 0, 0]);
        assert_eq!(texture_color(&t, 9, 7, 10, 20), [200, 100, 50]);
        let mid = texture_color(&t, 5, 0, 11, 20);
        assert!(mid[0] > 80 && mid[0] < 120);
        // a split gradient restarts in the middle with its own colours
        t.gradient = Some(Direction::SplitVertical);
        t.color_split = [10, 10, 10];
        t.color_to_split = [20, 20, 20];
        assert_eq!(texture_color(&t, 0, 19, 4, 20), [20, 20, 20]);
        assert_eq!(texture_color(&t, 0, 10, 4, 20)[0] < 30, true);
    }

    #[test]
    fn a_raised_bevel_is_light_on_top_and_dark_below() {
        let mut cv = Canvas::new(20, 12);
        let mut t = Texture::solid([100, 100, 100]);
        t.relief = Relief::Raised;
        t.bevel = 1;
        fill(&mut cv, 0, 0, 20, 12, &t);
        assert!(cv.pixel(5, 0)[0] > 100 && cv.pixel(0, 5)[0] > 100, "light top and left");
        assert!(cv.pixel(5, 11)[0] < 100 && cv.pixel(19, 5)[0] < 100, "dark bottom and right");
        assert_eq!(cv.pixel(5, 5), [100, 100, 100, 255]);
        t.relief = Relief::Sunken;
        fill(&mut cv, 0, 0, 20, 12, &t);
        assert!(cv.pixel(5, 0)[0] < 100 && cv.pixel(5, 11)[0] > 100, "sunken is the other way round");
        let mut b = Texture::solid([100, 100, 100]);
        b.border = true;
        b.border_color = [1, 2, 3];
        fill(&mut cv, 0, 0, 20, 12, &b);
        assert_eq!(cv.pixel(0, 0)[..3], [1, 2, 3]);
        assert_eq!(cv.pixel(10, 6)[..3], [100, 100, 100]);
    }

    #[test]
    fn the_frame_has_a_border_a_title_bar_buttons_and_a_hole_for_the_client() {
        let t = theme();
        let (w, h) = (300usize, 200usize);
        let title_h = t.title_height() as usize;
        let frame = t.frame_width() as usize;
        let buttons = [ButtonSpot { glyph: Glyph::Close, x: 270, y: (frame + 2) as i32, w: 14, h: 14 }];
        let cv = render_frame(&t, &FrameSpec { w, h, scale: 1.0, focused: true, label: None, buttons: &buttons });
        // the outer pixel is the border colour
        assert_eq!(cv.pixel(0, 0)[..3], [0x11, 0x22, 0x33]);
        assert_eq!(cv.pixel(w - 1, h - 1)[..3], [0x11, 0x22, 0x33]);
        // the title bar is the gradient: lighter at the top than at the bottom
        let top = cv.pixel(100, frame + 1);
        let bottom = cv.pixel(100, frame + title_h - 2);
        assert!(top[2] > bottom[2] + 10, "gradient: {top:?} {bottom:?}");
        // the strip between border and client at the side is the client colour
        assert_eq!(cv.pixel(1, 100)[..3], [0xcc, 0xcc, 0xcc]);
        // the client's place is see-through
        assert_eq!(cv.pixel(150, 100), [0, 0, 0, 0]);
        // the button's X is drawn in the button colour
        let drawn = (270..284).flat_map(|x| (frame + 2..frame + 16).map(move |y| (x, y))).any(|(x, y)| cv.pixel(x, y)[..3] == [0xff, 0xee, 0xdd]);
        assert!(drawn, "the close glyph");
        // not focused: the flat grey title
        let off = render_frame(&t, &FrameSpec { w, h, scale: 1.0, focused: false, label: None, buttons: &[] });
        assert_eq!(off.pixel(100, frame + 3)[..3], [0xbb, 0xbb, 0xbb]);
        assert_eq!(off.pixel(0, 0)[..3], [0x99, 0x99, 0x99]);
    }

    #[test]
    fn the_label_is_placed_by_the_justification() {
        let mut t = theme();
        let label_px = vec![255u8; 40 * 10 * 4];
        let spec = |t: &ObTheme| render_frame(t, &FrameSpec { w: 300, h: 100, scale: 1.0, focused: true, label: Some(Label { rgba: &label_px, w: 40, h: 10 }), buttons: &[] });
        let first_white = |cv: &Canvas| (0..300).find(|&x| cv.pixel(x, t_title_y(&theme())).iter().all(|&c| c == 255));
        t.justify = Justify::Left;
        let left = first_white(&spec(&t)).unwrap();
        t.justify = Justify::Center;
        let center = first_white(&spec(&t)).unwrap();
        t.justify = Justify::Right;
        let right = first_white(&spec(&t)).unwrap();
        assert!(left < center && center < right, "{left} {center} {right}");
        assert!((center as i32 - 130).abs() < 12, "centred in the 300 px: {center}");
    }

    fn t_title_y(t: &ObTheme) -> usize {
        t.frame_width() as usize + t.title_height() as usize / 2
    }

    #[test]
    fn sizes_follow_the_scale() {
        let t = theme();
        let one = render_frame(&t, &FrameSpec { w: 300, h: 200, scale: 1.0, focused: true, label: None, buttons: &[] });
        let two = render_frame(&t, &FrameSpec { w: 600, h: 400, scale: 2.0, focused: true, label: None, buttons: &[] });
        // the client's hole starts twice as far in at double scale
        let hole = |c: &Canvas| (0..c.h).find(|&y| c.pixel(c.w / 2, y)[3] == 0).unwrap();
        assert_eq!(hole(&two), hole(&one) * 2 - 0);
    }

    #[test]
    fn theme_folders_are_found_listed_and_names_cannot_escape() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["Alpha", "Beta Dark", "NoOpenbox"] {
            std::fs::create_dir_all(dir.path().join(n).join(if n == "NoOpenbox" { "gtk-3.0" } else { "openbox-3" })).unwrap();
        }
        std::fs::write(dir.path().join("Alpha/openbox-3/themerc"), CLEARLOOKS_LIKE).unwrap();
        std::fs::write(dir.path().join("Beta Dark/openbox-3/themerc"), "border.width: 3\n").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(list_names(&dirs), vec!["Alpha".to_string(), "Beta Dark".to_string()]);
        let found = find_dir("Beta Dark", &dirs).unwrap();
        assert_eq!(ObTheme::load(&found).unwrap().border_width, 3);
        assert!(find_dir("NoOpenbox", &dirs).is_none());
        assert!(find_dir("../Alpha", &dirs).is_none());
        assert!(find_dir(".hidden", &dirs).is_none());
        assert!(!valid_name("a/b") && !valid_name("") && valid_name("Clearlooks-Olive 2.1"));
    }

    #[test]
    fn a_button_bitmap_from_the_theme_replaces_the_stand_in() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("openbox-3")).unwrap();
        std::fs::write(dir.path().join("openbox-3/close.xbm"), "#define c_width 8\n#define c_height 8\nstatic char c_bits[] = { 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff };").unwrap();
        let t = ObTheme::parse("", "T", dir.path());
        assert_eq!(glyph_bitmap(&t, Glyph::Close).bits.iter().filter(|b| **b).count(), 64, "the theme's solid square");
        assert!(glyph_bitmap(&t, Glyph::Iconify).bits.iter().filter(|b| **b).count() < 20, "no file: the plain stand-in");
    }

    /// Every Openbox theme installed on this computer (if any) reads and draws without trouble.
    #[test]
    fn the_installed_openbox_themes_all_load_and_draw() {
        let dirs = theme_dirs();
        let names = list_names(&dirs);
        if names.is_empty() {
            eprintln!("no Openbox themes installed: test skipped");
            return;
        }
        for name in names {
            let dir = find_dir(&name, &dirs).unwrap();
            let t = ObTheme::load(&dir).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(t.frame_width() <= 32 && t.title_height() >= 14 && t.title_height() <= 80, "{name}: sizes {} {}", t.frame_width(), t.title_height());
            for scale in [1.0f32, 2.0] {
                for focused in [true, false] {
                    let (w, h) = ((400.0 * scale) as usize, (300.0 * scale) as usize);
                    let buttons = [
                        ButtonSpot { glyph: Glyph::Iconify, x: w as i32 - 70, y: 4, w: 14, h: 14 },
                        ButtonSpot { glyph: Glyph::Maximize, x: w as i32 - 50, y: 4, w: 14, h: 14 },
                        ButtonSpot { glyph: Glyph::Close, x: w as i32 - 30, y: 4, w: 14, h: 14 },
                    ];
                    let cv = render_frame(&t, &FrameSpec { w, h, scale, focused, label: None, buttons: &buttons });
                    assert_eq!(cv.px.len(), w * h * 4, "{name}");
                    assert_eq!(cv.pixel(w / 2, h / 2), [0, 0, 0, 0], "{name}: the client's place is empty");
                    assert_eq!(cv.pixel(w / 2, 0)[3], 255, "{name}: the border is opaque");
                }
            }
        }
    }
}
