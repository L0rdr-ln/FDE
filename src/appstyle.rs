//! Styling other programs. FDE writes the colours (and a few rules) of the active theme where GTK4,
//! GTK3 and Qt programs look for them, so that a file manager or a settings window shares the
//! look of the desktop instead of showing stock Adwaita:
//!
//! - `~/.config/gtk-4.0/gtk.css` and `gtk-3.0/gtk.css`: libadwaita's named colours and glossy
//!   header bars, buttons and entries;
//! - `~/.config/gtk-4.0/settings.ini`, `gtk-3.0/settings.ini`: the icon theme (FDE);
//! - `~/.config/qt5ct/colors/fde.conf` and `qt6ct/colors/fde.conf`: a colour scheme (and a
//!   `qt5ct.conf` / `qt6ct.conf` that uses it, when the user has none).
//!
//! A file is only written when it does not exist or when its first line says `fde-managed`:
//! a file the user wrote themselves is never touched (remove the marker line to take a file over).
//! `appearance.style_apps = false` turns all of this off.

use std::path::{Path, PathBuf};

use crate::config::Config;

pub const MARKER: &str = "fde-managed";

/// What was done with one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum What {
    Wrote,
    Unchanged,
    /// not touched, and why
    Skipped(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub path: PathBuf,
    pub what: What,
}

/// The colours everything is made from (0 .. 255 per channel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub accent: [u8; 3],
    /// the accent lit from above and shaded below: for gradients of bars and buttons
    pub accent_light: [u8; 3],
    pub accent_dark: [u8; 3],
    pub window_bg: [u8; 3],
    pub view_bg: [u8; 3],
    pub fg: [u8; 3],
    /// text on the accent
    pub on_accent: [u8; 3],
    pub border: [u8; 3],
    /// corner radius in pixels
    pub radius: u32,
    /// how glossy (0 flat .. 1 Aero)
    pub shine: f32,
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)).round() as u8)
}

fn luminance(c: [u8; 3]) -> f32 {
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn rgba(c: [u8; 3], a: f32) -> String {
    format!("rgba({}, {}, {}, {:.2})", c[0], c[1], c[2], a)
}

impl Colors {
    /// The colours of the theme in use (with the user's own accent, if they set one).
    pub fn from_config(cfg: &Config) -> Colors {
        let theme = cfg.theme();
        let (accent, _, _, gloss) = cfg.look();
        let a = crate::config::parse_color(&accent).unwrap_or([0.29, 0.66, 0.93]);
        let to8 = |c: [f32; 3]| [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8];
        let accent = to8(a);
        let ink = crate::config::parse_color(&theme.title_ink).map(to8).unwrap_or([0x06, 0x23, 0x3d]);
        Colors::new(accent, ink, theme.radius, gloss)
    }

    pub fn new(accent: [u8; 3], ink: [u8; 3], theme_radius: f32, gloss: f32) -> Colors {
        let window_bg = mix([255, 255, 255], accent, 0.07);
        // the text is the theme's dark ink, darkened a little so that it reads on the light windows
        let fg = if luminance(ink) < 0.35 { ink } else { mix(accent, [0, 0, 0], 0.78) };
        Colors {
            accent,
            accent_light: mix(accent, [255, 255, 255], 0.30),
            accent_dark: mix(accent, [0, 0, 0], 0.22),
            window_bg,
            view_bg: [255, 255, 255],
            fg,
            on_accent: if luminance(accent) > 0.62 { fg } else { [255, 255, 255] },
            border: mix(accent, [255, 255, 255], 0.55),
            radius: (theme_radius * 0.6).round().clamp(2.0, 10.0) as u32,
            shine: (gloss / 0.8).clamp(0.0, 1.0),
        }
    }
}

fn header(theme_name: &str) -> String {
    format!("/* {MARKER}: written by FDE from the theme \"{theme_name}\" at every start and theme change.\n   Delete this line to keep your own edits (FDE then leaves the file alone). */\n")
}

/// The rules shared by GTK4 and GTK3 (header bars, buttons, entries, switches, progress bars).
fn rules(c: &Colors) -> String {
    let (r, shine) = (c.radius, c.shine);
    let hi = rgba([255, 255, 255], 0.35 + 0.45 * shine);
    let bar_top = hex(c.accent_light);
    let bar = hex(c.accent);
    let bar_bot = hex(c.accent_dark);
    let fg = hex(c.on_accent);
    let btn_top = hex(mix(c.window_bg, [255, 255, 255], 0.85));
    let btn_bot = hex(mix(c.window_bg, c.accent, 0.16));
    let border = hex(c.border);
    format!(
        r#"
headerbar, .titlebar {{
  background-image: linear-gradient(to bottom, {bar_top}, {bar} 55%, {bar_bot});
  color: {fg};
  box-shadow: inset 0 1px {hi}, 0 1px 3px rgba(0, 0, 0, 0.30);
  border-bottom: 1px solid {bar_bot};
  text-shadow: 0 1px rgba(0, 0, 0, 0.25);
}}
headerbar:backdrop, .titlebar:backdrop {{
  background-image: linear-gradient(to bottom, {btn_top}, {btn_bot});
  color: {};
  text-shadow: none;
}}
headerbar button, .titlebar button {{
  background-image: linear-gradient(to bottom, {}, {});
  border: 1px solid {bar_bot};
  color: {fg};
  box-shadow: inset 0 1px {hi};
  text-shadow: none;
}}
button {{
  background-image: linear-gradient(to bottom, {btn_top}, {btn_bot});
  border: 1px solid {border};
  border-radius: {r}px;
  box-shadow: inset 0 1px {hi}, 0 1px 1px rgba(0, 0, 0, 0.10);
}}
button:hover {{
  background-image: linear-gradient(to bottom, {}, {});
  border-color: {bar};
}}
button:active, button:checked {{
  background-image: linear-gradient(to bottom, {bar_bot}, {bar});
  color: {fg};
  box-shadow: inset 0 2px 3px rgba(0, 0, 0, 0.25);
}}
button.suggested-action {{
  background-image: linear-gradient(to bottom, {bar_top}, {bar} 55%, {bar_bot});
  color: {fg};
  border-color: {bar_bot};
  text-shadow: 0 1px rgba(0, 0, 0, 0.25);
}}
button.destructive-action {{
  background-image: linear-gradient(to bottom, #ff8a6a, #cc2c1a);
  color: #ffffff;
  border-color: #a82414;
}}
entry, spinbutton, textview, searchbar > revealer > box {{
  border-radius: {r}px;
  border: 1px solid {border};
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.12);
}}
entry:focus-within, spinbutton:focus-within {{
  border-color: {bar};
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.12), 0 0 0 2px {};
}}
switch:checked {{
  background-image: linear-gradient(to bottom, {bar_bot}, {bar});
}}
progressbar progress, scale highlight, levelbar block.filled {{
  background-image: linear-gradient(to bottom, {bar_top}, {bar_bot});
  border-radius: {r}px;
}}
scrollbar slider {{
  background-image: linear-gradient(to right, {btn_top}, {btn_bot});
  border: 1px solid {border};
  border-radius: 99px;
}}
tooltip, tooltip.background {{
  background-image: linear-gradient(to bottom, {bar_top}, {bar_bot});
  color: {fg};
  border-radius: {r}px;
}}
"#,
        hex(mix(c.fg, c.window_bg, 0.45)),
        hex(mix(c.accent_light, [255, 255, 255], 0.20)),
        hex(c.accent),
        hex(mix(c.window_bg, [255, 255, 255], 0.95)),
        hex(mix(c.window_bg, c.accent, 0.28)),
        rgba(c.accent, 0.30),
    )
}

/// `~/.config/gtk-4.0/gtk.css`: libadwaita's named colours, then the rules.
pub fn gtk4_css(c: &Colors, theme_name: &str) -> String {
    let named = [
        ("accent_bg_color", c.accent),
        ("accent_fg_color", c.on_accent),
        ("accent_color", mix(c.accent, [0, 0, 0], 0.18)),
        ("window_bg_color", c.window_bg),
        ("window_fg_color", c.fg),
        ("view_bg_color", c.view_bg),
        ("view_fg_color", c.fg),
        ("headerbar_bg_color", c.accent),
        ("headerbar_fg_color", c.on_accent),
        ("headerbar_border_color", c.accent_dark),
        ("headerbar_backdrop_color", mix(c.window_bg, c.accent, 0.12)),
        ("card_bg_color", c.view_bg),
        ("card_fg_color", c.fg),
        ("dialog_bg_color", c.window_bg),
        ("dialog_fg_color", c.fg),
        ("popover_bg_color", mix(c.window_bg, [255, 255, 255], 0.6)),
        ("popover_fg_color", c.fg),
        ("sidebar_bg_color", mix(c.window_bg, c.accent, 0.10)),
        ("sidebar_fg_color", c.fg),
        ("sidebar_backdrop_color", c.window_bg),
        ("secondary_sidebar_bg_color", mix(c.window_bg, c.accent, 0.06)),
    ];
    let mut s = header(theme_name);
    for (n, col) in named {
        s.push_str(&format!("@define-color {n} {};\n", hex(col)));
    }
    s.push_str(&rules(c));
    s
}

/// `~/.config/gtk-3.0/gtk.css`: the classic named colours, then the rules.
pub fn gtk3_css(c: &Colors, theme_name: &str) -> String {
    let named = [
        ("theme_bg_color", c.window_bg),
        ("theme_fg_color", c.fg),
        ("theme_base_color", c.view_bg),
        ("theme_text_color", c.fg),
        ("theme_selected_bg_color", c.accent),
        ("theme_selected_fg_color", c.on_accent),
        ("theme_tooltip_bg_color", c.accent_dark),
        ("theme_tooltip_fg_color", c.on_accent),
        ("insensitive_bg_color", mix(c.window_bg, c.accent, 0.04)),
        ("insensitive_fg_color", mix(c.fg, c.window_bg, 0.55)),
        ("borders", c.border),
        ("warning_color", [0xf2, 0xa5, 0x16]),
        ("error_color", [0xd9, 0x38, 0x1e]),
        ("success_color", [0x2e, 0x9b, 0x22]),
    ];
    let mut s = header(theme_name);
    for (n, col) in named {
        s.push_str(&format!("@define-color {n} {};\n", hex(col)));
    }
    s.push_str(&rules(c));
    s
}

/// A `[Settings]` file for GTK: the icon theme (and the cursor theme if the config names one).
pub fn gtk_settings_ini(cfg: &Config) -> String {
    let mut s = format!("# {MARKER}: written by FDE; delete this line to keep your own edits\n[Settings]\ngtk-icon-theme-name=FDE\ngtk-application-prefer-dark-theme=false\n");
    if !cfg.appearance.cursor_theme.is_empty() {
        s.push_str(&format!("gtk-cursor-theme-name={}\n", cfg.appearance.cursor_theme));
    }
    s
}

/// The colour scheme for qt5ct / qt6ct: 21 colours each for active, disabled and inactive.
pub fn qt_colors(c: &Colors) -> String {
    let dim = |x: [u8; 3]| mix(x, c.window_bg, 0.5);
    let roles = |fg: [u8; 3], disabled: bool| -> String {
        let f = |x: [u8; 3]| hex(if disabled { dim(x) } else { x }).replace('#', "#ff");
        let list = [
            f(fg),                                  // windowText
            f(mix(c.window_bg, c.accent, 0.12)),    // button
            f([255, 255, 255]),                     // light
            f(mix(c.window_bg, [255, 255, 255], 0.5)), // midlight
            f(mix(c.window_bg, [0, 0, 0], 0.35)),   // dark
            f(mix(c.window_bg, [0, 0, 0], 0.18)),   // mid
            f(fg),                                  // text
            f([255, 255, 255]),                     // brightText
            f(fg),                                  // buttonText
            f(c.view_bg),                           // base
            f(c.window_bg),                         // window
            f([0, 0, 0]),                           // shadow
            f(c.accent),                            // highlight
            f(c.on_accent),                         // highlightedText
            f(mix(c.accent, [0, 0, 0], 0.25)),      // link
            f(mix(c.accent, [0x80, 0x30, 0xa0], 0.5)), // linkVisited
            f(mix(c.view_bg, c.accent, 0.05)),      // alternateBase
            f([0, 0, 0]),                           // noRole
            f(mix(c.accent_dark, [0, 0, 0], 0.2)),  // toolTipBase
            f(c.on_accent),                         // toolTipText
            f(mix(fg, c.window_bg, 0.5)),           // placeholderText
        ];
        list.join(", ")
    };
    format!(
        "# {MARKER}: written by FDE; delete this line to keep your own edits\n[ColorScheme]\nactive_colors={}\ndisabled_colors={}\ninactive_colors={}\n",
        roles(c.fg, false),
        roles(c.fg, true),
        roles(c.fg, false)
    )
}

/// A minimal `qt5ct.conf` / `qt6ct.conf` that uses the scheme (written only when there is none).
pub fn qtct_conf(scheme: &Path) -> String {
    format!("# {MARKER}: written by FDE; delete this line to keep your own edits\n[Appearance]\ncolor_scheme_path={}\ncustom_palette=true\nicon_theme=FDE\nstandard_dialogs=default\nstyle=Fusion\n", scheme.display())
}

/// Write `text` to `path` if the file may be written (it does not exist or is ours).
fn write_managed(path: &Path, text: &str) -> Outcome {
    let skipped = |why: &str| Outcome { path: path.to_path_buf(), what: What::Skipped(why.to_string()) };
    match std::fs::read_to_string(path) {
        Ok(old) => {
            if !old.lines().take(3).any(|l| l.contains(MARKER)) {
                return skipped("a file of your own (no fde-managed line)");
            }
            if old == text {
                return Outcome { path: path.to_path_buf(), what: What::Unchanged };
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return skipped("cannot be read"),
    }
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return skipped("its folder cannot be made");
        }
    }
    let tmp = path.with_extension("fde-tmp");
    if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path)).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return skipped("cannot be written");
    }
    Outcome { path: path.to_path_buf(), what: What::Wrote }
}

/// Write everything for `cfg` below `config_home` (`~/.config`). Does nothing (and says so) when
/// `appearance.style_apps` is off.
pub fn apply(cfg: &Config, config_home: &Path) -> Vec<Outcome> {
    if !cfg.appearance.style_apps {
        return Vec::new();
    }
    let colors = Colors::from_config(cfg);
    let name = cfg.theme().name;
    let mut out = vec![
        write_managed(&config_home.join("gtk-4.0/gtk.css"), &gtk4_css(&colors, &name)),
        write_managed(&config_home.join("gtk-3.0/gtk.css"), &gtk3_css(&colors, &name)),
        write_managed(&config_home.join("gtk-4.0/settings.ini"), &gtk_settings_ini(cfg)),
        write_managed(&config_home.join("gtk-3.0/settings.ini"), &gtk_settings_ini(cfg)),
    ];
    for qt in ["qt5ct", "qt6ct"] {
        let scheme = config_home.join(qt).join("colors/fde.conf");
        out.push(write_managed(&scheme, &qt_colors(&colors)));
        out.push(write_managed(&config_home.join(qt).join(format!("{qt}.conf")), &qtct_conf(&scheme)));
    }
    out
}

/// `~/.config` (or `$XDG_CONFIG_HOME`).
pub fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// Apply the running configuration to the user's own folders.
pub fn apply_current() -> Vec<Outcome> {
    let (Ok(cfg), Some(home)) = (Config::load(), config_home()) else { return Vec::new() };
    apply(&cfg, &home)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> Colors {
        Colors::new([0x4a, 0xa8, 0xee], [0x06, 0x23, 0x3d], 12.0, 0.8)
    }

    #[test]
    fn the_colours_follow_the_accent_and_stay_readable() {
        let c = colors();
        assert!(luminance(c.window_bg) > 0.9, "light windows");
        assert!(luminance(c.fg) < 0.2, "dark text");
        assert_eq!(c.on_accent, [255, 255, 255], "white on a mid blue");
        assert!(luminance(c.accent_light) > luminance(c.accent) && luminance(c.accent) > luminance(c.accent_dark));
        // a pale accent gets dark text on it
        let pale = Colors::new([0xff, 0xe6, 0xbd], [0x40, 0x20, 0x00], 12.0, 0.9);
        assert_eq!(pale.on_accent, pale.fg);
        // the radius follows the theme, within reason; flat themes get small corners
        assert_eq!(Colors::new([1, 2, 3], [0, 0, 0], 0.0, 0.0).radius, 2);
        assert_eq!(Colors::new([1, 2, 3], [0, 0, 0], 100.0, 0.0).radius, 10);
        assert_eq!(Colors::new([1, 2, 3], [0, 0, 0], 12.0, 0.0).shine, 0.0);
    }

    #[test]
    fn the_gtk_css_has_the_named_colours_and_the_rules() {
        let c = colors();
        let g4 = gtk4_css(&c, "Frutiger Aero");
        assert!(g4.lines().next().unwrap().contains(MARKER) && g4.contains("Frutiger Aero"));
        for name in ["accent_bg_color", "window_bg_color", "headerbar_bg_color", "sidebar_bg_color", "popover_bg_color", "card_bg_color"] {
            assert!(g4.contains(&format!("@define-color {name} #")), "{name}");
        }
        assert!(g4.contains("#4aa8ee"), "the accent");
        assert!(g4.contains("headerbar") && g4.contains("button.suggested-action") && g4.contains("linear-gradient"));
        let g3 = gtk3_css(&c, "Frutiger Aero");
        for name in ["theme_bg_color", "theme_fg_color", "theme_selected_bg_color", "theme_base_color"] {
            assert!(g3.contains(&format!("@define-color {name} #")), "{name}");
        }
        // every colour is a proper #rrggbb, and braces balance
        for text in [&g4, &g3] {
            assert_eq!(text.matches('{').count(), text.matches('}').count());
            for line in text.lines().filter(|l| l.starts_with("@define-color")) {
                let v = line.split_whitespace().nth(2).unwrap().trim_end_matches(';');
                assert!(v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|ch| ch.is_ascii_hexdigit()), "{line}");
            }
        }
    }

    #[test]
    fn the_qt_scheme_has_21_colours_for_each_state() {
        let q = qt_colors(&colors());
        for key in ["active_colors", "disabled_colors", "inactive_colors"] {
            let line = q.lines().find(|l| l.starts_with(key)).unwrap_or_else(|| panic!("{key}"));
            let list: Vec<&str> = line.split('=').nth(1).unwrap().split(", ").collect();
            assert_eq!(list.len(), 21, "{key}");
            assert!(list.iter().all(|c| c.len() == 9 && c.starts_with("#ff")), "{key}: {list:?}");
        }
        assert!(q.contains("[ColorScheme]") && q.lines().next().unwrap().contains(MARKER));
        let active = q.lines().find(|l| l.starts_with("active_colors")).unwrap();
        let disabled = q.lines().find(|l| l.starts_with("disabled_colors")).unwrap();
        assert_ne!(active, disabled, "disabled text is dimmer");
        assert!(qtct_conf(Path::new("/x/fde.conf")).contains("color_scheme_path=/x/fde.conf"));
    }

    #[test]
    fn files_are_written_updated_and_a_users_own_file_is_never_touched() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::default();
        let first = apply(&cfg, dir.path());
        assert_eq!(first.len(), 8);
        assert!(first.iter().all(|o| o.what == What::Wrote), "{first:?}");
        let css = std::fs::read_to_string(dir.path().join("gtk-4.0/gtk.css")).unwrap();
        assert!(css.contains("@define-color accent_bg_color"));
        assert!(dir.path().join("qt6ct/colors/fde.conf").is_file() && dir.path().join("qt5ct/qt5ct.conf").is_file());
        assert!(std::fs::read_to_string(dir.path().join("gtk-3.0/settings.ini")).unwrap().contains("gtk-icon-theme-name=FDE"));
        // again: nothing to do
        assert!(apply(&cfg, dir.path()).iter().all(|o| o.what == What::Unchanged));
        // another accent: the managed files follow
        let mut other = Config::default();
        other.appearance.accent = "#d0301c".into();
        other.explicit.push("accent".into());
        let again = apply(&other, dir.path());
        assert!(again.iter().any(|o| o.what == What::Wrote && o.path.ends_with("gtk-4.0/gtk.css")));
        assert!(std::fs::read_to_string(dir.path().join("gtk-4.0/gtk.css")).unwrap().contains("#d0301c"));
        // a file the user wrote is left alone
        std::fs::write(dir.path().join("gtk-4.0/gtk.css"), "/* mine */\nwindow { background: red; }\n").unwrap();
        let third = apply(&cfg, dir.path());
        let css_outcome = third.iter().find(|o| o.path.ends_with("gtk-4.0/gtk.css")).unwrap();
        assert!(matches!(css_outcome.what, What::Skipped(_)));
        assert_eq!(std::fs::read_to_string(dir.path().join("gtk-4.0/gtk.css")).unwrap(), "/* mine */\nwindow { background: red; }\n");
        // switched off: nothing is written at all
        let off_dir = tempfile::tempdir().unwrap();
        let mut off = Config::default();
        off.appearance.style_apps = false;
        assert!(apply(&off, off_dir.path()).is_empty());
        assert!(std::fs::read_dir(off_dir.path()).unwrap().next().is_none());
    }
}
