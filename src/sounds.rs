//! Event sounds: a short sound for the start of the session, a notification, a change of the
//! volume and the end of the session (`[sounds] enabled = true`). The files are looked for as
//! `NAME.oga`, `.ogg`, `.opus`, `.wav` in `~/.local/share/fde/sounds`, `/usr/local/share/fde/sounds`,
//! `/usr/share/fde/sounds` and next to the programs (`../share/fde/sounds`); they are played with
//! `pw-play`, `paplay` or `aplay`, whichever is installed. Licences: `data/sounds/README.md`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Startup,
    Logout,
    Notification,
    Volume,
}

impl Event {
    pub fn file_stem(self) -> &'static str {
        match self {
            Event::Startup => "startup",
            Event::Logout => "logout",
            Event::Notification => "notification",
            Event::Volume => "volume",
        }
    }
}

const EXTENSIONS: [&str; 4] = ["oga", "ogg", "opus", "wav"];

/// The folders searched, in order.
pub fn dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(base) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))) {
        v.push(base.join("fde/sounds"));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        v.push(exe_dir.join("../share/fde/sounds"));
    }
    v.push("/usr/local/share/fde/sounds".into());
    v.push("/usr/share/fde/sounds".into());
    v
}

/// The sound file of an event in the first folder that has one.
pub fn find(event: Event, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|d| EXTENSIONS.iter().map(|e| d.join(format!("{}.{e}", event.file_stem()))).find(|p| p.is_file()))
}

/// The player and its arguments for a file at `volume` (0 .. 1): `available` says which programs exist.
pub fn player_command(file: &Path, volume: f32, available: impl Fn(&str) -> bool) -> Option<(String, Vec<String>)> {
    let v = volume.clamp(0.0, 1.0);
    let f = file.to_string_lossy().into_owned();
    if available("pw-play") {
        return Some(("pw-play".into(), vec![format!("--volume={v:.2}"), f]));
    }
    if available("paplay") {
        return Some(("paplay".into(), vec![format!("--volume={}", (v * 65536.0) as u32), f]));
    }
    if available("aplay") && file.extension().is_some_and(|e| e == "wav") {
        return Some(("aplay".into(), vec!["-q".into(), f]));
    }
    None
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

static LAST: Mutex<Option<(Event, Instant)>> = Mutex::new(None);

/// Play the sound of an event if sounds are on and a file and a player exist. Does not wait; the
/// same event is not played again within a quarter of a second (holding a volume key).
pub fn play(event: Event) {
    let config = crate::config::Config::load().unwrap_or_default();
    play_with(event, &config.sounds, &dirs());
}

pub fn play_with(event: Event, cfg: &crate::config::Sounds, dirs: &[PathBuf]) {
    if !cfg.enabled || std::env::var_os("FDE_NO_SOUNDS").is_some() {
        return;
    }
    if let Ok(mut last) = LAST.lock() {
        if last.is_some_and(|(e, t)| e == event && t.elapsed() < Duration::from_millis(250)) {
            return;
        }
        *last = Some((event, Instant::now()));
    }
    let Some(file) = find(event, dirs) else { return };
    let Some((program, args)) = player_command(&file, cfg.volume, on_path) else { return };
    if let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        // reaped in the background so no zombie stays
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sound_file_is_found_in_the_first_folder_that_has_one() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(b.path().join("startup.ogg"), b"x").unwrap();
        std::fs::write(a.path().join("startup.wav"), b"x").unwrap();
        std::fs::write(b.path().join("logout.oga"), b"x").unwrap();
        let dirs = vec![a.path().to_path_buf(), b.path().to_path_buf()];
        assert_eq!(find(Event::Startup, &dirs), Some(a.path().join("startup.wav")));
        assert_eq!(find(Event::Logout, &dirs), Some(b.path().join("logout.oga")));
        assert_eq!(find(Event::Volume, &dirs), None);
    }

    #[test]
    fn the_player_is_the_first_installed_one_and_gets_the_volume() {
        let f = Path::new("/x/a.oga");
        let all = |_: &str| true;
        assert_eq!(player_command(f, 0.5, all), Some(("pw-play".into(), vec!["--volume=0.50".into(), "/x/a.oga".into()])));
        let pa = |p: &str| p == "paplay";
        assert_eq!(player_command(f, 2.0, pa), Some(("paplay".into(), vec!["--volume=65536".into(), "/x/a.oga".into()])));
        // aplay cannot play Ogg
        assert_eq!(player_command(f, 1.0, |p| p == "aplay"), None);
        assert!(player_command(Path::new("/x/a.wav"), 1.0, |p| p == "aplay").is_some());
        assert_eq!(player_command(f, 1.0, |_| false), None);
    }

    #[test]
    fn nothing_is_played_when_sounds_are_off() {
        let cfg = crate::config::Sounds { enabled: false, volume: 1.0 };
        play_with(Event::Notification, &cfg, &[]);
    }
}

#[cfg(test)]
mod licence {
    /// Every shipped sound has a row in the licence table of its folder.
    #[test]
    fn every_shipped_sound_has_a_licence_row() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/sounds");
        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".md") {
                continue;
            }
            assert!(readme.contains(&format!("`{name}`")), "{name} has no licence row in data/sounds/README.md");
        }
        assert!(readme.contains("CC0"));
    }
}
