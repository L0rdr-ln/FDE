//! The keys of a laptop: volume, brightness and media. Each runs on a thread of its own (a sound
//! server or D-Bus that is slow must not stall the compositor), and says what it did on the
//! screen through `Fde::hud_inbox`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Messages from the threads, shown on the screen by the compositor's loop.
pub type Inbox = Arc<Mutex<Vec<String>>>;

/// The brightness (0 .. `max`) after a step of `percent` of the full range: never below 1 (a
/// black screen cannot be brought back with the keys), never above `max`.
pub fn next_brightness(cur: u32, max: u32, percent: i32) -> u32 {
    let step = (max as i64 * percent as i64 / 100).abs().max(1);
    let next = if percent >= 0 { cur as i64 + step } else { cur as i64 - step };
    next.clamp(1, max.max(1) as i64) as u32
}

/// The first backlight under `root` (`/sys/class/backlight`): its name, brightness and maximum.
pub fn backlight_in(root: &Path) -> Option<(String, u32, u32)> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let read = |f: &str| std::fs::read_to_string(d.join(f)).ok()?.trim().parse::<u32>().ok();
        if let (Some(cur), Some(max)) = (read("brightness"), read("max_brightness")) {
            if max > 0 {
                return Some((d.file_name()?.to_string_lossy().into_owned(), cur, max));
            }
        }
    }
    None
}

/// The first media player on the session bus in the output of `gdbus ... ListNames`
/// (`(['org.freedesktop.DBus', 'org.mpris.MediaPlayer2.vlc'],)`).
pub fn first_mpris_player(list_names: &str) -> Option<String> {
    list_names.split('\'').find(|n| n.starts_with("org.mpris.MediaPlayer2.")).map(String::from)
}

fn say(inbox: &Inbox, text: String) {
    if let Ok(mut v) = inbox.lock() {
        v.push(text);
    }
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("timeout").arg("2").arg(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Louder or quieter by `delta` percent.
pub fn volume(inbox: &Inbox, delta: i32) {
    let inbox = inbox.clone();
    std::thread::spawn(move || {
        crate::status::change_volume(delta);
        match crate::status::volume() {
            Some(v) if v.muted => say(&inbox, "Volume: muted".into()),
            Some(v) => say(&inbox, format!("Volume: {}%", v.percent)),
            None => say(&inbox, "No sound server".into()),
        }
    });
}

pub fn mute(inbox: &Inbox) {
    let inbox = inbox.clone();
    std::thread::spawn(move || {
        crate::status::toggle_mute();
        match crate::status::volume() {
            Some(v) if v.muted => say(&inbox, "Volume: muted".into()),
            Some(v) => say(&inbox, format!("Volume: {}%", v.percent)),
            None => say(&inbox, "No sound server".into()),
        }
    });
}

/// Brighter or dimmer by `percent` of the range: with `brightnessctl` if it is installed, else
/// through logind (which lets the logged-in user set it without being root).
pub fn brightness(inbox: &Inbox, percent: i32) {
    let inbox = inbox.clone();
    std::thread::spawn(move || {
        let Some((name, cur, max)) = backlight_in(Path::new("/sys/class/backlight")) else {
            say(&inbox, "No screen brightness to change".into());
            return;
        };
        let next = next_brightness(cur, max, percent);
        let arg = format!("{}%{}", percent.abs(), if percent >= 0 { "+" } else { "-" });
        let done = run("brightnessctl", &["--min-value=1", "set", &arg]).is_some()
            || run(
                "gdbus",
                &["call", "--system", "--dest", "org.freedesktop.login1", "--object-path", "/org/freedesktop/login1/session/auto", "--method", "org.freedesktop.login1.Session.SetBrightness", "backlight", &name, &next.to_string()],
            )
            .is_some();
        if done {
            say(&inbox, format!("Brightness: {}%", next as u64 * 100 / max as u64));
        } else {
            say(&inbox, "Cannot change the brightness (install brightnessctl)".into());
        }
    });
}

/// A media key: `PlayPause`, `Next`, `Previous` or `Stop`, for the first player that answers.
pub fn media(command: &'static str) {
    std::thread::spawn(move || {
        let ctl = match command {
            "PlayPause" => "play-pause",
            "Next" => "next",
            "Previous" => "previous",
            _ => "stop",
        };
        if run("playerctl", &[ctl]).is_some() {
            return;
        }
        let names = run("gdbus", &["call", "--session", "--dest", "org.freedesktop.DBus", "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.ListNames"]).unwrap_or_default();
        if let Some(player) = first_mpris_player(&names) {
            let method = format!("org.mpris.MediaPlayer2.Player.{command}");
            let _ = run("gdbus", &["call", "--session", "--dest", &player, "--object-path", "/org/mpris/MediaPlayer2", "--method", &method]);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_steps_stay_between_one_and_the_maximum() {
        assert_eq!(next_brightness(50, 100, 5), 55);
        assert_eq!(next_brightness(50, 100, -5), 45);
        assert_eq!(next_brightness(98, 100, 5), 100, "not above the maximum");
        assert_eq!(next_brightness(3, 100, -5), 1, "never fully dark");
        assert_eq!(next_brightness(500, 1000, -10), 400);
        assert_eq!(next_brightness(5, 10, 5), 6, "a step is at least one unit on a coarse backlight");
    }

    #[test]
    fn the_backlight_is_read_from_sysfs_and_a_player_from_the_bus() {
        let dir = tempfile::tempdir().unwrap();
        let none = dir.path().join("empty");
        std::fs::create_dir_all(&none).unwrap();
        assert_eq!(backlight_in(&none), None, "a desktop has none");
        let b = dir.path().join("intel_backlight");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("brightness"), "480\n").unwrap();
        std::fs::write(b.join("max_brightness"), "960\n").unwrap();
        assert_eq!(backlight_in(dir.path()), Some(("intel_backlight".into(), 480, 960)));
        assert_eq!(
            first_mpris_player("(['org.freedesktop.DBus', ':1.5', 'org.mpris.MediaPlayer2.vlc', 'org.mpris.MediaPlayer2.mpv'],)"),
            Some("org.mpris.MediaPlayer2.vlc".into())
        );
        assert_eq!(first_mpris_player("(['org.freedesktop.DBus'],)"), None);
    }
}
