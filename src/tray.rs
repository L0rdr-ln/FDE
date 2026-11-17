//! The system tray: programs that want an icon in the task bar (Discord, Syncthing, network and
//! sound applets ...) register a StatusNotifierItem on the session bus. `fde-tray` is the
//! watcher and host of those items; it writes what it knows to `$XDG_RUNTIME_DIR/fde/tray/items`
//! (and the pictures it was sent as PNG files next to it), the task bar draws the icons from there
//! and starts `fde-tray activate|secondary|context SERVICE PATH X Y` for a click.
//!
//! This file is the part without a bus: reading what programs register, picking and converting
//! their pictures, and the state file.

use std::path::{Path, PathBuf};

/// Where an item lives on the bus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemRef {
    pub service: String,
    pub path: String,
}

/// What a program passed to `RegisterStatusNotifierItem`: its bus name (the path is then
/// `/StatusNotifierItem`), the path of an object on the sender's connection, or both joined
/// (`:1.45/org/ayatana/NotificationItem/x`). `sender` is the unique name of the caller.
pub fn parse_registration(arg: &str, sender: &str) -> Option<ItemRef> {
    let arg = arg.trim();
    if arg.is_empty() || arg.len() > 300 {
        return None;
    }
    let item = if arg.starts_with('/') {
        ItemRef { service: sender.to_string(), path: arg.to_string() }
    } else if let Some(i) = arg.find('/') {
        ItemRef { service: arg[..i].to_string(), path: arg[i..].to_string() }
    } else {
        ItemRef { service: arg.to_string(), path: "/StatusNotifierItem".to_string() }
    };
    let good_service = !item.service.is_empty() && item.service.len() < 256 && item.service.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'));
    let good_path = item.path.starts_with('/') && item.path.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_'));
    (good_service && good_path).then_some(item)
}

/// An item as the task bar needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub item: ItemRef,
    pub title: String,
    /// an icon name from the themes, or the path of a PNG (empty: none yet)
    pub icon: String,
    /// "Active", "Passive" (hidden) or "NeedsAttention"
    pub status: String,
    /// the object path of the item's menu (com.canonical.dbusmenu), if it has one
    pub menu: String,
    /// the item shows its menu on a click and does nothing on Activate
    pub is_menu: bool,
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(120).collect()
}

/// The state file: one item per line, fields separated by tabs.
pub fn to_text(entries: &[Entry]) -> String {
    entries
        .iter()
        .filter(|e| e.status != "Passive")
        .map(|e| format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", e.item.service, e.item.path, e.status, clean(&e.icon), clean(&e.title), e.menu, e.is_menu as u8))
        .collect()
}

pub fn parse_text(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 7 {
                return None;
            }
            let item = parse_registration(&format!("{}{}", f[0], f[1]), "")?;
            Some(Entry { item, status: f[2].to_string(), icon: f[3].to_string(), title: f[4].to_string(), menu: f[5].to_string(), is_menu: f[6] == "1" })
        })
        .collect()
}

pub fn dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("fde/tray"))
}

pub fn read_entries() -> Vec<Entry> {
    dir().and_then(|d| std::fs::read_to_string(d.join("items")).ok()).map(|t| parse_text(&t)).unwrap_or_default()
}

/// One row of an item's menu (com.canonical.dbusmenu), flattened: submenus follow their parent
/// with a greater `depth`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub checked: bool,
    pub depth: u8,
    /// it opens a submenu (the rows after it are inside it)
    pub has_children: bool,
}

/// A menu entry's label as shown: the `_` before a letter marks a shortcut and goes, `__` is one `_`.
pub fn clean_label(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.chars().take(80).collect()
}

/// The menu as an item sent it: an entry and what is inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub separator: bool,
    pub checked: bool,
    pub children: Vec<Node>,
}

/// The rows of a menu (the root entry itself is not one), submenus opened in place, at most `max`.
pub fn flatten(root: &Node, max: usize) -> Vec<MenuRow> {
    fn walk(n: &Node, depth: u8, out: &mut Vec<MenuRow>, max: usize) {
        for c in &n.children {
            if !c.visible || out.len() >= max * 3 + 8 {
                continue; // (the real limit is applied after the separators are tidied)
            }
            out.push(MenuRow { id: c.id, label: clean_label(&c.label), enabled: c.enabled, separator: c.separator, checked: c.checked, depth, has_children: !c.children.is_empty() });
            if depth < 3 {
                walk(c, depth + 1, out, max);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out, max);
    // separators at the ends or twice in a row are noise
    let mut cleaned: Vec<MenuRow> = Vec::new();
    for r in out {
        if r.separator && cleaned.last().is_none_or(|l| l.separator) {
            continue;
        }
        cleaned.push(r);
    }
    cleaned.truncate(max);
    while cleaned.last().is_some_and(|r| r.separator) {
        cleaned.pop();
    }
    cleaned
}

/// The rows as text lines (`fde-tray menu` prints them, the task bar reads them): id, flags
/// (`e` enabled, `s` separator, `c` checked, `p` has a submenu), depth and label, tab separated.
pub fn menu_to_text(rows: &[MenuRow]) -> String {
    rows.iter()
        .map(|r| {
            let flags: String = [(r.enabled, 'e'), (r.separator, 's'), (r.checked, 'c'), (r.has_children, 'p')].iter().filter(|(on, _)| *on).map(|(_, c)| *c).collect();
            format!("{}\t{}\t{}\t{}\n", r.id, flags, r.depth, r.label.replace(['\t', '\n'], " "))
        })
        .collect()
}

pub fn menu_from_text(text: &str) -> Vec<MenuRow> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(4, '\t').collect();
            if f.len() != 4 {
                return None;
            }
            Some(MenuRow { id: f[0].parse().ok()?, enabled: f[1].contains('e'), separator: f[1].contains('s'), checked: f[1].contains('c'), has_children: f[1].contains('p'), depth: f[2].parse().ok()?, label: f[3].to_string() })
        })
        .collect()
}

/// One picture as an item sends it: width, height and ARGB32 bytes (network byte order: A, R, G, B).
pub type Pixmap = (i32, i32, Vec<u8>);

/// The picture closest to `want` pixels (the smallest one that is big enough, else the biggest).
pub fn best_pixmap(pixmaps: &[Pixmap], want: i32) -> Option<&Pixmap> {
    let valid = |p: &&Pixmap| p.0 > 0 && p.1 > 0 && p.0 <= 512 && p.1 <= 512 && p.2.len() == (p.0 as usize) * (p.1 as usize) * 4;
    let mut good: Vec<&Pixmap> = pixmaps.iter().filter(valid).collect();
    good.sort_by_key(|p| p.0.max(p.1));
    good.iter().find(|p| p.0.max(p.1) >= want).or(good.last()).copied()
}

/// ARGB32 to straight RGBA8.
pub fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
    argb.chunks_exact(4).flat_map(|p| [p[1], p[2], p[3], p[0]]).collect()
}

/// Write straight RGBA8 as a PNG.
pub fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut wr| wr.write_image_data(rgba)).map_err(|e| e.to_string())
}

/// A short stable number for a picture's bytes (to give a changed picture a new file name, so that
/// the task bar's cache does not show the old one).
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrations_come_in_three_forms_and_nonsense_is_refused() {
        let r = |s: &str| parse_registration(s, ":1.23");
        assert_eq!(r("org.kde.StatusNotifierItem-4242-1"), Some(ItemRef { service: "org.kde.StatusNotifierItem-4242-1".into(), path: "/StatusNotifierItem".into() }));
        assert_eq!(r("/org/ayatana/NotificationItem/x"), Some(ItemRef { service: ":1.23".into(), path: "/org/ayatana/NotificationItem/x".into() }));
        assert_eq!(r(":1.45/org/ayatana/NotificationItem/y"), Some(ItemRef { service: ":1.45".into(), path: "/org/ayatana/NotificationItem/y".into() }));
        assert_eq!(r(""), None);
        assert_eq!(r("bad name with spaces"), None);
        assert_eq!(r("svc/../../etc"), None, "a path is letters, digits, / and _ only");
        assert_eq!(r(&"a".repeat(400)), None);
    }

    #[test]
    fn the_state_file_round_trips_and_hides_passive_items() {
        let e = |svc: &str, status: &str, title: &str| Entry {
            item: ItemRef { service: svc.into(), path: "/StatusNotifierItem".into() },
            title: title.into(),
            icon: "/run/user/1000/fde/tray/a-1.png".into(),
            status: status.into(),
            menu: "/MenuBar".into(),
            is_menu: false,
        };
        let all = vec![e("org.a", "Active", "A\tprogram\nwith odd chars"), e("org.b", "Passive", "hidden"), e("org.c", "NeedsAttention", "C")];
        let text = to_text(&all);
        let back = parse_text(&text);
        assert_eq!(back.len(), 2, "the passive one is not written");
        assert_eq!(back[0].item.service, "org.a");
        assert_eq!(back[0].title, "Aprogramwith odd chars", "tabs and line breaks cannot break the file");
        assert_eq!(back[1].status, "NeedsAttention");
        assert_eq!(back[0].menu, "/MenuBar");
        assert!(parse_text("garbage\nonly\tthree\tfields\n").is_empty());
    }

    #[test]
    fn the_best_picture_is_the_smallest_that_is_big_enough() {
        let px = |n: i32| -> Pixmap { (n, n, vec![255; (n * n * 4) as usize]) };
        let list = vec![px(16), px(48), px(32), px(128)];
        assert_eq!(best_pixmap(&list, 24).unwrap().0, 32);
        assert_eq!(best_pixmap(&list, 64).unwrap().0, 128);
        assert_eq!(best_pixmap(&list, 500).unwrap().0, 128, "none big enough: the biggest");
        assert_eq!(best_pixmap(&[], 24), None);
        // pictures whose bytes do not match their size are not trusted
        let liar: Pixmap = (64, 64, vec![0; 10]);
        assert_eq!(best_pixmap(&[liar.clone()], 24), None);
        assert_eq!(best_pixmap(&[liar, px(16)], 24).unwrap().0, 16);
        let huge: Pixmap = (100_000, 100_000, vec![]);
        assert_eq!(best_pixmap(&[huge], 24), None);
    }

    #[test]
    fn colours_are_reordered_and_a_png_is_written() {
        // ARGB: a half-transparent red pixel and an opaque green one
        let argb = [128, 255, 0, 0, 255, 0, 255, 0];
        assert_eq!(argb_to_rgba(&argb), vec![255, 0, 0, 128, 0, 255, 0, 255]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.png");
        write_png(&path, 2, 1, &argb_to_rgba(&argb)).unwrap();
        let decoder = png::Decoder::new(std::fs::File::open(&path).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&buf[..8], &[255, 0, 0, 128, 0, 255, 0, 255]);
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
        assert_eq!(fingerprint(b"same"), fingerprint(b"same"));
    }

    fn node(id: i32, label: &str, children: Vec<Node>) -> Node {
        Node { id, label: label.into(), enabled: true, visible: true, separator: false, checked: false, children }
    }

    #[test]
    fn menu_labels_lose_their_shortcut_marks_and_menus_are_flattened_tidily() {
        assert_eq!(clean_label("_Open"), "Open");
        assert_eq!(clean_label("Save _as…"), "Save as…");
        assert_eq!(clean_label("a__b"), "a_b");
        assert_eq!(clean_label("tab\there"), "tabhere");
        let mut sep = node(9, "", vec![]);
        sep.separator = true;
        let mut hidden = node(7, "Hidden", vec![]);
        hidden.visible = false;
        let mut off = node(8, "Greyed", vec![]);
        off.enabled = false;
        let mut check = node(6, "Mute", vec![]);
        check.checked = true;
        let root = node(0, "", vec![sep.clone(), node(1, "_Open", vec![]), hidden, sep.clone(), sep.clone(), node(2, "Recent", vec![node(3, "one", vec![]), node(4, "two", vec![])]), off, check, sep]);
        let rows = flatten(&root, 50);
        let labels: Vec<(&str, u8)> = rows.iter().map(|r| (r.label.as_str(), r.depth)).collect();
        assert_eq!(labels, vec![("Open", 0), ("", 0), ("Recent", 0), ("one", 1), ("two", 1), ("Greyed", 0), ("Mute", 0)], "leading, doubled and trailing separators and hidden entries are gone");
        assert!(rows[2].has_children && !rows[0].has_children);
        assert!(!rows[5].enabled && rows[6].checked);
        assert!(rows[1].separator);
        assert_eq!(flatten(&root, 3).len(), 3, "at most so many rows");
        assert_eq!(flatten(&root, 2).len(), 1, "cut after Open, the separator that would end the menu goes too");
        // text round trip
        assert_eq!(menu_from_text(&menu_to_text(&rows)), rows);
        assert!(menu_from_text("junk\nx\ty\n").is_empty());
    }
}
