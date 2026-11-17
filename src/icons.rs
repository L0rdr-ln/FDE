//! App icons for the task bar and the start menu: looked up by name in the installed icon themes
//! (the freedesktop icon theme layout, simplified), PNG or SVG, scaled to a square of `px`
//! pixels, as premultiplied RGBA.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    /// width and height in pixels
    pub size: i32,
    /// premultiplied RGBA, top row first
    pub rgba: Vec<u8>,
}

/// Icon themes looked at, in order (hicolor holds what applications install).
const THEMES: &[&str] = &["hicolor", "FDE", "Adwaita", "breeze", "Papirus", "elementary", "Humanity", "gnome"];
const SIZES: &[i32] = &[16, 22, 24, 32, 36, 48, 64, 96, 128, 192, 256, 512];

fn data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    match std::env::var_os("XDG_DATA_HOME") {
        Some(d) => v.push(PathBuf::from(d)),
        None => {
            if let Some(h) = std::env::var_os("HOME") {
                v.push(PathBuf::from(h).join(".local/share"));
            }
        }
    }
    // the icons that come with FDE, where this build was made (for running from a source tree)
    let shipped = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/data"));
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(sys.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    if shipped.is_dir() {
        v.push(shipped);
    }
    v
}

/// The file of icon `name` for about `px` pixels: the smallest PNG at least that big, else an
/// SVG, else the biggest smaller PNG, else one in /usr/share/pixmaps.
pub fn find(name: &str, px: i32) -> Option<PathBuf> {
    find_in(&data_dirs(), name, px)
}

pub fn find_in(dirs: &[PathBuf], name: &str, px: i32) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    if name.starts_with('/') {
        let p = PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    let mut bigger: Vec<i32> = SIZES.iter().copied().filter(|&s| s >= px).collect();
    bigger.sort();
    let mut smaller: Vec<i32> = SIZES.iter().copied().filter(|&s| s < px).collect();
    smaller.sort_by(|a, b| b.cmp(a));
    const SUBS: [&str; 8] = ["apps", "categories", "places", "devices", "mimetypes", "legacy", "actions", "status"];
    // themes keep sizes as "48x48/apps" (hicolor, Adwaita) or "apps/48" (Humanity, elementary)
    let png = |sizes: &[i32]| -> Option<PathBuf> {
        for theme in THEMES {
            for d in dirs {
                for s in sizes {
                    for sub in SUBS {
                        for p in [d.join(format!("icons/{theme}/{s}x{s}/{sub}/{name}.png")), d.join(format!("icons/{theme}/{sub}/{s}/{name}.png"))] {
                            if p.is_file() {
                                return Some(p);
                            }
                        }
                    }
                }
            }
        }
        None
    };
    if let Some(p) = png(&bigger) {
        return Some(p);
    }
    for theme in THEMES {
        for d in dirs {
            for sub in SUBS {
                let mut candidates = vec![d.join(format!("icons/{theme}/scalable/{sub}/{name}.svg")), d.join(format!("icons/{theme}/{sub}/scalable/{name}.svg"))];
                // themes that ship SVGs in their size folders (Humanity: apps/48/name.svg)
                candidates.extend(bigger.iter().chain(smaller.iter()).map(|s| d.join(format!("icons/{theme}/{sub}/{s}/{name}.svg"))));
                if let Some(p) = candidates.into_iter().find(|p| p.is_file()) {
                    return Some(p);
                }
            }
        }
    }
    if let Some(p) = png(&smaller) {
        return Some(p);
    }
    for d in dirs {
        for ext in ["png", "svg"] {
            let p = d.join(format!("pixmaps/{name}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

thread_local! {
    static SHARED: std::cell::RefCell<IconCache> = std::cell::RefCell::new(IconCache::default());
}

/// Icon `name` (or the first of `fallbacks` that exists) at `px` pixels, from a cache shared by the
/// program's drawing code (one per thread).
pub fn cached(name: &str, px: i32, fallbacks: &[&str]) -> Option<Icon> {
    SHARED.with(|c| c.borrow_mut().get(name, px, fallbacks).cloned())
}

/// Read an icon file (PNG or SVG) as a `px` x `px` picture.
pub fn load(path: &Path, px: i32) -> Option<Icon> {
    let px = px.max(1);
    match path.extension().and_then(|e| e.to_str()) {
        Some("svg") => load_svg(path, px),
        Some("png") => load_png(path, px),
        _ => None,
    }
}

/// Icons larger than this are not icons: refused before they are read into memory.
const MAX_ICON_SIDE: u32 = 2048;
const MAX_SVG_BYTES: u64 = 4 << 20;

fn load_svg(path: &Path, px: i32) -> Option<Icon> {
    if std::fs::metadata(path).ok()?.len() > MAX_SVG_BYTES {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px as u32, px as u32)?;
    let size = tree.size();
    let k = px as f32 / size.width().max(size.height());
    let (dx, dy) = ((px as f32 - size.width() * k) / 2.0, (px as f32 - size.height() * k) / 2.0);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_row(k, 0.0, 0.0, k, dx, dy), &mut pixmap.as_mut());
    Some(Icon { size: px, rgba: pixmap.data().to_vec() })
}

fn load_png(path: &Path, px: i32) -> Option<Icon> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    if reader.info().width > MAX_ICON_SIDE || reader.info().height > MAX_ICON_SIDE {
        return None;
    }
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let src = &buf[..info.buffer_size()];
    // to premultiplied RGBA
    let mut rgba = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let (r, g, b, a) = match info.color_type {
            png::ColorType::Rgba => (src[i * 4], src[i * 4 + 1], src[i * 4 + 2], src[i * 4 + 3]),
            png::ColorType::Rgb => (src[i * 3], src[i * 3 + 1], src[i * 3 + 2], 255),
            png::ColorType::GrayscaleAlpha => (src[i * 2], src[i * 2], src[i * 2], src[i * 2 + 1]),
            png::ColorType::Grayscale => (src[i], src[i], src[i], 255),
            png::ColorType::Indexed => return None, // expanded above
        };
        let m = |c: u8| (c as u32 * a as u32 / 255) as u8;
        rgba.extend_from_slice(&[m(r), m(g), m(b), a]);
    }
    Some(Icon { size: px, rgba: resize(&rgba, w, h, px as usize) })
}

/// Scale a premultiplied picture to `n` x `n` (area average going down, bilinear going up).
pub fn resize(src: &[u8], w: usize, h: usize, n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    let (sx, sy) = (w as f32 / n as f32, h as f32 / n as f32);
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0f32; 4];
            if sx >= 1.0 && sy >= 1.0 {
                let (x0, x1) = ((x as f32 * sx) as usize, (((x + 1) as f32 * sx).ceil() as usize).min(w));
                let (y0, y1) = ((y as f32 * sy) as usize, (((y + 1) as f32 * sy).ceil() as usize).min(h));
                let mut count = 0f32;
                for yy in y0..y1.max(y0 + 1) {
                    for xx in x0..x1.max(x0 + 1) {
                        let i = (yy.min(h - 1) * w + xx.min(w - 1)) * 4;
                        for c in 0..4 {
                            acc[c] += src[i + c] as f32;
                        }
                        count += 1.0;
                    }
                }
                for a in acc.iter_mut() {
                    *a /= count;
                }
            } else {
                let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
                let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
                let (x0, y0) = (fx as usize, fy as usize);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                for c in 0..4 {
                    let p = |xx: usize, yy: usize| src[(yy * w + xx) * 4 + c] as f32;
                    acc[c] = p(x0, y0) * (1.0 - tx) * (1.0 - ty) + p(x1, y0) * tx * (1.0 - ty) + p(x0, y1) * (1.0 - tx) * ty + p(x1, y1) * tx * ty;
                }
            }
            let o = (y * n + x) * 4;
            for c in 0..4 {
                out[o + c] = acc[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Icons already looked up, by name and size (also the ones that were not found).
#[derive(Default)]
pub struct IconCache {
    map: HashMap<(String, i32), Option<Icon>>,
}

impl IconCache {
    /// Icon `name` at `px` pixels, or the first of `fallbacks` that exists.
    pub fn get(&mut self, name: &str, px: i32, fallbacks: &[&str]) -> Option<&Icon> {
        let mut chosen = None;
        for n in std::iter::once(name).chain(fallbacks.iter().copied()) {
            let key = (n.to_string(), px);
            if !self.map.contains_key(&key) {
                // an icon that draws nothing (an SVG made of text, which is not rendered) is no icon
                let icon = find(n, px).and_then(|p| load(&p, px)).filter(|i| i.rgba.chunks(4).any(|c| c[3] > 64));
                self.map.insert(key.clone(), icon);
            }
            if self.map.get(&key).is_some_and(|i| i.is_some()) {
                chosen = Some(key);
                break;
            }
        }
        chosen.and_then(|k| self.map.get(&k)).and_then(|i| i.as_ref())
    }
}

/// Draw `icon` onto `buf` (premultiplied, `width` x `height`) with its corner at (x0, y0).
pub fn blit_icon(buf: &mut [u8], width: i32, height: i32, icon: &Icon, x0: i32, y0: i32, alpha: f32) {
    for y in 0..icon.size {
        for x in 0..icon.size {
            let (px, py) = (x0 + x, y0 + y);
            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }
            let s = ((y * icon.size + x) * 4) as usize;
            let d = ((py * width + px) * 4) as usize;
            let a = icon.rgba[s + 3] as f32 / 255.0 * alpha;
            for c in 0..4 {
                let src = icon.rgba[s + c] as f32 * alpha;
                buf[d + c] = (src + buf[d + c] as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_found_by_size_preference_and_drawn_square() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("icons/hicolor");
        let write_png = |p: &Path, n: u32| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            let f = std::fs::File::create(p).unwrap();
            let mut e = png::Encoder::new(f, n, n);
            e.set_color(png::ColorType::Rgba);
            e.write_header().unwrap().write_image_data(&vec![200u8; (n * n * 4) as usize]).unwrap();
        };
        write_png(&theme.join("16x16/apps/demo.png"), 16);
        write_png(&theme.join("48x48/apps/demo.png"), 48);
        std::fs::create_dir_all(theme.join("scalable/apps")).unwrap();
        std::fs::write(
            theme.join("scalable/apps/vector.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#,
        )
        .unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert!(find_in(&dirs, "demo", 32).unwrap().ends_with("48x48/apps/demo.png"), "the smallest PNG at least as big");
        assert!(find_in(&dirs, "demo", 64).unwrap().ends_with("48x48/apps/demo.png"), "no bigger one: the biggest smaller one");
        assert!(find_in(&dirs, "vector", 24).unwrap().ends_with("vector.svg"));
        assert!(find_in(&dirs, "missing", 24).is_none());
        let icon = load(&find_in(&dirs, "demo", 32).unwrap(), 24).unwrap();
        assert_eq!((icon.size, icon.rgba.len()), (24, 24 * 24 * 4));
        let svg = load(&find_in(&dirs, "vector", 24).unwrap(), 24).unwrap();
        let mid = ((12 * 24 + 12) * 4) as usize;
        assert_eq!(&svg.rgba[mid..mid + 4], &[255, 0, 0, 255], "the SVG is drawn");
    }

    /// Every icon of the FDE theme draws something: not blank, not a flat square, and the common
    /// names are found through the ordinary lookup. `FDE_ICON_SHEET=<file.png>` writes a contact sheet.
    #[test]
    fn the_fde_icon_theme_draws_and_is_found() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/icons/FDE");
        let mut all: Vec<PathBuf> = Vec::new();
        for sub in std::fs::read_dir(root.join("scalable")).unwrap().flatten() {
            for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
                if f.path().extension().is_some_and(|e| e == "svg") {
                    all.push(f.path());
                }
            }
        }
        all.sort();
        assert!(all.len() >= 100, "{} icons", all.len());
        let mut sheet: Vec<Icon> = Vec::new();
        let mut seen: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
        for p in &all {
            let ic = load(p, 64).unwrap_or_else(|| panic!("{} does not load", p.display()));
            let opaque = ic.rgba.chunks(4).filter(|px| px[3] > 200).count();
            let clear = ic.rgba.chunks(4).filter(|px| px[3] < 10).count();
            assert!(opaque > 64 * 64 / 5, "{}: hardly anything is drawn ({opaque} solid pixels)", p.display());
            assert!(clear > 100, "{}: it fills the whole square (no transparent corners)", p.display());
            // the glyph: pixels near white somewhere inside (a tile without a glyph is a blank tile)
            let bright = ic.rgba.chunks(4).filter(|px| px[3] > 200 && px[0] > 235 && px[1] > 235 && px[2] > 235).count();
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let own_shape = stem.starts_with("folder") || stem.starts_with("inode-") || stem.starts_with("dialog-");
            assert!(bright > 40 || own_shape, "{}: no glyph ({bright})", p.display());
            seen.insert(ic.rgba.clone());
            sheet.push(ic);
        }
        assert!(seen.len() >= 45, "{} different pictures: the icons are meant to differ", seen.len());
        // the lookup finds them by their names, after the icons applications bring themselves
        let dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("data")];
        for name in ["utilities-terminal", "system-file-manager", "folder", "user-home", "system-shutdown", "dialog-warning", "audio-volume-muted"] {
            let p = find_in(&dirs, name, 48).unwrap_or_else(|| panic!("{name} not found"));
            assert!(p.to_string_lossy().contains("icons/FDE/"), "{name}: {}", p.display());
        }
        if let Some(out) = std::env::var_os("FDE_ICON_SHEET") {
            let cols = 12usize;
            let rows = sheet.len().div_ceil(cols);
            let (w, h) = (cols * 72, rows * 72);
            let mut px = vec![0u8; w * h * 4];
            for c in px.chunks_mut(4) {
                c.copy_from_slice(&[0xcf, 0xe6, 0xff, 255]);
            }
            for (i, ic) in sheet.iter().enumerate() {
                let (ox, oy) = ((i % cols) * 72 + 4, (i / cols) * 72 + 4);
                for y in 0..64 {
                    for x in 0..64 {
                        let s = &ic.rgba[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
                        let d = ((oy + y) * w + ox + x) * 4;
                        let inv = 255 - s[3] as u32;
                        for k in 0..3 {
                            px[d + k] = (s[k] as u32 + px[d + k] as u32 * inv / 255) as u8;
                        }
                    }
                }
            }
            crate::demo::write_png(out.to_str().unwrap(), &px, w as i32, h as i32, false).unwrap();
        }
    }
}
