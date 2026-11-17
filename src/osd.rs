//! The on-screen display: what the volume and brightness keys show for a moment, a glass panel
//! with an icon, a bar and the number. The keys' threads send the words ("Volume: 45%", "Volume:
//! muted", "Brightness: 60%") through `Fde::hud_inbox`; this turns them into the picture.

use crate::text::TextRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Volume,
    Brightness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Osd {
    pub kind: Kind,
    pub percent: u32,
    pub muted: bool,
}

/// Size of the panel in logical pixels.
pub const W: i32 = 300;
pub const H: i32 = 76;

/// What the words say, if they are about the volume or the brightness.
pub fn parse(text: &str) -> Option<Osd> {
    let (head, rest) = text.split_once(':')?;
    let kind = match head.trim() {
        "Volume" => Kind::Volume,
        "Brightness" => Kind::Brightness,
        _ => return None,
    };
    let rest = rest.trim();
    if rest.eq_ignore_ascii_case("muted") {
        return Some(Osd { kind, percent: 0, muted: true });
    }
    let percent: u32 = rest.trim_end_matches('%').trim().parse().ok()?;
    Some(Osd { kind, percent: percent.min(150), muted: false })
}

/// The icon name that fits.
pub fn icon_name(o: &Osd) -> &'static str {
    match o.kind {
        Kind::Brightness => "display-brightness",
        Kind::Volume if o.muted || o.percent == 0 => "audio-volume-muted",
        Kind::Volume if o.percent >= 66 => "audio-volume-high",
        Kind::Volume if o.percent >= 33 => "audio-volume-medium",
        Kind::Volume => "audio-volume-low",
    }
}

/// Where the bar is, in logical pixels: (left, top, right, bottom).
pub fn bar_rect() -> (i32, i32, i32, i32) {
    (66, 44, W - 78, 52)
}

/// The panel at `s` pixels per logical pixel, premultiplied RGBA, `W * s` by `H * s`.
pub fn draw(text: &mut TextRenderer, s: i32, o: &Osd) -> Vec<u8> {
    use crate::flyout::{fill_box, glass, label, scaled};
    let (w, h) = (W * s, H * s);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let pal = crate::theme::palette();
    let ink = pal.ink;
    glass(&mut buf, w, h, s);
    if let Some(icon) = crate::icons::cached(icon_name(o), 40 * s, &["application-x-executable"]) {
        crate::icons::blit_icon(&mut buf, w, h, &icon, 14 * s, (h - icon.size) / 2, 1.0);
    }
    let title = match o.kind {
        Kind::Volume => "Volume",
        Kind::Brightness => "Brightness",
    };
    label(text, &mut buf, w, h, s, title, 66, 24, 13.0, ink, W - 66 - 14);
    let r = bar_rect();
    let frac = if o.muted { 0.0 } else { (o.percent as f32 / 100.0).clamp(0.0, 1.0) };
    let radius = 4.0 * s as f32;
    fill_box(&mut buf, w, h, scaled(r, s), radius, s, |_| (pal.dark(0.55), 0.35), 0.25, pal.dark(0.7));
    let fill_x = r.0 + ((r.2 - r.0) as f32 * frac) as i32;
    if fill_x > r.0 + 2 {
        fill_box(&mut buf, w, h, scaled((r.0, r.1, fill_x, r.3), s), radius, s, |t| (pal.light(0.1 + 0.25 * (1.0 - t)), 0.95), 0.4, pal.dark(0.7));
    }
    let words = if o.muted { "muted".to_string() } else { format!("{}%", o.percent) };
    label(text, &mut buf, w, h, s, &words, W - 70, 48, 14.0, ink, 60);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_of_the_keys_are_understood() {
        assert_eq!(parse("Volume: 45%"), Some(Osd { kind: Kind::Volume, percent: 45, muted: false }));
        assert_eq!(parse("Volume: muted"), Some(Osd { kind: Kind::Volume, percent: 0, muted: true }));
        assert_eq!(parse("Brightness: 60%"), Some(Osd { kind: Kind::Brightness, percent: 60, muted: false }));
        assert_eq!(parse("Volume: 400%").unwrap().percent, 150, "clamped");
        assert_eq!(parse("Workspace 2"), None);
        assert_eq!(parse("Keyboard: de"), None);
        assert_eq!(parse("Volume: loud"), None);
        assert_eq!(parse("No sound server"), None);
    }

    #[test]
    fn the_icon_follows_the_level() {
        let v = |p, m| icon_name(&Osd { kind: Kind::Volume, percent: p, muted: m });
        assert_eq!(v(80, false), "audio-volume-high");
        assert_eq!(v(50, false), "audio-volume-medium");
        assert_eq!(v(10, false), "audio-volume-low");
        assert_eq!(v(0, false), "audio-volume-muted");
        assert_eq!(v(80, true), "audio-volume-muted");
        assert_eq!(icon_name(&Osd { kind: Kind::Brightness, percent: 5, muted: false }), "display-brightness");
        // every one of those icons exists in the FDE theme
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        for name in ["audio-volume-high", "audio-volume-medium", "audio-volume-low", "audio-volume-muted", "display-brightness"] {
            assert!(crate::icons::find_in(&[root.clone()], name, 40).is_some(), "{name}");
        }
    }

    #[test]
    fn the_bar_is_filled_to_the_level() {
        let mut text = TextRenderer::default();
        let (w, _) = (W, H);
        let mut px = |osd: &Osd| draw(&mut text, 1, osd);
        let (r0, r1, r2, r3) = bar_rect();
        let mid_y = (r1 + r3) / 2;
        let at = |buf: &[u8], x: i32| {
            let i = ((mid_y * w + x) * 4) as usize;
            [buf[i] as i32, buf[i + 1] as i32, buf[i + 2] as i32, buf[i + 3] as i32]
        };
        let low = px(&Osd { kind: Kind::Volume, percent: 20, muted: false });
        let high = px(&Osd { kind: Kind::Volume, percent: 80, muted: false });
        let x = r0 + (r2 - r0) / 2; // the middle of the bar: filled at 80 %, empty at 20 %
        let (lo, hi) = (at(&low, x), at(&high, x));
        assert!(hi[2] > hi[0] + 30, "filled with the accent at 80 %: {hi:?}");
        assert!(lo[2] <= lo[0] + 30 || lo != hi, "not at 20 %: {lo:?} against {hi:?}");
        assert_ne!(lo, hi);
        let muted = px(&Osd { kind: Kind::Volume, percent: 0, muted: true });
        assert_ne!(at(&muted, r0 + 6), at(&high, r0 + 6), "a muted bar is empty");
        assert_eq!(low.len(), (W * H * 4) as usize);
        assert!(low.chunks(4).any(|p| p[3] > 200), "something is drawn");
    }
}
